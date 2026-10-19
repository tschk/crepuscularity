use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::collections::HashSet;

type Edit = (usize, usize, String);

fn is_marker(name: &JSXElementName<'_>, marker: &str) -> bool {
    matches!(name, JSXElementName::IdentifierReference(id) if id.name == marker)
}

fn markup_parent(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::JSXElement(_) | AstKind::JSXFragment(_) | AstKind::JSXAttribute(_)
    )
}

fn unique(
    base: &str,
    fresh: &dyn Fn(&str) -> String,
    source: &str,
    reserved: &HashSet<String>,
    taken: &[&str],
) -> String {
    let mut name = fresh(base);
    while source.contains(&name) || reserved.contains(&name) || taken.contains(&name.as_str()) {
        name.push('_');
    }
    name
}

fn apply(source: &str, mut edits: Vec<Edit>) -> String {
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    out
}

fn flag(slot: &mut Option<bool>, name: &str, attribute: &JSXAttribute<'_>) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("duplicate title marker attribute {name}"));
    }
    let value = match &attribute.value {
        None => true,
        Some(JSXAttributeValue::ExpressionContainer(container)) => {
            match container.expression.as_expression() {
                Some(Expression::BooleanLiteral(value)) => value.value,
                _ => return Err(format!("title marker {name} requires a boolean literal")),
            }
        }
        Some(_) => return Err(format!("title marker {name} requires a boolean literal")),
    };
    *slot = Some(value);
    Ok(())
}

fn title_props(element: &JSXElement<'_>, source: &str) -> Result<String, String> {
    if element.closing_element.is_some() || !element.children.is_empty() {
        return Err("title marker must be self-closing".into());
    }
    let mut parts = None;
    let mut calls = None;
    let mut strip = None;
    for item in &element.opening_element.attributes {
        let JSXAttributeItem::Attribute(attribute) = item else {
            return Err("title marker spreads are unsupported".into());
        };
        let JSXAttributeName::Identifier(name) = &attribute.name else {
            return Err("title marker namespaced attributes are unsupported".into());
        };
        match name.name.as_str() {
            "parts" => {
                if parts.is_some() {
                    return Err("duplicate title marker attribute parts".into());
                }
                let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value
                else {
                    return Err("title marker parts requires an array expression".into());
                };
                let Some(Expression::ArrayExpression(array)) = container.expression.as_expression()
                else {
                    return Err("title marker parts requires an array expression".into());
                };
                parts = Some(&source[array.span.start as usize..array.span.end as usize]);
            }
            "calls" => flag(&mut calls, "calls", attribute)?,
            "stripLeadingNewline" => flag(&mut strip, "stripLeadingNewline", attribute)?,
            other => return Err(format!("unexpected title marker attribute {other}")),
        }
    }
    let parts = parts.ok_or("title marker requires parts")?;
    let calls = calls.map_or(String::new(), |value| format!(",calls:{value}"));
    let strip = strip.map_or(String::new(), |value| {
        format!(",stripLeadingNewline:{value}")
    });
    Ok(format!("{{parts:{parts}{calls}{strip}}}"))
}

fn reserved_names(semantic: &oxc_semantic::Semantic<'_>) -> HashSet<String> {
    semantic
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::BindingIdentifier(id) => Some(id.name.to_string()),
            AstKind::IdentifierReference(id) => Some(id.name.to_string()),
            _ => None,
        })
        .collect()
}

fn lower_titles(
    source: &str,
    head: Option<&str>,
    title: &str,
    reader: &str,
    target: MoonshineModuleTarget,
    fresh: &dyn Fn(&str) -> String,
) -> Result<(String, Option<String>), String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid head title source syntax".into());
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    let semantic = built.semantic;
    let nodes = semantic.nodes();
    let mut found = Vec::new();
    for node in nodes.iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !is_marker(&element.opening_element.name, title) {
            continue;
        }
        let inside_head = head.is_some_and(|head| {
            nodes.ancestors(node.id()).any(|ancestor| {
                matches!(ancestor.kind(), AstKind::JSXElement(parent)
                    if is_marker(&parent.opening_element.name, head))
            })
        });
        if !inside_head {
            return Err("title marker must be inside a head marker".into());
        }
        let props = title_props(element, source)?;
        found.push((
            element.span,
            markup_parent(nodes.parent_kind(node.id())),
            props,
            match nodes.parent_kind(node.id()) {
                AstKind::JSXElement(parent) => Some(parent.opening_element.span.end as usize),
                AstKind::JSXFragment(parent) => Some(parent.span.start as usize + 2),
                _ => None,
            },
        ));
    }
    if found.is_empty() {
        return Ok((source.to_owned(), None));
    }
    let reserved = reserved_names(&semantic);
    let taken = [reader, title, head.unwrap_or_default()];
    let alias = unique("__crepusSvelteTitle", fresh, source, &reserved, &taken);
    let mut hoisted = std::collections::BTreeMap::<usize, String>::new();
    let mut edits: Vec<Edit> = found
        .into_iter()
        .map(|(span, markup, props, hoist)| {
            let call = match target {
                MoonshineModuleTarget::Dom => format!("{alias}({reader}({props}))"),
                MoonshineModuleTarget::Ssr => format!("{alias}(()=>{reader}({props}))"),
            };
            let text = if markup { format!("{{{call}}}") } else { call };
            if let Some(at) = hoist {
                hoisted.entry(at).or_default().push_str(&text);
                (span.start as usize, span.end as usize, String::new())
            } else {
                (span.start as usize, span.end as usize, text)
            }
        })
        .collect();
    edits.extend(hoisted.into_iter().map(|(at, text)| (at, at, text)));
    Ok((apply(source, edits), Some(alias)))
}

fn lower_heads(
    source: &str,
    head: &str,
    taken: &[&str],
    fresh: &dyn Fn(&str) -> String,
) -> Result<(String, Option<String>), String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid head source syntax".into());
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    let semantic = built.semantic;
    let nodes = semantic.nodes();
    let mut found = Vec::new();
    for node in nodes.iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !is_marker(&element.opening_element.name, head) {
            continue;
        }
        if !element.opening_element.attributes.is_empty() {
            return Err("head marker does not accept attributes".into());
        }
        if nodes.ancestors(node.id()).any(|ancestor| {
            matches!(ancestor.kind(), AstKind::JSXElement(parent)
                if is_marker(&parent.opening_element.name, head))
        }) {
            return Err("head marker cannot be nested".into());
        }
        let children = element.closing_element.as_ref().map_or("", |closing| {
            &source[element.opening_element.span.end as usize..closing.span.start as usize]
        });
        found.push((
            element.span(),
            markup_parent(nodes.parent_kind(node.id())),
            children.to_owned(),
        ));
    }
    if found.is_empty() {
        return Ok((source.to_owned(), None));
    }
    let reserved = reserved_names(&semantic);
    let alias = unique("__crepusSvelteHead", fresh, source, &reserved, taken);
    let edits = found
        .into_iter()
        .map(|(span, markup, children)| {
            let call = format!("{alias}(()=> <>{children}</>)");
            let text = if markup { format!("{{{call}}}") } else { call };
            (span.start as usize, span.end as usize, text)
        })
        .collect();
    Ok((apply(source, edits), Some(alias)))
}

pub(crate) fn transform(
    source: &str,
    head_marker: Option<&str>,
    title_marker: Option<&str>,
    title_reader_alias: &str,
    target: MoonshineModuleTarget,
    fresh: &dyn Fn(&str) -> String,
) -> Result<String, String> {
    if head_marker.is_none() && title_marker.is_none() {
        return Ok(source.to_owned());
    }
    let (source, title_alias) = match title_marker {
        Some(title) => lower_titles(
            source,
            head_marker,
            title,
            title_reader_alias,
            target,
            fresh,
        )?,
        None => (source.to_owned(), None),
    };
    let (source, head_alias) = match head_marker {
        Some(head) => {
            let taken = [
                title_reader_alias,
                head,
                title_marker.unwrap_or_default(),
                title_alias.as_deref().unwrap_or_default(),
            ];
            lower_heads(&source, head, &taken, fresh)?
        }
        None => (source, None),
    };
    let mut imports = Vec::new();
    if let Some(alias) = &head_alias {
        imports.push(format!("head as {alias}"));
    }
    if let Some(alias) = &title_alias {
        imports.push(format!("title as {alias}"));
    }
    if imports.is_empty() {
        return Ok(source);
    }
    let module = match target {
        MoonshineModuleTarget::Dom => "@tschk/moonshine-dom/head",
        MoonshineModuleTarget::Ssr => "@tschk/moonshine-dom/head-ssr",
    };
    Ok(format!(
        "import {{{}}} from \"{module}\";\n{source}",
        imports.join(",")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const READER: &str = "__crepusSvelteTitleReader";

    fn lower(source: &str, target: MoonshineModuleTarget) -> Result<String, String> {
        let output = transform(
            source,
            Some("HeadMarker"),
            Some("TitleMarker"),
            READER,
            target,
            &|base| base.to_owned(),
        )?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{output}");
        Ok(output)
    }

    const BOTH: [MoonshineModuleTarget; 2] =
        [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr];

    #[test]
    fn dom_and_ssr_call_shapes() {
        let source = "export default function C(){return <HeadMarker><TitleMarker parts={[\"Hi \",()=>name]}/><meta name=\"x\"/></HeadMarker>}";
        let dom = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert_eq!(
            dom,
            "import {head as __crepusSvelteHead,title as __crepusSvelteTitle} from \"@tschk/moonshine-dom/head\";\nexport default function C(){return __crepusSvelteHead(()=> <>{__crepusSvelteTitle(__crepusSvelteTitleReader({parts:[\"Hi \",()=>name]}))}<meta name=\"x\"/></>)}"
        );
        let ssr = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert_eq!(
            ssr,
            "import {head as __crepusSvelteHead,title as __crepusSvelteTitle} from \"@tschk/moonshine-dom/head-ssr\";\nexport default function C(){return __crepusSvelteHead(()=> <>{__crepusSvelteTitle(()=>__crepusSvelteTitleReader({parts:[\"Hi \",()=>name]}))}<meta name=\"x\"/></>)}"
        );
        for output in [dom, ssr] {
            assert!(!output.contains("HeadMarker") && !output.contains("TitleMarker"));
        }
    }

    #[test]
    fn head_placement_follows_parent_context() {
        for target in BOTH {
            let child = lower(
                "export default ()=> <><HeadMarker><link rel=\"icon\"/></HeadMarker><p/></>",
                target,
            )
            .unwrap();
            assert!(
                child.contains("<>{__crepusSvelteHead(()=> <><link rel=\"icon\"/></>)}<p/></>"),
                "{child}"
            );
            let expression =
                lower("export default ()=> <>{open && <HeadMarker/>}</>", target).unwrap();
            assert!(
                expression.contains("{open && __crepusSvelteHead(()=> <></>)}"),
                "{expression}"
            );
            let conditional = lower(
                "export default ()=> <HeadMarker>{open && <TitleMarker parts={[\"a\"]}/>}{<TitleMarker parts={[]}/>}</HeadMarker>",
                target,
            )
            .unwrap();
            assert!(
                conditional.contains("{open && __crepusSvelteTitle("),
                "{conditional}"
            );
            assert!(
                conditional.contains("{__crepusSvelteTitle("),
                "{conditional}"
            );
            assert!(conditional.contains("export default ()=> __crepusSvelteHead(()=> <>"));
            let attribute = lower("export default ()=> <p data=<HeadMarker/>/>", target).unwrap();
            assert!(
                attribute.contains("data={__crepusSvelteHead(()=> <></>)}"),
                "{attribute}"
            );
        }
    }

    #[test]
    fn nested_template_callbacks_components_and_namespaces_are_preserved() {
        let source = "export default ()=> <HeadMarker><Child render={()=> <svg:use xlink:href=\"#a\"/>}/>{items.map(item=> <meta content={item}/>)}</HeadMarker>";
        for target in BOTH {
            let output = lower(source, target).unwrap();
            assert!(output.contains("<Child render={()=> <svg:use xlink:href=\"#a\"/>}/>"));
            assert!(output.contains("{items.map(item=> <meta content={item}/>)}"));
            assert_eq!(output.matches("__crepusSvelteHead(").count(), 1);
            assert!(!output.contains("title as"), "{output}");
        }
    }

    #[test]
    fn missing_markers_leave_source_unchanged_without_parsing_or_naming() {
        let source = "const a=;<<<";
        for target in BOTH {
            let output = transform(source, None, None, READER, target, &|_| {
                panic!("alias requested")
            })
            .unwrap();
            assert_eq!(output, source);
        }
        let plain = "export default ()=> <p>{x}</p>";
        for target in BOTH {
            assert_eq!(lower(plain, target).unwrap(), plain);
        }
    }

    #[test]
    fn text_and_entity_parts_are_retained_untouched() {
        let source = "export default ()=> <HeadMarker><TitleMarker parts={[{text:\"a &amp; b\"},\"&lt;\\n\",()=>x,{value:call()}]} calls stripLeadingNewline={false}/></HeadMarker>";
        for target in BOTH {
            let output = lower(source, target).unwrap();
            assert!(
                output.contains("{parts:[{text:\"a &amp; b\"},\"&lt;\\n\",()=>x,{value:call()}],calls:true,stripLeadingNewline:false}"),
                "{output}"
            );
        }
        for target in BOTH {
            let output = lower(
                "export default ()=> <HeadMarker><TitleMarker parts={[]}/></HeadMarker>",
                target,
            )
            .unwrap();
            assert!(output.contains("({parts:[]})"), "{output}");
        }
    }

    #[test]
    fn malformed_ir_is_rejected() {
        let wrap = |inner: &str| format!("export default ()=> <HeadMarker>{inner}</HeadMarker>");
        for inner in [
            "<TitleMarker/>",
            "<TitleMarker parts=\"x\"/>",
            "<TitleMarker parts={rows}/>",
            "<TitleMarker parts={([\"a\"])}/>",
            "<TitleMarker parts={[]} parts={[]}/>",
            "<TitleMarker parts={[]} extra/>",
            "<TitleMarker parts={[]} {...props}/>",
            "<TitleMarker parts={[]} xml:calls/>",
            "<TitleMarker parts={[]} calls=\"yes\"/>",
            "<TitleMarker parts={[]} calls={flag}/>",
            "<TitleMarker parts={[]} calls calls/>",
            "<TitleMarker parts={[]}>x</TitleMarker>",
            "<TitleMarker parts={[]}></TitleMarker>",
        ] {
            for target in BOTH {
                assert!(lower(&wrap(inner), target).is_err(), "{inner}");
            }
        }
        for source in [
            "export default ()=> <TitleMarker parts={[]}/>",
            "export default ()=> <p><TitleMarker parts={[]}/></p>",
            "export default ()=> <HeadMarker title=\"x\"/>",
            "export default ()=> <HeadMarker><HeadMarker/></HeadMarker>",
            "export default ()=> <HeadMarker>{x}</HeadMarker",
        ] {
            for target in BOTH {
                assert!(lower(source, target).is_err(), "{source}");
            }
        }
        for target in BOTH {
            let missing_head = transform(
                "export default ()=> <TitleMarker parts={[]}/>",
                None,
                Some("TitleMarker"),
                READER,
                target,
                &|base| base.to_owned(),
            );
            assert!(missing_head.is_err());
        }
    }

    #[test]
    fn generated_aliases_do_not_shadow_or_collide_with_source_names() {
        let source = "const __crepusSvelteHead=1;const \\u005f_crepusSvelteTitle=2;export default function C(){return <HeadMarker><TitleMarker parts={[]}/></HeadMarker>}";
        for target in BOTH {
            let output = lower(source, target).unwrap();
            assert!(output.contains("head as __crepusSvelteHead_"), "{output}");
            assert!(output.contains("title as __crepusSvelteTitle_"), "{output}");
            assert!(output.contains("const __crepusSvelteHead=1;"));
            assert!(
                output.contains("__crepusSvelteHead_(()=> <>{__crepusSvelteTitle_("),
                "{output}"
            );
        }
        let output = transform(
            "export default ()=> <HeadMarker><TitleMarker parts={[]}/></HeadMarker>",
            Some("HeadMarker"),
            Some("TitleMarker"),
            READER,
            MoonshineModuleTarget::Dom,
            &|_| "same".to_owned(),
        )
        .unwrap();
        assert!(output.contains("head as same_,title as same"), "{output}");
    }

    #[test]
    fn head_without_title_imports_only_head() {
        for target in BOTH {
            let output = lower(
                "export default ()=> <HeadMarker><meta/></HeadMarker>",
                target,
            )
            .unwrap();
            assert!(
                output.contains("import {head as __crepusSvelteHead} from"),
                "{output}"
            );
            assert!(!output.contains("title as"));
        }
    }
}
