use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crepuscularity_core::TemplateContext;
use crepuscularity_native::{render_from_files, render_template_to_ir, to_json};

fn files() -> HashMap<String, String> {
    HashMap::from([
        ("main.crepus".into(), "include child.crepus".into()),
        ("ui.crepus".into(), "--- Card\ninclude child.crepus".into()),
        ("child.crepus".into(), "span \"RootChild\"".into()),
        (
            "unrelated/child.crepus".into(),
            "span \"CallerChild\"".into(),
        ),
    ])
}

#[test]
fn root_bundle_entries_and_components_ignore_caller_base() {
    let files = files();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(PathBuf::from("unrelated"));
    for entry in ["main.crepus", "ui.crepus#Card"] {
        let json = to_json(&render_from_files(&files, entry, &ctx).unwrap()).unwrap();
        assert!(json.contains("RootChild"), "{entry}: {json}");
        assert!(!json.contains("CallerChild"), "{entry}: {json}");
    }
    assert_eq!(ctx.base_dir, Some(PathBuf::from("unrelated")));
}

#[test]
fn ordinary_template_render_retains_explicit_caller_base() {
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(PathBuf::from("unrelated"));
    ctx.virtual_files = Arc::new(files());
    let json = to_json(&render_template_to_ir("include child.crepus", &ctx).unwrap()).unwrap();
    assert!(json.contains("CallerChild"), "{json}");
    assert!(!json.contains("RootChild"), "{json}");
}
