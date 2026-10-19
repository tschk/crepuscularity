//! Astro template frontend.
//!
//! Compiles `.astro` markup into the shared `Node`/`Element` AST, so Astro
//! templates flow through the same lowering as every other frontend and reach
//! every renderer unchanged. This is a native frontend — it does not depend on
//! `@astrojs/compiler`.
//!
//! Scope: this frontend compiles the TEMPLATE only. The `---` frontmatter fence
//! is JavaScript/TypeScript that Astro runs on the server; this frontend never
//! executes it, so imports, `Astro.props`, top-level `await` and component
//! resolution are all outside its scope. The fence is blanked out (newlines
//! preserved) before parsing so byte offsets in error messages still point at
//! the original source. Expressions in markup are evaluated by the shared
//! crepuscularity evaluator against the template context.
//!
//! The body is JSX-like, so tag and attribute scanning reuses the JSX
//! frontend's machinery rather than a second expression parser.

mod exprs;
mod scan;
mod tags;
#[cfg(test)]
mod tests;

use super::RawParseError;
use crate::ast::Node;

/// Maximum element nesting depth, mirroring the JSX frontend's guard.
pub(crate) const MAX_ASTRO_DEPTH: usize = 256;

pub(crate) fn parse_astro_template(src: &str) -> Result<Vec<Node>, RawParseError> {
    parse_template_mode(src, ParseMode::default())
}

/// Parse Astro markup for authored modules, retaining component tag names for
/// the module compiler to resolve. Frontmatter is preserved by `split_astro_source`.
pub fn parse_astro_module_template(src: &str) -> Result<Vec<Node>, crate::error::CrepusError> {
    parse_template_mode(
        src,
        ParseMode {
            components: true,
            ..ParseMode::default()
        },
    )
    .map_err(|error| crate::error::CrepusError::parse(error.message))
}

/// Parse authored Astro using the caller-provided complete-JavaScript-expression validator.
/// Markup syntax and source slices remain owned by this frontend.
pub fn parse_astro_module_template_with_expression_validator(
    src: &str,
    validator: fn(&str) -> bool,
) -> Result<Vec<Node>, crate::error::CrepusError> {
    parse_template_mode(
        src,
        ParseMode {
            components: true,
            validator: Some(validator),
            ..ParseMode::default()
        },
    )
    .map_err(|error| crate::error::CrepusError::parse(error.message))
}

#[derive(Clone, Copy, Default)]
pub(super) struct ParseMode {
    components: bool,
    preserve_whitespace: bool,
    validator: Option<fn(&str) -> bool>,
}

fn parse_template_mode(src: &str, mode: ParseMode) -> Result<Vec<Node>, RawParseError> {
    let body = blank_frontmatter(src)?;
    let root = body.as_str();
    let (nodes, rest) = parse_astro_nodes(root, root, 0, mode)?;
    let rest = rest.trim_start();
    if !rest.is_empty() {
        return Err(astro_err(
            root,
            rest,
            format!(
                "unexpected trailing markup in Astro template: {}",
                super::diagnostic_prefix(rest, 40)
            ),
        ));
    }
    Ok(nodes)
}

/// Frontmatter located by [`split_astro_source`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstroFrontmatter<'a> {
    /// Text between the opening and closing fence lines, excluding both fence
    /// lines. Equal to `&source[span.clone()]`.
    pub content: &'a str,
    /// Byte range of `content` in the original source.
    pub span: std::ops::Range<usize>,
}

/// Result of splitting an Astro source into frontmatter and template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AstroSource<'a> {
    /// The `---` fenced frontmatter, if the source opens with one.
    pub frontmatter: Option<AstroFrontmatter<'a>>,
    /// Markup after the closing fence line (starting at that line's end, so a
    /// following newline is retained), or the whole source without a fence.
    pub template: &'a str,
    /// Byte offset of `template` in the original source.
    pub template_start: usize,
}

/// Split an Astro source at its `---` frontmatter fence. This only locates the
/// fence using the shared template frontend's line-based matching. It neither
/// executes the script nor validates the markup.
pub fn split_astro_source(src: &str) -> Result<AstroSource<'_>, crate::error::CrepusError> {
    match split_fence(src).map_err(|e| crate::error::CrepusError::parse(e.message))? {
        None => Ok(AstroSource {
            frontmatter: None,
            template: src,
            template_start: 0,
        }),
        Some(fence) => Ok(AstroSource {
            frontmatter: Some(AstroFrontmatter {
                content: &src[fence.content.clone()],
                span: fence.content,
            }),
            template: &src[fence.end..],
            template_start: fence.end,
        }),
    }
}

struct Fence {
    lead: usize,
    content: std::ops::Range<usize>,
    end: usize,
}

fn split_fence(src: &str) -> Result<Option<Fence>, RawParseError> {
    let lead = src.len() - src.trim_start().len();
    let after_lead = &src[lead..];
    let first_line_end = after_lead.find('\n').unwrap_or(after_lead.len());
    if after_lead[..first_line_end].trim_end() != "---" {
        return Ok(None);
    }

    let content_start = lead + first_line_end + 1;
    let mut cursor = content_start;
    while cursor < src.len() {
        let line_end = src[cursor..]
            .find('\n')
            .map(|n| cursor + n)
            .unwrap_or(src.len());
        if src[cursor..line_end].trim() == "---" {
            return Ok(Some(Fence {
                lead,
                content: content_start..cursor,
                end: line_end,
            }));
        }
        cursor = line_end + 1;
    }
    Err(astro_err(
        src,
        &src[lead..],
        "unclosed Astro frontmatter fence",
    ))
}

/// Replace the leading `---` frontmatter fence with spaces, keeping newlines so
/// byte offsets into the blanked body still map onto the original source.
fn blank_frontmatter(src: &str) -> Result<String, RawParseError> {
    let mut out = src.to_string();
    if let Some(fence) = split_fence(src)? {
        blank_range(&mut out, fence.lead, fence.end);
    }
    Ok(out)
}

fn blank_range(buf: &mut String, start: usize, end: usize) {
    let mut replacement: String = buf[start..end]
        .chars()
        .map(|c| if c == '\n' { '\n' } else { ' ' })
        .collect();
    // One char per source char; ASCII output can be shorter in bytes than
    // multi-byte input, so pad to preserve offsets.
    while replacement.len() < end - start {
        replacement.push(' ');
    }
    buf.replace_range(start..end, &replacement);
}

pub(crate) fn astro_err(root: &str, at: &str, message: impl Into<String>) -> RawParseError {
    RawParseError {
        message: message.into(),
        byte_offset: Some(super::subslice_byte_offset(root, at)),
    }
}

pub(crate) fn parse_astro_nodes<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
    mode: ParseMode,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
        parse_astro_nodes_inner(root, src, depth, mode)
    })
}

fn parse_astro_nodes_inner<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
    mode: ParseMode,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    if depth > MAX_ASTRO_DEPTH {
        return Err(astro_err(
            root,
            src,
            format!("maximum Astro nesting depth ({MAX_ASTRO_DEPTH}) exceeded"),
        ));
    }

    let mut nodes: Vec<Node> = Vec::new();
    let mut rest = src;

    loop {
        let t = rest.trim_start();
        if t.is_empty() || t.starts_with('<') || t.starts_with('{') {
            if mode.preserve_whitespace {
                let prefix = &rest[..rest.len() - t.len()];
                if !prefix.is_empty() {
                    nodes.push(Node::Text(vec![crate::ast::TextPart::Literal(
                        prefix.into(),
                    )]));
                }
            } else if let Some(space) = super::markup_space_prefix(
                rest,
                depth > 0 || (mode.components && !nodes.is_empty() && !t.is_empty()),
            ) {
                nodes.push(space);
            }
            rest = t;
        }

        if t.is_empty() || t.starts_with("</") {
            break;
        }

        if let Some(after) = t.strip_prefix("<!--") {
            rest = match after.find("-->") {
                Some(end) => &after[end + 3..],
                None => return Err(astro_err(root, t, "unclosed markup comment")),
            };
            continue;
        }

        if t.starts_with('{') {
            let (inner, next) = exprs::read_brace_slice(root, t, mode)?;
            rest = next;
            if inner.starts_with("/*") {
                continue;
            }
            if inner.starts_with("//") {
                if inner
                    .split_once('\n')
                    .is_some_and(|(_, rest)| !rest.trim().is_empty())
                {
                    return Err(astro_err(
                        root,
                        t,
                        "unexpected expression after Astro line comment",
                    ));
                }
                continue;
            }
            nodes.extend(exprs::lower_astro_expr(root, inner, depth, mode)?);
            continue;
        }

        if t.starts_with('<') {
            let (produced, next) = tags::parse_astro_tag(root, t, depth, mode)?;
            nodes.extend(produced);
            rest = next;
            continue;
        }

        let text_src = if depth > 0 { rest } else { t };
        let (node_opt, next) = astro_text_node(text_src, depth > 0, mode)?;
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

/// Consume a text run, stopping at the next tag or at a `{…}` group that
/// contains markup. Interpolations without markup stay inside the run and are
/// lowered to `TextPart::Expr` by the shared text-template parser.
fn astro_text_node(
    src: &str,
    nested: bool,
    mode: ParseMode,
) -> Result<(Option<Node>, &str), RawParseError> {
    let bytes = src.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'<' => break,
            b'{' => {
                let expression = src[i + 1..].trim_start();
                if expression.starts_with("/*") || expression.starts_with("//") {
                    break;
                }
                let Some(end) = exprs::match_brace(&src[i..], mode) else {
                    i = bytes.len();
                    break;
                };
                let group = &src[i..i + end + 1];
                if exprs::is_structural_expression(group, mode) {
                    break;
                }
                i += end + 1;
            }
            _ => i += 1,
        }
    }

    let trimmed = if mode.preserve_whitespace {
        &src[..i]
    } else {
        super::markup_text_region(&src[..i], nested)
    };
    if trimmed.is_empty() {
        return Ok((None, &src[i..]));
    }
    let parts = if mode.components {
        let mut parts = Vec::new();
        let mut cursor = 0;
        while let Some(position) = trimmed[cursor..].find('{') {
            let start = cursor + position;
            if start > cursor {
                parts.push(crate::ast::TextPart::Literal(
                    trimmed[cursor..start].to_owned(),
                ));
            }
            let end = exprs::match_brace(&trimmed[start..], mode)
                .ok_or_else(|| astro_err(src, &src[start..], "unclosed Astro interpolation"))?;
            parts.push(crate::ast::TextPart::Expr(
                trimmed[start + 1..start + end].to_owned(),
            ));
            cursor = start + end + 1;
        }
        if cursor < trimmed.len() {
            parts.push(crate::ast::TextPart::Literal(trimmed[cursor..].to_owned()));
        }
        parts
    } else {
        super::indent::parse_text_template(trimmed)?
    };
    Ok((Some(Node::Text(parts)), &src[i..]))
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
        let root_text = parse_astro_template(" Hello ").unwrap();
        assert_eq!(literal(&root_text[0]), "Hello");
        let nodes = parse_astro_template(" \t<p>Hello <b>World</b> !</p> \t").unwrap();
        assert_eq!(nodes.len(), 1);
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        assert_eq!(literal(&element.children[0]), "Hello ");
        assert_eq!(literal(&element.children[2]), " !");
        let nodes = parse_astro_template("<div> <span /> <!--comment--> </div>").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        assert_eq!(element.children.len(), 4);
        for index in [0, 2, 3] {
            assert_eq!(literal(&element.children[index]), " ");
        }
    }

    #[test]
    fn expression_prefixes_survive_but_multiline_indentation_does_not() {
        let nodes = parse_astro_template("<div> {value} </div>").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        assert_eq!(literal(&element.children[0]), " ");
        assert!(
            matches!(&element.children[1], Node::Text(parts) if matches!(&parts[..], [TextPart::Expr(value)] if value == "value"))
        );
        assert_eq!(literal(&element.children[2]), " ");
        let nodes = parse_astro_template("<div>\n  <span>x</span>\n</div>").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element");
        };
        assert!(matches!(&element.children[..], [Node::Element(_)]));
    }
}
