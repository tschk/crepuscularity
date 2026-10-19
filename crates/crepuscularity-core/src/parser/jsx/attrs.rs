//! JSX attribute parsing.

use super::super::{match_delimiter, RawParseError};
use super::text::jsx_err;

type ExpressionBoundary<'a> = &'a dyn Fn(&str) -> Option<usize>;

pub(crate) struct JsxAttr {
    pub key: String,
    pub value: JsxAttrValue,
}

pub(crate) enum JsxAttrItem {
    Named(JsxAttr),
    Spread(String),
}

pub(crate) enum JsxAttrValue {
    Bool(bool),
    Str(String),
    Expr(String),
}

impl JsxAttr {
    pub(crate) fn event_handler(&self) -> Result<String, &'static str> {
        match &self.value {
            JsxAttrValue::Str(handler) | JsxAttrValue::Expr(handler)
                if !handler.trim().is_empty() =>
            {
                Ok(handler.trim().to_string())
            }
            _ => Err("event directives require a nonempty handler"),
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        if let JsxAttrValue::Str(s) = &self.value {
            Some(s)
        } else {
            None
        }
    }

    /// Returns an evaluable expression string for the attribute value.
    pub(crate) fn as_expr(&self) -> Option<String> {
        match &self.value {
            JsxAttrValue::Expr(e) => Some(e.clone()),
            JsxAttrValue::Str(s) => Some(super::super::quote_expression_string(s)),
            JsxAttrValue::Bool(b) => Some(b.to_string()),
        }
    }
}

pub(crate) fn jsx_parse_attrs<'a>(
    norm_root: &'a str,
    src: &'a str,
) -> Result<(Vec<JsxAttr>, &'a str, bool), RawParseError> {
    let (items, rest, closed) = parse_attrs_inner(norm_root, src, None, false, false)?;
    Ok((named_attrs(items), rest, closed))
}

pub(crate) fn jsx_parse_attrs_with_boundary<'a>(
    norm_root: &'a str,
    src: &'a str,
    boundary: fn(&str) -> Option<usize>,
) -> Result<(Vec<JsxAttr>, &'a str, bool), RawParseError> {
    let (items, rest, closed) = parse_attrs_inner(norm_root, src, Some(&boundary), false, true)?;
    Ok((named_attrs(items), rest, closed))
}

/// Custom expression grammar with ordinary literal quoted attributes.
pub(crate) fn jsx_parse_attrs_with_expression_boundary<'a>(
    norm_root: &'a str,
    src: &'a str,
    boundary: impl Fn(&str) -> Option<usize>,
) -> Result<(Vec<JsxAttr>, &'a str, bool), RawParseError> {
    let (items, rest, closed) = parse_attrs_inner(norm_root, src, Some(&boundary), false, false)?;
    Ok((named_attrs(items), rest, closed))
}

/// Authored Astro preserves spread items while keeping quoted attributes literal.
pub(crate) fn jsx_parse_authored_attrs_with_expression_boundary<'a>(
    norm_root: &'a str,
    src: &'a str,
    boundary: impl Fn(&str) -> Option<usize>,
) -> Result<(Vec<JsxAttrItem>, &'a str, bool), RawParseError> {
    parse_attrs_inner(norm_root, src, Some(&boundary), true, false)
}

fn named_attrs(items: Vec<JsxAttrItem>) -> Vec<JsxAttr> {
    items
        .into_iter()
        .map(|item| match item {
            JsxAttrItem::Named(attribute) => attribute,
            JsxAttrItem::Spread(_) => unreachable!("spreads disabled"),
        })
        .collect()
}

pub(crate) fn jsx_parse_authored_attrs<'a>(
    norm_root: &'a str,
    src: &'a str,
    boundary: fn(&str) -> Option<usize>,
) -> Result<(Vec<JsxAttrItem>, &'a str, bool), RawParseError> {
    parse_attrs_inner(norm_root, src, Some(&boundary), true, true)
}

fn parse_attrs_inner<'a>(
    norm_root: &'a str,
    src: &'a str,
    boundary: Option<ExpressionBoundary<'_>>,
    authored: bool,
    interpolate_quoted: bool,
) -> Result<(Vec<JsxAttrItem>, &'a str, bool), RawParseError> {
    let mut attrs = Vec::new();
    let mut rest = src.trim_start();
    let mut self_closing = false;

    loop {
        rest = rest.trim_start();
        if rest.is_empty() {
            return Err(jsx_err(norm_root, rest, "unclosed JSX tag"));
        }
        if rest.starts_with("/>") {
            self_closing = true;
            rest = &rest[2..];
            break;
        }
        if rest.starts_with('>') {
            rest = &rest[1..];
            break;
        }

        if rest
            .strip_prefix('{')
            .is_some_and(|tail| tail.trim_start().starts_with("..."))
        {
            if !authored {
                return Err(jsx_err(
                    norm_root,
                    rest,
                    "spread attributes are not supported: use named bindings instead",
                ));
            }
            let end = boundary.unwrap_or(&default_boundary)(rest)
                .ok_or_else(|| jsx_err(norm_root, rest, "unclosed spread attribute"))?;
            let expression = rest[1..end]
                .trim_start()
                .strip_prefix("...")
                .unwrap()
                .trim();
            if expression.is_empty() {
                return Err(jsx_err(norm_root, rest, "spread requires an expression"));
            }
            let mut expression = expression.to_string();
            if expression.contains("//") {
                expression.push('\n');
            }
            rest = rest[end + 1..].trim_start();
            if rest.starts_with('=') {
                return Err(jsx_err(
                    norm_root,
                    rest,
                    "spread attributes cannot have an assigned value",
                ));
            }
            attrs.push(JsxAttrItem::Spread(expression));
            continue;
        }

        let key_end = if rest.starts_with('{') {
            boundary.unwrap_or(&default_boundary)(rest)
                .ok_or_else(|| jsx_err(norm_root, rest, "unclosed shorthand attribute"))?
                + 1
        } else {
            rest.find(|c: char| c.is_whitespace() || c == '=' || c == '>' || c == '/')
                .unwrap_or(rest.len())
        };
        if key_end == 0 {
            return Err(jsx_err(
                norm_root,
                rest,
                "expected an attribute name or tag terminator",
            ));
        }
        let key = rest[..key_end].to_string();
        rest = rest[key_end..].trim_start();

        if rest.starts_with('=') {
            if key.starts_with('{') {
                return Err(jsx_err(
                    norm_root,
                    rest,
                    "shorthand attributes cannot have an assigned value",
                ));
            }
            rest = rest[1..].trim_start();
            let (value, next) = if boundary.is_some() {
                attr_value_inner(norm_root, rest, boundary, interpolate_quoted)?
            } else {
                jsx_attr_value(norm_root, rest)?
            };
            // Svelte bare attribute tokens are strings; only omitted values
            // mean boolean true (including binding shorthand).
            let value = match value {
                JsxAttrValue::Bool(value) if authored => JsxAttrValue::Str(value.to_string()),
                other => other,
            };
            attrs.push(JsxAttrItem::Named(JsxAttr { key, value }));
            rest = next;
        } else {
            attrs.push(JsxAttrItem::Named(JsxAttr {
                key,
                value: JsxAttrValue::Bool(true),
            }));
        }
    }

    Ok((attrs, rest, self_closing))
}

pub(crate) fn jsx_attr_value<'a>(
    norm_root: &'a str,
    src: &'a str,
) -> Result<(JsxAttrValue, &'a str), RawParseError> {
    attr_value_inner(norm_root, src, None, false)
}

fn default_boundary(src: &str) -> Option<usize> {
    match_delimiter(src, '{', '}')
}

fn attr_value_inner<'a>(
    norm_root: &'a str,
    src: &'a str,
    boundary: Option<ExpressionBoundary<'_>>,
    interpolate_quoted: bool,
) -> Result<(JsxAttrValue, &'a str), RawParseError> {
    if src.starts_with(['"', '\'']) {
        let bytes = src.as_bytes();
        let quote = bytes[0];
        let mut i = 1;
        let mut literal_start = 1;
        let mut authored_content = String::new();
        let decode_literal = |literal: &str| {
            if quote == b'"' {
                literal.replace("\\\"", "\"")
            } else {
                literal.replace("\\'", "'")
            }
        };
        while i < bytes.len() {
            if let (b'{', Some(boundary)) = (bytes[i], boundary.filter(|_| interpolate_quoted)) {
                let end = boundary(&src[i..]).ok_or_else(|| {
                    jsx_err(
                        norm_root,
                        &src[i..],
                        "unclosed quoted attribute interpolation",
                    )
                })?;
                authored_content.push_str(&decode_literal(&src[literal_start..i]));
                authored_content.push_str(&src[i..i + end + 1]);
                i += end + 1;
                literal_start = i;
            } else if bytes[i] == b'\\' {
                i += 2;
            } else if bytes[i] == quote {
                let content = if interpolate_quoted && boundary.is_some() {
                    authored_content.push_str(&decode_literal(&src[literal_start..i]));
                    authored_content
                } else if quote == b'"' {
                    src[1..i].replace("\\\"", "\"")
                } else {
                    src[1..i].replace("\\'", "'")
                };
                return Ok((JsxAttrValue::Str(content), &src[i + 1..]));
            } else {
                i += 1;
            }
        }
        Err(jsx_err(norm_root, src, "unclosed quoted JSX attribute"))
    } else if src.starts_with('{') {
        let (mut expr, rest) =
            brace_expr_inner(norm_root, src, boundary.unwrap_or(&default_boundary))?;
        if boundary.is_some() && expr.contains("//") {
            expr.push('\n');
        }
        Ok((JsxAttrValue::Expr(expr), rest))
    } else {
        let end = src
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .unwrap_or(src.len());
        if end == 0 {
            return Err(jsx_err(
                norm_root,
                src,
                "attribute requires a value after '='",
            ));
        }
        let val = &src[..end];
        let value = match val {
            "true" => JsxAttrValue::Bool(true),
            "false" => JsxAttrValue::Bool(false),
            other => JsxAttrValue::Str(other.to_string()),
        };
        Ok((value, &src[end..]))
    }
}

pub(crate) fn jsx_brace_expr<'a>(
    norm_root: &'a str,
    src: &'a str,
) -> Result<(String, &'a str), RawParseError> {
    brace_expr_inner(norm_root, src, &default_boundary)
}

fn brace_expr_inner<'a>(
    norm_root: &'a str,
    src: &'a str,
    boundary: ExpressionBoundary<'_>,
) -> Result<(String, &'a str), RawParseError> {
    let src = src.trim_start();
    if !src.starts_with('{') {
        return Err(jsx_err(
            norm_root,
            src,
            format!(
                "expected '{{', got: {}",
                super::super::diagnostic_prefix(src, 10)
            ),
        ));
    }
    let end =
        boundary(src).ok_or_else(|| jsx_err(norm_root, src, "unclosed '{' in JSX expression"))?;
    Ok((src[1..end].trim().to_string(), &src[end + 1..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_quotes_decode_literals_but_preserve_expression_escapes() {
        for (source, expected) in [
            (r#" title="say \"hello\"" >"#, r#"say "hello""#),
            (r#" title='it\'s fine' >"#, "it's fine"),
            (
                r#" title="say \"hello\" {"inner \"quote\""}" >"#,
                r#"say "hello" {"inner \"quote\""}"#,
            ),
        ] {
            let (attributes, rest, _) =
                jsx_parse_attrs_with_boundary(source, source, default_boundary).unwrap();
            assert_eq!(rest, "");
            let JsxAttrValue::Str(value) = &attributes[0].value else {
                panic!("expected string");
            };
            assert_eq!(value, expected);
        }
    }

    #[test]
    fn unterminated_quoted_attributes_return_errors() {
        for source in ["'", "'界", "\"", "\"界", "'escaped\\'", "\"escaped\\\""] {
            assert!(jsx_attr_value(source, source).is_err(), "accepted {source}");
        }
    }

    #[test]
    fn escaped_quotes_preserve_attribute_boundaries() {
        for (source, expected) in [
            (r#"'it\'s 界' trailing"#, "it's 界"),
            (r#""say \"界\"" trailing"#, "say \"界\""),
        ] {
            let (JsxAttrValue::Str(value), rest) = jsx_attr_value(source, source).unwrap() else {
                panic!("expected string")
            };
            assert_eq!(value, expected);
            assert_eq!(rest, " trailing");
        }
    }

    #[test]
    fn braces_in_strings_do_not_end_expressions() {
        for expression in [
            r#""}""#,
            r#"'{'"#,
            r#"{ key: "}" }"#,
            r#""a\"}b""#,
            r#"`界}界`"#,
        ] {
            let source = format!("{{{expression}}} trailing");
            let (parsed, rest) = jsx_brace_expr(&source, &source).expect("balanced expression");
            assert_eq!(parsed, expression);
            assert_eq!(rest, " trailing");
        }
    }

    #[test]
    fn unclosed_string_does_not_close_expression() {
        let source = "{\"unterminated }";
        assert!(jsx_brace_expr(source, source).is_err());
    }
}
