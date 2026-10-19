use crepuscularity_native::{emit_solid_component, emit_svelte_component, ViewIr};
use serde_json::json;

fn ir(bind: &str, checked: bool) -> ViewIr {
    serde_json::from_value(json!({"version":7,"root":[{
        "kind":"checkbox","label":"State","bind":bind,"checked":checked,
        "style":{"id":"state","formName":"choice","formValue":"yes"}
    }]}))
    .unwrap()
}

#[test]
fn unbound_solid_and_svelte_checkboxes_keep_their_initial_reset_baseline() {
    for bind in ["", "  "] {
        for checked in [true, false] {
            let tree = ir(bind, checked);
            let solid = emit_solid_component(&tree, "States");
            assert!(solid.contains(&format!("checked={{{checked}}}")), "{solid}");
            assert!(
                solid.contains(&format!("prop:defaultChecked={{{checked}}}")),
                "{solid}"
            );
            let svelte = emit_svelte_component(&tree);
            assert!(
                svelte.contains(&format!("defaultChecked={{{checked}}}")),
                "{svelte}"
            );
            assert!(
                svelte.contains(&format!(" checked={{{checked}}}")),
                "{svelte}"
            );
            for output in [&solid, &svelte] {
                for retained in ["id=\"state\"", "name=\"choice\"", "value=\"yes\""] {
                    assert!(output.contains(retained), "missing {retained}: {output}");
                }
            }
        }
    }
}

#[test]
fn bound_solid_and_svelte_checkboxes_retain_live_checked_without_reset_defaults() {
    let tree = ir("prefs.enabled", true);
    for output in [
        emit_solid_component(&tree, "States"),
        emit_svelte_component(&tree),
    ] {
        assert!(output.contains(" checked={"), "{output}");
        assert!(output.contains("prefs?.enabled"), "{output}");
        assert!(!output.contains("defaultChecked="), "{output}");
    }
}
