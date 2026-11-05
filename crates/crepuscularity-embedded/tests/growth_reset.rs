use crepuscularity_embedded::{style_from_classes, SizeHint};

#[test]
fn growth_reset_respects_order_and_preserves_explicit_sizes() {
    for reset in ["grow-0", "flex-none"] {
        for growth in ["grow", "flex-auto", "flex-1"] {
            let style = style_from_classes(&[growth.into(), reset.into()]);
            assert_eq!(style.width, SizeHint::Auto, "{growth} {reset}");
            assert_eq!(style.height, SizeHint::Auto, "{growth} {reset}");
            let reversed = style_from_classes(&[reset.into(), growth.into()]);
            assert_eq!(reversed.width, SizeHint::Flex1, "{reset} {growth}");
        }
        let fixed = style_from_classes(&[
            "flex-1".into(),
            "w-[20px]".into(),
            "h-[10px]".into(),
            reset.into(),
        ]);
        assert_eq!(fixed.width, SizeHint::Fixed(20));
        assert_eq!(fixed.height, SizeHint::Fixed(10));
        let mixed = style_from_classes(&["flex-1".into(), "w-[20px]".into(), reset.into()]);
        assert_eq!(mixed.width, SizeHint::Fixed(20));
        assert_eq!(mixed.height, SizeHint::Auto);
        let fractional = style_from_classes(&["w-1/2".into(), "h-1/2".into(), reset.into()]);
        assert_eq!(fractional.width, SizeHint::Fraction { num: 1, den: 2 });
        assert_eq!(fractional.height, SizeHint::Fraction { num: 1, den: 2 });
        let full = style_from_classes(&["w-full".into(), "h-full".into(), reset.into()]);
        assert_eq!(full.width, SizeHint::Fill);
        assert_eq!(full.height, SizeHint::Fill);
    }
}

#[cfg(feature = "std")]
#[test]
fn resetting_growth_restores_non_growing_layout_bounds() {
    use crepuscularity_embedded::{
        render_template_to_framebuffer, Color, Rgb565Buffer, ScreenSize, TemplateContext,
    };
    let screen = ScreenSize::new(100, 100);
    for direction in ["flex-row", "flex-col"] {
        let bounds = |classes: &str| {
            let source = format!(
                "div {direction}\n  div #item {classes} bg-[#ffffff]\n  div w-[20px] h-[20px]"
            );
            let mut buffer = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
            let document = render_template_to_framebuffer(
                &source,
                &TemplateContext::new(),
                screen,
                &mut buffer,
            )
            .unwrap();
            (
                document.node_by_id("item").unwrap().bounds,
                buffer.pixels().to_vec(),
                document.hit_test(40, 40).map(str::to_owned),
            )
        };
        let reference = bounds("");
        for reset in ["grow-0", "flex-none"] {
            assert_eq!(
                bounds(&format!("flex-1 {reset}")),
                reference,
                "{direction} {reset}"
            );
            assert_ne!(
                bounds(&format!("{reset} flex-1")),
                reference,
                "{direction} reverse {reset}"
            );
        }
    }
}
