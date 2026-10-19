use crepuscularity_native::{emit_imba_component, emit_vue_component, ViewIr};
use serde_json::json;

fn tree(bind: &str) -> ViewIr {
    serde_json::from_value(json!({"version":7,"root":[{
        "kind":"picker","bind":bind,"options":[
            {"value":"","label":"Empty"},
            {"value":"one","label":"First"},
            {"value":"one","label":"Duplicate"},
            {"value":"two","label":"Two"}
        ]
    }]}))
    .unwrap()
}

#[test]
fn bound_vue_and_imba_options_encode_first_matching_selection_for_server_html() {
    let ir = tree("prefs.choice");
    let vue = emit_vue_component(&ir);
    let imba = emit_imba_component(&ir).unwrap();
    for (name, output, selected) in [
        ("vue", vue.as_str(), ":selected="),
        ("imba", imba.source.as_str(), " html:selected="),
    ] {
        let options: Vec<_> = output
            .lines()
            .filter(|line| line.contains("<option"))
            .collect();
        assert_eq!(options.len(), 4, "{name}: {output}");
        for index in [0, 1, 3] {
            assert!(
                options[index].contains(selected),
                "{name}: {}",
                options[index]
            );
        }
        assert!(
            !options[2].contains(selected),
            "duplicate must not select twice: {}",
            options[2]
        );
    }
    for line in imba
        .source
        .lines()
        .filter(|line| line.contains("html:selected="))
    {
        assert!(
            line.contains("? \"\" : null)"),
            "false selection must omit the attribute: {line}"
        );
    }
    assert!(vue.contains("prefs"), "{vue}");
    assert!(imba.bindings.contains("prefs"), "{}", imba.bindings);
}

#[test]
fn unbound_vue_and_imba_pickers_leave_option_selection_to_the_browser() {
    for bind in ["", "  "] {
        let ir = tree(bind);
        let vue = emit_vue_component(&ir);
        let imba = emit_imba_component(&ir).unwrap();
        for (output, selected) in [(vue, ":selected="), (imba.source, " html:selected=")] {
            assert!(!output.contains(selected), "{output}");
        }
    }
}
