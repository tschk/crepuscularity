#[cfg(any(target_arch = "wasm32", test))]
use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;

/// Read SSR context and wire signals to DOM.
/// `setup` receives the deserialized server context and should call bind_* functions.
/// On non-WASM targets this is a no-op (useful for native tests).
pub fn hydrate_root<F: FnOnce(HashMap<String, Value>)>(setup: F) {
    #[cfg(target_arch = "wasm32")]
    {
        let ctx = read_ctx();
        setup(ctx);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = setup;
    }
}

#[cfg(any(target_arch = "wasm32", test))]
fn decode_ctx_payload(raw: &str, encoding: Option<&str>) -> HashMap<String, Value> {
    let json = match encoding {
        Some("base64") => base64::engine::general_purpose::STANDARD
            .decode(raw.trim())
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok()),
        _ => Some(raw.to_string()),
    };
    json.and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|value| {
            if let Some(ctx) = value.get("ctx").and_then(Value::as_object) {
                return Some(ctx.clone().into_iter().collect());
            }
            value
                .as_object()
                .cloned()
                .map(|obj| obj.into_iter().collect())
        })
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn read_ctx() -> HashMap<String, Value> {
    let doc = match web_sys::window().and_then(|w| w.document()) {
        Some(d) => d,
        None => return Default::default(),
    };
    if let Some(el) = doc.get_element_by_id("__crepus_hydration__") {
        let encoding = el.get_attribute("data-crepus-encoding");
        return decode_ctx_payload(&el.text_content().unwrap_or_default(), encoding.as_deref());
    }
    if let Some(el) = doc.get_element_by_id("__crepus_ctx__") {
        let encoding = el.get_attribute("data-encoding");
        return decode_ctx_payload(&el.text_content().unwrap_or_default(), encoding.as_deref());
    }
    Default::default()
}

#[cfg(test)]
mod tests {
    use super::decode_ctx_payload;
    use base64::Engine;

    #[test]
    fn decodes_base64_context_payload() {
        let encoded = base64::engine::general_purpose::STANDARD
            .encode(r#"{"v":1,"ctx":{"name":"Ada","n":2},"bind":{}}"#);
        let ctx = decode_ctx_payload(&encoded, Some("base64"));
        assert_eq!(ctx["name"], "Ada");
        assert_eq!(ctx["n"], 2);
    }

    #[test]
    fn keeps_legacy_base64_context_payload_compatible() {
        let encoded = base64::engine::general_purpose::STANDARD.encode(r#"{"name":"Ada","n":2}"#);
        let ctx = decode_ctx_payload(&encoded, Some("base64"));
        assert_eq!(ctx["name"], "Ada");
        assert_eq!(ctx["n"], 2);
    }

    #[test]
    fn keeps_raw_json_context_compatible() {
        let ctx = decode_ctx_payload(r#"{"name":"Grace"}"#, None);
        assert_eq!(ctx["name"], "Grace");
    }

    #[test]
    fn canonical_payload_decodes_ctx_and_ignores_bind_metadata() {
        let ctx = decode_ctx_payload(
            r#"{"v":1,"ctx":{"count":3},"bind":{"0":{"kind":"text"}}}"#,
            None,
        );
        assert_eq!(ctx["count"], 3);
        assert!(!ctx.contains_key("bind"));
    }
}
