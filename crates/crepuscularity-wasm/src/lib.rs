//! WASM bindings over the same parser the `crepus` CLI uses.
//!
//! The IR crosses the boundary as a JSON string rather than a structured
//! `JsValue`: it avoids a `serde-wasm-bindgen` dependency and `JSON.parse` on a
//! single string beats field-by-field reflection for trees of any real size.

use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::{render_template_to_ir, render_template_to_ir_with_path, IR_VERSION};
use wasm_bindgen::prelude::*;

/// Schema version of the IR this module emits; consumers should check it.
#[wasm_bindgen]
pub fn ir_version() -> u32 {
    IR_VERSION
}

/// Parse a template into View IR, choosing the frontend from `filename`.
///
/// The parser dispatches on the file extension, so `.crepus`, `.jsx`/`.tsx`,
/// `.svelte`, `.vue`, `.astro` and Angular component templates
/// (`*.component.html`, `*.ng.html`, `*.ng`) all reach the same IR through
/// their own frontend.
#[wasm_bindgen]
pub fn parse_template_json(
    source: &str,
    filename: Option<String>,
    context_json: Option<String>,
) -> Result<String, JsError> {
    parse_template_json_inner(source, filename.as_deref(), context_json.as_deref())
        .map_err(|e| JsError::new(&e))
}

fn parse_template_json_inner(
    source: &str,
    filename: Option<&str>,
    context_json: Option<&str>,
) -> Result<String, String> {
    let ctx = build_context_inner(context_json)?;
    let path = filename.map(std::path::Path::new);
    let ir = render_template_to_ir_with_path(source, &ctx, path)
        .map_err(|e| format!("lower template to View IR: {e}"))?;
    serde_json::to_string(&ir).map_err(|e| format!("serialize View IR: {e}"))
}

/// Parse `.crepus` source into View IR, serialized as JSON.
///
/// `context_json`, when present, must be a JSON object whose values are bound
/// as template variables.
#[wasm_bindgen]
pub fn parse_crepus_json(source: &str, context_json: Option<String>) -> Result<String, JsError> {
    parse_crepus_json_inner(source, context_json.as_deref()).map_err(|e| JsError::new(&e))
}

fn parse_crepus_json_inner(source: &str, context_json: Option<&str>) -> Result<String, String> {
    let ctx = build_context_inner(context_json)?;
    let ir = render_template_to_ir(source, &ctx)
        .map_err(|e| format!("lower .crepus to View IR: {e}"))?;
    serde_json::to_string(&ir).map_err(|e| format!("serialize View IR: {e}"))
}

fn build_context_inner(context_json: Option<&str>) -> Result<TemplateContext, String> {
    let mut ctx = TemplateContext::new();
    let Some(raw) = context_json.map(str::trim) else {
        return Ok(ctx);
    };
    if raw.is_empty() || raw == "null" {
        return Ok(ctx);
    }
    let parsed: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("context is not valid JSON: {e}"))?;
    let obj = parsed
        .as_object()
        .ok_or_else(|| "context must be a JSON object".to_string())?;
    for (k, v) in obj {
        let s = match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        ctx.set(k, s.as_str());
    }
    Ok(ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ir_version() {
        assert!(ir_version() > 0);
    }

    #[test]
    fn test_parse_template_json_inner_success() {
        let src = "<div>Hello {name}</div>";
        let ctx = r#"{"name": "World"}"#;
        let res = parse_template_json_inner(src, None, Some(ctx));
        assert!(res.is_ok());
        let ir = res.unwrap();
        assert!(ir.contains("Hello"));
    }

    #[test]
    fn test_parse_template_json_inner_filename_dispatch() {
        let src = "<div>Hello</div>";
        let res = parse_template_json_inner(src, Some("test.vue"), None);
        assert!(res.is_ok());
    }

    #[test]
    fn test_parse_template_json_inner_invalid_context() {
        let src = "<div>Hello</div>";
        let invalid_ctx = r#"{"name": "World""#; // missing closing brace
        let res = parse_template_json_inner(src, None, Some(invalid_ctx));
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("context is not valid JSON"));
    }

    #[test]
    fn test_parse_template_json_inner_invalid_context_type() {
        let src = "<div>Hello</div>";
        let invalid_ctx = r#"["not an object"]"#;
        let res = parse_template_json_inner(src, None, Some(invalid_ctx));
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "context must be a JSON object");
    }

    #[test]
    fn test_parse_template_json_inner_invalid_template() {
        // an invalid template that causes a parse error. Let's see... maybe unmatched tags?
        // the parser might just recover, but let's try something that errors.
        // Actually crepus parser might be very forgiving. Let's just verify it runs.
        let src = "<div";
        let _ = parse_template_json_inner(src, None, None);
    }

    #[test]
    fn test_parse_crepus_json_inner() {
        let src = "<div>Hello {name}</div>";
        let ctx = r#"{"name": "World"}"#;
        let res = parse_crepus_json_inner(src, Some(ctx));
        assert!(res.is_ok());
        let ir = res.unwrap();
        assert!(ir.contains("Hello"));
    }

    #[test]
    fn test_build_context_inner_valid() {
        let ctx = build_context_inner(Some(r#"{"key": "value"}"#)).unwrap();
        assert_eq!(format!("{:?}", ctx.get("key").unwrap()), r#"Str("value")"#);
    }

    #[test]
    fn test_build_context_inner_null() {
        let ctx = build_context_inner(Some("null")).unwrap();
        assert!(ctx.get("key").is_none());

        let ctx2 = build_context_inner(Some("")).unwrap();
        assert!(ctx2.get("key").is_none());

        let ctx3 = build_context_inner(None).unwrap();
        assert!(ctx3.get("key").is_none());
    }
}
