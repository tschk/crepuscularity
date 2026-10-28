use crepuscularity_core::{parse_template, TemplateContext, TemplateValue};
use crepuscularity_lvgl::render_template_to_lvgl_xml;
use std::sync::Arc;

#[test]
fn repeated_named_and_ordinary_slots_forward_caller_locals_and_reuse_content() {
    for (path, prefix) in [("shell.crepus", ""), ("shell.crepus#Card", "--- Card\n")] {
        let mut ctx = TemplateContext::new();
        let files = Arc::make_mut(&mut ctx.virtual_files);
        files.insert("shell.crepus".into(),format!("{prefix}$: default title = 'DEFAULT'\nsection\n  span \"title:{{title}}\"\n  include inner.crepus\n    slot\n    slot\n  include inner.crepus"));
        files.insert(
            "inner.crepus".into(),
            "section\n  slot\n    span \"INNER-FALLBACK\"".into(),
        );
        let rows = ["ONE", "TWO"]
            .into_iter()
            .map(|title| {
                let mut row = TemplateContext::new();
                row.set("title", title);
                row
            })
            .collect();
        ctx.set("rows", TemplateValue::List(rows));
        let source = format!("for row in rows\n  include {path} title={{row.title}}\n    $: let local = row.title\n    span \"caller:{{local}}\"\ninclude {path}");
        let xml = render_template_to_lvgl_xml(&source, &ctx).unwrap();
        for title in ["ONE", "TWO"] {
            assert_eq!(xml.matches(&format!("title:{title}")).count(), 1, "{xml}");
            assert_eq!(xml.matches(&format!("caller:{title}")).count(), 2, "{xml}");
        }
        assert_eq!(xml.matches("title:DEFAULT").count(), 1, "{xml}");
        assert_eq!(xml.matches("INNER-FALLBACK").count(), 3, "{xml}");
        assert!(!ctx.vars.contains_key("local"));
    }
}

#[test]
fn borrowed_forwarding_preserves_external_owned_slot_bundle_and_directory() {
    let mut captured = TemplateContext::new();
    captured.base_dir = Some("foreign".into());
    Arc::make_mut(&mut captured.virtual_files)
        .insert("foreign/label.crepus".into(), "span \"CAPTURED\"".into());
    let mut root = TemplateContext::new();
    root.slot = Some((
        parse_template("include label.crepus").unwrap(),
        Arc::new(captured),
    ));
    let files = Arc::make_mut(&mut root.virtual_files);
    files.insert("label.crepus".into(), "span \"WRONG-BUNDLE\"".into());
    files.insert("shell.crepus".into(), "slot".into());
    let xml = render_template_to_lvgl_xml("include shell.crepus\n  slot", &root).unwrap();
    assert_eq!(xml.matches("CAPTURED").count(), 1, "{xml}");
    assert!(!xml.contains("WRONG-BUNDLE"), "{xml}");
}

#[test]
fn unused_slot_remains_lazy_and_used_slot_includes_keep_depth_checks() {
    let mut ctx = TemplateContext::new();
    Arc::make_mut(&mut ctx.virtual_files).insert("ignore.crepus".into(), "span \"OK\"".into());
    assert!(render_template_to_lvgl_xml("include ignore.crepus\n  span \"{1 +}\"", &ctx).is_ok());
    Arc::make_mut(&mut ctx.virtual_files).insert("shell.crepus".into(), "slot".into());
    let error =
        render_template_to_lvgl_xml("include shell.crepus\n  span \"{1 +}\"", &ctx).unwrap_err();
    assert_eq!(
        error.to_string(),
        crepuscularity_core::eval::eval_expr("1 +", &ctx)
            .unwrap_err()
            .to_string()
    );
    Arc::make_mut(&mut ctx.virtual_files).insert(
        "cycle.crepus".into(),
        "include shell.crepus\n  include cycle.crepus".into(),
    );
    let error = render_template_to_lvgl_xml("include cycle.crepus", &ctx).unwrap_err();
    assert!(
        error.to_string().contains("maximum include depth"),
        "{error}"
    );
}
