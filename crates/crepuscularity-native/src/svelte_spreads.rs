//! Preserve ordered authored component and intrinsic layers through JSX lowering.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType, Span};
use std::collections::HashSet;

#[derive(Default, Debug)]
pub(crate) struct SpreadMetadata {
    pub(crate) markers: Vec<String>,
    pub(crate) element_markers: Vec<String>,
}

type Edit = (usize, usize, String);
fn render(source: &str, span: Span, edits: &[Edit]) -> String {
    let mut out = source[span.start as usize..span.end as usize].to_owned();
    let mut nested: Vec<_> = edits
        .iter()
        .filter(|(start, end, _)| *start >= span.start as usize && *end <= span.end as usize)
        .collect();
    nested.sort_by_key(|(start, end, _)| (*start, *end));
    for (start, end, text) in nested.into_iter().rev() {
        out.replace_range(start - span.start as usize..end - span.start as usize, text);
    }
    out
}
fn comments(source: &str, program: &Program<'_>, start: usize, end: usize) -> String {
    let mut out = String::new();
    for comment in &program.comments {
        if comment.span.start as usize >= start && comment.span.end as usize <= end {
            out.push_str(&source[comment.span.start as usize..comment.span.end as usize]);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
pub(crate) fn prepare(source: &str) -> Result<(String, SpreadMetadata), String> {
    prepare_target(source, MoonshineModuleTarget::Dom, None)
}

pub(crate) fn prepare_target(
    source: &str,
    target: MoonshineModuleTarget,
    namespace: Option<&str>,
) -> Result<(String, SpreadMetadata), String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid authored Svelte spread syntax".into());
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut edits = Vec::new();
    let mut metadata = SpreadMetadata::default();
    let mut index = 0;
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if matches!(target, MoonshineModuleTarget::Ssr)
            && matches!(&element.opening_element.name, JSXElementName::Identifier(_))
            && element
                .opening_element
                .attributes
                .iter()
                .any(|attribute| matches!(attribute, JSXAttributeItem::SpreadAttribute(_)))
        {
            for attribute in &element.opening_element.attributes {
                let JSXAttributeItem::Attribute(attribute) = attribute else {
                    continue;
                };
                if matches!(&attribute.name, JSXAttributeName::Identifier(name) if Some(name.name.as_str()) == namespace)
                {
                    continue;
                }
                let Some(JSXAttributeValue::StringLiteral(value)) = &attribute.value else {
                    continue;
                };
                // Svelte's SSR spread object retains authored entity spelling.
                let literal = serde_json::to_string(value.value.as_str())
                    .map_err(|error| error.to_string())?;
                edits.push((
                    value.span.start as usize,
                    value.span.end as usize,
                    format!("{{{literal}}}"),
                ));
            }
        }
        for attribute in &element.opening_element.attributes {
            let JSXAttributeItem::SpreadAttribute(spread) = attribute else {
                continue;
            };
            let component = matches!(
                &element.opening_element.name,
                JSXElementName::IdentifierReference(_) | JSXElementName::MemberExpression(_)
            );
            if !component && !matches!(&element.opening_element.name, JSXElementName::Identifier(_))
            {
                return Err("unsupported authored Svelte spread element".into());
            }
            let marker = loop {
                let name = if component {
                    format!("__crepusSvelteSpread{index}")
                } else {
                    format!("__crepusSvelteElementSpread{index}")
                };
                index += 1;
                if !source.contains(&name) {
                    break name;
                }
            };
            let trivia = comments(
                source,
                &parsed.program,
                spread.span.start as usize,
                spread.argument.span().start as usize,
            );
            edits.push((
                spread.span.start as usize,
                spread.argument.span().start as usize,
                format!("{marker}={{{trivia}"),
            ));
            if component {
                metadata.markers.push(marker);
            } else {
                metadata.element_markers.push(marker);
            }
        }
    }
    edits.sort_by_key(|edit| edit.0);
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    Ok((out, metadata))
}

fn object_create(
    call: &CallExpression<'_>,
    globals: &crate::source_globals::SourceGlobals,
) -> bool {
    matches!(&call.callee,Expression::StaticMemberExpression(create) if create.property.name=="create"&&matches!(&create.object,Expression::StaticMemberExpression(object) if object.property.name=="Object"&&globals.matches_realm(&object.object)))
        && call.arguments.len() == 2
        && matches!(call.arguments.first(),Some(Argument::StaticMemberExpression(prototype)) if prototype.property.name=="prototype"&&matches!(&prototype.object,Expression::StaticMemberExpression(object) if object.property.name=="Object"&&globals.matches_realm(&object.object)))
}
fn marker<'a>(property: &'a ObjectPropertyKind<'_>, metadata: &SpreadMetadata) -> Option<&'a str> {
    let ObjectPropertyKind::ObjectProperty(property) = property else {
        return None;
    };
    match &property.key {
        PropertyKey::StaticIdentifier(name)
            if !property.computed
                && metadata
                    .markers
                    .iter()
                    .any(|marker| marker == name.name.as_str()) =>
        {
            Some(name.name.as_str())
        }
        PropertyKey::StringLiteral(name)
            if !property.computed
                && metadata
                    .markers
                    .iter()
                    .any(|marker| marker == name.value.as_str()) =>
        {
            Some(name.value.as_str())
        }
        _ => None,
    }
}

fn finish_elements(
    source: &str,
    metadata: &SpreadMetadata,
    target: MoonshineModuleTarget,
    globals: &crate::source_globals::SourceGlobals,
) -> Result<(String, Option<String>), String> {
    if metadata.element_markers.is_empty() {
        return Ok((source.to_owned(), None));
    }
    if matches!(target, MoonshineModuleTarget::Dom) {
        if metadata
            .element_markers
            .iter()
            .any(|marker| source.contains(marker))
        {
            return Err("intrinsic Svelte spread marker was not lowered".into());
        }
        return Ok((
            source.to_owned(),
            Some("@tschk/moonshine-svelte/runes".into()),
        ));
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid compiled intrinsic Svelte spread syntax".into());
    }
    let aliases: HashSet<_> = parsed.program.body.iter().filter_map(|statement| {
        let Statement::ImportDeclaration(import) = statement else { return None; };
        if import.source.value != "@tschk/moonshine-dom/ssr" { return None; }
        Some(import.specifiers.iter().flatten().filter_map(|specifier| {
            let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else { return None; };
            if matches!(&specifier.imported, ModuleExportName::IdentifierName(name) if name.name=="hSource") { Some(specifier.local.name.as_str()) } else { None }
        }))
    }).flatten().collect();
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name) {
            name.push('_');
        }
        name
    };
    let factory = fresh("__crepusSpreadAttributeElement");
    let helper = fresh("__crepusSpreadAttributeEntries");
    let form_helper = fresh("__crepusSpreadFormElement");
    let mut forms_used = false;
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut edits: Vec<Edit> = Vec::new();
    let mut consumed = HashSet::new();
    let calls: Vec<_> = semantic.nodes().iter().filter_map(|node| match node.kind() {
        AstKind::CallExpression(call) if matches!(&call.callee, Expression::Identifier(name) if aliases.contains(name.name.as_str())) => Some(call),
        _ => None,
    }).collect();
    for call in calls.into_iter().rev() {
        let Some(Argument::ObjectExpression(object)) = call.arguments.get(1) else {
            continue;
        };
        if !object.properties.iter().any(|property| matches!(property, ObjectPropertyKind::ObjectProperty(property) if property.key.static_name().is_some_and(|name| metadata.element_markers.iter().any(|marker| marker == name.as_ref())))) { continue; }
        let mut layers = Vec::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return Err("invalid intrinsic Svelte spread property".into());
            };
            let Some(name) = property.key.static_name() else {
                return Err("invalid intrinsic Svelte spread key".into());
            };
            let raw = render(source, property.value.span(), &edits);
            let value = if matches!(
                property.value.get_inner_expression(),
                Expression::ArrowFunctionExpression(_)
            ) {
                format!("({raw})()")
            } else {
                raw
            };
            if metadata
                .element_markers
                .iter()
                .any(|marker| marker == name.as_ref())
            {
                if !consumed.insert(name.to_string()) {
                    return Err("duplicate intrinsic Svelte spread marker".into());
                }
                // Snapshot here: getters precede evaluation of the next source layer.
                let realm = &globals.realm;
                layers.push(format!(
                    "{realm}.Object.assign({realm}.Object.create(null),{value})"
                ));
            } else {
                let name = if name == "className" {
                    "class"
                } else {
                    name.as_ref()
                };
                let name = serde_json::to_string(name).map_err(|error| error.to_string())?;
                layers.push(format!("{{[{name}]:{value}}}"));
            }
        }
        let tag = call
            .arguments
            .first()
            .ok_or("missing intrinsic Svelte spread tag")?;
        let children = call
            .arguments
            .iter()
            .skip(2)
            .map(|argument| render(source, argument.span(), &edits))
            .collect::<Vec<_>>();
        let tail = if children.is_empty() {
            String::new()
        } else {
            format!(",{}", children.join(","))
        };
        let is_form = matches!(tag, Argument::StringLiteral(value) if matches!(value.value.as_str(), "input" | "textarea" | "select" | "option"));
        let replacement = if is_form {
            forms_used = true;
            format!(
                "{form_helper}({tag},[{}]{tail})",
                layers.join(","),
                tag = render(source, tag.span(), &edits)
            )
        } else {
            format!(
                "{factory}({tag},{helper}([{}]){tail})",
                layers.join(","),
                tag = render(source, tag.span(), &edits)
            )
        };
        let span = call.span;
        edits.retain(|(start, end, _)| {
            !(*start >= span.start as usize && *end <= span.end as usize)
        });
        edits.push((span.start as usize, span.end as usize, replacement));
    }
    if metadata
        .element_markers
        .iter()
        .any(|marker| !consumed.contains(marker))
    {
        return Err("intrinsic Svelte spread marker was not lowered".into());
    }
    edits.sort_by_key(|edit| edit.0);
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    out = format!("import{{hAttributeEntries as {factory}}}from\"@tschk/moonshine-dom/ssr\";\nimport{{spreadServerEntries as {helper}}}from\"@tschk/moonshine-svelte/ssr\";\n{out}");
    if forms_used {
        out = format!("import{{spreadServerElement as {form_helper}}}from\"@tschk/moonshine-svelte/ssr\";\n{out}");
    }
    Ok((out, Some("@tschk/moonshine-svelte/ssr".into())))
}

pub(crate) fn finish(
    source: &str,
    metadata: &SpreadMetadata,
    target: MoonshineModuleTarget,
    globals: &crate::source_globals::SourceGlobals,
) -> Result<(String, Option<String>), String> {
    let realm = &globals.realm;
    let (element_source, element_import) = finish_elements(source, metadata, target, globals)?;
    let source = element_source.as_str();
    if metadata.markers.is_empty() {
        return Ok((source.to_owned(), element_import));
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid compiled Svelte spread syntax".into());
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut alias = "__crepusSvelteSpreadProps".to_owned();
    while source.contains(&alias) {
        alias.push('_');
    }
    let mut edits: Vec<Edit> = Vec::new();
    let mut consumed = HashSet::new();
    let calls: Vec<_> = semantic
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::CallExpression(call) if object_create(call, globals) => Some(call),
            _ => None,
        })
        .collect();
    for call in calls.into_iter().rev() {
        let Some(Argument::ObjectExpression(object)) = call.arguments.get(1) else {
            continue;
        };
        if !object
            .properties
            .iter()
            .any(|property| marker(property, metadata).is_some())
        {
            continue;
        }
        let mut layers = Vec::new();
        let mut chunk: Option<Span> = None;
        let mut cursor = object.span.start as usize + 1;
        let mut pending = String::new();
        for property in &object.properties {
            let span = property.span();
            let gap = comments(source, &parsed.program, cursor, span.start as usize);
            cursor = span.end as usize;
            if let Some(name) = marker(property, metadata) {
                if !consumed.insert(name.to_owned()) {
                    return Err("duplicate compiled Svelte spread marker".into());
                }
                if let Some(span) = chunk.take() {
                    layers.push(format!(
                        "{pending}{realm}.Object.create({realm}.Object.prototype,{{{}}})",
                        render(source, span, &edits)
                    ));
                    pending.clear();
                }
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    unreachable!()
                };
                let Expression::ObjectExpression(descriptor) =
                    property.value.get_inner_expression()
                else {
                    return Err("invalid compiled Svelte spread descriptor".into());
                };
                let getters:Vec<_>=descriptor.properties.iter().filter_map(|property|match property{ObjectPropertyKind::ObjectProperty(property) if !property.computed&&matches!(&property.key,PropertyKey::StaticIdentifier(key) if key.name=="get")=>Some(property),_=>None}).collect();
                if getters.len() != 1 {
                    return Err(
                        "compiled Svelte spread descriptor requires exactly one getter".into(),
                    );
                }
                let enumerable=descriptor.properties.iter().filter(|property|matches!(property,ObjectPropertyKind::ObjectProperty(property) if !property.computed&&matches!(&property.key,PropertyKey::StaticIdentifier(key) if key.name=="enumerable")&&matches!(&property.value,Expression::BooleanLiteral(value) if value.value))).count();
                if descriptor.properties.len() != 2
                    || enumerable != 1
                    || !matches!(
                        getters[0].value.get_inner_expression(),
                        Expression::ArrowFunctionExpression(_)
                            | Expression::FunctionExpression(_)
                            | Expression::CallExpression(_)
                    )
                {
                    return Err("invalid compiled Svelte spread getter descriptor".into());
                }
                let getter = render(source, getters[0].value.span(), &edits);
                layers.push(match target {
                    MoonshineModuleTarget::Dom => format!("{gap}({getter})"),
                    MoonshineModuleTarget::Ssr => format!("{gap}({getter})()"),
                });
            } else {
                let ObjectPropertyKind::ObjectProperty(property) = property else {
                    return Err("invalid compiled Svelte ordinary prop descriptor".into());
                };
                let Expression::ObjectExpression(descriptor) =
                    property.value.get_inner_expression()
                else {
                    return Err("invalid compiled Svelte ordinary prop descriptor".into());
                };
                let configurable=descriptor.properties.iter().find_map(|property|match property {
                    ObjectPropertyKind::ObjectProperty(property) if !property.computed&&matches!(&property.key,PropertyKey::StaticIdentifier(key) if key.name=="configurable")=>Some(property.value.span()),
                    _=>None,
                });
                if let Some(span) = configurable {
                    edits.push((span.start as usize, span.end as usize, "true".into()));
                } else {
                    // Ordinary authored prop layers behave like object literals:
                    // later layers may redefine the same property during SSR.
                    let at = descriptor.span.start as usize + 1;
                    edits.push((at, at, "configurable:true,".into()));
                }
                if let Some(chunk) = &mut chunk {
                    chunk.end = span.end;
                } else {
                    pending = gap;
                    chunk = Some(span);
                }
            }
        }
        if let Some(span) = chunk {
            layers.push(format!(
                "{pending}{realm}.Object.create({realm}.Object.prototype,{{{}}})",
                render(source, span, &edits)
            ));
        }
        let tail = comments(source, &parsed.program, cursor, object.span.end as usize);
        let replacement = match target {
            MoonshineModuleTarget::Dom => format!("{alias}({}{tail})", layers.join(",")),
            MoonshineModuleTarget::Ssr => format!("{alias}([{}{tail}])", layers.join(",")),
        };
        let span = call.span;
        edits.retain(|(start, end, _)| {
            !(*start >= span.start as usize && *end <= span.end as usize)
        });
        edits.push((span.start as usize, span.end as usize, replacement));
    }
    if metadata
        .markers
        .iter()
        .any(|marker| !consumed.contains(marker))
    {
        return Err("compiled Svelte spread marker was not lowered".into());
    }
    edits.sort_by_key(|edit| edit.0);
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    let (name, spec) = match target {
        MoonshineModuleTarget::Dom => ("spreadProps", "@tschk/moonshine-svelte/runes"),
        MoonshineModuleTarget::Ssr => ("spreadServerProps", "@tschk/moonshine-svelte/ssr"),
    };
    out = format!("import {{{name} as {alias}}} from {spec:?};\n{out}");
    Ok((out, Some(spec.to_owned())))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid(source: &str) {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{source}", parsed.errors);
    }
    fn descriptor(name: &str, read: &str) -> String {
        format!("{name}:{{enumerable:true,get:{read}}}")
    }
    fn object(props: &str) -> String {
        format!("globalThis.Object.create(globalThis.Object.prototype,{{{props}}})")
    }
    #[test]
    fn prepare_preserves_order_expressions_and_component_scope() {
        let(source,metadata)=prepare("function C(){return <Child before={1} {...first} middle={2} {...make()}><Other {...other}/></Child>}").unwrap();
        valid(&source);
        assert_eq!(metadata.markers.len(), 3);
        assert!(source.contains(
            "before={1} __crepusSvelteSpread0={first} middle={2} __crepusSvelteSpread1={make()}"
        ));
        assert!(source.contains("<Other __crepusSvelteSpread2={other}/>"));
        let (element, metadata) = prepare("function C(){return <div {...props}/>} ").unwrap();
        assert!(metadata.markers.is_empty());
        assert_eq!(metadata.element_markers.len(), 1);
        assert!(element.contains("__crepusSvelteElementSpread0={props}"));
    }
    #[test]
    fn dom_merges_ordered_live_layers_and_keeps_children_and_symbol_descriptors() {
        let metadata = SpreadMetadata {
            element_markers: vec![],
            markers: vec!["mark".into()],
        };
        let source = format!(
            "const props={};",
            object(&format!(
                "{}, {}, {}, {}",
                descriptor("before", "()=>1"),
                descriptor("mark", "()=>layer"),
                descriptor("[ref]", "()=>binding"),
                descriptor("children", "readChildren")
            ))
        );
        let (out, spec) = finish(
            &source,
            &metadata,
            MoonshineModuleTarget::Dom,
            &Default::default(),
        )
        .unwrap();
        valid(&out);
        assert_eq!(spec.as_deref(), Some("@tschk/moonshine-svelte/runes"));
        assert!(out.contains("spreadProps as __crepusSvelteSpreadProps"));
        assert!(out.contains("(()=>layer),globalThis.Object.create"));
        assert!(!out.contains("mark:{"));
        assert!(out.contains("[ref]:{configurable:true,enumerable:true,get:()=>binding}"));
        assert!(out.contains("children:{configurable:true,enumerable:true,get:readChildren}"));
    }
    #[test]
    fn ssr_reads_spread_once_and_merges_ordered_chunks() {
        let metadata = SpreadMetadata {
            element_markers: vec![],
            markers: vec!["mark".into()],
        };
        let source = format!(
            "const props={};",
            object(&format!(
                "{},{}",
                descriptor("mark", "cached(()=>layer,true)"),
                descriptor("after", "()=>2")
            ))
        );
        let (out, spec) = finish(
            &source,
            &metadata,
            MoonshineModuleTarget::Ssr,
            &Default::default(),
        )
        .unwrap();
        valid(&out);
        assert_eq!(spec.as_deref(), Some("@tschk/moonshine-svelte/ssr"));
        assert!(out.contains(
            "__crepusSvelteSpreadProps([(cached(()=>layer,true))(),globalThis.Object.create"
        ));
        assert!(out.contains("spreadServerProps as"));
    }
    #[test]
    fn nested_component_spreads_coalesce_inner_edits() {
        let metadata = SpreadMetadata {
            element_markers: vec![],
            markers: vec!["outer".into(), "inner".into()],
        };
        let nested = object(&descriptor("inner", "()=>nested"));
        let outer = object(&format!(
            "{},{}",
            descriptor("outer", "()=>layer"),
            descriptor("content", &format!("()=>h(Child,{nested})"))
        ));
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let (out, _) = finish(
                &format!("const props={outer};"),
                &metadata,
                target,
                &Default::default(),
            )
            .unwrap();
            valid(&out);
            assert_eq!(out.matches("__crepusSvelteSpreadProps(").count(), 2);
            assert!(!out.contains("outer:{"));
            assert!(!out.contains("inner:{"));
        }
    }
    #[test]
    fn aliases_comments_and_existing_prop_memoization_survive() {
        let(source,metadata)=prepare("const __crepusSvelteSpread0=0;function C(){function make(){}return <Child {... /*keep*/ make()}/>}").unwrap();
        valid(&source);
        assert_eq!(metadata.markers, ["__crepusSvelteSpread1"]);
        assert!(source.contains("/*keep*/"));
        let (out, _) = crate::svelte_component_props::transform(
            &source,
            std::path::Path::new("spread.svelte"),
            &[],
            None,
        )
        .unwrap();
        assert!(out.contains("__crepusSvelteComponentProp"));
        assert!(out.contains("make()),true"));
        let source=format!("const __crepusSvelteSpreadProps=0;const props={};",object("/*first*/a:{enumerable:true,get:()=>1},/*layer*/mark:{enumerable:true,get:()=>layer},/*last*/b:{enumerable:true,get:()=>2}/*tail*/"));
        let (out, _) = finish(
            &source,
            &SpreadMetadata {
                element_markers: vec![],
                markers: vec!["mark".into()],
            },
            MoonshineModuleTarget::Dom,
            &Default::default(),
        )
        .unwrap();
        valid(&out);
        assert!(out.contains("as __crepusSvelteSpreadProps_"));
        for comment in ["/*first*/", "/*layer*/", "/*last*/", "/*tail*/"] {
            assert!(out.contains(comment));
        }
    }
    #[test]
    fn overlapping_ordinary_layers_are_configurable_without_losing_descriptors() {
        let metadata = SpreadMetadata {
            element_markers: vec![],
            markers: vec!["mark".into()],
        };
        let source=format!("const props={};",object("same:{enumerable:true,get:()=>before},mark:{enumerable:true,get:()=>spread},same:{enumerable:true,get:()=>after},[ref]:{enumerable:true,get:()=>binding}"));
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let (out, _) = finish(&source, &metadata, target, &Default::default()).unwrap();
            valid(&out);
            assert_eq!(out.matches("configurable:true").count(), 3);
            assert!(out.contains("same:{configurable:true,enumerable:true,get:()=>before}"));
            assert!(out.contains("same:{configurable:true,enumerable:true,get:()=>after}"));
            assert!(out.contains("[ref]:{configurable:true,enumerable:true,get:()=>binding}"));
            assert!(!out.contains("mark:{"));
        }
    }
    #[test]
    fn malformed_or_missing_descriptors_fail_explicitly() {
        let metadata = SpreadMetadata {
            element_markers: vec![],
            markers: vec!["mark".into()],
        };
        for value in [
            "mark:layer",
            "mark:{enumerable:true}",
            "mark:{enumerable:true,get:1}",
            "mark:{enumerable:false,get:()=>layer}",
        ] {
            assert!(finish(
                &format!("const props={};", object(value)),
                &metadata,
                MoonshineModuleTarget::Dom,
                &Default::default(),
            )
            .is_err());
        }
        assert!(finish(
            "const props={};",
            &metadata,
            MoonshineModuleTarget::Dom,
            &Default::default()
        )
        .is_err());
        assert!(prepare("const broken=;").is_err());
    }
    #[test]
    fn supplied_realm_matches_only_owned_objects_and_preserves_authored_reads() {
        let globals =
            crate::source_globals::SourceGlobals::with_aliases("__Realm", "__Types", true);
        let metadata = SpreadMetadata {
            element_markers: vec![],
            markers: vec!["mark".into()],
        };
        let props =
            "a:{enumerable:true,get:()=>globalThis.value},mark:{enumerable:true,get:()=>layer}";
        let source = format!(
            "const props={realm}.Object.create({realm}.Object.prototype,{{{props}}});",
            realm = globals.realm
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let (output, _) = finish(&source, &metadata, target, &globals).unwrap();
            valid(&output);
            assert!(output.contains("globalThis.value"), "{output}");
            assert!(output.contains("(__Realm()).Object.create"), "{output}");
            assert!(!output.contains("mark:{"), "{output}");
            let stale = format!("const props={};", object(props));
            assert!(finish(&stale, &metadata, target, &globals).is_err());
        }
    }
}
