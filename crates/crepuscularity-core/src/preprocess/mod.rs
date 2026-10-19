//! Indent-syntax decorators: top-of-file Google Font pragmas and trailing `.alias` class shortcuts.
//!
//! Font pragmas (only at the top of the file, before real template lines):
//! - `google-font Inter` or `google-font: Inter` — one family, unquoted (spaces allowed).
//! - `google-font "Inter"` — one family, quoted (use quotes when the name has edge cases).
//! - `google-fonts "Inter" "JetBrains Mono"` — several families in one line (each must be quoted).

use std::collections::HashMap;

pub mod aliases;
pub mod css_tail;
pub mod fonts;
pub mod slot;

pub use aliases::{
    expand_class_aliases_in_nodes, expand_class_list_in_place, expand_class_token,
    expand_class_token_owned,
};
pub use fonts::{
    google_font_css_family_name, google_fonts_head_markup, merge_unique_font_families,
};
pub use slot::{slot_rotate_child_phrases, slot_rotate_words_json_attr};

/// Result of stripping indent-only decorators before parse.
#[derive(Debug, Clone)]
pub struct IndentDecorators {
    /// Source with pragma lines removed (ready for `collect_lines` / `parse_template`).
    pub body: String,
    /// Google Font family names (e.g. `"Inter"`, `"JetBrains Mono"`).
    pub google_fonts: Vec<String>,
    /// Maps shortcut name (without leading dot) → expanded utility string.
    pub class_aliases: HashMap<String, String>,
    /// Raw CSS collected from trailing style blocks / CSS tails.
    pub inline_css: String,
}

/// Strip `google-font` / `google-fonts` lines from the top and `.name tokens…` alias lines from the bottom.
/// JSX mode templates are returned unchanged (no stripping).
pub fn strip_indent_decorators(raw: &str) -> IndentDecorators {
    let lines: Vec<&str> = raw.lines().collect();
    if lines.is_empty() {
        return IndentDecorators {
            body: raw.to_string(),
            google_fonts: Vec::new(),
            class_aliases: HashMap::new(),
            inline_css: String::new(),
        };
    }

    let (i, google_fonts) = fonts::strip_google_font_pragmas(&lines);
    let (end, class_aliases) = aliases::strip_class_aliases(&lines, i, lines.len());
    let (end, inline_css) = css_tail::strip_trailing_inline_css(&lines, i, end);
    let body = {
        let slice = &lines[i..end];
        let mut s = String::with_capacity(slice.iter().map(|l| l.len() + 1).sum());
        for (idx, line) in slice.iter().enumerate() {
            if idx > 0 {
                s.push('\n');
            }
            s.push_str(line);
        }
        s
    };
    IndentDecorators {
        body,
        google_fonts,
        class_aliases,
        inline_css,
    }
}

/// If the template starts with a `head` block at indent zero, extract its indented
/// children as raw head content and return the remaining body. Returns `(head_raw, body_raw)`.
///
/// ```crepus
/// head
///   title "Notes"
///   meta charset="utf-8"
///   link rel="icon" href="./static/favicon.svg"
///
/// div wrap
///   ...
/// ```
pub fn extract_head_block(raw: &str) -> (Option<String>, String) {
    let lines: Vec<&str> = raw.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }
        let trimmed_lower = trimmed.to_lowercase();
        if trimmed_lower == "head" || trimmed_lower.starts_with("head ") {
            let head_indent = lines[i].len() - lines[i].trim_start().len();
            let mut j = i + 1;
            let mut head_lines: Vec<&str> = Vec::new();
            while j < lines.len() {
                let line = lines[j];
                if line.trim().is_empty() {
                    head_lines.push("");
                    j += 1;
                    continue;
                }
                let line_indent = line.len() - line.trim_start().len();
                if line_indent > head_indent {
                    let dedented = line
                        .get((head_indent + 2).min(line_indent)..)
                        .unwrap_or_else(|| line.trim_start());
                    head_lines.push(dedented);
                    j += 1;
                } else {
                    break;
                }
            }
            let head_raw = if head_lines.is_empty() {
                String::new()
            } else {
                head_lines.join("\n")
            };
            let body = if j < lines.len() {
                let mut body_lines: Vec<&str> = Vec::new();
                body_lines.push("# head block removed"); // marker so parser knows this is the body
                body_lines.extend(&lines[j..]);
                body_lines.join("\n")
            } else {
                String::new()
            };
            return (
                if head_raw.is_empty() {
                    None
                } else {
                    Some(head_raw)
                },
                body,
            );
        }
        break;
    }
    (None, raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_class_token() {
        let mut aliases = HashMap::new();
        aliases.insert(
            "center".to_string(),
            "items-center justify-center flex".to_string(),
        );
        aliases.insert("btn".to_string(), "bg-blue-500".to_string());
        aliases.insert("empty".to_string(), "".to_string());
        aliases.insert("whitespace".to_string(), "   ".to_string());

        // Existing alias mapping to multiple tokens
        assert_eq!(
            expand_class_token("center", &aliases),
            vec![
                "items-center".to_string(),
                "justify-center".to_string(),
                "flex".to_string()
            ]
        );

        // Existing alias mapping to a single token
        assert_eq!(
            expand_class_token("btn", &aliases),
            vec!["bg-blue-500".to_string()]
        );

        // Existing alias mapping to empty string or whitespace
        let empty_vec: Vec<String> = vec![];
        assert_eq!(expand_class_token("empty", &aliases), empty_vec);
        assert_eq!(expand_class_token("whitespace", &aliases), empty_vec);

        // Token that does not exist in the aliases map
        assert_eq!(
            expand_class_token("unknown", &aliases),
            vec!["unknown".to_string()]
        );

        // Empty token not in the aliases map
        assert_eq!(expand_class_token("", &aliases), vec!["".to_string()]);
    }

    #[test]
    fn extract_head_block_preserves_content_with_short_indentation() {
        for indent in [" ", "\t", "\u{2003}", "  "] {
            let source = format!("head\n{indent}éclair\ndiv");
            assert_eq!(extract_head_block(&source).0.as_deref(), Some("éclair"));
        }
    }

    #[test]
    fn extract_head_block_basic() {
        let input = "head\n  title \"Notes\"\n  meta charset=\"utf-8\"\n\ndiv wrap\n  \"content\"";
        let (head, body) = extract_head_block(input);
        assert_eq!(head.unwrap(), "title \"Notes\"\nmeta charset=\"utf-8\"\n");
        assert_eq!(body, "# head block removed\ndiv wrap\n  \"content\"");
    }

    #[test]
    fn extract_head_block_empty_head() {
        let input = "head\ndiv wrap\n  \"content\"";
        let (head, body) = extract_head_block(input);
        assert_eq!(head, None);
        assert_eq!(body, "# head block removed\ndiv wrap\n  \"content\"");
    }

    #[test]
    fn extract_head_block_no_head() {
        let input = "div wrap\n  \"content\"\n  p \"hello\"";
        let (head, body) = extract_head_block(input);
        assert_eq!(head, None);
        assert_eq!(body, "div wrap\n  \"content\"\n  p \"hello\"");
    }

    #[test]
    fn extract_head_block_with_comments_before() {
        let input = "# a comment\n\nhead\n  title \"Notes\"\n\ndiv wrap";
        let (head, body) = extract_head_block(input);
        assert_eq!(head.unwrap(), "title \"Notes\"\n");
        assert_eq!(body, "# head block removed\ndiv wrap");
    }

    #[test]
    fn extract_head_block_only_head() {
        let input = "head\n  title \"Notes\"";
        let (head, body) = extract_head_block(input);
        assert_eq!(head.unwrap(), "title \"Notes\"");
        assert_eq!(body, "");
    }

    #[test]
    fn extract_head_block_nested_indent() {
        let input = "head\n  script\n    \"console.log('hi');\"\n\ndiv wrap";
        let (head, body) = extract_head_block(input);
        assert_eq!(head.unwrap(), "script\n  \"console.log('hi');\"\n");
        assert_eq!(body, "# head block removed\ndiv wrap");
    }

    #[test]
    fn strips_fonts_and_aliases() {
        let s = r#"google-font Inter
google-font JetBrains Mono

div center
  "hi"
.center items-center justify-center flex
.body-text text-sm text-black
"#;
        let d = strip_indent_decorators(s);
        assert_eq!(d.google_fonts, vec!["Inter", "JetBrains Mono"]);
        assert_eq!(
            d.class_aliases.get("center").map(String::as_str),
            Some("items-center justify-center flex")
        );
        assert!(d.body.contains("div center"));
        assert!(!d.body.contains("google-font"));
        assert!(!d.body.contains(".center"));
        assert!(d.inline_css.is_empty());
    }

    #[test]
    fn google_fonts_one_line_quoted() {
        let s = r#"google-fonts "Inter" "JetBrains Mono"

div
  "x"
"#;
        let d = strip_indent_decorators(s);
        assert_eq!(d.google_fonts, vec!["Inter", "JetBrains Mono"]);
    }

    #[test]
    fn google_font_quoted_single() {
        let s = "google-font \"IBM Plex Sans\"\ndiv\n";
        let d = strip_indent_decorators(s);
        assert_eq!(d.google_fonts, vec!["IBM Plex Sans"]);
    }

    #[test]
    fn strips_trailing_style_block_into_inline_css() {
        let s = r#"div p-4
  "hello"
<style>
  @keyframes sunset {
    0% { opacity: .6; }
    100% { opacity: 1; }
  }
</style>
"#;
        let d = strip_indent_decorators(s);
        assert!(d.body.contains("div p-4"));
        assert!(!d.body.contains("<style>"));
        assert!(d.inline_css.contains("@keyframes sunset"));
    }

    #[test]
    fn strips_trailing_raw_css_tail() {
        let s = r#"div
  "x"
@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}
.animate-fade-in {
  animation: fade-in 1s ease-in-out;
}
"#;
        let d = strip_indent_decorators(s);
        assert_eq!(d.body.trim(), "div\n  \"x\"");
        assert!(d.inline_css.contains(".animate-fade-in"));
    }

    #[test]
    fn strips_trailing_css_blocks_separated_by_blank_lines() {
        // Regression: multiple CSS blocks separated by blank lines must all be
        // stripped, not just the last block.
        let s = r#"div
  "x"
@keyframes sunset {
  0% { opacity: .5; }
  100% { opacity: 1; }
}
.animate-sunset {
  animation: sunset 24s ease-in-out infinite;
}

@media (prefers-reduced-motion: reduce) {
  .animate-sunset {
    animation: none;
  }
}
"#;
        let d = strip_indent_decorators(s);
        assert_eq!(d.body.trim(), "div\n  \"x\"");
        assert!(d.inline_css.contains("@keyframes sunset"));
        assert!(d.inline_css.contains(".animate-sunset"));
        assert!(d
            .inline_css
            .contains("@media (prefers-reduced-motion: reduce)"));
    }

    #[test]
    fn google_fonts_head_markup_smoke() {
        let s = google_fonts_head_markup(&["JetBrains Mono".into(), "Inter".into()]);
        assert!(s.contains("fonts.googleapis.com"));
        assert!(s.contains("JetBrains+Mono"));
        assert!(s.contains("family=Inter"));
    }

    #[test]
    fn google_fonts_head_markup_supports_material_symbols_shorthand() {
        let s = google_fonts_head_markup(&["Material Symbols Outlined".into()]);
        assert!(s.contains("Material+Symbols+Outlined:opsz,wght,FILL,GRAD@24,400,0,0"));
        assert!(!s.contains("Material+Symbols+Outlined:wght@400"));
    }

    #[test]
    fn google_fonts_head_markup_preserves_axis_suffix() {
        let s = google_fonts_head_markup(&[
            "Inter".into(),
            "Material Symbols Rounded:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200".into(),
        ]);
        assert!(s.contains("family=Inter:wght@400;500;600;700"));
        assert!(s.contains(
            "family=Material+Symbols+Rounded:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200"
        ));
    }

    #[test]
    fn does_not_strip_trailing_text_with_interpolation() {
        let s = "div w-full h-full flex-col\n  div\n    \"Hello {name}\"\n";
        let d = strip_indent_decorators(s);
        assert!(
            d.body.contains("Hello {name}"),
            "trailing text node was stripped: body={:?} css={:?}",
            d.body,
            d.inline_css
        );
        assert!(d.inline_css.is_empty(), "css={:?}", d.inline_css);
    }

    #[test]
    fn does_not_strip_trailing_bare_expression() {
        let s = "div\n  {score}\n";
        let d = strip_indent_decorators(s);
        assert!(
            d.body.contains("{score}"),
            "bare expression was stripped: body={:?} css={:?}",
            d.body,
            d.inline_css
        );
        assert!(d.inline_css.is_empty());
    }

    #[test]
    fn does_not_strip_trailing_let_decl() {
        let s = "div\n  $: let total = {price * qty}\n";
        let d = strip_indent_decorators(s);
        assert!(
            d.body.contains("$: let total"),
            "$: let was stripped: body={:?} css={:?}",
            d.body,
            d.inline_css
        );
        assert!(d.inline_css.is_empty());
    }

    #[test]
    fn still_strips_at_rule_tail_with_interpolation_above() {
        let s = "div\n  \"score: {score}\"\n@keyframes pulse {\n  0% { opacity: .5; }\n  100% { opacity: 1; }\n}\n";
        let d = strip_indent_decorators(s);
        assert!(d.body.contains("score: {score}"), "body={:?}", d.body);
        assert!(d.inline_css.contains("@keyframes pulse"));
    }

    #[test]
    fn does_not_strip_trailing_element_with_binding() {
        let s = "div bind:href={url}\n";
        let d = strip_indent_decorators(s);
        assert!(d.body.contains("bind:href={url}"), "body={:?}", d.body);
        assert!(d.inline_css.is_empty());
    }

    #[test]
    fn preserves_attribute_blocks_with_rust_paths_and_semicolons() {
        for line in [
            "div @keydown|ctrl+s={move |_, _, _| { hits.fetch_add(1, Ordering::SeqCst); }}",
            "div bind:href={String::from(\"url\"); url}",
        ] {
            for tail in ["", "\nbutton[data-kind=primary] { color: red; }"] {
                let d = strip_indent_decorators(&format!("{line}{tail}"));
                assert_eq!(d.body, line);
                assert_eq!(d.inline_css, tail.trim());
            }
        }
    }

    #[test]
    fn does_not_strip_trailing_class_binding() {
        let s = "div\n  span class:active={selected}\n";
        let d = strip_indent_decorators(s);
        assert!(
            d.body.contains("class:active={selected}"),
            "body={:?}",
            d.body
        );
        assert!(d.inline_css.is_empty());
    }

    #[test]
    fn strips_css_after_trailing_binding_line_without_blank_separator() {
        // Regression: a bound element on the last template line directly
        // followed by a CSS `@keyframes` block (no blank line in between)
        // must keep the binding in the body and strip only the CSS.
        let s = "div bind:href={url}\n@keyframes pulse {\n  0% { opacity: .5; }\n  100% { opacity: 1; }\n}\n";
        let d = strip_indent_decorators(s);
        assert!(d.body.contains("bind:href={url}"), "body={:?}", d.body);
        assert!(
            d.inline_css.contains("@keyframes pulse"),
            "css={:?}",
            d.inline_css
        );
        assert!(
            !d.body.contains("@keyframes"),
            "css leaked into body: body={:?}",
            d.body
        );
    }

    #[test]
    fn does_not_strip_trailing_match_header() {
        let s = "div\n  match {status}\n    \"a\" =>\n      div\n        \"A\"\n";
        let d = strip_indent_decorators(s);
        assert!(d.body.contains("match {status}"), "body={:?}", d.body);
        assert!(d.inline_css.is_empty());
    }

    #[test]
    fn does_not_strip_trailing_for_header() {
        let s = "div\n  for item in {items}\n    div\n      {item}\n";
        let d = strip_indent_decorators(s);
        assert!(d.body.contains("for item in {items}"), "body={:?}", d.body);
        assert!(d.inline_css.is_empty());
    }

    #[test]
    fn css_only_file_without_template_placeholder() {
        let s = ".foo {\n  color: red;\n}\n.bar {\n  color: blue;\n}\n";
        let d = strip_indent_decorators(s);
        assert!(
            d.body.trim().is_empty(),
            "body should be empty, got {:?}",
            d.body
        );
        assert!(
            d.inline_css.contains(".foo"),
            "css missing .foo: {:?}",
            d.inline_css
        );
        assert!(
            d.inline_css.contains(".bar"),
            "css missing .bar: {:?}",
            d.inline_css
        );
    }

    #[test]
    fn test_google_font_css_family_name() {
        assert_eq!(super::google_font_css_family_name("Inter"), "Inter");
        assert_eq!(
            super::google_font_css_family_name("JetBrains Mono:wght@400;700"),
            "JetBrains Mono"
        );
        assert_eq!(
            super::google_font_css_family_name("Roboto:ital,wght@0,400;1,700"),
            "Roboto"
        );
        assert_eq!(
            super::google_font_css_family_name(
                "Material Symbols Outlined:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200"
            ),
            "Material Symbols Outlined"
        );
        assert_eq!(super::google_font_css_family_name("  Inter  "), "Inter");
        assert_eq!(
            super::google_font_css_family_name("  Inter : wght@400  "),
            "Inter"
        );
        assert_eq!(super::google_font_css_family_name(""), "");
        assert_eq!(super::google_font_css_family_name("  "), "");
        assert_eq!(
            super::google_font_css_family_name("Multiple:Colons:Here"),
            "Multiple"
        );
        assert_eq!(super::google_font_css_family_name(":startsWithColon"), "");
        assert_eq!(
            super::google_font_css_family_name("endsWithColon:"),
            "endsWithColon"
        );
    }

    #[test]
    fn test_merge_unique_font_families() {
        let input = vec![
            "  Inter  ".to_string(), // Trimming
            "".to_string(),          // Empty string
            "   ".to_string(),       // Whitespace-only string
            "Roboto".to_string(),    // Normal
            "inter".to_string(),     // Duplicate, different case
            "INTER".to_string(),     // Duplicate, different case
            "Open Sans".to_string(), // Normal
            "roboto".to_string(),    // Duplicate, different case
        ];

        let result = merge_unique_font_families(input);

        assert_eq!(
            result,
            vec![
                "Inter".to_string(),
                "Roboto".to_string(),
                "Open Sans".to_string(),
            ]
        );
    }

    #[test]
    fn test_merge_unique_font_families_edge_cases() {
        // Empty iterator
        let empty_input: Vec<String> = vec![];
        assert!(merge_unique_font_families(empty_input).is_empty());

        // Iterator with only empty/whitespace strings
        let whitespace_input = vec![
            "".to_string(),
            " ".to_string(),
            "   ".to_string(),
            "\n".to_string(),
            "\t".to_string(),
        ];
        assert!(merge_unique_font_families(whitespace_input).is_empty());

        // Duplicates differing only by leading/trailing whitespace
        let whitespace_dups = vec![
            "Arial".to_string(),
            " Arial".to_string(),
            "Arial ".to_string(),
            " Arial ".to_string(),
            "\tArial\n".to_string(),
        ];
        assert_eq!(
            merge_unique_font_families(whitespace_dups),
            vec!["Arial".to_string()]
        );
    }

    #[test]
    fn test_expand_class_list_in_place() {
        let mut aliases = std::collections::HashMap::new();
        aliases.insert("btn".to_string(), "px-4 py-2 bg-blue-500".to_string());
        aliases.insert("text-center".to_string(), "text-center".to_string());

        let mut classes = vec!["btn".to_string(), "mt-2".to_string()];
        expand_class_list_in_place(&mut classes, &aliases);

        assert_eq!(
            classes,
            vec![
                "px-4".to_string(),
                "py-2".to_string(),
                "bg-blue-500".to_string(),
                "mt-2".to_string(),
            ]
        );

        // Test with empty aliases
        let empty_aliases = std::collections::HashMap::new();
        let mut original_classes = vec!["btn".to_string(), "mt-2".to_string()];
        expand_class_list_in_place(&mut original_classes, &empty_aliases);
        assert_eq!(
            original_classes,
            vec!["btn".to_string(), "mt-2".to_string()]
        );

        // Test with empty input vec
        let mut empty_classes: Vec<String> = vec![];
        expand_class_list_in_place(&mut empty_classes, &aliases);
        assert_eq!(empty_classes, Vec::<String>::new());

        // Test tokens not in aliases pass through unchanged
        let mut no_match = vec!["flex".to_string(), "gap-4".to_string()];
        expand_class_list_in_place(&mut no_match, &aliases);
        assert_eq!(no_match, vec!["flex".to_string(), "gap-4".to_string()]);
    }

    #[test]
    fn test_strip_indent_decorators_empty() {
        let s = "";
        let d = strip_indent_decorators(s);
        assert_eq!(d.body, "");
        assert!(d.google_fonts.is_empty());
        assert!(d.class_aliases.is_empty());
        assert!(d.inline_css.is_empty());
    }

    #[test]
    fn test_strip_indent_decorators_whitespace_only() {
        let s = "   \n\t  \n";
        let d = strip_indent_decorators(s);
        assert_eq!(d.body, "");
        assert!(d.google_fonts.is_empty());
        assert!(d.class_aliases.is_empty());
        assert!(d.inline_css.is_empty());
    }

    #[test]
    fn test_strip_indent_decorators_no_trailing_newline() {
        let s = "div\n  span";
        let d = strip_indent_decorators(s);
        assert_eq!(d.body, "div\n  span");
        assert!(d.google_fonts.is_empty());
        assert!(d.class_aliases.is_empty());
        assert!(d.inline_css.is_empty());
    }

    #[test]
    fn test_strip_indent_decorators_only_decorators() {
        let s = "google-font Inter\n\n.btn px-4 py-2\n";
        let d = strip_indent_decorators(s);
        assert_eq!(d.body, "");
        assert_eq!(d.google_fonts, vec!["Inter"]);
        assert_eq!(
            d.class_aliases.get("btn").map(|s| s.as_str()),
            Some("px-4 py-2")
        );
        assert!(d.inline_css.is_empty());
    }

    #[test]
    fn test_strip_indent_decorators_multiple_empty_lines() {
        let s = "google-font Inter\n\n\n\n\n\n.btn px-4 py-2\n\n";
        let d = strip_indent_decorators(s);
        assert_eq!(d.body, "");
        assert_eq!(d.google_fonts, vec!["Inter"]);
        assert_eq!(
            d.class_aliases.get("btn").map(|s| s.as_str()),
            Some("px-4 py-2")
        );
        assert!(d.inline_css.is_empty());
    }
}
