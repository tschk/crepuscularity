use crepuscularity_native::{emit_imba_component, ViewIr};
use serde_json::json;

#[test]
fn imba_dropzones_preserve_content_identity_and_cancel_native_defaults() {
    let action = "files.drop\"\\{key}";
    for callback in [None, Some(action)] {
        let mut node = json!({
            "kind":"dropzone", "label":"Drop files", "children":[{"kind":"text","content":"Visible child"}],
            "style":{"id":"drop-target","classes":["rounded","p-2"],"ariaLabel":"File drop target"}
        });
        if let Some(callback) = callback {
            node["onDrop"] = callback.into();
        }
        let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[node]})).unwrap();
        let output = emit_imba_component(&ir).unwrap();
        let element = output
            .source
            .lines()
            .find(|line| line.contains("<div"))
            .unwrap();
        for retained in [
            "id=(\"drop-target\")",
            "className=(\"rounded p-2\")",
            "aria-label=(\"File drop target\")",
            "@dragover=(e.preventDefault())",
        ] {
            assert!(element.contains(retained), "{element}");
        }
        let key = callback
            .map(|key| {
                serde_json::to_string(key)
                    .unwrap()
                    .replace('{', "\\u007b")
                    .replace('}', "\\u007d")
            })
            .unwrap_or_else(|| "null".into());
        assert!(
            element.contains(&format!("@drop=(bindings.drop(handlers,{key},e))")),
            "{element}"
        );
        assert!(output.source.contains("Visible child"), "{}", output.source);
        assert!(
            output.bindings.contains("export function drop("),
            "{}",
            output.bindings
        );
        let helper = output
            .bindings
            .split("export function drop(")
            .nth(1)
            .unwrap();
        let cancel = helper.find("preventDefault()").unwrap();
        let dispatch = helper.find("dispatch(handlers, key, event)").unwrap();
        assert!(cancel < dispatch, "{helper}");
        assert!(helper.contains("key !== null"), "{helper}");
    }
}
