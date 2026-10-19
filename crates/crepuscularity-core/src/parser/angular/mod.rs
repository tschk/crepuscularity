//! Angular component-template frontend.
//!
//! Compiles an Angular component template into the same `Node`/`Element` AST
//! every other frontend produces, so Angular markup reaches every renderer
//! unchanged. This is a native frontend — it does not depend on the Angular
//! compiler.
//!
//! Scope: this frontend compiles the TEMPLATE only. The component class is
//! never read, so `@Input`/`@Output`, pipes, dependency injection and template
//! reference variables are all outside its scope. Expressions in markup are
//! evaluated by the shared crepuscularity evaluator against the template
//! context, exactly as `{name}` is in the indentation frontend.
//!
//! Angular's directive model is close to Vue's, so structural directives lower
//! the same way `v-if` / `v-for` do and `{{ … }}` reuses the Vue text scanner.

mod blocks;
mod builders;
mod tags;
#[cfg(test)]
mod tests;

use super::RawParseError;
use crate::ast::Node;

/// Maximum element nesting depth, mirroring the JSX frontend's guard.
pub(crate) const MAX_ANGULAR_DEPTH: usize = 256;

/// Control-flow blocks that open a new construct.
pub(crate) const BLOCK_STARTERS: &[&str] = &["if", "for", "switch", "defer"];

/// Control-flow blocks that continue or terminate an enclosing construct.
pub(crate) const BLOCK_TERMINATORS: &[&str] = &[
    "else",
    "empty",
    "case",
    "default",
    "placeholder",
    "loading",
    "error",
];

pub(crate) fn parse_angular_template(src: &str) -> Result<Vec<Node>, RawParseError> {
    let (nodes, rest) = parse_angular_nodes(src, src, 0, false)?;
    let rest = rest.trim_start();
    if !rest.is_empty() {
        return Err(angular_err(
            src,
            rest,
            format!(
                "unexpected trailing markup in Angular template: {}",
                super::diagnostic_prefix(rest, 40)
            ),
        ));
    }
    Ok(nodes)
}

pub(crate) fn angular_err(root: &str, at: &str, message: impl Into<String>) -> RawParseError {
    RawParseError {
        message: message.into(),
        byte_offset: Some(super::subslice_byte_offset(root, at)),
    }
}

fn keyword_at(src: &str, table: &[&'static str]) -> Option<&'static str> {
    let rest = src.strip_prefix('@')?;
    table.iter().copied().find(|kw| {
        rest.strip_prefix(*kw)
            .is_some_and(|tail| !tail.starts_with(|c: char| c.is_alphanumeric() || c == '_'))
    })
}

/// The opening `@`-block keyword starting `src`, if any.
pub(crate) fn block_starter_at(src: &str) -> Option<&'static str> {
    keyword_at(src, BLOCK_STARTERS)
}

/// The continuing/terminating `@`-block keyword starting `src`, if any.
pub(crate) fn block_terminator_at(src: &str) -> Option<&'static str> {
    keyword_at(src, BLOCK_TERMINATORS)
}

pub(crate) fn parse_angular_nodes<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
    in_block: bool,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
        parse_angular_nodes_inner(root, src, depth, in_block)
    })
}

fn parse_angular_nodes_inner<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
    in_block: bool,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    if depth > MAX_ANGULAR_DEPTH {
        return Err(angular_err(
            root,
            src,
            format!("maximum Angular nesting depth ({MAX_ANGULAR_DEPTH}) exceeded"),
        ));
    }

    let mut nodes: Vec<Node> = Vec::new();
    let mut rest = src;

    loop {
        let t = rest.trim_start();
        if t.is_empty()
            || t.starts_with('<')
            || (in_block && t.starts_with('}'))
            || block_starter_at(t).is_some()
            || block_terminator_at(t).is_some()
        {
            // Curly block edges are syntax padding; inline body siblings still
            // retain their horizontal separators, as ordinary markup does.
            let keep_prefix = depth > 0 && !(in_block && (nodes.is_empty() || t.starts_with('}')));
            if let Some(space) = super::markup_space_prefix(rest, keep_prefix) {
                nodes.push(space);
            }
            rest = t;
        }

        if t.is_empty() || t.starts_with("</") {
            break;
        }
        if in_block && (t.starts_with('}') || block_terminator_at(t).is_some()) {
            break;
        }

        if let Some(after) = t.strip_prefix("<!--") {
            rest = match after.find("-->") {
                Some(end) => &after[end + 3..],
                None => return Err(angular_err(root, t, "unclosed markup comment")),
            };
            continue;
        }

        if block_starter_at(t).is_some() || block_terminator_at(t).is_some() {
            let (produced, next) = blocks::parse_angular_block(root, t, depth)?;
            nodes.extend(produced);
            rest = next;
            continue;
        }

        if t.starts_with('<') {
            let (produced, next) = tags::parse_angular_tag(root, t, depth)?;
            nodes.extend(produced);
            rest = next;
            continue;
        }

        let text_src = if depth > 0 { rest } else { t };
        let (node_opt, next) = angular_text_node(text_src, in_block, depth > 0)?;
        if let Some(node) = node_opt {
            nodes.push(node);
        }
        if next.len() == text_src.len() {
            let skip = text_src
                .char_indices()
                .nth(1)
                .map(|(i, _)| i)
                .unwrap_or(text_src.len());
            rest = &text_src[skip..];
        } else {
            rest = next;
        }
    }

    Ok((nodes, rest))
}

/// Consume a text run, stopping at the next tag, at a control-flow block and —
/// inside a block body — at the closing brace. `{{ … }}` groups are consumed
/// whole so their contents never look like a block terminator.
fn angular_text_node(
    src: &str,
    in_block: bool,
    nested: bool,
) -> Result<(Option<Node>, &str), RawParseError> {
    let bytes = src.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            break;
        }
        if src[i..].starts_with("{{") {
            let end = super::match_delimiter(&src[i..], '{', '}').ok_or_else(|| RawParseError {
                message: "unclosed moustache interpolation".to_owned(),
                byte_offset: None,
            })?;
            i += end + 1;
            continue;
        }
        if in_block && bytes[i] == b'}' {
            break;
        }
        if bytes[i] == b'@'
            && (block_starter_at(&src[i..]).is_some() || block_terminator_at(&src[i..]).is_some())
        {
            break;
        }
        i += src[i..].chars().next().unwrap().len_utf8();
    }

    let trimmed = super::markup_text_region(&src[..i], nested);
    if trimmed.is_empty() {
        return Ok((None, &src[i..]));
    }
    let mut parts = super::vue::template::parse_vue_text(trimmed)?;
    // Interpolation is recognized in source, never in decoded literal braces.
    for part in &mut parts {
        if let crate::ast::TextPart::Literal(value) = part {
            *value = super::svelte_entities::decode_html_entities(value, false);
        }
    }
    if parts.is_empty() {
        Ok((None, &src[i..]))
    } else {
        Ok((Some(Node::Text(parts)), &src[i..]))
    }
}

#[cfg(test)]
mod markup_whitespace_tests {
    use super::*;
    use crate::ast::TextPart;

    fn literal(node: &Node) -> String {
        let Node::Text(parts) = node else {
            panic!("text: {node:?}");
        };
        parts
            .iter()
            .map(|part| match part {
                TextPart::Literal(value) => value.as_str(),
                _ => panic!("literal"),
            })
            .collect()
    }

    #[test]
    fn inline_regions_and_structural_prefixes_keep_horizontal_spaces_once() {
        let root_text = parse_angular_template(" Hello ").unwrap();
        assert_eq!(literal(&root_text[0]), "Hello");
        let nodes = parse_angular_template(" \t<p>Hello <b>World</b> !</p> \t").unwrap();
        assert_eq!(nodes.len(), 1);
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        assert_eq!(literal(&element.children[0]), "Hello ");
        assert_eq!(literal(&element.children[2]), " !");
        let nodes = parse_angular_template("<div> <span /> <!--comment--> </div>").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        assert_eq!(element.children.len(), 4);
        for index in [0, 2, 3] {
            assert_eq!(literal(&element.children[index]), " ");
        }
    }

    #[test]
    fn block_prefixes_survive_without_changing_else_or_indentation() {
        let nodes = parse_angular_template(
            "<div> @if (visible) {<span>x</span>} @else {<span>y</span>} </div>",
        )
        .unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        assert_eq!(literal(&element.children[0]), " ");
        assert!(matches!(&element.children[1], Node::If(block) if block.else_children.is_some()));
        assert_eq!(literal(&element.children[2]), " ");
        let nodes = parse_angular_template("<div>\n  <span>x</span>\n</div>").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        assert!(matches!(&element.children[..], [Node::Element(_)]));
    }
    #[test]
    fn curly_block_edges_ignore_padding_but_keep_inline_sibling_separators() {
        let nodes =
            parse_angular_template("<div>@if (visible) { <b>a</b> <i>b</i> }</div>").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        let Node::If(block) = &element.children[0] else {
            panic!("if");
        };
        assert!(matches!(
            &block.then_children[..],
            [Node::Element(_), Node::Text(_), Node::Element(_)]
        ));
        assert_eq!(literal(&block.then_children[1]), " ");
    }
}
