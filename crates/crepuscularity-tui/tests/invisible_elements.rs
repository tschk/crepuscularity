use crepuscularity_tui::{
    collect_focusable_ids,
    ratatui::{backend::TestBackend, buffer::Buffer, style::Color, Terminal},
    Template,
};

fn draw(template: &Template) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(32, 12)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    terminal.backend().buffer().clone()
}
fn text(buffer: &Buffer) -> String {
    buffer.content.iter().map(|cell| cell.symbol()).collect()
}
fn marker(buffer: &Buffer) -> usize {
    buffer
        .content
        .windows(4)
        .position(|cells| {
            cells
                .iter()
                .map(|cell| cell.symbol())
                .eq(["T", "A", "I", "L"])
        })
        .expect("TAIL must remain visible")
}

#[test]
fn invisible_subtrees_keep_flex_fit_gap_and_scroll_geometry_without_paint() {
    for source in [
        "div flex-col gap-1\n  div h-[3] border bg-red-500 VISIBILITY\n    \"SECRET\"\n  span h-[1]\n    \"TAIL\"",
        "div flex-row gap-2\n  div w-fit VISIBILITY\n    \"SECRET\"\n  span w-[4]\n    \"TAIL\"",
        "div h-fit flex-col gap-1\n  div w-[7] h-fit VISIBILITY\n    \"SECRET wrapped across lines\"\n  span h-[1]\n    \"TAIL\"",
        "div h-[4] overflow-y-scroll scroll-bottom=true flex-col gap-1\n  div h-[3] VISIBILITY\n    \"SECRET\"\n  span h-[1]\n    \"TAIL\"",
    ] {
        let visible = draw(&Template::from_source(source.replace("VISIBILITY", "")));
        for class in ["invisible", "tui:invisible"] {
            let invisible = draw(&Template::from_source(source.replace("VISIBILITY", class)));
            assert_eq!(marker(&invisible), marker(&visible), "{source}: {class}");
            let output=text(&invisible);
            assert!(!output.contains("SECRET") && !output.contains('─') && !output.contains('│'), "{output}");
            assert!(invisible.content.iter().all(|cell| cell.bg == Color::Reset), "background must not paint");
        }
    }
}

#[test]
fn invisible_root_and_slot_preserve_parent_pixels() {
    let blank = draw(&Template::from_source(""));
    for source in [
        "div invisible border bg-red-500\n  \"SECRET\"",
        "slot invisible\n  \"SECRET\"",
    ] {
        assert_eq!(draw(&Template::from_source(source)), blank, "{source}");
    }
    let mut template=Template::from_source("div\n  slot invisible\n    button #fallback focusable\n      \"SECRET\"\n  span h-[1]\n    \"TAIL\"");
    template.refresh_focus();
    assert!(template.focus().focusable_ids.is_empty());
    assert!(!text(&draw(&template)).contains("SECRET"));
}

#[test]
fn invisible_focus_restores_after_dynamic_class_and_include_changes() {
    let mut template = Template::from_source(
        "include child.crepus conceal={conceal}\n  button #caller focusable\n    \"SECRET\"",
    );
    std::sync::Arc::make_mut(&mut template.context_mut().virtual_files).insert(
        "child.crepus".into(),
        "div class:invisible={conceal}\n  button #included focusable\n    \"SECRET\"\n  slot"
            .into(),
    );
    for invisible in [true, false, true, false] {
        template.set("conceal", invisible);
        template.refresh_focus();
        assert_eq!(template.focus().focusable_ids.is_empty(), invisible);
        assert_eq!(text(&draw(&template)).contains("SECRET"), !invisible);
        if !invisible {
            assert_eq!(template.focus().focusable_ids, ["included", "caller"]);
        }
    }
    assert!(
        collect_focusable_ids("div invisible\n  button #child focusable")
            .unwrap()
            .is_empty()
    );
}
