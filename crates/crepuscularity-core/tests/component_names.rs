use crepuscularity_core::parser::parse_component_file;

#[test]
fn component_headers_accept_whitespace_separators() {
    let file = parse_component_file("---\tCard\nspan\n--- Button\nbutton").unwrap();
    assert!(file.components.contains_key("Card"));
    assert!(file.components.contains_key("Button"));
}

#[test]
fn ignored_preamble_quotes_do_not_hide_component_headers() {
    let file = parse_component_file("\"Ignored preamble\n--- Card\nspan").unwrap();
    assert!(file.components.contains_key("Card"));
}

#[test]
fn section_markers_inside_multiline_text_remain_literal() {
    let source = "--- Card\nspan\n  \"Literal\n  --- Other\n  still literal\"\n--- Button\nbutton";
    let file = parse_component_file(source).unwrap();
    assert_eq!(file.components.len(), 2);
    assert!(file.components.contains_key("Card"));
    assert!(file.components.contains_key("Button"));
    assert!(!file.components.contains_key("Other"));
}

#[test]
fn duplicate_component_names_are_errors() {
    for source in [
        "--- Card\nspan\n--- Card\nbutton",
        "+++\n[Card.defaults]\ntitle = 'Saved'\n+++\n--- Card\nspan\n--- Card  \nbutton",
        "--- Card\n--- Card\n",
    ] {
        let error = parse_component_file(source).unwrap_err();
        assert!(error.to_string().contains("duplicate component name"));
        assert!(error.to_string().contains("Card"));
    }
}

#[test]
fn distinct_component_names_keep_their_definitions() {
    let file = parse_component_file("--- Card\nspan\n--- Button\nbutton").unwrap();
    assert_eq!(file.components.len(), 2);
    assert_eq!(file.components["Card"].nodes.len(), 1);
    assert_eq!(file.components["Button"].nodes.len(), 1);
}
