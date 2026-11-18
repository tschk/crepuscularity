//! Tests for the `hydration` feature flag.
#![cfg(feature = "hydration")]

use base64::{engine::general_purpose::STANDARD, Engine as _};
use crepuscularity_core::TemplateContext;
use crepuscularity_core::TemplateValue;
use crepuscularity_web::render_template_to_html_with_hydration;

use serde_json::Value;

fn hydration_context_json(html: &str) -> String {
    let needle = "id=\"__crepus_hydration__\"";
    let start = html.find(needle).expect("hydration context script");
    let after = &html[start..];
    let b64_start = after.find('>').expect("script open") + 1;
    let b64_end = after[b64_start..].find("</script>").expect("script close") + b64_start;
    let b64 = &after[b64_start..b64_end];
    String::from_utf8(STANDARD.decode(b64).expect("base64 context")).expect("utf-8 context")
}

fn hydration_context_value(html: &str) -> Value {
    serde_json::from_str(&hydration_context_json(html)).expect("hydration json")
}

#[test]
fn hydration_injects_root_marker() {
    let tpl = r#"div p-4
  "Hello {name}""#;
    let mut ctx = TemplateContext::new();
    ctx.set("name", "World");
    let html = render_template_to_html_with_hydration(tpl, &ctx).unwrap();
    assert!(
        html.contains("data-crepus-root"),
        "expected data-crepus-root, got: {html}"
    );
    assert!(
        html.contains("id=\"__crepus_hydration__\""),
        "expected __crepus_hydration__, got: {html}"
    );
    assert!(
        html.contains("type=\"application/json\""),
        "expected non-executable hydration script, got: {html}"
    );
}

#[test]
fn hydration_injects_dynamic_id() {
    let tpl = r#"div
  p
    "Count: {count}""#;
    let mut ctx = TemplateContext::new();
    ctx.set("count", 42i64);
    let html = render_template_to_html_with_hydration(tpl, &ctx).unwrap();
    assert!(
        html.contains("data-crepus-id="),
        "expected data-crepus-id on dynamic node, got: {html}"
    );
}

#[test]
fn hydration_ctx_json_contains_vars() {
    let tpl = r#"div
  "Hello""#;
    let mut ctx = TemplateContext::new();
    ctx.set("city", "London");
    ctx.set("temp", 14i64);
    let html = render_template_to_html_with_hydration(tpl, &ctx).unwrap();
    let value = hydration_context_value(&html);
    assert!(
        value["ctx"].get("city").is_some(),
        "expected city key in ctx JSON, got: {value}"
    );
    assert!(
        value["ctx"]["city"] == "London",
        "expected London value in ctx JSON, got: {value}"
    );
}

#[test]
fn hydration_payload_uses_canonical_manifest_shape() {
    let tpl = r#"button @click="increment"
  "Count: {count}""#;
    let mut ctx = TemplateContext::new();
    ctx.set("count", 1i64);

    let html = render_template_to_html_with_hydration(tpl, &ctx).unwrap();
    let value = hydration_context_value(&html);

    assert_eq!(value["v"], 1);
    assert!(value["ctx"].is_object());
    assert!(value["bind"].is_object());
    assert_eq!(value["ctx"]["count"], 1);
    assert!(html.contains("data-onclick=\"increment\""));
}

#[test]
fn hydration_context_does_not_allow_script_breakout() {
    let tpl = r#"div
  "Hello""#;
    let mut ctx = TemplateContext::new();
    ctx.set("payload", "</script><script>alert(1)</script>\u{2028}<>&");
    let html = render_template_to_html_with_hydration(tpl, &ctx).unwrap();
    assert!(
        !html.contains("</script><script>"),
        "raw script breakout sequence leaked into hydration HTML: {html}"
    );
    assert!(
        !html.contains("window.__crepus_ctx__"),
        "hydration context must not be emitted as executable JavaScript: {html}"
    );
    let json = hydration_context_json(&html);
    assert!(json.contains("</script><script>alert(1)</script>"));
}

// ── Golden DOM / event reattachment tests ─────────────────────────────────

#[test]
fn golden_static_element_output() {
    let tpl = "div w-full bg-white p-4\n  h1 text-xl font-bold\n    \"Welcome\"\n  span text-sm text-gray-500\n    \"Subtitle text\"";
    let ctx = TemplateContext::new();
    let html = render_template_to_html_with_hydration(tpl, &ctx).unwrap();

    // Root element carries boolean data-crepus-root attribute
    assert!(html.contains("data-crepus-root"), "missing root marker");
    // Static children should NOT carry data-crepus-id (no bindings)
    assert!(html.contains("<h1"), "missing h1");
    assert!(html.contains("Welcome"), "missing text content");
    assert!(html.contains("Subtitle text"), "missing subtitle");
    // Hydration payload must be present
    assert!(
        html.contains("__crepus_hydration__"),
        "missing hydration payload"
    );
    // No dynamic binding markers on fully static template
    assert!(
        !html.contains("data-crepus-id="),
        "static template should not have binding markers"
    );
}

#[test]
fn golden_dynamic_text_with_binding() {
    let tpl = "div\n  span\n    \"Count: {count}\"";
    let mut ctx = TemplateContext::new();
    ctx.set("count", 42i64);
    let html = render_template_to_html_with_hydration(tpl, &ctx).unwrap();

    assert!(html.contains("Count: 42"), "missing rendered value");
    assert!(html.contains("data-crepus-root"), "missing root marker");
    // At least one element should have a binding marker
    assert!(
        html.contains("data-crepus-id="),
        "missing id for client reattachment"
    );
}

#[test]
fn golden_event_handler_emits_data_attribute() {
    let tpl = "button @click=\"increment\" class:hidden={hidden}\n  \"Click me\"";
    let mut ctx = TemplateContext::new();
    ctx.set("hidden", false);
    let html = render_template_to_html_with_hydration(tpl, &ctx).unwrap();

    assert!(
        html.contains("data-onclick=\"increment\""),
        "missing onclick data attribute"
    );
    assert!(html.contains("Click me"), "missing button text");
    // Conditional class binding should produce a marker
    assert!(
        html.contains("data-crepus-id="),
        "missing binding marker for conditional class"
    );
}

#[test]
fn golden_for_loop_output() {
    // The conventional "value" field binds directly to the loop variable.
    let tpl = "div\n for item in {items}\n  span item-row\n    \"{item}\"";
    let mut ctx = TemplateContext::new();
    let mut item1 = TemplateContext::new();
    item1.set("value", "A");
    let mut item2 = TemplateContext::new();
    item2.set("value", "B");
    ctx.set("items", TemplateValue::List(vec![item1, item2]));
    let html = render_template_to_html_with_hydration(tpl, &ctx).unwrap();

    assert!(html.contains("A"), "missing first item, html: {html}");
    assert!(html.contains("B"), "missing second item");
    assert!(html.contains("data-crepus-root"), "missing root marker");
}

#[test]
fn hydration_payload_serializes_bind_map() {
    let tpl = "div\n  input type=\"text\" value={name}\n  span\n    \"Hello {name}\"";
    let mut ctx = TemplateContext::new();
    ctx.set("name", "World");
    let html = render_template_to_html_with_hydration(tpl, &ctx).unwrap();
    let value = hydration_context_value(&html);

    assert_eq!(value["v"], 1, "version must be 1");
    assert!(value["ctx"].is_object(), "ctx must be an object");
    // Context must carry the variable value
    assert_eq!(value["ctx"]["name"], "World");
    // Hydration payload exists
    assert!(
        html.contains("__crepus_hydration__"),
        "missing hydration script"
    );
}

#[test]
fn hydration_output_uses_shared_dynamic_class_filtering() {
    use crepuscularity_core::TemplateValue;
    for (value, expected) in [
        (TemplateValue::Bool(false), "base active"),
        (TemplateValue::Null, "base active"),
        (TemplateValue::Str(String::new()), "base active"),
        (
            TemplateValue::Str("tui:hidden web:p-4".into()),
            "base p-4 active",
        ),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("dynamic", value);
        ctx.set("enabled", true);
        let html = render_template_to_html_with_hydration(
            r#"<div class="base" className={dynamic} class:active={enabled}></div>"#,
            &ctx,
        )
        .unwrap();
        assert!(html.contains(&format!("class=\"{expected}\"")), "{html}");
    }
}

#[test]
fn hydration_preserves_static_ids_on_dynamic_elements() {
    let mut ctx = TemplateContext::new();
    ctx.set("value", "Content");
    let html = render_template_to_html_with_hydration(
        r#"<div id="entry&amp;anchor"><span id="label">{value}</span></div>"#,
        &ctx,
    )
    .unwrap();
    assert!(html.contains("id=\"entry&amp;amp;anchor\""), "{html}");
    assert!(html.contains("id=\"label\""), "{html}");
    assert!(html.contains("data-crepus-id="), "{html}");
    assert!(html.contains("Content"), "{html}");
}

#[test]
fn hydration_bindings_use_shared_url_filtering() {
    for (url, allowed) in [
        ("javascript:alert(1)", false),
        ("https://example.com/page", true),
        ("/local", true),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("url", url);
        let html = render_template_to_html_with_hydration("<a href={url}>Link</a>", &ctx).unwrap();
        let opening = html.split('>').next().unwrap();
        assert_eq!(opening.contains("href="), allowed, "{opening}");
        if allowed {
            assert!(opening.contains(url), "{opening}");
        }
    }
}

#[test]
fn hydration_url_filtering_covers_sources_forms_and_obfuscated_schemes() {
    for (tag, property, value, allowed) in [
        ("img", "src", "JaVaScRiPt:alert(1)", false),
        ("img", "src", "java\nscript:alert(1)", false),
        ("form", "action", "data:text/html,unsafe", false),
        ("button", "formaction", "vbscript:unsafe", false),
        ("img", "src", "data:image/png;base64,iVBORw0KGgo=", true),
        ("img", "src", "/image.png", true),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("url", value);
        let source = format!("<{tag} {property}={{url}} />");
        let html = render_template_to_html_with_hydration(&source, &ctx).unwrap();
        let opening = html.split('>').next().unwrap();
        assert_eq!(
            opening.contains(&format!("{property}=")),
            allowed,
            "{opening}"
        );
    }
}

#[test]
fn hydration_rejects_unsafe_urls_with_mixed_case_attribute_names() {
    for (property, value) in [
        ("HREF", "javascript:alert(1)"),
        ("ACTION", "data:text/html,unsafe"),
        ("SrC", "vbscript:unsafe"),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("url", value);
        let source = format!("<a {property}={{url}}>Link</a>");
        let html = render_template_to_html_with_hydration(&source, &ctx).unwrap();
        assert!(
            !html
                .split('>')
                .next()
                .unwrap()
                .contains(&format!("{property}=")),
            "{html}"
        );
        #[cfg(feature = "ssr")]
        {
            let ssr =
                crepuscularity_web::render_template_to_html_with_ssr(&source, &ctx, true).unwrap();
            assert!(
                !ssr.split('>')
                    .next()
                    .unwrap()
                    .contains(&format!("{property}=")),
                "{ssr}"
            );
        }
    }
}

#[test]
fn hydration_declarations_apply_in_source_order() {
    let mut ctx = TemplateContext::new();
    ctx.set("count", 1);
    let source = r#"<span>{count}</span><let name="count" value={2} /><span>{count}</span><let-default name="count" value={3} /><span>{count}</span>"#;
    let html = render_template_to_html_with_hydration(source, &ctx).unwrap();
    assert_eq!(html.matches(">1</span>").count(), 1, "{html}");
    assert_eq!(html.matches(">2</span>").count(), 2, "{html}");
    assert!(!html.contains(">3</span>"), "{html}");
    assert_eq!(ctx.get_str("count"), "1");
}

#[test]
fn hydration_child_declarations_do_not_leak_into_parent_siblings() {
    let mut ctx = TemplateContext::new();
    ctx.set("count", 1);
    let source = r#"<div><span>{count}</span><let name="count" value={2} /><span>{count}</span></div><span>{count}</span>"#;
    let html = render_template_to_html_with_hydration(source, &ctx).unwrap();
    assert_eq!(html.matches(">1</span>").count(), 2, "{html}");
    assert_eq!(html.matches(">2</span>").count(), 1, "{html}");
    assert!(
        html.find(">1</span>").unwrap() < html.find(">2</span>").unwrap(),
        "{html}"
    );
    assert_eq!(ctx.get_str("count"), "1");
}

#[test]
fn boolean_binding_semantics_match_ssr_and_hydration_output() {
    let source = r#"<input DISABLED={disabled} checked={disabled} aria-disabled={disabled} />"#;
    for value in [TemplateValue::Bool(false), TemplateValue::Null] {
        let mut ctx = TemplateContext::new();
        ctx.set("disabled", value);
        let hydrated = render_template_to_html_with_hydration(source, &ctx).unwrap();
        let assert_attributes = |html: String| {
            let opening = html.split('>').next().unwrap();
            assert!(!opening.contains(" DISABLED="), "{opening}");
            assert!(!opening.contains(" checked="), "{opening}");
            assert!(opening.contains(" aria-disabled="), "{opening}");
        };
        assert_attributes(hydrated);
        #[cfg(feature = "ssr")]
        assert_attributes(
            crepuscularity_web::render_template_to_html_with_ssr(source, &ctx, true).unwrap(),
        );
    }
}

#[test]
fn dynamic_ids_survive_hydration_and_ssr_output() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "entry&anchor");
    let source = "<div id={name}>Content</div>";
    let assert_id = |html: String| {
        assert_eq!(
            html.matches(" id=\"entry&amp;anchor\"").count(),
            1,
            "{html}"
        );
        assert!(html.contains("data-crepus-id="), "{html}");
    };
    assert_id(render_template_to_html_with_hydration(source, &ctx).unwrap());
    #[cfg(feature = "ssr")]
    assert_id(crepuscularity_web::render_template_to_html_with_ssr(source, &ctx, true).unwrap());
}

#[test]
fn animation_repeat_metadata_matches_plain_ssr_and_hydration_output() {
    let ctx = TemplateContext::new();
    for (settings, expected) in [
        ("300ms linear repeat", "300ms linear repeat"),
        ("300ms linear", "300ms linear"),
    ] {
        let source = format!("div animate:opacity={{{settings}}}\n  \"Content\"");
        for html in [
            crepuscularity_web::render_template_to_html(&source, &ctx).unwrap(),
            #[cfg(feature = "ssr")]
            crepuscularity_web::render_template_to_html_with_ssr(&source, &ctx, false).unwrap(),
            render_template_to_html_with_hydration(&source, &ctx).unwrap(),
        ] {
            assert!(
                html.contains(&format!("data-animate-opacity=\"{expected}\"")),
                "{html}"
            );
        }
    }
}

#[test]
fn animation_metadata_escapes_quotes_and_markup_in_all_html_paths() {
    let ctx = TemplateContext::new();
    let source = r#"<div animate:opacity={"a&b" "e<ase" repeat}>Content</div>"#;
    for html in [
        crepuscularity_web::render_template_to_html(source, &ctx).unwrap(),
        #[cfg(feature = "ssr")]
        crepuscularity_web::render_template_to_html_with_ssr(source, &ctx, false).unwrap(),
        render_template_to_html_with_hydration(source, &ctx).unwrap(),
    ] {
        assert!(
            html.contains(
                "data-animate-opacity=\"&quot;a&amp;b&quot; &quot;e&lt;ase&quot; repeat\""
            ),
            "{html}"
        );
    }
}
