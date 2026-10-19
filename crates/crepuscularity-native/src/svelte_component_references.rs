//! Bind authored Svelte component instances through the native reference symbol.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

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
    )
}

pub(crate) fn transform_with_policy(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    marker: &str,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<(String, Option<(String, String)>), String> {
    let fail = |message: &str| format!("{}: bind:this {message}", path.display());
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(fail("has invalid component reference syntax"));
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(fail("has invalid component reference scope"));
    }
    let semantic = built.semantic;
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name) {
            name.push('_');
        }
        name
    };
    let symbol = fresh("__crepusSvelteComponentRef");
    let value = fresh("__crepusReferenceValue");
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut found = false;
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        let mut count = 0;
        for item in &element.opening_element.attributes {
            let JSXAttributeItem::Attribute(attribute) = item else {
                continue;
            };
            if !matches!(&attribute.name,JSXAttributeName::Identifier(name) if name.name==marker) {
                continue;
            }
            count += 1;
            if count > 1 {
                return Err(fail("has duplicate reference bindings"));
            }
            if !matches!(
                &element.opening_element.name,
                JSXElementName::IdentifierReference(_) | JSXElementName::MemberExpression(_)
            ) {
                return Err(fail("requires a component tag"));
            }
            let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
                return Err(fail("requires an assignable target"));
            };
            let Some(expression) = container.expression.as_expression() else {
                return Err(fail("requires an assignable target"));
            };
            let Some(root) = crate::svelte_bindings::binding_root(expression) else {
                return Err(fail(
                    "requires an identifier-rooted target; optional chains and function bindings are unsupported",
                ));
            };
            let reference = semantic
                .scoping()
                .get_reference(root.reference_id.get().unwrap());
            let Some(binding) = reference.symbol_id() else {
                return Err(fail("target must be declared"));
            };
            let scalar = matches!(expression.get_inner_expression(), Expression::Identifier(_));
            if scalar {
                let flags = semantic.scoping().symbol_flags(binding);
                if flags.is_const_variable() || flags.is_import() {
                    return Err(fail("cannot assign to a constant binding"));
                }
            }
            found = true;
            if matches!(target, MoonshineModuleTarget::Ssr) {
                edits.push((
                    attribute.span.start as usize,
                    attribute.span.end as usize,
                    String::new(),
                ));
                continue;
            }
            let span = expression.span();
            let text = &source[span.start as usize..span.end as usize];
            // Ordinary Svelte bind:this does not subscribe to the target path.
            // Empty parts select get-before-set publication without tracking
            // object/key reads; both callbacks evaluate the authored target anew.
            let parts = if scalar { "" } else { "parts:()=>[] as const," };
            edits.push((
                span.start as usize,
                span.start as usize,
                format!(r#"{{cleanup:"commit",publication:"post",{parts}get:()=> ("#),
            ));
            edits.push((
                span.end as usize,
                span.end as usize,
                format!("),set:({value}:any)=>({text}={value})}}"),
            ));
        }
    }
    if !found {
        return Ok((source.to_owned(), None));
    }
    edits.sort_by_key(|(start, end, _)| (*start, std::cmp::Reverse(*end)));
    // SSR removes outer attributes wholesale, including any nested JSX markers.
    let mut disjoint = Vec::new();
    let mut covered = 0;
    for edit in edits {
        if edit.0 < covered {
            continue;
        }
        covered = edit.1;
        disjoint.push(edit);
    }
    let mut out = source.to_owned();
    for (start, end, text) in disjoint.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    if matches!(target, MoonshineModuleTarget::Ssr) {
        return Ok((out, None));
    }
    out = format!(
        "import {{componentRef as {symbol}}} from \"@tschk/moonshine-dom/component-instance\";\n{out}"
    );
    Ok((out, Some((marker.to_owned(), symbol))))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lower(
        source: &str,
        target: MoonshineModuleTarget,
    ) -> Result<(String, Option<(String, String)>), String> {
        let result = transform(source, Path::new("reference.svelte"), target, "marker")?;
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
    fn scalar_reference_has_post_publication_commit_cleanup_and_symbol_mapping() {
        let (out, mapping) = lower(
            "function C(){let child;return <Child marker={child}/>}",
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert_eq!(
            mapping,
            Some(("marker".into(), "__crepusSvelteComponentRef".into()))
        );
        assert!(out.contains(
            "marker={{cleanup:\"commit\",publication:\"post\",get:()=> (child),set:(__crepusReferenceValue:any)=>(child=__crepusReferenceValue)}}"
        ));
        assert!(!out.contains("function __crepusSvelteMemberRef"));
    }
    #[test]
    fn member_reference_reevaluates_target_without_tracking_path() {
        let(out,_)=lower("function C(){const refs={nested:{}};let index=0;return <><Child marker={refs.nested.child}/><Child marker={refs[index++]} /></>}",MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("parts:()=>[] as const,get:()=> (refs.nested.child)"));
        assert!(out.contains("get:()=> (refs[index++]),set:(__crepusReferenceValue:any)=>(refs[index++]=__crepusReferenceValue)"));
        assert!(!out.contains("parts:()=>[refs"));
        assert!(!out.contains("function __crepusSvelteMemberRef"));
    }
    #[test]
    fn ssr_validates_without_importing_or_evaluating_targets() {
        let (out, mapping) = lower(
            "function C(){let refs;return <Child marker={refs[choose()]} value={1}/>}",
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert_eq!(mapping, None);
        assert!(!out.contains("choose()"));
        assert!(!out.contains("component-instance"));
        assert!(!out.contains("marker="));
        assert!(out.contains("value={1}"));
    }
    #[test]
    fn invalid_targets_rejected_both_targets() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for source in [
                "function C(){const child=0;return <Child marker={child}/>}",
                "import child from './child';function C(){return <Child marker={child}/>}",
                "function C(){return <Child marker={missing}/>}",
                "function C(){return <Child marker={missing.member}/>}",
                "function C(){let refs;return <Child marker={refs?.child}/>}",
                "function C(){let refs;return <Child marker={get().child}/>}",
                "function C(){let refs;return <Child marker={refs,()=>1}/>}",
                "function C(){let refs;return <div marker={refs}/>}",
            ] {
                assert!(lower(source, target).is_err(), "{source}");
            }
        }
    }
    #[test]
    fn transparent_wrappers_computed_keys_and_nested_jsx_remain_disjoint() {
        let(out,_)=lower("function C(){let refs:any={};let other;return <Child marker={refs[choose()?.key]} content={<Other marker={other}/>}/>}",MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("get:()=> (refs[choose()?.key])"));
        assert!(out.contains("get:()=> (other)"));
        lower(
            "function C(){let refs:any={};return <Child marker={(refs as any).child}/>}",
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
    }
    #[test]
    fn setter_parameter_cannot_shadow_target_or_computed_key() {
        let(out,_)=lower("function C(){let value;let refs;let __crepusReferenceValue;return <><Child marker={value}/><Child marker={refs[__crepusReferenceValue]}/></>}",MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("set:(__crepusReferenceValue_:any)=>(value=__crepusReferenceValue_)"));
        assert!(out.contains("set:(__crepusReferenceValue_:any)=>(refs[__crepusReferenceValue]=__crepusReferenceValue_)"));
    }
    #[test]
    fn hygiene_and_no_binding_identity() {
        let(out,mapping)=lower("const __crepusSvelteComponentRef=0;function C(){let child;return <Child marker={child}/>}",MoonshineModuleTarget::Dom).unwrap();
        assert_eq!(mapping.unwrap().1, "__crepusSvelteComponentRef_");
        assert!(out.contains("componentRef as __crepusSvelteComponentRef_"));
        let source = "function C(){return <Child/>}";
        assert_eq!(
            lower(source, MoonshineModuleTarget::Dom).unwrap(),
            (source.into(), None)
        );
    }
}
