use crepuscularity_core::parser::{parse_vue_sfc, parse_vue_sfc_with_generics};
#[test]
fn assigned_sfc_languages_are_literals_instead_of_javascript_values() {
    for (attribute, expected) in [
        ("lang={ts}", "{ts}"),
        ("lang={}", "{}"),
        ("lang=false", "false"),
        ("lang=true", "true"),
        ("lang=ts", "ts"),
        ("lang=' ts '", " ts "),
        ("lang=\"\"", ""),
    ] {
        let source = format!("<script {attribute}></script><style {attribute}></style>");
        let sfc = parse_vue_sfc(&source).unwrap();
        assert_eq!(
            sfc.scripts[0].lang.as_deref(),
            Some(expected),
            "{attribute}"
        );
        assert_eq!(sfc.styles[0].lang.as_deref(), Some(expected), "{attribute}");
    }
}
#[test]
fn malformed_unquoted_sfc_values_are_errors() {
    for value in [r#"{"ts"}"#, "x=y", "`ts`", "<ts"] {
        let source = format!("<script lang={value}></script><template/>");
        assert!(parse_vue_sfc(&source).is_err(), "{source}");
    }
}
#[test]
fn sfc_boolean_flags_use_presence_and_generic_values_stay_literal() {
    let (sfc, generics)=parse_vue_sfc_with_generics("<script setup=\"false\" generic={T}></script><template lang/><style scoped=\"false\" module=\"false\"></style>").unwrap();
    assert!(sfc.scripts[0].setup);
    assert!(sfc.styles[0].scoped);
    assert!(sfc.styles[0].module);
    assert_eq!(generics, vec![Some("{T}".to_owned())]);
}

#[test]
fn default_template_languages_and_literal_slashes_follow_block_boundaries() {
    for attribute in ["lang", "lang=\"\"", "lang=html"] {
        let source = format!("<template {attribute}><p>HTML</p></template>");
        assert!(parse_vue_sfc(&source).is_ok(), "{source}");
    }
    assert!(parse_vue_sfc("<script lang=ts />").is_ok());
    assert!(parse_vue_sfc("<script lang=ts/>").is_err());
    assert_eq!(
        parse_vue_sfc("<script lang=ts/></script>").unwrap().scripts[0]
            .lang
            .as_deref(),
        Some("ts/")
    );
}

#[test]
fn style_attribute_names_are_retained_for_backend_validation() {
    let source = r#"<template><p /></template><style global data-note="x>y">p{color:red}</style>"#;
    let (sfc, metadata) = crepuscularity_core::parser::parse_vue_sfc_with_metadata(source).unwrap();
    assert_eq!(metadata.style_attributes[0], ["global", "data-note"]);
    assert_eq!(sfc.styles[0].content, "p{color:red}");
}
