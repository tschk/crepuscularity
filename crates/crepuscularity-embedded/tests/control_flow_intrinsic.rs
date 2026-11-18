#![cfg(feature = "std")]
use crepuscularity_embedded::{
    render_template_to_framebuffer, Color, EmbeddedDocument, Rgb565Buffer, ScreenSize,
    TemplateContext, TemplateValue,
};

fn render(source: &str, ctx: &TemplateContext) -> (EmbeddedDocument, Vec<u16>) {
    let screen = ScreenSize::new(96, 96);
    let mut buffer = Rgb565Buffer::new(screen, Color::rgb(0, 0, 0));
    let document = render_template_to_framebuffer(source, ctx, screen, &mut buffer).unwrap();
    (document, buffer.pixels().to_vec())
}
fn indent(source: &str, spaces: usize) -> String {
    source
        .lines()
        .map(|line| format!("{}{line}", " ".repeat(spaces)))
        .collect::<Vec<_>>()
        .join("\n")
}
fn rows() -> TemplateContext {
    let mut ctx = TemplateContext::new();
    let items = ["one", "two", "third"]
        .into_iter()
        .map(|id| {
            let mut item = TemplateContext::new();
            item.set("id", id);
            item
        })
        .collect();
    ctx.set("items", TemplateValue::List(items));
    ctx
}
const CHILDREN:&str="div #one w-[8px] h-[8px] bg-[#ff0000]\ndiv #two w-[8px] h-[8px] bg-[#ff0000]\ndiv #third w-[8px] h-[8px] bg-[#ff0000]";
const AFTER: &str = "div #after w-[8px] h-[8px] bg-[#0000ff]";

#[test]
fn if_for_and_match_contents_do_not_overlap_the_following_sibling() {
    let ctx = rows();
    let controls = [
        format!("if true\n{}", indent(CHILDREN, 2)),
        "for item in items\n  div id={item.id} w-[8px] h-[8px] bg-[#ff0000]".into(),
        format!("match {{1}}\n  1 =>\n{}", indent(CHILDREN, 4)),
    ];
    let (_, reference) = render(
        &format!(
            "div bg-[#000000]\n{}\n{}",
            indent(CHILDREN, 2),
            indent(AFTER, 2)
        ),
        &ctx,
    );
    for control in controls {
        let source = format!(
            "div bg-[#000000]\n{}\n{}",
            indent(&control, 2),
            indent(AFTER, 2)
        );
        let (document, pixels) = render(&source, &ctx);
        let third = document.node_by_id("third").unwrap().bounds;
        let after = document.node_by_id("after").unwrap().bounds;
        assert!(
            after.y >= third.y + third.h,
            "{source}: third={third:?}, after={after:?}"
        );
        assert_eq!(pixels, reference, "{source}");
        assert_eq!(
            pixels[third.y as usize * 96],
            Color::rgb(255, 0, 0).to_rgb565().0
        );
        assert_eq!(document.hit_test(0, third.y), Some("third"));
        assert_eq!(document.hit_test(0, after.y), Some("after"));
    }
}

#[test]
fn nested_control_wrappers_measure_descendant_extent() {
    let source = format!(
        "div bg-[#000000]\n  if true\n    if true\n{}\n{}",
        indent(CHILDREN, 6),
        indent(AFTER, 2)
    );
    let (document, pixels) = render(&source, &TemplateContext::new());
    let (_, reference) = render(
        &format!(
            "div bg-[#000000]\n{}\n{}",
            indent(CHILDREN, 2),
            indent(AFTER, 2)
        ),
        &TemplateContext::new(),
    );
    let third = document.node_by_id("third").unwrap().bounds;
    let after = document.node_by_id("after").unwrap().bounds;
    assert!(after.y >= third.y + third.h);
    assert_eq!(pixels, reference);
    assert_eq!(document.hit_test(0, third.y), Some("third"));
}

#[test]
fn control_wrapper_cross_extent_can_exceed_the_existing_row_minimum() {
    let source="div flex-row bg-[#000000]\n  if true\n    div #wide w-[48px] h-[8px] bg-[#ff0000]\n  div #after w-[8px] h-[8px] bg-[#0000ff]";
    let (document, pixels) = render(source, &TemplateContext::new());
    let wide = document.node_by_id("wide").unwrap().bounds;
    let after = document.node_by_id("after").unwrap().bounds;
    assert_eq!(wide.w, 48);
    assert!(after.x >= wide.x + wide.w);
    assert_eq!(pixels[40], Color::rgb(255, 0, 0).to_rgb565().0);
    assert_eq!(document.hit_test(40, 0), Some("wide"));
    assert_eq!(document.hit_test(after.x, 0), Some("after"));
}

#[test]
fn small_control_wrappers_keep_the_existing_auto_minimum() {
    for (direction, expected) in [("flex-col", (0, 16)), ("flex-row", (32, 0))] {
        let source = format!(
            "div {direction}\n  if true\n    div w-[8px] h-[8px]\n  div #after w-[8px] h-[8px]"
        );
        let (document, _) = render(&source, &TemplateContext::new());
        let after = document.node_by_id("after").unwrap().bounds;
        assert_eq!((after.x, after.y), expected, "{direction}");
    }
}

#[test]
fn intrinsic_sizes_include_insets_and_only_visible_child_gaps() {
    let source = "div\n  div #box flex-row p-[2px] gap-[3px]\n    div #left w-[10px] h-[20px]\n    div hidden w-[80px] h-[80px]\n    div #right invisible w-[14px] h-[8px]\n  div #after h-[8px]";
    let (document, _) = render(source, &TemplateContext::new());
    assert_eq!(document.node_by_id("box").unwrap().bounds.h, 24);
    assert_eq!(document.node_by_id("right").unwrap().bounds.x, 15);
    assert_eq!(document.node_by_id("after").unwrap().bounds.y, 24);
}

#[test]
fn relative_and_flexible_children_resolve_within_auto_minimum() {
    for (class, height) in [("flex-1", 16), ("h-full", 16), ("h-1/2", 8)] {
        let source = format!("div\n  div #box\n    div #child {class}\n  div #after h-[8px]");
        let (document, _) = render(&source, &TemplateContext::new());
        assert_eq!(document.node_by_id("box").unwrap().bounds.h, 16, "{class}");
        assert_eq!(
            document.node_by_id("child").unwrap().bounds.h,
            height,
            "{class}"
        );
        assert_eq!(
            document.node_by_id("after").unwrap().bounds.y,
            16,
            "{class}"
        );
    }
}
