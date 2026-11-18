#![cfg(feature = "std")]
use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, Rgb565Buffer, ScreenSize, TemplateContext,
};

#[test]
fn empty_control_flow_does_not_reserve_space_or_gaps() {
    for body in [
        "  if false\n    span\n      \"no\"\n",
        "  for item in items\n    span\n      \"no\"\n",
        "  match {1}\n    2 =>\n      span\n        \"no\"\n",
        "  if true\n    $: let unused = 1\n",
        "  if true\n    if false\n      span\n        \"no\"\n",
        "  if true\n    span class='hidden'\n      \"no\"\n",
    ] {
        for direction in ["flex-col", "flex-row"] {
            let source = format!("div {direction} gap-[3px] bg-[#000000]\n{body}  span #visible text-[#ffffff]\n    \"OK\"");
            let screen = ScreenSize::new(32, 32);
            let mut actual = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
            let mut ctx = TemplateContext::new();
            ctx.set(
                "items",
                crepuscularity_embedded::TemplateValue::List(vec![]),
            );
            let doc = render_template_to_framebuffer(&source, &ctx, screen, &mut actual).unwrap();
            let visible = doc.node_by_id("visible").unwrap();
            assert_eq!(
                (visible.bounds.x, visible.bounds.y),
                (0, 0),
                "{direction}: {body}"
            );
            let mut expected = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
            render_template_to_framebuffer(
                "div bg-[#000000]\n  span text-[#ffffff]\n    \"OK\"",
                &TemplateContext::new(),
                screen,
                &mut expected,
            )
            .unwrap();
            assert_eq!(actual.pixels(), expected.pixels(), "{body}");
            assert_eq!(doc.hit_test(0, 0), Some("visible"));
            assert!(doc.root[0].children[0].style.hidden);
        }
    }
}

#[test]
fn nonempty_and_invisible_control_flow_keeps_its_layout_space() {
    for classes in ["", "invisible", "opacity-0"] {
        let source =
            format!("div\n  if true\n    span {classes}\n      \"K\"\n  span #after\n    \"L\"");
        let screen = ScreenSize::new(32, 32);
        let mut fb = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
        let doc = render_template_to_framebuffer(&source, &TemplateContext::new(), screen, &mut fb)
            .unwrap();
        assert!(!doc.root[0].children[0].style.hidden);
        assert_eq!(doc.node_by_id("after").unwrap().bounds.y, 16);
    }
}
