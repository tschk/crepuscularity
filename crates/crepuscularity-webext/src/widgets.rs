//! Generic widget-rendering utilities for browser extensions.
//!
//! Provides helpers any extension can use to build sandboxed iframe documents
//! and convert JSON props into the crepuscularity template engine's value type.

use crepuscularity_core::context::{TemplateContext, TemplateValue};
use serde_json::Value;

// ---------------------------------------------------------------------------
// JSON → TemplateValue
// ---------------------------------------------------------------------------

/// Convert a `serde_json::Value` into a [`TemplateValue`] so JSON props from
/// JavaScript can be fed directly into the crepuscularity renderer.
pub fn json_to_template(v: Value) -> TemplateValue {
    match v {
        Value::Bool(b) => TemplateValue::Bool(b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                TemplateValue::Int(i)
            } else {
                TemplateValue::Float(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => TemplateValue::Str(s),
        Value::Array(arr) => {
            let list = arr
                .into_iter()
                .map(|item| {
                    let mut ctx = TemplateContext::new();
                    if let Value::Object(obj) = item {
                        for (k, v) in obj {
                            ctx.set(k, json_to_template(v));
                        }
                    }
                    ctx
                })
                .collect();
            TemplateValue::List(list)
        }
        _ => TemplateValue::Null,
    }
}

// ---------------------------------------------------------------------------
// Frame document builder
//
// Script and style content must be inserted raw — HTML escaping would break
// the code. This function uses Rust format strings directly rather than the
// crepus renderer, which always escapes text node content.
// ---------------------------------------------------------------------------

/// Build the `srcdoc` HTML for a sandboxed widget iframe.
///
/// - `html` — body content (raw HTML)
/// - `css`  — extra CSS appended after the base styles
/// - `js`   — JavaScript module source (inserted as `<script type="module">`)
/// - `unocss` — UnoCSS engine source to inline (pass `""` to omit)
/// - `empty_msg` — fallback body HTML when `html` is empty
///
/// Script and style sources are raw HTML contents; callers must prevent closing
/// tags in authored source. `BrowserProgram::emit_module` escapes its string literals.
pub fn build_frame_doc(html: &str, css: &str, js: &str, unocss: &str, empty_msg: &str) -> String {
    let base_css = r#"html,body{margin:0;padding:0;background:#fffdf8;color:#111;font-family:"IBM Plex Sans",sans-serif;}body{padding:16px;}"#;
    let fonts_link = r#"<link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;500;600&display=swap" rel="stylesheet">"#;
    let unocss_tag = if unocss.is_empty() {
        String::new()
    } else {
        format!("\n  <script>{unocss}</script>")
    };
    let body_html = if html.is_empty() { empty_msg } else { html };
    let js_tag = if js.is_empty() {
        String::new()
    } else {
        format!("\n  <script type=\"module\">{js}</script>")
    };
    let resize_script = r#"<script>
(function () {
  function report() {
    var h = Math.max(
      document.documentElement.scrollHeight,
      document.body ? document.body.scrollHeight : 0
    );
    parent.postMessage({ type: "anywhere-resize", height: h }, "*");
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", report);
  } else {
    report();
  }
  window.addEventListener("load", report);
  if (typeof ResizeObserver !== "undefined") {
    new ResizeObserver(report).observe(document.documentElement);
  }
})();
</script>"#;
    format!(
        "<!doctype html>\n<html>\n<head>\n  <meta charset=\"utf-8\">\n  \
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n  \
         {fonts_link}\n  \
         <style>{base_css}{css}</style>{unocss_tag}\n</head>\n<body>\n  \
         {body_html}{js_tag}{resize_script}\n</body>\n</html>"
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_frame_doc_basic() {
        let doc = build_frame_doc("<p>hi</p>", "", "", "", "empty");
        assert!(doc.starts_with("<!doctype html>"));
        assert!(doc.contains("<p>hi</p>"));
    }

    #[test]
    fn build_frame_doc_empty_uses_fallback() {
        let doc = build_frame_doc("", "", "", "", "no content");
        assert!(doc.contains("no content"));
    }

    #[test]
    fn build_frame_doc_inlines_css_and_js() {
        let doc = build_frame_doc("x", ".foo{}", "console.log(1)", "", "");
        assert!(doc.contains(".foo{}"));
        assert!(doc.contains("console.log(1)"));
        assert!(doc.contains("type=\"module\""));
    }

    #[test]
    fn build_frame_doc_omits_unocss_when_empty() {
        let doc = build_frame_doc("x", "", "", "", "");
        assert!(!doc.contains("<script>{"));
    }

    #[test]
    fn build_frame_doc_includes_resize_postmessage() {
        let doc = build_frame_doc("x", "", "", "", "");
        assert!(doc.contains("anywhere-resize"));
    }

    #[test]
    fn json_to_template_bool() {
        assert!(matches!(
            json_to_template(Value::Bool(true)),
            TemplateValue::Bool(true)
        ));
        assert!(matches!(
            json_to_template(Value::Bool(false)),
            TemplateValue::Bool(false)
        ));
    }

    #[test]
    fn json_to_template_string() {
        assert!(matches!(
            json_to_template(Value::String("hello".into())),
            TemplateValue::Str(s) if s == "hello"
        ));
    }

    #[test]
    fn json_to_template_null_and_object() {
        assert!(matches!(json_to_template(Value::Null), TemplateValue::Null));
        // Plain objects (not arrays) become Null
        assert!(matches!(
            json_to_template(serde_json::json!({"x": 1})),
            TemplateValue::Null
        ));
    }

    #[test]
    fn json_to_template_array_becomes_list() {
        assert!(matches!(
            json_to_template(serde_json::json!([{"a": 1}, {"b": 2}])),
            TemplateValue::List(v) if v.len() == 2
        ));
    }
}
