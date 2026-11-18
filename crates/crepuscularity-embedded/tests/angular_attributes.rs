#![cfg(feature = "std")]
use crepuscularity_core::{parse_template_with_path, TemplateValue};
use crepuscularity_embedded::{
    render_parsed_nodes_to_framebuffer, Color, Rgb565Buffer, ScreenSize, TemplateContext,
};
use std::path::Path;

#[test]
fn case_insensitive_attribute_ids_reach_pixels_and_hits() {
    let screen = ScreenSize::new(32, 16);
    let nodes = parse_template_with_path(
        "<button [attr.ID]='name' [attr.hidden]='null' [disabled]='false' class='bg-[#ffffff]'>Go</button>",
        Some(Path::new("a.ng")),
    ).unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("name", "last");
    let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
    let doc = render_parsed_nodes_to_framebuffer(&nodes, &ctx, screen, &mut fb).unwrap();
    assert!(doc.node_by_id("last").is_some());
    assert_eq!(doc.hit_test(0, 0), Some("last"));
    assert!(fb.pixels().contains(&u16::MAX));
}

#[test]
fn angular_attribute_presence_and_null_ids_remain_distinct() {
    let nodes = parse_template_with_path(
        "<div [attr.id]='name' [attr.hidden]='state'><span>Visible</span></div>",
        Some(Path::new("a.ng")),
    )
    .unwrap();
    let screen = ScreenSize::new(64, 16);
    let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
    let mut ctx = TemplateContext::new();
    ctx.set("name", TemplateValue::Null);
    ctx.set("state", false);
    let doc = render_parsed_nodes_to_framebuffer(&nodes, &ctx, screen, &mut fb).unwrap();
    assert!(doc.root[0].id.is_none());
    assert!(doc.root[0].style.hidden);
    ctx.set("state", TemplateValue::Null);
    ctx.set("name", "{missing}");
    let doc = render_parsed_nodes_to_framebuffer(&nodes, &ctx, screen, &mut fb).unwrap();
    assert!(!doc.root[0].style.hidden);
    assert!(doc.node_by_id("{missing}").is_some());
}

#[test]
fn angular_literal_attribute_classes_and_disabled_presence() {
    let nodes =
        parse_template_with_path("<div [attr.class]='classes'/>", Some(Path::new("a.ng"))).unwrap();
    let screen = ScreenSize::new(8, 8);
    let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
    let mut ctx = TemplateContext::new();
    ctx.set("classes", "bg-[#ff0000]");
    render_parsed_nodes_to_framebuffer(&nodes, &ctx, screen, &mut fb).unwrap();
    assert!(fb
        .pixels()
        .iter()
        .all(|p| *p == Color::rgb(255, 0, 0).to_rgb565().0));
    let nodes = parse_template_with_path(
        "<button [attr.disabled]='state'>Go</button>",
        Some(Path::new("a.ng")),
    )
    .unwrap();
    ctx.set("state", false);
    assert!(
        render_parsed_nodes_to_framebuffer(&nodes, &ctx, screen, &mut fb)
            .unwrap_err()
            .to_string()
            .contains("disabled buttons")
    );
    ctx.set("state", TemplateValue::Null);
    assert!(render_parsed_nodes_to_framebuffer(&nodes, &ctx, screen, &mut fb).is_ok());
}
