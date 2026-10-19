//! Expression punctuation belongs to JavaScript, not nested markup text.
// Authored compilers may validate JavaScript fragments without importing a JS parser here.
use super::super::jsx::text::jsx_close;
use super::tags::{read_opening, VOID_TAGS};
use super::ParseMode;

pub(super) fn match_delimiter(
    src: &str,
    open: char,
    close: char,
    mode: ParseMode,
) -> Option<usize> {
    match_group(src, open, close, 0, mode)
}

fn match_group(
    src: &str,
    open: char,
    close: char,
    nesting: usize,
    mode: ParseMode,
) -> Option<usize> {
    let mut depth = 0usize;
    walk(src, nesting, mode, 1, |_, c| {
        if c == open {
            depth += 1;
        } else if c == close {
            let Some(next) = depth.checked_sub(1) else {
                return false;
            };
            depth = next;
            return depth == 0;
        }
        false
    })
}

pub(super) fn scan_top_level(
    src: &str,
    mode: ParseMode,
    mut f: impl FnMut(usize, char) -> bool,
) -> Option<usize> {
    let mut depth = 0i32;
    walk(src, 0, mode, 0, |i, c| match c {
        '{' | '[' | '(' => {
            depth += 1;
            false
        }
        '}' | ']' | ')' => {
            depth -= 1;
            false
        }
        _ => depth == 0 && f(i, c),
    })
}

fn walk(
    src: &str,
    nesting: usize,
    mode: ParseMode,
    prefix_start: usize,
    f: impl FnMut(usize, char) -> bool,
) -> Option<usize> {
    if nesting > super::MAX_ASTRO_DEPTH {
        return None;
    }
    stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
        walk_inner(src, nesting, mode, prefix_start, f)
    })
}

fn walk_inner(
    src: &str,
    nesting: usize,
    mode: ParseMode,
    prefix_start: usize,
    mut f: impl FnMut(usize, char) -> bool,
) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            quote @ (b'\'' | b'"' | b'`') => {
                i += quoted_end(&src[i..], quote, nesting + 1, mode)?;
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                i += src[i..].find('\n').unwrap_or(src.len() - i);
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2 + src[i + 2..].find("*/")? + 2;
                continue;
            }
            b'/' if i >= prefix_start && mode.validator.is_some() => {
                let valid = mode.validator?;
                if !valid(&src[prefix_start..i]) {
                    if let Some(end) = regex_end(&src[i..], valid) {
                        i += end;
                        continue;
                    }
                }
            }
            b'(' | b'[' | b'{' if i >= prefix_start && mode.validator.is_some() => {
                if let Some(end) = javascript_group_end(&src[i..], nesting, mode) {
                    i += end;
                    continue;
                }
            }
            b'<' if markup_position(src, i) => {
                if let Some(end) = markup_end(&src[i..], nesting + 1, mode) {
                    i += end;
                    continue;
                }
            }
            _ => {}
        }
        let c = src[i..].chars().next()?;
        if f(i, c) {
            return Some(i);
        }
        i += c.len_utf8();
    }
    None
}

fn markup_end(src: &str, nesting: usize, mode: ParseMode) -> Option<usize> {
    if nesting > super::MAX_ASTRO_DEPTH || !src.as_bytes().get(1)?.is_ascii_alphabetic() {
        return None;
    }
    let (tag, _, mut rest, closed) = read_opening(src, src, nesting, mode).ok()?;
    if closed || VOID_TAGS.contains(&tag) {
        return Some(src.len() - rest.len());
    }
    let mut tags = vec![tag];
    while !rest.is_empty() {
        if nesting + tags.len() > super::MAX_ASTRO_DEPTH {
            return None;
        }
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = &comment[comment.find("-->")? + 3..];
        } else if rest.starts_with("</") {
            rest = jsx_close(src, rest, tags.pop()?).ok()?;
            if tags.is_empty() {
                return Some(src.len() - rest.len());
            }
        } else if rest.starts_with('<') {
            let (tag, _, next, closed) =
                read_opening(src, rest, nesting + tags.len(), mode).ok()?;
            if !closed && !VOID_TAGS.contains(&tag) {
                tags.push(tag);
            }
            rest = next;
        } else if rest.starts_with('{') {
            let end = match_group(rest, '{', '}', nesting + tags.len(), mode)?;
            rest = &rest[end + 1..];
        } else {
            let end = rest.find(['<', '{']).unwrap_or(rest.len());
            rest = &rest[end..];
        }
    }
    None
}

fn markup_position(src: &str, at: usize) -> bool {
    !src[..at]
        .chars()
        .rev()
        .find(|c| !c.is_whitespace())
        .is_some_and(|c| {
            c.is_alphanumeric() || matches!(c, '_' | '$' | ')' | ']' | '}' | '\'' | '"' | '`')
        })
}

fn quoted_end(src: &str, quote: u8, nesting: usize, mode: ParseMode) -> Option<usize> {
    if nesting > super::MAX_ASTRO_DEPTH {
        return None;
    }
    let bytes = src.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
        } else if bytes[i] == quote {
            return Some(i + 1);
        } else if quote == b'`' && bytes[i..].starts_with(b"${") {
            i += 1 + match_group(&src[i + 1..], '{', '}', nesting + 1, mode)? + 1;
        } else {
            i += 1;
        }
    }
    None
}

pub(super) fn match_brace_at(src: &str, nesting: usize, mode: ParseMode) -> Option<usize> {
    match_group(src, '{', '}', nesting, mode)
}

fn regex_end(src: &str, valid: fn(&str) -> bool) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut i = 1;
    let mut class = false;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'[' => class = true,
            b']' => class = false,
            b'/' if !class => {
                let close = i + 1;
                let flags = bytes[close..]
                    .iter()
                    .take_while(|byte| byte.is_ascii_alphabetic())
                    .count();
                let end = close + flags;
                return valid(&src[..end]).then_some(end);
            }
            b'\n' | b'\r' => return None,
            _ => {}
        }
        i += 1;
    }
    None
}

fn javascript_group_end(src: &str, nesting: usize, mode: ParseMode) -> Option<usize> {
    let valid = mode.validator?;
    let open = src.as_bytes()[0];
    let close = match open {
        b'(' => ')',
        b'[' => ']',
        b'{' => '}',
        _ => return None,
    };
    let complete = |candidate: &str| {
        valid(candidate)
            || match open {
                b'(' => {
                    valid(&format!("__crepusProbe{candidate}")) || valid(&format!("{candidate}=>0"))
                }
                b'{' => valid(&format!("async function* __crepusProbe(){candidate}")),
                _ => false,
            }
    };
    // Common groups need one grammar validation; punctuation inside a regex may
    // make the legacy candidate wrong, so later candidates still use the parser.
    let candidate = match_group(
        src,
        char::from(open),
        close,
        nesting + 1,
        ParseMode {
            validator: None,
            ..mode
        },
    )
    .map(|end| end + 1);
    if let Some(end) = candidate {
        if complete(&src[..end]) {
            return Some(end);
        }
    }
    for (offset, _) in src.match_indices(close) {
        let end = offset + 1;
        if Some(end) != candidate && complete(&src[..end]) {
            return Some(end);
        }
    }
    None
}
