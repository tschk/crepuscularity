#![cfg(feature = "std")]

use crepuscularity_embedded::{
    render_template_to_framebuffer, Rgb565Buffer, ScreenSize, TemplateContext, TemplateValue,
    DEFAULT_BG,
};

fn context() -> TemplateContext {
    let mut ctx = TemplateContext::new();
    ctx.set("label", "Parent");
    let mut first = TemplateContext::new();
    first.set("value", "First");
    let mut second = TemplateContext::new();
    second.set("value", "Second");
    ctx.set("items", TemplateValue::List(vec![first, second]));
    std::sync::Arc::make_mut(&mut ctx.virtual_files).extend([
        ("label.crepus".into(), "span \"{label}\"".into()),
        (
            "named.crepus".into(),
            "--- Label\n$: default label = \"Default\"\nspan \"{label}\"".into(),
        ),
        ("shell.crepus".into(), "slot\n  span \"Fallback\"".into()),
    ]);
    ctx
}

fn assert_label(body: &str, expected: &str) {
    let ctx = context();
    let screen = ScreenSize::new(96, 32);
    let mut actual = Rgb565Buffer::new(screen, DEFAULT_BG);
    let mut reference = Rgb565Buffer::new(screen, DEFAULT_BG);
    let button = "button #target border-2 border-[#00ff00] p-[1px] bg-[#000000] text-[#ffffff]";
    let source = format!(
        "{button}\n{}",
        body.lines()
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let document = render_template_to_framebuffer(&source, &ctx, screen, &mut actual).unwrap();
    let direct = format!("{button}\n  span \"{expected}\"");
    let expected_doc =
        render_template_to_framebuffer(&direct, &ctx, screen, &mut reference).unwrap();
    let label = document.node_by_id("target").unwrap();
    assert_eq!(label.text.as_deref(), Some(expected), "{source}");
    assert_eq!(
        label.bounds,
        expected_doc.node_by_id("target").unwrap().bounds,
        "{source}"
    );
    assert_eq!(actual.pixels(), reference.pixels(), "{source}");
    assert_eq!(document.hit_test(label.bounds.w - 1, 0), Some("target"));
    assert_eq!(document.hit_test(label.bounds.w, 0), None);
    assert_eq!(ctx.get_str("label"), "Parent");
}

#[test]
fn evaluated_control_flow_and_include_labels_match_direct_button_pixels() {
    for (body, expected) in [
        ("if true\n  span \"OK\"\nelse\n  span \"Wrong\"", "OK"),
        ("if false\n  span \"Wrong\"\nelse\n  span \"OK\"", "OK"),
        (
            "match label\n  \"Parent\" =>\n    span \"Matched\"\n  _ =>\n    span \"Wrong\"",
            "Matched",
        ),
        ("for item in items\n  span \"{item}\"", "First"),
        ("include label.crepus label=\"Included\"", "Included"),
        ("include named.crepus#Label", "Default"),
        ("include named.crepus#Label label=\"Provided\"", "Provided"),
        ("include shell.crepus\n  span \"{label}\"", "Parent"),
        ("include shell.crepus", "Fallback"),
        ("$: let label = \"Local\"\nspan \"{label}\"", "Local"),
    ] {
        assert_label(body, expected);
    }
}

#[test]
fn visibility_empty_precedence_and_nested_declaration_scopes_are_retained() {
    for body in [
        "if true\n  span hidden \"Wrong\"\nspan \"OK\"",
        "if true\n  span invisible \"Wrong\"\nspan \"OK\"",
        "if true\n  span opacity-0 \"Wrong\"\nspan \"OK\"",
        "span\n  if false\n    span \"Wrong\"\nspan \"OK\"",
        "if true\n  $: let label = \"Wrong\"\nspan \"OK\"",
    ] {
        assert_label(body, "OK");
    }
    assert_label("if true\n  span \"\"\nspan \"Fallback\"", "");
    assert_label("span \"First\"\nif true\n  span \"Second\"", "First");
    assert_label("div\n  span \"Not a primary span\"\nspan \"OK\"", "OK");
}

#[test]
fn evaluated_labels_do_not_bypass_active_island_errors_or_mutate_pixels() {
    let screen = ScreenSize::new(96, 32);
    let mut fb = Rgb565Buffer::new(screen, DEFAULT_BG);
    let before = fb.pixels().to_vec();
    let error = render_template_to_framebuffer(
        "button\n  if true\n    span \"OK\"\n  embed widget.js adapter=\"react\"",
        &context(),
        screen,
        &mut fb,
    )
    .unwrap_err();
    assert!(error.to_string().contains("widget.js"));
    assert_eq!(fb.pixels(), before);
}
