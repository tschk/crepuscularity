//! JSX text nodes and offset mapping helpers.

use crate::ast::*;

use super::super::indent::parse_text_template;
use super::super::{subslice_byte_offset, RawParseError};

pub(crate) fn normalize_jsx_mapped(s: &str) -> (String, Vec<usize>) {
    let mut norm = String::with_capacity(s.len());
    let mut map: Vec<usize> = Vec::with_capacity(s.len() + 1);
    for (orig_b, ch) in s.char_indices() {
        match ch {
            '\u{FF5B}' => {
                norm.push('{');
                map.push(orig_b);
            }
            '\u{FF5D}' => {
                norm.push('}');
                map.push(orig_b);
            }
            c => {
                let mut buf = [0u8; 4];
                let enc = c.encode_utf8(&mut buf);
                for _ in 0..enc.len() {
                    map.push(orig_b);
                }
                norm.push_str(enc);
            }
        }
    }
    // EOF errors need an original-source offset too, after width-changing normalization.
    map.push(s.len());
    debug_assert_eq!(norm.len() + 1, map.len());
    (norm, map)
}

pub(crate) fn map_jsx_offset(map: &[usize], off: usize) -> usize {
    map.get(off).copied().unwrap_or(off)
}

#[inline]
pub(crate) fn jsx_err(norm_root: &str, at: &str, message: impl Into<String>) -> RawParseError {
    RawParseError {
        message: message.into(),
        byte_offset: Some(subslice_byte_offset(norm_root, at)),
    }
}

/// Recognize comment-only JSX containers without treating comment contents as syntax.
/// Mixed JavaScript comments and DSL expressions are unsupported, rather than discarded.
pub(crate) fn jsx_comment_container<'a>(
    norm_root: &str,
    src: &'a str,
) -> Result<Option<&'a str>, RawParseError> {
    if !src.starts_with('{') {
        return Ok(None);
    }
    let mut index = 1;
    let mut depth = 1usize;
    let mut quote = None;
    let mut comment = false;
    let mut code = false;
    while index < src.len() {
        let rest = &src[index..];
        let ch = rest.chars().next().unwrap();
        if let Some(delimiter) = quote {
            if ch == '\\' {
                index += ch.len_utf8();
                if let Some(escaped) = src[index..].chars().next() {
                    index += escaped.len_utf8();
                }
                continue;
            }
            if ch == delimiter {
                quote = None;
            }
            index += ch.len_utf8();
            continue;
        }
        if let Some(body) = rest.strip_prefix("/*") {
            comment = true;
            let end = body
                .find("*/")
                .ok_or_else(|| jsx_err(norm_root, rest, "unclosed JSX block comment"))?;
            index += 2 + end + 2;
            continue;
        }
        if rest.starts_with("//") {
            comment = true;
            let end = rest
                .find(['\n', '\r', '\u{2028}', '\u{2029}'])
                .ok_or_else(|| jsx_err(norm_root, rest, "unclosed JSX line comment container"))?;
            index += end;
            continue;
        }
        match ch {
            '\'' | '"' | '`' => {
                code = true;
                quote = Some(ch);
            }
            '{' => {
                code = true;
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return if comment && code {
                        Err(jsx_err(
                            norm_root,
                            src,
                            "JSX comments cannot be mixed with executable expressions",
                        ))
                    } else if comment {
                        Ok(Some(&src[index + 1..]))
                    } else {
                        Ok(None)
                    };
                }
            }
            ch if !ch.is_whitespace() => code = true,
            _ => {}
        }
        index += ch.len_utf8();
    }
    if comment {
        Err(jsx_err(norm_root, src, "unclosed JSX comment container"))
    } else {
        Ok(None)
    }
}

/// Consume a text region up to the next tag, preserving inline space boundaries.
pub(crate) fn jsx_text_node<'a>(
    norm_root: &str,
    src: &'a str,
    nested: bool,
) -> Result<(Option<Node>, &'a str), RawParseError> {
    let mut end = 0;
    let mut chunk_start = 0;
    let mut chunks = Vec::new();
    while end < src.len() {
        let ch = src[end..].chars().next().unwrap();
        if ch == '<' || src[end..].starts_with("{=") {
            break;
        }
        if ch == '{' {
            if let Some(rest) = jsx_comment_container(norm_root, &src[end..])? {
                chunks.push((chunk_start, end));
                end = src.len() - rest.len();
                chunk_start = end;
                continue;
            }
            let close = super::super::match_delimiter(&src[end..], '{', '}')
                .ok_or_else(|| jsx_err(norm_root, &src[end..], "unclosed text interpolation"))?;
            end += close + 1;
        } else {
            end += ch.len_utf8();
        }
    }
    if end == 0 {
        return Ok((None, src));
    }
    chunks.push((chunk_start, end));
    let region = &src[..end];
    let text = super::super::markup_text_region(region, nested);
    let mut first = text.as_ptr() as usize - region.as_ptr() as usize;
    let mut last = first + text.len();
    // Match parse_text_template's quote policy once for the entire text region,
    // rather than independently stripping quotes from comment-separated chunks.
    let quoted = region.trim();
    if quoted.len() >= 2 && quoted.starts_with('"') && quoted.ends_with('"') {
        first = region.len() - region.trim_start().len() + 1;
        last = region.trim_end().len() - 1;
    }
    let mut parts = Vec::new();
    for (start, end) in chunks {
        let start = start.max(first);
        let end = end.min(last);
        if start < end {
            let chunk = &src[start..end];
            let quoted = chunk.len() >= 2 && chunk.starts_with('"') && chunk.ends_with('"');
            if quoted {
                parts.push(TextPart::Literal("\"".into()));
            }
            parts.extend(parse_text_template(chunk).map_err(|mut error| {
                error.byte_offset = Some(
                    subslice_byte_offset(norm_root, &src[start..]) + error.byte_offset.unwrap_or(0),
                );
                error
            })?);
            if quoted {
                parts.push(TextPart::Literal("\"".into()));
            }
        }
    }
    Ok((
        (!parts.is_empty()).then_some(Node::Text(parts)),
        &src[end..],
    ))
}

pub(crate) fn jsx_open_tag_tail<'a>(src: &'a str, tag: &str) -> Option<&'a str> {
    src.strip_prefix('<')?.strip_prefix(tag).filter(|tail| {
        tail.is_empty() || tail.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/')
    })
}

pub(crate) fn jsx_close<'a>(
    norm_root: &str,
    src: &'a str,
    tag: &str,
) -> Result<&'a str, RawParseError> {
    let src = src.trim_start();
    let prefix = format!("</{}", tag);
    if let Some(rest) = src.strip_prefix(&prefix) {
        let rest = rest.trim_start();
        if let Some(rest) = rest.strip_prefix('>') {
            return Ok(rest);
        }
    }
    Err(jsx_err(
        norm_root,
        src,
        format!(
            "expected </{}>, got: {}",
            tag,
            super::super::diagnostic_prefix(src, 40)
        ),
    ))
}
