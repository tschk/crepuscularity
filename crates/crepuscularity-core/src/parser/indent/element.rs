//! Element line parsing, tokenization, and text template handling.

use crate::ast::*;

pub(crate) fn parse_element_line(line: &str, children: Vec<Node>) -> Element {
    let tokens = tokenize_line(line);
    if tokens.is_empty() {
        return Element {
            tag: "div".to_string(),
            id: None,
            classes: vec![],
            conditional_classes: vec![],
            event_handlers: vec![],
            bindings: vec![],
            animations: vec![],
            children,
        };
    }

    let tag = tokens[0].clone();
    let mut children = children;
    let inline_text = tokens
        .last()
        .filter(|token| is_inline_text_token(token))
        .cloned();
    let parse_limit = if inline_text.is_some() {
        tokens.len().saturating_sub(1)
    } else {
        tokens.len()
    };
    if let Some(text) = inline_text {
        let mut new_children = Vec::with_capacity(children.len() + 1);
        new_children.push(Node::Text(parse_text_template(&text)));
        new_children.extend(children);
        children = new_children;
    }

    let mut id = None;
    let mut classes = Vec::with_capacity(4);
    let mut conditional_classes = Vec::new();
    let mut event_handlers = Vec::new();
    let mut bindings = Vec::new();
    let mut animations = Vec::new();

    for token in &tokens[1..parse_limit] {
        if let Some(rest) = token.strip_prefix('@') {
            if let Some(eq_pos) = rest.find('=') {
                let event_part = &rest[..eq_pos];
                let handler = strip_optional_quotes(&rest[eq_pos + 1..]).to_string();
                let event = event_part.split('|').next().unwrap_or("").to_string();
                let modifiers: Vec<String> = event_part
                    .split('|')
                    .skip(1)
                    .map(|s| s.to_string())
                    .collect();
                event_handlers.push(EventHandler {
                    event,
                    modifiers,
                    handler,
                });
            }
        } else if let Some(rest) = token.strip_prefix("when:") {
            if let Some((condition, raw_classes)) = parse_when_attribute_suffix(rest) {
                let classes_src = strip_optional_quotes(raw_classes.trim());
                let mut iter = classes_src.split_whitespace();
                if let Some(first) = iter.next() {
                    let mut prev = first;
                    for next in iter {
                        if prev.is_empty() {
                            prev = next;
                            continue;
                        }
                        conditional_classes.push(ConditionalClass {
                            class: prev.to_string(),
                            condition: condition.clone(),
                        });
                        prev = next;
                    }
                    if !prev.is_empty() {
                        conditional_classes.push(ConditionalClass {
                            class: prev.to_string(),
                            condition,
                        });
                    }
                }
            }
        } else if let Some(rest) = token.strip_prefix("class:") {
            if let Some(eq_pos) = rest.find('=') {
                let class = rest[..eq_pos].to_string();
                let cond_str = rest[eq_pos + 1..].trim();
                let condition = if cond_str.starts_with('{') && cond_str.ends_with('}') {
                    cond_str[1..cond_str.len() - 1].trim().to_string()
                } else {
                    cond_str.to_string()
                };
                conditional_classes.push(ConditionalClass { class, condition });
            }
        } else if let Some(rest) = token.strip_prefix("bind:") {
            if let Some(eq_pos) = rest.find('=') {
                let prop = rest[..eq_pos].to_string();
                let value = rest[eq_pos + 1..]
                    .trim_matches(|c| c == '{' || c == '}')
                    .to_string();
                bindings.push(Binding {
                    origin: crate::ast::BindingOrigin::Expression,
                    prop,
                    value,
                });
            }
        } else if let Some(rest) = token.strip_prefix("animate:") {
            // animate:property={duration easing} or animate:property={duration easing repeat}
            if let Some(eq_pos) = rest.find('=') {
                let property = rest[..eq_pos].to_string();
                let value_str = rest[eq_pos + 1..]
                    .trim_matches(|c| c == '{' || c == '}')
                    .trim()
                    .to_string();
                let parts: Vec<&str> = value_str.split_whitespace().collect();
                let duration_expr = parts.first().unwrap_or(&"300ms").to_string();
                let easing = parts.get(1).unwrap_or(&"linear").to_string();
                let repeat = parts.get(2).map(|s| *s == "repeat").unwrap_or(false);
                animations.push(AnimationSpec {
                    property,
                    duration_expr,
                    easing,
                    repeat,
                });
            }
        } else if let Some(rest) = token.strip_prefix('#') {
            if !rest.is_empty() {
                id = Some(rest.to_string());
            }
        } else if let Some(eq_pos) = token.find('=') {
            // HTML attribute: class="foo bar", type="button", data-action="x", key={expr}
            let key = &token[..eq_pos];
            let valid_key = !key.is_empty()
                && key
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_');
            if valid_key {
                let raw = token[eq_pos + 1..].trim();
                let unquoted = if raw.len() >= 2
                    && ((raw.starts_with('"') && raw.ends_with('"'))
                        || (raw.starts_with('\'') && raw.ends_with('\'')))
                {
                    &raw[1..raw.len() - 1]
                } else {
                    raw
                };
                if key == "class" {
                    // class="foo bar" → individual class tokens
                    for cls in unquoted.split_whitespace() {
                        classes.push(cls.to_string());
                    }
                } else if key == "id" {
                    id = Some(unquoted.to_string());
                } else {
                    let expr = if raw.starts_with('{') && raw.ends_with('}') {
                        raw[1..raw.len() - 1].trim().to_string()
                    } else {
                        format!("\"{}\"", unquoted)
                    };
                    bindings.push(Binding {
                        origin: crate::ast::BindingOrigin::Expression,
                        prop: key.to_string(),
                        value: expr,
                    });
                }
            } else {
                classes.push(token.clone());
            }
        } else if matches!(
            token.as_str(),
            "checked"
                | "disabled"
                | "hidden"
                | "required"
                | "readonly"
                | "multiple"
                | "selected"
                | "autofocus"
                | "open"
        ) {
            // Boolean HTML attributes
            bindings.push(Binding {
                origin: crate::ast::BindingOrigin::Expression,
                prop: token.clone(),
                value: "\"\"".to_string(),
            });
        } else {
            classes.push(token.clone());
        }
    }

    Element {
        tag,
        id,
        classes,
        conditional_classes,
        event_handlers,
        bindings,
        animations,
        children,
    }
}

fn is_inline_text_token(token: &str) -> bool {
    token.len() >= 2 && token.starts_with('"') && token.ends_with('"')
}

fn strip_optional_quotes(s: &str) -> &str {
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Parses the right-hand side of a `when:` attribute (everything after the `when:` prefix).
///
/// Accepts:
/// - `{expr}=quoted-or-bare-classes` — expression may contain `=` (e.g. `{a == b}="x y"`)
/// - `ident=classes` — simple condition (variable name)
///
/// Returns `(condition_source, raw_value)`; the caller should strip optional surrounding
/// quotes from `raw_value` (matching the parser's `when:` value rules) and split on whitespace
/// for Tailwind tokens.
pub fn parse_when_attribute_suffix(src: &str) -> Option<(String, String)> {
    let s = src.trim();
    if s.is_empty() {
        return None;
    }
    if s.starts_with('{') {
        let mut depth = 0usize;
        for (i, c) in s.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let cond = s[1..i].trim().to_string();
                        let mut tail = s[i + 1..].trim_start();
                        tail = tail.strip_prefix('=')?;
                        return Some((cond, tail.trim().to_string()));
                    }
                }
                _ => {}
            }
        }
        return None;
    }
    let eq_pos = s.find('=')?;
    let cond = s[..eq_pos].trim().to_string();
    if cond.is_empty() {
        return None;
    }
    Some((cond, s[eq_pos + 1..].trim().to_string()))
}

fn tokenize_line(line: &str) -> Vec<String> {
    let line = normalize_fullwidth_braces(line);
    let mut tokens = Vec::with_capacity(8);
    let mut current = String::with_capacity(line.len());
    let mut bracket_depth: usize = 0;
    let mut brace_depth: usize = 0;
    let mut in_string = false;
    let mut string_char = ' ';

    for ch in line.chars() {
        match ch {
            '[' if !in_string && brace_depth == 0 => {
                bracket_depth += 1;
                current.push(ch);
            }
            ']' if !in_string && brace_depth == 0 => {
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(ch);
            }
            '{' if !in_string && bracket_depth == 0 => {
                brace_depth += 1;
                current.push(ch);
            }
            '}' if !in_string && bracket_depth == 0 => {
                brace_depth = brace_depth.saturating_sub(1);
                current.push(ch);
            }
            '\'' | '"' => {
                if in_string && ch == string_char {
                    in_string = false;
                } else if !in_string {
                    in_string = true;
                    string_char = ch;
                }
                current.push(ch);
            }
            ' ' | '\t' if bracket_depth == 0 && brace_depth == 0 && !in_string => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Unescape `\n`, `\r`, `\t`, `\\`, `\"`, and `\'` inside a `.crepus` quoted text segment.
///
/// Unknown escapes keep the backslash (e.g. `\x` → `\x`).
pub fn unescape_crepus_text_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

pub(crate) fn parse_text_template(line: &str) -> Vec<TextPart> {
    let content = if line.starts_with('"') && line.ends_with('"') && line.len() >= 2 {
        &line[1..line.len() - 1]
    } else {
        line
    };

    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            if !literal.is_empty() {
                parts.push(TextPart::Literal(unescape_crepus_text_literal(&literal)));
                literal.clear();
            }
            let mut expr = String::new();
            let mut depth = 1usize;
            for ec in chars.by_ref() {
                match ec {
                    '{' => {
                        depth += 1;
                        expr.push(ec);
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                        expr.push(ec);
                    }
                    _ => expr.push(ec),
                }
            }
            parts.push(TextPart::Expr(expr.trim().to_string()));
        } else {
            literal.push(ch);
        }
    }

    if !literal.is_empty() {
        parts.push(TextPart::Literal(unescape_crepus_text_literal(&literal)));
    }

    parts
}

fn normalize_fullwidth_braces(s: &str) -> String {
    if !s.contains('\u{FF5B}') && !s.contains('\u{FF5D}') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\u{FF5B}' => out.push('{'),
            '\u{FF5D}' => out.push('}'),
            _ => out.push(ch),
        }
    }
    out
}
