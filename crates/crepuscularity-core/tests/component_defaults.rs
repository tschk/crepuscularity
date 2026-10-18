use crepuscularity_core::parser::parse_component_file;

#[test]
fn frontmatter_delimiter_must_fill_its_line() {
    let source = "+++\n[Card]\ndescription = '''\n+++literal\n'''\n+++\n--- Card\nspan";
    let file = parse_component_file(source).unwrap();
    assert_eq!(
        file.components["Card"].meta.description.as_deref(),
        Some("+++literal\n")
    );
    let source = "+++\n[Card]\ndescription = 'Kept'\n+++suffix\n--- Card\nspan";
    assert!(parse_component_file(source).is_err());
}

#[test]
fn unclosed_frontmatter_is_an_error() {
    for source in [
        "+++",
        "+++\n[Card.defaults]\ntitle = 'Lost'\n--- Card\nspan",
        "  \n+++\r\n[Card]\r\n--- Card\r\nspan",
    ] {
        let error = parse_component_file(source).unwrap_err();
        assert!(error.to_string().contains("unclosed component frontmatter"));
    }
    assert!(parse_component_file("+++\n+++\n--- Card\nspan").is_ok());
    assert!(parse_component_file("--- Card\nspan").is_ok());
    let diagnostics = crepuscularity_core::diagnose_crepus_source("\n  +++\n[Card]");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].start_line, 1);
    assert_eq!(diagnostics[0].start_character, 2);
}

#[test]
fn component_defaults_must_be_a_table() {
    for defaults in ["'ignored'", "[1, 2]", "true", "1"] {
        let source = format!("+++\n[Card]\ndefaults = {defaults}\n+++\n--- Card\nspan");
        let error = parse_component_file(&source).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Card.defaults must be a TOML table"),
            "{error}"
        );
    }
    let source = "+++\n[Card]\ndefaults = { count = 1 }\n+++\n--- Card\nspan";
    let file = parse_component_file(source).unwrap();
    assert_eq!(file.components["Card"].meta.defaults["count"], "1");
}

#[test]
fn unsupported_component_default_values_are_errors_instead_of_empty_strings() {
    for value in [
        "nan",
        "inf",
        "-inf",
        "[1, 2]",
        "{ nested = 1 }",
        "2026-10-13",
    ] {
        let source =
            format!("+++\n[Card.defaults]\nvalue = {value}\n+++\n--- Card\nspan\n  \"{{value}}\"");
        let error = parse_component_file(&source).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("invalid component default Card.value"),
            "{error}"
        );
    }
    let source = "+++\n[Card.defaults]\nempty = ''\ncount = 1\nratio = 1.0\nactive = true\n+++\n--- Card\nspan";
    let file = parse_component_file(source).unwrap();
    assert_eq!(file.components["Card"].meta.defaults.len(), 4);
}
