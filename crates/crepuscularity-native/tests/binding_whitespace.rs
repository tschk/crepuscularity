use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    generate_native_source, render_template_to_ir, NativeCodegenTarget, ViewIr,
};
use serde_json::Value;
use std::path::Path;

fn controls(bind: &str) -> String {
    format!(
        r#"input bind="{bind}" @change=sync
toggle bind="{bind}" @change=sync
checkbox bind="{bind}" @change=sync
slider bind="{bind}" @change=sync
picker bind="{bind}" @change=sync
  option value="one" label="One"
tabs bind="{bind}" @change=sync
  tab value="one" label="One"
    text "Page""#
    )
}

fn rewrite_bindings(value: &mut Value, rewrite: &impl Fn(&str) -> String) {
    match value {
        Value::Object(fields) => {
            if let Some(Value::String(bind)) = fields.get_mut("bind") {
                *bind = rewrite(bind);
            }
            for child in fields.values_mut() {
                rewrite_bindings(child, rewrite);
            }
        }
        Value::Array(children) => {
            for child in children {
                rewrite_bindings(child, rewrite);
            }
        }
        _ => {}
    }
}

fn changed_bindings(ir: &ViewIr, rewrite: impl Fn(&str) -> String) -> ViewIr {
    let mut value = serde_json::to_value(ir).unwrap();
    rewrite_bindings(&mut value, &rewrite);
    serde_json::from_value(value).unwrap()
}

fn assert_same_native_output(expected: &ViewIr, actual: &ViewIr) {
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        assert_eq!(
            generate_native_source(actual, target, "Bindings"),
            generate_native_source(expected, target, "Bindings"),
            "{target:?}"
        );
    }
}

#[test]
fn authored_quoted_bindings_trim_edges_for_every_native_control() {
    let ctx = TemplateContext::new();
    let canonical = render_template_to_ir(&controls("title"), &ctx).unwrap();
    let padded = render_template_to_ir(&controls(" \ttitle\u{2003} "), &ctx).unwrap();
    assert_eq!(padded, canonical);
    assert_same_native_output(&canonical, &padded);
    let empty = render_template_to_ir(&controls(""), &ctx).unwrap();
    let whitespace = render_template_to_ir(&controls(" \t\u{2003} "), &ctx).unwrap();
    assert_eq!(whitespace, empty);
    assert_same_native_output(&empty, &whitespace);
}

#[test]
fn custom_bind_edges_normalize_across_all_source_frontends() {
    let ctx = TemplateContext::new();
    for (file, canonical, padded) in [
        (
            "app.crepus",
            "input bind=\"title\"",
            "input bind=\" title \"",
        ),
        (
            "app.jsx",
            "<input bind=\"title\" />",
            "<input bind=\" title \" />",
        ),
        (
            "app.svelte",
            "<input bind=\"title\" />",
            "<input bind=\" title \" />",
        ),
        (
            "app.vue",
            "<template><input bind=\"title\" /></template>",
            "<template><input bind=\" title \" /></template>",
        ),
        (
            "app.astro",
            "<input bind=\"title\" />",
            "<input bind=\" title \" />",
        ),
        (
            "app.ng.html",
            "<input bind=\"title\" />",
            "<input bind=\" title \" />",
        ),
    ] {
        let canonical = crepuscularity_native::render_template_to_ir_with_path(
            canonical,
            &ctx,
            Some(Path::new(file)),
        )
        .unwrap();
        let padded = crepuscularity_native::render_template_to_ir_with_path(
            padded,
            &ctx,
            Some(Path::new(file)),
        )
        .unwrap();
        assert_eq!(padded, canonical, "{file}");
        assert_same_native_output(&canonical, &padded);
    }
}

#[test]
fn manual_ir_control_paths_use_the_same_canonical_read_draft_and_write_keys() {
    let ir = render_template_to_ir(&controls("title"), &TemplateContext::new()).unwrap();
    assert_same_native_output(
        &ir,
        &changed_bindings(&ir, |bind| format!(" \t{bind}\u{2003} ")),
    );
    let empty = changed_bindings(&ir, |_| String::new());
    let whitespace = changed_bindings(&ir, |_| " \t\u{2003} ".into());
    assert_same_native_output(&empty, &whitespace);
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        let output = generate_native_source(&whitespace, target, "Bindings");
        assert!(!output.contains("CrepusActions.performChange("), "{output}");
        assert!(!output.contains(".draftText("), "{output}");
        assert!(!output.contains(".draftBool("), "{output}");
        assert!(!output.contains(".draftNumber("), "{output}");
    }
}

#[test]
fn manual_ir_nested_collection_and_outer_alias_paths_trim_consistently() {
    let source = "for group in groups\n  for item in group.items\n    input bind=group.title @change=sync\n    input bind=item.title @change=sync\n    for item in item.children\n      input bind=group.title @change=sync\n      input bind=item.title @change=sync";
    let ir = render_template_to_ir(source, &TemplateContext::new()).unwrap();
    let padded = changed_bindings(&ir, |bind| format!(" \t{bind}\u{2003} "));
    assert_same_native_output(&ir, &padded);
}

#[test]
fn normalization_preserves_inner_spaces_and_nul_in_bindings() {
    for bind in ["user .title", "title\0tail"] {
        let source = format!("input bind=\"{bind}\" @change=sync");
        let ir = render_template_to_ir(&source, &TemplateContext::new()).unwrap();
        assert_eq!(serde_json::to_value(&ir).unwrap()["root"][0]["bind"], bind);
        let padded = changed_bindings(&ir, |bind| format!(" {bind} "));
        assert_same_native_output(&ir, &padded);
    }
}
