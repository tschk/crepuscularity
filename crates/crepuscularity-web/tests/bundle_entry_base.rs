use crepuscularity_core::TemplateContext;
use crepuscularity_web::{par_render_from_files, render_from_files};
use std::collections::HashMap;

fn files() -> HashMap<String, String> {
    HashMap::from([
        ("entry.crepus".into(), "include child.crepus".into()),
        (
            "named.crepus".into(),
            "--- Root\ninclude child.crepus".into(),
        ),
        ("child.crepus".into(), "span \"ROOT CHILD\"".into()),
        (
            "unrelated/child.crepus".into(),
            "span \"WRONG CHILD\"".into(),
        ),
        ("nested/entry.crepus".into(), "include child.crepus".into()),
        ("nested/child.crepus".into(), "span \"NESTED CHILD\"".into()),
    ])
}

#[test]
fn root_bundle_entries_replace_incoming_base_directory_in_all_modes() {
    let files = files();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some("unrelated".into());
    for entry in ["entry.crepus", "named.crepus#Root"] {
        #[allow(unused_mut)]
        let mut outputs = vec![render_from_files(&files, entry, &ctx).unwrap()];
        #[cfg(feature = "ssr")]
        for markers in [false, true] {
            outputs.push(
                crepuscularity_web::render_from_files_with_ssr(&files, entry, &ctx, markers)
                    .unwrap(),
            );
        }
        for html in outputs {
            assert!(
                html.contains("ROOT CHILD") && !html.contains("WRONG CHILD"),
                "{entry}: {html}"
            );
        }
    }
    assert_eq!(
        ctx.base_dir.as_deref(),
        Some(std::path::Path::new("unrelated"))
    );
}

#[test]
fn parallel_entries_each_use_their_own_namespace() {
    let files = files();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some("unrelated".into());
    let entries = ["entry.crepus", "nested/entry.crepus", "named.crepus#Root"];
    let outputs = par_render_from_files(&files, &entries, &ctx);
    assert_eq!(outputs.len(), entries.len());
    for ((entry, html), expected) in
        outputs
            .into_iter()
            .zip(["ROOT CHILD", "NESTED CHILD", "ROOT CHILD"])
    {
        let html = html.unwrap();
        assert!(
            html.contains(expected) && !html.contains("WRONG CHILD"),
            "{entry}: {html}"
        );
    }
}

#[test]
fn ordinary_template_rendering_retains_the_callers_base_directory() {
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some("unrelated".into());
    std::sync::Arc::make_mut(&mut ctx.virtual_files).insert(
        "unrelated/child.crepus".into(),
        "span \"CALLER CHILD\"".into(),
    );
    let html = crepuscularity_web::render_template_to_html("include child.crepus", &ctx).unwrap();
    assert!(html.contains("CALLER CHILD"), "{html}");
}
