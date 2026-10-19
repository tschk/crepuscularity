use crepuscularity_native::{emit_solid_component, ViewIr};
use serde_json::json;

fn emit(bind: &str) -> String {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"picker","bind":bind,"options":[
            {"value":"","label":"Empty"},
            {"value":"one","label":"First"},
            {"value":"one","label":"Duplicate"},
            {"value":"two","label":"Two"}
        ]
    }]}))
    .unwrap();
    emit_solid_component(&ir, "Choices")
}

#[test]
fn bound_solid_pickers_emit_per_option_selection_for_server_html() {
    let output = emit("prefs.choice");
    let options: Vec<_> = output
        .lines()
        .filter(|line| line.contains("<option"))
        .collect();
    assert_eq!(options.len(), 4, "{output}");
    assert!(options[0].contains("selected={"), "{}", options[0]);
    assert!(options[1].contains("selected={"), "{}", options[1]);
    assert!(options[3].contains("selected={"), "{}", options[3]);
    assert!(output.contains("prefs?.choice"), "{output}");
    assert!(
        output.contains("String("),
        "selection must normalize DOM values: {output}"
    );
}

#[test]
fn duplicate_option_values_do_not_emit_multiple_matching_selected_attributes() {
    let output = emit("choice");
    let duplicate = output
        .lines()
        .find(|line| line.contains(">Duplicate</option>"))
        .unwrap();
    assert!(
        !duplicate.contains("selected={") || duplicate.contains("selected={false}"),
        "{duplicate}"
    );
}

#[test]
fn unbound_pickers_leave_selection_to_the_browser() {
    for bind in ["", "  "] {
        let output = emit(bind);
        assert!(!output.contains(" selected="), "{output}");
        assert!(!output.contains(" value={"), "{output}");
    }
}
