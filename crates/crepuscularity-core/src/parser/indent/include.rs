//! Include and embed directive parsing.

use crate::ast::*;

pub(crate) fn try_parse_include(line: &str) -> Result<Option<IncludeNode>, &'static str> {
    if line == "include" {
        return Err("include directive requires a source path");
    }
    let Some(rest) = line
        .strip_prefix("include")
        .filter(|rest| rest.starts_with(char::is_whitespace))
    else {
        return Ok(None);
    };
    let rest = rest.trim_start();
    let (path, props_str) = extract_source_path(rest)?;
    if path.is_empty() {
        return Err("include directive requires a source path");
    }
    let props = parse_props(props_str)?;
    Ok(Some(IncludeNode {
        path,
        props,
        slot: vec![],
    }))
}

pub(crate) fn try_parse_embed(line: &str) -> Result<Option<EmbedNode>, &'static str> {
    if line == "embed" {
        return Err("embed directive requires a source path");
    }
    let Some(rest) = line
        .strip_prefix("embed")
        .filter(|rest| rest.starts_with(char::is_whitespace))
    else {
        return Ok(None);
    };
    let rest = rest.trim_start();
    let (src, props_str) = extract_source_path(rest)?;
    if src.is_empty() {
        return Err("embed directive requires a source path");
    }
    let mut props = parse_props(props_str)?;
    let adapter = take_literal_prop(&mut props, "adapter");
    Ok(Some(EmbedNode {
        src,
        adapter,
        props,
    }))
}

fn extract_source_path(source: &str) -> Result<(String, &str), &'static str> {
    if source.starts_with(['"', '\'']) {
        let (expression, rest) = extract_prop_value(source)?;
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return Err("directive source path requires a whitespace separator");
        }
        return Ok((
            unquote_expr_string(&expression).unwrap_or(expression),
            rest.trim(),
        ));
    }
    let end = source.find(char::is_whitespace).unwrap_or(source.len());
    Ok((source[..end].to_string(), source[end..].trim()))
}

fn take_literal_prop(props: &mut Vec<(String, String)>, key: &str) -> Option<String> {
    let pos = props.iter().position(|(k, _)| k == key)?;
    let (_, value) = props.remove(pos);
    Some(unquote_expr_string(&value).unwrap_or(value))
}

fn unquote_expr_string(value: &str) -> Option<String> {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        Some(crate::parser::unescape_crepus_text_literal(
            &value[1..value.len() - 1],
        ))
    } else {
        None
    }
}

fn parse_props(s: &str) -> Result<Vec<(String, String)>, &'static str> {
    let mut props: Vec<(String, String)> = Vec::new();
    let mut remaining = s.trim();

    while !remaining.is_empty() {
        // Find key= (key is an identifier, no spaces)
        let eq_pos = match remaining.find('=') {
            Some(p) => p,
            None => return Err("directive prop must have a key and value"),
        };
        let key = remaining[..eq_pos].trim().to_string();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err("invalid directive prop name");
        }
        remaining = remaining[eq_pos + 1..].trim_start();
        if props.iter().any(|(name, _)| name == &key) {
            return Err("duplicate directive prop");
        }

        // Extract value
        let (expr_str, rest) = extract_prop_value(remaining)?;
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return Err("directive props require whitespace separators");
        }
        props.push((key, expr_str));
        remaining = rest.trim_start();
    }

    Ok(props)
}

/// Extract a prop value token from the start of `s`.
/// Returns `(expr_string, remaining)`.
/// - `"quoted"` → returns the string content wrapped in quotes for the evaluator
/// - `{expr}` → returns the inner expr string
/// - `bare_token` → returns the token as-is (treated as a variable name / literal)
fn extract_prop_value(s: &str) -> Result<(String, &str), &'static str> {
    if s.is_empty() {
        return Err("missing directive prop value");
    }

    if s.starts_with('"') || s.starts_with('\'') {
        let quote = s.as_bytes()[0];
        let mut i = 1;
        let mut escaped = false;
        while i < s.len() {
            let byte = s.as_bytes()[i];
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                let content = &s[1..i];
                let expr = crate::parser::quote_expression_string(content);
                let rest = if i < s.len() { &s[i + 1..] } else { "" };
                return Ok((expr, rest));
            }
            i += 1;
        }

        return Err("unclosed quoted directive prop value");
    }

    if s.starts_with('{') {
        if let Some(close) = crate::parser::match_delimiter(s, '{', '}') {
            let expression = s[1..close].trim();
            if expression.is_empty() {
                return Err("empty directive prop expression");
            }
            return Ok((expression.to_string(), &s[close + 1..]));
        }
        return Err("unclosed braced directive prop value");
    }

    // Bare token: ends at the next whitespace separator.
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    Ok((s[..end].to_string(), &s[end..]))
}
