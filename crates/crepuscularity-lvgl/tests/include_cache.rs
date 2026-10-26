use crepuscularity_core::{parse_template, TemplateContext, TemplateValue};
use crepuscularity_lvgl::render_template_to_lvgl_xml;
use std::sync::Arc;

#[test]
fn repeated_includes_keep_per_occurrence_props_defaults_and_caller_slots() {
    for (path, definition) in [
        (
            "card.crepus",
            "$: default title = 'DEFAULT'\nsection\n  span \"title:{title}\"\n  slot",
        ),
        (
            "card.crepus#Card",
            "--- Card\n$: default title = 'DEFAULT'\nsection\n  span \"title:{title}\"\n  slot",
        ),
    ] {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).insert("card.crepus".into(), definition.into());
        let mut rows = Vec::new();
        for title in ["ONE", "TWO", "THREE"] {
            let mut row = TemplateContext::new();
            row.set("title", title);
            rows.push(row);
        }
        ctx.set("rows", TemplateValue::List(rows));
        let source=format!("for row in rows\n  include {path} title={{row.title}}\n    span \"slot:{{row.title}}\"\ninclude {path}");
        let xml = render_template_to_lvgl_xml(&source, &ctx).unwrap();
        for title in ["ONE", "TWO", "THREE"] {
            assert_eq!(xml.matches(&format!("title:{title}")).count(), 1, "{xml}");
            assert_eq!(xml.matches(&format!("slot:{title}")).count(), 1, "{xml}");
        }
        assert_eq!(xml.matches("title:DEFAULT").count(), 1, "{xml}");
        assert!(!ctx.vars.contains_key("title"));
    }
}

#[test]
fn captured_slot_bundles_with_the_same_path_remain_distinct() {
    for (path, prefix) in [("same.crepus", ""), ("same.crepus#Card", "--- Card\n")] {
        let mut root = TemplateContext::new();
        Arc::make_mut(&mut root.virtual_files).insert(
            "same.crepus".into(),
            format!("{prefix}span \"ROOT-BUNDLE\""),
        );
        let mut captured = TemplateContext::new();
        Arc::make_mut(&mut captured.virtual_files).insert(
            "same.crepus".into(),
            format!("{prefix}span \"SLOT-BUNDLE\""),
        );
        root.slot = Some((
            parse_template(&format!("include {path}")).unwrap(),
            Arc::new(captured),
        ));
        let xml =
            render_template_to_lvgl_xml(&format!("include {path}\nslot\ninclude {path}"), &root)
                .unwrap();
        assert_eq!(xml.matches("ROOT-BUNDLE").count(), 2, "{xml}");
        assert_eq!(xml.matches("SLOT-BUNDLE").count(), 1, "{xml}");
    }
}

#[test]
fn edited_virtual_and_disk_sources_are_fresh_on_the_next_public_render() {
    for (path, prefix) in [("card.crepus", ""), ("card.crepus#Card", "--- Card\n")] {
        let mut virtual_ctx = TemplateContext::new();
        for label in ["BEFORE", "AFTER"] {
            Arc::make_mut(&mut virtual_ctx.virtual_files)
                .insert("card.crepus".into(), format!("{prefix}span \"{label}\""));
            let xml = render_template_to_lvgl_xml(
                &format!("include {path}\ninclude {path}"),
                &virtual_ctx,
            )
            .unwrap();
            assert_eq!(xml.matches(label).count(), 2, "{xml}");
            if label == "AFTER" {
                assert!(!xml.contains("BEFORE"), "{xml}");
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let mut disk_ctx = TemplateContext::new();
        disk_ctx.base_dir = Some(dir.path().into());
        for label in ["BEFORE", "AFTER"] {
            std::fs::write(
                dir.path().join("card.crepus"),
                format!("{prefix}span \"{label}\""),
            )
            .unwrap();
            let xml =
                render_template_to_lvgl_xml(&format!("include {path}\ninclude {path}"), &disk_ctx)
                    .unwrap();
            assert_eq!(xml.matches(label).count(), 2, "{xml}");
            if label == "AFTER" {
                assert!(!xml.contains("BEFORE"), "{xml}");
            }
        }
    }
}

#[test]
fn cache_hits_preserve_lazy_errors_component_lookup_and_depth_checks() {
    let mut ctx = TemplateContext::new();
    Arc::make_mut(&mut ctx.virtual_files).extend([
        ("bad.crepus".into(), "if {".into()),
        ("named.crepus".into(), "--- Card\nspan \"OK\"".into()),
        ("cycle.crepus".into(), "include cycle.crepus".into()),
        (
            "finite.crepus".into(),
            "if descend\n  include finite.crepus descend=false\nelse\n  span \"STOP\"".into(),
        ),
    ]);
    let inactive="if false\n  include bad.crepus\n  include absent.crepus\n  include named.crepus#Missing\n  include cycle.crepus";
    assert!(render_template_to_lvgl_xml(inactive, &ctx).is_ok());
    let xml = render_template_to_lvgl_xml("include finite.crepus descend=true", &ctx).unwrap();
    assert!(xml.contains("STOP"), "{xml}");
    for (source, message) in [
        ("include bad.crepus", "include parse error"),
        ("include absent.crepus", "include error"),
        (
            "include named.crepus#Card\ninclude named.crepus#Missing",
            "component 'Missing' not found",
        ),
        ("include cycle.crepus", "maximum include depth (64)"),
    ] {
        let error = render_template_to_lvgl_xml(source, &ctx)
            .unwrap_err()
            .to_string();
        assert!(error.contains(message), "{source}: {error}");
    }
    Arc::make_mut(&mut ctx.virtual_files).insert("bad.crepus".into(), "span \"RECOVERED\"".into());
    assert!(render_template_to_lvgl_xml("include bad.crepus", &ctx)
        .unwrap()
        .contains("RECOVERED"));
}
