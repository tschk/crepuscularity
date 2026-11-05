#![cfg(feature = "std")]

use crepuscularity_core::{parse_template, TemplateContext};
use crepuscularity_embedded::{
    render_nodes_to_document, render_template_to_framebuffer, Rgb565Buffer, ScreenSize, DEFAULT_BG,
};
use std::sync::Arc;

#[test]
fn disabled_presence_is_rejected_instead_of_exposing_an_active_button() {
    let screen = ScreenSize::new(64, 32);
    let mut ctx = TemplateContext::new();
    ctx.set("blocked", true);
    for attribute in [
        "disabled",
        "disabled={true}",
        "disabled={blocked}",
        "disabled=\"false\"",
        "disabled=\"\"",
        "DISABLED={true}",
    ] {
        let source = format!("button #danger {attribute} @click=\"deleteAll\"\n  \"Delete\"");
        let nodes = parse_template(&source).unwrap();
        let error = render_nodes_to_document(&nodes, &ctx, screen).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("disabled buttons are not supported"),
            "{attribute}: {error}"
        );
    }
}

#[test]
fn false_and_null_preserve_button_paint_handler_and_hit() {
    let screen = ScreenSize::new(64, 32);
    let ctx = TemplateContext::new();
    let mut expected = Rgb565Buffer::new(screen, DEFAULT_BG);
    render_template_to_framebuffer(
        "button #danger @click=\"deleteAll\"\n  \"Delete\"",
        &ctx,
        screen,
        &mut expected,
    )
    .unwrap();
    for attribute in ["disabled={false}", "disabled={null}"] {
        let mut actual = Rgb565Buffer::new(screen, DEFAULT_BG);
        let doc = render_template_to_framebuffer(
            &format!("button #danger {attribute} @click=\"deleteAll\"\n  \"Delete\""),
            &ctx,
            screen,
            &mut actual,
        )
        .unwrap();
        assert_eq!(actual.pixels(), expected.pixels());
        assert_eq!(doc.node_at(1, 1), Some("danger"));
        assert_eq!(
            doc.node_by_id("danger").unwrap().on_click.as_deref(),
            Some("deleteAll")
        );
    }
}

#[test]
fn only_active_control_flow_and_expanded_include_slots_are_validated() {
    let screen = ScreenSize::new(64, 32);
    let mut ctx = TemplateContext::new();
    let inactive = parse_template("if false\n  button disabled\nelse\n  span \"Safe\"").unwrap();
    assert!(render_nodes_to_document(&inactive, &ctx, screen).is_ok());
    Arc::make_mut(&mut ctx.virtual_files).insert("child.crepus".into(), "button disabled".into());
    let nodes = parse_template("include child.crepus").unwrap();
    assert!(render_nodes_to_document(&nodes, &ctx, screen)
        .unwrap_err()
        .to_string()
        .contains("disabled buttons"));
    ctx.slot = Some((
        parse_template("button disabled").unwrap(),
        Arc::new(TemplateContext::new()),
    ));
    let nodes = parse_template("slot").unwrap();
    assert!(render_nodes_to_document(&nodes, &ctx, screen)
        .unwrap_err()
        .to_string()
        .contains("disabled buttons"));
}

#[test]
fn invalid_disabled_expression_and_unsupported_state_leave_framebuffer_unchanged() {
    let screen = ScreenSize::new(64, 32);
    for attribute in ["disabled={1 +}", "disabled={true}"] {
        let mut fb = Rgb565Buffer::new(screen, DEFAULT_BG);
        let before = fb.pixels().to_vec();
        let source = format!("span \"First\"\nbutton {attribute}\n  \"Second\"");
        assert!(
            render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut fb)
                .is_err(),
            "{attribute}"
        );
        assert_eq!(fb.pixels(), before);
    }
}
