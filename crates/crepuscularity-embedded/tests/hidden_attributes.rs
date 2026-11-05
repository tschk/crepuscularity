#![cfg(feature = "std")]

use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, Rgb565Buffer, ScreenSize, TemplateContext,
};

#[test]
fn hidden_attributes_control_layout_paint_and_hits() {
    for (attribute, hidden) in [
        ("hidden", true),
        ("hidden=''", true),
        ("hidden='false'", true),
        ("hidden='until-found'", true),
        ("hidden={true}", true),
        ("hidden={0}", true),
        ("hidden={false}", false),
        ("hidden={null}", false),
        ("HIDDEN={true}", true),
        ("class='hidden' hidden={false}", true),
    ] {
        let source = format!("div flex-row bg-[#000000]\n  div #secret {attribute} w-[8px] h-full bg-[#ffffff]\n    div #nested w-full h-full bg-[#ffffff]\n  div #visible w-[8px] h-full bg-[#ffffff]");
        let screen = ScreenSize::new(16, 8);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(255, 0, 0));
        let doc = render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut fb)
            .unwrap();
        assert_eq!(
            doc.node_by_id("secret").unwrap().style.hidden,
            hidden,
            "{attribute}"
        );
        assert_eq!(
            doc.node_by_id("visible").unwrap().bounds.x,
            if hidden { 0 } else { 8 },
            "{attribute}"
        );
        assert_eq!(
            doc.hit_test(0, 0),
            Some(if hidden { "visible" } else { "nested" }),
            "{attribute}"
        );
        for row in fb.pixels().chunks(16) {
            assert!(row[..8].iter().all(|p| *p == u16::MAX));
            assert!(
                row[8..]
                    .iter()
                    .all(|p| *p == if hidden { 0 } else { u16::MAX }),
                "{attribute}"
            );
        }
    }
}

#[test]
fn changing_hidden_binding_updates_every_element_path() {
    for tag in ["div", "span", "button", "slot"] {
        let source = format!("{tag} #target hidden={{hide}} bg-[#ffffff]\n  \"A\"");
        let screen = ScreenSize::new(8, 8);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let mut ctx = TemplateContext::new();
        for hide in [true, false, true] {
            ctx.set("hide", hide);
            let doc = render_template_to_framebuffer(&source, &ctx, screen, &mut fb).unwrap();
            assert_eq!(
                doc.node_by_id("target").unwrap().style.hidden,
                hide,
                "{tag}"
            );
            assert_eq!(
                doc.hit_test(0, 0),
                if hide { None } else { Some("target") },
                "{tag}"
            );
        }
    }
}

#[test]
fn invalid_hidden_bindings_fail_before_framebuffer_painting() {
    let screen = ScreenSize::new(8, 8);
    let mut fb = Rgb565Buffer::new(screen, Color::rgb(255, 0, 0));
    let before = fb.pixels().to_vec();
    assert!(render_template_to_framebuffer(
        "div hidden={1 +}",
        &TemplateContext::new(),
        screen,
        &mut fb
    )
    .is_err());
    assert_eq!(fb.pixels(), before);
}

#[test]
fn button_labels_skip_hidden_spans() {
    let screen = ScreenSize::new(32, 8);
    for attribute in ["hidden", "class='hidden'", "hidden={true}"] {
        let source =
            format!("button #target\n  span {attribute}\n    \"secret\"\n  span\n    \"OK\"");
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let doc = render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut fb)
            .unwrap();
        assert_eq!(
            doc.node_by_id("target").unwrap().text.as_deref(),
            Some("OK"),
            "{attribute}"
        );
    }
}

#[test]
fn every_frontend_preserves_hidden_attributes() {
    for (filename, source) in [
        ("view.crepus", "div #target hidden bg-[#ffffff]"),
        (
            "view.jsx",
            "<div id=\"target\" hidden className=\"bg-[#ffffff]\" />",
        ),
        (
            "view.vue",
            "<template><div id=\"target\" hidden class=\"bg-[#ffffff]\"></div></template>",
        ),
        (
            "view.svelte",
            "<div id=\"target\" hidden class=\"bg-[#ffffff]\"></div>",
        ),
        (
            "view.astro",
            "<div id=\"target\" hidden class=\"bg-[#ffffff]\"></div>",
        ),
        (
            "view.ng.html",
            "<div id=\"target\" hidden class=\"bg-[#ffffff]\"></div>",
        ),
    ] {
        let nodes = crepuscularity_core::parse_template_with_path(
            source,
            Some(std::path::Path::new(filename)),
        )
        .unwrap();
        let screen = ScreenSize::new(8, 8);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let doc = crepuscularity_embedded::render_parsed_nodes_to_framebuffer(
            &nodes,
            &TemplateContext::new(),
            screen,
            &mut fb,
        )
        .unwrap();
        assert!(doc.node_by_id("target").unwrap().style.hidden, "{filename}");
        assert_eq!(doc.hit_test(0, 0), None, "{filename}");
    }
}
