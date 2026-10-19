use crepuscularity_native::{
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component, ViewIr,
};
use serde_json::json;

#[test]
fn dropzones_cancel_browser_defaults_with_or_without_a_host_callback() {
    let action = "files.drop\"\\{key}";
    for callback in [None, Some(action)] {
        let mut node = json!({
            "kind":"dropzone", "label":"Drop here", "children":[{"kind":"text","content":"Visible child"}],
            "style":{"id":"drop-target","ariaLabel":"Drop files","classes":["rounded","p-2"]}
        });
        if let Some(callback) = callback {
            node["onDrop"] = callback.into();
        }
        let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[node]})).unwrap();
        for (name, output, drag_event, drop_event) in [
            (
                "react",
                emit_react_component(&ir, "DropTarget"),
                "onDragOver=",
                "onDrop=",
            ),
            (
                "solid",
                emit_solid_component(&ir, "DropTarget"),
                "onDragOver=",
                "onDrop=",
            ),
            (
                "svelte",
                emit_svelte_component(&ir),
                "ondragover=",
                "ondrop=",
            ),
            ("vue", emit_vue_component(&ir), "@dragover=", "@drop="),
        ] {
            let output = output.replace("&quot;", "\"").replace("&amp;", "&");
            assert!(output.contains(drag_event), "{name}: {output}");
            assert!(output.contains(drop_event), "{name}: {output}");
            assert!(
                output.matches("preventDefault()").count() >= 2,
                "{name}: {output}"
            );
            for retained in ["drop-target", "Drop files", "rounded p-2", "Visible child"] {
                assert!(
                    output.contains(retained),
                    "{name}: missing {retained}: {output}"
                );
            }
            if callback.is_some() {
                let encoded = serde_json::to_string(action).unwrap();
                assert!(output.contains(&encoded), "{name}: {output}");
                assert!(output.contains("__crepusOwnHandler("), "{name}: {output}");
                assert!(
                    output.contains("?.(event)") || output.contains("?.($event)"),
                    "{name}: {output}"
                );
            } else {
                assert!(
                    !output.contains("__crepusOwnHandler(handlers"),
                    "{name}: {output}"
                );
            }
        }
    }
}
