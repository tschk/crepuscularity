use crepuscularity_core::parse_template_with_path;
use std::path::Path;

#[test]
fn vue_loops_reject_unsupported_bindings() {
    for pattern in [
        "(item, i)",
        "(value, key, index)",
        "item, i",
        "{name}",
        "[item]",
        "a+b",
        "user.name",
        "true",
        "123",
        "界",
    ] {
        let source =
            format!("<template><div v-for=\"{pattern} in items\">{{{{item}}}}</div></template>");
        assert!(
            parse_template_with_path(&source, Some(Path::new("view.vue"))).is_err(),
            "accepted {source}"
        );
    }
}

#[test]
fn vue_single_item_loop_forms_keep_the_same_ast() {
    for name in ["item", "_value", "value2"] {
        let mut expected = None;
        for head in [
            format!("{name} in items"),
            format!("({name}) in items"),
            format!("{name} of items"),
        ] {
            let source = format!("<template><div v-for=\"{head}\">{{{{{name}}}}}</div></template>");
            let nodes = parse_template_with_path(&source, Some(Path::new("view.vue"))).unwrap();
            let crepuscularity_core::ast::Node::For(block) = &nodes[0] else {
                panic!("expected loop")
            };
            assert_eq!(block.pattern, name);
            assert_eq!(block.iterator, "items");
            if let Some(expected) = &expected {
                assert_eq!(&nodes, expected);
            } else {
                expected = Some(nodes);
            }
        }
    }
}
