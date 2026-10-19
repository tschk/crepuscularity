//! Select authored Svelte components by identity while retaining live props.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::JSXElementName, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::{collections::HashSet, path::Path};

#[cfg(test)]
pub(crate) fn transform(
    source: &str,
    path: &Path,
    static_helpers: &[String],
) -> Result<String, String> {
    transform_with_policy(
        source,
        path,
        static_helpers,
        crate::svelte_javascript::IrPolicy::Typed,
        MoonshineModuleTarget::Ssr,
    )
}

pub(crate) fn transform_with_policy(
    source: &str,
    path: &Path,
    static_helpers: &[String],
    policy: crate::svelte_javascript::IrPolicy,
    target: MoonshineModuleTarget,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    policy.configure(&mut parsed.program);
    if !parsed.errors.is_empty() {
        return Err(format!(
            "{}: invalid Svelte component JSX: {:?}",
            path.display(),
            parsed.errors
        ));
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(format!(
            "{}: invalid Svelte component scope: {:?}",
            path.display(),
            built.errors
        ));
    }
    let semantic = built.semantic;
    let nodes = semantic.nodes();
    let mut used = HashSet::new();
    let mut fresh = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name) || used.contains(&name) {
            name.push('_');
        }
        used.insert(name.clone());
        name
    };
    let dynamic = fresh("__crepusSvelteDynamic");
    let map = fresh("__crepusSvelteMapComponent");
    let mut edits = Vec::new();
    let mut found = false;
    for node in nodes.iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        let name = &element.opening_element.name;
        let dynamic_name = match name {
            JSXElementName::MemberExpression(_) => true,
            JSXElementName::IdentifierReference(id)
                if static_helpers
                    .iter()
                    .any(|helper| helper == id.name.as_str()) =>
            {
                false
            }
            JSXElementName::IdentifierReference(id) => {
                let reference = semantic
                    .scoping()
                    .get_reference(id.reference_id.get().unwrap());
                !reference
                    .symbol_id()
                    .is_some_and(|symbol| semantic.scoping().symbol_flags(symbol).is_import())
            }
            _ => false,
        };
        if !dynamic_name {
            continue;
        }
        found = true;
        let selected = fresh("__CrepusSvelteSelected");
        let name_span = name.span();
        let original = &source[name_span.start as usize..name_span.end as usize];
        let prefix=format!("<{dynamic} get={{()=>{original}}} render={{({selected}: (typeof {original}) & {{}})=>(");
        let suffix = ")}/>".to_owned();
        // Insertions leave authored children intact so nested component edits stay disjoint.
        edits.push((
            element.span.start as usize,
            element.span.start as usize,
            prefix,
        ));
        edits.push((element.span.end as usize, element.span.end as usize, suffix));
        edits.push((
            name_span.start as usize,
            name_span.end as usize,
            selected.clone(),
        ));
        if let Some(closing) = &element.closing_element {
            let span = closing.name.span();
            edits.push((span.start as usize, span.end as usize, selected));
        }
    }
    if !found {
        return Ok(source.to_owned());
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    out = format!("import {{mapArray as {map}}} from \"@tschk/moonshine-dom/control\";\n{out}");
    // Ordered DOM consumes selection inside the enclosing insertion producer,
    // before retiring its previous component. SSR retains its existing memo.
    let schedule = match target {
        MoonshineModuleTarget::Dom => ",undefined,read=>read",
        MoonshineModuleTarget::Ssr => "",
    };
    out.push_str(&format!("\nfunction {dynamic}<T>(props:{{get:()=>T|null|undefined;render:(component:T)=>unknown}}):any{{return {map}(()=>{{const component=props.get();return component?[component]:[];}},component=>props.render(component){schedule});}}\n"));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lower(source: &str) -> String {
        let out = transform(source, Path::new("component.svelte"), &[]).unwrap();
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &out, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{out}", parsed.errors);
        out
    }
    #[test]
    fn generated_binding_helper_stays_static() {
        let source = "function Checked(){};const view=<Checked/>;";
        assert_eq!(
            transform(source, Path::new("binding.svelte"), &["Checked".into()]).unwrap(),
            source
        );
    }
    #[test]
    fn imported_components_stay_static_and_members_select_by_identity() {
        let out=lower("import Static from './Static.svelte';import * as UI from './ui';export default function C(){return <><Static/><UI.Button label='ok'/></>}");
        assert!(out.contains("<Static/>"));
        assert!(out.contains("<__crepusSvelteDynamic get={()=>UI.Button} render={(__CrepusSvelteSelected: (typeof UI.Button) & {})=>(<__CrepusSvelteSelected label='ok'/>)}/>"));
        assert!(out.contains("component?[component]:[]"));
    }
    #[test]
    fn nested_dynamic_children_keep_props_and_matching_tags() {
        let out=lower("export default function C(){const Current={value:null};const Child=()=>null;return <Current.value label={'雪'}><Child>hello {/* keep */} world</Child></Current.value>}");
        assert!(out.contains("<__crepusSvelteDynamic get={()=>Current.value}"));
        assert!(out.contains("label={'雪'}"));
        assert!(out.contains("{/* keep */}"));
        assert!(out.contains("</__CrepusSvelteSelected>"));
        assert!(out.contains("<__crepusSvelteDynamic get={()=>Child} render={(__CrepusSvelteSelected_: (typeof Child) & {})=>"));
    }
    #[test]
    fn expression_children_and_shadowed_imports_are_distinguished() {
        let out=lower("import Child from './Child';export default function C(){function local(Child){return <Child/>};return <div>{true?<Child/>:local(()=>null)}</div>}");
        assert!(out.contains("return <__crepusSvelteDynamic get={()=>Child}"));
        assert!(out.contains("true?<Child/>"));
    }
    #[test]
    fn attribute_values_and_existing_helper_names_stay_valid() {
        let out=lower("import Wrapper from './Wrapper';const __CrepusSvelteSelected=1;const __crepusSvelteDynamic=2;export default function C(){let Current;const quoted=\"a\\\"b\";return <Wrapper child=<Current/>/>}");
        assert!(out.contains("child=<__crepusSvelteDynamic_ get={()=>Current} render={(__CrepusSvelteSelected_: (typeof Current) & {})=>"));
        assert!(out.contains("const quoted=\"a\\\"b\""));
    }
    #[test]
    fn static_markup_is_unchanged_and_invalid_source_fails() {
        let source = "import Child from './Child';export default ()=> <div><Child/></div>";
        assert_eq!(lower(source), source);
        assert!(transform("const x=<A></B>", Path::new("bad.svelte"), &[]).is_err());
    }
    #[test]
    fn dom_selection_reconciles_inside_reader_and_ssr_keeps_existing_memo() {
        let source = "function C(){let Current;return <Current/>}";
        let dom = transform_with_policy(
            source,
            Path::new("component.svelte"),
            &[],
            crate::svelte_javascript::IrPolicy::Typed,
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert!(
            dom.contains("component=>props.render(component),undefined,read=>read)"),
            "{dom}"
        );
        assert!(!dom.contains("catch("), "{dom}");
        let ssr = transform_with_policy(
            source,
            Path::new("component.svelte"),
            &[],
            crate::svelte_javascript::IrPolicy::Typed,
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert_eq!(
            ssr,
            transform(source, Path::new("component.svelte"), &[]).unwrap()
        );
        assert!(
            ssr.ends_with("component=>props.render(component));}\n"),
            "{ssr}"
        );
    }
}
