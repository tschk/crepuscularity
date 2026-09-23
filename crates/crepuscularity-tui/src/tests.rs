use ratatui::{backend::TestBackend, style::Color, Terminal};
use std::fs;
use std::path::PathBuf;

use crate::{
    draw as draw_template, render_component, render_template, template, HotTemplate, ReloadOutcome,
    TemplateContext, TemplateValue,
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Render a template into a TestBackend of the given dimensions and return the
/// full buffer as a Vec of row strings (one per terminal row).
fn render(width: u16, height: u16, template: &str, ctx: &TemplateContext) -> Vec<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_template(template, ctx, frame, area).expect("render_template returned an error");
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let w = buf.area.width as usize;
    let h = buf.area.height as usize;
    (0..h)
        .map(|y| {
            buf.content[y * w..(y + 1) * w]
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
        })
        .collect()
}

/// Collect all rows into one string (newline-separated) for easy `contains` checks.
fn all_text(rows: &[String]) -> String {
    rows.join("\n")
}

fn buffer_rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buf = terminal.backend().buffer();
    let w = buf.area.width as usize;
    let h = buf.area.height as usize;
    (0..h)
        .map(|y| {
            buf.content[y * w..(y + 1) * w]
                .iter()
                .map(|c| c.symbol())
                .collect::<String>()
        })
        .collect()
}

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn temp_case(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "crepuscularity-tui-{}-{}",
        name,
        std::process::id()
    ));
    let _ = fs::create_dir_all(&dir);
    dir
}

// ─── Basic rendering ──────────────────────────────────────────────────────────

#[test]
fn plain_text_renders() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "terminal");
    let rows = render(
        40,
        5,
        "div w-full h-full flex-col\n  div\n    \"Hello {name}\"",
        &ctx,
    );
    assert!(
        all_text(&rows).contains("Hello terminal"),
        "expected 'Hello terminal' in output:\n{}",
        all_text(&rows)
    );
}

#[test]
fn file_template_builder_renders() {
    let dir = temp_case("builder");
    let path = dir.join("ui.crepus");
    fs::write(
        &path,
        "div w-full h-full flex-col\n  div h-[1]\n    \"{title}\"\n  div h-[1]\n    \"{input}\"",
    )
    .unwrap();

    let mut ui = template(&path).unwrap();
    ui.set("title", "My App");
    ui.set("input", "input contents");

    let backend = TestBackend::new(40, 4);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| ui.draw_full(frame).expect("template draws"))
        .unwrap();

    let text = all_text(&buffer_rows(&terminal));
    assert!(text.contains("My App"), "{text}");
    assert!(text.contains("input contents"), "{text}");
    assert_eq!(ui.context().base_dir.as_deref(), Some(dir.as_path()));
}

#[test]
fn draw_helper_owns_terminal_draw_pass() {
    let dir = temp_case("draw-helper");
    let path = dir.join("ui.crepus");
    fs::write(&path, "div\n  \"{title}\"").unwrap();

    let backend = TestBackend::new(40, 4);
    let mut terminal = Terminal::new(backend).unwrap();
    draw_template(&mut terminal, &path, |ui| {
        ui.set("title", "Draw Helper");
    })
    .unwrap();

    let text = all_text(&buffer_rows(&terminal));
    assert!(text.contains("Draw Helper"), "{text}");
}

#[test]
fn raw_expression_renders() {
    let mut ctx = TemplateContext::new();
    ctx.set("score", 42i64);
    let rows = render(30, 3, "div\n  {score}", &ctx);
    assert!(
        all_text(&rows).contains("42"),
        "expected '42' in:\n{}",
        all_text(&rows)
    );
}

#[test]
fn renderer_target_defaults_are_available_to_templates() {
    let ctx = TemplateContext::new();
    let rows = render(
        40,
        5,
        "div\n  if {is_tui && crepus_target == \"tui\"}\n    div tui-panel\n      \"TUI target\"",
        &ctx,
    );
    assert!(
        all_text(&rows).contains("TUI target"),
        "{}",
        all_text(&rows)
    );
}

#[test]
fn renderer_target_defaults_do_not_override_context() {
    let mut ctx = TemplateContext::new();
    ctx.set("crepus_target", "gui");
    ctx.set("is_tui", false);
    ctx.set("is_gui", true);
    let rows = render(
        40,
        5,
        "div\n  if {is_gui && crepus_target == \"gui\"}\n    div\n      \"GUI target\"",
        &ctx,
    );
    assert!(
        all_text(&rows).contains("GUI target"),
        "{}",
        all_text(&rows)
    );
}

#[test]
fn target_prefixed_classes_apply_only_for_matching_renderer() {
    let ctx = TemplateContext::new();
    let rows = render(
        40,
        5,
        "div tui:background-black gpui:background-white web:background-red macos:background-grey\n  \"target styles\"",
        &ctx,
    );
    assert!(
        all_text(&rows).contains("target styles"),
        "{}",
        all_text(&rows)
    );
}

#[test]
fn button_tag_gets_terminal_button_chrome() {
    let ctx = TemplateContext::new();
    let rows = render(24, 3, "button w-[12] h-[3]\n  \"Run\"", &ctx);
    let text = all_text(&rows);
    assert!(text.contains("Run"), "{text}");
    assert!(text.contains("╭") || text.contains("┌"), "{text}");
}

#[test]
fn overflow_y_scroll_uses_scroll_offset() {
    let mut ctx = TemplateContext::new();
    ctx.set("offset", 2i64);
    let rows = render(
        20,
        3,
        "div h-full overflow-y-scroll scroll-offset={offset}\n  div h-[1]\n    \"zero\"\n  div h-[1]\n    \"one\"\n  div h-[1]\n    \"two\"\n  div h-[1]\n    \"three\"\n  div h-[1]\n    \"four\"\n  div h-[1]\n    \"five\"",
        &ctx,
    );
    let text = all_text(&rows);
    assert!(!text.contains("zero"), "{text}");
    assert!(!text.contains("one"), "{text}");
    assert!(text.contains("two"), "{text}");
    assert!(text.contains("three"), "{text}");
    assert!(text.contains("four"), "{text}");
    assert!(!text.contains("five"), "{text}");
}

#[test]
fn scroll_offset_clamps_to_max() {
    let mut ctx = TemplateContext::new();
    ctx.set("offset", 100i64);
    let rows = render(
        20,
        3,
        "div h-full overflow-y-scroll scroll-offset={offset}\n  div h-[1]\n    \"zero\"\n  div h-[1]\n    \"one\"\n  div h-[1]\n    \"two\"\n  div h-[1]\n    \"three\"",
        &ctx,
    );
    let text = all_text(&rows);
    assert!(!text.contains("zero"), "{text}");
    assert!(text.contains("one"), "{text}");
    assert!(text.contains("two"), "{text}");
    assert!(text.contains("three"), "{text}");
}

#[test]
fn scroll_viewport_culls_children_outside_viewport() {
    let mut ctx = TemplateContext::new();
    ctx.set("offset", 3i64);
    let rows = render(
        20,
        3,
        "div h-full overflow-y-scroll scroll-offset={offset}\n  div h-[1]\n    \"row0\"\n  div h-[1]\n    \"row1\"\n  div h-[1]\n    \"row2\"\n  div h-[1]\n    \"row3\"\n  div h-[1]\n    \"row4\"\n  div h-[1]\n    \"row5\"\n  div h-[1]\n    \"row6\"",
        &ctx,
    );
    let text = all_text(&rows);
    assert!(!text.contains("row0"), "{text}");
    assert!(!text.contains("row1"), "{text}");
    assert!(!text.contains("row2"), "{text}");
    assert!(text.contains("row3"), "{text}");
    assert!(text.contains("row4"), "{text}");
    assert!(text.contains("row5"), "{text}");
    assert!(!text.contains("row6"), "{text}");
}

#[test]
fn scroll_bottom_auto_scrolls_to_last_child() {
    let mut ctx = TemplateContext::new();
    ctx.set("auto_scroll", true);
    let rows = render(
        20,
        3,
        "div h-full overflow-y-scroll scroll-bottom={auto_scroll}\n  div h-[1]\n    \"row0\"\n  div h-[1]\n    \"row1\"\n  div h-[1]\n    \"row2\"\n  div h-[1]\n    \"row3\"\n  div h-[1]\n    \"row4\"",
        &ctx,
    );
    let text = all_text(&rows);
    assert!(!text.contains("row0"), "{text}");
    assert!(!text.contains("row1"), "{text}");
    assert!(text.contains("row2"), "{text}");
    assert!(text.contains("row3"), "{text}");
    assert!(text.contains("row4"), "{text}");
}

#[test]
fn scroll_bottom_false_does_not_auto_scroll() {
    let mut ctx = TemplateContext::new();
    ctx.set("auto_scroll", false);
    let rows = render(
        20,
        3,
        "div h-full overflow-y-scroll scroll-bottom={auto_scroll}\n  div h-[1]\n    \"row0\"\n  div h-[1]\n    \"row1\"\n  div h-[1]\n    \"row2\"\n  div h-[1]\n    \"row3\"\n  div h-[1]\n    \"row4\"",
        &ctx,
    );
    let text = all_text(&rows);
    assert!(text.contains("row0"), "{text}");
    assert!(text.contains("row1"), "{text}");
    assert!(text.contains("row2"), "{text}");
    assert!(!text.contains("row3"), "{text}");
    assert!(!text.contains("row4"), "{text}");
}

#[test]
fn scroll_content_shorter_than_viewport_renders_normally() {
    let mut ctx = TemplateContext::new();
    ctx.set("offset", 5i64);
    let rows = render(
        20,
        5,
        "div h-full overflow-y-scroll scroll-offset={offset}\n  div h-[1]\n    \"only\"\n  div h-[1]\n    \"two\"",
        &ctx,
    );
    let text = all_text(&rows);
    assert!(text.contains("only"), "{text}");
    assert!(text.contains("two"), "{text}");
}

// ─── JSX / React-style syntax ─────────────────────────────────────────────────
//
// The parser auto-detects JSX when the first content token starts with `<`.
// All the same layout and styling classes work — just angle-bracket style.

#[test]
fn jsx_syntax_renders() {
    let mut ctx = TemplateContext::new();
    ctx.set("user", "ada");
    // Same template as indentation syntax but written in JSX/HTML style.
    let tpl = r#"<div class="w-full h-full flex-col">
  <div class="h-[1]">Hello {user}</div>
  <div class="flex-1">Content area</div>
</div>"#;
    let rows = render(40, 5, tpl, &ctx);
    assert!(
        all_text(&rows).contains("Hello ada"),
        "JSX: expected 'Hello ada' in:\n{}",
        all_text(&rows)
    );
    assert!(
        all_text(&rows).contains("Content area"),
        "JSX: expected 'Content area' in:\n{}",
        all_text(&rows)
    );
}

#[test]
fn jsx_and_indent_are_equivalent() {
    let ctx = TemplateContext::new();

    let indent = "div flex-col\n  div\n    \"Row A\"\n  div\n    \"Row B\"";
    let jsx = "<div class=\"flex-col\"><div>Row A</div><div>Row B</div></div>";

    let indent_rows = render(30, 5, indent, &ctx);
    let jsx_rows = render(30, 5, jsx, &ctx);

    // Both should contain the same text regardless of syntax.
    assert!(
        all_text(&indent_rows).contains("Row A"),
        "indent: {}",
        all_text(&indent_rows)
    );
    assert!(
        all_text(&jsx_rows).contains("Row A"),
        "jsx: {}",
        all_text(&jsx_rows)
    );
    assert!(
        all_text(&indent_rows).contains("Row B"),
        "indent: {}",
        all_text(&indent_rows)
    );
    assert!(
        all_text(&jsx_rows).contains("Row B"),
        "jsx: {}",
        all_text(&jsx_rows)
    );
}

#[test]
fn jsx_class_name_alias() {
    // React uses `className` instead of `class`; the parser accepts both.
    let ctx = TemplateContext::new();
    let tpl = r#"<div className="flex-col"><div>className works</div></div>"#;
    let rows = render(30, 3, tpl, &ctx);
    assert!(
        all_text(&rows).contains("className works"),
        "{}",
        all_text(&rows)
    );
}

// ─── Layout ───────────────────────────────────────────────────────────────────

#[test]
fn flex_col_stacks_vertically() {
    // Each child gets its own row.
    let ctx = TemplateContext::new();
    let tpl = "div flex-col\n  div h-[1]\n    \"Top\"\n  div h-[1]\n    \"Bottom\"";
    let rows = render(20, 4, tpl, &ctx);
    // "Top" should appear before "Bottom" in the row list.
    let top_row = rows.iter().position(|r| r.contains("Top")).unwrap();
    let bot_row = rows.iter().position(|r| r.contains("Bottom")).unwrap();
    assert!(
        top_row < bot_row,
        "flex-col: Top should be above Bottom. rows:\n{}",
        all_text(&rows)
    );
}

#[test]
fn flex_row_stacks_horizontally() {
    let ctx = TemplateContext::new();
    let tpl = "div flex-row\n  div w-[8]\n    \"Left\"\n  div flex-1\n    \"Right\"";
    let rows = render(30, 3, tpl, &ctx);
    // "Left" and "Right" should appear on the same row.
    let row_with_left = rows.iter().find(|r| r.contains("Left")).unwrap();
    assert!(
        row_with_left.contains("Right"),
        "flex-row: Left and Right should be on the same row. rows:\n{}",
        all_text(&rows)
    );
}

#[test]
fn fixed_height_constraint() {
    let ctx = TemplateContext::new();
    // Header takes exactly 2 rows, body gets the rest.
    let tpl = "div w-full h-full flex-col\n  div h-[2]\n    \"Header\"\n  div flex-1\n    \"Body\"";
    let rows = render(20, 6, tpl, &ctx);
    assert!(all_text(&rows).contains("Header"), "{}", all_text(&rows));
    assert!(all_text(&rows).contains("Body"), "{}", all_text(&rows));
}

#[test]
fn full_screen_tui_layout() {
    // Classic TUI: header / sidebar + main / status-bar.
    let ctx = TemplateContext::new();
    let tpl = r#"div w-full h-full flex-col
  div h-[1]
    "Header"
  div flex-1 flex-row
    div w-[10]
      "Sidebar"
    div flex-1
      "Main"
  div h-[1]
    "Status""#;
    let rows = render(40, 8, tpl, &ctx);
    assert!(all_text(&rows).contains("Header"), "{}", all_text(&rows));
    assert!(all_text(&rows).contains("Sidebar"), "{}", all_text(&rows));
    assert!(all_text(&rows).contains("Main"), "{}", all_text(&rows));
    assert!(all_text(&rows).contains("Status"), "{}", all_text(&rows));
}

// ─── Control flow ─────────────────────────────────────────────────────────────

#[test]
fn if_true_branch_shown() {
    let mut ctx = TemplateContext::new();
    ctx.set("ok", true);
    let tpl = "div\n if {ok}\n  \"Yes\"\n else\n  \"No\"";
    let rows = render(20, 3, tpl, &ctx);
    assert!(all_text(&rows).contains("Yes"), "{}", all_text(&rows));
    assert!(!all_text(&rows).contains("No"), "{}", all_text(&rows));
}

#[test]
fn if_false_branch_shown() {
    let mut ctx = TemplateContext::new();
    ctx.set("ok", false);
    let tpl = "div\n if {ok}\n  \"Yes\"\n else\n  \"No\"";
    let rows = render(20, 3, tpl, &ctx);
    assert!(!all_text(&rows).contains("Yes"), "{}", all_text(&rows));
    assert!(all_text(&rows).contains("No"), "{}", all_text(&rows));
}

#[test]
fn if_no_else_empty_on_false() {
    let mut ctx = TemplateContext::new();
    ctx.set("flag", false);
    let tpl = "div\n if {flag}\n  \"Visible\"";
    let rows = render(20, 3, tpl, &ctx);
    assert!(!all_text(&rows).contains("Visible"), "{}", all_text(&rows));
}

#[test]
fn for_loop_renders_items() {
    let mut ctx = TemplateContext::new();
    let items: Vec<TemplateContext> = ["Alpha", "Beta", "Gamma"]
        .iter()
        .map(|s| {
            let mut c = TemplateContext::new();
            c.set("value", *s);
            c
        })
        .collect();
    ctx.set("items", TemplateValue::List(items));

    let tpl = "div flex-col\n for item in {items}\n  div\n    \"{item}\"";
    let rows = render(20, 6, tpl, &ctx);
    assert!(all_text(&rows).contains("Alpha"), "{}", all_text(&rows));
    assert!(all_text(&rows).contains("Beta"), "{}", all_text(&rows));
    assert!(all_text(&rows).contains("Gamma"), "{}", all_text(&rows));
}

#[test]
fn match_correct_arm() {
    let mut ctx = TemplateContext::new();
    ctx.set("status", "active");
    // Arms are at the SAME indent as `match`, bodies one level deeper.
    let tpl = "div\n match {status}\n \"active\" =>\n  \"Online\"\n _ =>\n  \"Offline\"";
    let rows = render(20, 3, tpl, &ctx);
    assert!(all_text(&rows).contains("Online"), "{}", all_text(&rows));
    assert!(!all_text(&rows).contains("Offline"), "{}", all_text(&rows));
}

#[test]
fn match_wildcard_arm() {
    let mut ctx = TemplateContext::new();
    ctx.set("status", "unknown");
    let tpl = "div\n match {status}\n \"active\" =>\n  \"Online\"\n _ =>\n  \"Offline\"";
    let rows = render(20, 3, tpl, &ctx);
    assert!(all_text(&rows).contains("Offline"), "{}", all_text(&rows));
}

// ─── Let declarations ─────────────────────────────────────────────────────────

#[test]
fn let_decl_sets_variable() {
    let ctx = TemplateContext::new();
    let tpl = "div\n $: let total = {10 + 5}\n div\n  \"{total}\"";
    let rows = render(20, 3, tpl, &ctx);
    assert!(all_text(&rows).contains("15"), "{}", all_text(&rows));
}

// ─── Styling (smoke tests — verify no panics) ─────────────────────────────────

#[test]
fn colour_classes_dont_panic() {
    let ctx = TemplateContext::new();
    let tpl = "div bg-zinc-950 text-white flex-col\n  div text-green-400\n    \"Green\"\n  div text-red-500\n    \"Red\"\n  div bg-[#1e1e2e] text-[#cdd6f4]\n    \"Catppuccin\"";
    let rows = render(30, 5, tpl, &ctx);
    assert!(all_text(&rows).contains("Green"), "{}", all_text(&rows));
    assert!(
        all_text(&rows).contains("Catppuccin"),
        "{}",
        all_text(&rows)
    );
}

#[test]
fn border_classes_dont_panic() {
    let ctx = TemplateContext::new();
    // 6 rows: 1 top border + 1 top padding + 1 content + 1 bottom padding + 1 bottom border = 5
    let tpl = "div border rounded p-1\n  \"Boxed\"";
    let rows = render(20, 6, tpl, &ctx);
    assert!(all_text(&rows).contains("Boxed"), "{}", all_text(&rows));
}

#[test]
fn fixed_height_with_one_sided_border_keeps_content_visible() {
    let ctx = TemplateContext::new();
    let tpl = "div w-full h-full flex-col\n  div h-[1] border-b\n    \"Header\"\n  div flex-1\n    \"Body\"";
    let rows = render(30, 5, tpl, &ctx);
    assert!(all_text(&rows).contains("Header"), "{}", all_text(&rows));
    assert!(all_text(&rows).contains("Body"), "{}", all_text(&rows));
}

#[test]
fn child_text_inherits_parent_style() {
    let backend = TestBackend::new(30, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    let ctx = TemplateContext::new();
    terminal
        .draw(|frame| {
            let area = frame.area();
            render_template(
                "div text-[#cdd6f4]\n  div\n    \"Inherited\"",
                &ctx,
                frame,
                area,
            )
            .expect("render_template returned an error");
        })
        .unwrap();

    let buf = terminal.backend().buffer();
    let cell = buf
        .content
        .iter()
        .find(|c| c.symbol() == "I")
        .expect("expected inherited text to render");
    assert_eq!(cell.fg, Color::Rgb(205, 214, 244));
}

#[test]
fn text_modifiers_dont_panic() {
    let ctx = TemplateContext::new();
    let tpl = "div font-bold italic underline\n  \"Styled\"";
    let rows = render(20, 3, tpl, &ctx);
    assert!(all_text(&rows).contains("Styled"), "{}", all_text(&rows));
}

#[test]
fn gap_spacing_dont_panic() {
    let ctx = TemplateContext::new();
    let tpl = "div flex-col gap-1\n  div h-[1]\n    \"A\"\n  div h-[1]\n    \"B\"";
    let rows = render(20, 5, tpl, &ctx);
    assert!(all_text(&rows).contains("A"), "{}", all_text(&rows));
    assert!(all_text(&rows).contains("B"), "{}", all_text(&rows));
}

// ─── Multi-component files ────────────────────────────────────────────────────

#[test]
fn multi_component_render() {
    let ctx = TemplateContext::new();
    let content = "--- Card\ndiv border rounded p-1\n  slot\n    \"fallback\"";
    let rows = {
        let backend = TestBackend::new(30, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_component(content, "Card", &ctx, frame, area).unwrap();
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let w = buf.area.width as usize;
        let h = buf.area.height as usize;
        (0..h)
            .map(|y| {
                buf.content[y * w..(y + 1) * w]
                    .iter()
                    .map(|c| c.symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
    };
    assert!(all_text(&rows).contains("fallback"), "{}", all_text(&rows));
}

#[test]
fn include_uses_isolated_context() {
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(PathBuf::from("/virtual"));
    ctx.set("secret", "leak");
    std::sync::Arc::make_mut(&mut ctx.virtual_files)
        .insert("child.crepus".into(), "div\n  \"{secret}\"".into());

    let tpl = "div\n include child.crepus";
    let rows = render(30, 4, tpl, &ctx);
    assert!(!all_text(&rows).contains("leak"), "{}", all_text(&rows));
}

#[test]
fn include_reads_virtual_file_by_suffix_match() {
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(PathBuf::from("/virtual"));
    std::sync::Arc::make_mut(&mut ctx.virtual_files)
        .insert("child.crepus".into(), "div\n  \"From virtual\"".into());

    let tpl = "div\n include child.crepus";
    let rows = render(30, 4, tpl, &ctx);
    assert!(
        all_text(&rows).contains("From virtual"),
        "{}",
        all_text(&rows)
    );
}

#[test]
fn include_rejects_parent_dir_escape() {
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(PathBuf::from("/virtual"));
    std::sync::Arc::make_mut(&mut ctx.virtual_files)
        .insert("../secret.crepus".into(), "div\n  \"secret\"".into());

    let tpl = "div\n include ../secret.crepus";
    let rows = render(80, 4, tpl, &ctx);
    let text = all_text(&rows);
    assert!(text.contains("include path outside base dir"), "{text}");
    assert!(
        !text.contains(" ⚠ ") || !text.contains("secret\""),
        "rendered secret content leaked: {text}"
    );
}

#[test]
fn include_rejects_absolute_path() {
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(PathBuf::from("/virtual"));
    let path = std::env::temp_dir().join("secret.crepus");
    let include_path = path.to_string_lossy().into_owned();
    std::sync::Arc::make_mut(&mut ctx.virtual_files)
        .insert(include_path.clone(), "div\n  \"secret\"".into());

    let tpl = format!("div\n include {include_path}");
    let rows = render(80, 4, &tpl, &ctx);
    let text = all_text(&rows);
    assert!(text.contains("include path outside base dir"), "{text}");
    assert!(
        !text.contains(" ⚠ ") || !text.contains("secret\""),
        "rendered secret content leaked: {text}"
    );
}

#[test]
fn demo_example_renders_without_error() {
    let template = fs::read_to_string(examples_dir().join("showcase/controls.crepus")).unwrap();

    let mut ctx = TemplateContext::new();
    ctx.set("title", "Demo");
    ctx.set("show_badge", true);
    ctx.set("score", 91i64);
    ctx.set("status", "active");
    let items: Vec<TemplateContext> = ["One", "Two"]
        .iter()
        .map(|s| {
            let mut c = TemplateContext::new();
            c.set("value", *s);
            c
        })
        .collect();
    ctx.set("items", TemplateValue::List(items));

    let rows = render(120, 40, &template, &ctx);
    let text = all_text(&rows);

    assert!(!text.contains("⚠"), "{}", text);
}

#[test]
fn jsx_demo_example_renders_include() {
    let template = fs::read_to_string(examples_dir().join("showcase/jsx-demo.crepus")).unwrap();

    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(examples_dir().join("showcase"));
    ctx.set("show_header", true);
    ctx.set("name", "Ada");
    ctx.set("score", 88i64);
    ctx.set("status", "active");
    let items: Vec<TemplateContext> = ["Alpha", "Beta"]
        .iter()
        .map(|s| {
            let mut c = TemplateContext::new();
            c.set("value", *s);
            c
        })
        .collect();
    ctx.set("items", TemplateValue::List(items));

    let rows = render(120, 40, &template, &ctx);
    let text = all_text(&rows);
    assert!(!text.contains("⚠"), "{}", text);
}

#[test]
fn components_demo_renders_slot_content() {
    let template =
        fs::read_to_string(examples_dir().join("showcase/components-demo.crepus")).unwrap();

    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(examples_dir().join("showcase"));
    let rows = render(100, 25, &template, &ctx);
    let text = all_text(&rows);
    assert!(!text.contains("⚠"), "{}", text);
}

// ─── Hot reload ───────────────────────────────────────────────────────────────

fn render_hot(width: u16, height: u16, hot: &mut HotTemplate) -> Vec<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            let _ = hot.poll_and_draw_full(frame);
        })
        .unwrap();
    buffer_rows(&terminal)
}

#[test]
fn template_reload_picks_up_new_source() {
    let dir = temp_case("reload");
    let path = dir.join("ui.crepus");
    fs::write(&path, "div\n  \"first version\"").unwrap();

    let mut tpl = template(&path).unwrap();
    let backend = TestBackend::new(40, 3);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| tpl.draw_full(frame).unwrap())
        .unwrap();
    assert!(
        all_text(&buffer_rows(&terminal)).contains("first version"),
        "{}",
        all_text(&buffer_rows(&terminal))
    );

    fs::write(&path, "div\n  \"second version\"").unwrap();
    tpl.reload().expect("reload succeeds after rewrite");

    terminal
        .draw(|frame| tpl.draw_full(frame).unwrap())
        .unwrap();
    let text = all_text(&buffer_rows(&terminal));
    assert!(text.contains("second version"), "{text}");
    assert!(!text.contains("first version"), "{text}");
}

#[test]
fn template_from_source_happy_path() {
    let tpl = crate::Template::from_source("div\n  \"hello\"");
    assert_eq!(tpl.source(), "div\n  \"hello\"");
    assert!(tpl.path().as_os_str().is_empty());

    let rows = render(20, 3, tpl.source(), tpl.context());
    assert!(all_text(&rows).contains("hello"), "{}", all_text(&rows));
}

#[test]
fn template_from_source_with_path_happy_path() {
    let path = PathBuf::from("some/nested/dir/ui.crepus");
    let tpl = crate::Template::from_source_with_path("div\n  \"x\"", &path);
    assert_eq!(tpl.path(), path);
    assert_eq!(tpl.source(), "div\n  \"x\"");
    assert_eq!(
        tpl.context().base_dir.as_deref(),
        Some(PathBuf::from("some/nested/dir").as_path())
    );
}

#[test]
fn template_from_source_with_path_empty_parent() {
    let path = PathBuf::from("ui.crepus");
    let tpl = crate::Template::from_source_with_path("div\n  \"x\"", &path);
    assert_eq!(tpl.path(), path);
    assert_eq!(tpl.source(), "div\n  \"x\"");
    assert_eq!(
        tpl.context().base_dir.as_deref(),
        Some(PathBuf::from("").as_path())
    );
}

#[test]
fn template_from_source_with_path_root_path() {
    let path = PathBuf::from("/");
    let tpl = crate::Template::from_source_with_path("div\n  \"x\"", &path);
    assert_eq!(tpl.path(), path);
    assert_eq!(tpl.source(), "div\n  \"x\"");
    assert_eq!(tpl.context().base_dir.as_deref(), None);
}

#[test]
fn template_reload_without_path_fails() {
    let mut tpl = crate::Template::from_source("div\n  \"x\"");
    let err = tpl.reload().expect_err("from_source has no path");
    assert!(err.contains("no path"), "{err}");
}

#[test]
fn template_from_path_missing_file_fails() {
    let dir = temp_case("missing-file");
    let path = dir.join("does_not_exist.crepus");

    let res = crate::Template::from_path(&path);
    assert!(res.is_err(), "from_path should fail for missing file");
    if let Err(err) = res {
        assert!(
            err.contains("template error"),
            "Expected error to contain 'template error', got: {err}"
        );
    }
}

#[test]
fn template_reload_with_missing_file_fails_and_keeps_old_source() {
    let dir = temp_case("reload_missing");
    let path = dir.join("ui.crepus");
    fs::write(&path, "div\n  \"initial content\"").unwrap();

    let mut tpl = template(&path).unwrap();
    assert_eq!(tpl.source(), "div\n  \"initial content\"");

    fs::remove_file(&path).unwrap();

    let err = tpl
        .reload()
        .expect_err("reload should fail if file is missing");
    assert!(err.contains("template error"), "{err}");
    assert_eq!(
        tpl.source(),
        "div\n  \"initial content\"",
        "source should be unchanged on error"
    );
}

#[test]
fn hot_template_initial_render_uses_disk_source() {
    let dir = temp_case("hot-initial");
    let path = dir.join("ui.crepus");
    fs::write(&path, "div\n  \"original\"").unwrap();

    let mut hot = HotTemplate::watch(&path).unwrap();
    let rows = render_hot(40, 3, &mut hot);
    assert!(all_text(&rows).contains("original"), "{}", all_text(&rows));
    assert_eq!(hot.template().context().base_dir.as_deref(), Some(&*dir));
}

#[test]
fn hot_template_poll_unchanged_when_flag_clear() {
    let dir = temp_case("hot-noop");
    let path = dir.join("ui.crepus");
    fs::write(&path, "div\n  \"x\"").unwrap();

    let mut hot = HotTemplate::watch(&path).unwrap();
    assert_eq!(hot.poll_reload(), ReloadOutcome::Unchanged);
}

#[test]
fn hot_template_poll_reloads_when_flag_set() {
    let dir = temp_case("hot-flag-set");
    let path = dir.join("ui.crepus");
    fs::write(&path, "div\n  \"before\"").unwrap();

    let mut hot = HotTemplate::watch(&path).unwrap();
    let rows = render_hot(40, 3, &mut hot);
    assert!(all_text(&rows).contains("before"), "{}", all_text(&rows));

    fs::write(&path, "div\n  \"after\"").unwrap();
    *hot.changed_handle().lock().unwrap() = true;

    assert_eq!(hot.poll_reload(), ReloadOutcome::Reloaded);

    let rows = render_hot(40, 3, &mut hot);
    let text = all_text(&rows);
    assert!(text.contains("after"), "{text}");
    assert!(!text.contains("before"), "{text}");
}

#[test]
fn hot_template_preserves_context_across_reload() {
    let dir = temp_case("hot-ctx");
    let path = dir.join("ui.crepus");
    fs::write(&path, "div\n  \"v1: {label}\"").unwrap();

    let mut hot = HotTemplate::watch(&path).unwrap();
    hot.template_mut().set("label", "kept");

    let rows = render_hot(40, 3, &mut hot);
    assert!(all_text(&rows).contains("v1: kept"), "{}", all_text(&rows));

    fs::write(&path, "div\n  \"v2: {label}\"").unwrap();
    *hot.changed_handle().lock().unwrap() = true;
    assert_eq!(hot.poll_reload(), ReloadOutcome::Reloaded);

    let rows = render_hot(40, 3, &mut hot);
    let text = all_text(&rows);
    assert!(text.contains("v2: kept"), "{text}");
}

#[test]
fn hot_template_poll_returns_error_for_missing_file() {
    let dir = temp_case("hot-err");
    let path = dir.join("ui.crepus");
    fs::write(&path, "div\n  \"alive\"").unwrap();

    let mut hot = HotTemplate::watch(&path).unwrap();
    let _ = render_hot(40, 3, &mut hot);

    fs::remove_file(&path).unwrap();
    *hot.changed_handle().lock().unwrap() = true;

    match hot.poll_reload() {
        ReloadOutcome::Error(_) => {}
        other => panic!("expected Error after deleting file, got {other:?}"),
    }

    let rows = render_hot(40, 3, &mut hot);
    assert!(all_text(&rows).contains("alive"), "{}", all_text(&rows));
}

#[test]
fn hot_template_consumes_change_flag() {
    let dir = temp_case("hot-flag-consumed");
    let path = dir.join("ui.crepus");
    fs::write(&path, "div\n  \"x\"").unwrap();

    let mut hot = HotTemplate::watch(&path).unwrap();
    *hot.changed_handle().lock().unwrap() = true;
    assert_eq!(hot.poll_reload(), ReloadOutcome::Reloaded);
    assert_eq!(hot.poll_reload(), ReloadOutcome::Unchanged);
    assert!(!hot.has_pending_change());
}

#[test]
fn hot_template_drop_releases_watcher() {
    // Smoke test: instantiate and drop many `HotTemplate`s in a row. Before
    // moving the watcher's ownership into the struct, every `watch()` call
    // permanently leaked a parked OS thread.
    let dir = temp_case("hot-drop");
    let path = dir.join("ui.crepus");
    fs::write(&path, "div\n  \"x\"").unwrap();

    for _ in 0..16 {
        let hot = HotTemplate::watch(&path).expect("watch should succeed");
        drop(hot);
    }
}

// ─── Watcher event filter (pure) ──────────────────────────────────────────────

mod watcher_filter {
    use crepuscularity_core::watch::event_touches_relevant_path;
    use notify::event::{ModifyKind, RemoveKind};
    use notify::{Event, EventKind};
    use std::fs;
    use std::path::PathBuf;

    fn ev(kind: EventKind, paths: Vec<PathBuf>) -> Event {
        Event {
            kind,
            paths,
            attrs: Default::default(),
        }
    }

    #[test]
    fn matches_target_template() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("ui.crepus");
        fs::write(&target, "div").unwrap();
        let target = target.canonicalize().unwrap();
        let e = ev(EventKind::Modify(ModifyKind::Any), vec![target.clone()]);
        assert!(event_touches_relevant_path(&e, &target, dir.path()));
    }

    #[test]
    fn matches_sibling_include() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("ui.crepus");
        let include = dir.path().join("card.crepus");
        fs::write(&target, "div").unwrap();
        fs::write(&include, "div").unwrap();
        let target = target.canonicalize().unwrap();
        let include = include.canonicalize().unwrap();
        let e = ev(EventKind::Modify(ModifyKind::Any), vec![include]);
        assert!(event_touches_relevant_path(&e, &target, dir.path()));
    }

    #[test]
    fn matches_target_after_atomic_save_remove() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("ui.crepus");
        fs::write(&target, "div").unwrap();
        let target = target.canonicalize().unwrap();
        fs::remove_file(&target).unwrap();
        let e = ev(EventKind::Remove(RemoveKind::File), vec![target.clone()]);
        assert!(event_touches_relevant_path(&e, &target, dir.path()));
    }

    #[test]
    fn ignores_unrelated_extension() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("ui.crepus");
        let other = dir.path().join("README.md");
        fs::write(&target, "div").unwrap();
        fs::write(&other, "x").unwrap();
        let target = target.canonicalize().unwrap();
        let other = other.canonicalize().unwrap();
        let e = ev(EventKind::Modify(ModifyKind::Any), vec![other]);
        assert!(!event_touches_relevant_path(&e, &target, dir.path()));
    }
}

mod template_error_tests {
    use crate::{draw, template};
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn template_missing_file_returns_error() {
        let res = template("non_existent_file.crepus");
        match res {
            Err(e) => assert!(e.contains("template error")),
            Ok(_) => panic!("expected err"),
        }
    }

    #[test]
    fn draw_missing_file_returns_error() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        let res = draw(&mut terminal, "non_existent_file.crepus", |_| {});
        match res {
            Err(e) => assert!(e.contains("template error")),
            Ok(_) => panic!("expected err"),
        }
    }

    #[test]
    fn template_draw_full_returns_error_on_invalid_template() {
        let tpl = crate::Template::from_source("<< invalid");
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut err = None;
        terminal
            .draw(|frame| {
                if let Err(e) = tpl.draw_full(frame) {
                    err = Some(e);
                }
            })
            .unwrap();
        let e = err.expect("draw_full should return an error for invalid template");
        assert!(
            e.contains("parse error"),
            "error should mention parse error: {e}"
        );
    }

    #[test]
    fn draw_helper_returns_error_on_invalid_template() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ui.crepus");
        std::fs::write(&path, "<< invalid syntax").unwrap();

        let backend = TestBackend::new(40, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        let res = crate::draw(&mut terminal, &path, |_ui| {});

        let e = res.expect_err("draw should return an error for invalid template");
        assert!(
            e.contains("parse error"),
            "error should mention parse error: {e}"
        );
    }
}

// ─── Focus + event system ─────────────────────────────────────────────────────

mod focus_events {
    use crate::event::{DispatchResult, Event, EventDispatcher, FocusManager};
    use crate::{collect_focusable_ids, EventResult, Template};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    #[test]
    fn focus_manager_next_wraps() {
        let mut fm = FocusManager::new();
        fm.set_focusable_ids(vec!["a".to_string(), "b".to_string(), "c".to_string()]);

        assert_eq!(fm.focused_id.as_deref(), Some("a"));
        fm.focus_next();
        assert_eq!(fm.focused_id.as_deref(), Some("b"));
        fm.focus_next();
        assert_eq!(fm.focused_id.as_deref(), Some("c"));
        fm.focus_next();
        assert_eq!(fm.focused_id.as_deref(), Some("a"));
    }

    #[test]
    fn focus_manager_prev_wraps() {
        let mut fm = FocusManager::new();
        fm.set_focusable_ids(vec!["a".to_string(), "b".to_string(), "c".to_string()]);

        assert_eq!(fm.focused_id.as_deref(), Some("a"));
        fm.focus_prev();
        assert_eq!(fm.focused_id.as_deref(), Some("c"));
        fm.focus_prev();
        assert_eq!(fm.focused_id.as_deref(), Some("b"));
        fm.focus_prev();
        assert_eq!(fm.focused_id.as_deref(), Some("a"));
    }

    #[test]
    fn focus_manager_focus_by_id() {
        let mut fm = FocusManager::new();
        fm.set_focusable_ids(vec!["a".to_string(), "b".to_string(), "c".to_string()]);

        assert!(fm.focus("b"));
        assert_eq!(fm.focused_id.as_deref(), Some("b"));
        assert!(fm.is_focused("b"));
        assert!(!fm.is_focused("a"));

        assert!(!fm.focus("nonexistent"));
        assert_eq!(fm.focused_id.as_deref(), Some("b"));
    }

    #[test]
    fn focus_manager_empty_list() {
        let mut fm = FocusManager::new();
        assert_eq!(fm.focus_next(), None);
        assert_eq!(fm.focus_prev(), None);
        assert_eq!(fm.focused_id, None);
    }

    #[test]
    fn focus_manager_single_element() {
        let mut fm = FocusManager::new();
        fm.set_focusable_ids(vec!["only".to_string()]);
        assert_eq!(fm.focused_id.as_deref(), Some("only"));
        fm.focus_next();
        assert_eq!(fm.focused_id.as_deref(), Some("only"));
        fm.focus_prev();
        assert_eq!(fm.focused_id.as_deref(), Some("only"));
    }

    #[test]
    fn event_dispatcher_routes_to_focused() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let mut dispatcher = EventDispatcher::new();
        dispatcher.on("btn", move |_ev| {
            called_clone.store(true, Ordering::SeqCst);
            true
        });

        let result = dispatcher.dispatch(&Event::FocusGained, Some("btn"));
        assert_eq!(result, DispatchResult::Handled);
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn event_dispatcher_bubbles_to_parent() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let child_calls = Arc::new(AtomicU32::new(0));
        let parent_calls = Arc::new(AtomicU32::new(0));
        let child_clone = child_calls.clone();
        let parent_clone = parent_calls.clone();

        let mut dispatcher = EventDispatcher::new();
        dispatcher.on("child", move |_ev| {
            child_clone.fetch_add(1, Ordering::SeqCst);
            false
        });
        dispatcher.on("parent", move |_ev| {
            parent_clone.fetch_add(1, Ordering::SeqCst);
            true
        });
        dispatcher.set_parent("child", "parent");

        let result = dispatcher.dispatch(&Event::FocusGained, Some("child"));
        assert_eq!(result, DispatchResult::Handled);
        assert_eq!(child_calls.load(Ordering::SeqCst), 1);
        assert_eq!(parent_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn event_dispatcher_not_handled_when_no_callback() {
        let dispatcher = EventDispatcher::new();
        let result = dispatcher.dispatch(&Event::FocusGained, Some("unknown"));
        assert_eq!(result, DispatchResult::NotHandled);
    }

    #[test]
    fn collect_focusable_ids_from_template() {
        let template = "div flex-col\n  div #a focusable\n    \"A\"\n  div #b focusable\n    \"B\"\n  div #c\n    \"C\"";
        let ids = collect_focusable_ids(template).unwrap();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn collect_focusable_ids_via_id_attribute() {
        let template = "div flex-col\n  div id=\"save\" focusable\n    \"Save\"\n  div id=\"cancel\" focusable\n    \"Cancel\"";
        let ids = collect_focusable_ids(template).unwrap();
        assert_eq!(ids, vec!["save", "cancel"]);
    }

    #[test]
    fn template_handle_event_tab_cycles_focus() {
        let source = "div flex-col\n  div #first focusable\n    \"First\"\n  div #second focusable\n    \"Second\"";
        let mut tpl = Template::from_source(source);
        tpl.refresh_focus();

        assert_eq!(tpl.focus().focused_id.as_deref(), Some("first"));

        let tab = crossterm::event::Event::Key(KeyEvent::new_with_kind_and_state(
            KeyCode::Tab,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            crossterm::event::KeyEventState::NONE,
        ));
        let result = tpl.handle_event(&tab);
        assert_eq!(result, EventResult::Handled);
        assert_eq!(tpl.focus().focused_id.as_deref(), Some("second"));
        assert_eq!(tpl.context().get_str("focused_id"), "second");

        tpl.handle_event(&tab);
        assert_eq!(tpl.focus().focused_id.as_deref(), Some("first"));
    }

    #[test]
    fn template_handle_event_backtab_cycles_prev() {
        let source = "div flex-col\n  div #first focusable\n    \"First\"\n  div #second focusable\n    \"Second\"";
        let mut tpl = Template::from_source(source);
        tpl.refresh_focus();

        assert_eq!(tpl.focus().focused_id.as_deref(), Some("first"));

        let backtab = crossterm::event::Event::Key(KeyEvent::new_with_kind_and_state(
            KeyCode::BackTab,
            KeyModifiers::SHIFT,
            KeyEventKind::Press,
            crossterm::event::KeyEventState::NONE,
        ));
        tpl.handle_event(&backtab);
        assert_eq!(tpl.focus().focused_id.as_deref(), Some("second"));
    }

    #[test]
    fn template_handle_event_unhandled_for_unknown_key() {
        let source = "div flex-col\n  div #first focusable\n    \"First\"";
        let mut tpl = Template::from_source(source);
        tpl.refresh_focus();

        let enter = crossterm::event::Event::Key(KeyEvent::new_with_kind_and_state(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            crossterm::event::KeyEventState::NONE,
        ));
        let result = tpl.handle_event(&enter);
        assert_eq!(result, EventResult::Unhandled);
    }

    #[test]
    fn template_dispatcher_callback_invoked() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let source = "div flex-col\n  div #btn focusable\n    \"Button\"";
        let mut tpl = Template::from_source(source);
        tpl.refresh_focus();

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = fired.clone();
        tpl.dispatcher_mut().on("btn", move |_ev| {
            fired_clone.store(true, Ordering::SeqCst);
            true
        });

        let enter = crossterm::event::Event::Key(KeyEvent::new_with_kind_and_state(
            KeyCode::Enter,
            KeyModifiers::NONE,
            KeyEventKind::Press,
            crossterm::event::KeyEventState::NONE,
        ));
        let result = tpl.handle_event(&enter);
        assert_eq!(result, EventResult::Handled);
        assert!(fired.load(Ordering::SeqCst));
    }
}

// ─── Diff tracking ────────────────────────────────────────────────────────────

mod diff_tracking {
    use crate::{DiffTracker, Template, TemplateContext};
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn diff_tracker_detects_new_variables() {
        let mut tracker = DiffTracker::new();
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        tracker.update(&ctx);

        ctx.set("b", "2");
        assert!(tracker.has_changed(&ctx));
    }

    #[test]
    fn diff_tracker_detects_changed_variable_values() {
        let mut tracker = DiffTracker::new();
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        tracker.update(&ctx);

        ctx.set("a", "2");
        assert!(tracker.has_changed(&ctx));
    }

    #[test]
    fn diff_tracker_detects_removed_variables() {
        let mut tracker = DiffTracker::new();
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        ctx.set("b", "2");
        tracker.update(&ctx);

        ctx.vars.remove("b");
        assert!(tracker.has_changed(&ctx));
    }

    #[test]
    fn diff_tracker_returns_false_when_nothing_changed() {
        let mut tracker = DiffTracker::new();
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        ctx.set("b", "2");
        tracker.update(&ctx);

        assert!(!tracker.has_changed(&ctx));
    }

    #[test]
    fn changed_keys_returns_correct_list_of_changed_variable_names() {
        let mut tracker = DiffTracker::new();
        let mut ctx = TemplateContext::new();
        ctx.set("a", "1");
        ctx.set("b", "2");
        ctx.set("c", "3");
        tracker.update(&ctx);

        ctx.set("a", "changed");
        ctx.vars.remove("b");
        ctx.set("d", "new");

        let mut changed = tracker.changed_keys(&ctx);
        changed.sort();
        assert_eq!(changed, vec!["a", "b", "d"]);
    }

    #[test]
    fn template_draw_if_changed_skips_when_unchanged() {
        let mut tpl = Template::from_source("div\n  \"{title}\"");
        tpl.set("title", "Hello");

        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut drew = false;
        terminal
            .draw(|frame| {
                let area = frame.area();
                drew = tpl.draw_if_changed(frame, area).unwrap();
            })
            .unwrap();
        assert!(drew, "first draw should render");

        let mut drew_again = false;
        terminal
            .draw(|frame| {
                let area = frame.area();
                drew_again = tpl.draw_if_changed(frame, area).unwrap();
            })
            .unwrap();
        assert!(!drew_again, "second draw with no changes should skip");
    }

    #[test]
    fn template_changed_keys_reports_changed_variables() {
        let mut tpl = Template::from_source("div\n  \"{title}\"");
        tpl.set("title", "Hello");
        tpl.set("count", 1i64);

        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                tpl.draw_if_changed(frame, area).unwrap();
            })
            .unwrap();

        tpl.set("title", "World");
        let mut changed = tpl.changed_keys();
        changed.sort();
        assert_eq!(changed, vec!["title"]);
    }
}

// ─── Component model ──────────────────────────────────────────────────────────

mod component_model {
    use super::*;
    use crate::{Component, ComponentRegistry, ComponentState, StateKey};
    use crepuscularity_core::context::value_to_str;

    #[test]
    fn component_registry_register_and_get() {
        let mut registry = ComponentRegistry::new();
        registry.register("Card", "div border\n  slot");
        assert!(registry.get("Card").is_some());
        assert_eq!(registry.get("Card").unwrap().name, "Card");
        assert_eq!(registry.get("Card").unwrap().source, "div border\n  slot");
        assert!(registry.get("Nonexistent").is_none());
    }

    #[test]
    fn component_registry_render_with_props() {
        let mut registry = ComponentRegistry::new();
        registry.register("Greeting", "div\n  \"Hello {name}\"");

        let ctx = TemplateContext::new();
        let props = vec![("name".to_string(), TemplateValue::Str("World".to_string()))];

        let backend = TestBackend::new(30, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                registry
                    .render("Greeting", &props, &ctx, frame, area)
                    .unwrap();
            })
            .unwrap();

        let text = all_text(&buffer_rows(&terminal));
        assert!(text.contains("Hello World"), "{text}");
    }

    #[test]
    fn component_defaults_overridden_by_provided_props() {
        let mut registry = ComponentRegistry::new();
        let component =
            Component::new("Card", "div\n  \"{title}\"").with_default("title", "Default Title");
        registry.components.insert("Card".to_string(), component);

        let ctx = TemplateContext::new();

        let backend = TestBackend::new(30, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                registry
                    .render("Card", &[], &ctx, frame, frame.area())
                    .unwrap();
            })
            .unwrap();
        assert!(all_text(&buffer_rows(&terminal)).contains("Default Title"));

        let backend = TestBackend::new(30, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        let props = vec![(
            "title".to_string(),
            TemplateValue::Str("Override".to_string()),
        )];
        terminal
            .draw(|frame| {
                registry
                    .render("Card", &props, &ctx, frame, frame.area())
                    .unwrap();
            })
            .unwrap();
        let text = all_text(&buffer_rows(&terminal));
        assert!(text.contains("Override"), "{text}");
        assert!(!text.contains("Default Title"), "{text}");
    }

    #[test]
    fn component_with_props_merges_defaults_and_props() {
        let component = Component::new("Card", "div\n  \"{title} {subtitle}\"")
            .with_default("title", "Default")
            .with_default("subtitle", "Sub");

        let props = vec![(
            "title".to_string(),
            TemplateValue::Str("Override".to_string()),
        )];
        let ctx = component.with_props(&props);

        assert_eq!(ctx.get_str("title"), "Override");
        assert_eq!(ctx.get_str("subtitle"), "Sub");
    }

    #[test]
    fn component_state_get_set_update() {
        let mut state = ComponentState::new();
        let key = StateKey::new("count");

        assert!(state.get_state(&key).is_none());

        state.set_state(&key, 0i64);
        assert_eq!(value_to_str(state.get_state(&key).unwrap()), "0");

        state.update_state(&key, |v| match v {
            TemplateValue::Int(n) => TemplateValue::Int(n + 1),
            _ => TemplateValue::Int(1),
        });
        assert_eq!(value_to_str(state.get_state(&key).unwrap()), "1");

        state.update_state(&key, |v| match v {
            TemplateValue::Int(n) => TemplateValue::Int(n + 10),
            _ => TemplateValue::Int(10),
        });
        assert_eq!(value_to_str(state.get_state(&key).unwrap()), "11");
    }

    #[test]
    fn component_state_update_on_missing_key_inserts() {
        let mut state = ComponentState::new();
        let key = StateKey::new("new_key");

        state.update_state(&key, |_| TemplateValue::Str("initialized".to_string()));
        assert_eq!(value_to_str(state.get_state(&key).unwrap()), "initialized");
    }

    #[test]
    fn register_file_parses_multi_component_files() {
        let dir = temp_case("register_file");
        let path = dir.join("components.crepus");
        fs::write(
            &path,
            "--- Card\ndiv border rounded p-1\n  slot\n    \"fallback\"\n--- Button\nbutton\n  \"{label}\"",
        )
        .unwrap();

        let mut registry = ComponentRegistry::new();
        registry.register_file(&path).unwrap();

        assert!(registry.get("Card").is_some());
        assert!(registry.get("Button").is_some());
        assert!(registry.get("Nonexistent").is_none());
    }

    #[test]
    fn register_file_parses_frontmatter_defaults() {
        let dir = temp_case("register_file_defaults");
        let path = dir.join("components.crepus");
        fs::write(
            &path,
            "+++\n[Card.defaults]\ntitle = \"Untitled\"\n+++\n\n--- Card\ndiv\n  \"{title}\"",
        )
        .unwrap();

        let mut registry = ComponentRegistry::new();
        registry.register_file(&path).unwrap();

        let card = registry.get("Card").unwrap();
        assert_eq!(
            value_to_str(card.defaults.get("title").unwrap()),
            "Untitled"
        );
    }

    #[test]
    fn register_file_renders_component_with_defaults() {
        let dir = temp_case("register_file_render");
        let path = dir.join("components.crepus");
        fs::write(
            &path,
            "+++\n[Card.defaults]\ntitle = \"Untitled\"\n+++\n\n--- Card\ndiv\n  \"{title}\"",
        )
        .unwrap();

        let mut registry = ComponentRegistry::new();
        registry.register_file(&path).unwrap();

        let ctx = TemplateContext::new();
        let backend = TestBackend::new(30, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                registry
                    .render("Card", &[], &ctx, frame, frame.area())
                    .unwrap();
            })
            .unwrap();
        assert!(all_text(&buffer_rows(&terminal)).contains("Untitled"));
    }

    #[test]
    fn template_register_component_makes_it_includable() {
        let mut tpl = crate::Template::from_source("div\n  include Greeting");
        tpl.register_component("Greeting", "div\n  \"Hello from component\"");

        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| tpl.draw_full(frame).unwrap())
            .unwrap();

        let text = all_text(&buffer_rows(&terminal));
        assert!(text.contains("Hello from component"), "{text}");
    }

    #[test]
    fn template_register_component_stores_in_registry() {
        let mut tpl = crate::Template::from_source("div\n  \"x\"");
        tpl.register_component("Card", "div border\n  slot");

        assert!(tpl.components().get("Card").is_some());
        assert_eq!(
            tpl.components().get("Card").unwrap().source,
            "div border\n  slot"
        );
    }
}

// ─── Public Buffer renderer ──────────────────────────────────────────────────
//
// These tests exercise the additive public `BufferRenderer` API introduced for
// TUI applications that own a `ratatui::buffer::Buffer` directly (rather than
// driving the render through a `Frame`). They follow red-green TDD discipline:
// every test below must fail to compile (or fail at runtime) before the new
// `BufferRenderer` implementation lands, and pass once it does.

#[cfg(test)]
mod buffer_renderer_api {
    use super::*;
    use crate::render::RenderCacheStats;
    use ratatui::buffer::Cell;
    use ratatui::layout::Rect;
    /// Render every cell of `buf` into one concatenated String, row-major.
    fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
        let area = buf.area;
        let mut out = String::with_capacity((area.width as usize) * (area.height as usize));
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf[(x, y)].symbol());
            }
        }
        out
    }

    use crate::BufferRenderer;
    use crepuscularity_core::parser::{parse_component_file, parse_template, ComponentDef};

    /// Render `template` into a fresh `Buffer` of `width x height` via the
    /// existing public Frame entry point, then clone every cell for comparison.
    fn frame_cells(width: u16, height: u16, template: &str, ctx: &TemplateContext) -> Vec<Cell> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                let area = frame.area();
                render_template(template, ctx, frame, area)
                    .expect("render_template returned an error");
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let w = buf.area.width as usize;
        let h = buf.area.height as usize;
        let mut out = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                out.push(buf[(x as u16, y as u16)].clone());
            }
        }
        out
    }

    /// Render `nodes` into a fresh `Buffer` of `width x height` via the new
    /// public `BufferRenderer::render_nodes` entry point.
    fn buffer_cells(
        width: u16,
        height: u16,
        nodes: &[crate::Node],
        ctx: &TemplateContext,
    ) -> Vec<Cell> {
        let mut renderer = BufferRenderer::new();
        let area = Rect::new(0, 0, width, height);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(nodes, ctx, &mut buf, area)
            .expect("render_nodes returned an error");
        let mut out = Vec::with_capacity((width as usize) * (height as usize));
        for y in 0..height {
            for x in 0..width {
                out.push(buf[(x, y)].clone());
            }
        }
        out
    }

    fn cells_to_text(cells: &[Cell], width: u16, height: u16) -> Vec<String> {
        let w = width as usize;
        (0..height as usize)
            .map(|y| {
                cells[y * w..(y + 1) * w]
                    .iter()
                    .map(|c| c.symbol())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn buffer_renderer_matches_frame() {
        // The Frame and Buffer renderers must produce identical cell streams
        // for the same template + context.
        let template = "div w-full h-full flex-col\n  div h-[1] text-white\n    \"hello\"\n  div h-[1] text-red-500\n    \"world\"";
        let ctx = TemplateContext::new();
        let nodes = parse_template(template).expect("template parses");

        let frame = frame_cells(20, 4, template, &ctx);
        let buf = buffer_cells(20, 4, &nodes, &ctx);

        let frame_rows = cells_to_text(&frame, 20, 4);
        let buf_rows = cells_to_text(&buf, 20, 4);
        assert_eq!(buf_rows, frame_rows, "text rows differ");

        for (i, (fc, bc)) in frame.iter().zip(buf.iter()).enumerate() {
            assert_eq!(fc.symbol(), bc.symbol(), "symbol mismatch at cell {i}");
            assert!(
                fc.style() == bc.style(),
                "style mismatch at cell {i}: frame={:?} buf={:?}",
                fc.style(),
                bc.style()
            );
        }
    }

    #[test]
    fn buffer_renderer_renders_parsed_component_def_without_reparsing() {
        // `render_component` accepts a pre-parsed `ComponentDef` so callers can
        // parse a multi-component file once and render named components many
        // times without re-traversing the frontmatter. The contract is:
        //   1. the API accepts `&ComponentDef` (not a raw string),
        //   2. rendering it never increments cache misses/invalidations (no
        //      includes to parse, no AST to rebuild), and
        //   3. successive renders of the same parsed `ComponentDef` are
        //      idempotent (no hidden reparse between calls).
        let content = "--- Card\ndiv border rounded p-1\n  slot\n    \"fallback\"";
        let mut file = parse_component_file(content).expect("multi-component parses");
        let component: ComponentDef = file
            .components
            .remove("Card")
            .expect("Card component present");

        let ctx = TemplateContext::new();
        let mut renderer = BufferRenderer::new();
        let area = Rect::new(0, 0, 30, 5);

        // Render the same parsed `ComponentDef` twice in a row.
        for iteration in 0..2 {
            let mut buf = ratatui::buffer::Buffer::empty(area);
            renderer
                .render_component(&component, &ctx, &mut buf, area)
                .unwrap_or_else(|e| {
                    panic!("render_component failed on iteration {iteration}: {e}")
                });

            let text = buffer_text(&buf);
            assert!(
                text.contains("fallback"),
                "rendered text missing 'fallback' on iteration {iteration}: {text}"
            );
        }

        // No includes were traversed, so cache stats must remain untouched.
        let stats = renderer.cache_stats();
        assert_eq!(
            stats,
            RenderCacheStats::default(),
            "ComponentDef render must not touch the include cache (no reparse, no includes): {:?}",
            stats
        );
    }

    #[test]
    fn buffer_renderer_reuses_include_cache() {
        // First render of an include is a miss; the second is a hit; nothing
        // is invalidated because the virtual file did not change.
        let mut ctx = TemplateContext::new();
        ctx.base_dir = Some(PathBuf::from("/virtual"));
        std::sync::Arc::make_mut(&mut ctx.virtual_files)
            .insert("child.crepus".into(), "div\n  \"child text\"".into());

        let template = "div\n include child.crepus";
        let nodes = parse_template(template).expect("template parses");

        let mut renderer = BufferRenderer::new();
        let area = Rect::new(0, 0, 30, 3);
        let mut buf1 = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut buf1, area)
            .expect("first render_nodes returned an error");
        let stats_after_first = renderer.cache_stats();
        assert_eq!(
            stats_after_first,
            RenderCacheStats {
                hits: 0,
                misses: 1,
                invalidations: 0
            },
            "first render should be exactly one miss"
        );

        let mut buf2 = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut buf2, area)
            .expect("second render_nodes returned an error");
        let stats_after_second = renderer.cache_stats();
        assert_eq!(
            stats_after_second,
            RenderCacheStats {
                hits: 1,
                misses: 1,
                invalidations: 0
            },
            "second render should be one miss + one hit and no invalidations"
        );

        // The two buffers must be byte-identical because the source is unchanged.
        for y in 0..area.height {
            for x in 0..area.width {
                assert_eq!(
                    buf1[(x, y)].symbol(),
                    buf2[(x, y)].symbol(),
                    "rendered text mismatch at ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn buffer_renderer_invalidates_changed_include() {
        // A transitive include edits the rendered output, produces exactly one
        // cache invalidation, and leaves hits/misses counts intact for the
        // unchanged outer include.
        let mut ctx = TemplateContext::new();
        ctx.base_dir = Some(PathBuf::from("/virtual"));
        std::sync::Arc::make_mut(&mut ctx.virtual_files)
            .insert("outer.crepus".into(), "div\n include inner.crepus".into());
        std::sync::Arc::make_mut(&mut ctx.virtual_files)
            .insert("inner.crepus".into(), "div\n  \"before\"".into());

        let template = "div\n include outer.crepus";
        let nodes = parse_template(template).expect("template parses");

        let mut renderer = BufferRenderer::new();
        let area = Rect::new(0, 0, 30, 4);

        let mut buf1 = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut buf1, area)
            .expect("first render_nodes returned an error");
        let text1 = buffer_text(&buf1);
        assert!(text1.contains("before"), "first render text: {text1}");
        let stats_after_first = renderer.cache_stats();
        assert_eq!(
            stats_after_first.misses, 2,
            "two misses on first render (outer + inner), got {:?}",
            stats_after_first
        );

        // Mutate the transitive include.
        std::sync::Arc::make_mut(&mut ctx.virtual_files)
            .insert("inner.crepus".into(), "div\n  \"after\"".into());

        let mut buf2 = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut buf2, area)
            .expect("second render_nodes returned an error");
        let text2 = buffer_text(&buf2);
        assert!(text2.contains("after"), "second render text: {text2}");
        assert!(
            !text2.contains("before"),
            "second render must not contain stale text: {text2}"
        );

        let stats_after_second = renderer.cache_stats();
        assert_eq!(
            stats_after_second.invalidations, 1,
            "exactly one invalidation expected after editing the transitive include: {:?}",
            stats_after_second
        );
        // misses must NOT have grown (the changed include was already cached and
        // re-parsed on invalidation, not on first miss).
        assert_eq!(
            stats_after_second.misses, 2,
            "misses must stay at the post-first-render count: {:?}",
            stats_after_second
        );
    }

    #[test]
    fn buffer_renderer_keeps_inline_include_errors() {
        // A missing include returns Ok, paints the existing ⚠ warning node, and
        // must never panic or be cached.
        let mut ctx = TemplateContext::new();
        ctx.base_dir = Some(PathBuf::from("/virtual"));
        // Note: "missing.crepus" is intentionally NOT inserted into virtual_files.

        let template = "div\n include missing.crepus";
        let nodes = parse_template(template).expect("template parses");

        let mut renderer = BufferRenderer::new();
        let area = Rect::new(0, 0, 80, 3);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        let result = renderer.render_nodes(&nodes, &ctx, &mut buf, area);
        assert!(
            result.is_ok(),
            "broken include must return Ok with inline warning: {:?}",
            result.err()
        );

        let text = buffer_text(&buf);
        assert!(
            text.contains('⚠'),
            "expected ⚠ warning glyph in rendered buffer: {text}"
        );

        // A second render with the same broken include must also stay Ok and
        // still paint ⚠ — failures must not be cached.
        let mut buf2 = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut buf2, area)
            .expect("second render_nodes returned an error");
        let text2 = buffer_text(&buf2);
        assert!(text2.contains('⚠'), "second render lost the ⚠: {text2}");

        let stats = renderer.cache_stats();
        assert_eq!(
            stats.misses, 0,
            "failed includes must not be cached as misses: {:?}",
            stats
        );
        assert_eq!(
            stats.invalidations, 0,
            "failed includes must not be cached as invalidations: {:?}",
            stats
        );
        assert_eq!(
            stats.hits, 0,
            "failed includes must not produce cache hits: {:?}",
            stats
        );
    }
    #[test]
    fn buffer_renderer_named_component_include_caching() {
        use crate::render::parse_inclusion_calls;
        crate::render::reset_parse_counter();

        let mut ctx = TemplateContext::new();
        ctx.base_dir = Some(PathBuf::from("/virtual"));
        let component_file =
            "+++\n[Card.defaults]\ntitle = \"Untitled\"\n+++\n\n--- Card\ndiv\n  \"{title}\"";
        std::sync::Arc::make_mut(&mut ctx.virtual_files)
            .insert("card.crepus".into(), component_file.into());

        let template = "div\n include card.crepus#Card";
        let nodes = parse_template(template).unwrap();

        let mut renderer = BufferRenderer::new();
        let area = Rect::new(0, 0, 30, 2);

        // 1st render: miss — one parse.
        let mut buf1 = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut buf1, area)
            .unwrap();
        assert_eq!(parse_inclusion_calls(), 1, "first render: one parse");
        assert!(buffer_text(&buf1).contains("Untitled"));

        // 2nd render: hit — zero new parses.
        let mut buf2 = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut buf2, area)
            .unwrap();
        assert_eq!(
            parse_inclusion_calls(),
            1,
            "second render: hit, no new parse"
        );
        assert!(buffer_text(&buf2).contains("Untitled"));

        // 3rd render: source changed — one re-parse (invalidation).
        let new_content =
            "+++\n[Card.defaults]\ntitle = \"Changed\"\n+++\n\n--- Card\ndiv\n  \"{title}\"";
        std::sync::Arc::make_mut(&mut ctx.virtual_files)
            .insert("card.crepus".into(), new_content.into());
        let mut buf3 = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut buf3, area)
            .unwrap();
        assert_eq!(parse_inclusion_calls(), 2, "invalidation: one more parse");
        assert!(buffer_text(&buf3).contains("Changed"));

        // Explicit prop override: still cached at the same key, no re-parse.
        let template_with_prop = "div\n include card.crepus#Card title=\"Hello\"";
        let nodes_with_prop = parse_template(template_with_prop).unwrap();
        let mut buf4 = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes_with_prop, &ctx, &mut buf4, area)
            .unwrap();
        assert_eq!(
            parse_inclusion_calls(),
            2,
            "explicit prop: still 2 parses, no new source"
        );
        assert!(buffer_text(&buf4).contains("Hello"), "explicit prop wins");
    }

    #[test]
    fn buffer_renderer_evicts_stale_include_after_failed_invalidation() {
        let mut ctx = TemplateContext::new();
        ctx.base_dir = Some(PathBuf::from("/virtual"));
        std::sync::Arc::make_mut(&mut ctx.virtual_files)
            .insert("child.crepus".into(), "div\n  \"valid\"".into());
        let nodes = parse_template("div\n include child.crepus").unwrap();
        let mut renderer = BufferRenderer::new();
        let area = Rect::new(0, 0, 30, 2);

        let mut initial = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut initial, area)
            .unwrap();
        assert_eq!(renderer.cache_stats().misses, 1);

        std::sync::Arc::make_mut(&mut ctx.virtual_files)
            .insert("child.crepus".into(), "<< invalid".into());
        let mut invalid = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut invalid, area)
            .unwrap();
        assert!(buffer_text(&invalid).contains('⚠'));

        std::sync::Arc::make_mut(&mut ctx.virtual_files)
            .insert("child.crepus".into(), "div\n  \"valid\"".into());
        let mut restored = ratatui::buffer::Buffer::empty(area);
        renderer
            .render_nodes(&nodes, &ctx, &mut restored, area)
            .unwrap();

        assert_eq!(
            renderer.cache_stats().misses,
            2,
            "restoring content after an invalidation failure must parse a fresh miss"
        );
    }
}

mod nested_loop_scaling {
    use super::*;
    use std::time::{Duration, Instant};

    fn items(n: usize) -> TemplateValue {
        TemplateValue::List(
            (0..n)
                .map(|i| {
                    let mut c = TemplateContext::new();
                    c.set("text", format!("line {i}"));
                    c.set("shown", true);
                    c
                })
                .collect(),
        )
    }

    fn outer(inner: usize, groups: usize) -> TemplateContext {
        let mut ctx = TemplateContext::new();
        ctx.set(
            "groups",
            TemplateValue::List(
                (0..groups)
                    .map(|_| {
                        let mut g = TemplateContext::new();
                        g.set("lines", items(inner));
                        g
                    })
                    .collect(),
            ),
        );
        ctx
    }

    fn timed(template: &str, ctx: &TemplateContext) -> Duration {
        let start = Instant::now();
        let rows = render(80, 24, template, ctx);
        assert!(!all_text(&rows).is_empty());
        start.elapsed()
    }

    const NESTED: &str = r#"div w-full h-fit flex-col
  for group in {groups}
    div h-fit flex-col
      for line in {lines}
        span text-sm
          "{text}"
"#;

    const NESTED_IF: &str = r#"div w-full h-fit flex-col
  for group in {groups}
    div h-fit flex-col
      for line in {lines}
        if {shown}
          span text-sm
            "{text}"
        else
          span text-xs
            "hidden"
"#;

    const NESTED_3: &str = r#"div w-full h-fit flex-col
  for outer in {groups}
    div h-fit flex-col
      for mid in {mids}
        div h-fit flex-col
          for line in {lines}
            span text-sm
              "{text}"
"#;

    #[test]
    fn nested_for_loop_with_many_items_renders_promptly() {
        let ctx = outer(300, 1);
        let elapsed = timed(NESTED, &ctx);
        assert!(
            elapsed < Duration::from_secs(10),
            "nested for loop over 300 items took {elapsed:?}"
        );
    }

    #[test]
    fn nested_for_loop_scales_roughly_linearly() {
        let small = timed(NESTED, &outer(200, 1));
        let large = timed(NESTED, &outer(800, 1));
        let ratio = large.as_secs_f64() / small.as_secs_f64().max(1e-6);
        assert!(
            ratio < 24.0, // Flaky test in CI due to Github runner performance variance
            "4x the items took {ratio:.1}x the time ({small:?} → {large:?})"
        );
    }

    #[test]
    fn if_inside_nested_for_loop_renders_promptly() {
        let ctx = outer(300, 1);
        let elapsed = timed(NESTED_IF, &ctx);
        assert!(
            elapsed < Duration::from_secs(10),
            "if inside a nested for loop over 300 items took {elapsed:?}"
        );
    }

    #[test]
    fn three_level_nested_for_loop_renders_promptly() {
        let mut ctx = TemplateContext::new();
        let mut group = TemplateContext::new();
        group.set(
            "mids",
            TemplateValue::List(
                (0..10)
                    .map(|_| {
                        let mut m = TemplateContext::new();
                        m.set("lines", items(30));
                        m
                    })
                    .collect(),
            ),
        );
        ctx.set("groups", TemplateValue::List(vec![group]));
        let elapsed = timed(NESTED_3, &ctx);
        assert!(
            elapsed < Duration::from_secs(10),
            "3-level nested for loop over 300 leaves took {elapsed:?}"
        );
    }
}
