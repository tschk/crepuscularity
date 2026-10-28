use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_semantic::{SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::scope::ScopeFlags;
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    For,
    Index,
}

struct Binding {
    kind: Kind,
    alias: String,
}

struct Piece {
    span: Span,
    parenthesize: bool,
}

struct Site {
    span: Span,
    symbol: SymbolId,
    braces: bool,
    each: Piece,
    children: Piece,
    fallback: Option<Piece>,
    comments: Vec<Span>,
}

struct Helper {
    name: String,
    kind: Kind,
    alias: String,
}

struct Names(HashSet<String>);

impl<'a> Visit<'a> for Names {
    fn visit_identifier_reference(&mut self, node: &IdentifierReference<'a>) {
        self.0.insert(node.name.to_string());
    }
    fn visit_binding_identifier(&mut self, node: &BindingIdentifier<'a>) {
        self.0.insert(node.name.to_string());
    }
}

struct Suspends(bool);

impl<'a> Visit<'a> for Suspends {
    fn visit_await_expression(&mut self, _: &AwaitExpression<'a>) {
        self.0 = true;
    }
    fn visit_yield_expression(&mut self, _: &YieldExpression<'a>) {
        self.0 = true;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
}

struct Namer<'s> {
    reserved: HashSet<String>,
    source: &'s str,
}

impl Namer<'_> {
    fn fresh(&mut self, base: &str) -> String {
        let mut name = base.to_string();
        while self.reserved.contains(&name) || self.source.contains(&name) {
            name.push('_');
        }
        self.reserved.insert(name.clone());
        name
    }
}

struct Renderer<'s> {
    source: &'s str,
    sites: &'s [Site],
    helpers: &'s [Helper],
    symbols: &'s [SymbolId],
}

impl Renderer<'_> {
    fn range(&self, start: u32, end: u32) -> String {
        let mut out = String::new();
        let mut cursor = start as usize;
        for site in self.sites {
            if site.span.start < start || site.span.end > end || (site.span.start as usize) < cursor
            {
                continue;
            }
            out.push_str(&self.source[cursor..site.span.start as usize]);
            out.push_str(&self.site(site));
            cursor = site.span.end as usize;
        }
        out.push_str(&self.source[cursor..end as usize]);
        out
    }

    fn lazy(&self, piece: &Piece) -> String {
        let text = self.range(piece.span.start, piece.span.end);
        if piece.parenthesize {
            format!("()=>({text})")
        } else {
            format!("()=>{text}")
        }
    }

    fn site(&self, site: &Site) -> String {
        let index = self
            .symbols
            .iter()
            .position(|symbol| *symbol == site.symbol)
            .expect("site symbol has a helper");
        let mut call = format!(
            "{}({},{}",
            self.helpers[index].name,
            self.lazy(&site.each),
            self.lazy(&site.children)
        );
        if let Some(fallback) = &site.fallback {
            call.push(',');
            call.push_str(&self.lazy(fallback));
        }
        call.push(')');
        if !site.comments.is_empty() {
            let mut commented = String::new();
            for comment in &site.comments {
                commented.push_str(&self.source[comment.start as usize..comment.end as usize]);
                commented.push('\n');
            }
            commented.push_str(&call);
            call = if site.braces {
                commented
            } else {
                format!("({commented})")
            };
        }
        if site.braces {
            format!("{{{call}}}")
        } else {
            call
        }
    }
}

fn bare(expression: &Expression<'_>) -> bool {
    matches!(
        expression,
        Expression::Identifier(_)
            | Expression::CallExpression(_)
            | Expression::StaticMemberExpression(_)
            | Expression::ComputedMemberExpression(_)
            | Expression::ParenthesizedExpression(_)
            | Expression::ArrayExpression(_)
            | Expression::JSXElement(_)
            | Expression::JSXFragment(_)
            | Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::TemplateLiteral(_)
    )
}

fn piece(
    expression: &Expression<'_>,
    role: &str,
    tag: &str,
    always_parenthesize: bool,
) -> Result<Piece, String> {
    let mut suspends = Suspends(false);
    suspends.visit_expression(expression);
    if suspends.0 {
        return Err(format!(
            "`<{tag}>` {role} is evaluated lazily and cannot contain a top-level await or yield"
        ));
    }
    Ok(Piece {
        span: expression.span(),
        parenthesize: always_parenthesize || !bare(expression),
    })
}

fn parse_site(
    element: &JSXElement<'_>,
    tag: &str,
) -> Result<(Piece, Piece, Option<Piece>), String> {
    let opening = &element.opening_element;
    if opening.type_arguments.is_some() {
        return Err(format!("`<{tag}>` does not accept JSX type arguments"));
    }
    let mut each = None;
    let mut fallback = None;
    let mut explicit = None;
    for item in &opening.attributes {
        let JSXAttributeItem::Attribute(attribute) = item else {
            return Err(format!(
                "`<{tag}>` does not support spread props; pass `each`, an optional `fallback` and one callback child"
            ));
        };
        let JSXAttributeName::Identifier(name) = &attribute.name else {
            return Err(format!("`<{tag}>` does not support namespaced props"));
        };
        let name = name.name.as_str();
        let slot = match name {
            "each" => &mut each,
            "fallback" => &mut fallback,
            "children" => &mut explicit,
            other => {
                return Err(format!(
                    "`<{tag}>` received unexpected prop `{other}`; supported props are `each`, `fallback` and `children`"
                ))
            }
        };
        if slot.is_some() {
            return Err(format!("`<{tag}>` received duplicate prop `{name}`"));
        }
        let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
            return Err(format!(
                "`<{tag}>` prop `{name}` requires an expression value"
            ));
        };
        let Some(expression) = container.expression.as_expression() else {
            return Err(format!("`<{tag}>` prop `{name}` has an empty expression"));
        };
        *slot = Some(expression);
    }
    let mut body = None;
    for child in &element.children {
        match child {
            JSXChild::Text(text) => {
                if !text.value.as_str().trim().is_empty() {
                    return Err(format!(
                        "`<{tag}>` children must be one callback expression, not text"
                    ));
                }
            }
            JSXChild::ExpressionContainer(container) => {
                if let Some(expression) = container.expression.as_expression() {
                    if body.replace(expression).is_some() {
                        return Err(format!(
                            "`<{tag}>` accepts exactly one callback child expression, found several"
                        ));
                    }
                }
            }
            JSXChild::Spread(_) => {
                return Err(format!("`<{tag}>` does not support spread children"));
            }
            JSXChild::Element(_) | JSXChild::Fragment(_) => {
                return Err(format!(
                    "`<{tag}>` children must be one callback expression, not a JSX element"
                ));
            }
        }
    }
    let children = match (explicit, body) {
        (Some(_), Some(_)) => {
            return Err(format!(
                "`<{tag}>` received both a `children` prop and child expressions; use only one"
            ))
        }
        (Some(expression), None) => {
            if element.closing_element.is_some() {
                return Err(format!(
                    "`<{tag}>` with a `children` prop must be self-closing"
                ));
            }
            expression
        }
        (None, Some(expression)) => expression,
        (None, None) => {
            return Err(format!(
                "`<{tag}>` requires a callback child or `children` prop"
            ))
        }
    };
    let Some(each) = each else {
        return Err(format!("`<{tag}>` requires an `each` prop"));
    };
    Ok((
        piece(each, "`each`", tag, false)?,
        piece(children, "children", tag, true)?,
        fallback
            .map(|expression| piece(expression, "`fallback`", tag, false))
            .transpose()?,
    ))
}

fn helper_source(helper: &Helper, typed: bool, target: &Target<'_>, params: &Params) -> String {
    let Target { host, child } = target;
    let Params {
        each,
        children,
        fallback,
        generics: (t, u, f),
    } = params;
    let (name, alias) = (&helper.name, &helper.alias);
    let fields = format!("get each(){{return {each}()}},get children(){{return {children}()}}");
    let body = format!(
        "return {host}({alias}{},{fallback}?{{{fields},get fallback(){{return {fallback}?.()}}}}:{{{fields}}});",
        if typed { format!("<{t},{u},{f}>") } else { String::new() }
    );
    if !typed {
        return format!("function {name}({each},{children},{fallback}){{{body}}}\n");
    }
    let (item_type, index_type) = match helper.kind {
        Kind::For => (t.to_string(), "()=>number".to_string()),
        Kind::Index => (format!("()=>{t}"), "number".to_string()),
    };
    format!(
        "function {name}<{t},{u} extends {child},{f} extends {child}={u}>({each}:()=>readonly {t}[]|false|null|undefined,{children}:()=>((item:{item_type},index:{index_type})=>{u}),{fallback}?:()=>{f}):{child}{{{body}}}\n"
    )
}

struct Target<'a> {
    host: &'a str,
    child: &'a str,
}

struct Params {
    each: String,
    children: String,
    fallback: String,
    generics: (String, String, String),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LoweredLists {
    pub source: String,
    pub helpers: Vec<String>,
}

pub(crate) fn lower(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
) -> Result<Option<LoweredLists>, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(
        path.with_extension(
            path.extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase(),
        ),
    )
    .map_err(|error| error.to_string())?
    .with_module(true);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("module parse failed: {:?}", parsed.errors));
    }
    if crate::solid_source::selector(source, &parsed.program)?.is_empty() {
        return Ok(None);
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(format!("module semantic errors: {:?}", built.errors));
    }
    let semantic = built.semantic;
    let mut bindings = HashMap::new();
    let mut type_only = Vec::new();
    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        if import.source.value != "solid-js" {
            continue;
        }
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        for specifier in specifiers {
            let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                continue;
            };
            let kind = match specifier.imported.name().as_str() {
                "For" => Kind::For,
                "Index" => Kind::Index,
                _ => continue,
            };
            let alias = specifier.local.name.to_string();
            if import.import_kind == ImportOrExportKind::Type
                || specifier.import_kind == ImportOrExportKind::Type
            {
                type_only.push(alias);
            } else if let Some(symbol) = specifier.local.symbol_id.get() {
                bindings.insert(symbol, Binding { kind, alias });
            }
        }
    }
    if bindings.is_empty() && type_only.is_empty() {
        return Ok(None);
    }
    let nodes = semantic.nodes();
    let scoping = semantic.scoping();
    let mut sites = Vec::new();
    for node in nodes.iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        let JSXElementName::IdentifierReference(name) = &element.opening_element.name else {
            continue;
        };
        let symbol = name
            .reference_id
            .get()
            .and_then(|reference| scoping.get_reference(reference).symbol_id());
        let type_alias = match symbol {
            Some(_) => None,
            None => type_only.iter().find(|alias| *alias == name.name.as_str()),
        };
        if let Some(alias) = type_alias {
            return Err(format!(
                "`{alias}` is imported as a type from solid-js; import it as a value to use `<{alias}>`"
            ));
        }
        let Some(symbol) = symbol else {
            continue;
        };
        if !bindings.contains_key(&symbol) {
            continue;
        }
        let (each, children, fallback) = parse_site(element, name.name.as_str())?;
        let braces = matches!(
            nodes.parent_kind(node.id()),
            AstKind::JSXElement(_) | AstKind::JSXFragment(_) | AstKind::JSXAttribute(_)
        );
        sites.push(Site {
            span: element.span,
            symbol,
            braces,
            each,
            children,
            fallback,
            comments: Vec::new(),
        });
    }
    if sites.is_empty() {
        return Ok(None);
    }
    sites.sort_by(|a, b| {
        a.span
            .start
            .cmp(&b.span.start)
            .then(b.span.end.cmp(&a.span.end))
    });
    for site in &mut sites {
        let pieces = [
            Some(&site.each),
            Some(&site.children),
            site.fallback.as_ref(),
        ];
        site.comments = parsed
            .program
            .comments
            .iter()
            .map(|comment| comment.span)
            .filter(|comment| {
                site.span.start <= comment.start
                    && comment.end <= site.span.end
                    && !pieces.iter().flatten().any(|piece| {
                        piece.span.start <= comment.start && comment.end <= piece.span.end
                    })
            })
            .collect();
    }

    let mut names = Names(scoping.symbol_names().map(str::to_string).collect());
    names.visit_program(&parsed.program);
    let mut namer = Namer {
        reserved: names.0,
        source,
    };
    let typed = source_type.is_typescript();
    let (module, child_import) = match target {
        MoonshineModuleTarget::Dom => ("@tschk/moonshine-solid/h", "DOMChild"),
        MoonshineModuleTarget::Ssr => ("@tschk/moonshine-solid/ssr", "SSRChild"),
    };
    let host_local = namer.fresh("__crepusListH");
    let child_local = namer.fresh("__CrepusListChild");
    let params = Params {
        each: namer.fresh("__crepusEach"),
        children: namer.fresh("__crepusChildren"),
        fallback: namer.fresh("__crepusFallback"),
        generics: (
            namer.fresh("__CrepusT"),
            namer.fresh("__CrepusU"),
            namer.fresh("__CrepusF"),
        ),
    };
    let mut symbols = Vec::new();
    let mut helpers = Vec::new();
    for site in &sites {
        if symbols.contains(&site.symbol) {
            continue;
        }
        let binding = &bindings[&site.symbol];
        symbols.push(site.symbol);
        let base = match binding.kind {
            Kind::For => "__crepusSolidFor",
            Kind::Index => "__crepusSolidIndex",
        };
        helpers.push(Helper {
            name: namer.fresh(base),
            kind: binding.kind,
            alias: binding.alias.clone(),
        });
    }
    let target = Target {
        host: &host_local,
        child: &child_local,
    };
    let mut header = if typed {
        format!(
            "import {{h as {host_local},type {} as {child_local}}} from '{}';\n",
            child_import, module
        )
    } else {
        format!("import {{h as {host_local}}} from '{module}';\n")
    };
    for helper in &helpers {
        header.push_str(&helper_source(helper, typed, &target, &params));
    }
    let insert = parsed
        .program
        .body
        .first()
        .map(|statement| statement.span().start as usize)
        .ok_or("Solid list lowering requires a module body")?;
    let renderer = Renderer {
        source,
        sites: &sites,
        helpers: &helpers,
        symbols: &symbols,
    };
    let source = format!(
        "{}{header}{}",
        &source[..insert],
        renderer.range(insert as u32, source.len() as u32)
    );
    Ok(Some(LoweredLists {
        source,
        helpers: helpers.into_iter().map(|helper| helper.name).collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: &str = "/** @jsxImportSource solid-js */\n";
    const IMPORT: &str = "import {For, Index} from 'solid-js';\n";

    fn run(
        body: &str,
        file: &str,
        target: MoonshineModuleTarget,
    ) -> Result<Option<String>, String> {
        lower(&format!("{P}{body}"), Path::new(file), target)
            .map(|lowered| lowered.map(|lowered| lowered.source))
    }

    fn ok(body: &str) -> String {
        run(body, "a.tsx", MoonshineModuleTarget::Dom)
            .unwrap()
            .expect("lowered")
    }

    fn err(body: &str) -> String {
        run(body, "a.tsx", MoonshineModuleTarget::Dom).unwrap_err()
    }

    fn assert_complete(output: &str, file: &str) {
        let allocator = Allocator::default();
        let source_type = SourceType::from_path(file).unwrap().with_module(true);
        let parsed = Parser::new(&allocator, output, source_type).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
        assert_eq!(
            lower(output, Path::new(file), MoonshineModuleTarget::Dom).unwrap(),
            None,
            "{output}"
        );
    }

    #[test]
    fn typed_dom_helper_wraps_lazy_factories() {
        let output = ok(&format!(
            "{IMPORT}export const App = () => <F.x/>;\nexport const B = () => <For each={{items()}} fallback={{<p/>}}>{{(row, index) => <Child/>}}</For>;"
        ));
        assert!(output.contains("@tschk/moonshine-solid/h"), "{output}");
        assert!(output.contains("export const B = () => __crepusSolidFor(()=>items(),()=>((row, index) => <Child/>),()=><p/>);"), "{output}");
        assert_complete(&output, "a.tsx");
    }

    #[test]
    fn ssr_and_index_use_their_own_runtime_and_accessor_shapes() {
        let output = run(
            &format!(
                "{IMPORT}export const A = () => <Index each={{xs}}>{{(row, i) => <b/>}}</Index>;"
            ),
            "a.tsx",
            MoonshineModuleTarget::Ssr,
        )
        .unwrap()
        .unwrap();
        assert!(output.contains("import {h as __crepusListH,type SSRChild as __CrepusListChild} from '@tschk/moonshine-solid/ssr';"), "{output}");
        assert!(
            output.contains("(item:()=>__CrepusT,index:number)=>__CrepusU"),
            "{output}"
        );
        assert!(
            output.contains("Index<__CrepusT,__CrepusU,__CrepusF>"),
            "{output}"
        );
        assert!(
            output.contains("__crepusSolidIndex(()=>xs,()=>((row, i) => <b/>))"),
            "{output}"
        );
        assert!(!output.contains("For<"), "{output}");
    }

    #[test]
    fn javascript_helper_has_no_typescript_syntax() {
        let output = run(
            &format!("{IMPORT}export const A = () => <For each={{xs}}>{{(row) => row}}</For>;"),
            "a.jsx",
            MoonshineModuleTarget::Dom,
        )
        .unwrap()
        .unwrap();
        assert!(
            output.contains("import {h as __crepusListH} from '@tschk/moonshine-solid/h';"),
            "{output}"
        );
        assert!(
            !output.contains("type ") && !output.contains("<__Crepus"),
            "{output}"
        );
        assert_complete(&output, "a.jsx");
    }

    #[test]
    fn aliases_lower_and_shadows_stay_untouched() {
        let output = ok("import {For as Loop} from 'solid-js';\nexport const A = () => <Loop each={xs}>{(r) => r}</Loop>;");
        assert!(
            output.contains("Loop<__CrepusT,__CrepusU,__CrepusF>"),
            "{output}"
        );
        assert!(output.contains("__crepusSolidFor(()=>xs,"), "{output}");
        assert_complete(&output, "a.tsx");
        let shadowed = "import {For} from 'solid-js';\nexport const A = (For: any) => <For each={xs}>{(r) => r}</For>;\nfunction B() { const For = () => null; return <For each={xs}/>; }\nconst Index = 1; export const C = <Index/>;";
        assert_eq!(
            run(shadowed, "a.tsx", MoonshineModuleTarget::Dom).unwrap(),
            None
        );
    }

    #[test]
    fn requires_exact_pragma_and_real_builtin_tags() {
        let plain = "import {For} from 'solid-js';\nexport const A = () => <For each={xs}>{(r) => r}</For>;";
        assert_eq!(
            lower(plain, Path::new("a.tsx"), MoonshineModuleTarget::Dom).unwrap(),
            None
        );
        assert_eq!(
            run(
                "import {For} from 'solid-js';\nexport const A = () => <div/>;",
                "a.tsx",
                MoonshineModuleTarget::Dom
            )
            .unwrap(),
            None
        );
        assert_eq!(
            run(
                "import {For} from 'other';\nexport const A = () => <For each={xs}/>;",
                "a.tsx",
                MoonshineModuleTarget::Dom
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn nested_lists_in_callbacks_fallbacks_and_each_are_lowered() {
        let output = ok(&format!(
            "{IMPORT}export const A = () => <For each={{<Index each={{a}}>{{(x) => x}}</Index>}} fallback={{<Index each={{b}}>{{(y) => <For each={{y()}}>{{(z) => z}}</For>}}</Index>}}>{{(row) => <ul><For each={{row.kids}}>{{(kid) => <li>{{kid}}</li>}}</For></ul>}}</For>;"
        ));
        assert!(
            !output.contains("</For>") && !output.contains("</Index>"),
            "{output}"
        );
        assert_eq!(
            output.matches("function __crepusSolid").count(),
            2,
            "{output}"
        );
        assert!(
            output.contains(
                "<ul>{__crepusSolidFor(()=>row.kids,()=>((kid) => <li>{kid}</li>))}</ul>"
            ),
            "{output}"
        );
        assert!(output.contains("()=>__crepusSolidIndex(()=>b,()=>((y) => __crepusSolidFor(()=>y(),()=>((z) => z))))"), "{output}");
        assert_complete(&output, "a.tsx");
    }

    #[test]
    fn parent_context_decides_expression_braces() {
        let output = ok(&format!(
            "{IMPORT}export const A = () => <div title=<For each={{a}} children={{f}}/> data={{<For each={{a}} children={{f}}/>}}><For each={{a}} children={{f}}/><>{{<For each={{a}} children={{f}}/>}}<For each={{a}} children={{f}}/></></div>;\nexport function B() {{ return <For each={{a}}>{{g}}</For>; }}\nexport const C = () => (<For each={{a}}>{{g.h}}</For>);"
        ));
        let call = |c: &str| format!("__crepusSolidFor(()=>a,()=>({c}))");
        assert!(
            output.contains(&format!(
                "title={{{}}} data={{{}}}>{{{}}}<>{{{}}}{{{}}}</>",
                call("f"),
                call("f"),
                call("f"),
                call("f"),
                call("f")
            )),
            "{output}"
        );
        assert!(
            output.contains(&format!("return {};", call("g"))),
            "{output}"
        );
        assert!(
            output.contains(&format!("= () => ({});", call("g.h"))),
            "{output}"
        );
        assert_complete(&output, "a.tsx");
    }

    #[test]
    fn complex_expressions_are_parenthesized_and_comments_survive() {
        let output = ok(&format!(
            "{IMPORT}export const A = () => <div><For /* keep-a */ each={{a ?? b}} // keep-b\n fallback={{/* keep-c */ x as any}}>{{/* keep-d */ g}}</For></div>;\nexport const B = () => {{ return <For /* ret */ each={{a}} children={{f}}/>; }};"
        ));
        assert!(output.contains("{/* keep-a */\n// keep-b\n/* keep-c */\n/* keep-d */\n__crepusSolidFor(()=>(a ?? b),()=>(g),()=>(x as any))}"), "{output}");
        assert!(
            output.contains("return (/* ret */\n__crepusSolidFor(()=>a,()=>(f)));"),
            "{output}"
        );
        assert_complete(&output, "a.tsx");
    }

    #[test]
    fn license_and_directive_stay_before_inserted_helpers() {
        let output = run(
            &format!("'use strict';\n/* license */\n{IMPORT}export const A = () => <For each={{a}} children={{f}}/>;"),
            "a.tsx",
            MoonshineModuleTarget::Dom,
        )
        .unwrap()
        .unwrap();
        assert!(
            output.starts_with(&format!(
                "{P}'use strict';\n/* license */\nimport {{h as __crepusListH"
            )),
            "{output}"
        );
    }

    #[test]
    fn invalid_shapes_fail_with_distinct_errors() {
        let call = |attrs: &str, children: &str| {
            format!("{IMPORT}export const A = () => <For {attrs}>{children}</For>;")
        };
        let cases = [
            (call("{...p}", "{f}"), "spread props"),
            (call("each={a} each={b}", "{f}"), "duplicate prop `each`"),
            (call("each={a} key={k}", "{f}"), "unexpected prop `key`"),
            (call("a:b={a} each={a}", "{f}"), "namespaced"),
            (call("each=\"a\"", "{f}"), "requires an expression value"),
            (call("", "{f}"), "requires an `each` prop"),
            (call("each={a}", ""), "requires a callback child"),
            (call("each={a}", "{f}{g}"), "exactly one callback child"),
            (call("each={a}", "text"), "not text"),
            (call("each={a}", "<li/>"), "not a JSX element"),
            (call("each={a}", "{...f}"), "spread children"),
            (call("each={a} children={f}", "{g}"), "both a `children` prop"),
            (call("each={a} children={f}", ""), "must be self-closing"),
            (format!("{IMPORT}export const A = async () => <For each={{await a}}>{{f}}</For>;"), "await or yield"),
            (format!("{IMPORT}export const A = () => <For<number> each={{a}}>{{f}}</For>;"), "type arguments"),
            ("import type {For} from 'solid-js';\nexport const A = () => <For each={a}>{f}</For>;".into(), "imported as a type"),
            ("import {type For} from 'solid-js';\nexport const A = () => <For each={a}>{f}</For>;".into(), "imported as a type"),
        ];
        for (source, expected) in cases {
            let message = err(&source);
            assert!(message.contains(expected), "{expected}: {message}");
        }
    }

    #[test]
    fn fresh_names_avoid_authored_and_decoded_bindings() {
        let output = ok(&format!(
            "{IMPORT}const __crepusListH = 1; function \\u005f_crepusSolidFor() {{}} let __CrepusT, __crepusEach; export const A = () => <For each={{a}} children={{f}}/>;\nexport const B = () => <Index each={{__crepusSolidIndex}} children={{f}}/>;"
        ));
        assert!(output.contains("h as __crepusListH_,"), "{output}");
        assert!(
            output.contains("function __crepusSolidFor_<__CrepusT_,"),
            "{output}"
        );
        assert!(output.contains("function __crepusSolidIndex_<"), "{output}");
        assert!(output.contains("__crepusEach_:()=>"), "{output}");
        assert_complete(&output, "a.tsx");
    }

    #[test]
    fn one_helper_per_imported_symbol() {
        let output = ok("import {For, For as Again, Index} from 'solid-js';\nexport const A = () => <><For each={a} children={f}/><For each={b} children={f}/><Again each={c} children={f}/><Index each={d} children={f}/></>;");
        assert_eq!(
            output.matches("function __crepusSolid").count(),
            3,
            "{output}"
        );
        assert!(output.contains("function __crepusSolidFor_<"), "{output}");
        assert_complete(&output, "a.tsx");
    }
}
