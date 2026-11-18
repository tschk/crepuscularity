#![cfg(feature = "std")]

use crepuscularity_core::TemplateContext;
use crepuscularity_embedded::{render_template_to_framebuffer, Rgb565View, ScreenSize, Template};

#[test]
fn active_embeds_error_in_roots_includes_and_slots_before_painting() {
    let mut context = TemplateContext::new();
    let files = std::sync::Arc::make_mut(&mut context.virtual_files);
    files.insert(
        "child.crepus".into(),
        "embed widget.js adapter=\"react\"".into(),
    );
    files.insert("shell.crepus".into(), "slot".into());
    files.insert(
        "components.crepus".into(),
        "--- Card\nembed widget.js adapter=\"react\"".into(),
    );
    let screen = ScreenSize::new(8, 8);
    for source in [
        "embed widget.js adapter=\"react\"",
        "include child.crepus",
        "include components.crepus#Card",
        "include shell.crepus\n  embed widget.js adapter=\"react\"",
        "button\n  embed widget.js adapter=\"react\"",
        "button\n  \"Label\"\n  embed widget.js adapter=\"react\"",
        "button\n  include child.crepus",
    ] {
        let source = format!(
            "div bg-[#ffffff]\n  span \"Before\"\n  {}",
            source.replace('\n', "\n  ")
        );
        let mut pixels = [0x1234; 64];
        let mut framebuffer = Rgb565View::new(screen, &mut pixels).unwrap();
        let error = render_template_to_framebuffer(&source, &context, screen, &mut framebuffer)
            .unwrap_err();
        assert!(error.to_string().contains("embed"), "{error}");
        assert!(error.to_string().contains("widget.js"), "{error}");
        assert!(error.to_string().contains("embedded"), "{error}");
        assert_eq!(framebuffer.pixels_mut(), &[0x1234; 64]);
    }
}

#[test]
fn skipped_web_islands_render_the_embedded_fallback() {
    let screen = ScreenSize::new(8, 8);
    let mut pixels = [0x1234; 64];
    let mut framebuffer = Rgb565View::new(screen, &mut pixels).unwrap();
    let document = render_template_to_framebuffer(
        "if {is_web}\n  embed widget.js adapter=\"react\"\nelse\n  div #fallback bg-[#ffffff]",
        &TemplateContext::new(),
        screen,
        &mut framebuffer,
    )
    .unwrap();
    assert!(document.node_by_id("fallback").is_some());
    assert_eq!(framebuffer.pixels_mut(), &[0xffff; 64]);
}

#[test]
fn rejected_island_preserves_the_last_frame_and_template_can_recover() {
    let screen = ScreenSize::new(8, 8);
    let mut pixels = [0; 64];
    let mut framebuffer = Rgb565View::new(screen, &mut pixels).unwrap();
    let mut template = Template::from_source("div bg-[#ffffff]", screen);
    template.draw(&mut framebuffer).unwrap();
    template.set_source("embed widget.js adapter=\"react\"");
    assert!(template.draw(&mut framebuffer).is_err());
    assert_eq!(framebuffer.pixels_mut(), &[0xffff; 64]);
    template.set_source("div bg-[#000000]");
    template.draw(&mut framebuffer).unwrap();
    assert_eq!(framebuffer.pixels_mut(), &[0; 64]);
}

#[test]
fn button_validation_skips_inactive_branches_and_unused_slot_fallbacks() {
    let screen = ScreenSize::new(8, 8);
    for source in [
        "button\n  if is_web\n    embed widget.js adapter=\"react\"\n  \"Label\"",
        "button\n  \"Label\"\n  if is_web\n    embed widget.js adapter=\"react\"",
        "button\n  include shell.crepus\n    span \"Caller\"",
    ] {
        let mut context = TemplateContext::new();
        std::sync::Arc::make_mut(&mut context.virtual_files).insert(
            "shell.crepus".into(),
            "slot\n  embed widget.js adapter=\"react\"".into(),
        );
        let mut pixels = [0; 64];
        let mut framebuffer = Rgb565View::new(screen, &mut pixels).unwrap();
        render_template_to_framebuffer(source, &context, screen, &mut framebuffer).unwrap();
    }
}
