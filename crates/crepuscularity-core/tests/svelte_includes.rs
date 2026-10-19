use std::path::Path;

use crepuscularity_core::{ast::Node, parse_template_with_path};

fn parse(source: &str) -> Vec<Node> {
    parse_template_with_path(source, Some(Path::new("view.svelte"))).unwrap()
}

#[test]
fn svelte_include_sources_and_props_lower_to_include_ast() {
    for source_attribute in ["src", "path"] {
        let source = format!("<include {source_attribute}=\"parts/card.vue\" count={{count + 1}} title=\"Local\" {{name}} />");
        let nodes = parse(&source);
        let Node::Include(include) = &nodes[0] else {
            panic!("include: {nodes:?}");
        };
        assert_eq!(include.path, "parts/card.vue");
        assert_eq!(
            include.props,
            [
                ("count".into(), "count + 1".into()),
                ("title".into(), "\"Local\"".into()),
                ("name".into(), "name".into())
            ]
        );
        assert!(include.slot.is_empty());
    }
}

#[test]
fn svelte_include_slots_preserve_svelte_control_blocks() {
    let nodes = parse("<include src=\"card.crepus\">{#if visible}<span>{title}</span>{:else}<b>Fallback</b>{/if}{#each items as item}<span>{item}</span>{/each}</include>");
    let Node::Include(include) = &nodes[0] else {
        panic!("include: {nodes:?}");
    };
    assert!(
        matches!(&include.slot[..], [Node::If(_), Node::For(block)] if block.pattern == "item" && block.iterator == "items")
    );
}

#[test]
fn svelte_include_invalid_sources_are_hard_parse_errors() {
    for source in [
        "<include />",
        "<include src />",
        "<include src=\"\" />",
        "<include path=\"   \" />",
        "<include src={source} />",
        "<include {src} />",
        "<include src=\"a.crepus\" path=\"b.crepus\" />",
        "<include src=\"a.crepus\" src=\"b.crepus\" />",
        "<include src=\"a.crepus\" value={one} value={two} />",
        "<include src={source}><span>Slot</span></include>",
    ] {
        assert!(
            parse_template_with_path(source, Some(Path::new("view.svelte"))).is_err(),
            "{source}"
        );
    }
}
