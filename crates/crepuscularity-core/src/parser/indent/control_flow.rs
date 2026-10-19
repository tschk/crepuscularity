//! Control-flow node parsing (if, for, match, let).

use crate::ast::*;

use super::super::RawParseError;
use super::attrs::merge_attr_only_children;
use super::element::{parse_element_line, parse_text_template};
use super::include::{try_parse_embed, try_parse_include};

/// Maximum nesting depth for recursive `parse_nodes` calls.
/// Prevents stack overflow on pathologically deep input.
const MAX_DEPTH: usize = 256;

pub(crate) fn parse_nodes(
    lines: &[(usize, String)],
    start: usize,
    expected_indent: usize,
) -> Result<(Vec<Node>, usize), RawParseError> {
    parse_nodes_with_depth(lines, start, expected_indent, 0)
}

fn parse_nodes_with_depth(
    lines: &[(usize, String)],
    start: usize,
    expected_indent: usize,
    depth: usize,
) -> Result<(Vec<Node>, usize), RawParseError> {
    stacker::maybe_grow(512 * 1024, 2 * 1024 * 1024, || {
        parse_nodes_with_depth_inner(lines, start, expected_indent, depth)
    })
}

fn parse_nodes_with_depth_inner(
    lines: &[(usize, String)],
    start: usize,
    expected_indent: usize,
    depth: usize,
) -> Result<(Vec<Node>, usize), RawParseError> {
    if depth >= MAX_DEPTH {
        return Err(RawParseError {
            message: format!("maximum nesting depth ({MAX_DEPTH}) exceeded"),
            byte_offset: None,
        });
    }

    let mut nodes = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let (indent, line) = &lines[i];

        if *indent < expected_indent {
            break;
        }
        if *indent > expected_indent {
            return Err(RawParseError {
                message: format!("unexpected indentation before '{line}'"),
                byte_offset: None,
            });
        }

        validate_control_expression(line)?;

        // `else` and `else if` belong to the caller's `if`
        if line == "else" || line.starts_with("else if ") {
            break;
        }

        // Match arm terminators
        if line.ends_with(" =>") || line == "_ =>" {
            break;
        }

        if let Some(embed) = try_parse_embed(line).map_err(|message| RawParseError {
            message: message.to_string(),
            byte_offset: None,
        })? {
            nodes.push(Node::Embed(embed));
            i += 1;
            continue;
        }

        // include directive
        if let Some(mut inc) = try_parse_include(line).map_err(|message| RawParseError {
            message: message.to_string(),
            byte_offset: None,
        })? {
            i += 1;
            let (slot, next_i) = if i < lines.len() && lines[i].0 > expected_indent {
                let child_indent = lines[i].0;
                parse_nodes_with_depth(lines, i, child_indent, depth + 1)?
            } else {
                (vec![], i)
            };
            i = next_i;
            inc.slot = slot;
            nodes.push(Node::Include(inc));
            continue;
        }

        // $: let declaration
        if let Some(decl) = try_parse_let_decl(line)? {
            nodes.push(Node::LetDecl(decl));
            i += 1;
            continue;
        }

        // match block
        if let Some(expr) = try_parse_match(line) {
            i += 1;
            let (arms, next_i) = if i < lines.len()
                && (lines[i].0 > expected_indent
                    || (lines[i].0 == expected_indent
                        && try_parse_match_arm(&lines[i].1).is_some()))
            {
                parse_match_arms_with_depth(lines, i, lines[i].0, depth + 1)?
            } else {
                (vec![], i)
            };
            i = next_i;
            nodes.push(Node::Match(MatchBlock { expr, arms }));
            continue;
        }

        // if block
        if try_parse_if(line).is_some() {
            let (node, next_i) = parse_if_node_with_depth(lines, i, expected_indent, depth + 1)?;
            i = next_i;
            nodes.push(node);
            continue;
        }

        i += 1;

        // Children: lines with strictly greater indent
        let (children, next_i) = if i < lines.len() && lines[i].0 > expected_indent {
            let child_indent = lines[i].0;
            parse_nodes_with_depth(lines, i, child_indent, depth + 1)?
        } else {
            (vec![], i)
        };
        i = next_i;

        if let Some((pattern, iterator)) = try_parse_for(line) {
            nodes.push(Node::For(ForBlock {
                pattern,
                iterator,
                body: children,
            }));
        } else if line.starts_with('"') {
            let parts = parse_text_template(line)?;
            nodes.push(Node::Text(parts));
        } else if is_raw_html_expr(line) {
            let inner = line[2..line.len() - 1].trim();
            if inner.is_empty() {
                return Err(RawParseError {
                    message: "raw HTML requires a nonempty expression".into(),
                    byte_offset: None,
                });
            }
            nodes.push(Node::RawHtml(inner.to_string()));
        } else if is_raw_expr(line) {
            // Raw expressions — rendered as evaluated text
            let inner = line[1..line.len() - 1].trim();
            if inner.is_empty() {
                return Err(RawParseError {
                    message: "raw text requires a nonempty expression".into(),
                    byte_offset: None,
                });
            }
            nodes.push(Node::RawText(inner.to_string()));
        } else {
            let element = merge_attr_only_children(parse_element_line(line, children)?)?;
            nodes.push(Node::Element(element));
        }
    }

    Ok((nodes, i))
}

fn parse_if_node_with_depth(
    lines: &[(usize, String)],
    i: usize,
    expected_indent: usize,
    depth: usize,
) -> Result<(Node, usize), RawParseError> {
    if depth >= MAX_DEPTH {
        return Err(RawParseError {
            message: format!("maximum nesting depth ({MAX_DEPTH}) exceeded"),
            byte_offset: None,
        });
    }

    let line = &lines[i].1;
    validate_control_expression(line)?;
    let condition = try_parse_if(line).unwrap_or_default();
    let mut i = i + 1;

    let (then_children, next_i) = if i < lines.len() && lines[i].0 > expected_indent {
        let child_indent = lines[i].0;
        parse_nodes_with_depth(lines, i, child_indent, depth + 1)?
    } else {
        (vec![], i)
    };
    i = next_i;

    let else_children = if i < lines.len() && lines[i].0 == expected_indent {
        let else_line = &lines[i].1;
        if else_line == "else" {
            i += 1;
            if i < lines.len() && lines[i].0 > expected_indent {
                let else_indent = lines[i].0;
                let (else_nodes, next_i) =
                    parse_nodes_with_depth(lines, i, else_indent, depth + 1)?;
                i = next_i;
                Some(else_nodes)
            } else {
                Some(vec![])
            }
        } else if else_line.starts_with("else if ") {
            let rewritten = else_line
                .strip_prefix("else ")
                .unwrap_or(else_line)
                .to_string();
            let mut patched = lines.to_vec();
            patched[i].1 = rewritten;
            let (else_if_node, next_i) =
                parse_if_node_with_depth(&patched, i, expected_indent, depth + 1)?;
            i = next_i;
            Some(vec![else_if_node])
        } else {
            None
        }
    } else {
        None
    };

    Ok((
        Node::If(IfBlock {
            condition,
            then_children,
            else_children,
        }),
        i,
    ))
}

fn parse_match_arms_with_depth(
    lines: &[(usize, String)],
    start: usize,
    expected_indent: usize,
    depth: usize,
) -> Result<(Vec<MatchArm>, usize), RawParseError> {
    if depth >= MAX_DEPTH {
        return Err(RawParseError {
            message: format!("maximum nesting depth ({MAX_DEPTH}) exceeded"),
            byte_offset: None,
        });
    }

    let mut arms = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let (indent, line) = &lines[i];
        if *indent < expected_indent {
            break;
        }
        if *indent > expected_indent {
            return Err(RawParseError {
                message: format!("unexpected indentation before match arm '{line}'"),
                byte_offset: None,
            });
        }

        if let Some(pattern) = try_parse_match_arm(line) {
            if pattern.is_empty() {
                return Err(RawParseError {
                    message: "match arm requires a nonempty pattern".into(),
                    byte_offset: None,
                });
            }
            i += 1;
            let (body, next_i) = if i < lines.len() && lines[i].0 > expected_indent {
                let body_indent = lines[i].0;
                parse_nodes_with_depth(lines, i, body_indent, depth + 1)?
            } else {
                (vec![], i)
            };
            i = next_i;
            arms.push(MatchArm { pattern, body });
        } else {
            break;
        }
    }

    Ok((arms, i))
}

fn validate_control_expression(line: &str) -> Result<(), RawParseError> {
    if matches!(
        line,
        "if" | "else if" | "for" | "match" | "$: let" | "$: default"
    ) {
        return Err(RawParseError {
            message: format!("`{line}` requires arguments"),
            byte_offset: None,
        });
    }
    if let Some(header) = line.strip_prefix("for ") {
        let (pattern, iterator) = header.split_once(" in ").ok_or_else(|| RawParseError {
            message: "loop requires a variable and `in` iterator".into(),
            byte_offset: None,
        })?;
        if pattern.trim().is_empty() || iterator.trim().is_empty() {
            return Err(RawParseError {
                message: "loop requires a nonempty variable and iterator".into(),
                byte_offset: None,
            });
        }
        if !crate::eval::is_variable_name(pattern.trim()) {
            return Err(RawParseError {
                message: "loop requires a supported item variable name".into(),
                byte_offset: None,
            });
        }
    }
    let expression = line
        .strip_prefix("if ")
        .or_else(|| line.strip_prefix("match "))
        .or_else(|| {
            line.strip_prefix("for ")
                .and_then(|rest| rest.split_once(" in ").map(|(_, expression)| expression))
        })
        .or_else(|| {
            line.strip_prefix("$: let ")
                .or_else(|| line.strip_prefix("$: default "))
                .and_then(|rest| rest.split_once('=').map(|(_, expression)| expression))
        });
    if let Some(expression) = expression
        .map(str::trim)
        .filter(|expression| expression.starts_with('{'))
    {
        let end =
            super::super::match_delimiter(expression, '{', '}').ok_or_else(|| RawParseError {
                message: "control-flow expression requires a closing brace".into(),
                byte_offset: None,
            })?;
        if expression[1..end].trim().is_empty() {
            return Err(RawParseError {
                message: "control flow requires a nonempty expression".into(),
                byte_offset: None,
            });
        }
        if !expression[end + 1..].trim().is_empty() {
            return Err(RawParseError {
                message: "unexpected text after control-flow expression".into(),
                byte_offset: None,
            });
        }
    }
    Ok(())
}

fn try_parse_if(line: &str) -> Option<String> {
    let rest = line.strip_prefix("if ")?;
    Some(extract_braced(rest.trim()).unwrap_or_else(|| rest.trim().to_string()))
}

fn try_parse_for(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("for ")?;
    let in_pos = rest.find(" in ")?;
    let pattern = rest[..in_pos].trim().to_string();
    let after_in = rest[in_pos + 4..].trim();
    let iterator = extract_braced(after_in).unwrap_or_else(|| after_in.to_string());
    Some((pattern, iterator))
}

fn try_parse_match(line: &str) -> Option<String> {
    let rest = line.strip_prefix("match ")?;
    Some(extract_braced(rest.trim()).unwrap_or_else(|| rest.trim().to_string()))
}

fn try_parse_match_arm(line: &str) -> Option<String> {
    let pattern = line.strip_suffix(" =>")?;
    let pattern = pattern.trim();
    if pattern.starts_with('{') && pattern.ends_with('}') {
        Some(pattern[1..pattern.len() - 1].trim().to_string())
    } else {
        Some(pattern.to_string())
    }
}

pub(crate) fn try_parse_let_decl(line: &str) -> Result<Option<LetDecl>, RawParseError> {
    let Some((rest, is_default)) = line
        .strip_prefix("$: default ")
        .map(|r| (r, true))
        .or_else(|| line.strip_prefix("$: let ").map(|r| (r, false)))
    else {
        return Ok(None);
    };
    validate_control_expression(line)?;
    let eq_pos = rest.find('=').ok_or_else(|| RawParseError {
        message: "declaration requires a variable assignment".into(),
        byte_offset: None,
    })?;
    let name = rest[..eq_pos].trim().to_string();
    let expr_str = rest[eq_pos + 1..].trim();
    let expr = extract_braced(expr_str).unwrap_or_else(|| expr_str.to_string());
    if name.is_empty() || expr.is_empty() {
        return Err(RawParseError {
            message: "declaration requires a nonempty variable name and expression".into(),
            byte_offset: None,
        });
    }
    if !crate::eval::is_variable_name(&name) {
        return Err(RawParseError {
            message: "declaration requires a supported variable name".into(),
            byte_offset: None,
        });
    }
    Ok(Some(LetDecl {
        name,
        expr,
        is_default,
    }))
}

fn is_raw_expr(line: &str) -> bool {
    line.starts_with('{') && line.ends_with('}') && {
        let inner = &line[1..line.len() - 1];
        !inner.starts_with('=')
    }
}

fn is_raw_html_expr(line: &str) -> bool {
    line.starts_with("{=") && line.ends_with('}')
}

fn extract_braced(s: &str) -> Option<String> {
    if !s.starts_with('{') {
        return None;
    }
    let end = super::super::match_delimiter(s, '{', '}')?;
    Some(s[1..end].trim().to_string())
}
