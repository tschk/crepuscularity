//! Connect authored raw HTML to the native explicit unsanitized HTML boundary.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType, Span};
use std::{collections::HashSet, path::Path};

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
fn intrinsic(name: &JSXElementName<'_>) -> bool {
    matches!(name,JSXElementName::Identifier(name) if crate::moonshine_module::supported_html_tag(name.name.as_str()))
}

/// Check authored placement before control/snippet lowering relocates their JSX.
/// Unresolved component or ordinary function invocation namespaces remain unknown.
pub(crate) fn validate_known_namespaces(
    source: &str,
    path: &Path,
    marker: &str,
    controls: &[&str],
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<(), String> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(format!(
            "{}: Svelte raw HTML has invalid syntax",
            path.display()
        ));
    }
    policy.configure(&mut parsed.program);
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let nodes = semantic.nodes();
    for node in nodes.iter() {
        if !matches!(node.kind(), AstKind::JSXElement(element) if matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if name.name == marker))
        {
            continue;
        }
        for ancestor in nodes.ancestors(node.id()) {
            match ancestor.kind() {
                AstKind::JSXElement(element) => match &element.opening_element.name {
                    JSXElementName::Identifier(name) if name.name == "foreignObject" => break,
                    JSXElementName::Identifier(name)
                        if matches!(name.name.as_str(), "svg" | "g") =>
                    {
                        return Err(format!(
                            "{}: Svelte raw HTML in a known SVG namespace is unsupported",
                            path.display()
                        ));
                    }
                    JSXElementName::IdentifierReference(name)
                        if controls.contains(&name.name.as_str()) => {}
                    JSXElementName::IdentifierReference(_)
                    | JSXElementName::MemberExpression(_) => break,
                    _ => {}
                },
                AstKind::ArrowFunctionExpression(_) => {
                    // Only callback bodies attached directly to compiler-owned
                    // fragment controls inherit their authored placement.
                    let control = nodes.ancestors(ancestor.id()).find_map(|parent| match parent.kind() {
                        AstKind::JSXElement(element) => Some(matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if controls.contains(&name.name.as_str()))),
                        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => Some(false),
                        _ => None,
                    }).unwrap_or(false);
                    if !control {
                        break;
                    }
                }
                AstKind::Function(_) => break,
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn transform(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    marker: &str,
) -> Result<(String, Option<(String, String)>), String> {
    transform_with_policy(
        source,
        path,
        target,
        marker,
        crate::svelte_javascript::IrPolicy::Typed,
        &Default::default(),
    )
}

pub(crate) fn transform_with_policy(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    marker: &str,
    policy: crate::svelte_javascript::IrPolicy,
    globals: &crate::source_globals::SourceGlobals,
) -> Result<(String, Option<(String, String)>), String> {
    let types = &globals.types;
    let fail = |message: &str| format!("{}: Svelte raw HTML {message}", path.display());
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
        return Err(fail("has invalid scope"));
    }
    let semantic = built.semantic;
    let nodes = semantic.nodes();
    let reserved = if matches!(target, MoonshineModuleTarget::Ssr) {
        nodes
            .iter()
            .filter_map(|node| match node.kind() {
                AstKind::BindingIdentifier(id) => Some(id.name.to_string()),
                AstKind::IdentifierReference(id) => Some(id.name.to_string()),
                _ => None,
            })
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name) || reserved.contains(&name) {
            name.push('_');
        }
        name
    };
    let html = fresh("__crepusSvelteRawHTML");
    let content = fresh("__crepusSvelteHTMLContent");
    let defer = fresh("__crepusSvelteHTMLDefer");
    let peek = fresh("__crepusSvelteHTMLConsumer");
    let enter = fresh("__crepusSvelteHTMLWithConsumer");
    let element_name = fresh("__crepusSvelteHTMLElement");
    let mut edits: Vec<Edit> = Vec::new();
    let mut sole = false;
    let mut fallback = false;
    let markers:Vec<_>=nodes.iter().filter(|node|matches!(node.kind(),AstKind::JSXElement(element) if matches!(&element.opening_element.name,JSXElementName::IdentifierReference(name) if name.name==marker))).collect();
    for node in markers.into_iter().rev() {
        let AstKind::JSXElement(element) = node.kind() else {
            unreachable!()
        };
        if element.opening_element.attributes.len() != 1 || !element.children.is_empty() {
            return Err(fail("marker requires exactly one value and no children"));
        }
        let JSXAttributeItem::Attribute(attribute) = &element.opening_element.attributes[0] else {
            return Err(fail("marker spreads are unsupported"));
        };
        if !matches!(&attribute.name,JSXAttributeName::Identifier(name) if name.name=="value") {
            return Err(fail("marker requires a value"));
        }
        let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
            return Err(fail("marker requires an expression"));
        };
        let Some(expression) = container.expression.as_expression() else {
            return Err(fail("marker requires an expression"));
        };
        let parent = match nodes.parent_kind(node.id()) {
            AstKind::JSXElement(parent)
                if intrinsic(&parent.opening_element.name) && parent.children.len() == 1 =>
            {
                Some(parent)
            }
            _ => None,
        };
        if let (MoonshineModuleTarget::Dom, Some(parent)) = (target, parent) {
            sole = true;
            let expression = render(source, expression.span(), &edits);
            // Relocation consumes nested edits; their transformed source is now
            // carried by the parent's private reference attribute.
            edits.retain(|(start, end, _)| {
                !(*start >= element.span.start as usize && *end <= element.span.end as usize)
            });
            edits.push((
                element.span.start as usize,
                element.span.end as usize,
                String::new(),
            ));
            let offset = parent.opening_element.span.end as usize - 1;
            edits.push((offset,offset,format!(" {marker}={{({element_name}:{types}.Element)=>{content}({element_name},()=> ({expression}))}}")));
        } else {
            fallback = true;
            let name = attribute.name.span();
            let expression = expression.span();
            edits.push((name.start as usize, name.end as usize, "read".into()));
            edits.push((
                expression.start as usize,
                expression.start as usize,
                "()=> (".into(),
            ));
            edits.push((expression.end as usize, expression.end as usize, ")".into()));
        }
    }
    if edits.is_empty() {
        return Ok((source.to_owned(), None));
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    match target {
        MoonshineModuleTarget::Dom => {
            let mut imports = Vec::new();
            if sole {
                imports.push(format!("htmlContent as {content}"));
            }
            if fallback {
                imports.push(format!("rawHTML as {html}"));
            }
            out = format!(
                "import {{{}}} from \"@tschk/moonshine-dom/html\";\n{out}",
                imports.join(",")
            );
            if fallback {
                out.push_str(&format!("\nfunction {marker}(props:{{read:()=>unknown}}):any{{return {html}(props.read);}}\n"));
            }
        }
        MoonshineModuleTarget::Ssr => {
            out=format!("import {{rawHTML as {html},defer as {defer}}} from \"@tschk/moonshine-dom/ssr\";\nimport {{peekComponentContext as {peek},withComponentConsumer as {enter}}} from \"@tschk/moonshine-dom/component\";\n{out}");
            out.push_str(&format!("\nfunction {marker}(props:{{read:()=>unknown}}):any{{const consumer={peek}();const read=()=>{html}(props.read());return {defer}(()=>consumer?{enter}(consumer,read):read());}}\n"));
        }
    }
    Ok((out, sole.then(|| (marker.to_owned(), "\"ref\"".into()))))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lower(
        source: &str,
        target: MoonshineModuleTarget,
    ) -> Result<(String, Option<(String, String)>), String> {
        let result = transform(source, Path::new("html.svelte"), target, "Raw")?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &result.0, SourceType::tsx()).parse();
        assert!(
            parsed.errors.is_empty(),
            "{:?}\n{}",
            parsed.errors,
            result.0
        );
        Ok(result)
    }
    #[test]
    fn sole_intrinsic_child_uses_html_content_reference() {
        let (out, mapping) = lower(
            "function C(){return <section id=\"x\"><Raw value={html}/></section>}",
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert_eq!(mapping, Some(("Raw".into(), "\"ref\"".into())));
        assert!(out.contains("<section id=\"x\" Raw={(__crepusSvelteHTMLElement:globalThis.Element)=>__crepusSvelteHTMLContent(__crepusSvelteHTMLElement,()=> (html))}></section>"));
        assert!(out.contains("htmlContent as __crepusSvelteHTMLContent"));
        assert!(!out.contains("rawHTML as"));
        assert!(!out.contains("function Raw"));
    }
    #[test]
    fn siblings_roots_fragments_and_components_use_owned_raw_html_helper() {
        for source in [
            "function C(){return <Raw value={html}/>}",
            "function C(){return <div>before<Raw value={html}/>after</div>}",
            "function C(){return <><Raw value={html}/></>}",
            "function C(){return <Widget><Raw value={html}/></Widget>}",
        ] {
            let (out, mapping) = lower(source, MoonshineModuleTarget::Dom).unwrap();
            assert!(out.contains("<Raw read={()=> (html)}/>"));
            assert!(out.contains("return __crepusSvelteRawHTML(props.read)"));
            assert_eq!(mapping, None);
            assert!(!out.contains("htmlContent as"));
        }
    }
    #[test]
    fn ssr_always_defers_explicit_unsanitized_markup() {
        let (out, mapping) = lower(
            "function C(){return <p><Raw value={html}/></p>}",
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert_eq!(mapping, None);
        assert!(out.contains("<Raw read={()=> (html)}/>"));
        assert!(out.contains("from \"@tschk/moonshine-dom/ssr\""));
        assert!(
            out.contains("const read=()=>__crepusSvelteRawHTML(props.read());return __crepusSvelteHTMLDefer(()=>consumer?__crepusSvelteHTMLWithConsumer(consumer,read):read())")
        );
        assert!(!out.contains("htmlContent"));
    }
    #[test]
    fn ssr_raw_consumer_aliases_reserve_decoded_authored_names() {
        let source = r"const \u005f\u005fcrepusSvelteHTMLConsumer=1; function C(){return <p><Raw value={\u005f\u005fcrepusSvelteHTMLWithConsumer}/></p>}";
        let (out, _) = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(
            out.contains("peekComponentContext as __crepusSvelteHTMLConsumer_"),
            "{out}"
        );
        assert!(
            out.contains("withComponentConsumer as __crepusSvelteHTMLWithConsumer_"),
            "{out}"
        );
        let (dom, _) = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert!(!dom.contains("peekComponentContext"), "{dom}");
    }

    #[test]
    fn nested_marker_expressions_keep_disjoint_edits_after_relocation() {
        let (out, mapping) = lower(
            "function C(){return <main><Raw value={<div><Raw value={html}/></div>}/></main>}",
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert!(mapping.is_some());
        assert_eq!(
            out.matches("Raw={(__crepusSvelteHTMLElement:globalThis.Element)")
                .count(),
            2
        );
        assert!(!out.contains("<Raw value="));
        let (out, _) = lower(
            "function C(){return <Raw value={<><Raw value={html}/></>}/>}",
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert_eq!(out.matches("read={()=> (").count(), 2);
    }
    #[test]
    fn imports_and_callback_parameter_are_hygienic() {
        let(out,_)=lower("const __crepusSvelteHTMLElement='authored';const __crepusSvelteHTMLContent=0;function C(){return <p><Raw value={__crepusSvelteHTMLElement}/></p>}",MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("htmlContent as __crepusSvelteHTMLContent_"));
        assert!(out.contains("(__crepusSvelteHTMLElement_:globalThis.Element)=>__crepusSvelteHTMLContent_(__crepusSvelteHTMLElement_,()=> (__crepusSvelteHTMLElement))"));
    }
    #[test]
    fn invalid_markers_and_original_syntax_fail() {
        for source in [
            "function C(){return <Raw/>}",
            "function C(){return <Raw value={html} other={1}/>}",
            "function C(){return <Raw value=\"x\"/>}",
            "function C(){return <Raw value={html}>text</Raw>}",
            "function C(){return <Raw {...props}/>}",
            "function C(){return <Raw value={}/>}",
        ] {
            assert!(
                lower(source, MoonshineModuleTarget::Dom).is_err(),
                "{source}"
            );
        }
        assert!(lower("const a=;", MoonshineModuleTarget::Dom).is_err());
    }
    #[test]
    fn whitespace_and_unsupported_namespaces_do_not_use_content_optimization() {
        let (out, mapping) = lower(
            "function C(){return <div> <Raw value={html}/> </div>}",
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert_eq!(mapping, None);
        assert!(out.contains("<div> <Raw read="));
        let (out, mapping) = lower(
            "function C(){return <svg><Raw value={html}/></svg>}",
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert_eq!(mapping, None);
        assert!(out.contains("<svg><Raw read="));
    }
    #[test]
    fn namespace_guard_does_not_guess_user_function_invocation_context() {
        let source = "function render(){return <Raw value={value}/>;}function C(){return <svg>{render()}</svg>}";
        validate_known_namespaces(
            source,
            Path::new("html.svelte"),
            "Raw",
            &[],
            crate::svelte_javascript::IrPolicy::Typed,
        )
        .unwrap();
        let controlled =
            "function C(){return <svg><Each children={()=> <Raw value={value}/>}/></svg>}";
        assert!(validate_known_namespaces(
            controlled,
            Path::new("html.svelte"),
            "Raw",
            &["Each"],
            crate::svelte_javascript::IrPolicy::Typed
        )
        .unwrap_err()
        .contains("SVG namespace"));
    }
}
