use crepuscularity_tui::Template;
use ratatui::{backend::TestBackend, Terminal};
use std::sync::Arc;

fn check(source: &str, files: &[(&str, &str)], expected: &[&str], ids: &[&str]) {
    for virtual_files in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut template = Template::from_source_with_path(source, dir.path().join("entry.crepus"));
        template.set("prefix", "CALLER");
        for (name, content) in files {
            let path = dir.path().join(name);
            if virtual_files {
                Arc::make_mut(&mut template.context_mut().virtual_files)
                    .insert((*name).into(), (*content).into());
            } else {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, content).unwrap();
            }
        }
        template.refresh_focus();
        assert_eq!(
            template.focus().focusable_ids,
            ids,
            "virtual={virtual_files}"
        );
        let mut terminal = Terminal::new(TestBackend::new(32, 10)).unwrap();
        terminal
            .draw(|frame| template.draw_full(frame).unwrap())
            .unwrap();
        let rows: Vec<String> = terminal
            .backend()
            .buffer()
            .content
            .chunks(32)
            .map(|row| {
                row.iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .filter(|row| !row.is_empty())
            .collect();
        assert_eq!(rows, expected, "virtual={virtual_files}");
        assert_eq!(template.context().get_str("prefix"), "CALLER");
    }
}

#[test]
fn forwarded_named_slots_keep_caller_scope_and_relative_includes() {
    check(
        "include outer.crepus prefix=\"CALLEE\"\n  span #caller focusable h-[1]\n    \"{prefix}\"\n  include tail.crepus",
        &[
            ("outer.crepus", "span #outer focusable h-[1]\n  \"{prefix}\"\ninclude parts/card.crepus#Card\n  slot"),
            ("parts/card.crepus", "--- Card\n$: default name = \"inner\"\nspan id={name} focusable h-[1]\n  \"INNER\"\ninclude label.crepus\nslot"),
            ("parts/label.crepus", "span #child focusable h-[1]\n  \"CHILD\""),
            ("tail.crepus", "span #tail focusable h-[1]\n  \"TAIL\""),
            ("parts/tail.crepus", "span #wrong focusable h-[1]\n  \"WRONG\""),
        ],
        &["CALLEE", "INNER", "CHILD", "CALLER", "TAIL"],
        &["outer", "inner", "child", "caller", "tail"],
    );
}

#[test]
fn unsupplied_include_uses_its_fallback_without_inheriting_the_outer_slot() {
    check(
        "include outer.crepus\n  span #caller focusable h-[1]\n    \"{prefix}\"",
        &[
            ("outer.crepus", "include child.crepus\nslot"),
            (
                "child.crepus",
                "slot\n  span #fallback focusable h-[1]\n    \"FALLBACK\"",
            ),
        ],
        &["FALLBACK", "CALLER"],
        &["fallback", "caller"],
    );
}
