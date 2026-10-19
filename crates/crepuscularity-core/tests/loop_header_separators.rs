use crepuscularity_core::{ast::Node, parse_template_with_path};
use std::path::Path;

#[test]
fn vue_and_angular_keep_separator_words_inside_iterators() {
    for iterator in [
        "'snow in ice'",
        "'snow of ice'",
        "' in of '",
        "(' in ')",
        "items[' of ']",
        "getItems(' in ', ' of ')",
        "'snow\\' in ice'",
    ] {
        for keyword in ["in", "of"] {
            for (filename, source) in [
                (
                    "view.vue",
                    format!("<template><div v-for=\"item {keyword} {iterator}\"/></template>"),
                ),
                (
                    "view.ng.html",
                    format!("<div *ngFor=\"let item {keyword} {iterator}\"></div>"),
                ),
                (
                    "view.ng.html",
                    format!("@for (item {keyword} {iterator}; track item) {{<div/>}}"),
                ),
            ] {
                let nodes = parse_template_with_path(&source, Some(Path::new(filename))).unwrap();
                let Node::For(block) = &nodes[0] else {
                    panic!("expected loop")
                };
                assert_eq!(block.pattern, "item");
                assert_eq!(block.iterator, iterator, "{filename}: {source}");
            }
        }
    }
}
