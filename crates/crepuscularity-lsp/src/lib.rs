//! Library half of the `crepus-lsp` server.
//!
//! Splitting the LSP into a library + binary means every behaviour (diagnostics,
//! completion, hover) is exercised by plain unit tests against
//! [`Diagnostic`] / [`CompletionItem`] / [`Hover`] without spinning up a full
//! `tower-lsp` runtime.

use crepuscularity_core::diagnostics::source_lines;
use crepuscularity_core::{
    diagnose_crepus_source_with_path, strip_indent_decorators, CrepusDiagnostic,
};
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, Diagnostic, DiagnosticSeverity, Documentation, Hover,
    HoverContents, MarkupContent, MarkupKind, Position, Range,
};

pub fn crepus_diagnostics_to_lsp(source: &str) -> Vec<Diagnostic> {
    crepus_diagnostics_to_lsp_with_path(source, None)
}

pub fn crepus_diagnostics_to_lsp_with_path(
    source: &str,
    path: Option<&std::path::Path>,
) -> Vec<Diagnostic> {
    diagnose_crepus_source_with_path(source, path)
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
    /// Literal content after a closed markup tag.
    MarkupText,
}

/// Compute completion items for `(source, position)`.
///
/// JSX documents retain preceding tag, quote, and expression context.
/// Indentation syntax uses the current line's context.
pub fn completion_items(source: &str, position: Position) -> Vec<CompletionItem> {
    completion_items_with_path(source, position, None)
}

/// Complete with an optional JSX filename hint, using the parser's detection rules.
pub fn completion_items_with_path(
    source: &str,
    position: Position,
    path: Option<&std::path::Path>,
) -> Vec<CompletionItem> {
    let Some((prefix, markup)) = context_prefix_at(source, position, path) else {
        return Vec::new();
    };
    let ctx = detect_completion_context_in(prefix, markup);

    match ctx {
        CompletionCtx::JsxTag | CompletionCtx::IndentTag => tag_completions(),
        CompletionCtx::Classes => class_completions(source),
        CompletionCtx::Expression => expression_completions(source),
        CompletionCtx::QuotedString | CompletionCtx::MarkupText => Vec::new(),
    }
}

#[cfg(test)]
fn detect_completion_context(prefix: &str) -> CompletionCtx {
    detect_completion_context_in(
        prefix,
        prefix
            .trim_start_matches('\u{feff}')
            .trim_start()
            .starts_with('<'),
    )
}

fn detect_completion_context_in(prefix: &str, markup_line: bool) -> CompletionCtx {
    // Walk the prefix tracking brace + quote state.
    let mut in_string: Option<char> = None;
    let mut brace_depth: i32 = 0;
    let mut escaped = false;
    let mut markup_tag_start = None;
    let mut comment_end = 0;
    for (byte, ch) in prefix.char_indices() {
        if byte < comment_end {
            continue;
        }
        if markup_line
            && in_string.is_none()
            && brace_depth == 0
            && prefix[byte..].starts_with("<!--")
        {
            comment_end = prefix[byte + 4..]
                .find("-->")
                .map_or(prefix.len(), |end| byte + 4 + end + 3);
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        match (in_string, ch) {
            (Some(_), '\\') => escaped = true,
            (Some(q), c) if c == q => in_string = None,
            (Some(_), _) => {}
            (None, '"' | '\'') if !markup_line || markup_tag_start.is_some() || brace_depth > 0 => {
                in_string = Some(ch);
            }
            (None, '<') if brace_depth == 0 => markup_tag_start = Some(byte + 1),
            (None, '>') if brace_depth == 0 => markup_tag_start = None,
            (None, '{') => brace_depth += 1,
            (None, '}') => brace_depth = (brace_depth - 1).max(0),
            _ => {}
        }
    }

    if brace_depth > 0 {
        return CompletionCtx::Expression;
    }
    if in_string.is_some() {
        if markup_tag_start.is_some_and(|start| jsx_class_attribute_string(&prefix[start - 1..])) {
            return CompletionCtx::Classes;
        }
        return CompletionCtx::QuotedString;
    }

    let trimmed_start = prefix.trim_start_matches('\u{feff}').trim_start();
    if trimmed_start.is_empty() {
        return CompletionCtx::IndentTag;
    }
    if markup_line {
        let Some(start) = markup_tag_start else {
            return CompletionCtx::MarkupText;
        };
        return if prefix[start..].contains(char::is_whitespace) {
            CompletionCtx::Classes
        } else {
            CompletionCtx::JsxTag
        };
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
    if !trimmed.starts_with('<') {
        return false;
    }

    let mut in_string: Option<char> = None;
    let mut quote_at: Option<usize> = None;
    let mut escaped = false;
    for (i, ch) in prefix.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match (in_string, ch) {
            (Some(_), '\\') => escaped = true,
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
        .trim_end()
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

/// Show alias expansions in class lists and documentation for built-in DSL tags.
/// Quoted literals, expressions, and markup body text do not receive these hovers.
pub fn hover_for(source: &str, position: Position) -> Option<Hover> {
    hover_for_with_path(source, position, None)
}

/// Hover with an optional JSX filename hint, using the parser's detection rules.
pub fn hover_for_with_path(
    source: &str,
    position: Position,
    path: Option<&std::path::Path>,
) -> Option<Hover> {
    let line = nth_line(source, position.line as usize)?;
    let (prefix, markup) = context_prefix_at(source, position, path)?;
    let context = detect_completion_context_in(prefix, markup);
    if matches!(
        context,
        CompletionCtx::Expression | CompletionCtx::QuotedString | CompletionCtx::MarkupText
    ) {
        return None;
    }
    let token = token_at(line, position.character as usize)?;
    let aliases = if context == CompletionCtx::Classes {
        document_class_aliases(source)
    } else {
        Vec::new()
    };

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
    if let Some((_, doc)) = CREPUS_TAGS.iter().find(|(n, _)| {
        matches!(context, CompletionCtx::IndentTag | CompletionCtx::JsxTag) && *n == token
    }) {
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
    for (_, raw) in source_lines(source) {
        let trimmed = raw.trim_start();
        if let Some(rest) = trimmed.strip_prefix("$:") {
            let rest = rest.trim_start();
            let Some(rest) = rest
                .strip_prefix("let ")
                .or_else(|| rest.strip_prefix("default "))
            else {
                continue;
            };
            let rest = rest.trim_start();
            if let Some(eq) = rest.find('=') {
                let name = rest[..eq].trim();
                if is_ident(name) {
                    out.push(name.to_string());
                }
            }
        } else if let Some(rest) = trimmed
            .strip_prefix("<let-default")
            .or_else(|| trimmed.strip_prefix("<let"))
            .filter(|rest| rest.starts_with(char::is_whitespace))
        {
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
    let mut quote = None;
    let mut escaped = false;
    let after_eq = after_tag.char_indices().find_map(|(idx, ch)| {
        if escaped {
            escaped = false;
            return None;
        }
        if let Some(delimiter) = quote {
            if ch == '\\' {
                escaped = true;
            } else if ch == delimiter {
                quote = None;
            }
            return None;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            return None;
        }
        let next = idx + ch.len_utf8();
        if !ch.is_whitespace() {
            return None;
        }
        after_tag[next..]
            .strip_prefix("name")?
            .trim_start()
            .strip_prefix('=')
            .map(str::trim_start)
    })?;
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

fn context_prefix_at<'a>(
    source: &'a str,
    position: Position,
    path: Option<&std::path::Path>,
) -> Option<(&'a str, bool)> {
    let source_without_bom = source.strip_prefix('\u{feff}').unwrap_or(source);
    let markup = crepuscularity_core::parser::is_jsx_mode(source_without_bom, path);
    for (number, (offset, line)) in source_lines(source).enumerate() {
        if number == position.line as usize {
            let prefix = line_prefix_up_to(line, position.character as usize);
            if !markup {
                if prefix
                    .trim_start_matches('\u{feff}')
                    .trim_start()
                    .starts_with('#')
                {
                    return None;
                }
                return Some((prefix, false));
            }
            let full = &source[..offset + prefix.len()];
            let full = full.strip_prefix('\u{feff}').unwrap_or(full);
            let body = crepuscularity_core::parser::skip_jsx_preamble(full);
            return if !body.is_empty() {
                Some((body, true))
            } else if prefix
                .trim_start_matches('\u{feff}')
                .trim_start()
                .starts_with('#')
            {
                None
            } else {
                Some((prefix, false))
            };
        }
    }
    None
}

fn nth_line(source: &str, line: usize) -> Option<&str> {
    source_lines(source).nth(line).map(|(_, text)| text)
}

fn line_prefix_up_to(line: &str, character: usize) -> &str {
    let mut remaining = character;
    for (byte, ch) in line.char_indices() {
        if remaining < ch.len_utf16() {
            return &line[..byte];
        }
        remaining -= ch.len_utf16();
    }
    line
}

/// Token under `character` on `line`, treating identifier-class characters
/// (alphanumerics, `_`, and `-`, which appears in alias names like `slot-lede`)
/// as part of a single token. Cursor offsets count UTF-16 units per LSP;
/// token boundaries stay on UTF-8 character boundaries.
fn token_at(line: &str, character: usize) -> Option<&str> {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    if chars.is_empty() {
        return None;
    }
    let cursor_byte = line_prefix_up_to(line, character).len();
    let cursor_idx = chars.partition_point(|(byte, _)| *byte < cursor_byte);

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
    fn all_protocol_line_endings_preserve_queries_and_diagnostic_ranges() {
        let path = Some(std::path::Path::new("view.jsx"));
        for ending in ["\n", "\r\n", "\r"] {
            let source = format!("<div>😀{ending}<for");
            assert!(
                completion_items_with_path(&source, position(1, 3), path)
                    .iter()
                    .any(|item| item.label == "for"),
                "{source:?}"
            );
            assert!(
                hover_for_with_path(&source, position(1, 2), path).is_some(),
                "{source:?}"
            );
            assert_eq!(
                document_let_bindings(&format!("<let name=\"foo\"/>{ending}<let name=\"bar\"/>")),
                ["bar", "foo"]
            );
            let broken = format!("<div>😀{ending}</wrong>");
            let diagnostics = crepus_diagnostics_to_lsp_with_path(&broken, path);
            assert_eq!(diagnostics.len(), 1, "{broken:?}");
            assert_eq!(diagnostics[0].range.start.line, 1, "{broken:?}");
        }
    }

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

    #[test]
    fn completion_at_indent_line_start_offers_tags() {
        let items = completion_items("", position(0, 0));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"div"));
        assert!(labels.contains(&"if"));
        assert!(labels.contains(&"for"));
        assert!(labels.contains(&"slot-rotate"));
    }

    #[test]
    fn indent_comments_do_not_offer_completions_or_alias_hovers() {
        for comment in ["# lede", "  # lede", "# {value", "\u{feff}# lede"] {
            let source = format!("{comment}\ndiv\n.lede text-red-500\n$: let value = 1");
            let after_hash =
                comment[..comment.find('#').unwrap()].encode_utf16().count() as u32 + 1;
            assert!(!completion_items(&source, position(0, after_hash - 1)).is_empty());
            for character in after_hash..=comment.encode_utf16().count() as u32 {
                assert!(
                    completion_items(&source, position(0, character)).is_empty(),
                    "{comment}"
                );
                assert!(
                    hover_for(&source, position(0, character)).is_none(),
                    "{comment}"
                );
            }
            assert!(!completion_items(&source, position(1, 2)).is_empty());
        }
        assert!(!completion_items("div #id ", position(0, 8)).is_empty());
        assert!(completion_items("<div># text", position(0, 11)).is_empty());
    }

    #[test]
    fn completion_after_tag_offers_classes_and_local_aliases() {
        let source = "div \n.lede text-zinc-100 font-medium\n";
        let items = completion_items(source, position(0, 4));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"flex"));
        assert!(labels.contains(&"text-white"));
        assert!(labels.contains(&"lede"), "missing class alias");
    }

    #[test]
    fn completion_inside_braces_offers_local_let_bindings() {
        let source = "$: let total = {price * qty}\n$: default qty = 1\ndiv\n  {to";
        let items = completion_items(source, position(3, 5));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"total"));
        assert!(labels.contains(&"qty"));
    }

    #[test]
    fn completion_inside_quoted_string_returns_nothing() {
        let source = "div\n  \"hello world";
        let items = completion_items(source, position(1, 14));
        assert!(items.is_empty());
    }

    #[test]
    fn jsx_tag_completion_offers_tags() {
        let items = completion_items("<", position(0, 1));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"div"));
        assert!(labels.contains(&"if"));
    }

    #[test]
    fn jsx_partial_tag_still_offers_tags() {
        let items = completion_items("<div", position(0, 4));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"div"));
        assert!(labels.contains(&"if"));
    }

    #[test]
    fn jsx_attribute_area_offers_classes() {
        let items = completion_items("<div ", position(0, 5));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"flex"));
        assert!(labels.contains(&"text-white"));
        assert!(
            !labels.contains(&"if"),
            "attribute slot should not offer tag completions"
        );
    }

    #[test]
    fn indented_jsx_attribute_area_offers_classes() {
        let source = "  <button ";
        let items = completion_items(source, position(0, 10));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"flex"));
        assert!(!labels.contains(&"if"));
    }

    #[test]
    fn jsx_class_attribute_offers_class_completions() {
        let source = r#"<div class=""#;
        let items = completion_items(source, position(0, 12));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"flex"), "got {labels:?}");
        assert!(labels.contains(&"text-white"));
    }

    #[test]
    fn jsx_classname_attribute_offers_class_completions() {
        let source = r#"<div className="tex"#;
        let items = completion_items(source, position(0, 19));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.contains(&"text-white"), "got {labels:?}");
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
    fn hover_ignores_literal_text_expressions_and_unrelated_attributes() {
        for line in [
            "text \"for\"",
            "text \"lede\"",
            "text {lede}",
            "<div title=\"lede\" />",
            "<div class=\"for\" />",
            "<div>for</div>",
            "<div> lede</div>",
            "<div>class=\"lede\"</div>",
        ] {
            let source = format!("{line}\n.lede text-red-500\n");
            let target = if line.contains("lede") { "lede" } else { "for" };
            let cursor = line.find(target).unwrap() as u32 + 1;
            assert!(hover_for(&source, position(0, cursor)).is_none(), "{line}");
        }
    }

    #[test]
    fn hover_keeps_alias_docs_in_jsx_class_attributes() {
        for name in ["class", "className"] {
            let line = format!("<div>don't</div><div {name}=\"lede\" />");
            let source = format!("{line}\n.lede text-red-500\n");
            assert!(
                hover_for(&source, position(0, line.find("lede").unwrap() as u32 + 1)).is_some()
            );
        }
    }

    #[test]
    fn markup_context_tracks_sibling_tags_and_quoted_angle_brackets() {
        for prefix in ["<div>body", "<div title=\">\">body", "<div>don't"] {
            assert_eq!(detect_completion_context(prefix), CompletionCtx::MarkupText);
            assert!(completion_items(prefix, position(0, prefix.len() as u32)).is_empty());
        }
        for prefix in ["<div>body<for", "<div title=\">\"><for", "<div>don't<for"] {
            assert_eq!(detect_completion_context(prefix), CompletionCtx::JsxTag);
            assert!(hover_for(prefix, position(0, prefix.len() as u32)).is_some());
        }
        assert_eq!(
            detect_completion_context("<div title=\">\" "),
            CompletionCtx::Classes
        );
    }

    #[test]
    fn multiline_markup_retains_attribute_and_body_context() {
        for newline in ["\n", "\r\n"] {
            let source = [
                "<div",
                "  class=\"lede\"",
                ">",
                "  for",
                "</div>",
                ".lede text-red-500",
            ]
            .join(newline);
            let items = completion_items(&source, position(1, 10));
            assert!(items.iter().any(|item| item.label == "lede"));
            assert!(hover_for(&source, position(1, 10)).is_some());
            assert!(hover_for(&source, position(3, 3)).is_none());
            assert!(completion_items(&source, position(3, 5)).is_empty());
        }
    }

    #[test]
    fn multiline_markup_keeps_quoted_attributes_and_utf16_positions() {
        let source = "<div title=\"😀\nfor\n\"\nclass=\"😀 lede\" >\n</div>\n.lede text-red-500";
        assert!(hover_for(source, position(1, 1)).is_none());
        assert!(completion_items(source, position(1, 2)).is_empty());
        assert!(hover_for(source, position(3, 12)).is_some());
        assert!(completion_items(source, position(3, 12))
            .iter()
            .any(|item| item.label == "lede"));
    }

    #[test]
    fn markup_trailing_newline_is_body_text_but_indent_starts_a_new_tag() {
        assert!(completion_items("<div>\n", position(1, 0)).is_empty());
        assert!(!completion_items("div\n", position(1, 0)).is_empty());
        assert!(completion_items("<div>", position(99, 0)).is_empty());
        assert!(hover_for("<div>", position(99, 0)).is_none());
        for source in [
            "<div>\n it's fine\n</div>\n<for",
            "<!-- \"{\n -->\n<for",
            "\u{feff}<div>\n</div>\n<for",
        ] {
            let line = source.lines().count() as u32 - 1;
            assert!(hover_for(source, position(line, 3)).is_some(), "{source}");
            assert!(!completion_items(source, position(line, 3)).is_empty());
        }
        assert!(completion_items("<!-- \"{\n still a comment", position(1, 5)).is_empty());
    }

    #[test]
    fn jsx_filename_hints_distinguish_partial_body_text_from_indent_tags() {
        let source = "for";
        assert!(hover_for(source, position(0, 1)).is_some());
        for filename in ["view.jsx", "view.TSX", "view.csx"] {
            let path = Some(std::path::Path::new(filename));
            assert!(hover_for_with_path(source, position(0, 1), path).is_none());
            assert!(completion_items_with_path(source, position(0, 3), path).is_empty());
            assert!(!completion_items_with_path("<for", position(0, 3), path).is_empty());
            assert!(hover_for_with_path("<for", position(0, 2), path).is_some());
        }
        assert!(hover_for_with_path(
            source,
            position(0, 1),
            Some(std::path::Path::new("view.crepus"))
        )
        .is_some());
    }

    #[test]
    fn jsx_context_skips_preamble_comments_without_losing_declaration_context() {
        let source = "# comment with { and '😀\n$: let value = 1\n<div\n class=\"lede\"\n>\nfor\n</div>\n.lede text-red-500";
        assert!(hover_for(source, position(5, 1)).is_none());
        assert!(hover_for(source, position(3, 10)).is_some());
        assert!(completion_items(source, position(3, 10))
            .iter()
            .any(|item| item.label == "lede"));
        assert!(completion_items(source, position(0, 12)).is_empty());
        let declaration = "$: let before = 1\n$: let after = {before\n<div/>";
        assert!(completion_items(declaration, position(1, 24))
            .iter()
            .any(|item| item.label == "before"));
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
    fn cursor_positions_count_utf16_units() {
        let line = "😀 slot-lede next";
        assert_eq!(line_prefix_up_to(line, 1), "");
        assert_eq!(line_prefix_up_to(line, 2), "😀");
        assert_eq!(line_prefix_up_to(line, 12), "😀 slot-lede");
        assert_eq!(token_at(line, 12), Some("slot-lede"));
        assert_eq!(token_at(line, 14), Some("next"));
    }

    #[test]
    fn token_at_handles_multibyte_text() {
        // BMP characters occupy one UTF-16 unit even when UTF-8 uses multiple bytes.
        let line = "  héllo-world";
        let tok = token_at(line, 5).expect("expected a token");
        assert_eq!(tok, "héllo-world");
    }

    #[test]
    fn jsx_class_completion_accepts_spaces_around_equals() {
        for prefix in [r#"<div class = "fl"#, r#"<div className  =  'fl"#] {
            assert_eq!(detect_completion_context(prefix), CompletionCtx::Classes);
        }
        assert_eq!(
            detect_completion_context(r#"<div data-class = "fl"#),
            CompletionCtx::QuotedString
        );
    }

    #[test]
    fn quoted_angle_brackets_do_not_close_jsx_class_completion() {
        assert_eq!(
            detect_completion_context(r#"<div title=">" class="fl"#),
            CompletionCtx::Classes
        );
        assert_eq!(
            detect_completion_context(r#"<div>text "literal"#),
            CompletionCtx::MarkupText
        );
    }

    #[test]
    fn escaped_quotes_keep_completion_inside_literal_text() {
        let line = r#"text "say \"{"#;
        let source = format!("$: let local = 1\n{line}");
        assert!(completion_items(&source, position(1, line.len() as u32)).is_empty());
        assert_eq!(
            detect_completion_context(r#"text "\\" {"#),
            CompletionCtx::Expression
        );
        assert_eq!(
            detect_completion_context(r#"<div class="say \"x"#),
            CompletionCtx::Classes
        );
    }

    #[test]
    fn directive_binding_keywords_require_a_boundary() {
        let names = document_let_bindings(
            "$: letter = 1\n$: defaulted = 2\n$: ghost = 3\n$: let letter = 4\n$: default defaulted = 5\n"
        );
        assert_eq!(names, vec!["defaulted", "letter"]);
    }

    #[test]
    fn escaped_quotes_do_not_expose_binding_attributes_in_text() {
        assert_eq!(
            document_let_bindings(r#"<let title="say \" name='ghost'" name="actual" />"#),
            vec!["actual"]
        );
    }

    #[test]
    fn jsx_binding_names_accept_spaces_around_equals() {
        assert_eq!(
            document_let_bindings("<let name = \"foo\" />\n<let-default name  =  'bar' />\n"),
            vec!["bar", "foo"]
        );
    }

    #[test]
    fn jsx_binding_names_require_an_attribute_boundary() {
        let names = document_let_bindings(
            "<let surname=\"ghost\" />\n<let data-name=\"other\" name=\"actual\" />\n<let title=\" name='hidden'\" />\n"
        );
        assert_eq!(names, vec!["actual"]);
    }

    #[test]
    fn jsx_tags_with_let_prefix_do_not_create_bindings() {
        assert!(document_let_bindings(
            "<letter name=\"ghost\" />\n<let-defaulted name=\"other\" />\n"
        )
        .is_empty());
    }

    #[test]
    fn document_let_bindings_picks_up_jsx_let_tags() {
        let source = "<let name=\"foo\" value={1} />\n<let-default name=\"bar\" value=\"x\" />\n";
        let names = document_let_bindings(source);
        assert_eq!(names, vec!["bar".to_string(), "foo".to_string()]);
    }
}
