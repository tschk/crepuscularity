//! Cache authored Svelte component prop computations at their owning occurrence.
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

/// The returned factory name must be recognized as a getter factory by the
/// Svelte-only component getter lowering, rather than wrapped in another getter.
#[cfg(test)]
pub(crate) fn transform(
    source: &str,
    path: &Path,
    static_helpers: &[String],
    reference_marker: Option<&str>,
) -> Result<(String, Option<String>), String> {
    transform_with_policy(
        source,
        path,
        static_helpers,
        reference_marker,
        crate::svelte_javascript::IrPolicy::Typed,
    )
}

pub(crate) fn transform_with_policy(
    source: &str,
    path: &Path,
    static_helpers: &[String],
    reference_marker: Option<&str>,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<(String, Option<String>), String> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(format!(
            "{}: invalid Svelte component props syntax",
            path.display()
        ));
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(format!(
            "{}: invalid Svelte component props scope",
            path.display()
        ));
    }
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name) {
            name.push('_');
        }
        name
    };
    let helper = fresh("__crepusSvelteComponentProp");
    let derived = fresh("__crepusSvelteComponentPropDerived");
    let mut edits = Vec::new();
    let mut memo_needed = false;
    for node in built.semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        match &element.opening_element.name {
            JSXElementName::IdentifierReference(id)
                if !static_helpers.iter().any(|name| name == id.name.as_str()) => {}
            JSXElementName::MemberExpression(_) => {}
            _ => continue,
        }
        for attribute in &element.opening_element.attributes {
            let JSXAttributeItem::Attribute(attribute) = attribute else {
                continue;
            };
            if matches!(&attribute.name, JSXAttributeName::Identifier(name) if reference_marker.is_some_and(|marker| name.name == marker))
            {
                continue;
            }
            let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
                continue;
            };
            let Some(expression) = container.expression.as_expression() else {
                continue;
            };
            let (has_call, has_state) =
                crate::svelte_expression::metadata(expression, &built.semantic);
            let simple = matches!(
                expression.get_inner_expression(),
                Expression::Identifier(_)
                    | Expression::StaticMemberExpression(_)
                    | Expression::ComputedMemberExpression(_)
            );
            let memoize = has_call || (!simple && has_state);
            // Simple reactive reads must remain live. Everything else is either
            // a derived computation or an eagerly captured, nonreactive value.
            if !memoize && has_state {
                continue;
            }
            memo_needed |= memoize;
            let span = expression.span();
            edits.push((span.start as usize, format!("{helper}(()=> (")));
            edits.push((span.end as usize, format!("),{memoize})")));
        }
    }
    if edits.is_empty() {
        return Ok((source.to_owned(), None));
    }
    edits.sort_by_key(|(offset, _)| *offset);
    let mut out = source.to_owned();
    for (offset, text) in edits.into_iter().rev() {
        out.insert_str(offset, &text);
    }
    let initialize = if memo_needed {
        format!("if(memoize){{const memo={derived}(read);return ()=>memo.value;}}")
    } else {
        String::new()
    };
    let parameter = if memo_needed { "memoize" } else { "_memoize" };
    out.push_str(&format!("\nfunction {helper}<T>(read:()=>T,{parameter}:boolean):()=>T{{{initialize}const value=read();return ()=>value;}}\n"));
    if memo_needed {
        out = format!(
            "import {{createDerived as {derived}}} from \"@tschk/moonshine-svelte/runes\";\n{out}"
        );
    }
    Ok((out, Some(helper)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lower(source: &str) -> (String, Option<String>) {
        let result = transform(source, Path::new("props.svelte"), &[], None).unwrap();
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &result.0, SourceType::tsx()).parse();
        assert!(
            parsed.errors.is_empty(),
            "{:?}\n{}",
            parsed.errors,
            result.0
        );
        result
    }
    #[test]
    fn local_factories_are_memoized_and_global_factories_captured_eagerly() {
        let (out, alias) = lower(
            "import Child from './Child'; export default function C(){function make(){return ()=>1};return <Child onclick={make()} other={globalFactory()}/>}",
        );
        let alias = alias.unwrap();
        assert!(
            out.contains(&format!("onclick={{{alias}(()=> (make()),true)")),
            "{out}"
        );
        assert!(out.contains(&format!("other={{{alias}(()=> (globalFactory()),false)")));
        assert!(out
            .contains("const memo=__crepusSvelteComponentPropDerived(read);return ()=>memo.value"));
        assert!(out.contains("const value=read();return ()=>value"));
    }
    #[test]
    fn complex_state_computations_memoize_but_simple_reads_remain_live() {
        let (out, _) = lower(
            "function C(){let n=$state(1);const object={callback:()=>1};return <Child active={n===1} value={n} callback={object.callback} data={{n}}/>}",
        );
        assert!(out.contains("active={__crepusSvelteComponentProp(()=> (n===1),true)}"));
        assert!(out.contains("value={n} callback={object.callback}"));
        assert!(out.contains("data={__crepusSvelteComponentProp(()=> ({n}),true)}"));
    }
    #[test]
    fn inline_functions_and_known_primitive_expressions_capture_without_memo() {
        let (out, _) =
            lower("function C(){const n=1;return <Child callback={()=>n} label={n+1}/>}");
        assert!(out.contains("callback={__crepusSvelteComponentProp(()=> (()=>n),false)}"));
        assert!(out.contains("label={__crepusSvelteComponentProp(()=> (n+1),false)}"));
        assert!(!out.contains("import {createDerived"));
    }
    #[test]
    fn traversal_order_and_shadowing_match_call_analysis() {
        let (out, _) = lower(
            "function C(){let n=$state(0);return <Child a={globalFactory()||n} b={n||globalFactory()} c={globalFactory(n)}/>}",
        );
        assert!(out.contains("a={__crepusSvelteComponentProp(()=> (globalFactory()||n),true)}"));
        assert!(out.contains("b={__crepusSvelteComponentProp(()=> (n||globalFactory()),true)}"));
        assert!(out.contains("c={__crepusSvelteComponentProp(()=> (globalFactory(n)),true)}"));
        let (out, _) = lower("function C(globalFactory:any){return <Child x={globalFactory()}/>}");
        assert!(out.contains("globalFactory()),true)"));
    }
    #[test]
    fn helpers_and_intrinsics_are_excluded_and_nested_edits_are_disjoint() {
        let source = "function C(){function make(){};return <><Internal get={make()}/><button onClick={make()}/><Child content={<Other value={make()}/>}/></>}";
        let (out, _) = transform(
            source,
            Path::new("props.svelte"),
            &["Internal".into()],
            None,
        )
        .unwrap();
        assert!(out.contains("<Internal get={make()}/>"));
        assert!(out.contains("<button onClick={make()}/>"));
        assert!(out.contains("value={__crepusSvelteComponentProp(()=> (make()),true)}"));
        let allocator = Allocator::default();
        assert!(
            Parser::new(&allocator, &out, SourceType::tsx())
                .parse()
                .errors
                .is_empty(),
            "{out}"
        );
    }
    #[test]
    fn bound_function_members_remain_live_and_global_members_capture() {
        let (out, _) = lower(
            "function C(){function fn(){};return <Child a={fn.custom} b={globalThis.callback}/>}",
        );
        assert!(out.contains("a={fn.custom}"));
        assert!(out.contains("b={__crepusSvelteComponentProp(()=> (globalThis.callback),false)}"));
    }
    #[test]
    fn hygienic_alias_and_empty_module() {
        let (out, alias) =
            lower("const __crepusSvelteComponentProp=0;function C(){return <Child x={1}/>}");
        assert_eq!(alias.as_deref(), Some("__crepusSvelteComponentProp_"));
        assert!(out.contains("x={__crepusSvelteComponentProp_"));
        assert_eq!(lower("function C(){return <div/>}").1, None);
    }
}
