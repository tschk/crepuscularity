use crepuscularity_tui::{
    collect_focusable_ids,
    ratatui::{backend::TestBackend, Terminal},
    Template,
};

fn rows(template: &Template) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(24, 8)).unwrap();
    terminal
        .draw(|frame| template.draw_full(frame).unwrap())
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .chunks(24)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect())
        .collect()
}

#[test]
fn hidden_root_and_nested_subtrees_never_paint_or_add_gap() {
    for classes in [
        "hidden",
        "class=\"hidden\"",
        "hidden w-[12] h-[2]",
        "w-[12] h-[2] hidden",
        "tui:hidden",
    ] {
        let root = Template::from_source(format!("div {classes}\n  \"SECRET\""));
        assert!(
            rows(&root).iter().all(|row| row.trim().is_empty()),
            "{classes}"
        );
        let hidden = Template::from_source(format!("div flex-col gap-1\n  span h-[1]\n    \"FIRST\"\n  div {classes}\n    \"SECRET\"\n  span h-[1]\n    \"LAST\""));
        let reference = Template::from_source(
            "div flex-col gap-1\n  span h-[1]\n    \"FIRST\"\n  span h-[1]\n    \"LAST\"",
        );
        assert_eq!(rows(&hidden), rows(&reference), "{classes}");
    }
}

#[test]
fn hidden_pseudo_elements_do_not_render_or_validate_their_children() {
    for tag in ["slot", "slot-rotate"] {
        let template =
            Template::from_source(format!("{tag} hidden\n  embed widget.js adapter=\"react\""));
        assert!(
            rows(&template).iter().all(|row| row.trim().is_empty()),
            "{tag}"
        );
    }
}

#[test]
fn hidden_focus_ancestry_is_pruned_and_reappears_when_revealed() {
    let mut template = Template::from_source("div class:hidden={hide}\n  button #secret focusable\n    \"SECRET\"\nbutton #visible focusable\n  \"VISIBLE\"");
    template.set("hide", true);
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["visible"]);
    assert!(!rows(&template).concat().contains("SECRET"));
    template.set("hide", false);
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["secret", "visible"]);
    assert!(rows(&template).concat().contains("SECRET"));
    assert_eq!(
        collect_focusable_ids("div hidden\n  button #secret focusable\nbutton #visible focusable")
            .unwrap(),
        ["visible"]
    );
}

#[test]
fn hidden_includes_and_slots_use_their_resolved_contexts() {
    let mut template = Template::from_source(
        "include child.crepus hide={hide}\n  button #caller focusable\n    \"CALLER\"",
    );
    std::sync::Arc::make_mut(&mut template.context_mut().virtual_files).insert("child.crepus".into(),"div hidden={hide}\n  button #included focusable\n    \"INCLUDED\"\n  slot\n    button #fallback focusable\n      \"FALLBACK\"".into());
    template.set("hide", true);
    template.refresh_focus();
    assert!(template.focus().focusable_ids.is_empty());
    assert!(rows(&template).iter().all(|row| row.trim().is_empty()));
    template.set("hide", false);
    template.refresh_focus();
    assert_eq!(template.focus().focusable_ids, ["included", "caller"]);
    let output = rows(&template).concat();
    assert!(
        output.contains("INCLUDED") && output.contains("CALLER") && !output.contains("FALLBACK"),
        "{output}"
    );
}

#[test]
fn hidden_attributes_follow_boolean_presence_and_dynamic_context() {
    for (attribute, hidden) in [
        ("hidden", true),
        ("hidden=\"\"", true),
        ("hidden=\"false\"", true),
        ("hidden={true}", true),
        ("hidden={false}", false),
        ("hidden={null}", false),
        ("web:hidden", false),
    ] {
        let mut template = Template::from_source(format!(
            "button #target focusable {attribute}\n  \"CONTENT\""
        ));
        template.refresh_focus();
        assert_eq!(
            rows(&template).concat().contains("CONTENT"),
            !hidden,
            "{attribute}"
        );
        assert_eq!(
            template.focus().focusable_ids.is_empty(),
            hidden,
            "{attribute}"
        );
        let static_ids =
            collect_focusable_ids(&format!("button #target focusable {attribute}")).unwrap();
        assert_eq!(static_ids.is_empty(), hidden, "{attribute}");
    }
    let mut template =
        Template::from_source("div hidden={hide}\n  button #target focusable\n    \"CONTENT\"");
    // Syntactic discovery must retain a nonliteral binding without evaluating it.
    assert_eq!(
        collect_focusable_ids("button #target focusable hidden={hide}").unwrap(),
        ["target"]
    );
    for hidden in [true, false, true, false] {
        template.set("hide", hidden);
        template.refresh_focus();
        assert_eq!(template.focus().focusable_ids.is_empty(), hidden);
        assert_eq!(
            template.focus().focused_id.as_deref(),
            (!hidden).then_some("target")
        );
        assert_eq!(rows(&template).concat().contains("CONTENT"), !hidden);
    }
}
