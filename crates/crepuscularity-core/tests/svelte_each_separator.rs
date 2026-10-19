use crepuscularity_core::{ast::Node, parse_template_with_path};
use std::path::Path;

#[test]
fn each_separator_skips_quoted_and_nested_as_text() {
    for iterator in [
        "'snow as ice'",
        "\"snow as ice\"",
        "('snow as ice')",
        "getItems(' as ')",
        "items[\" as \"]",
        "'snow\\' as ice'",
    ] {
        let source = format!("{{#each {iterator} as item}}<span>{{item}}</span>{{/each}}");
        let nodes = parse_template_with_path(&source, Some(Path::new("view.svelte"))).unwrap();
        let Node::For(block) = &nodes[0] else {
            panic!("expected loop")
        };
        assert_eq!(block.iterator, iterator);
        assert_eq!(block.pattern, "item");
    }
}

#[test]
fn quoted_or_nested_as_does_not_replace_a_binding_separator() {
    for head in ["'snow as ice'", "getItems(' as item')", "items[' as item']"] {
        let source = format!("{{#each {head}}}<span/>{{/each}}");
        let error = parse_template_with_path(&source, Some(Path::new("view.svelte"))).unwrap_err();
        assert!(
            error.to_string().contains("without an `as` binding"),
            "{error}"
        );
    }
}
