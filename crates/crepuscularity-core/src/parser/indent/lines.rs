//! Line collection and structural indent handling.

use super::super::RawParseError;

/// Strip exactly `n` leading spaces from `s`, leaving any additional
/// whitespace intact (it is content, not structural indent).
pub(crate) fn strip_structural_indent(s: &str, n: usize) -> &str {
    for (count, (byte_pos, ch)) in s.char_indices().enumerate() {
        if count >= n || ch != ' ' {
            return &s[byte_pos..];
        }
    }
    // Entire string was spaces (≤ n of them).
    ""
}

pub(crate) fn collect_lines(template: &str) -> Result<Vec<(usize, String)>, RawParseError> {
    let source_lines: Vec<&str> = template.lines().collect();
    let mut raw: Vec<(usize, String)> = Vec::new();
    let mut i = 0;

    while i < source_lines.len() {
        let line = source_lines[i];
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        i += 1;

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Multi-line strings: a `"` that opens but doesn't close on the same line
        // is merged with subsequent lines until the closing `"` is found.
        // Continuation lines have exactly `indent` leading spaces stripped (the
        // structural indent of the opening `"` line); any additional whitespace
        // is part of the string content and must be preserved.
        let mut escaped = false;
        if trimmed.starts_with('"') && !string_is_closed(trimmed.chars().skip(1), &mut escaped) {
            let mut combined = trimmed.to_string();
            let mut closed = false;
            while i < source_lines.len() {
                combined.push('\n');
                let continuation = strip_structural_indent(source_lines[i], indent);
                combined.push_str(continuation);
                i += 1;
                closed = string_is_closed(
                    std::iter::once('\n').chain(continuation.chars()),
                    &mut escaped,
                );
                if closed {
                    break;
                }
            }
            if !closed {
                return Err(RawParseError {
                    message: "unterminated quoted text".to_owned(),
                    byte_offset: None,
                });
            }
            raw.push((indent, combined));
        } else {
            raw.push((indent, trimmed.to_string()));
        }
    }

    // Normalize indentation so root elements always start at column 0.
    let min_indent = raw.iter().map(|(i, _)| *i).min().unwrap_or(0);
    if min_indent == 0 {
        return Ok(raw);
    }
    Ok(raw.into_iter().map(|(i, l)| (i - min_indent, l)).collect())
}

/// Scan newly appended text after the opening quote, retaining escape state.
fn string_is_closed(chars: impl Iterator<Item = char>, escaped: &mut bool) -> bool {
    for ch in chars {
        if *escaped {
            *escaped = false;
            continue;
        }
        if ch == '\\' {
            *escaped = true;
            continue;
        }
        if ch == '"' {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiline_quotes_preserve_escapes_newlines_and_indentation() {
        for source in [
            "  \"雪\\\"\n    more\"\n  div",
            "  \"雪\\\n    \"\n  div",
            "  \"雪\\\\\n    \"\n  div",
        ] {
            let lines = collect_lines(source).unwrap();
            let expected = source
                .lines()
                .take(2)
                .map(|line| strip_structural_indent(line, 2))
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(lines, vec![(0, expected), (0, "div".into())]);
        }
        let error = collect_lines("\"雪\\\"\nunfinished").unwrap_err();
        assert_eq!(error.message, "unterminated quoted text");
        assert_eq!(error.byte_offset, None);
    }

    #[test]
    fn multiline_scanner_visits_each_appended_character_once() {
        for lines in [1_000, 2_000] {
            let mut escaped = false;
            let mut visited = 0;
            for _ in 0..lines {
                assert!(!string_is_closed(
                    "\n雪\\\"".chars().inspect(|_| visited += 1),
                    &mut escaped
                ));
            }
            assert!(string_is_closed(std::iter::once('"'), &mut escaped));
            assert_eq!(visited, lines * 4);
        }
    }
}
