//! Svelte logic blocks: `{#if}`, `{#each}`.

use crate::ast::*;

use super::super::jsx::attrs::jsx_brace_expr;
use super::super::{scan_top_level, RawParseError};
use super::{parse_svelte_nodes, svelte_err};

/// Read a `{…}` tag, returning its trimmed inner text and the remaining source.
pub(crate) fn read_brace_tag<'a>(
    root: &'a str,
    src: &'a str,
) -> Result<(String, &'a str), RawParseError> {
    jsx_brace_expr(root, src)
}

pub(crate) fn parse_svelte_block<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
) -> Result<(Node, &'a str), RawParseError> {
    let (inner, rest) = read_brace_tag(root, src)?;

    if let Some(cond) = inner.strip_prefix("#if ") {
        let (block, next) = parse_if(root, cond.trim().to_string(), rest, depth)?;
        return Ok((Node::If(block), next));
    }
    if let Some(head) = inner.strip_prefix("#each ") {
        return parse_each(root, src, head.trim(), rest, depth);
    }

    let name = inner
        .split_whitespace()
        .next()
        .unwrap_or("#")
        .trim_start_matches('#')
        .to_string();
    Err(svelte_err(
        root,
        src,
        format!(
            "`{{#{name}}}` is not supported by the Svelte frontend \
             (supported blocks: `{{#if}}`, `{{#each}}`)"
        ),
    ))
}

fn parse_if<'a>(
    root: &'a str,
    condition: String,
    body_src: &'a str,
    depth: usize,
) -> Result<(IfBlock, &'a str), RawParseError> {
    let (then_children, rest) = parse_svelte_nodes(root, body_src, depth + 1)?;
    let rest = rest.trim_start();

    if rest.starts_with("{:") {
        let (inner, after_tag) = read_brace_tag(root, rest)?;
        if let Some(cond) = inner.strip_prefix(":else if ") {
            let (nested, next) = parse_if(root, cond.trim().to_string(), after_tag, depth)?;
            return Ok((
                IfBlock {
                    condition,
                    then_children,
                    else_children: Some(vec![Node::If(nested)]),
                },
                next,
            ));
        }
        if inner.trim() == ":else" {
            let (else_children, after_else) = parse_svelte_nodes(root, after_tag, depth + 1)?;
            let next = expect_close(root, after_else, "if")?;
            return Ok((
                IfBlock {
                    condition,
                    then_children,
                    else_children: Some(else_children),
                },
                next,
            ));
        }
        return Err(svelte_err(
            root,
            rest,
            format!("unexpected `{{{inner}}}` inside `{{#if}}`"),
        ));
    }

    let next = expect_close(root, rest, "if")?;
    Ok((
        IfBlock {
            condition,
            then_children,
            else_children: None,
        },
        next,
    ))
}

fn parse_each<'a>(
    root: &'a str,
    tag_src: &'a str,
    head: &str,
    body_src: &'a str,
    depth: usize,
) -> Result<(Node, &'a str), RawParseError> {
    let Some(as_pos) = scan_top_level(head, |i, _| head[i..].starts_with(" as ")) else {
        return Err(svelte_err(
            root,
            tag_src,
            "`{#each …}` without an `as` binding is not supported \
             (the shared ForBlock always binds an item variable)",
        ));
    };

    let iterator = head[..as_pos].trim().to_string();
    let mut binding = head[as_pos + 4..].trim();

    // Drop a trailing `(key)` keyed-each expression — keying is a DOM-diffing
    // concern the shared IR does not model.
    if let Some(open) = binding.find('(') {
        let key = &binding[open..];
        let close = super::super::match_delimiter(key, '(', ')')
            .ok_or_else(|| svelte_err(root, tag_src, "unclosed keyed-each expression"))?;
        if key[1..close].trim().is_empty() || !key[close + 1..].trim().is_empty() {
            return Err(svelte_err(
                root,
                tag_src,
                "keyed-each requires one nonempty key expression with no trailing text",
            ));
        }
        binding = binding[..open].trim();
    }

    if binding.is_empty() {
        return Err(svelte_err(
            root,
            tag_src,
            "`{#each}` requires a nonempty item binding before its key",
        ));
    }

    if binding.contains(',') {
        return Err(svelte_err(
            root,
            tag_src,
            "`{#each … as item, i}` index bindings are not supported: the shared \
             ForBlock has no index variable, so `i` would be undefined at render time",
        ));
    }
    if binding.starts_with('{') || binding.starts_with('[') {
        return Err(svelte_err(
            root,
            tag_src,
            "destructuring `{#each … as {a, b}}` is not supported: the shared \
             ForBlock binds a single item variable",
        ));
    }

    if !crate::eval::is_variable_name(binding) {
        return Err(svelte_err(
            root,
            tag_src,
            "loop requires a supported item variable name",
        ));
    }

    let (body, rest) = parse_svelte_nodes(root, body_src, depth + 1)?;
    let rest = rest.trim_start();

    if rest.starts_with("{:") {
        let (inner, _) = read_brace_tag(root, rest)?;
        if inner.trim() == ":else" {
            return Err(svelte_err(
                root,
                rest,
                "`{#each}…{:else}` is not supported: the shared ForBlock has no \
                 empty-list branch",
            ));
        }
        return Err(svelte_err(
            root,
            rest,
            format!("unexpected `{{{inner}}}` inside `{{#each}}`"),
        ));
    }

    let next = expect_close(root, rest, "each")?;
    Ok((
        Node::For(ForBlock {
            pattern: binding.to_string(),
            iterator,
            body,
        }),
        next,
    ))
}

fn expect_close<'a>(root: &'a str, src: &'a str, name: &str) -> Result<&'a str, RawParseError> {
    let src = src.trim_start();
    if src.starts_with("{/") {
        let (inner, rest) = read_brace_tag(root, src)?;
        if inner.trim() == format!("/{name}") {
            return Ok(rest);
        }
        return Err(svelte_err(
            root,
            src,
            format!("expected `{{/{name}}}`, found `{{{inner}}}`"),
        ));
    }
    Err(svelte_err(
        root,
        src,
        format!(
            "expected `{{/{name}}}`, found: {}",
            super::super::diagnostic_prefix(src, 40)
        ),
    ))
}
