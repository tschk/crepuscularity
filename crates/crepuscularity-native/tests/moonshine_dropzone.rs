use crepuscularity_native::{emit_moonshine_dom_component, emit_moonshine_ssr_component, ViewIr};
use serde_json::json;

#[test]
fn direct_host_dropzones_cancel_defaults_and_preserve_callbacks_and_content() {
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
        for output in [
            emit_moonshine_dom_component(&ir, "DropTarget").unwrap(),
            emit_moonshine_ssr_component(&ir, "DropTarget").unwrap(),
        ] {
            for retained in [
                "onDragOver",
                "onDrop",
                "drop-target",
                "rounded p-2",
                "File drop target",
                "Visible child",
            ] {
                assert!(output.contains(retained), "missing {retained}: {output}");
            }
            assert!(output.matches("preventDefault()").count() >= 2, "{output}");
            if let Some(callback) = callback {
                assert!(
                    output.contains(&format!(
                        "__crepusOwnHandler(handlers(), {})",
                        serde_json::to_string(callback).unwrap()
                    )),
                    "{output}"
                );
                assert!(output.contains("?.(event)"), "{output}");
                let drop = output.rsplit("onDrop:").next().unwrap();
                assert!(
                    drop.find("preventDefault()").unwrap()
                        < drop.find("__crepusOwnHandler(").unwrap(),
                    "{drop}"
                );
            } else {
                assert!(
                    !output.contains("__crepusOwnHandler(handlers()"),
                    "{output}"
                );
            }
        }
    }
}
