use base64::Engine;
use crepuscularity_core::context::{TemplateContext, TemplateValue};
use crepuscularity_web::{
    render_bundle_with_ssr, render_from_files_with_ssr, render_ssr_document,
    render_template_to_html_with_ssr, SsrDocument,
};
use serde_json::Value;

fn decode_hydration(html: &str) -> Value {
    let needle = "id=\"__crepus_hydration__\"";
    let start = html.find(needle).expect("hydration script");
    let after = &html[start..];
    let b64_start = after.find('>').expect("script open") + 1;
    let b64_end = after[b64_start..].find("</script>").expect("script close") + b64_start;
    let b64 = after[b64_start..b64_end].trim();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .expect("base64");
    let s = String::from_utf8(bytes).expect("utf8");
    serde_json::from_str(&s).expect("json")
}

#[test]
fn void_br_has_no_closing_tag() {
    let ctx = TemplateContext::new();
    let tpl = "pre\n  span text-zinc-500\n    \"# install\"\n  br\n  \"$ cargo install\"";
    let html = render_template_to_html_with_ssr(tpl, &ctx, false).unwrap();
    assert!(!html.contains("<br></br>"), "{html}");
    assert!(html.contains("<br />") || html.contains("<br>"), "{html}");
}

#[test]
fn markers_false_matches_plain() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "Ada");
    let tpl = "div\n  \"Hi {name}\"";
    let a = render_template_to_html_with_ssr(tpl, &ctx, false).unwrap();
    let b = crepuscularity_web::render_template_to_html(tpl, &ctx).unwrap();
    assert_eq!(a, b);
}

#[test]
fn dynamic_text_emits_span_and_bind() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "Bob");
    let tpl = "div\n  \"Hello {name}\"";
    let html = render_template_to_html_with_ssr(tpl, &ctx, true).unwrap();
    assert!(html.contains("data-crepus-kind=\"text\""));
    assert!(html.contains("data-crepus-id="));
    let v = decode_hydration(&html);
    assert_eq!(v["v"], 1);
    assert!(!v["bind"].as_object().unwrap().is_empty());
}

#[test]
fn root_element_gets_data_crepus_root() {
    let mut ctx = TemplateContext::new();
    ctx.set("x", true);
    let tpl = "div show\n  \"{x}\"";
    let html = render_template_to_html_with_ssr(tpl, &ctx, true).unwrap();
    assert!(html.contains("data-crepus-root=\"1\""));
}

#[test]
fn crepus_hash_id_emits_dom_id_on_static_element() {
    let ctx = TemplateContext::new();
    let tpl = "div #tsc-heading\n  \"ok\"";
    let html = render_template_to_html_with_ssr(tpl, &ctx, true).unwrap();
    assert!(
        html.contains(r#"id="tsc-heading""#),
        "SSR must emit el.id like client render: {html}"
    );
}

#[test]
fn static_tree_has_minimal_bind() {
    let ctx = TemplateContext::new();
    let tpl = "div\n  span\n    \"ok\"";
    let html = render_template_to_html_with_ssr(tpl, &ctx, true).unwrap();
    let v = decode_hydration(&html);
    let binds = v["bind"].as_object().unwrap();
    assert!(
        binds.is_empty(),
        "expected no bind entries for static tree, got {binds:?}"
    );
}

#[test]
fn ctx_round_trips_in_manifest() {
    let mut ctx = TemplateContext::new();
    ctx.set("title", "Hi");
    ctx.set("n", 7i64);
    let tpl = "div\n  \"{title}\"";
    let html = render_template_to_html_with_ssr(tpl, &ctx, true).unwrap();
    let v = decode_hydration(&html);
    assert_eq!(v["ctx"]["title"], "Hi");
    assert_eq!(v["ctx"]["n"], 7);
}

#[test]
fn for_loop_inner_markers() {
    let mut ctx = TemplateContext::new();
    let mut a = TemplateContext::new();
    a.set("value", "a");
    let mut b = TemplateContext::new();
    b.set("value", "b");
    ctx.set(
        "items",
        crepuscularity_core::TemplateValue::List(vec![a, b]),
    );
    let tpl = "div\n for item in {items}\n  span\n    \"{item}\"";
    let html = render_template_to_html_with_ssr(tpl, &ctx, true).unwrap();
    assert!(html.contains("data-crepus-kind=\"for\""));
    assert!(html.contains("data-crepus-kind=\"text\""));
}

#[test]
fn bundle_with_ssr() {
    let bundle = serde_json::json!({
        "entry": "i.crepus",
        "files": { "i.crepus": "div\n  \"x{y}\"\n" }
    })
    .to_string();
    let mut ctx = TemplateContext::new();
    ctx.set("y", "!");
    // Bundle path uses empty ctx from API — use dynamic-free for smoke
    let html = render_bundle_with_ssr(&bundle, true).unwrap();
    assert!(html.contains("__crepus_hydration__"));
}

#[test]
fn named_virtual_component_keeps_ssr_markers() {
    let files = std::collections::HashMap::from([(
        "ui.crepus".to_string(),
        r#"--- App
div
  "Hello {name}"
"#
        .to_string(),
    )]);
    let mut ctx = TemplateContext::new();
    ctx.set("name", "Ada");
    let html = render_from_files_with_ssr(&files, "ui.crepus#App", &ctx, true).unwrap();
    assert!(html.contains("data-crepus-root=\"1\""));
    assert!(html.contains("data-crepus-kind=\"text\""));
    assert!(html.contains("__crepus_hydration__"));
}

#[test]
fn ssr_document_wraps() {
    let ctx = TemplateContext::new();
    let doc = SsrDocument {
        lang: "en",
        title: "T",
        head_extra: "<link rel=\"x\" href=\"/\">",
        body_class: Some("app"),
    };
    let html = render_ssr_document("div\n  \"ok\"", &ctx, &doc, true).unwrap();
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("<title>T</title>"));
    assert!(html.contains("class=\"app\""));
    assert!(html.contains("__crepus_hydration__"));
}

#[test]
fn raw_html_sanitization_ssr() {
    let mut ctx = TemplateContext::new();
    ctx.set("malicious", "<script>alert(1)</script><b>safe</b>");
    let tpl = "div\n  {=malicious}";
    let html = render_template_to_html_with_ssr(tpl, &ctx, false).unwrap();
    assert!(
        !html.contains("<script>"),
        "Expected script tag to be sanitized out: {}",
        html
    );
    assert!(html.contains("<b>safe</b>"), "Expected safe html: {}", html);
}

#[test]
fn test_render_template_to_html_with_ssr_no_markers() {
    let mut ctx = TemplateContext::new();
    ctx.vars
        .insert("name".to_string(), TemplateValue::Str("World".to_string()));
    let html = render_template_to_html_with_ssr("div\n  \"Hello {name}\"", &ctx, false).unwrap();
    assert_eq!(html, "<div>Hello World</div>");
}

#[test]
fn test_render_template_to_html_with_ssr_with_markers() {
    let mut ctx = TemplateContext::new();
    ctx.vars
        .insert("name".to_string(), TemplateValue::Str("World".to_string()));
    let html = render_template_to_html_with_ssr("div\n  \"Hello {name}\"", &ctx, true).unwrap();

    assert!(html.contains("Hello World"));
    assert!(html.contains("data-crepus-id="));
    assert!(html.contains("__crepus_hydration__"));
}

#[test]
fn test_render_template_to_html_with_ssr_parse_error() {
    let ctx = TemplateContext::new();
    let res = render_template_to_html_with_ssr("{{ invalid }}", &ctx, true);
    assert!(res.is_err());
}

#[test]
fn head_extra_strips_malicious_script() {
    let ctx = TemplateContext::new();
    let doc = SsrDocument {
        lang: "en",
        title: "Safe",
        head_extra: "<script>alert('xss')</script>",
        body_class: None,
    };
    let html = render_ssr_document("div\n  \"ok\"", &ctx, &doc, false).unwrap();
    assert!(
        !html.contains("<script>"),
        "malicious script in head_extra must be stripped: {html}"
    );
    assert!(
        !html.contains("alert("),
        "script payload must not appear in document: {html}"
    );
    assert!(html.contains("<title>Safe</title>"));
    assert!(html.contains("<div>ok</div>"));
}

#[test]
fn raw_html_sanitization_normal() {
    let mut ctx = TemplateContext::new();
    ctx.set("malicious", "<script>alert(1)</script><b>safe</b>");
    let tpl = "div\n  {=malicious}";
    let html = crepuscularity_web::render_template_to_html(tpl, &ctx).unwrap();
    assert!(
        !html.contains("<script>"),
        "Expected script tag to be sanitized out: {}",
        html
    );
    assert!(html.contains("<b>safe</b>"), "Expected safe html: {}", html);
}
