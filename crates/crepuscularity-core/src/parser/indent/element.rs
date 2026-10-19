//! Element line parsing, tokenization, and text template handling.

use super::super::RawParseError;
use crate::ast::*;

pub(crate) fn parse_element_line(
    line: &str,
    children: Vec<Node>,
) -> Result<Element, RawParseError> {
    let tokens = tokenize_line(line);
    if tokens.is_empty() {
        return Ok(Element {
            tag: "div".to_string(),
            id: None,
            classes: vec![],
            conditional_classes: vec![],
            event_handlers: vec![],
            bindings: vec![],
            animations: vec![],
            children,
        });
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
        new_children.push(Node::Text(parse_text_template(&text)?));
        new_children.extend(children);
        children = new_children;
    }

    if tokens[1..parse_limit]
        .iter()
        .filter(|token| {
            token.starts_with("id=")
                || token.starts_with("bind:id=")
                || token.strip_prefix('#').is_some_and(|id| !id.is_empty())
        })
        .take(2)
        .count()
        > 1
    {
        return Err(RawParseError {
            message: "an element may have only one id attribute".into(),
            byte_offset: None,
        });
    }

    let mut id = None;
    let mut classes = Vec::with_capacity(4);
    let mut conditional_classes = Vec::new();
    let mut event_handlers = Vec::new();
    let mut bindings = Vec::new();
    let mut animations = Vec::new();

    for token in &tokens[1..parse_limit] {
        if let Some(rest) = token.strip_prefix('@') {
            let (event_part, raw_handler) = rest.split_once('=').ok_or_else(|| RawParseError {
                message: "event requires a name and handler".into(),
                byte_offset: None,
            })?;
            let handler = strip_optional_quotes(raw_handler).trim();
            let expression = handler
                .strip_prefix('{')
                .and_then(|handler| handler.strip_suffix('}'))
                .unwrap_or(handler)
                .trim();
            let mut parts = event_part.split('|');
            let event = parts.next().unwrap_or("");
            let modifiers: Vec<String> = parts.map(str::to_string).collect();
            if event.trim().is_empty()
                || expression.is_empty()
                || modifiers.iter().any(|modifier| modifier.trim().is_empty())
            {
                return Err(RawParseError {
                    message: "event requires a nonempty name, handler, and modifier entries".into(),
                    byte_offset: None,
                });
            }
            event_handlers.push(EventHandler {
                model_before_listener: None,
                event: event.to_string(),
                modifiers,
                handler: handler.to_string(),
            });
        } else if let Some(rest) = token.strip_prefix("when:") {
            let (condition, raw_classes) =
                parse_when_attribute_suffix(rest).ok_or_else(|| RawParseError {
                    message: "when directive requires a nonempty condition and class assignment"
                        .into(),
                    byte_offset: None,
                })?;
            conditional_classes.extend(
                split_class_tokens(strip_optional_quotes(raw_classes.trim()))
                    .into_iter()
                    .map(|class| ConditionalClass {
                        class,
                        condition: condition.clone(),
                    }),
            );
        } else if let Some(rest) = token.strip_prefix("class:") {
            let (class, value) = rest.split_once('=').ok_or_else(|| RawParseError {
                message: "conditional class requires a name and condition".into(),
                byte_offset: None,
            })?;
            let value = value.trim();
            let condition = value
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
                .unwrap_or(value)
                .trim();
            if class.trim().is_empty() || condition.is_empty() {
                return Err(RawParseError {
                    message: "conditional class requires a nonempty name and condition".into(),
                    byte_offset: None,
                });
            }
            conditional_classes.push(ConditionalClass {
                class: class.to_string(),
                condition: condition.to_string(),
            });
        } else if let Some(rest) = token.strip_prefix("bind:") {
            let (prop, value) = rest.split_once('=').ok_or_else(|| RawParseError {
                message: "binding requires a property and expression".into(),
                byte_offset: None,
            })?;
            let value = value
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
                .unwrap_or(value)
                .trim();
            if prop.trim().is_empty() || value.is_empty() {
                return Err(RawParseError {
                    message: "binding requires a nonempty property and expression".into(),
                    byte_offset: None,
                });
            }
            bindings.push(Binding {
                origin: crate::ast::BindingOrigin::Expression,
                prop: prop.to_string(),
                value: value.to_string(),
            });
        } else if let Some(rest) = token.strip_prefix("animate:") {
            // animate:property={duration easing} or animate:property={duration easing repeat}
            let (property, value) = rest.split_once('=').ok_or_else(|| RawParseError {
                message: "animation requires a property and settings assignment".into(),
                byte_offset: None,
            })?;
            if property.trim().is_empty() {
                return Err(RawParseError {
                    message: "animation requires a property name".into(),
                    byte_offset: None,
                });
            }
            let value = strip_optional_quotes(value.trim());
            let value = value
                .strip_prefix('{')
                .and_then(|value| value.strip_suffix('}'))
                .unwrap_or(value)
                .trim();
            let parts: Vec<&str> = value.split_whitespace().collect();
            if parts.len() > 3 || parts.get(2).is_some_and(|part| *part != "repeat") {
                return Err(RawParseError {
                    message: "animation settings accept duration, easing, and optional repeat only"
                        .into(),
                    byte_offset: None,
                });
            }
            animations.push(AnimationSpec {
                property: property.to_string(),
                duration_expr: parts.first().unwrap_or(&"300ms").to_string(),
                easing: parts.get(1).unwrap_or(&"linear").to_string(),
                repeat: parts.get(2).is_some_and(|part| *part == "repeat"),
            });
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
                    if let Some(expression) =
                        raw.strip_prefix('{').and_then(|raw| raw.strip_suffix('}'))
                    {
                        if expression.trim().is_empty() {
                            return Err(RawParseError {
                                message: "class binding requires a nonempty expression".into(),
                                byte_offset: None,
                            });
                        }
                        classes.push(raw.to_string());
                    } else {
                        classes.extend(split_class_tokens(unquoted));
                    }
                } else if key == "id" {
                    if let Some(expression) =
                        raw.strip_prefix('{').and_then(|raw| raw.strip_suffix('}'))
                    {
                        if expression.trim().is_empty() {
                            return Err(RawParseError {
                                message: "id binding requires a nonempty expression".into(),
                                byte_offset: None,
                            });
                        }
                        bindings.push(Binding {
                            origin: crate::ast::BindingOrigin::Expression,
                            prop: "id".into(),
                            value: expression.trim().to_string(),
                        });
                    } else {
                        id = Some(unquoted.to_string());
                    }
                } else {
                    let expr = if raw.starts_with('{') && raw.ends_with('}') {
                        let expression = raw[1..raw.len() - 1].trim();
                        if expression.is_empty() {
                            return Err(RawParseError {
                                message: format!(
                                    "attribute `{key}` requires a nonempty expression"
                                ),
                                byte_offset: None,
                            });
                        }
                        expression.to_string()
                    } else {
                        crate::parser::quote_expression_string(unquoted)
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
                | "defaultChecked"
                | "default-checked"
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

    Ok(Element {
        tag,
        id,
        classes,
        conditional_classes,
        event_handlers,
        bindings,
        animations,
        children,
    })
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
/// quotes from `raw_value` and split whitespace outside interpolation spans for class tokens.
pub fn parse_when_attribute_suffix(src: &str) -> Option<(String, String)> {
    let s = src.trim();
    if s.is_empty() {
        return None;
    }
    if s.starts_with('{') {
        let end = super::super::match_delimiter(s, '{', '}')?;
        let condition = s[1..end].trim();
        if condition.is_empty() {
            return None;
        }
        let classes = s[end + 1..].trim_start().strip_prefix('=')?.trim();
        return Some((condition.to_string(), classes.to_string()));
    }
    let eq_pos = s.find('=')?;
    let cond = s[..eq_pos].trim().to_string();
    if cond.is_empty() {
        return None;
    }
    Some((cond, s[eq_pos + 1..].trim().to_string()))
}

pub(crate) fn split_class_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut start = 0;
    let mut offset = 0;
    while let Some(character) = value[offset..].chars().next() {
        if character == '{' {
            // Keep interpolation whitespace and quoted braces inside one token.
            offset = super::super::match_delimiter(&value[offset..], '{', '}')
                .map_or(value.len(), |end| offset + end + 1);
        } else if character.is_whitespace() {
            if start < offset {
                tokens.push(value[start..offset].to_string());
            }
            offset += character.len_utf8();
            start = offset;
        } else {
            offset += character.len_utf8();
        }
    }
    if start < offset {
        tokens.push(value[start..offset].to_string());
    }
    tokens
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

pub(crate) fn parse_text_template(line: &str) -> Result<Vec<TextPart>, RawParseError> {
    let content = if line.starts_with('"') && line.ends_with('"') && line.len() >= 2 {
        &line[1..line.len() - 1]
    } else {
        line
    };
    let mut parts = Vec::new();
    let mut remaining = content;
    while let Some(start) = remaining.find('{') {
        if start != 0 {
            parts.push(TextPart::Literal(unescape_crepus_text_literal(
                &remaining[..start],
            )));
        }
        let placeholder = &remaining[start..];
        let end =
            super::super::match_delimiter(placeholder, '{', '}').ok_or_else(|| RawParseError {
                message: "unclosed text interpolation".to_owned(),
                byte_offset: None,
            })?;
        let expression = placeholder[1..end].trim();
        if expression.is_empty() {
            return Err(RawParseError {
                message: "text interpolation requires a nonempty expression".into(),
                byte_offset: None,
            });
        }
        parts.push(TextPart::Expr(expression.to_owned()));
        remaining = &placeholder[end + 1..];
    }
    if !remaining.is_empty() {
        parts.push(TextPart::Literal(unescape_crepus_text_literal(remaining)));
    }
    Ok(parts)
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
