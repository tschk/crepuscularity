//! Authored await structure is retained until Oxc validates expressions and bindings.
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};

#[derive(Debug, PartialEq)]
pub(crate) struct Header {
    input: String,
    arm: Option<(&'static str, String)>,
}
fn expression(source: &str, policy: crate::svelte_javascript::IrPolicy) -> bool {
    let allocator = Allocator::default();
    let wrapped = format!("({source}\n)");
    let source_type = match policy {
        crate::svelte_javascript::IrPolicy::Typed => SourceType::tsx(),
        crate::svelte_javascript::IrPolicy::JavaScript => SourceType::jsx(),
    };
    Parser::new(&allocator, &wrapped, source_type)
        .parse_expression()
        .is_ok_and(|expression| expression.span().end as usize == wrapped.len())
}
pub(crate) fn binding(source: &str) -> bool {
    binding_with_policy(source, crate::svelte_javascript::IrPolicy::Typed)
}
fn binding_with_policy(source: &str, policy: crate::svelte_javascript::IrPolicy) -> bool {
    if source.trim().is_empty() {
        return true;
    }
    let allocator = Allocator::default();
    let wrapped = format!("({source})=>0");
    let source_type = match policy {
        crate::svelte_javascript::IrPolicy::Typed => SourceType::tsx(),
        crate::svelte_javascript::IrPolicy::JavaScript => SourceType::jsx(),
    };
    let parsed = Parser::new(&allocator, &wrapped, source_type).parse_expression();
    matches!(parsed, Ok(Expression::ArrowFunctionExpression(ref arrow)) if
        arrow.span.end as usize == wrapped.len() && !arrow.r#async &&
        arrow.params.rest.is_none() && arrow.params.items.len() == 1 &&
        arrow.params.items[0].initializer.is_none() &&
        !arrow.params.items[0].optional)
}
/// Candidate separators are accepted only when Oxc validates both complete sides.
pub(crate) fn header(source: &str) -> Result<Header, String> {
    header_with_policy(source, crate::svelte_javascript::IrPolicy::Typed)
}
fn header_with_policy(
    source: &str,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<Header, String> {
    if expression(source, policy) {
        return Ok(Header {
            input: source.into(),
            arm: None,
        });
    }
    for kind in ["then", "catch"] {
        for (at, _) in source.match_indices(kind) {
            if !source[..at].ends_with(char::is_whitespace)
                || (!source[at + kind.len()..].is_empty()
                    && !source[at + kind.len()..].starts_with(char::is_whitespace))
            {
                continue;
            }
            let input = source[..at].trim();
            let pattern = source[at + kind.len()..].trim();
            if expression(input, policy) && binding_with_policy(pattern, policy) {
                return Ok(Header {
                    input: input.into(),
                    arm: Some((kind, pattern.into())),
                });
            }
        }
    }
    Err("await requires an expression and optional then/catch binding pattern".into())
}
fn attribute<'s, 'a>(element: &'s JSXElement<'a>, name: &str) -> Option<&'s JSXAttribute<'a>> {
    element.opening_element.attributes.iter().find_map(|item|match item {
        JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,JSXAttributeName::Identifier(id) if id.name==name)=>Some(attribute.as_ref()), _=>None
    })
}
fn text<'s, 'a>(attribute: &'s JSXAttribute<'a>) -> Option<&'s str> {
    let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
        return None;
    };
    match &container.expression {
        JSXExpression::StringLiteral(value) => Some(value.value.as_str()),
        _ => None,
    }
}
fn arrow<'s, 'a>(attribute: &'s JSXAttribute<'a>) -> Option<&'s ArrowFunctionExpression<'a>> {
    let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
        return None;
    };
    match &container.expression {
        JSXExpression::ArrowFunctionExpression(arrow) => Some(arrow.as_ref()),
        _ => None,
    }
}
fn fresh(names: &mut std::collections::HashSet<String>, base: &str) -> String {
    let mut name = base.to_owned();
    while names.contains(&name) {
        name.push('_')
    }
    names.insert(name.clone());
    name
}
fn apply(source: &str, mut edits: Vec<(usize, usize, String)>) -> String {
    edits.sort_by_key(|edit| (edit.0, edit.1));
    let mut output = source.to_owned();
    for (start, end, replacement) in edits.into_iter().rev() {
        output.replace_range(start..end, &replacement)
    }
    output
}
/// Rename only compiler-owned JSX tags after reserving decoded script names.
pub(crate) fn rename_marker(source: &str, old: &str, new: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid await marker syntax".into());
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    let mut edits = Vec::new();
    for node in built.semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        let JSXElementName::IdentifierReference(name) = &element.opening_element.name else {
            continue;
        };
        if name.name == old {
            if element.closing_element.is_some() {
                return Err("invalid compiler-owned await marker".into());
            }
            edits.push((name.span.start as usize, name.span.end as usize, new.into()));
        }
    }
    Ok(apply(source, edits))
}
#[cfg(test)]
pub(crate) fn prepare(source: &str, marker: &str) -> Result<String, String> {
    prepare_with_policy(source, marker, crate::svelte_javascript::IrPolicy::Typed)
}

pub(crate) fn prepare_with_policy(
    source: &str,
    marker: &str,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid await component syntax".into());
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err("invalid await component scope".into());
    }
    let mut edits = Vec::new();
    let mut count = 0;
    for node in built.semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name,JSXElementName::IdentifierReference(id) if id.name==marker)
        {
            continue;
        }
        let metadata = attribute(element, "header").ok_or("missing await header")?;
        let parsed_header = header_with_policy(
            text(metadata).ok_or("invalid await header metadata")?,
            policy,
        )?;
        let initial = attribute(element, "initial").ok_or("missing await initial branch")?;
        let (kind, pattern) = parsed_header
            .arm
            .as_ref()
            .map_or(("pending", ""), |(kind, pattern)| (*kind, pattern.as_str()));
        if kind != "pending" && attribute(element, kind).is_some() {
            return Err("duplicate shorthand await branch".into());
        }
        if kind == "catch" && attribute(element, "then").is_some() {
            return Err("then branch cannot follow shorthand catch".into());
        }
        edits.push((
            metadata.span.start as usize,
            metadata.span.end as usize,
            format!("read={{()=>({}\n)}}", parsed_header.input),
        ));
        edits.push((
            initial.name.span().start as usize,
            initial.name.span().end as usize,
            kind.into(),
        ));
        let initial_arrow = arrow(initial).ok_or("invalid await initial branch")?;
        edits.push((
            initial_arrow.params.span.start as usize,
            initial_arrow.params.span.end as usize,
            format!("({pattern})"),
        ));
        for arm in ["then", "catch"] {
            if let Some(metadata) = attribute(element, &format!("{arm}Pattern")) {
                let pattern = text(metadata).ok_or("invalid await binding metadata")?;
                if !binding_with_policy(pattern, policy) {
                    return Err("invalid await branch binding pattern".into());
                }
                let body = arrow(attribute(element, arm).ok_or("missing await branch")?)
                    .ok_or("invalid await branch callback")?;
                edits.push((
                    metadata.span.start as usize,
                    metadata.span.end as usize,
                    String::new(),
                ));
                edits.push((
                    body.params.span.start as usize,
                    body.params.span.end as usize,
                    format!("({pattern})"),
                ));
            }
        }
        count += 1;
    }
    if count == 0 {
        return Ok(source.into());
    }
    Ok(apply(source, edits))
}
/// Run after each-header materialization so lexical references are real Oxc nodes.
#[cfg(test)]
pub(crate) fn finish(
    source: &str,
    marker: &str,
    binding_markers: &[&str],
    inline_schedule: bool,
) -> Result<String, String> {
    finish_with_policy(
        source,
        marker,
        binding_markers,
        inline_schedule,
        crate::svelte_javascript::IrPolicy::Typed,
    )
}

pub(crate) fn finish_with_policy(
    source: &str,
    marker: &str,
    binding_markers: &[&str],
    inline_schedule: bool,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<String, String> {
    let output = source.to_owned();
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid await callback binding".into());
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err("invalid await binding scope".into());
    }
    let semantic = &built.semantic;
    // Oxc resolves ValueAsType into Type flags, so distinguish queries by AST ancestry.
    let is_type_query = |node_id, span: oxc_span::Span| {
        for parent in semantic.nodes().ancestors(node_id) {
            match parent.kind() {
                AstKind::TSQualifiedName(name) if name.left.span().start == span.start => {}
                AstKind::TSTypeQuery(query) => return query.expr_name.span().start == span.start,
                _ => return false,
            }
        }
        false
    };

    let mut names = std::collections::HashSet::new();
    for node in semantic.nodes().iter() {
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
    let unbound = fresh(&mut names, "__crepusAwaitUnbound");
    let mut uses_unbound = false;
    let call_span = |node_id, mut span: oxc_span::Span| {
        for parent in semantic.nodes().ancestors(node_id) {
            match parent.kind() {
                AstKind::ParenthesizedExpression(_)
                | AstKind::TSAsExpression(_)
                | AstKind::TSSatisfiesExpression(_)
                | AstKind::TSNonNullExpression(_)
                | AstKind::TSTypeAssertion(_)
                | AstKind::TSInstantiationExpression(_) => span = parent.kind().span(),
                AstKind::CallExpression(call) if call.callee.span() == span => return Some(span),
                AstKind::TaggedTemplateExpression(tag) if tag.tag.span() == span => {
                    return Some(span)
                }
                _ => break,
            }
        }
        None
    };
    let derived = fresh(&mut names, "__crepusAwaitDerived");
    let runtime = fresh(&mut names, "__crepusAwaitRuntime");
    let awaited = fresh(&mut names, "__crepusAwaitValue");
    let input_type = fresh(&mut names, "__crepusAwaitInput");
    let mut edits = Vec::new();
    let mut projections: Vec<(_, String)> = Vec::new();
    let mut parameter_spans = Vec::new();
    let mut ordinal = 0;
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name,JSXElementName::IdentifierReference(id) if id.name==marker)
        {
            continue;
        }
        let reader = fresh(&mut names, "__crepusAwaitRead");
        let read_attribute = attribute(element, "read").ok_or("missing await source reader")?;
        let Some(JSXAttributeValue::ExpressionContainer(read_container)) = &read_attribute.value
        else {
            return Err("invalid await source reader".into());
        };
        edits.push((
            read_attribute.name.span().start as usize,
            read_attribute.name.span().end as usize,
            "build".into(),
        ));
        edits.push((
            read_container.span.start as usize,
            read_container.span.start as usize + 1,
            format!("{{()=>{{const {reader}="),
        ));
        edits.push((
            read_container.span.end as usize - 1,
            read_container.span.end as usize,
            format!(";return{{read:{reader},"),
        ));
        for kind in ["pending", "then", "catch"] {
            if let Some(branch) = attribute(element, kind) {
                let Some(JSXAttributeValue::ExpressionContainer(container)) = &branch.value else {
                    return Err("invalid await branch reader".into());
                };
                edits.push((
                    branch.name.span().start as usize,
                    container.span.start as usize + 1,
                    format!("{kind}:"),
                ));
                edits.push((
                    container.span.end as usize - 1,
                    container.span.end as usize,
                    ",".into(),
                ));
            }
        }
        edits.push((
            element.opening_element.span.end as usize - 2,
            element.opening_element.span.end as usize,
            "};}}/>".into(),
        ));
        for kind in ["then", "catch"] {
            let Some(callback) = attribute(element, kind).and_then(arrow) else {
                continue;
            };
            if callback.params.items.is_empty() {
                continue;
            }
            let parameter = &callback.params.items[0];
            let raw = fresh(&mut names, &format!("__crepusAwaitSource{ordinal}"));
            ordinal += 1;
            let bindings = parameter.pattern.get_binding_identifiers();
            let pattern_span = parameter.pattern.span();
            parameter_spans.push(parameter.span);
            let pattern_text = &output[pattern_span.start as usize..pattern_span.end as usize];
            let mut pattern_edits = Vec::new();
            let mut annotation_edits = Vec::new();
            let annotation_span = parameter
                .type_annotation
                .as_ref()
                .map(|annotation| annotation.span);
            for default_node in semantic.nodes().iter() {
                let AstKind::IdentifierReference(id) = default_node.kind() else {
                    continue;
                };
                if id.span.start < parameter.span.start || id.span.end > parameter.span.end {
                    continue;
                }
                let reference = semantic.scoping().get_reference(
                    id.reference_id
                        .get()
                        .ok_or("unresolved await default reference")?,
                );
                let Some(symbol) = reference.symbol_id() else {
                    continue;
                };
                if bindings.iter().any(|id| id.symbol_id.get() == Some(symbol)) {
                    if reference.is_write() {
                        return Err("await bindings cannot be reassigned inside defaults".into());
                    }
                    continue;
                }
                let Some((_, read)) = projections.iter().find(|(binding, _)| *binding == symbol)
                else {
                    continue;
                };
                if reference.is_write()
                    || ((reference.is_type() || reference.flags().is_value_as_type())
                        && !is_type_query(default_node.id(), id.span))
                {
                    return Err("await bindings cannot be reassigned or used as types".into());
                }
                if let Some(span) = call_span(default_node.id(), id.span) {
                    uses_unbound = true;
                    pattern_edits.push((
                        (span.start - pattern_span.start) as usize,
                        (span.start - pattern_span.start) as usize,
                        format!("{unbound}("),
                    ));
                    pattern_edits.push((
                        (span.end - pattern_span.start) as usize,
                        (span.end - pattern_span.start) as usize,
                        ")".into(),
                    ));
                }
                let read = match semantic.nodes().parent_kind(default_node.id()) {
                    AstKind::ObjectProperty(property) if property.shorthand => {
                        crate::svelte_expression::shorthand_property(id.name.as_str(), read)
                    }
                    _ => read.clone(),
                };
                if id.span.end <= pattern_span.end {
                    pattern_edits.push((
                        (id.span.start - pattern_span.start) as usize,
                        (id.span.end - pattern_span.start) as usize,
                        read,
                    ));
                } else if let Some(span) = annotation_span {
                    annotation_edits.push((
                        (id.span.start - span.start) as usize,
                        (id.span.end - span.start) as usize,
                        read,
                    ));
                }
            }
            let pattern = apply(pattern_text, pattern_edits);
            let annotation = parameter
                .type_annotation
                .as_ref()
                .map(|annotation| {
                    apply(
                        &output[annotation.span.start as usize..annotation.span.end as usize],
                        annotation_edits,
                    )
                })
                .unwrap_or_default();
            let mut declaration = String::new();
            let read = if kind == "catch" {
                format!("({raw} as ()=>any)()")
            } else {
                format!("{raw}()")
            };
            if let BindingPattern::BindingIdentifier(id) = &parameter.pattern {
                declaration.push_str(&format!(
                    "const {}={{get value(){}{{return {read}}}}};",
                    id.name, annotation
                ));
            } else {
                let aggregate = fresh(&mut names, &format!("__crepusAwaitPattern{ordinal}"));
                let fields = bindings
                    .iter()
                    .map(|id| {
                        serde_json::to_string(id.name.as_str())
                            .map(|key| format!("[{key}]:{}", id.name))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| format!("cannot encode await bindings: {error}"))?;
                declaration.push_str(&format!(
                    "const {aggregate}={derived}(()=>{{const {pattern}{annotation}={read};return {{{}}};}});",
                    fields.join(",")
                ));
                for id in &bindings {
                    let key = serde_json::to_string(id.name.as_str())
                        .map_err(|error| format!("cannot encode await alias: {error}"))?;
                    declaration.push_str(&format!(
                        "const {}={derived}(()=>{aggregate}.value[{key}]);",
                        id.name
                    ));
                }
            }
            for id in bindings {
                projections.push((
                    id.symbol_id.get().ok_or("unresolved await binding")?,
                    format!("{}.value", id.name),
                ));
            }
            edits.push((
                parameter.span.start as usize,
                parameter.span.end as usize,
                format!(
                    "{raw}:()=>{}",
                    if kind == "catch" {
                        "unknown".to_owned()
                    } else {
                        format!("{awaited}<typeof {reader}>")
                    }
                ),
            ));
            if !callback.expression {
                return Err("await branch metadata requires an expression callback".into());
            }
            edits.push((
                callback.body.span.start as usize,
                callback.body.span.start as usize,
                format!("{{{declaration}return ("),
            ));
            edits.push((
                callback.body.span.end as usize,
                callback.body.span.end as usize,
                ");}".into(),
            ));
        }
    }
    for node in semantic.nodes().iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        if parameter_spans
            .iter()
            .any(|span| id.span.start >= span.start && id.span.end <= span.end)
        {
            continue;
        }
        let reference = semantic
            .scoping()
            .get_reference(id.reference_id.get().ok_or("unresolved await reference")?);
        let Some(symbol) = reference.symbol_id() else {
            continue;
        };
        let Some((_, read)) = projections.iter().find(|(binding, _)| *binding == symbol) else {
            continue;
        };
        if reference.is_write()
            || ((reference.is_type() || reference.flags().is_value_as_type())
                && !is_type_query(node.id(), id.span))
        {
            return Err("await bindings cannot be reassigned or used as types".into());
        }
        let mut span = id.span;
        let mut call = false;
        let mut tag = false;
        let mut container = false;
        for parent in semantic.nodes().ancestors(node.id()) {
            match parent.kind() {
                AstKind::ParenthesizedExpression(_)
                | AstKind::TSAsExpression(_)
                | AstKind::TSSatisfiesExpression(_)
                | AstKind::TSNonNullExpression(_)
                | AstKind::TSTypeAssertion(_)
                | AstKind::TSInstantiationExpression(_) => {
                    span = parent.kind().span();
                    continue;
                }
                AstKind::CallExpression(value) => call = value.callee.span() == span,
                AstKind::TaggedTemplateExpression(value) => tag = value.tag.span() == span,
                AstKind::JSXExpressionContainer(value) if value.expression.span() == span => {
                    container = true;
                    continue;
                }
                AstKind::JSXAttribute(value) if container => {
                    if matches!(&value.name,JSXAttributeName::Identifier(name) if binding_markers.contains(&name.name.as_str()))
                    {
                        return Err(
                            "await bindings cannot be bound directly; bind a member instead".into(),
                        );
                    }
                }
                _ => {}
            }
            break;
        }
        if tag || call {
            uses_unbound = true;
            // Insert around the full transparent wrapper, retaining authored casts
            // and independent edits to type queries within that wrapper.
            edits.push((
                span.start as usize,
                span.start as usize,
                format!("{unbound}("),
            ));
            edits.push((span.end as usize, span.end as usize, ")".into()));
        }
        let read = read.clone();
        let read = match semantic.nodes().parent_kind(node.id()) {
            AstKind::ObjectProperty(property) if property.shorthand => {
                crate::svelte_expression::shorthand_property(id.name.as_str(), &read)
            }
            _ => read,
        };
        edits.push((id.span.start as usize, id.span.end as usize, read));
    }
    let mut output = apply(&output, edits);
    if uses_unbound {
        output.push_str(&format!(
            "\nfunction {unbound}<T>(value:T):T{{return value;}}\n"
        ));
    }
    // The official language-tool transformer initializes only catch bindings from
    // __sveltets_2_any(); keep that compatibility cast local, not on then or props.
    // sveltejs/language-tools b1ab063, htmlxtojsx_v2/nodes/AwaitPendingCatchBlock.ts.
    let schedule = if inline_schedule { ",read=>read" } else { "" };
    Ok(format!(
        "import {{awaitBlock as {runtime},type AwaitValue as {awaited},type AwaitInput as {input_type}}} from \"@tschk/moonshine-svelte/await\";\nimport {{createDerived as {derived}}} from \"@tschk/moonshine-svelte/runes\";\nfunction {marker}<T,R>(props:{{build:()=>{{read:()=>T;pending?:()=>R;then?:(value:()=>{awaited}<()=>T>)=>R;catch?:(error:()=>unknown)=>R}}}}){{const branches=props.build();return {runtime}(branches.read as ()=>{input_type}<{awaited}<()=>T>>,branches{schedule});}}\n{output}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn transform(source: &str, marker: &str, binding_markers: &[&str]) -> Result<String, String> {
        let output = finish(&prepare(source, marker)?, marker, binding_markers, false)?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{output}\n{:?}", parsed.errors);
        Ok(output)
    }
    fn lower(header: &str, pattern: &str, body: &str) -> Result<String, String> {
        let header = serde_json::to_string(header).unwrap();
        let pattern = serde_json::to_string(pattern).unwrap();
        transform(
            &format!(
                "export default function App(){{return <Await header={{{header}}} initial={{()=> <b>pending</b>}} thenPattern={{{pattern}}} then={{()=> <>{body}</>}}/>;}}"
            ),
            "Await",
            &["binding"],
        )
    }
    #[test]
    fn decoded_script_names_do_not_capture_compiler_await_helpers() {
        for target in [
            crate::MoonshineModuleTarget::Dom,
            crate::MoonshineModuleTarget::Ssr,
        ] {
            let source = r#"<script lang="ts">const \u005f_CrepusAwaitBlock0=7;const input=Promise.resolve(1);</script>{#await input then value}<b>{value}/{__CrepusAwaitBlock0}</b>{/await}"#;
            let output = crate::compile_moonshine_svelte(
                source,
                std::path::Path::new("Hygiene.svelte"),
                target,
            )
            .unwrap()
            .source;
            assert!(output.contains("__CrepusAwaitBlock0 = 7"), "{output}");
            assert!(
                !output.contains("function __CrepusAwaitBlock0<"),
                "{output}"
            );
        }
    }
    #[test]
    fn all_helper_families_reserve_decoded_names_across_nested_adjacent_arms() {
        let source = r#"<script lang="ts">type Awaited<T>=string;type ReturnType<T>=never;type PromiseLike<T>=never;const \u005f_crepusAwaitRead=5;const \u005f_crepusAwaitValue=6;const \u005f_crepusAwaitInput=7;const \u005f_crepusAwaitDerived=10;const \u005f_crepusAwaitRuntime=20;const \u005f_crepusAwaitSource0=30;const \u005f_crepusAwaitPattern0=40;const input=Promise.resolve({});</script>{#await input then {n=__crepusAwaitSource0}}{n}/{__crepusAwaitDerived}/{__crepusAwaitRuntime}/{__crepusAwaitPattern0}{#await n then inner}{inner}{/await}{/await}{#await input then {n=__crepusAwaitPattern0}}{n}{/await}"#;
        for target in [
            crate::MoonshineModuleTarget::Dom,
            crate::MoonshineModuleTarget::Ssr,
        ] {
            let output = crate::compile_moonshine_svelte(
                source,
                std::path::Path::new("Families.svelte"),
                target,
            )
            .unwrap()
            .source;
            for name in [
                "__crepusAwaitRead",
                "__crepusAwaitValue",
                "__crepusAwaitDerived",
                "__crepusAwaitRuntime",
                "__crepusAwaitSource0",
                "__crepusAwaitPattern0",
            ] {
                assert!(output.contains(&format!("const {name} =")), "{output}");
            }
            assert!(
                !output.contains("createDerived as __crepusAwaitDerived }"),
                "{output}"
            );
            assert!(
                !output.contains("awaitBlock as __crepusAwaitRuntime }"),
                "{output}"
            );
        }
    }
    #[test]
    fn lexical_reader_is_built_once_inside_existing_marker_descriptor() {
        let output = lower("input", "value", "{value}").unwrap();
        let allocator = Allocator::default();
        assert!(
            Parser::new(&allocator, &output, SourceType::tsx())
                .parse()
                .errors
                .is_empty(),
            "{output}"
        );
        assert!(output.contains("<Await build={"), "{output}");
        assert_eq!(output.matches("props.build()").count(), 1, "{output}");
        assert!(output.contains("const __crepusAwaitRead=()=>"), "{output}");
        assert!(
            !output.contains("{(()=>{const __crepusAwaitRead"),
            "{output}"
        );
    }
    #[test]
    fn marker_renaming_changes_only_compiler_owned_jsx_names() {
        let source = r#"const text="Old"; function App(){return <Old header={"Old"} initial={()=> <b>Old</b>}/>;}"#;
        let output = rename_marker(source, "Old", "Fresh").unwrap();
        assert!(output.contains("<Fresh header={\"Old\"}"));
        assert!(output.contains("const text=\"Old\""));
        assert!(output.contains("<b>Old</b>"));
    }
    #[test]
    fn headers_use_expression_and_formal_binding_grammar() {
        for source in [
            "input",
            "flag ? first : second",
            "(/then}/).test(text) ? a : b",
            "input then value",
            "input\tthen\r\nvalue",
            "input\r\ncatch\terror",
            "input catch {message='fallback'}",
            "input then [first,,...rest]",
            "input then {nested:{value=outer}={},...rest}",
            "input then value:Result",
            "input catch error:Failure",
        ] {
            assert!(header(source).is_ok(), "{source}");
        }
        for source in [
            "",
            "input then (",
            "input then value, other",
            "input then ...rest",
            "input then value?",
            "input then value=default",
        ] {
            assert!(header(source).is_err(), "{source}");
        }
    }
    #[test]
    fn destructuring_has_one_aggregate_and_precise_leaf_projections() {
        let output = lower(
            "input",
            "{title:label='fallback',nested:{count=0}={},...rest}",
            "{label}/{count}/{rest.extra}",
        )
        .unwrap();
        assert!(output.contains("const {title:label='fallback',nested:{count=0}={},...rest}="));
        assert_eq!(
            output
                .matches("return {[\"label\"]:label,[\"count\"]:count,[\"rest\"]:rest}")
                .count(),
            1
        );
        assert!(output.contains("label.value"));
        assert!(output.contains("count.value"));
        assert!(output.contains("rest.value.extra"));
    }
    #[test]
    fn readonly_bindings_keep_member_writes_and_scope_shadows() {
        assert!(lower("input", "value", "{(()=>{value++;return value})()}").is_err());
        assert!(lower("input", "{value}", "{(()=>{value=1;return value})()}").is_err());
        assert!(lower(
            "input",
            "value",
            "{(()=>{value.count++;return value.count})()}"
        )
        .is_ok());
        let output = lower("input", "value", "{value}/{((value)=>value)(2)}").unwrap();
        assert!(output.contains("{value.value}"));
        assert!(output.contains("((value)=>value)(2)"));
    }
    #[test]
    fn callback_receiver_and_prototype_shorthand_are_preserved() {
        let output = lower(
            "input",
            "{callback,tag,__proto__}",
            "{callback?.()}/{tag`value`}/{({__proto__})}",
        )
        .unwrap();
        assert!(output.contains("__crepusAwaitUnbound(callback.value)?.()"));
        assert!(output.contains("__crepusAwaitUnbound(tag.value)`value`"));
        assert!(output.contains("[\"__proto__\"]"));
    }
    #[test]
    fn typed_bindings_and_catch_only_dynamic_boundary_remain_local() {
        let output = lower("input", "value:Result", "{value.name}").unwrap();
        assert!(output.contains("get value():Result"));
        let output=transform("function App(){return <Await header={\"input catch {message}\"} initial={()=> <b>{message}</b>}/>;}","Await",&[]).unwrap();
        assert!(output.contains("as ()=>any"));
        assert!(output.contains("read:()=>T;"));
        assert!(output.contains("catch?:(error:()=>unknown)"));
        assert!(transform("function App(){return <Await header={\"input then value\"} initial={()=>null} thenPattern={\"other\"} then={()=>null}/>;}","Await",&[]).is_err());
    }
    #[test]
    fn type_queries_project_values_and_preserve_invalid_type_references() {
        let output=lower("input", "value", "{(()=>{type V=typeof value;type M=typeof value.member;const first:V=value;return first.member})()}").unwrap();
        assert!(output.contains("typeof value.value"), "{output}");
        assert!(output.contains("typeof value.value.member"), "{output}");
        // Genuine value-used-as-type references stay untouched for the TS checker.
        let invalid = lower("input", "value", "{(()=>{type Invalid=value;return 1})()}").unwrap();
        assert!(invalid.contains("type Invalid=value"), "{invalid}");
        let invalid = lower(
            "input",
            "value",
            "{(()=>{type Invalid=value.member;return 1})()}",
        )
        .unwrap();
        assert!(invalid.contains("type Invalid=value.member"), "{invalid}");
        let source="function App(){return <Await header={\"input then outer\"} initial={()=> <Await header={\"other then {value=(()=>{type V=typeof outer;const result:V=outer;return result})()}\"} initial={()=> <b>{value}</b>}/>}/>;}";
        let output = transform(source, "Await", &[]).unwrap();
        assert!(output.contains("typeof outer.value"), "{output}");
        let source="function App(){return <Await header={\"input then outer\"} initial={()=> <Await header={\"other then value:typeof outer\"} initial={()=> <b>{value.member}</b>}/>}/>;}";
        let output = transform(source, "Await", &[]).unwrap();
        assert!(
            output.contains("get value():typeof outer.value"),
            "{output}"
        );
    }
    #[test]
    fn direct_bindings_reject_and_member_bindings_survive() {
        assert!(lower("input", "value", "<input binding={value}/>").is_err());
        assert!(lower("input", "value", "<input binding={value.name}/>").is_ok());
    }
    #[test]
    fn wrapped_calls_tags_and_method_receivers_preserve_types() {
        let output=lower("input", "{callback,tag,object}", "{(callback as ()=>string)()}/{(callback!)?.()}/{(tag as Function)`text`}/{object.method()}").unwrap();
        assert!(
            output.contains("__crepusAwaitUnbound((callback.value as ()=>string))()"),
            "{output}"
        );
        assert!(
            output.contains("__crepusAwaitUnbound((callback.value!))?.()"),
            "{output}"
        );
        assert!(
            output.contains("__crepusAwaitUnbound((tag.value as Function))`text`"),
            "{output}"
        );
        assert!(output.contains("object.value.method()"), "{output}");
        assert!(
            !output.contains("__crepusAwaitUnbound(object.value)"),
            "{output}"
        );
        assert_eq!(
            output.matches("function __crepusAwaitUnbound<T>").count(),
            1
        );
    }
    #[test]
    fn nested_defaults_unbind_outer_callbacks_through_transparent_wrappers() {
        let source="function App(){return <Await header={\"input then outer\"} initial={()=> <Await header={\"other then {value=(outer as ()=>string)(),tagged=(outer!)`text`}\"} initial={()=> <b>{value}/{tagged}</b>}/>}/>;}";
        let output = transform(source, "Await", &[]).unwrap();
        assert!(
            output.contains("value=__crepusAwaitUnbound((outer.value as ()=>string))()"),
            "{output}"
        );
        assert!(
            output.contains("tagged=__crepusAwaitUnbound((outer.value!))`text`"),
            "{output}"
        );
    }
    #[test]
    fn type_queries_do_not_allocate_runtime_unbound_helpers() {
        let output = lower("input", "value", "{(()=>{type V=typeof value;return 1})()}").unwrap();
        assert!(output.contains("typeof value.value"));
        assert!(!output.contains("function __crepusAwaitUnbound"));
    }
    #[test]
    fn instantiation_wrappers_are_unbound_without_removing_type_arguments() {
        let output = lower(
            "input",
            "callback",
            "{(callback<number>)()}/{(callback<string>)`text`}",
        )
        .unwrap();
        assert!(
            output.contains("__crepusAwaitUnbound((callback.value<number>))()"),
            "{output}"
        );
        assert!(
            output.contains("__crepusAwaitUnbound((callback.value<string>))`text`"),
            "{output}"
        );
    }
}
