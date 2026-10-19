#![cfg(feature = "ssr")]

use base64::Engine;
use crepuscularity_core::TemplateContext;
use crepuscularity_web::render_template_to_html_with_ssr;
use serde_json::Value;
use std::sync::Arc;

fn payload(html: &str) -> Value {
    let after = html.split("id=\"__crepus_hydration__\"").nth(1).unwrap();
    let encoded = after
        .split_once('>')
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0;
    serde_json::from_slice(
        &base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap(),
    )
    .unwrap()
}

fn assert_range(html: &str, kind: &str) {
    let payload = payload(html);
    let bindings = payload["bind"].as_object().unwrap();
    let entries: Vec<_> = bindings
        .iter()
        .filter(|(_, value)| value["kind"] == kind)
        .collect();
    assert!(!entries.is_empty(), "missing {kind}: {html}");
    for (id, detail) in entries {
        assert_eq!(detail["marker"], "range", "{html}");
        let start = format!("<!--crepus:{kind}:c{id}:start-->");
        let end = format!("<!--crepus:{kind}:c{id}:end-->");
        assert_eq!(html.matches(&start).count(), 1, "{html}");
        assert_eq!(html.matches(&end).count(), 1, "{html}");
        assert!(html.find(&start) < html.find(&end));
    }
    assert_eq!(payload["v"], 1);
}

#[test]
fn restricted_parents_emit_ranges_for_every_region_kind() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "cell");
    let mut row = TemplateContext::new();
    row.set("value", "row");
    ctx.set("rows", vec![row]);
    Arc::make_mut(&mut ctx.virtual_files)
        .insert("row.crepus".into(), "tr\n  td \"included\"".into());
    for (kind, source) in [
        ("text", "select\n  option\n    \"{name}\""),
        ("raw", "select\n  option\n    {name}\n    {=name}"),
        (
            "if",
            "table\n  tbody\n    if true\n      tr\n        td \"cell\"",
        ),
        (
            "for",
            "table\n  tbody\n    for row in rows\n      tr\n        td \"{row}\"",
        ),
        (
            "match",
            "select\n  match name\n    \"cell\" =>\n      option \"matched\"",
        ),
        ("include", "table\n  tbody\n    include row.crepus"),
    ] {
        let html = render_template_to_html_with_ssr(source, &ctx, true).unwrap();
        assert_range(&html, kind);
        assert_eq!(html.matches("data-crepus-root=\"1\"").count(), 1, "{html}");
    }
}

#[test]
fn all_restricted_html_parent_tags_select_ranges() {
    for parent in [
        "table", "thead", "tbody", "tfoot", "tr", "colgroup", "select", "optgroup", "option",
        "TBODY",
    ] {
        let source = format!("{parent}\n  if true\n    \"content\"");
        let html =
            render_template_to_html_with_ssr(&source, &TemplateContext::new(), true).unwrap();
        assert_range(&html, "if");
    }
    for source in [
        "table\n  if true\n    tr\n      td \"direct row\"",
        "table\n  if true\n    col",
        "select\n  optgroup\n    if true\n      option \"choice\"",
    ] {
        assert_range(
            &render_template_to_html_with_ssr(source, &TemplateContext::new(), true).unwrap(),
            "if",
        );
    }
}

#[test]
fn transparent_includes_and_forwarded_slots_inherit_insertion_context() {
    let mut ctx = TemplateContext::new();
    Arc::make_mut(&mut ctx.virtual_files).insert("slot.crepus".into(), "slot".into());
    Arc::make_mut(&mut ctx.virtual_files)
        .insert("outer.crepus".into(), "include slot.crepus\n  slot".into());
    let html = render_template_to_html_with_ssr("table\n  tbody\n    include outer.crepus\n      if true\n        tr\n          td \"provided\"", &ctx, true).unwrap();
    assert_range(&html, "include");
    assert_range(&html, "if");
    assert!(html.contains("provided"));
}

#[test]
fn actual_element_children_and_later_siblings_restore_span_markers() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "visible");
    let source = "div\n  table\n    tbody\n      if true\n        tr\n          td \"{name}\"\n  span \"{name}\"";
    let html = render_template_to_html_with_ssr(source, &ctx, true).unwrap();
    assert_range(&html, "if");
    let data = payload(&html);
    for (_, descriptor) in data["bind"]
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, d)| d["kind"] == "text")
    {
        assert!(
            descriptor.get("marker").is_none(),
            "ordinary text keeps span ABI: {html}"
        );
    }
    assert_eq!(html.matches("data-crepus-kind=\"text\"").count(), 2);
    assert_eq!(html.matches("data-crepus-root=\"1\"").count(), 1);
    let plain = crepuscularity_web::render_template_to_html(source, &ctx).unwrap();
    assert_eq!(
        render_template_to_html_with_ssr(source, &ctx, false).unwrap(),
        plain
    );
    assert!(!plain.contains("<!--crepus:"));
}

#[test]
fn inactive_regions_keep_empty_ranges_and_failed_render_does_not_leak_context() {
    let ctx = TemplateContext::new();
    let html =
        render_template_to_html_with_ssr("select\n  if false\n    option \"inactive\"", &ctx, true)
            .unwrap();
    assert_range(&html, "if");
    assert!(!html.contains("inactive"));
    assert!(render_template_to_html_with_ssr("table\n  tbody\n    {1 +}", &ctx, true).is_err());
    let mut ctx = ctx;
    ctx.set("name", "ordinary");
    let html = render_template_to_html_with_ssr("div \"{name}\"", &ctx, true).unwrap();
    assert!(html.contains("data-crepus-kind=\"text\""));
    assert!(!html.contains("<!--crepus:"));
}
