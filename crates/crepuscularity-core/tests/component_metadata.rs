use crepuscularity_core::parser::parse_component_file;

#[test]
fn description_must_be_a_string() {
    for value in ["42", "true", "[1]", "{ text = 'Lost' }", "2026-10-13"] {
        let source = format!("+++\n[Card]\ndescription = {value}\n+++\n--- Card\nspan");
        let error = parse_component_file(&source).unwrap_err();
        assert!(error
            .to_string()
            .contains("Card.description must be a TOML string"));
    }
    let file = parse_component_file("+++\n[Card]\ndescription = ''\n+++\n--- Card\nspan").unwrap();
    assert_eq!(
        file.components["Card"].meta.description.as_deref(),
        Some("")
    );
    let file = parse_component_file("--- Card\nspan").unwrap();
    assert_eq!(file.components["Card"].meta.description, None);
}
