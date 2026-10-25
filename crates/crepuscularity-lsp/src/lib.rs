//! Library half of the `crepus-lsp` server.
//!
//! Splitting the LSP into a library + binary means every behaviour (diagnostics,
//! completion, hover) is exercised by plain unit tests against
//! [`Diagnostic`] / [`CompletionItem`] / [`Hover`] without spinning up a full
//! `tower-lsp` runtime.

use crepuscularity_core::{diagnose_crepus_source, strip_indent_decorators, CrepusDiagnostic};
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Diagnostic, DiagnosticSeverity, Documentation, Hover,
    HoverContents, MarkupContent, MarkupKind, Position, Range,
};

pub fn crepus_diagnostics_to_lsp(source: &str) -> Vec<Diagnostic> {
    diagnose_crepus_source(source)
        .into_iter()
        .map(crepus_diagnostic_to_lsp)
        .collect()
}

fn diagnostic_position(line: u32, character: u32) -> Position {
    Position { line, character }
}

fn diagnostic_range(d: &CrepusDiagnostic) -> Range {
    Range {
        start: diagnostic_position(d.start_line, d.start_character),
        end: diagnostic_position(d.end_line, d.end_character),
    }
}

pub fn crepus_diagnostic_to_lsp(d: CrepusDiagnostic) -> Diagnostic {
    Diagnostic {
        range: diagnostic_range(&d),
        severity: Some(DiagnosticSeverity::ERROR),
        message: d.message,
        ..Diagnostic::default()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Completion
// ────────────────────────────────────────────────────────────────────────────

/// Tags built into Crepuscularity's DSL — control flow, includes, and slot
/// machinery. Surfaced by completion alongside the standard HTML tags.
const CREPUS_TAGS: &[(&str, &str)] = &[
    ("if", "Conditional render: `if {expr}`"),
    ("else", "Else branch of a preceding `if`."),
    ("else-if", "Chained `else if {expr}` branch."),
    ("for", "Iteration: `for item in {items}`."),
    (
        "match",
        "Pattern match: `match {expr}` followed by `pattern =>` arms.",
    ),
    (
        "include",
        "Inline another template/component: `include path/to.crepus prop=value`.",
    ),
    ("slot", "Render the caller's children inside a component."),
    (
        "slot-rotate",
        "Cycle through quoted phrase children on an `interval={ms}` timer.",
    ),
    (
        "$:",
        "Per-template variable declaration: `$: let foo = {expr}` / `$: default foo = value`.",
    ),
];

/// A small but representative slice of the Tailwind-style classes that the
/// project's renderers actually understand. Not exhaustive — the goal is to
/// give editors useful suggestions without bloating the LSP binary or pinning
/// us to a specific Tailwind version.
const CREPUS_CLASS_HINTS: &[&str] = &[
    // Layout
    "flex",
    "flex-row",
    "flex-col",
    "flex-1",
    "grid",
    "block",
    "inline",
    "inline-block",
    "hidden",
    "items-center",
    "items-start",
    "items-end",
    "justify-center",
    "justify-between",
    "justify-start",
    "justify-end",
    // Spacing
    "gap-1",
    "gap-2",
    "gap-4",
    "gap-8",
    "p-1",
    "p-2",
    "p-4",
    "p-6",
    "p-8",
    "px-2",
    "px-4",
    "py-2",
    "py-4",
    "m-2",
    "m-4",
    "mx-auto",
    // Sizing
    "w-full",
    "w-screen",
    "h-full",
    "h-screen",
    "min-h-0",
    "min-w-0",
    "max-w-md",
    "max-w-lg",
    "max-w-xl",
    "max-w-2xl",
    // Typography
    "text-sm",
    "text-base",
    "text-lg",
    "text-xl",
    "text-2xl",
    "text-3xl",
    "font-bold",
    "font-medium",
    "font-light",
    "leading-tight",
    "leading-relaxed",
    "uppercase",
    "lowercase",
    "capitalize",
    // Color (zinc/neutral palette stays useful across light/dark)
    "text-white",
    "text-black",
    "text-zinc-100",
    "text-zinc-300",
    "text-zinc-500",
    "text-zinc-700",
    "bg-white",
    "bg-black",
    "bg-zinc-50",
    "bg-zinc-900",
    "bg-zinc-950",
    "border",
    "rounded",
    "rounded-md",
    "rounded-lg",
    "shadow",
    "shadow-md",
    "shadow-lg",
    // Behavior
    "overflow-hidden",
    "overflow-y-scroll",
    "overflow-scroll",
];

/// What the cursor is currently sitting in. Drives which completion list we
/// surface — tags inside `<...`, classes after a tag name, variables inside
/// `{...}`, and so on.
#[derive(Debug, PartialEq, Eq)]
enum CompletionCtx {
    /// JSX-style tag name slot: cursor is just after `<` or partway into a
    /// tag identifier.
    JsxTag,
    /// Indent-syntax tag name slot: cursor is at the start of a non-empty,
    /// non-quoted line.
    IndentTag,
    /// Cursor is inside a class list (after the tag name on an indent line, or
    /// inside `class="…"` on a JSX tag).
    Classes,
    /// Cursor is between `{` and an unbalanced `}` — completing variable / let
    /// names from the document.
    Expression,
    /// Cursor is inside a quoted string with no enclosing `{` — no completion.
    QuotedString,
}

/// Compute completion items for `(source, position)`.
///
/// The matcher is intentionally lightweight: it only inspects the line under
/// the cursor for context, so it's cheap to invoke per-keystroke.
pub fn completion_items(source: &str, position: Position) -> Vec<CompletionItem> {
    let line = nth_line(source, position.line as usize).unwrap_or("");
    let prefix = line_prefix_up_to(line, position.character as usize);
    let ctx = detect_completion_context(prefix);

    match ctx {
        CompletionCtx::JsxTag | CompletionCtx::IndentTag => tag_completions(),
        CompletionCtx::Classes => class_completions(source),
        CompletionCtx::Expression => expression_completions(source),
        CompletionCtx::QuotedString => Vec::new(),
    }
}

fn detect_completion_context(prefix: &str) -> CompletionCtx {
    // Walk the prefix tracking brace + quote state.
    let mut in_string: Option<char> = None;
    let mut brace_depth: i32 = 0;
    for ch in prefix.chars() {
        match (in_string, ch) {
            (Some(q), c) if c == q => in_string = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => in_string = Some(ch),
            (None, '{') => brace_depth += 1,
            (None, '}') => brace_depth = (brace_depth - 1).max(0),
            _ => {}
        }
    }

    if brace_depth > 0 {
        return CompletionCtx::Expression;
    }
    if in_string.is_some() {
        if jsx_class_attribute_string(prefix) {
            return CompletionCtx::Classes;
        }
        return CompletionCtx::QuotedString;
    }

    let trimmed_start = prefix.trim_start();
    if trimmed_start.is_empty() {
        return CompletionCtx::IndentTag;
    }
    if trimmed_start.starts_with('<') {
        // Inside a JSX tag if no `>` has closed it on this line yet.
        if !trimmed_start.contains('>') {
            // Inside an attribute area only after the first space.
            if trimmed_start.contains(char::is_whitespace) {
                return CompletionCtx::Classes;
            }
            return CompletionCtx::JsxTag;
        }
    }

    // Indent syntax: the first whitespace after the tag name kicks off the
    // class list.
    if trimmed_start.contains(char::is_whitespace) {
        return CompletionCtx::Classes;
    }
    CompletionCtx::IndentTag
}

fn jsx_class_attribute_string(prefix: &str) -> bool {
    let trimmed = prefix.trim_start();
    if !trimmed.starts_with('<') || trimmed.contains('>') {
        return false;
    }

    let mut in_string: Option<char> = None;
    let mut quote_at: Option<usize> = None;
    for (i, ch) in prefix.char_indices() {
        match (in_string, ch) {
            (Some(q), c) if c == q => {
                in_string = None;
                quote_at = None;
            }
            (Some(_), _) => {}
            (None, '"' | '\'') => {
                in_string = Some(ch);
                quote_at = Some(i);
            }
            _ => {}
        }
    }
    let Some(qpos) = quote_at else {
        return false;
    };
    let before = prefix[..qpos].trim_end();
    let Some(name) = before.strip_suffix('=') else {
        return false;
    };
    let name = name
        .rsplit(|c: char| c.is_whitespace() || c == '<' || c == '/')
        .next()
        .unwrap_or("");
    name.eq_ignore_ascii_case("class") || name.eq_ignore_ascii_case("classname")
}

fn tag_completions() -> Vec<CompletionItem> {
    let html = [
        "div", "span", "p", "a", "h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "li", "section",
        "header", "footer", "nav", "main", "article", "aside", "button", "input", "form", "label",
        "img", "code", "pre", "table", "thead", "tbody", "tr", "td", "th", "br",
    ];
    let mut items: Vec<CompletionItem> = html
        .iter()
        .map(|t| CompletionItem {
            label: (*t).to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("HTML element".to_string()),
            ..Default::default()
        })
        .collect();
    for (tag, doc) in CREPUS_TAGS {
        items.push(CompletionItem {
            label: (*tag).to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Crepuscularity DSL".to_string()),
            documentation: Some(Documentation::String((*doc).to_string())),
            ..Default::default()
        });
    }
    items
}

fn class_completions(source: &str) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = CREPUS_CLASS_HINTS
        .iter()
        .map(|c| CompletionItem {
            label: (*c).to_string(),
            kind: Some(CompletionItemKind::ENUM_MEMBER),
            detail: Some("Tailwind utility".to_string()),
            ..Default::default()
        })
        .collect();

    // Class aliases declared at the bottom of the file are valid in the class
    // slot and surfacing them as completion items is the highest-value
    // local addition we can make.
    for (name, expansion) in document_class_aliases(source) {
        items.push(CompletionItem {
            label: name,
            kind: Some(CompletionItemKind::CLASS),
            detail: Some("Class alias (this file)".to_string()),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("Expands to: `{expansion}`"),
            })),
            ..Default::default()
        });
    }

    items
}

fn expression_completions(source: &str) -> Vec<CompletionItem> {
    document_let_bindings(source)
        .into_iter()
        .map(|name| CompletionItem {
            label: name,
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("local binding".to_string()),
            ..Default::default()
        })
        .collect()
}

// ────────────────────────────────────────────────────────────────────────────
// Hover
// ────────────────────────────────────────────────────────────────────────────

/// If the cursor is over a class-alias token (`.name` reference inside a class
/// list), return a hover popup showing what it expands to. Returns `None` for
/// plain tokens, tags, and other contexts so the editor falls through to its
/// default behaviour.
pub fn hover_for(source: &str, position: Position) -> Option<Hover> {
    let line = nth_line(source, position.line as usize)?;
    let token = token_at(line, position.character as usize)?;
    let aliases = document_class_aliases(source);

    if let Some(expansion) = aliases.iter().find(|(n, _)| n == token).map(|(_, v)| v) {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("**class alias** `.{token}` →\n\n```\n{expansion}\n```"),
            }),
            range: None,
        });
    }

    // Hover over a built-in tag: surface its DSL doc.
    if let Some((_, doc)) = CREPUS_TAGS.iter().find(|(n, _)| *n == token) {
        return Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("**`<{token}>`** — {doc}"),
            }),
            range: None,
        });
    }

    None
}

// ────────────────────────────────────────────────────────────────────────────
// Document scanning
// ────────────────────────────────────────────────────────────────────────────

fn document_class_aliases(source: &str) -> Vec<(String, String)> {
    // `strip_indent_decorators` already does the hard work of identifying the
    // alias block; we just materialize it as a stable-ordered Vec for UI
    // surfaces (HashMap doesn't iterate deterministically).
    let decorators = strip_indent_decorators(source);
    let mut aliases: Vec<(String, String)> = decorators.class_aliases.into_iter().collect();
    aliases.sort_by(|a, b| a.0.cmp(&b.0));
    aliases
}

/// Pull `$: let name = …` / `$: default name = …` and JSX `<let name="…" />` bindings out
/// of the document. Conservative — parses line-by-line so a malformed line
/// doesn't poison the whole list.
fn document_let_bindings(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in source.lines() {
        let trimmed = raw.trim_start();
        if let Some(rest) = trimmed.strip_prefix("$:") {
            let rest = rest.trim_start();
            let rest = rest
                .strip_prefix("let")
                .or_else(|| rest.strip_prefix("default"))
                .unwrap_or(rest);
            let rest = rest.trim_start();
            if let Some(eq) = rest.find('=') {
                let name = rest[..eq].trim();
                if is_ident(name) {
                    out.push(name.to_string());
                }
            }
        } else if let Some(rest) = trimmed.strip_prefix("<let") {
            // `<let name="x" value={…} />` — pull out the name attr.
            if let Some(name) = parse_let_name_attr(rest) {
                out.push(name);
            }
        } else if let Some(rest) = trimmed.strip_prefix("<let-default") {
            if let Some(name) = parse_let_name_attr(rest) {
                out.push(name);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn parse_let_name_attr(after_tag: &str) -> Option<String> {
    let idx = after_tag.find("name=")?;
    let after_eq = &after_tag[idx + "name=".len()..];
    let bytes = after_eq.as_bytes();
    let quote = match bytes.first()? {
        b'"' => b'"',
        b'\'' => b'\'',
        _ => return None,
    };
    let after_open = &after_eq[1..];
    let close = after_open.bytes().position(|b| b == quote)?;
    let candidate = &after_open[..close];
    is_ident(candidate).then(|| candidate.to_string())
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn nth_line(source: &str, line: usize) -> Option<&str> {
    source.lines().nth(line)
}

fn line_prefix_up_to(line: &str, character: usize) -> &str {
    let max = line
        .char_indices()
        .nth(character)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    &line[..max]
}

/// Token under `character` on `line`, treating identifier-class characters
/// (alphanumerics, `_`, and `-`, which appears in alias names like `slot-lede`)
/// as part of a single token. UTF-8-safe — walks `char_indices` rather than
/// raw bytes so multi-byte codepoints are not split mid-token.
fn token_at(line: &str, character: usize) -> Option<&str> {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    if chars.is_empty() {
        return None;
    }
    let cursor_idx = character.min(chars.len());
    let cursor_byte = chars
        .get(cursor_idx)
        .map(|(i, _)| *i)
        .unwrap_or_else(|| line.len());

    // Walk back to the start of the token by character.
    let mut start_char = cursor_idx;
    while start_char > 0 && is_token_char(chars[start_char - 1].1) {
        start_char -= 1;
    }
    let start_byte = chars
        .get(start_char)
        .map(|(i, _)| *i)
        .unwrap_or(cursor_byte);

    // Walk forward to the end of the token by character.
    let mut end_char = cursor_idx;
    while end_char < chars.len() && is_token_char(chars[end_char].1) {
        end_char += 1;
    }
    let end_byte = chars.get(end_char).map(|(i, _)| *i).unwrap_or(line.len());

    if start_byte == end_byte {
        return None;
    }
    Some(&line[start_byte..end_byte])
}

fn is_token_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '.'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsx_error_becomes_diagnostic_range() {
        let source = "<div ";
        let ds = crepus_diagnostics_to_lsp(source);
        assert!(!ds.is_empty());
        assert!(!ds[0].message.is_empty());
    }

    fn position(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    fn check_completions(source: &str, pos: Position, expected: &[&str], not_expected: &[&str]) {
        let items = completion_items(source, pos);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        for e in expected {
            assert!(
                labels.contains(e),
                "expected completion {:?} not found in {:?}",
                e,
                labels
            );
        }
        for n in not_expected {
            assert!(
                !labels.contains(n),
                "unexpected completion {:?} found in {:?}",
                n,
                labels
            );
        }
    }

    #[test]
    fn completion_at_indent_line_start_offers_tags() {
        check_completions(
            "",
            position(0, 0),
            &["div", "if", "for", "slot-rotate"],
            &[],
        );
    }

    #[test]
    fn completion_after_tag_offers_classes_and_local_aliases() {
        let source = "div \n.lede text-zinc-100 font-medium\n";
        check_completions(source, position(0, 4), &["flex", "text-white", "lede"], &[]);
    }

    #[test]
    fn completion_inside_braces_offers_local_let_bindings() {
        let source = "$: let total = {price * qty}\n$: default qty = 1\ndiv\n  {to";
        check_completions(source, position(3, 5), &["total", "qty"], &[]);
    }

    #[test]
    fn completion_inside_quoted_string_returns_nothing() {
        let source = "div\n  \"hello world";
        let items = completion_items(source, position(1, 14));
        assert!(items.is_empty());
    }

    #[test]
    fn jsx_tag_completion_offers_tags() {
        check_completions("<", position(0, 1), &["div", "if"], &[]);
    }

    #[test]
    fn jsx_partial_tag_still_offers_tags() {
        check_completions("<div", position(0, 4), &["div", "if"], &[]);
    }

    #[test]
    fn jsx_attribute_area_offers_classes() {
        check_completions("<div ", position(0, 5), &["flex", "text-white"], &["if"]);
    }

    #[test]
    fn indented_jsx_attribute_area_offers_classes() {
        let source = "  <button ";
        check_completions(source, position(0, 10), &["flex"], &["if"]);
    }

    #[test]
    fn jsx_class_attribute_offers_class_completions() {
        let source = r#"<div class=""#;
        check_completions(source, position(0, 12), &["flex", "text-white"], &[]);
    }

    #[test]
    fn jsx_classname_attribute_offers_class_completions() {
        let source = r#"<div className="tex"#;
        check_completions(source, position(0, 19), &["text-white"], &[]);
    }

    #[test]
    fn jsx_id_attribute_string_returns_nothing() {
        let items = completion_items(r#"<div id="fl"#, position(0, 11));
        assert!(items.is_empty());
    }

    #[test]
    fn hover_over_class_alias_shows_expansion() {
        let source = "div lede\n.lede text-zinc-100 font-medium\n";
        let hover = hover_for(source, position(0, 5)).expect("hover should be present");
        let HoverContents::Markup(content) = hover.contents else {
            panic!("expected markup hover")
        };
        assert!(content.value.contains("text-zinc-100"));
        assert!(content.value.contains("font-medium"));
    }

    #[test]
    fn hover_over_unknown_token_returns_none() {
        let source = "div text-white\n";
        assert!(hover_for(source, position(0, 8)).is_none());
    }

    #[test]
    fn hover_over_builtin_tag_shows_doc() {
        let source = "for item in {items}\n  div\n";
        let hover = hover_for(source, position(0, 1)).expect("hover should be present");
        let HoverContents::Markup(content) = hover.contents else {
            panic!()
        };
        assert!(content.value.to_lowercase().contains("iteration"));
    }

    #[test]
    fn token_at_handles_multibyte_text() {
        // The cursor walker indexes by *character* offsets, so multi-byte
        // characters before the cursor must not corrupt the token boundary.
        let line = "  héllo-world";
        let tok = token_at(line, 5).expect("expected a token");
        assert_eq!(tok, "héllo-world");
    }

    #[test]
    fn document_let_bindings_picks_up_jsx_let_tags() {
        let source = "<let name=\"foo\" value={1} />\n<let-default name=\"bar\" value=\"x\" />\n";
        let names = document_let_bindings(source);
        assert_eq!(names, vec!["bar".to_string(), "foo".to_string()]);
    }
}
