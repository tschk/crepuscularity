use crepuscularity_core::parser::{split_component_body_sections, split_frontmatter_parts};
use crepuscularity_core::{
    diagnose_crepus_source, is_multi_component_file, parse_component_file, TemplateContext,
};

#[test]
fn component_bom_preserves_metadata_defaults_sections_and_render_context() {
    for source in [
        "+++\n[Card]\ndescription = 'A card'\n[Card.defaults]\ntitle = 'Hello'\n+++\n--- Card\nspan\n  \"{title}\"\n--- Other\nspan\n  \"inside\u{feff}text\"",
        "--- Card\nspan\n  \"Hello\"\n--- Other\nspan\n  \"inside\u{feff}text\"",
    ] {
        let bom=format!("\u{feff}{source}");
        let plain=parse_component_file(source).unwrap();
        let actual=parse_component_file(&bom).unwrap();
        assert_eq!(actual.components.len(),2);
        for name in ["Card","Other"] {
            assert_eq!(actual.components[name].nodes,plain.components[name].nodes);
            assert_eq!(actual.components[name].meta.defaults,plain.components[name].meta.defaults);
            assert_eq!(actual.components[name].meta.description,plain.components[name].meta.description);
        }
        let (_, body, offset)=split_frontmatter_parts(&bom);
        assert_eq!(&bom[offset..],body);
        let sections=split_component_body_sections(body);
        assert_eq!(sections.len(),2);
        assert_eq!(sections[0].0,"Card");
        for (_, text, start) in sections {assert!(body[start..].starts_with(text.lines().next().unwrap()));}
        let rendered=crepuscularity_core::render_entry::render_component_file(&bom,"Card",&TemplateContext::new(),|nodes,ctx|Ok((nodes.to_vec(),ctx.get("title").map(crepuscularity_core::context::value_to_str)))).unwrap();
        assert_eq!(rendered.0,plain.components["Card"].nodes);
        if source.starts_with("+++") {assert_eq!(rendered.1,Some("Hello".into()));}
    }
}

#[test]
fn bom_frontmatter_detection_and_errors_keep_original_coordinates() {
    for source in [
        "+++",
        "+++\n[Card]\ntitle = 'unclosed'",
        "+++\n[Card]\ndescription = [\n+++\n--- Card\nspan",
    ] {
        let bom = format!("\u{feff}{source}");
        assert!(is_multi_component_file(&bom));
        assert!(parse_component_file(&bom).is_err());
        let plain = diagnose_crepus_source(source);
        let actual = diagnose_crepus_source(&bom);
        assert_eq!(actual.len(), 1);
        assert_eq!(actual[0].message, plain[0].message);
        assert_eq!(actual[0].start_line, plain[0].start_line);
        let shift = u32::from(plain[0].start_line == 0);
        assert_eq!(actual[0].start_character, plain[0].start_character + shift);
    }
}

#[test]
fn an_interior_bom_does_not_create_a_component_header() {
    let source = "+++\n+++\n\u{feff}--- Ghost\nspan\n--- Card\nspan\n  \"inside\u{feff}text\"";
    let file = parse_component_file(source).unwrap();
    assert!(!file.components.contains_key("Ghost"));
    assert!(file.components.contains_key("Card"));
}
