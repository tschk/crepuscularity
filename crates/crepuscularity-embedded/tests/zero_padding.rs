use crepuscularity_embedded::{style_from_classes, EmbeddedStyle};

#[cfg(feature = "std")]
use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, Rgb565Buffer, ScreenSize, TemplateContext, DEFAULT_BG,
};

fn style(classes: &str) -> EmbeddedStyle {
    style_from_classes(
        &classes
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>(),
    )
}

fn edges(style: &EmbeddedStyle) -> [u16; 4] {
    [
        style.pad_left(),
        style.pad_right(),
        style.pad_top(),
        style.pad_bottom(),
    ]
}

#[test]
fn explicit_zero_padding_keeps_side_axis_and_shorthand_precedence() {
    for (classes, expected) in [
        ("p-4 pl-0", [0, 16, 16, 16]),
        ("p-4 pr-0", [16, 0, 16, 16]),
        ("p-4 pt-0", [16, 16, 0, 16]),
        ("p-4 pb-0", [16, 16, 16, 0]),
        ("p-4 px-0", [0, 0, 16, 16]),
        ("p-4 py-0", [16, 16, 0, 0]),
        ("px-4 pl-0", [0, 16, 0, 0]),
        ("py-4 pt-0", [0, 0, 0, 16]),
        ("p-4 px-[0px]", [0, 0, 16, 16]),
        ("p-4 pl-[0px]", [0, 16, 16, 16]),
        ("pl-0 p-4", [0, 16, 16, 16]),
        ("px-0 p-4", [0, 0, 16, 16]),
        ("p-4 pl-0 pl-2", [8, 16, 16, 16]),
        ("p-4 pl-2 pl-0", [0, 16, 16, 16]),
        ("p-4 px-0 px-2", [8, 8, 16, 16]),
        ("p-4 px-2 px-0", [0, 0, 16, 16]),
        ("p-4 pl-0 p-2", [0, 8, 8, 8]),
        ("p-4 px-0 pl-2", [8, 0, 16, 16]),
        ("p-4 hover:pl-0 sm:px-0", [16, 16, 16, 16]),
        ("p-4 pl-nope px-nope", [16, 16, 16, 16]),
    ] {
        assert_eq!(edges(&style(classes)), expected, "{classes}");
    }
}

#[test]
fn existing_nonzero_fields_manual_styles_and_full_range_remain_supported() {
    let nonzero = style("p-4 px-2 pl-1");
    assert_eq!(
        (nonzero.padding, nonzero.padding_x, nonzero.padding_l),
        (16, 8, 4)
    );
    assert_eq!(edges(&nonzero), [4, 8, 16, 16]);
    let manual = EmbeddedStyle {
        padding: 9,
        padding_x: 2,
        padding_t: 3,
        ..EmbeddedStyle::default()
    };
    assert_eq!(edges(&manual), [2, 2, 3, 9]);
    let max = style("p-[65535px] pl-0");
    assert_eq!(edges(&max), [0, u16::MAX, u16::MAX, u16::MAX]);
    assert_eq!(edges(&style("p-0")), [0; 4]);
}

#[cfg(feature = "std")]
#[test]
fn zero_padding_matches_explicit_edges_in_child_origin_and_pixels() {
    for (classes, explicit, origin) in [
        ("p-[4px] pl-0", "pl-0 pr-[4px] pt-[4px] pb-[4px]", (0, 4)),
        ("p-[4px] px-[0px]", "pl-0 pr-0 pt-[4px] pb-[4px]", (0, 4)),
        ("p-[4px] py-0", "pl-[4px] pr-[4px] pt-0 pb-0", (4, 0)),
    ] {
        for border in [0, 2] {
            let size = ScreenSize::new(40, 32);
            let mut actual = Rgb565Buffer::new(size, DEFAULT_BG);
            let mut expected = Rgb565Buffer::new(size, DEFAULT_BG);
            let source = |padding| {
                format!("div border-{border} border-[#00ff00] bg-[#000000] {padding}\n  span #label text-[#ffffff] \"K\"")
            };
            let document = render_template_to_framebuffer(
                &source(classes),
                &TemplateContext::new(),
                size,
                &mut actual,
            )
            .unwrap();
            let reference = render_template_to_framebuffer(
                &source(explicit),
                &TemplateContext::new(),
                size,
                &mut expected,
            )
            .unwrap();
            let bounds = document.node_by_id("label").unwrap().bounds;
            assert_eq!(
                (bounds.x, bounds.y),
                (origin.0 + border, origin.1 + border),
                "{classes}, border {border}"
            );
            assert_eq!(bounds, reference.node_by_id("label").unwrap().bounds);
            assert_eq!(
                actual.pixels(),
                expected.pixels(),
                "{classes}, border {border}"
            );
            assert!(actual
                .pixels()
                .contains(&Color::rgb(255, 255, 255).to_rgb565().0));
        }
    }
}
