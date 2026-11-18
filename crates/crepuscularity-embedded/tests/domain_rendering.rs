#![cfg(feature = "std")]

use crepuscularity_embedded::{
    layout_tree, paint::paint_document, Color, EmbeddedDocument, EmbeddedNode, EmbeddedStyle,
    FlexDir, Justify, Rect, Rgb565Buffer, ScreenSize, SizeHint, Template, DEFAULT_BG,
};

fn node(text: Option<&str>) -> EmbeddedNode {
    EmbeddedNode {
        id: None,
        tag: "div".into(),
        text: text.map(str::to_owned),
        on_click: None,
        style: EmbeddedStyle::default(),
        bounds: Rect::default(),
        children: Vec::new(),
    }
}

#[test]
fn row_text_uses_the_correct_axes_and_respects_explicit_allocations() {
    let mut root = node(None);
    root.style.flex_dir = FlexDir::Row;
    root.children = vec![node(Some("ABCD")), node(Some("XY"))];
    layout_tree(&mut root, ScreenSize::new(100, 40));
    assert_eq!(root.children[0].bounds, Rect::new(0, 0, 32, 8));
    assert_eq!(root.children[1].bounds, Rect::new(32, 0, 16, 8));
    root.children[0].style.width = SizeHint::Fixed(45);
    root.children[0].style.height = SizeHint::Fixed(17);
    layout_tree(&mut root, ScreenSize::new(100, 40));
    assert_eq!(root.children[0].bounds, Rect::new(0, 0, 45, 17));
    assert_eq!(root.children[1].bounds.x, 45);
    root.style.width = SizeHint::Fixed(60);
    root.style.height = SizeHint::Fixed(30);
    layout_tree(&mut root, ScreenSize::new(100, 40));
    assert_eq!(root.bounds, Rect::new(0, 0, 60, 30));
}

#[test]
fn intrinsic_sizes_reset_and_hidden_children_do_not_take_allocations() {
    let mut root = node(None);
    let mut leaf = node(None);
    leaf.style.height = SizeHint::Fixed(8);
    let mut wrapper = node(None);
    wrapper.children = vec![leaf.clone(), leaf.clone(), leaf.clone()];
    root.children = vec![wrapper, leaf];
    layout_tree(&mut root, ScreenSize::new(100, 100));
    assert_eq!(root.children[1].bounds.y, 24);
    for child in &mut root.children[0].children {
        child.style.height = SizeHint::Fixed(4);
    }
    layout_tree(&mut root, ScreenSize::new(100, 100));
    assert_eq!(root.children[1].bounds.y, 16);
    root.children[0].style.hidden = true;
    layout_tree(&mut root, ScreenSize::new(100, 100));
    assert_eq!(root.children[0].bounds, Rect::default());
    assert_eq!(root.children[1].bounds.y, 0);
    root.style.hidden = true;
    layout_tree(&mut root, ScreenSize::new(100, 100));
    assert_eq!(root.bounds, Rect::default());
}

#[test]
fn flex_remainders_and_zero_allocations_stay_inside_the_parent() {
    let mut root = node(None);
    root.style.flex_dir = FlexDir::Row;
    let mut flexible = node(Some("X"));
    flexible.style.width = SizeHint::Flex1;
    root.children = vec![flexible.clone(), flexible.clone(), flexible];
    layout_tree(&mut root, ScreenSize::new(10, 8));
    assert_eq!(
        root.children.iter().map(|n| n.bounds.w).collect::<Vec<_>>(),
        [4, 3, 3]
    );
    assert_eq!(root.children[2].bounds.x, 7);
    layout_tree(&mut root, ScreenSize::new(1, 8));
    assert_eq!(
        root.children.iter().map(|n| n.bounds.w).collect::<Vec<_>>(),
        [1, 0, 0]
    );
}

#[test]
fn justification_handles_both_axes_and_large_overflowing_totals() {
    for direction in [FlexDir::Row, FlexDir::Column] {
        let mut root = node(None);
        root.style.flex_dir = direction;
        let mut child = node(None);
        child.style.width = SizeHint::Fixed(10);
        child.style.height = SizeHint::Fixed(10);
        root.children = vec![child.clone(), child];
        for (justify, offsets) in [(Justify::Around, [20, 70]), (Justify::Evenly, [26, 62])] {
            root.style.justify = justify;
            layout_tree(&mut root, ScreenSize::new(100, 100));
            assert_eq!(
                root.children
                    .iter()
                    .map(|n| if direction == FlexDir::Row {
                        n.bounds.x
                    } else {
                        n.bounds.y
                    })
                    .collect::<Vec<_>>(),
                offsets
            );
        }
        root.style.justify = Justify::End;
        for child in &mut root.children {
            child.style.width = SizeHint::Fixed(40000);
            child.style.height = SizeHint::Fixed(40000);
        }
        layout_tree(&mut root, ScreenSize::new(u16::MAX, u16::MAX));
        assert_eq!(root.children[0].bounds.x, 0);
        assert_eq!(root.children[0].bounds.y, 0);
    }
}

#[test]
fn clipped_borders_do_not_reappear_on_the_framebuffer_edge() {
    let mut root = node(None);
    root.bounds = Rect::new(0, 0, 5, 5);
    root.style.bg = Some(Color::rgb(0, 0, 0));
    root.style.border_width = 1;
    root.style.border_color = Some(Color::rgb(255, 255, 255));
    let doc = EmbeddedDocument::new(vec![root.clone()], ScreenSize::new(3, 3));
    let mut fb = Rgb565Buffer::new(doc.screen, DEFAULT_BG);
    paint_document(&mut fb, &doc);
    assert_eq!(fb.pixels(), [65535, 65535, 65535, 65535, 0, 0, 65535, 0, 0]);
    root.bounds = Rect::new(1, 1, 0, 2);
    paint_document(&mut fb, &EmbeddedDocument::new(vec![root], doc.screen));
    assert!(fb.pixels().iter().all(|p| *p == 0));
}

#[test]
fn hidden_roots_do_not_supply_background_and_text_inherits_color() {
    let mut hidden = node(None);
    hidden.style.hidden = true;
    hidden.style.bg = Some(Color::rgb(255, 0, 0));
    let mut root = node(None);
    root.style.text = Some(Color::rgb(0, 255, 0));
    root.children = vec![node(Some("K"))];
    layout_tree(&mut root, ScreenSize::new(8, 8));
    let mut fb = Rgb565Buffer::new(ScreenSize::new(8, 8), DEFAULT_BG);
    paint_document(
        &mut fb,
        &EmbeddedDocument::new(vec![hidden, root.clone()], fb_size()),
    );
    assert!(fb.pixels().contains(&0x07e0));
    assert!(fb
        .pixels()
        .iter()
        .all(|p| *p == 0x07e0 || *p == DEFAULT_BG.to_rgb565().0));
    root.style.opacity_percent = Some(0);
    paint_document(&mut fb, &EmbeddedDocument::new(vec![root], fb_size()));
    assert!(fb.pixels().iter().all(|p| *p == DEFAULT_BG.to_rgb565().0));
}

fn fb_size() -> ScreenSize {
    ScreenSize::new(8, 8)
}

#[test]
fn template_filename_selects_frontend_and_component_defaults_are_applied() {
    let mut fb = Rgb565Buffer::new(ScreenSize::new(80, 20), DEFAULT_BG);
    let mut template = Template::from_source_with_path(
        "<template><span id=\"result\">Vue</span></template>",
        "view.vue",
        fb_size(),
    );
    assert!(template
        .draw(&mut fb)
        .unwrap()
        .node_by_id("result")
        .is_some());
    let mut template = Template::from_source(
        "+++\n[Card.defaults]\ntitle = 'DEFAULT'\n+++\n--- Card\nspan #result \"{title}\"",
        ScreenSize::new(80, 20),
    );
    template.set_component("Card");
    assert_eq!(
        template
            .draw(&mut fb)
            .unwrap()
            .node_by_id("result")
            .unwrap()
            .text
            .as_deref(),
        Some("DEFAULT")
    );
    template.set("title", "OVERRIDE");
    assert_eq!(
        template
            .draw(&mut fb)
            .unwrap()
            .node_by_id("result")
            .unwrap()
            .text
            .as_deref(),
        Some("OVERRIDE")
    );
}

#[test]
fn panel_byte_extraction_tracks_pixel_updates_in_both_orders() {
    use crepuscularity_embedded::{Framebuffer, PanelConfig, Rgb565ByteOrder, Ui};
    for order in [Rgb565ByteOrder::Rgb, Rgb565ByteOrder::Bgr] {
        let mut ui = Ui::new(2, 1, "").panel(PanelConfig {
            byte_order: order,
            ..PanelConfig::default()
        });
        for color in [Color::rgb(255, 0, 0), Color::rgb(0, 0, 255)] {
            ui.buffer_mut().fill(color);
            let mut pixel = color.to_rgb565().0.to_ne_bytes();
            if order == Rgb565ByteOrder::Bgr {
                pixel.reverse();
            }
            let expected = [pixel[0], pixel[1], pixel[0], pixel[1]];
            assert_eq!(ui.rgb565_for_panel(), expected);
            assert_eq!(ui.rgb565_for_panel(), expected);
        }
    }
}
