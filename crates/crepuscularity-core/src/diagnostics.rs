use crate::parser::{parse_component_file_inner, parse_template_raw_with_path, RawParseError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrepusDiagnostic {
    pub message: String,
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// Source lines with original byte offsets, accepting LF, CRLF and lone CR.
/// Includes the empty line after a final line ending, as required by LSP positions.
pub fn source_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut offset = 0;
    let mut finished = false;
    std::iter::from_fn(move || {
        if finished {
            return None;
        }
        let start = offset;
        let rest = &source[start..];
        let end = rest.find(['\r', '\n']).unwrap_or(rest.len());
        if end == rest.len() {
            finished = true;
        } else {
            let ending =
                if rest.as_bytes()[end] == b'\r' && rest.as_bytes().get(end + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
            offset += end + ending;
        }
        Some((start, &rest[..end]))
    })
}

fn byte_offset_to_position(source: &str, byte: usize) -> (u32, u32) {
    let b = byte.min(source.len());
    let (line, (start, text)) = source_lines(source)
        .enumerate()
        .take_while(|(_, (start, _))| *start <= b)
        .last()
        .unwrap();
    let end = b.min(start + text.len());
    (
        line as u32,
        source[start..end].encode_utf16().count() as u32,
    )
}

fn range_for_byte_span(source: &str, start: usize, end: usize) -> (u32, u32, u32, u32) {
    let start = start.min(source.len());
    let end = end.max(start).min(source.len());
    let (sl, sc) = byte_offset_to_position(source, start);
    let (el, ec) = byte_offset_to_position(source, end);
    (sl, sc, el, ec)
}

fn diagnostic_from_offset(source: &str, message: String, byte: usize) -> CrepusDiagnostic {
    let byte = byte.min(source.len());
    let (sl, sc) = byte_offset_to_position(source, byte);
    let (line_start, line) = source_lines(source).nth(sl as usize).unwrap();
    let line_end_byte = line_start + line.len();
    let char_width = source[byte..]
        .chars()
        .next()
        .map(char::len_utf8)
        .unwrap_or(0);
    let end_byte = (byte + char_width).min(line_end_byte);
    let (el, ec) = byte_offset_to_position(source, end_byte);
    CrepusDiagnostic {
        message,
        start_line: sl,
        start_character: sc,
        end_line: el,
        end_character: ec,
    }
}

fn diagnostic_fallback_first_line(source: &str, message: String) -> CrepusDiagnostic {
    let end_byte = source_lines(source).next().unwrap().1.len();
    let (sl, sc, el, ec) = range_for_byte_span(source, 0, end_byte);
    CrepusDiagnostic {
        message,
        start_line: sl,
        start_character: sc,
        end_line: el,
        end_character: ec,
    }
}

fn raw_parse_error_to_diagnostic(source: &str, e: RawParseError) -> CrepusDiagnostic {
    match e.byte_offset {
        Some(b) => diagnostic_from_offset(source, e.message, b),
        None => diagnostic_fallback_first_line(source, e.message),
    }
}

pub fn diagnose_crepus_source(source: &str) -> Vec<CrepusDiagnostic> {
    diagnose_crepus_source_with_path(source, None)
}

/// Diagnose using the same filename-based frontend selection as template compilation.
pub fn diagnose_crepus_source_with_path(
    source: &str,
    path: Option<&std::path::Path>,
) -> Vec<CrepusDiagnostic> {
    if is_multi_component_file(source) {
        match parse_component_file_inner(source) {
            Ok(_) => vec![],
            Err(e) => vec![raw_parse_error_to_diagnostic(source, e)],
        }
    } else {
        match parse_template_raw_with_path(source, path) {
            Ok(_) => vec![],
            Err(e) => vec![raw_parse_error_to_diagnostic(source, e)],
        }
    }
}

pub fn is_multi_component_file(source: &str) -> bool {
    source
        .strip_prefix('\u{feff}')
        .unwrap_or(source)
        .trim_start()
        .starts_with("+++")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_lines_preserve_offsets_and_clamp_inside_line_endings() {
        let source = "😀\r\n雪\rx\n";
        assert_eq!(
            source_lines(source).collect::<Vec<_>>(),
            [(0, "😀"), (6, "雪"), (10, "x"), (12, "")]
        );
        for (byte, expected) in [
            (0, (0, 0)),
            (4, (0, 2)),
            (5, (0, 2)),
            (6, (1, 0)),
            (9, (1, 1)),
            (10, (2, 0)),
            (11, (2, 1)),
            (12, (3, 0)),
        ] {
            assert_eq!(byte_offset_to_position(source, byte), expected);
        }
        assert_eq!(source_lines("").collect::<Vec<_>>(), [(0, "")]);
        let diagnostic = diagnostic_fallback_first_line("😀\rnext", "error".into());
        assert_eq!((diagnostic.end_line, diagnostic.end_character), (0, 2));
    }

    #[test]
    fn jsx_eof_diagnostics_use_original_unicode_positions() {
        let path = Some(std::path::Path::new("entry.jsx"));
        for (source, line, character) in [
            ("<div>｛x｝界", 0, 9),
            ("<div>｛x｝😀", 0, 10),
            ("\u{feff}<div>｛x｝界", 0, 10),
            ("<div>\r\n｛x｝界", 1, 4),
            ("\u{feff}<div>\r\n｛x｝😀", 1, 5),
        ] {
            assert!(
                crate::parse_template_with_path(source, path).is_err(),
                "{source:?}"
            );
            let diagnostics = diagnose_crepus_source_with_path(source, path);
            assert_eq!(diagnostics.len(), 1, "{source:?}");
            let diagnostic = &diagnostics[0];
            assert_eq!(
                (diagnostic.start_line, diagnostic.start_character),
                (line, character),
                "{source:?}: {diagnostic:?}"
            );
            assert_eq!(
                (diagnostic.end_line, diagnostic.end_character),
                (line, character),
                "{source:?}: {diagnostic:?}"
            );
        }
    }

    #[test]
    fn diagnostic_spans_cover_whole_unicode_characters() {
        let source = "a😀界\n";
        for (byte, start, end) in [(1, 1, 3), (5, 3, 4), (8, 4, 4), (9, 0, 0), (99, 0, 0)] {
            let diagnostic = diagnostic_from_offset(source, "invalid input".into(), byte);
            assert_eq!(diagnostic.start_character, start);
            assert_eq!(diagnostic.end_character, end);
            assert_eq!(diagnostic.start_line, diagnostic.end_line);
        }
    }

    #[test]
    fn filename_diagnostics_cover_all_markup_frontends() {
        for (filename, valid, invalid, expected) in [
            ("App.tsx", "<p />", "</orphan>", "trailing markup"),
            (
                "App.svelte",
                "{#if show}<p>ok</p>{/if}",
                "{#await promise}<p />{/await}",
                "await",
            ),
            (
                "App.vue",
                "<template><p v-if=\"show\">ok</p></template>",
                "<template><div v-once /></template>",
                "v-once",
            ),
            ("App.astro", "{show && <p>ok</p>}", "<slot />", "slot"),
            (
                "App.component.html",
                "@if (show) { <p>ok</p> }",
                "<ng-template><p /></ng-template>",
                "ng-template",
            ),
        ] {
            let path = Some(std::path::Path::new(filename));
            assert!(
                diagnose_crepus_source_with_path(valid, path).is_empty(),
                "{filename}"
            );
            let errors = diagnose_crepus_source_with_path(invalid, path);
            assert_eq!(errors.len(), 1, "{filename}");
            assert!(
                errors[0].message.contains(expected),
                "{filename}: {}",
                errors[0].message
            );
        }
    }

    #[test]
    fn filename_selects_svelte_diagnostics() {
        let path = Some(std::path::Path::new("App.svelte"));
        assert!(diagnose_crepus_source_with_path("{#if show}<p>ok</p>{/if}", path).is_empty());
        let diagnostics = diagnose_crepus_source_with_path("{#if show}<p>missing close</p>", path);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("if"));
    }

    #[test]
    fn jsx_unclosed_tag_has_line() {
        let src = "<div class=\"x\"";
        let d = diagnose_crepus_source(src);
        assert_eq!(d.len(), 1);
        assert!(
            d[0].message.contains("unclosed") || d[0].message.contains("</"),
            "{}",
            d[0].message
        );
        assert_eq!(d[0].start_line, 0);
    }

    #[test]
    fn indent_only_ok() {
        assert!(diagnose_crepus_source("div\n  \"hi\"").is_empty());
    }

    #[test]
    fn test_is_multi_component_file() {
        assert!(is_multi_component_file("+++"));
        assert!(is_multi_component_file("+++\nname: Test\n+++\n"));

        // With leading whitespace
        assert!(is_multi_component_file("   +++"));
        assert!(is_multi_component_file("\n\n+++"));
        assert!(is_multi_component_file("\t+++"));

        // Single component / not multi-component
        assert!(!is_multi_component_file("++"));
        assert!(!is_multi_component_file("+"));
        assert!(!is_multi_component_file("div\n  \"hi\""));
        assert!(!is_multi_component_file(""));
        assert!(!is_multi_component_file("   "));
        assert!(!is_multi_component_file("abc\n+++"));

        // `---` lines are body-section headers, not multi-component frontmatter
        assert!(!is_multi_component_file("---component"));
        assert!(!is_multi_component_file("--- component"));
        assert!(!is_multi_component_file("   ---component"));
    }
}
