//! Vue template block parsing into the shared AST.

use crate::ast::*;

use super::super::jsx::{jsx_close, jsx_err, jsx_parse_attrs};
use super::super::RawParseError;
use super::builders::{vue_build_element, vue_build_element_with_mode, VueBranch, VueElement};

const VOID_ELEMENTS: [&str; 14] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

const MAX_VUE_DEPTH: usize = 256;

pub(crate) fn parse_vue_nodes<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    parse_vue_nodes_with_mode(root, src, depth, false)
}

pub(crate) fn parse_vue_nodes_with_mode<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
    native_props: bool,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
        parse_vue_nodes_inner(root, src, depth, native_props)
    })
}

fn parse_vue_nodes_inner<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
    native_props: bool,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    if depth > MAX_VUE_DEPTH {
        return Err(jsx_err(
            root,
            src,
            format!("maximum Vue template nesting depth ({MAX_VUE_DEPTH}) exceeded"),
        ));
    }

    let mut nodes: Vec<Node> = Vec::new();
    let mut rest = src;
    let mut open_branch_chain = false;

    loop {
        let t = rest.trim_start();
        if t.is_empty() {
            if let Some(space) = super::super::markup_space_prefix(rest, depth > 0) {
                nodes.push(space);
            }
            rest = t;
            break;
        }
        if t.starts_with("</") {
            if let Some(space) = super::super::markup_space_prefix(rest, depth > 0) {
                nodes.push(space);
            }
            rest = t;
            break;
        }
        if let Some(after) = t.strip_prefix("<!--") {
            if let Some(space) = super::super::markup_space_prefix(rest, depth > 0) {
                nodes.push(space);
            }
            rest = match after.find("-->") {
                Some(end) => &after[end + 3..],
                None => return Err(jsx_err(root, t, "unclosed markup comment")),
            };
            continue;
        }
        if t.starts_with('<') {
            let (node, next) = parse_vue_tag(root, t, depth, native_props)?;
            if matches!(node.branch, Some(VueBranch::Else | VueBranch::ElseIf(_))) {
                if !open_branch_chain {
                    let directive = if matches!(node.branch, Some(VueBranch::Else)) {
                        "v-else"
                    } else {
                        "v-else-if"
                    };
                    return Err(jsx_err(
                        root,
                        t,
                        format!("{directive} without a preceding v-if"),
                    ));
                }
                // Vue ignores whitespace and comments between conditional branches.
                while nodes.last().is_some_and(|node| matches!(node, Node::Text(parts) if parts.iter().all(|part| matches!(part, TextPart::Literal(value) if value.trim().is_empty())))) {
                    nodes.pop();
                }
            } else if let Some(space) = super::super::markup_space_prefix(rest, depth > 0) {
                nodes.push(space);
            }
            // AST shape cannot distinguish an else-if from a terminal else
            // containing its own single nested if. Keep sibling provenance here.
            open_branch_chain =
                matches!(node.branch, Some(VueBranch::If(_) | VueBranch::ElseIf(_)));
            push_node(&mut nodes, node, root, t)?;
            rest = next;
            continue;
        }

        let (text, next) = vue_text_node(rest, depth > 0, native_props)?;
        if let Some(text) = text {
            if !matches!(&text, Node::Text(parts) if parts.iter().all(|part| matches!(part, TextPart::Literal(value) if value.trim().is_empty())))
            {
                open_branch_chain = false;
            }
            nodes.push(text);
        }
        if next.len() == rest.len() {
            let skip = next
                .char_indices()
                .nth(1)
                .map(|(i, _)| i)
                .unwrap_or(next.len());
            rest = &next[skip..];
        } else {
            rest = next;
        }
    }

    Ok((nodes, rest))
}

/// Parsed nodes together with the `v-if` chain slot they occupy.
struct VueItem {
    nodes: Vec<Node>,
    branch: Option<VueBranch>,
}

fn push_node(
    nodes: &mut Vec<Node>,
    item: VueItem,
    root: &str,
    at: &str,
) -> Result<(), RawParseError> {
    match item.branch {
        None => nodes.extend(item.nodes),
        Some(VueBranch::If(condition)) => nodes.push(Node::If(IfBlock {
            condition,
            then_children: item.nodes,
            else_children: None,
        })),
        Some(VueBranch::ElseIf(condition)) => {
            let slot = innermost_else_slot(nodes)
                .ok_or_else(|| jsx_err(root, at, "v-else-if without a preceding v-if"))?;
            *slot = Some(vec![Node::If(IfBlock {
                condition,
                then_children: item.nodes,
                else_children: None,
            })]);
        }
        Some(VueBranch::Else) => {
            let slot = innermost_else_slot(nodes)
                .ok_or_else(|| jsx_err(root, at, "v-else without a preceding v-if"))?;
            *slot = Some(item.nodes);
        }
    }
    Ok(())
}

/// Walk to the deepest unfilled `else_children` slot of the trailing `v-if` chain.
fn innermost_else_slot(nodes: &mut [Node]) -> Option<&mut Option<Vec<Node>>> {
    let Node::If(block) = nodes.last_mut()? else {
        return None;
    };
    let mut current = block;
    loop {
        let filled_with_if = matches!(current.else_children.as_deref(), Some([Node::If(_)]));
        if !filled_with_if {
            if current.else_children.is_some() {
                return None;
            }
            return Some(&mut current.else_children);
        }
        let Some([Node::If(next)]) = current.else_children.as_deref_mut() else {
            return None;
        };
        current = next;
    }
}

fn parse_vue_tag<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
    native_props: bool,
) -> Result<(VueItem, &'a str), RawParseError> {
    let after_lt = &src[1..];
    let name_end = after_lt
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(after_lt.len());
    let tag = &after_lt[..name_end];
    let attrs_src = after_lt[name_end..].trim_start();

    let (attrs, after_gt, self_closing) = jsx_parse_attrs(root, attrs_src)?;
    let void = VOID_ELEMENTS.contains(&tag.to_ascii_lowercase().as_str());

    let (children, rest) = if self_closing || void {
        (Vec::new(), after_gt)
    } else {
        let (children, rest) = parse_vue_nodes_with_mode(root, after_gt, depth + 1, native_props)?;
        let rest = jsx_close(root, rest, tag)?;
        (children, rest)
    };

    let VueElement {
        mut element,
        branch,
        for_spec,
        replacement_children,
    } = (if native_props {
        vue_build_element_with_mode(tag, attrs, children, true)
    } else {
        vue_build_element(tag, attrs, children)
    })
    .map_err(|mut e| {
        if e.byte_offset.is_none() {
            e.byte_offset = Some(super::super::subslice_byte_offset(root, src));
        }
        e
    })?;

    if let Some(children) = replacement_children {
        element.children = children;
    }

    // Only structural templates contribute their children; a plain template is an HTML element.
    let mut nodes = if tag == "template" && (branch.is_some() || for_spec.is_some()) {
        std::mem::take(&mut element.children)
    } else {
        vec![Node::Element(element)]
    };

    if let Some((pattern, iterator)) = for_spec {
        nodes = vec![Node::For(ForBlock {
            pattern,
            iterator,
            body: nodes,
        })];
    }

    Ok((VueItem { nodes, branch }, rest))
}

/// Consume text up to the next `<`, turning `{{ expr }}` into expression parts.
fn vue_text_node(
    src: &str,
    nested: bool,
    native_props: bool,
) -> Result<(Option<Node>, &str), RawParseError> {
    let mut end = 0;
    while end < src.len() {
        let ch = src[end..].chars().next().unwrap();
        if ch == '<' {
            break;
        }
        if src[end..].starts_with("{{") {
            let close = super::super::match_delimiter(&src[end..], '{', '}').ok_or_else(|| {
                RawParseError {
                    message: "unclosed moustache interpolation".to_owned(),
                    byte_offset: None,
                }
            })?;
            end += close + 1;
        } else {
            end += ch.len_utf8();
        }
    }
    let text = super::super::markup_text_region(&src[..end], nested);
    if text.is_empty() {
        return Ok((None, &src[end..]));
    }
    let mut parts = parse_vue_text(text)?;
    // Split authored interpolation before decoding: encoded braces remain data.
    if !native_props {
        for part in &mut parts {
            if let TextPart::Literal(value) = part {
                *value = super::super::svelte_entities::decode_html_entities(value, false);
            }
        }
    }
    if parts.is_empty() {
        Ok((None, &src[end..]))
    } else {
        Ok((Some(Node::Text(parts)), &src[end..]))
    }
}

pub(crate) fn parse_vue_text(text: &str) -> Result<Vec<TextPart>, RawParseError> {
    let mut parts = Vec::new();
    let mut literal = String::new();
    let mut rest = text;

    while let Some(open) = rest.find("{{") {
        let close = super::super::match_delimiter(&rest[open..], '{', '}').ok_or_else(|| {
            RawParseError {
                message: "unclosed moustache interpolation".to_owned(),
                byte_offset: None,
            }
        })?;
        literal.push_str(&rest[..open]);
        if !literal.is_empty() {
            parts.push(TextPart::Literal(std::mem::take(&mut literal)));
        }
        let expr = rest[open + 2..open + close - 1].trim();
        if expr.is_empty() {
            return Err(RawParseError {
                message: "moustache interpolation requires a nonempty expression".into(),
                byte_offset: None,
            });
        }
        parts.push(TextPart::Expr(expr.to_string()));
        rest = &rest[open + close + 1..];
    }

    literal.push_str(rest);
    if !literal.is_empty() {
        parts.push(TextPart::Literal(literal));
    }
    Ok(parts)
}

#[cfg(test)]
mod whitespace_tests {
    use super::*;

    #[test]
    fn raw_root_text_padding_is_trimmed() {
        let source = "   Hello   ";
        let (nodes, rest) = parse_vue_nodes(source, source, 0).unwrap();
        assert!(rest.is_empty());
        assert!(
            matches!(&nodes[..], [Node::Text(parts)] if parts == &vec![TextPart::Literal("Hello".into())])
        );
    }

    #[test]
    fn inline_spaces_survive_and_conditional_comment_separators_stay_adjacent() {
        let source = "<p> a <span>b</span> c </p>";
        let (nodes, rest) = parse_vue_nodes(source, source, 0).unwrap();
        assert!(rest.is_empty());
        let Node::Element(element) = &nodes[0] else {
            panic!("element")
        };
        assert!(
            matches!(&element.children[0], Node::Text(parts) if parts == &vec![TextPart::Literal(" a ".into())])
        );
        assert!(
            matches!(&element.children[2], Node::Text(parts) if parts == &vec![TextPart::Literal(" c ".into())])
        );
        let source = "<p><span v-if=\"ok\">yes</span> <!-- gap --> <span v-else>no</span></p>";
        let (nodes, _) = parse_vue_nodes(source, source, 0).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element")
        };
        assert!(
            matches!(&element.children[..], [Node::If(block)] if block.else_children.is_some())
        );
    }
}
