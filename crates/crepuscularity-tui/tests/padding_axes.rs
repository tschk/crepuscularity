use crepuscularity_tui::{parse_classes, Template};
use ratatui::{backend::TestBackend, buffer::Buffer, widgets::Padding, Terminal};

const CASES: &[(&str, [u16; 4])] = &[
    ("px-2 py-1", [2, 2, 1, 1]),
    ("py-1 px-2", [2, 2, 1, 1]),
    ("p-2 px-0", [0, 0, 2, 2]),
    ("p-2 py-0", [2, 2, 0, 0]),
    ("p-2 px-[0]", [0, 0, 2, 2]),
    ("p-2 py-[0]", [2, 2, 0, 0]),
    ("px-2 py-1 px-0", [0, 0, 1, 1]),
    ("py-1 px-2 py-0", [2, 2, 0, 0]),
    ("pt-3 px-2", [2, 2, 3, 0]),
    ("pl-3 py-2", [3, 0, 2, 2]),
    ("px-2 py-1 p-3", [3, 3, 3, 3]),
    ("p-3 px-1 py-2", [1, 1, 2, 2]),
];

#[test]
fn padding_axis_utilities_preserve_the_other_axis() {
    for &(classes, [left, right, top, bottom]) in CASES {
        let classes = classes
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            parse_classes(&classes).padding,
            Padding::new(left, right, top, bottom),
            "{classes:?}"
        );
    }
}

fn render(classes: &str) -> Buffer {
    let template = Template::from_source(format!("div border {classes}\n  \"X\""));
    let mut terminal = Terminal::new(TestBackend::new(20, 12)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    terminal.backend().buffer().clone()
}

#[test]
fn padding_axes_render_like_explicit_sides_with_borders() {
    for &(classes, [left, right, top, bottom]) in CASES {
        let actual = render(classes);
        let explicit = format!("pl-{left} pr-{right} pt-{top} pb-{bottom}");
        assert_eq!(actual, render(&explicit), "{classes} vs {explicit}");
        assert_eq!(actual[(1 + left, 1 + top)].symbol(), "X", "{classes}");
    }
}
