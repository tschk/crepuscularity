#![cfg(feature = "std")]

use crepuscularity_core::{TemplateContext, TemplateValue};
use crepuscularity_embedded::{
    render_template_to_framebuffer, EmbeddedNode, Rgb565Buffer, ScreenSize, DEFAULT_BG,
};

fn texts(nodes: &[EmbeddedNode], out: &mut Vec<String>) {
    for node in nodes {
        if let Some(text) = &node.text {
            out.push(text.clone());
        }
        texts(&node.children, out);
    }
}

#[test]
fn loop_overlays_and_nested_declarations_preserve_output_and_pixels() {
    let mut first = TemplateContext::new();
    first.set("item", "field collides with alias");
    first.set("value", 7_i64);
    first.set("temporary", "row");
    let mut second = TemplateContext::new();
    second.set("value", 9_i64);
    let mut ctx = TemplateContext::new();
    ctx.set("items", TemplateValue::List(vec![first, second]));
    ctx.set("item", "parent alias");
    ctx.set("temporary", "parent");
    ctx.set(
        "large",
        TemplateValue::List(vec![TemplateContext::new(); 128]),
    );
    let source = "div flex-col\n  for item in items\n    span \"{item}:{temporary}\"\n    if true\n      $: let temporary = \"nested\"\n      span \"{temporary}\"\n    span \"{temporary}\"\n  span \"{item}:{temporary}\"";
    let expanded = "div flex-col\n  div\n    span \"7:row\"\n    div\n      span \"nested\"\n    span \"row\"\n    span \"9:parent\"\n    div\n      span \"nested\"\n    span \"parent\"\n  span \"parent alias:parent\"";
    let size = ScreenSize::new(240, 160);
    let mut actual = Rgb565Buffer::new(size, DEFAULT_BG);
    let mut expected = Rgb565Buffer::new(size, DEFAULT_BG);
    let document = render_template_to_framebuffer(source, &ctx, size, &mut actual).unwrap();
    render_template_to_framebuffer(expanded, &ctx, size, &mut expected).unwrap();
    let mut rendered = Vec::new();
    texts(&document.root, &mut rendered);
    assert_eq!(
        rendered,
        [
            "7:row",
            "nested",
            "row",
            "9:parent",
            "nested",
            "parent",
            "parent alias:parent"
        ]
    );
    assert!(actual
        .pixels()
        .iter()
        .any(|pixel| *pixel != DEFAULT_BG.to_rgb565().0));
    assert_eq!(actual.pixels(), expected.pixels());
    assert_eq!(ctx.get_str("item"), "parent alias");
    assert_eq!(ctx.get_str("temporary"), "parent");
}

#[test]
fn default_declarations_and_loop_errors_leave_parent_scope_intact() {
    let mut row = TemplateContext::new();
    row.set("value", 1_i64);
    let mut ctx = TemplateContext::new();
    ctx.set("name", "parent");
    ctx.set("items", TemplateValue::List(vec![row]));
    let size = ScreenSize::new(128, 64);
    let mut fb = Rgb565Buffer::new(size, DEFAULT_BG);
    let source =
        "$: default name = \"ignored\"\nspan \"{name}\"\n$: let name = \"local\"\nspan \"{name}\"";
    let document = render_template_to_framebuffer(source, &ctx, size, &mut fb).unwrap();
    let mut rendered = Vec::new();
    texts(&document.root, &mut rendered);
    assert_eq!(rendered, ["parent", "local"]);
    assert_eq!(ctx.get_str("name"), "parent");
    let missing_dir = tempfile::tempdir().unwrap();
    ctx.base_dir = Some(missing_dir.path().to_path_buf());
    assert!(render_template_to_framebuffer(
        "for item in items\n  $: let name = \"changed\"\n  include __crepus_missing_loop_scope_test__.crepus",
        &ctx,
        size,
        &mut fb
    )
    .is_err());
    let document = render_template_to_framebuffer("span \"{name}\"", &ctx, size, &mut fb).unwrap();
    assert_eq!(document.root[0].text.as_deref(), Some("parent"));
}
