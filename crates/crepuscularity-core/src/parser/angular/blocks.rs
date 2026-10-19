//! Angular built-in control-flow blocks: `@if` / `@else` / `@for`.

use crate::ast::*;

use super::super::{match_delimiter, scan_top_level, RawParseError};
use super::{angular_err, block_starter_at, parse_angular_nodes};

pub(crate) fn parse_angular_block<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    match block_starter_at(src) {
        Some("if") => {
            let (block, rest) = parse_if(root, &src["@if".len()..], src, depth)?;
            Ok((vec![Node::If(block)], rest))
        }
        Some("for") => parse_for(root, &src["@for".len()..], src, depth),
        Some(name) => Err(angular_err(
            root,
            src,
            format!(
                "`@{name}` blocks are not supported by the Angular frontend \
                 (supported blocks: `@if`, `@else`, `@for`)"
            ),
        )),
        None => {
            let name = src
                .trim_start_matches('@')
                .split(|c: char| !c.is_alphanumeric())
                .next()
                .unwrap_or("")
                .to_string();
            Err(angular_err(
                root,
                src,
                format!("`@{name}` without a preceding `@if` or `@for` block"),
            ))
        }
    }
}

/// Read a parenthesised block head, returning its inner slice and the rest.
fn read_head<'a>(
    root: &'a str,
    src: &'a str,
    at: &'a str,
    keyword: &str,
) -> Result<(&'a str, &'a str), RawParseError> {
    let src = src.trim_start();
    if !src.starts_with('(') {
        return Err(angular_err(
            root,
            at,
            format!("`@{keyword}` must be followed by a parenthesised expression"),
        ));
    }
    let end = match_delimiter(src, '(', ')')
        .ok_or_else(|| angular_err(root, at, format!("unclosed `(` in `@{keyword}` block head")))?;
    Ok((src[1..end].trim(), &src[end + 1..]))
}

/// Parse a `{ … }` block body.
fn read_body<'a>(
    root: &'a str,
    src: &'a str,
    at: &'a str,
    keyword: &str,
    depth: usize,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    let src = src.trim_start();
    if !src.starts_with('{') {
        return Err(angular_err(
            root,
            at,
            format!("`@{keyword}` block body must be wrapped in `{{ … }}`"),
        ));
    }
    let (nodes, rest) = parse_angular_nodes(root, &src[1..], depth + 1, true)?;
    let rest = rest.trim_start();
    let Some(rest) = rest.strip_prefix('}') else {
        return Err(angular_err(
            root,
            rest,
            format!(
                "expected `}}` closing the `@{keyword}` block, found: {}",
                super::super::diagnostic_prefix(rest, 40)
            ),
        ));
    };
    Ok((nodes, rest))
}

fn parse_if<'a>(
    root: &'a str,
    src: &'a str,
    at: &'a str,
    depth: usize,
) -> Result<(IfBlock, &'a str), RawParseError> {
    let (condition, rest) = read_head(root, src, at, "if")?;
    if scan_top_level(condition, |index, character| {
        character == ';'
            || (character == 'a'
                && condition[..index].ends_with(char::is_whitespace)
                && condition[index..]
                    .strip_prefix("as")
                    .is_some_and(|rest| rest.starts_with(char::is_whitespace)))
    })
    .is_some()
    {
        return Err(angular_err(
            root,
            at,
            "`@if (expr; as name)` aliases are not supported: the shared IfBlock \
             binds no variable",
        ));
    }
    let (then_children, rest) = read_body(root, rest, at, "if", depth)?;

    let tail = rest.trim_start();
    if let Some(after_else) = tail.strip_prefix("@else") {
        let after_else_trimmed = after_else.trim_start();
        if let Some(after_if) = after_else_trimmed.strip_prefix("if") {
            let (nested, next) = parse_if(root, after_if, tail, depth + 1)?;
            return Ok((
                IfBlock {
                    condition: condition.to_string(),
                    then_children,
                    else_children: Some(vec![Node::If(nested)]),
                },
                next,
            ));
        }
        let (else_children, next) = read_body(root, after_else, tail, "else", depth)?;
        return Ok((
            IfBlock {
                condition: condition.to_string(),
                then_children,
                else_children: Some(else_children),
            },
            next,
        ));
    }

    Ok((
        IfBlock {
            condition: condition.to_string(),
            then_children,
            else_children: None,
        },
        rest,
    ))
}

fn parse_for<'a>(
    root: &'a str,
    src: &'a str,
    at: &'a str,
    depth: usize,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    let (head, rest) = read_head(root, src, at, "for")?;
    let (pattern, iterator) = super::builders::parse_for_head(head).map_err(|mut e| {
        e.byte_offset = Some(super::super::subslice_byte_offset(root, at));
        e
    })?;
    let (body, rest) = read_body(root, rest, at, "for", depth)?;

    let tail = rest.trim_start();
    if tail.starts_with("@empty") {
        return Err(angular_err(
            root,
            tail,
            "`@empty` is not supported: the shared ForBlock has no empty-list branch",
        ));
    }

    Ok((
        vec![Node::For(ForBlock {
            pattern,
            iterator,
            body,
        })],
        rest,
    ))
}
