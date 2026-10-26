#[cfg(any(all(target_arch = "wasm32", feature = "dom"), test))]
use base64::Engine;
use serde_json::Value;
use std::collections::HashMap;

/// Read SSR context and wire signals to DOM.
/// `setup` receives the deserialized server context and should call bind_* functions.
/// Without WASM and the `dom` feature this is a no-op.
pub fn hydrate_root<F: FnOnce(HashMap<String, Value>)>(setup: F) {
    #[cfg(all(target_arch = "wasm32", feature = "dom"))]
    {
        let ctx = read_ctx();
        setup(ctx);
    }
    #[cfg(not(all(target_arch = "wasm32", feature = "dom")))]
    {
        let _ = setup;
    }
}

#[cfg(any(all(target_arch = "wasm32", feature = "dom"), test))]
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
            if value.get("v").and_then(Value::as_u64) == Some(1) && value.get("ctx").is_some() {
                return value
                    .get("ctx")
                    .and_then(Value::as_object)
                    .cloned()
                    .map(|ctx| ctx.into_iter().collect());
            }
            value
                .as_object()
                .cloned()
                .map(|obj| obj.into_iter().collect())
        })
        .unwrap_or_default()
}

#[cfg(all(target_arch = "wasm32", feature = "dom"))]
fn read_ctx() -> HashMap<String, Value> {
    let Some(doc) = web_sys::window().and_then(|window| window.document()) else {
        return HashMap::new();
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
        let ctx = decode_ctx_payload(r#"{"ctx":null,"name":"Grace"}"#, None);
        assert_eq!(ctx["name"], "Grace");
        assert!(ctx["ctx"].is_null());
    }

    #[test]
    fn legacy_application_ctx_objects_are_not_unwrapped() {
        for payload in [
            r#"{"ctx":{"nested":1},"name":"Ada"}"#,
            r#"{"v":2,"ctx":{"nested":1},"name":"Ada"}"#,
        ] {
            let expected = serde_json::from_str::<serde_json::Value>(payload).unwrap();
            let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
            for (raw, encoding) in [(payload, None), (encoded.as_str(), Some("base64"))] {
                let decoded = decode_ctx_payload(raw, encoding);
                assert_eq!(serde_json::to_value(decoded).unwrap(), expected);
            }
        }
    }

    #[test]
    fn malformed_canonical_context_does_not_become_application_state() {
        for ctx in ["null", "false", "[]", "\"invalid\""] {
            let payload = format!(r#"{{"v":1,"ctx":{ctx},"bind":{{"0":{{"kind":"text"}}}}}}"#);
            assert!(decode_ctx_payload(&payload, None).is_empty());
        }
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
