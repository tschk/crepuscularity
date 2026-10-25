//! Lower authored Svelte snippets to accessor parameters and owned render calls.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::{collections::HashSet, path::Path};

fn optional_member(expression: &Expression<'_>) -> bool {
    match expression.get_inner_expression() {
        Expression::StaticMemberExpression(member) => {
            member.optional || optional_member(&member.object)
        }
        Expression::ComputedMemberExpression(member) => {
            member.optional || optional_member(&member.object)
        }
        Expression::ChainExpression(_) => true,
        _ => false,
    }
}

/// Check source binding targets before SSR removes their generated setters.
#[cfg(test)]
pub(crate) fn validate_bindings(
    source: &str,
    path: &Path,
    snippet_names: &[String],
    inline_snippet_names: &[String],
    markers: &[&str],
) -> Result<(), String> {
    validate_bindings_with_policy(
        source,
        path,
        snippet_names,
        inline_snippet_names,
        markers,
        crate::svelte_javascript::IrPolicy::Typed,
    )
}

pub(crate) fn validate_bindings_with_policy(
    source: &str,
    path: &Path,
    snippet_names: &[String],
    inline_snippet_names: &[String],
    markers: &[&str],
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<(), String> {
    if (snippet_names.is_empty() && inline_snippet_names.is_empty()) || markers.is_empty() {
        return Ok(());
    }
    let fail = |message: &str| format!("{}: Svelte snippet {message}", path.display());
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(fail("has invalid binding syntax"));
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(fail("has invalid binding scope"));
    }
    let semantic = built.semantic;
    let nodes = semantic.nodes();
    let mut parameters = HashSet::new();
    for node in nodes.iter() {
        let AstKind::Function(function) = node.kind() else {
            continue;
        };
        let Some(id) = &function.id else {
            continue;
        };
        let inline = inline_snippet_names
            .iter()
            .any(|name| name == id.name.as_str());
        if !inline && !snippet_names.iter().any(|name| name == id.name.as_str()) {
            continue;
        }
        // Inline names are private and unique; root names can be shadowed by script functions.
        if !inline && !matches!(nodes.parent_kind(node.id()), AstKind::FunctionBody(_)) {
            continue;
        }
        let Some(owner) = nodes
            .ancestors(node.id())
            .find(|parent| matches!(parent.kind(), AstKind::Function(_)))
        else {
            continue;
        };
        if !inline
            && !matches!(
                nodes.parent_kind(owner.id()),
                AstKind::ExportDefaultDeclaration(_)
            )
        {
            continue;
        }
        for parameter in &function.params.items {
            if let BindingPattern::BindingIdentifier(id) = &parameter.pattern {
                parameters.insert(id.symbol_id.get().unwrap());
            }
        }
    }
    for node in nodes.iter() {
        let AstKind::JSXAttribute(attribute) = node.kind() else {
            continue;
        };
        if !matches!(&attribute.name,JSXAttributeName::Identifier(name) if markers.contains(&name.name.as_str()))
        {
            continue;
        }
        let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
            continue;
        };
        let Some(expression) = container.expression.as_expression() else {
            continue;
        };
        let Expression::Identifier(id) = expression.get_inner_expression() else {
            continue;
        };
        if semantic
            .scoping()
            .get_reference(id.reference_id.get().unwrap())
            .symbol_id()
            .is_some_and(|symbol| parameters.contains(&symbol))
        {
            return Err(fail("parameters cannot be binding targets"));
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn transform(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    component_name: &str,
    snippet_names: &[String],
    inline_snippet_names: &[String],
    render_component: Option<&str>,
) -> Result<String, String> {
    transform_with_policy(
        source,
        path,
        target,
        component_name,
        (snippet_names, inline_snippet_names),
        render_component,
        crate::svelte_javascript::IrPolicy::Typed,
    )
}

pub(crate) fn transform_with_policy(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    component_name: &str,
    (snippet_names, inline_snippet_names): (&[String], &[String]),
    render_component: Option<&str>,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<String, String> {
    if snippet_names.is_empty() && inline_snippet_names.is_empty() && render_component.is_none() {
        return Ok(source.to_owned());
    }
    let fail = |message: &str| format!("{}: Svelte snippet {message}", path.display());
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(fail("has invalid syntax"));
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(fail("has invalid scope or a conflicting declaration"));
    }
    let semantic = built.semantic;
    let nodes = semantic.nodes();
    let component = nodes
        .iter()
        .find_map(|node| match node.kind() {
            AstKind::Function(function)
                if function
                    .id
                    .as_ref()
                    .is_some_and(|id| id.name == component_name) =>
            {
                Some(function)
            }
            _ => None,
        })
        .ok_or_else(|| fail("component declaration is missing"))?;
    let body = component
        .body
        .as_ref()
        .ok_or_else(|| fail("component body is missing"))?;
    let mut unbound = "__crepusSvelteSnippetUnbound".to_owned();
    while source.contains(&unbound) {
        unbound.push('_');
    }
    let mut unbound_used = false;
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut parameters = HashSet::new();
    let mut seen = HashSet::new();
    for name in snippet_names.iter().chain(inline_snippet_names) {
        if !seen.insert(name) {
            return Err(fail("names must be unique"));
        }
        let inline = inline_snippet_names.contains(name);
        let declarations:Vec<_>=nodes.iter().filter_map(|node|match node.kind(){AstKind::Function(function) if function.id.as_ref().is_some_and(|id|id.name==name.as_str()) && (inline || matches!(nodes.parent_kind(node.id()),AstKind::FunctionBody(parent) if parent.span==body.span))=>Some(function),_=>None}).collect();
        if declarations.len() != 1 {
            return Err(fail(
                "must have exactly one declaration at component top level; script name collisions are unsupported",
            ));
        }
        let function = declarations[0];
        if function.body.is_none()
            || function.r#async
            || function.generator
            || function.type_parameters.is_some()
            || function.this_param.is_some()
            || function.params.rest.is_some()
        {
            return Err(fail(
                "requires a synchronous function with simple identifier parameters",
            ));
        }
        for parameter in &function.params.items {
            let BindingPattern::BindingIdentifier(id) = &parameter.pattern else {
                return Err(fail("destructured parameters are unsupported"));
            };
            if parameter.optional
                || parameter.initializer.is_some()
                || !parameter.decorators.is_empty()
                || parameter.accessibility.is_some()
                || parameter.readonly
                || parameter.r#override
            {
                return Err(fail(
                    "optional, default, rest, and decorated parameters are unsupported",
                ));
            }
            parameters.insert(id.symbol_id.get().unwrap());
            if matches!(target, MoonshineModuleTarget::Ssr) {
                if let Some(annotation) = &parameter.type_annotation {
                    let end = annotation.span.end as usize;
                    edits.push((end, end, "=void 0 as any".into()));
                } else {
                    edits.push((
                        id.span.end as usize,
                        id.span.end as usize,
                        ":any=void 0".into(),
                    ));
                }
                continue;
            }
            if let Some(annotation) = &parameter.type_annotation {
                let span = annotation.type_annotation.span();
                edits.push((
                    span.start as usize,
                    span.start as usize,
                    "{readonly value:".into(),
                ));
                edits.push((
                    span.end as usize,
                    span.end as usize,
                    "}={get value():any{return void 0}}".into(),
                ));
            } else {
                edits.push((
                    id.span.end as usize,
                    id.span.end as usize,
                    ":{readonly value:any}={get value():any{return void 0}}".into(),
                ));
            }
        }
    }
    for node in nodes.iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        let reference = semantic
            .scoping()
            .get_reference(id.reference_id.get().unwrap());
        if !reference
            .symbol_id()
            .is_some_and(|symbol| parameters.contains(&symbol))
        {
            continue;
        }
        if reference.is_write() {
            return Err(fail("parameters cannot be reassigned"));
        }
        if matches!(target, MoonshineModuleTarget::Ssr) {
            continue;
        }
        if reference.is_type() {
            return Err(fail("parameter type queries are unsupported"));
        }
        let mut reference_span = id.span;
        let mut direct_call = false;
        for parent in nodes.ancestors(node.id()) {
            match parent.kind() {
                AstKind::ParenthesizedExpression(_)
                | AstKind::TSAsExpression(_)
                | AstKind::TSSatisfiesExpression(_)
                | AstKind::TSNonNullExpression(_)
                | AstKind::TSTypeAssertion(_)
                | AstKind::TSInstantiationExpression(_) => {
                    reference_span = parent.kind().span();
                    continue;
                }
                AstKind::CallExpression(call) => direct_call = call.callee.span() == reference_span,
                AstKind::TaggedTemplateExpression(tag) => {
                    direct_call = tag.tag.span() == reference_span
                }
                _ => {}
            }
            break;
        }
        if direct_call {
            let mut wrapped =
                source[reference_span.start as usize..reference_span.end as usize].to_owned();
            wrapped.replace_range(
                (id.span.start - reference_span.start) as usize
                    ..(id.span.end - reference_span.start) as usize,
                &format!("{}.value", id.name),
            );
            edits.push((
                reference_span.start as usize,
                reference_span.end as usize,
                format!("{unbound}({wrapped})"),
            ));
            unbound_used = true;
            continue;
        }
        let value = match nodes.parent_kind(node.id()) {
            AstKind::ObjectProperty(property) if property.shorthand => {
                crate::svelte_expression::shorthand_property(
                    id.name.as_str(),
                    &format!("{}.value", id.name),
                )
            }
            _ => format!("{}.value", id.name),
        };
        edits.push((id.span.start as usize, id.span.end as usize, value));
    }
    let mut render_used = false;
    let mut memo_needed = false;
    let mut cell_needed = false;
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name) {
            name.push('_');
        }
        name
    };
    let memo = fresh("__crepusSvelteSnippetMemo");
    let cell = fresh("__crepusSvelteSnippetCell");
    let derived = fresh("__crepusSvelteSnippetDerived");
    if let Some(alias) = render_component {
        for node in nodes.iter() {
            let AstKind::JSXElement(element) = node.kind() else {
                continue;
            };
            if !matches!(&element.opening_element.name,JSXElementName::IdentifierReference(id) if id.name==alias)
            {
                continue;
            }
            if !element.children.is_empty() || element.opening_element.attributes.len() != 1 {
                return Err(fail(
                    "render marker requires exactly one call and no children",
                ));
            }
            let JSXAttributeItem::Attribute(attribute) = &element.opening_element.attributes[0]
            else {
                return Err(fail("render spreads are unsupported"));
            };
            if !matches!(&attribute.name,JSXAttributeName::Identifier(name) if name.name=="call") {
                return Err(fail("render marker requires a call"));
            }
            let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
                return Err(fail("render marker requires a call"));
            };
            let Some(Expression::ArrowFunctionExpression(arrow)) =
                container.expression.as_expression()
            else {
                return Err(fail("render marker requires a call thunk"));
            };
            if arrow.r#async || !arrow.params.items.is_empty() || arrow.params.rest.is_some() {
                return Err(fail("render marker requires a zero-argument call thunk"));
            }
            let expression = arrow
                .get_expression()
                .ok_or_else(|| fail("render marker requires a call expression"))?;
            let call = match expression.get_inner_expression() {
                Expression::CallExpression(call) => call.as_ref(),
                Expression::ChainExpression(chain) => match &chain.expression {
                    ChainElement::CallExpression(call) => call.as_ref(),
                    _ => return Err(fail("render requires a call expression")),
                },
                _ => return Err(fail("render requires a call expression")),
            };
            if call.type_arguments.is_some()
                || call
                    .arguments
                    .iter()
                    .any(|argument| matches!(argument, Argument::SpreadElement(_)))
            {
                return Err(fail("generic and spread render arguments are unsupported"));
            }
            if optional_member(&call.callee) {
                return Err(fail("optional member render targets are unsupported"));
            }
            render_used = true;
            if matches!(target, MoonshineModuleTarget::Ssr) {
                continue;
            }
            let callee = call.callee.span();
            edits.push((
                attribute.span.start as usize,
                callee.start as usize,
                "get={()=> (".into(),
            ));
            if let Some(first) = call.arguments.first() {
                edits.push((
                    callee.end as usize,
                    first.span().start as usize,
                    ")} args={()=>[".into(),
                ));
                for pair in call.arguments.windows(2) {
                    edits.push((
                        pair[0].span().end as usize,
                        pair[1].span().start as usize,
                        ",".into(),
                    ));
                }
                for argument in &call.arguments {
                    let expression = argument.as_expression().unwrap();
                    let (has_call, _) = crate::svelte_expression::metadata(expression, &semantic);
                    memo_needed |= has_call;
                    cell_needed |= !has_call;
                    let span = expression.span();
                    edits.push((
                        span.start as usize,
                        span.start as usize,
                        if has_call {
                            format!("{memo}(()=> (")
                        } else {
                            format!("{cell}(()=> (")
                        },
                    ));
                    edits.push((span.end as usize, span.end as usize, "))".into()));
                }
                let last = call.arguments.last().unwrap().span();
                edits.push((
                    last.end as usize,
                    attribute.span.end as usize,
                    format!("]}} optional={{{}}}", call.optional),
                ));
            } else {
                edits.push((
                    callee.end as usize,
                    attribute.span.end as usize,
                    format!(")}} args={{()=>[]}} optional={{{}}}", call.optional),
                ));
            }
            render_used = true;
        }
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    if render_used {
        let alias = render_component.unwrap();
        let mut runtime = "__crepusSvelteSnippetRuntime".to_owned();
        while source.contains(&runtime) {
            runtime.push('_');
        }
        match target {
            MoonshineModuleTarget::Dom => {
                out = format!(
                    "import {{mapArray as {runtime}}} from \"@tschk/moonshine-dom/control\";\n{out}"
                );
                out.push_str(&format!("\nfunction {alias}(props:{{get:()=>any;args:()=>({{readonly value:any}}[]);optional:boolean}}):any{{const args=props.args();return {runtime}(()=>{{const fn=props.get();return props.optional&&fn==null?[]:[fn];}},fn=>fn(...args));}}\n"));
            }
            MoonshineModuleTarget::Ssr => {
                // Construct snippet children before later siblings in the same template.
                out.push_str(&format!(
                    "\nfunction {alias}(props:{{call:()=>any}}):any{{return props.call();}}\n"
                ));
            }
        }
    }
    if memo_needed {
        out = format!(
            "import {{createDerived as {derived}}} from \"@tschk/moonshine-svelte/runes\";\n{out}"
        );
        out.push_str(&format!("\nfunction {memo}<T>(read:()=>T):{{readonly value:T}}{{const memo={derived}(read);return {{get value(){{return memo.value;}}}};}}\n"));
    }
    if cell_needed {
        out.push_str(&format!("\nfunction {cell}<T>(read:()=>T):{{readonly value:T}}{{return {{get value(){{return read();}}}};}}\n"));
    }
    if unbound_used {
        out.push_str(&format!(
            "\nfunction {unbound}<T>(value:T):T{{return value;}}\n"
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lower(body: &str, target: MoonshineModuleTarget) -> Result<String, String> {
        let source = format!("export default function Component(){{{body}}}");
        let out = transform(
            &source,
            Path::new("snippet.svelte"),
            target,
            "Component",
            &["row".into()],
            &[],
            Some("Render"),
        )?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &out, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{out}", parsed.errors);
        Ok(out)
    }
    #[test]
    fn instantiated_snippet_callbacks_keep_unbound_calls_and_method_receivers() {
        let body="function row(callback,object){const a=(callback<number>)();const b=(callback<string>)?.();const c=(callback<boolean>)`text`;const d=(object.method<number>)();return <p>{a}</p>}return <Render call={()=> (row(handler,object))}/>";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = lower(body, target).unwrap();
            if matches!(target, MoonshineModuleTarget::Dom) {
                assert!(
                    out.contains("__crepusSvelteSnippetUnbound((callback.value<number>))()"),
                    "{out}"
                );
                assert!(
                    out.contains("__crepusSvelteSnippetUnbound((callback.value<string>))?.()"),
                    "{out}"
                );
                assert!(
                    out.contains("__crepusSvelteSnippetUnbound((callback.value<boolean>))`text`"),
                    "{out}"
                );
                assert!(out.contains("(object.value.method<number>)()"), "{out}");
                assert!(
                    !out.contains("__crepusSvelteSnippetUnbound(object.value"),
                    "{out}"
                );
            } else {
                assert!(out.contains("(callback<number>)()"), "{out}");
                assert!(out.contains("(object.method<number>)()"), "{out}");
                assert!(
                    !out.contains("function __crepusSvelteSnippetUnbound"),
                    "{out}"
                );
            }
        }
    }
    #[test]
    fn getter_params_types_shorthand_closures_and_shadowing() {
        let out=lower("function row(value:number,cb:()=>number){const obj={value};function inner(value){return value};return <p>{value}{()=>value}{cb()}</p>}return <Render call={()=> (row(count,()=>1))}/>",MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("value:{readonly value:number}={get value():any{return void 0}},cb:{readonly value:()=>number}={get value():any{return void 0}}"));
        assert!(out.contains("{value: value.value}"));
        assert!(out.contains("function inner(value){return value}"));
        assert!(out
            .contains("{value.value}{()=>value.value}{__crepusSvelteSnippetUnbound(cb.value)()}"));
        assert!(out
            .contains("get={()=> (row)} args={()=>[__crepusSvelteSnippetCell(()=> (count)),__crepusSvelteSnippetCell(()=> (()=>1))]} optional={false}"));
    }
    #[test]
    fn complex_call_args_memoize_once_outside_component_selection() {
        let out=lower("function make(){}function row(value){return <p>{value}{value}</p>}return <Render call={()=> (row(make()))}/>",MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("args={()=>[__crepusSvelteSnippetMemo(()=> (make()))]}"));
        assert!(out.contains("const args=props.args();return __crepusSvelteSnippetRuntime"));
        assert!(out.contains("fn=>fn(...args)"));
        assert!(out.contains(
            "const memo=__crepusSvelteSnippetDerived(read);return {get value(){return memo.value;}}"
        ));
    }
    #[test]
    fn required_and_optional_calls_keep_arguments_lazy() {
        let out=lower("function row(value){return <p>{value}</p>}return <><Render call={()=> (row())}/><Render call={()=> (body?.(effect()))}/></>",MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("get={()=> (row)} args={()=>[]} optional={false}"));
        assert!(out.contains(
            "get={()=> (body)} args={()=>[__crepusSvelteSnippetCell(()=> (effect()))]} optional={true}"
        ));
        assert!(out.contains("props.optional&&fn==null?[]:[fn]"));
    }
    #[test]
    fn ssr_retains_value_params_receiver_and_eager_call_arguments() {
        let out=lower("function row(value:number){return <p>{value}</p>}return <Render call={()=> (object.row(make()))}/>",MoonshineModuleTarget::Ssr).unwrap();
        assert!(out.contains("function row(value:number=void 0 as any)"));
        assert!(out.contains("<p>{value}</p>"));
        assert!(out.contains("call={()=> (object.row(make()))}"));
        assert!(!out.contains("defer as __crepusSvelteSnippetRuntime"));
        assert!(out.contains("return props.call();"));
        assert!(!out.contains("createDerived"));
    }
    #[test]
    fn reject_parameter_writes_patterns_and_invalid_renders() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for body in [
                "function row(value){value++;return <p/>}return <p/>",
                "function row({value}){return <p/>}return <p/>",
                "function row(value=1){return <p/>}return <p/>",
                "function row(...value){return <p/>}return <p/>",
                "async function row(value){return <p/>}return <p/>",
                "function row(value){return <p/>}return <Render call={()=> (row)}/>",
                "function row(value){return <p/>}return <Render call={()=> (obj?.row(value))}/>",
                "function row(value){return <p/>}return <Render call={()=> (row(...value))}/>",
            ] {
                assert!(lower(body, target).is_err(), "{body}");
            }
        }
        assert!(lower(
            "function outer(){function row(value){return <p/>}}return <p/>",
            MoonshineModuleTarget::Dom
        )
        .is_err());
        assert!(lower(
            "function row(){}function row(){}return <p/>",
            MoonshineModuleTarget::Dom
        )
        .is_err());
    }
    #[test]
    fn nested_renders_are_disjoint_and_runtime_alias_hygienic() {
        let source = "const __crepusSvelteSnippetRuntime=0;\
            function row(value){return <p>{value}</p>}\
            return <Render call={()=> (row(<Render call={()=> (row(1))}/>))}/>";
        let out = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("mapArray as __crepusSvelteSnippetRuntime_"));
        assert_eq!(out.matches("get={()=> (row)}").count(), 2);
        assert!(!out.contains("call={()=>"));
    }
    #[test]
    fn inline_snippet_parameters_follow_their_lexical_symbols() {
        let source = "export default function Component(){function row(value){return <p>{value}</p>}return (()=>{function Inline(value){function shadow(value){return value};return <p>{value}</p>}const row=Inline;return <Child row={row}/>})()}";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = transform(
                source,
                Path::new("inline.svelte"),
                target,
                "Component",
                &["row".into()],
                &["Inline".into()],
                None,
            )
            .unwrap();
            let allocator = Allocator::default();
            assert!(
                Parser::new(&allocator, &out, SourceType::tsx())
                    .parse()
                    .errors
                    .is_empty(),
                "{out}"
            );
            assert!(
                out.contains("function shadow(value){return value}"),
                "{out}"
            );
            assert_eq!(
                out.matches("{value.value}").count(),
                if matches!(target, MoonshineModuleTarget::Dom) {
                    2
                } else {
                    0
                }
            );
        }
        let bound = "export default function Component(){return (()=>{function Inline(value){return <input marker={value}/>};return <Child/>})()}";
        assert!(validate_bindings(
            bound,
            Path::new("inline.svelte"),
            &[],
            &["Inline".into()],
            &["marker"]
        )
        .is_err());
    }

    #[test]
    fn validate_source_bindings_rejects_only_direct_parameter_symbols() {
        let source = "export default function Component(){function row(value){return <input marker={value}/>};return <p/>}";
        assert!(validate_bindings(
            source,
            Path::new("snippet.svelte"),
            &["row".into()],
            &[],
            &["marker"]
        )
        .is_err());
        let source = "export default function Component(){function row(value){function inner(value){return <input marker={value}/>};return <input marker={value.text}/>};return <p/>}";
        validate_bindings(
            source,
            Path::new("snippet.svelte"),
            &["row".into()],
            &[],
            &["marker"],
        )
        .unwrap();
        let source = "export default function Component(){function row(value){function inner(){return <Child reference={value}/>};return <p/>};return <p/>}";
        assert!(validate_bindings(
            source,
            Path::new("snippet.svelte"),
            &["row".into()],
            &[],
            &["reference"]
        )
        .is_err());
    }
    #[test]
    fn simple_reads_stay_live_through_component_props_and_events() {
        let out=lower("function row(value){return <><Child x={value}/><button onClick={value}/></>}return <Render call={()=> (row(handler))}/>",MoonshineModuleTarget::Dom).unwrap();
        let (props, _) = crate::svelte_component_props::transform(
            &out,
            Path::new("snippet.svelte"),
            &["Render".into()],
            None,
        )
        .unwrap();
        assert!(props.contains("<Child x={value.value}/>"));
        let events =
            crate::svelte_events::transform_events(&props, Path::new("snippet.svelte"), &[])
                .unwrap();
        assert!(events.contains("function(this:any){return (value.value);},false)"));
        assert!(!events.contains("createDerived"));
    }
    #[test]
    fn callable_parameters_keep_unbound_receivers_and_transparent_types() {
        let out=lower("const __crepusSvelteSnippetUnbound=0;function row(callback:()=>string){const a=(callback as ()=>string)();const b=callback`tag`;return <p>{callback()}</p>}return <Render call={()=> (row(handler))}/>",MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("__crepusSvelteSnippetUnbound_((callback.value as ()=>string))()"));
        assert!(out.contains("__crepusSvelteSnippetUnbound_(callback.value)`tag`"));
        assert!(out.contains("{__crepusSvelteSnippetUnbound_(callback.value)()}"));
    }
    #[test]
    fn argument_cells_preserve_lexical_this_and_arguments_hygienically() {
        let out=lower("const __crepusSvelteSnippetCell=0;function row(value){return <p>{value}</p>}return <Render call={()=> (row(this,arguments[0]))}/>",MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("__crepusSvelteSnippetCell_(()=> (this))"));
        assert!(out.contains("__crepusSvelteSnippetCell_(()=> (arguments[0]))"));
        assert!(out.contains("get value(){return read();}"));
    }
    #[test]
    fn omitted_arguments_default_to_undefined_accessors() {
        let out=lower("function row(value:number,other){return <p>{value}{other}</p>}return <Render call={()=> (row())}/>",MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("value:{readonly value:number}={get value():any{return void 0}}"));
        assert!(out.contains("other:{readonly value:any}={get value():any{return void 0}}"));
        assert!(out.contains("args={()=>[]}"));
        let ssr = lower(
            "function row(value:number){return <p>{value}</p>}return <Render call={()=> (row())}/>",
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert!(ssr.contains("function row(value:number=void 0 as any)"));
        assert!(!ssr.contains("get value()"));
        let ssr = lower(
            "function row(value){return <p>{value}</p>}return <Render call={()=> (row(1))}/>",
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert!(ssr.contains("function row(value:any=void 0)"));
    }
    #[test]
    fn member_parameter_mutation_is_preserved() {
        let out=lower("function row(value){value.count++;return <p>{value.count}</p>}return <Render call={()=> (row(count))}/>",MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("value.value.count++"));
        assert!(out.contains("{value.value.count}"));
    }
}
