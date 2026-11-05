#![cfg(feature = "std")]

use std::sync::Arc;

use crepuscularity_core::{parse_template, TemplateContext};
use crepuscularity_embedded::{
    render_component_file_to_framebuffer, render_file_to_framebuffer, render_nodes_to_document,
    render_template_to_framebuffer, EmbeddedNode, Rgb565Buffer, ScreenSize, Template, DEFAULT_BG,
};

fn texts(nodes: &[EmbeddedNode]) -> Vec<String> {
    let mut out = Vec::new();
    for node in nodes {
        if let Some(text) = &node.text {
            out.push(text.clone());
        }
        out.extend(texts(&node.children));
    }
    out
}

fn bundle(files: &[(&str, &str)]) -> TemplateContext {
    let mut ctx = TemplateContext::new();
    Arc::make_mut(&mut ctx.virtual_files).extend(
        files
            .iter()
            .map(|(path, source)| (path.to_string(), source.to_string())),
    );
    ctx.set("private_parent", "must not leak");
    ctx
}

#[test]
fn ordinary_and_nested_includes_keep_reserved_environment_and_normal_props() {
    let ctx = bundle(&[
        ("outer.crepus", "include leaf.crepus title={title} screen_width=1 is_embedded=false"),
        ("leaf.crepus", "span \"{crepus_target}|{is_embedded}|{is_tui}|{is_web}|{is_gui}|{screen_width}x{screen_height}|{title}|{private_parent}\""),
    ]);
    let source = "include leaf.crepus title=\"direct\" crepus_target=\"fake\" is_embedded=false is_tui=true is_web=true is_gui=true screen_width=1 screen_height=2\ninclude outer.crepus title=\"nested\" screen_height=3";
    let size = ScreenSize::new(128, 64);
    let mut actual = Rgb565Buffer::new(size, DEFAULT_BG);
    let mut expected = Rgb565Buffer::new(size, DEFAULT_BG);
    let document = render_template_to_framebuffer(source, &ctx, size, &mut actual).unwrap();
    assert_eq!(
        texts(&document.root),
        [
            "embedded|true|false|false|false|128x64|direct|",
            "embedded|true|false|false|false|128x64|nested|"
        ]
    );
    render_template_to_framebuffer("span \"embedded|true|false|false|false|128x64|direct|\"\nspan \"embedded|true|false|false|false|128x64|nested|\"", &TemplateContext::new(), size, &mut expected).unwrap();
    assert!(actual
        .pixels()
        .iter()
        .any(|pixel| *pixel != DEFAULT_BG.to_rgb565().0));
    assert_eq!(actual.pixels(), expected.pixels());
    let changed = ScreenSize::new(160, 80);
    let mut fb = Rgb565Buffer::new(changed, DEFAULT_BG);
    let document = render_template_to_framebuffer(source, &ctx, changed, &mut fb).unwrap();
    assert_eq!(
        texts(&document.root)[1],
        "embedded|true|false|false|false|160x80|nested|"
    );
    assert!(ctx.get("screen_width").is_none());
}

#[test]
fn named_defaults_and_props_cannot_replace_target_environment() {
    let ctx = bundle(&[("named.crepus", "+++\n[Card.defaults]\ncrepus_target = 'fake'\nis_embedded = false\nis_tui = true\nis_web = true\nis_gui = true\nscreen_width = 1\nscreen_height = 2\ntitle = 'Default title'\n+++\n--- Card\nspan \"{crepus_target}|{is_embedded}|{is_tui}|{is_web}|{is_gui}|{screen_width}x{screen_height}|{title}|{private_parent}\"")]);
    let size = ScreenSize::new(128, 64);
    let mut fb = Rgb565Buffer::new(size, DEFAULT_BG);
    let document = render_template_to_framebuffer("include named.crepus#Card\ninclude named.crepus#Card title=\"Provided\" is_embedded=false screen_width=7", &ctx, size, &mut fb).unwrap();
    assert_eq!(
        texts(&document.root),
        [
            "embedded|true|false|false|false|128x64|Default title|",
            "embedded|true|false|false|false|128x64|Provided|"
        ]
    );
}

#[test]
fn nested_slot_contexts_keep_target_environment_and_parent_scope() {
    let ctx = bundle(&[
        ("outer.crepus", "include middle.crepus\n  slot"),
        ("middle.crepus", "include leaf.crepus\n  slot"),
        (
            "leaf.crepus",
            "if is_embedded\n  slot\nelse\n  span \"wrong target\"",
        ),
    ]);
    let size = ScreenSize::new(128, 64);
    let mut fb = Rgb565Buffer::new(size, DEFAULT_BG);
    let document = render_template_to_framebuffer("include outer.crepus\n  span \"{crepus_target}|{is_embedded}|{screen_width}x{screen_height}|{private_parent}\"", &ctx, size, &mut fb).unwrap();
    assert_eq!(
        texts(&document.root),
        ["embedded|true|128x64|must not leak"]
    );
}

#[test]
fn low_level_includes_do_not_invent_missing_environment() {
    let ctx = bundle(&[("child.crepus", "if is_embedded\n  span \"invented\"\nelse\n  span \"missing\"\nif screen_width == null\n  span \"no width\"")]);
    let nodes = parse_template("include child.crepus").unwrap();
    let document = render_nodes_to_document(&nodes, &ctx, ScreenSize::new(128, 64)).unwrap();
    assert_eq!(texts(&document.root), ["missing", "no width"]);
    let nodes = parse_template("include child.crepus is_embedded=true screen_width=7").unwrap();
    let document = render_nodes_to_document(&nodes, &ctx, ScreenSize::new(128, 64)).unwrap();
    assert_eq!(texts(&document.root), ["invented"]);
}

#[test]
fn disk_includes_receive_actual_screen_dimensions() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("child.crepus"),
        "span \"{crepus_target}:{screen_width}x{screen_height}\"",
    )
    .unwrap();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(dir.path().to_path_buf());
    let size = ScreenSize::new(128, 64);
    let mut fb = Rgb565Buffer::new(size, DEFAULT_BG);
    let document =
        render_template_to_framebuffer("include child.crepus", &ctx, size, &mut fb).unwrap();
    assert_eq!(texts(&document.root), ["embedded:128x64"]);
    let entry = dir.path().join("entry.crepus");
    std::fs::write(&entry, "include child.crepus").unwrap();
    let document =
        render_file_to_framebuffer(&entry, &TemplateContext::new(), size, &mut fb).unwrap();
    assert_eq!(texts(&document.root), ["embedded:128x64"]);
    let document = render_component_file_to_framebuffer(
        "--- Root\ninclude child.crepus",
        "Root",
        &ctx,
        size,
        &mut fb,
    )
    .unwrap();
    assert_eq!(texts(&document.root), ["embedded:128x64"]);
    let mut template = Template::from_source_with_path("include child.crepus", &entry, size);
    assert_eq!(
        texts(&template.draw(&mut fb).unwrap().root),
        ["embedded:128x64"]
    );
}

#[test]
fn explicit_external_slot_context_remains_caller_owned() {
    let mut ctx = TemplateContext::new();
    let mut slot_ctx = TemplateContext::new();
    slot_ctx
        .set("crepus_target", "external")
        .set("is_embedded", false)
        .set("screen_width", 4)
        .set("screen_height", 5);
    ctx.slot = Some((
        parse_template("span \"{crepus_target}:{is_embedded}:{screen_width}x{screen_height}\"")
            .unwrap(),
        Arc::new(slot_ctx),
    ));
    let size = ScreenSize::new(128, 64);
    let mut fb = Rgb565Buffer::new(size, DEFAULT_BG);
    let document = render_template_to_framebuffer("slot", &ctx, size, &mut fb).unwrap();
    assert_eq!(texts(&document.root), ["external:false:4x5"]);
}
