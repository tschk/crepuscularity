use crepuscularity_core::{ast::Node, parse_template_with_path, TemplateContext};
use crepuscularity_native::{render_nodes_to_ir, render_template_to_ir_with_path};
use std::path::Path;

#[test]
fn raw_html_is_rejected_in_ordinary_and_flattened_view_ir_paths() {
    for (filename, source) in [
        ("Entry.svelte", "{@html '<b>bold</b>'}"),
        ("Entry.svelte", "<div>{@html markup}</div>"),
        ("Entry.svelte", "<button>{@html markup}</button>"),
        (
            "Entry.svelte",
            "<button label='Explicit'>{@html markup}</button>",
        ),
        (
            "Entry.svelte",
            "<picker><option value='one'>{@html markup}</option></picker>",
        ),
        ("Entry.vue", "<template><p v-html=\"markup\"/></template>"),
        (
            "Entry.vue",
            "<template><div><p v-html=\"markup\"/></div></template>",
        ),
        (
            "Entry.vue",
            "<template><button v-html=\"markup\"/></template>",
        ),
        (
            "Entry.vue",
            "<template><button label='Explicit' v-html=\"markup\"/></template>",
        ),
        (
            "Entry.vue",
            "<template><picker><option value='one' v-html=\"markup\"/></picker></template>",
        ),
    ] {
        let mut context = TemplateContext::new();
        context.set("markup", "<b>bold</b>");
        let error = render_template_to_ir_with_path(source, &context, Some(Path::new(filename)))
            .expect_err(&format!(
                "raw HTML was escaped or dropped: {filename} {source}"
            ));
        assert!(
            error
                .to_string()
                .contains("View IR does not support raw HTML"),
            "{filename}: {error}"
        );
    }
}

#[test]
fn raw_text_and_vue_text_directives_keep_literal_markup_as_text() {
    let mut context = TemplateContext::new();
    context.set("markup", "<b>bold</b>");
    let direct = render_nodes_to_ir(&[Node::RawText("markup".into())], &context).unwrap();
    assert_eq!(
        serde_json::to_value(direct).unwrap()["root"][0]["content"],
        "<b>bold</b>"
    );
    let vue = render_template_to_ir_with_path(
        "<template><p v-text=\"markup\"/></template>",
        &context,
        Some(Path::new("Entry.vue")),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(vue).unwrap()["root"][0]["content"],
        "<b>bold</b>"
    );
}

#[test]
fn shared_frontend_parser_still_represents_raw_html_for_other_renderers() {
    let svelte =
        parse_template_with_path("{@html markup}", Some(Path::new("Entry.svelte"))).unwrap();
    assert!(matches!(&svelte[..], [Node::RawHtml(expression)] if expression == "markup"));
    let vue = parse_template_with_path(
        "<template><p v-html=\"markup\"/></template>",
        Some(Path::new("Entry.vue")),
    )
    .unwrap();
    assert!(
        matches!(&vue[..], [Node::Element(element)] if matches!(&element.children[..], [Node::RawHtml(expression)] if expression == "markup"))
    );
}
