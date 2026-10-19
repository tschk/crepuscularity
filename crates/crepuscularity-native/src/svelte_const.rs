//! Compiler-only lexical template declarations. Markers never reach the runtime.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::{SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};
use std::collections::{HashMap, HashSet};

fn attribute<'s, 'a>(element: &'s JSXElement<'a>, name: &str) -> Option<&'s JSXAttribute<'a>> {
    element.opening_element.attributes.iter().find_map(|item| match item {
        JSXAttributeItem::Attribute(attr) if matches!(&attr.name, JSXAttributeName::Identifier(id) if id.name == name) => Some(attr.as_ref()),
        _ => None,
    })
}
fn body<'s, 'a>(element: &'s JSXElement<'a>) -> Result<&'s ArrowFunctionExpression<'a>, String> {
    let Some(JSXAttributeValue::ExpressionContainer(container)) =
        attribute(element, "body").and_then(|attr| attr.value.as_ref())
    else {
        return Err("missing const fragment body".into());
    };
    let JSXExpression::ArrowFunctionExpression(arrow) = &container.expression else {
        return Err("invalid const fragment body".into());
    };
    if arrow.expression || !arrow.params.items.is_empty() || arrow.r#async {
        return Err("invalid const fragment callback".into());
    }
    Ok(arrow)
}
fn bindings<'s, 'a>(pattern: &'s BindingPattern<'a>) -> Vec<&'s BindingIdentifier<'a>> {
    // Oxc's visit lifetime is the AST arena lifetime; the collected references
    // are instead obtained directly from the finite binding tree.
    fn collect<'s, 'a>(pattern: &'s BindingPattern<'a>, out: &mut Vec<&'s BindingIdentifier<'a>>) {
        match pattern {
            BindingPattern::BindingIdentifier(id) => out.push(id),
            BindingPattern::AssignmentPattern(value) => collect(&value.left, out),
            BindingPattern::ObjectPattern(value) => {
                for property in &value.properties {
                    collect(&property.value, out);
                }
                if let Some(rest) = &value.rest {
                    collect(&rest.argument, out);
                }
            }
            BindingPattern::ArrayPattern(value) => {
                for item in value.elements.iter().flatten() {
                    collect(item, out);
                }
                if let Some(rest) = &value.rest {
                    collect(&rest.argument, out);
                }
            }
        }
    }
    let mut result = Vec::new();
    collect(pattern, &mut result);
    result
}
fn simple_object(pattern: &BindingPattern<'_>) -> bool {
    matches!(pattern, BindingPattern::ObjectPattern(object) if object.rest.is_none() && object.properties.iter().all(|property| !property.computed && property.shorthand && matches!(&property.value, BindingPattern::BindingIdentifier(_))))
}
fn slice(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}
fn apply(source: &str, span: Span, edits: &[(usize, usize, String)]) -> String {
    let mut result = slice(source, span).to_owned();
    for (start, end, value) in edits.iter().rev() {
        if *start >= span.start as usize && *end <= span.end as usize {
            result.replace_range(
                start - span.start as usize..end - span.start as usize,
                value,
            );
        }
    }
    result
}

/// Infer only the existing native-supported runes mode from semantic global
/// references, before prop/store/class lowering removes the authored macros.
pub(crate) fn runes_mode(source: &str, marker: &str, typescript: bool) -> Result<bool, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid const rune-mode syntax".into());
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    // Validate original declaration grammar before each/await or rune passes
    // introduce compiler-owned TypeScript into its initializer.
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name,JSXElementName::IdentifierReference(id) if id.name==marker)
        {
            continue;
        }
        for statement in &body(element)?.body.statements {
            if !matches!(statement, Statement::VariableDeclaration(_)) {
                continue;
            }
            let grammar = if typescript {
                SourceType::ts()
            } else {
                SourceType::mjs()
            };
            if !Parser::new(&allocator, slice(source, statement.span()), grammar)
                .parse()
                .errors
                .is_empty()
            {
                return Err(if typescript {
                    "invalid TypeScript const declaration"
                } else {
                    "TypeScript const syntax requires a lang=ts script"
                }
                .into());
            }
        }
    }
    let runes = semantic.nodes().iter().any(|node| match node.kind() {
        AstKind::IdentifierReference(id) if crate::svelte_script::is_rune(id.name.as_str()) => {
            !crate::svelte_script::has_runtime_binding(&semantic, id)
        }
        _ => false,
    });
    Ok(runes)
}

/// Lower only declarations in explicit markers; ordinary source remains byte-identical.
#[cfg(test)]
pub(crate) fn lower(
    source: &str,
    marker: &str,
    target: MoonshineModuleTarget,
    runes: bool,
) -> Result<String, String> {
    lower_with_policy(
        source,
        marker,
        target,
        runes,
        crate::svelte_javascript::IrPolicy::Typed,
    )
}

pub(crate) fn lower_with_policy(
    source: &str,
    marker: &str,
    target: MoonshineModuleTarget,
    runes: bool,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid const declaration syntax".into());
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err("invalid const declaration scope".into());
    }
    let semantic = built.semantic;
    let nodes = semantic.nodes();
    let mut groups = Vec::new();
    let mut reads = HashMap::<SymbolId, String>::new();
    let mut scalar_symbols = HashSet::new();
    let mut names = HashSet::new();
    for node in nodes.iter() {
        match node.kind() {
            AstKind::BindingIdentifier(id) => {
                names.insert(id.name.to_string());
            }
            AstKind::IdentifierReference(id) => {
                names.insert(id.name.to_string());
            }
            _ => {}
        }
    }
    let mut fresh = |prefix: &str| {
        let mut name = prefix.to_string();
        while names.contains(&name) {
            name.push('_');
        }
        names.insert(name.clone());
        name
    };
    let derived = fresh("__crepusConstDerived");
    for node in nodes.iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name, JSXElementName::IdentifierReference(id) if id.name == marker)
        {
            continue;
        }
        let arrow = body(element)?;
        let Some(Statement::ReturnStatement(_)) = arrow.body.statements.last() else {
            return Err("const fragment requires a final return".into());
        };
        let mut declarations = Vec::new();
        for statement in arrow
            .body
            .statements
            .iter()
            .take(arrow.body.statements.len() - 1)
        {
            let Statement::VariableDeclaration(declaration) = statement else {
                return Err("invalid const fragment declaration".into());
            };
            if declaration.kind != VariableDeclarationKind::Const
                || declaration.declarations.len() != 1
            {
                return Err("const tag requires one initialized declaration".into());
            }
            let entry = &declaration.declarations[0];
            if entry.init.is_none() {
                return Err("const tag requires an initializer".into());
            }
            if let Some(Expression::CallExpression(call)) =
                entry.init.as_ref().map(Expression::get_inner_expression)
            {
                let rune = match call.callee.get_inner_expression() {
                    Expression::Identifier(id)
                        if matches!(id.name.as_str(), "$state" | "$derived") =>
                    {
                        Some(id)
                    }
                    Expression::StaticMemberExpression(member)
                        if matches!(member.property.name.as_str(), "raw" | "by") =>
                    {
                        match member.object.get_inner_expression() {
                            Expression::Identifier(id)
                                if matches!(
                                    (id.name.as_str(), member.property.name.as_str()),
                                    ("$state", "raw") | ("$derived", "by")
                                ) =>
                            {
                                Some(id)
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(id) = rune {
                    if !crate::svelte_script::has_runtime_binding(&semantic, id) {
                        return Err(
                            "state and derived runes are invalid as const tag initializers".into(),
                        );
                    }
                }
            }
            let ids = bindings(&entry.id);
            let cell = if matches!(&entry.id, BindingPattern::BindingIdentifier(_)) {
                ids.first().ok_or("empty const binding")?.name.to_string()
            } else {
                fresh("__crepusConstPattern")
            };
            for id in &ids {
                let symbol = id.symbol_id.get().ok_or("unresolved const binding")?;
                let read = if matches!(&entry.id, BindingPattern::BindingIdentifier(_)) {
                    format!("{cell}.value")
                } else {
                    format!("{cell}.value.{}", id.name)
                };
                if matches!(&entry.id, BindingPattern::BindingIdentifier(_)) {
                    scalar_symbols.insert(symbol);
                }
                reads.insert(symbol, read);
            }
            declarations.push((declaration.as_ref(), cell));
        }
        groups.push(declarations);
    }
    if groups.is_empty() {
        return Ok(source.into());
    }
    let mut reference_edits = Vec::new();
    let mut unbound_helper = None;
    for node in nodes.iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        let reference = semantic
            .scoping()
            .get_reference(id.reference_id.get().ok_or("unresolved const reference")?);
        let Some(read) = reference.symbol_id().and_then(|symbol| reads.get(&symbol)) else {
            continue;
        };
        let own_pattern_read = groups.iter().flatten().any(|(declaration, _)| {
            let entry = &declaration.declarations[0];
            !matches!(&entry.id, BindingPattern::BindingIdentifier(_))
                && id.span.start >= entry.span.start
                && id.span.end <= entry.span.end
                && bindings(&entry.id)
                    .iter()
                    .any(|bound| bound.symbol_id.get() == reference.symbol_id())
        });
        if reference.is_write() {
            return Err("cannot assign to const template binding".into());
        }
        if own_pattern_read {
            continue;
        }
        if matches!(target, MoonshineModuleTarget::Ssr) {
            continue;
        }
        if reference.is_type() {
            return Err("const template binding type queries are unsupported".into());
        }
        let mut reference_span = id.span;
        let mut unbound = false;
        for parent in nodes.ancestors(node.id()) {
            match parent.kind() {
                AstKind::ParenthesizedExpression(_)
                | AstKind::TSAsExpression(_)
                | AstKind::TSSatisfiesExpression(_)
                | AstKind::TSNonNullExpression(_)
                | AstKind::TSTypeAssertion(_)
                | AstKind::TSInstantiationExpression(_) => {
                    reference_span = parent.kind().span();
                }
                AstKind::CallExpression(call) => {
                    unbound = call.callee.span() == reference_span;
                    break;
                }
                AstKind::TaggedTemplateExpression(tag) => {
                    unbound = tag.tag.span() == reference_span;
                    break;
                }
                _ => break,
            }
        }
        if unbound
            && reference
                .symbol_id()
                .is_some_and(|symbol| scalar_symbols.contains(&symbol))
        {
            let helper = unbound_helper.get_or_insert_with(|| fresh("__crepusConstUnbound"));
            // Insert around the complete callee so casts/type arguments and
            // independent edits folded into const initializers remain intact.
            reference_edits.push((
                reference_span.start as usize,
                reference_span.start as usize,
                format!("{helper}("),
            ));
            reference_edits.push((
                reference_span.end as usize,
                reference_span.end as usize,
                ")".into(),
            ));
        }
        let replacement = match nodes.parent_kind(node.id()) {
            AstKind::ObjectProperty(property) if property.shorthand => {
                crate::svelte_expression::shorthand_property(id.name.as_str(), read)
            }
            _ => read.clone(),
        };
        reference_edits.push((id.span.start as usize, id.span.end as usize, replacement));
    }
    reference_edits.sort_by_key(|edit| (edit.0, edit.1));
    let mut edits = Vec::new();
    let mut declaration_spans = Vec::new();
    for declarations in groups {
        let mut positions = HashMap::new();
        for (index, (declaration, _)) in declarations.iter().enumerate() {
            for id in bindings(&declaration.declarations[0].id) {
                positions.insert(
                    id.symbol_id.get().ok_or("unresolved const dependency")?,
                    index,
                );
            }
        }
        let mut dependencies = vec![HashSet::new(); declarations.len()];
        for node in nodes.iter() {
            let AstKind::IdentifierReference(id) = node.kind() else {
                continue;
            };
            let reference = semantic.scoping().get_reference(
                id.reference_id
                    .get()
                    .ok_or("unresolved const dependency reference")?,
            );
            let Some(dependency) = reference
                .symbol_id()
                .and_then(|symbol| positions.get(&symbol))
                .copied()
            else {
                continue;
            };
            for (index, (declaration, _)) in declarations.iter().enumerate() {
                let init = declaration.declarations[0]
                    .init
                    .as_ref()
                    .ok_or("missing const initializer")?
                    .span();
                if id.span.start >= init.start && id.span.end <= init.end {
                    dependencies[index].insert(dependency);
                }
            }
        }
        fn visit(
            index: usize,
            dependencies: &[HashSet<usize>],
            marks: &mut [u8],
            sorted: &mut Vec<usize>,
        ) -> Result<(), String> {
            if marks[index] == 2 {
                return Ok(());
            }
            if marks[index] == 1 {
                return Err("cyclical const template dependency".into());
            }
            marks[index] = 1;
            let mut deps = dependencies[index].iter().copied().collect::<Vec<_>>();
            deps.sort_unstable();
            for dep in deps {
                visit(dep, dependencies, marks, sorted)?;
            }
            marks[index] = 2;
            sorted.push(index);
            Ok(())
        }
        let mut sorted = Vec::new();
        let mut marks = vec![0; declarations.len()];
        if runes {
            sorted.extend(0..declarations.len());
        } else {
            for index in 0..declarations.len() {
                visit(index, &dependencies, &mut marks, &mut sorted)?;
            }
        }
        let mut lowered = String::new();
        for index in sorted {
            let (declaration, cell) = &declarations[index];
            let entry = &declaration.declarations[0];
            if matches!(target, MoonshineModuleTarget::Ssr) {
                lowered.push_str(slice(source, declaration.span));
                lowered.push('\n');
                continue;
            }
            let init = apply(
                source,
                entry
                    .init
                    .as_ref()
                    .ok_or("missing const initializer")?
                    .span(),
                &reference_edits,
            );
            if matches!(&entry.id, BindingPattern::BindingIdentifier(_)) || simple_object(&entry.id)
            {
                if let Some(annotation) = &entry.type_annotation {
                    // Preserve both initializer checking and const control-flow
                    // narrowing; a generic cell<T> would unnecessarily widen it.
                    let value = fresh("__crepusConstAnnotated");
                    let annotation = slice(source, annotation.span);
                    lowered.push_str(&format!("const {cell}={derived}(()=>{{const {value}{annotation}=({init});return {value};}});\n"));
                } else {
                    lowered.push_str(&format!("const {cell}={derived}(()=>({init}));\n"));
                }
            } else {
                let pattern = apply(source, entry.id.span(), &reference_edits);
                let annotation = entry
                    .type_annotation
                    .as_ref()
                    .map(|annotation| slice(source, annotation.span))
                    .unwrap_or("");
                let values = bindings(&entry.id)
                    .iter()
                    .map(|id| id.name.as_str())
                    .collect::<Vec<_>>()
                    .join(",");
                lowered.push_str(&format!("const {cell}={derived}(()=>{{const {pattern}{annotation}=({init});return {{{values}}};}});\n"));
            }
        }
        if let (Some((first, _)), Some((last, _))) = (declarations.first(), declarations.last()) {
            let span = Span::new(first.span.start, last.span.end);
            declaration_spans.push(span);
            edits.push((span.start as usize, span.end as usize, lowered));
        }
    }
    edits.extend(reference_edits.into_iter().filter(|(start, end, _)| {
        !declaration_spans
            .iter()
            .any(|span| *start >= span.start as usize && *end <= span.end as usize)
    }));
    edits.sort_by_key(|edit| (edit.0, edit.1));
    let mut output = source.to_owned();
    for (start, end, replacement) in edits.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    if matches!(target, MoonshineModuleTarget::Dom) {
        output.insert_str(
            0,
            &format!(
                "import {{createConstDerived as {derived}}} from \"@tschk/moonshine-svelte/const\";\n"
            ),
        );
    }
    if let Some(helper) = unbound_helper {
        output.push_str(&format!(
            "\nfunction {helper}<T>(value:T):T{{return value;}}\n"
        ));
    }
    Ok(output)
}

/// Called after await prepare and before each/await projection. The temporary
/// marker arrow is a nested lexical scope, so Oxc alone cannot catch these
/// Svelte-specific same-fragment payload conflicts.
pub(crate) fn validate_payloads(
    source: &str,
    marker: &str,
    each: Option<&str>,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<(), String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid const payload syntax".into());
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name,JSXElementName::IdentifierReference(id) if id.name==marker)
        {
            continue;
        }
        let Some(JSXAttributeValue::StringLiteral(kind)) =
            attribute(element, "kind").and_then(|attr| attr.value.as_ref())
        else {
            return Err("missing const fragment kind".into());
        };
        if !matches!(kind.value.as_str(), "each" | "await" | "component") {
            continue;
        }
        let local = body(element)?;
        let mut payload = HashSet::new();
        if kind.value == "each" {
            let enclosing=semantic.nodes().ancestors(node.id()).find_map(|parent| match parent.kind(){
                AstKind::JSXElement(element) if matches!(&element.opening_element.name,JSXElementName::IdentifierReference(id) if Some(id.name.as_str())==each)=>Some(element),_=>None
            }).ok_or("missing const each owner")?;
            let Some(JSXAttributeValue::ExpressionContainer(container)) =
                attribute(enclosing, "header").and_then(|attr| attr.value.as_ref())
            else {
                return Err("missing const each header".into());
            };
            let JSXExpression::StringLiteral(header) = &container.expression else {
                return Err("invalid const each header".into());
            };
            let header = crate::svelte_each::header_with_policy(header.value.as_str(), policy)?;
            let parameters = format!(
                "({}, {})=>0",
                header.item,
                header
                    .index
                    .as_deref()
                    .unwrap_or("__crepusUnusedConstIndex")
            );
            let Expression::ArrowFunctionExpression(arrow) =
                Parser::new(&allocator, &parameters, SourceType::tsx())
                    .parse_expression()
                    .map_err(|_| "invalid const each parameters")?
            else {
                return Err("invalid const each parameters".into());
            };
            for param in &arrow.params.items {
                for id in bindings(&param.pattern) {
                    payload.insert(id.name.to_string());
                }
            }
        } else if kind.value == "component" {
            let Some(JSXAttributeValue::ExpressionContainer(conflicts)) =
                attribute(element, "conflicts").and_then(|attr| attr.value.as_ref())
            else {
                continue;
            };
            let JSXExpression::ArrayExpression(conflicts) = &conflicts.expression else {
                return Err("invalid const component binding metadata".into());
            };
            for conflict in &conflicts.elements {
                let ArrayExpressionElement::StringLiteral(name) = conflict else {
                    return Err("invalid const component binding name".into());
                };
                payload.insert(name.value.to_string());
            }
        } else {
            let enclosing = semantic
                .nodes()
                .ancestors(node.id())
                .find_map(|parent| match parent.kind() {
                    AstKind::ArrowFunctionExpression(arrow) => Some(arrow),
                    _ => None,
                })
                .ok_or("missing const await owner")?;
            for param in &enclosing.params.items {
                for id in bindings(&param.pattern) {
                    payload.insert(id.name.to_string());
                }
            }
        }
        for statement in &local.body.statements {
            if let Statement::VariableDeclaration(declaration) = statement {
                for entry in &declaration.declarations {
                    for id in bindings(&entry.id) {
                        if payload.contains(id.name.as_str()) {
                            return Err(format!(
                                "const declaration duplicates fragment binding {}",
                                id.name
                            ));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Binding setters are erased on SSR, so validate their const target beforehand.
pub(crate) fn validate_bindings(
    source: &str,
    marker: &str,
    binding_markers: &[&str],
) -> Result<(), String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid const binding syntax".into());
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut symbols = HashSet::new();
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name,JSXElementName::IdentifierReference(id) if id.name==marker)
        {
            continue;
        }
        for statement in &body(element)?.body.statements {
            if let Statement::VariableDeclaration(declaration) = statement {
                for entry in &declaration.declarations {
                    for id in bindings(&entry.id) {
                        symbols.extend(id.symbol_id.get());
                    }
                }
            }
        }
    }
    for node in semantic.nodes().iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        let reference = semantic
            .scoping()
            .get_reference(id.reference_id.get().ok_or("unresolved const write")?);
        if reference.is_write()
            && reference
                .symbol_id()
                .is_some_and(|symbol| symbols.contains(&symbol))
        {
            return Err("cannot assign to const template binding".into());
        }
    }
    for node in semantic.nodes().iter() {
        let AstKind::JSXAttribute(attr) = node.kind() else {
            continue;
        };
        if !matches!(&attr.name,JSXAttributeName::Identifier(id) if binding_markers.contains(&id.name.as_str()))
        {
            continue;
        }
        let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value else {
            continue;
        };
        let Some(Expression::Identifier(id)) = container
            .expression
            .as_expression()
            .map(Expression::get_inner_expression)
        else {
            continue;
        };
        let reference = semantic.scoping().get_reference(
            id.reference_id
                .get()
                .ok_or("unresolved const binding target")?,
        );
        if reference
            .symbol_id()
            .is_some_and(|symbol| symbols.contains(&symbol))
        {
            return Err("cannot bind to const template binding".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(body: &str) -> String {
        format!("function App(){{return <Scope kind=\"if\" body={{()=>{{{body}}}}}/>;}}")
    }
    #[test]
    fn simple_pattern_keeps_property_getters_per_read_while_complex_pattern_is_shared() {
        let output=lower(&source("const {a}=read();const {a:b=other(),...rest}=read();return <p>{a}|{a}|{b}|{rest.x}</p>;"),"Scope",MoonshineModuleTarget::Dom,false).unwrap();
        assert!(output.contains(".value.a"), "{output}");
        assert!(output.contains("return {b,rest}"), "{output}");
        assert_eq!(output.matches("=__crepusConstDerived(").count(), 2);
    }
    #[test]
    fn server_preserves_eager_unused_const_and_client_is_lazy() {
        let input = source("const unused=sideEffect();const x=read();return <p>{x}{x}</p>;");
        let server = lower(&input, "Scope", MoonshineModuleTarget::Ssr, false).unwrap();
        assert!(server.contains("const unused=sideEffect()"));
        assert!(!server.contains("createDerived"));
        let client = lower(&input, "Scope", MoonshineModuleTarget::Dom, false).unwrap();
        assert!(
            client.contains("const unused=__crepusConstDerived(()=>(sideEffect()))"),
            "{client}"
        );
    }
    #[test]
    fn dependencies_use_symbols_not_names_and_pattern_defaults_remain_local() {
        let output=lower(&source("const y=x+1;const x=read();const {a=1,b=a}=object;return <p>{y}{b}{(()=>{const x=9;return x})()}</p>;"),"Scope",MoonshineModuleTarget::Dom,false).unwrap();
        assert!(output.find("const x=").unwrap() < output.find("const y=").unwrap());
        assert!(output.contains("b=a"), "{output}");
        assert!(output.contains("return x}"), "{output}");
    }
    #[test]
    fn reassignments_reject_but_member_writes_and_unbound_function_calls_survive() {
        assert!(lower(
            &source("const x=read();return <button onclick={()=>x++}/>;"),
            "Scope",
            MoonshineModuleTarget::Dom,
            false
        )
        .is_err());
        let output = lower(
            &source("const x=read();return <button onclick={()=>x.a++}>{x()}{(x)()}</button>;"),
            "Scope",
            MoonshineModuleTarget::Dom,
            false,
        )
        .unwrap();
        assert!(output.contains("x.value.a++"), "{output}");
        assert!(
            output.contains("__crepusConstUnbound(x.value)()"),
            "{output}"
        );
    }
    #[test]
    fn transparent_scalar_callees_keep_types_and_initializer_edit_folding() {
        let input=source("const callback=read();const object=readObject();const a=(callback<number>)();const b=(callback as ()=>string)();return <p>{(callback<string>)?.()}{(callback<boolean>)`tag`}{(object.method<number>)()}</p>;");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = lower(&input, "Scope", target, false).unwrap();
            if matches!(target, MoonshineModuleTarget::Dom) {
                assert!(
                    output.contains("__crepusConstUnbound((callback.value<number>))()"),
                    "{output}"
                );
                assert!(
                    output.contains("__crepusConstUnbound((callback.value as ()=>string))()"),
                    "{output}"
                );
                assert!(
                    output.contains("__crepusConstUnbound((callback.value<string>))?.()"),
                    "{output}"
                );
                assert!(
                    output.contains("__crepusConstUnbound((callback.value<boolean>))`tag`"),
                    "{output}"
                );
                assert!(
                    output.contains("(object.value.method<number>)()"),
                    "{output}"
                );
                assert!(
                    !output.contains("__crepusConstUnbound(object.value"),
                    "{output}"
                );
            } else {
                assert!(output.contains("(callback<number>)()"), "{output}");
                assert!(
                    !output.contains("function __crepusConstUnbound"),
                    "{output}"
                );
            }
            let allocator = Allocator::default();
            assert!(
                Parser::new(&allocator, &output, SourceType::tsx())
                    .parse()
                    .errors
                    .is_empty(),
                "{output}"
            );
        }
    }
    #[test]
    fn scalar_tags_keep_unbound_receivers_without_changing_pattern_tags() {
        let output = lower(
            &source("const tag=read();const {tag:member}=object;return <p>{tag`x`}{(tag)`y`}{member`z`}</p>;"),
            "Scope", MoonshineModuleTarget::Dom, false,
        ).unwrap();
        assert!(
            output.contains("__crepusConstUnbound(tag.value)`x`"),
            "{output}"
        );
        assert!(output.contains(".value.member`z`"), "{output}");
    }
    #[test]
    fn mode_uses_semantic_rune_references_and_keeps_runes_cycles_in_source_order() {
        assert!(runes_mode(
            "let value=$state(1);function App(){return value}",
            "Scope",
            true
        )
        .unwrap());
        assert!(!runes_mode(
            "function f($state){return $state(1)}let value=f(v=>v)",
            "Scope",
            true
        )
        .unwrap());
        assert!(!runes_mode("const text='$state(1)' ", "Scope", true).unwrap());
        let input = source("const y=x;const x=y;return <p>{x}</p>;");
        assert!(lower(&input, "Scope", MoonshineModuleTarget::Ssr, false).is_err());
        let output = lower(&input, "Scope", MoonshineModuleTarget::Ssr, true).unwrap();
        assert!(output.find("const y=").unwrap() < output.find("const x=").unwrap());
    }
    #[test]
    fn legacy_self_edge_rejects_and_pattern_calls_keep_their_aggregate_receiver() {
        assert!(lower(
            &source("const x=x;return <p>{x}</p>;"),
            "Scope",
            MoonshineModuleTarget::Dom,
            false
        )
        .is_err());
        assert!(lower(
            &source("const x=x;return <p>{x}</p>;"),
            "Scope",
            MoonshineModuleTarget::Dom,
            true
        )
        .is_ok());
        let output=lower(&source("const {fn}=object;const {fn:other,...rest}=object;const scalar=object.fn;return <p>{fn()}{other()}{scalar()}</p>;"),"Scope",MoonshineModuleTarget::Dom,false).unwrap();
        assert!(output.contains(".value.fn()"), "{output}");
        assert!(output.contains(".value.other()"), "{output}");
        assert!(
            output.contains("__crepusConstUnbound(scalar.value)()"),
            "{output}"
        );
    }
    #[test]
    fn explicit_annotations_keep_initializer_validation_and_declared_value_types() {
        let output=lower(&source("const x:string|number=1;const {a}:{a:number}=object;const [b,...rest]:number[]=[1,2];return <p>{x}{a}{b}{rest.length}</p>;"),"Scope",MoonshineModuleTarget::Dom,false).unwrap();
        assert!(output.contains(":string|number=(1)"), "{output}");
        assert!(output.contains(":{a:number}=(object)"), "{output}");
        assert!(output.contains("const [b,...rest]:number[]="), "{output}");
    }
}
