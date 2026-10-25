use crepuscularity_tui::{render_template_to_buffer, TemplateContext};
use ratatui::{buffer::Buffer, layout::Rect};

fn render(source: &str) -> Buffer {
    let area = Rect::new(0, 0, 40, 20);
    let mut buffer = Buffer::empty(area);
    let mut ctx = TemplateContext::new();
    ctx.set("content", "A\n界B");
    render_template_to_buffer(source, &ctx, &mut buffer, area).unwrap();
    buffer
}

#[test]
fn row_height_fit_measures_text_height_instead_of_child_widths() {
    let buffer = render("div\n  div flex-row h-fit\n    div w-[10]\n      \"{content}\"\n    div w-[6]\n      \"Side\"\n  div h-[1]\n    \"After\"");
    assert_eq!(buffer[(0, 1)].symbol(), "界");
    assert_eq!(buffer[(0, 2)].symbol(), "A");
    assert_eq!(buffer[(1, 2)].symbol(), "f");
}

#[test]
fn column_width_fit_measures_text_width_instead_of_child_heights() {
    for classes in ["w-fit", "w-fit h-fit"] {
        let buffer = render(&format!("div flex-row\n  div {classes}\n    div h-[1]\n      \"Longest\"\n    div h-[2]\n      \"界\"\n  div\n    \"After\""));
        assert_eq!(buffer[(0, 0)].symbol(), "L", "{classes}");
        assert_eq!(buffer[(7, 0)].symbol(), "A", "{classes}");
        assert_eq!(buffer[(8, 0)].symbol(), "f", "{classes}");
    }
}

#[test]
fn nested_row_width_fit_includes_gap_border_and_padding() {
    let buffer = render("div flex-row\n  div flex-row w-fit border p-1 gap-[1]\n    div\n      \"Hello\"\n    div\n      \"界\"\n  div\n    \"After\"");
    assert_eq!(buffer[(2, 2)].symbol(), "H");
    assert_eq!(buffer[(8, 2)].symbol(), "界");
    assert_eq!(buffer[(12, 0)].symbol(), "A");
    assert_eq!(buffer[(13, 0)].symbol(), "f");
}

#[test]
fn cross_axis_fit_preserves_a_containers_explicit_height() {
    let buffer = render(
        "div\n  div w-fit h-[1]\n    div\n      \"Long content\"\n  div h-[1]\n    \"After\"",
    );
    assert_eq!(buffer[(0, 1)].symbol(), "A");
    assert_eq!(buffer[(1, 1)].symbol(), "f");
}

#[test]
fn empty_fitted_boxes_and_titles_keep_their_overhead() {
    for (source, next_row) in [
        ("div\n  div h-fit border p-1\n  div h-[1]\n    \"After\"", 4),
        (
            "div\n  div h-fit title=\"Box\"\n  div h-[1]\n    \"After\"",
            1,
        ),
        (
            "div\n  div h-fit title=\"Box\"\n    \"Text\"\n  div h-[1]\n    \"After\"",
            2,
        ),
        (
            "div\n  div h-fit title=\"Box\"\n    div\n      \"Text\"\n  div h-[1]\n    \"After\"",
            2,
        ),
    ] {
        let buffer = render(source);
        assert_eq!(buffer[(0, next_row)].symbol(), "A", "{source}");
        assert_eq!(buffer[(1, next_row)].symbol(), "f", "{source}");
    }
}
