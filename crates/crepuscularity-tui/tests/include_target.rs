use std::sync::Arc;

use crepuscularity_tui::Template;
use ratatui::{backend::TestBackend, Terminal};

fn output(template: &Template) -> String {
    let mut terminal = Terminal::new(TestBackend::new(160, 8)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

fn files(template: &mut Template, sources: &[(&str, &str)]) {
    Arc::make_mut(&mut template.context_mut().virtual_files).extend(
        sources
            .iter()
            .map(|(path, source)| (path.to_string(), source.to_string())),
    );
}

#[test]
fn ordinary_named_and_nested_includes_inherit_all_target_flags() {
    let body = "span \"{crepus_target}|{is_tui}|{is_gui}|{is_web}|{crepus_platform}|{is_macos}|{is_windows}|{is_linux}\"";
    for overridden in [false, true] {
        let mut root = Template::from_source(body);
        if overridden {
            root.set("crepus_target", "custom");
            root.set("is_tui", false);
            root.set("is_gui", true);
            root.set("is_web", true);
            root.set("crepus_platform", "custom-os");
            root.set("is_macos", false);
            root.set("is_windows", true);
            root.set("is_linux", true);
        }
        let expected = output(&root);
        for source in [
            "include child.crepus",
            "include named.crepus#Card",
            "include nested.crepus",
        ] {
            let mut child = Template::from_source(source);
            child.context_mut().vars = root.context().vars.clone();
            files(
                &mut child,
                &[
                    ("child.crepus", body),
                    ("named.crepus", &format!("--- Card\n{body}")),
                    ("nested.crepus", "include child.crepus"),
                ],
            );
            assert_eq!(
                output(&child),
                expected,
                "{source}, overridden={overridden}"
            );
        }
    }
}

#[test]
fn included_target_guards_participate_in_focus_discovery() {
    let mut template = Template::from_source("include nested.crepus");
    files(
        &mut template,
        &[
            (
                "nested.crepus",
                "include child.crepus\ninclude named.crepus#Card",
            ),
            (
                "child.crepus",
                "if is_tui\n  button #ordinary focusable\n    \"Ordinary\"",
            ),
            (
                "named.crepus",
                "--- Card\nif is_tui\n  button #named focusable\n    \"Named\"",
            ),
        ],
    );
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["ordinary", "named"]);
    let rendered = output(&template);
    assert!(
        rendered.contains("Ordinary") && rendered.contains("Named"),
        "{rendered}"
    );
}

#[test]
fn include_props_override_inherited_flags_without_leaking_parent_vars_or_slot_scope() {
    let mut template = Template::from_source(
        "include child.crepus is_tui=false label={secret}\n  span \"slot:{secret}:{is_tui}\"",
    );
    template.set("secret", "caller");
    files(
        &mut template,
        &[(
            "child.crepus",
            "span \"child:{label}:{secret}:{is_tui}\"\nslot",
        )],
    );
    let rendered = output(&template);
    assert!(rendered.contains("child:caller::false"), "{rendered}");
    assert!(rendered.contains("slot:caller:true"), "{rendered}");
}

#[test]
fn inherited_flags_override_named_defaults_and_explicit_props_still_win() {
    for (source, expected) in [
        ("include named.crepus#Card", "tui:true:default"),
        (
            "include named.crepus#Card is_tui=false label=\"prop\"",
            "tui:false:prop",
        ),
    ] {
        let mut template = Template::from_source(source);
        files(&mut template, &[("named.crepus", "--- Card\n$: default crepus_target = \"wrong\"\n$: default is_tui = false\n$: default label = \"default\"\nspan \"{crepus_target}:{is_tui}:{label}\"")]);
        let rendered = output(&template);
        assert!(rendered.contains(expected), "{source}: {rendered}");
    }
}
