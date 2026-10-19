use crepuscularity_core::{ast::Node, parse_template_with_path};
use std::path::Path;

fn sources(name: &str) -> Vec<(&'static str, String)> {
    vec![
        ("view.crepus", format!("for {name} in items\n  div")),
        (
            "view.jsx",
            format!("<for let='{name}' in={{items}}><div/></for>"),
        ),
        (
            "view.svelte",
            format!("{{#each items as {name}}}<div/>{{/each}}"),
        ),
        ("view.astro", format!("{{items.map(({name}) => <div/>)}}")),
        (
            "view.vue",
            format!("<template><div v-for=\"{name} in items\"/></template>"),
        ),
        (
            "view.ng.html",
            format!("<div *ngFor=\"let {name} of items\"></div>"),
        ),
        (
            "view.ng.html",
            format!("@for ({name} of items; track {name}) {{<div/>}}"),
        ),
    ]
}

#[test]
fn loop_bindings_reject_unreferenceable_names() {
    for name in [
        "a+b",
        "user.name",
        "123",
        "true",
        "null",
        "a-b",
        "$value",
        "界",
    ] {
        for (filename, source) in sources(name) {
            assert!(
                parse_template_with_path(&source, Some(Path::new(filename))).is_err(),
                "accepted {filename}: {source}"
            );
        }
    }
}

#[test]
fn valid_loop_names_keep_their_binding_and_iterator() {
    for name in ["item", "_value", "value2"] {
        for (filename, source) in sources(name) {
            let nodes = parse_template_with_path(&source, Some(Path::new(filename))).unwrap();
            let Node::For(block) = &nodes[0] else {
                panic!("expected loop for {filename}: {source}")
            };
            assert_eq!(block.pattern, name);
            assert_eq!(block.iterator, "items");
        }
    }
}
