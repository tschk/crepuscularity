use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_dom_component, emit_moonshine_ssr_component, render_template_to_ir_with_path,
    ViewIr, ViewNode, IR_VERSION,
};
use std::path::Path;

fn condition_sources(expression: &str) -> Vec<(&'static str, String)> {
    vec![
        ("view.crepus", format!("if {expression}\n  div")),
        (
            "view.jsx",
            format!("<if condition={{ {expression} }}><div/></if>"),
        ),
        ("view.svelte", format!("{{#if {expression}}}<div/>{{/if}}")),
        (
            "view.vue",
            format!("<template><div v-if=\"{expression}\"/></template>"),
        ),
        ("view.astro", format!("{{({expression}) && <div/>}}")),
        ("view.ng", format!("<div *ngIf=\"{expression}\"/>")),
    ]
}
fn collection_sources(expression: &str) -> Vec<(&'static str, String)> {
    vec![
        ("view.crepus", format!("for item in {expression}\n  div")),
        (
            "view.jsx",
            format!("<for let='item' in={{ {expression} }}><div/></for>"),
        ),
        (
            "view.svelte",
            format!("{{#each {expression} as item}}<div/>{{/each}}"),
        ),
        (
            "view.vue",
            format!("<template><div v-for=\"item in {expression}\"/></template>"),
        ),
        (
            "view.astro",
            format!("{{({expression}).map((item) => <div/>)}}"),
        ),
        (
            "view.ng",
            format!("<div *ngFor=\"let item of {expression}\"/>"),
        ),
    ]
}
#[test]
fn every_frontend_rejects_malformed_preserved_control_expressions() {
    for expression in ["value +", "(value", "call()", "value; evil()"] {
        for (filename, source) in condition_sources(expression)
            .into_iter()
            .chain(collection_sources(expression))
        {
            assert!(
                render_template_to_ir_with_path(
                    &source,
                    &TemplateContext::new(),
                    Some(Path::new(filename))
                )
                .is_err(),
                "accepted {filename}: {source}"
            );
        }
    }
}
#[test]
fn runtime_conditions_need_no_snapshot_context() {
    for (filename, source) in condition_sources("runtime.ready") {
        assert!(
            render_template_to_ir_with_path(
                &source,
                &TemplateContext::new(),
                Some(Path::new(filename))
            )
            .is_ok(),
            "rejected {filename}: {source}"
        );
    }
}
#[test]
fn direct_ir_dom_and_ssr_reject_invalid_controls_but_keep_numeric_paths() {
    for expression in ["value +", "(value", "call()", "value; evil()"] {
        for node in [
            ViewNode::If {
                condition: expression.into(),
                then_children: vec![],
                else_children: None,
                style: None,
            },
            ViewNode::ForEach {
                bind: expression.into(),
                item_name: "item".into(),
                item_body: vec![],
                style: None,
            },
        ] {
            let ir = ViewIr {
                version: IR_VERSION,
                root: vec![node],
            };
            assert!(
                emit_moonshine_dom_component(&ir, "Invalid").is_err(),
                "DOM accepted {expression}"
            );
            assert!(
                emit_moonshine_ssr_component(&ir, "Invalid").is_err(),
                "SSR accepted {expression}"
            );
        }
    }
    let source = "for item in rows.0.children\n  div";
    let ir = render_template_to_ir_with_path(
        source,
        &TemplateContext::new(),
        Some(Path::new("view.crepus")),
    )
    .unwrap();
    assert!(matches!(&ir.root[0], ViewNode::ForEach { bind, .. } if bind == "rows.0.children"));
    for output in [
        emit_moonshine_dom_component(&ir, "Paths").unwrap(),
        emit_moonshine_ssr_component(&ir, "Paths").unwrap(),
    ] {
        assert!(output.contains("?.[\"0\"]?.children"), "{output}");
    }
}
