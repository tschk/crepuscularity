//! Svelte template frontend.
//!
//! Compiles `.svelte` markup directly into the shared `Node`/`Element` AST, so
//! Svelte templates flow through the same lowering as every other frontend and
//! reach every renderer (React, Solid, SwiftUI, Compose, TUI, LVGL) unchanged.
//! This is a native frontend — it does not depend on `svelte/compiler`.
//!
//! Scope: this frontend compiles the TEMPLATE only. `<script>` contents are
//! extracted verbatim and never executed: runes (`$state`, `$derived`,
//! `$effect`, `$props`), reactive statements, stores, imports and component
//! resolution are all outside this frontend. Expressions in markup are
//! evaluated by the shared crepuscularity evaluator against the template
//! context, exactly as `{name}` is in the indentation frontend.

mod authored;
pub mod entities;
pub use authored::{
    parse_svelte_module, parse_svelte_module_with_boundary,
    parse_svelte_module_with_styles_and_boundary, SvelteModule, SvelteSnippet,
};
mod blocks;
mod tags;
#[cfg(test)]
mod tests;

use super::RawParseError;
use crate::ast::Node;

/// Maximum element nesting depth, mirroring the JSX frontend's guard.
pub(crate) const MAX_SVELTE_DEPTH: usize = 256;

/// A parsed `.svelte` single-file component.
#[derive(Debug, Clone, Default)]
pub struct SvelteComponent {
    /// The compiled markup, as shared-AST nodes.
    pub nodes: Vec<Node>,
    /// Raw `<style>` body, preserved verbatim so a caller can emit it.
    pub style: Option<String>,
    /// Raw `<script>` body, preserved verbatim. It is NOT executed.
    pub script: Option<String>,
    /// Raw `<script context="module">` / `<script module>` body. NOT executed.
    pub module_script: Option<String>,
}

/// Parse a `.svelte` single-file component, keeping `<style>` and `<script>`.
pub fn parse_svelte_component(src: &str) -> Result<SvelteComponent, crate::error::CrepusError> {
    parse_svelte_component_raw(src).map_err(|e| crate::error::CrepusError::parse(e.message))
}

/// Parse `.svelte` markup into shared-AST nodes, discarding script/style.
pub(crate) fn parse_svelte_template(src: &str) -> Result<Vec<Node>, RawParseError> {
    parse_svelte_component_raw(src).map(|c| c.nodes)
}

pub(crate) fn parse_svelte_component_raw(src: &str) -> Result<SvelteComponent, RawParseError> {
    let extracted = extract_top_level_blocks(src)?;
    let body = extracted.blanked;
    let (nodes, rest) = parse_svelte_nodes(&body, &body, 0)?;
    if !rest.trim().is_empty() {
        return Err(svelte_err(
            &body,
            rest,
            "unexpected trailing markup in Svelte template",
        ));
    }
    Ok(SvelteComponent {
        nodes,
        style: extracted.style,
        script: extracted.script,
        module_script: extracted.module_script,
    })
}

struct Extracted {
    style_attributes: Vec<super::jsx::attrs::JsxAttr>,
    blanked: String,
    style: Option<String>,
    script: Option<String>,
    module_script: Option<String>,
    script_is_typescript: bool,
    module_script_is_typescript: bool,
}

/// Replace `<script>` / `<style>` regions with spaces (newlines preserved) so
/// byte offsets into the blanked body still map onto the original source.
fn extract_top_level_blocks(src: &str) -> Result<Extracted, RawParseError> {
    extract_top_level_blocks_mode(src, false)
}

fn extract_top_level_blocks_mode(src: &str, authored: bool) -> Result<Extracted, RawParseError> {
    extract_top_level_blocks_with_boundary(src, authored, None)
}

fn parse_block_attrs<'a>(
    src: &'a str,
    tail: &'a str,
    boundary: Option<fn(&str) -> Option<usize>>,
) -> Result<(Vec<super::jsx::attrs::JsxAttr>, &'a str, bool), RawParseError> {
    match boundary {
        Some(read) => super::jsx::attrs::jsx_parse_attrs_with_boundary(src, tail, read),
        None => super::jsx::jsx_parse_attrs(src, tail),
    }
}

fn extract_top_level_blocks_with_boundary(
    src: &str,
    authored: bool,
    boundary: Option<fn(&str) -> Option<usize>>,
) -> Result<Extracted, RawParseError> {
    let mut blanked = src.to_string();
    let mut style_attributes = Vec::new();
    let mut style = None;
    let mut script = None;
    let mut module_script = None;
    let mut script_is_typescript = false;
    let mut module_script_is_typescript = false;

    let lower = src.to_ascii_lowercase();
    let mut i = 0usize;
    let mut nesting = 0usize;
    let mut blocks = 0usize;
    while i < lower.len() {
        let Some(rel) = lower[i..].find(['<', '{']) else {
            break;
        };
        let at = i + rel;
        if lower.as_bytes()[at] == b'{' {
            let end = boundary
                .map_or_else(
                    || super::match_delimiter(&src[at..], '{', '}'),
                    |read| read(&src[at..]),
                )
                .ok_or_else(|| svelte_err(src, &src[at..], "unclosed Svelte expression"))?;
            let expression = src[at + 1..at + end].trim();
            if expression.starts_with('#') {
                blocks += 1;
            }
            if expression.starts_with('/') {
                blocks = blocks.saturating_sub(1);
            }
            i = at + end + 1;
            continue;
        }
        if lower[at..].starts_with("<!--") {
            i = lower[at + 4..]
                .find("-->")
                .map_or(lower.len(), |end| at + 4 + end + 3);
            continue;
        }
        let name = if super::jsx::text::jsx_open_tag_tail(&lower[at..], "script").is_some() {
            "script"
        } else if super::jsx::text::jsx_open_tag_tail(&lower[at..], "style").is_some() {
            "style"
        } else {
            let tail = &src[at + 1..];
            if tail.starts_with('/') {
                let end = tail
                    .find('>')
                    .ok_or_else(|| svelte_err(src, &src[at..], "unclosed closing tag"))?;
                nesting = nesting.saturating_sub(1);
                i = at + 1 + end + 1;
            } else {
                let name_end = tail
                    .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
                    .unwrap_or(tail.len());
                let tag = &tail[..name_end];
                let (remaining, self_closing) = if let Some(read) = boundary {
                    let (_, remaining, self_closing) =
                        super::jsx::attrs::jsx_parse_authored_attrs(src, &tail[name_end..], read)?;
                    (remaining, self_closing)
                } else {
                    let (_, remaining, self_closing) =
                        parse_block_attrs(src, &tail[name_end..], boundary)?;
                    (remaining, self_closing)
                };
                if authored && tag == "textarea" && !self_closing {
                    let mut body = src.len() - remaining.len();
                    loop {
                        let offset = src[body..]
                            .find(['<', '{'])
                            .ok_or_else(|| svelte_err(src, &src[at..], "unclosed textarea"))?;
                        body += offset;
                        if let Some(end) = authored::textarea_close(&src[body..]) {
                            i = body + end;
                            break;
                        }
                        if src.as_bytes()[body] == b'{' {
                            let end = boundary
                                .map_or_else(
                                    || super::match_delimiter(&src[body..], '{', '}'),
                                    |read| read(&src[body..]),
                                )
                                .ok_or_else(|| {
                                    svelte_err(src, &src[body..], "unclosed Svelte expression")
                                })?;
                            body += end + 1;
                        } else {
                            body += 1;
                        }
                    }
                    continue;
                }
                if !self_closing && !tags::VOID_TAGS.contains(&tag) {
                    nesting += 1;
                }
                i = src.len() - remaining.len();
            }
            continue;
        };
        if nesting != 0 || blocks != 0 {
            return Err(svelte_err(
                src,
                &src[at..],
                format!("nested {name} blocks are not supported"),
            ));
        }

        let (attributes, tail, _) = parse_block_attrs(src, &src[at + name.len() + 1..], boundary)?;
        if authored && name == "script" {
            authored::validate_script_attributes(src, at, &attributes)?;
        }
        let open_end = src.len() - tail.len();
        let is_module = attributes.iter().any(|attribute| {
            attribute.key == "module"
                || (attribute.key == "context" && attribute.as_str() == Some("module"))
        });
        let close = format!("</{name}");
        let rel = lower[open_end..]
            .match_indices(&close)
            .find(|(offset, _)| {
                let tail = &lower[open_end + offset + close.len()..];
                tail.starts_with(|ch: char| ch.is_whitespace() || ch == '>')
            })
            .map(|(offset, _)| offset)
            .ok_or_else(|| svelte_err(src, &src[at..], format!("unclosed {name} block")))?;
        let content_end = open_end + rel;
        let close_end = lower[content_end..]
            .find('>')
            .map(|offset| content_end + offset)
            .ok_or_else(|| {
                svelte_err(
                    src,
                    &src[content_end..],
                    format!("unclosed {name} closing tag"),
                )
            })?;
        if !lower[content_end + close.len()..close_end]
            .trim()
            .is_empty()
        {
            return Err(svelte_err(
                src,
                &src[content_end..],
                format!("invalid {name} closing tag"),
            ));
        }
        let block_end = close_end + 1;

        if name == "script" {
            let is_typescript = attributes
                .iter()
                .any(|attribute| attribute.key == "lang" && attribute.as_str() == Some("ts"));
            if is_module {
                module_script_is_typescript = is_typescript;
            } else {
                script_is_typescript = is_typescript;
            }
        }
        let content = src[open_end..content_end].to_string();
        if name == "style" {
            style_attributes = attributes;
        }
        let slot = if name == "style" {
            &mut style
        } else if is_module {
            &mut module_script
        } else {
            &mut script
        };
        if slot.is_some() {
            return Err(svelte_err(
                src,
                &src[at..],
                format!(
                    "duplicate {}{name} block",
                    if is_module && name == "script" {
                        "module "
                    } else {
                        ""
                    }
                ),
            ));
        }
        *slot = Some(content);

        blank_range(&mut blanked, at, block_end);
        i = block_end;
    }

    Ok(Extracted {
        style_attributes,
        blanked,
        style,
        script,
        module_script,
        script_is_typescript,
        module_script_is_typescript,
    })
}

fn blank_range(buf: &mut String, start: usize, end: usize) {
    let replacement: String = buf[start..end]
        .chars()
        .map(|c| if c == '\n' { '\n' } else { ' ' })
        .collect();
    // `replacement` keeps one char per source char; ASCII-only output can be
    // shorter in bytes than multi-byte input, so pad to preserve offsets.
    let mut replacement = replacement;
    while replacement.len() < end - start {
        replacement.push(' ');
    }
    buf.replace_range(start..end, &replacement);
}

pub(crate) fn svelte_err(root: &str, at: &str, message: impl Into<String>) -> RawParseError {
    RawParseError {
        message: message.into(),
        byte_offset: Some(super::subslice_byte_offset(root, at)),
    }
}

pub(crate) fn parse_svelte_nodes<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    stacker::maybe_grow(128 * 1024, 2 * 1024 * 1024, || {
        parse_svelte_nodes_inner(root, src, depth)
    })
}

fn parse_svelte_nodes_inner<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    if depth > MAX_SVELTE_DEPTH {
        return Err(svelte_err(
            root,
            src,
            format!("maximum Svelte nesting depth ({MAX_SVELTE_DEPTH}) exceeded"),
        ));
    }

    let mut nodes = Vec::new();
    let mut rest = src;

    loop {
        let t = rest.trim_start();
        if t.is_empty()
            || t.starts_with('<')
            || t.starts_with("{:")
            || t.starts_with("{/")
            || t.starts_with("{#")
            || t.starts_with("{@")
        {
            if let Some(space) = super::markup_space_prefix(rest, depth > 0) {
                nodes.push(space);
            }
            rest = t;
        }

        if t.is_empty() || t.starts_with("</") || t.starts_with("{:") || t.starts_with("{/") {
            break;
        }

        if let Some(after) = t.strip_prefix("<!--") {
            rest = match after.find("-->") {
                Some(end) => &after[end + 3..],
                None => return Err(svelte_err(root, t, "unclosed markup comment")),
            };
            continue;
        }

        if t.starts_with("{#") {
            let (node, next) = blocks::parse_svelte_block(root, t, depth)?;
            nodes.push(node);
            rest = next;
            continue;
        }

        if t.starts_with("{@") {
            let (node, next) = parse_at_tag(root, t)?;
            nodes.push(node);
            rest = next;
            continue;
        }

        if t.starts_with('<') {
            let (node, next) = tags::parse_svelte_tag(root, t, depth)?;
            nodes.push(node);
            rest = next;
            continue;
        }

        let (node_opt, next) = svelte_text_node(rest, depth > 0)?;
        if let Some(node) = node_opt {
            nodes.push(node);
        }
        if next.len() == rest.len() {
            let skip = rest
                .char_indices()
                .nth(1)
                .map(|(i, _)| i)
                .unwrap_or(rest.len());
            rest = &rest[skip..];
        } else {
            rest = next;
        }
    }

    Ok((nodes, rest))
}

fn parse_at_tag<'a>(root: &'a str, src: &'a str) -> Result<(Node, &'a str), RawParseError> {
    let (inner, rest) = blocks::read_brace_tag(root, src)?;
    if let Some(expr) = inner.strip_prefix("@html ") {
        return Ok((Node::RawHtml(expr.trim().to_string()), rest));
    }
    let name = inner
        .split_whitespace()
        .next()
        .unwrap_or("@")
        .trim_start_matches('@')
        .to_string();
    Err(svelte_err(
        root,
        src,
        format!("`{{@{name}}}` is not supported by the Svelte frontend (only `{{@html expr}}` is)"),
    ))
}

/// Consume a text run, stopping at the next tag or block/`{@…}` tag.
/// `{expr}` interpolations are consumed into the run and become `TextPart::Expr`.
fn svelte_text_node(src: &str, nested: bool) -> Result<(Option<Node>, &str), RawParseError> {
    let bytes = src.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'<' => break,
            b'{' => {
                let next = bytes.get(i + 1).copied();
                if matches!(next, Some(b'#') | Some(b':') | Some(b'/') | Some(b'@')) {
                    break;
                }
                let end =
                    super::match_delimiter(&src[i..], '{', '}').ok_or_else(|| RawParseError {
                        message: "unclosed text interpolation".to_owned(),
                        byte_offset: None,
                    })?;
                i += end + 1;
            }
            _ => i += 1,
        }
    }

    let text = &src[..i];
    let region = super::markup_text_region(text, nested);
    if region.is_empty() {
        return Ok((None, &src[i..]));
    }
    let parts = super::indent::parse_text_template(region)?;
    Ok((Some(Node::Text(parts)), &src[i..]))
}

#[cfg(test)]
mod whitespace_tests {
    use super::*;
    use crate::ast::TextPart;

    #[test]
    fn root_text_does_not_render_extracted_block_padding() {
        for source in [
            "   Hello   ",
            "<script>let x=1</script>Hello<style>p{}</style>",
        ] {
            let nodes = parse_svelte_template(source).unwrap();
            assert!(
                matches!(&nodes[..], [Node::Text(parts)] if parts == &vec![TextPart::Literal("Hello".into())]),
                "{nodes:?}"
            );
        }
    }

    #[test]
    fn inline_spaces_survive_but_formatting_indentation_does_not() {
        let nodes = parse_svelte_template("<p> a <span>b</span> c </p>").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element")
        };
        assert!(
            matches!(&element.children[0], Node::Text(parts) if parts == &vec![TextPart::Literal(" a ".into())])
        );
        assert!(
            matches!(&element.children[2], Node::Text(parts) if parts == &vec![TextPart::Literal(" c ".into())])
        );
        let nodes = parse_svelte_template("<p>\n  <span>b</span>\n</p>").unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("element")
        };
        assert_eq!(element.children.len(), 1);
    }
}

#[cfg(test)]
mod parse_svelte_component_raw_tests {
    use crate::ast::Node;
    use crate::parser::svelte::parse_svelte_component_raw;

    #[test]
    fn test_happy_path() {
        let src = r#"
<script>
    let x = 1;
</script>
<script context="module">
    export const y = 2;
</script>
<div>hello</div>
<style>
    div { color: red; }
</style>
"#;
        let component = parse_svelte_component_raw(src).unwrap();
        assert_eq!(component.script.as_deref().unwrap().trim(), "let x = 1;");
        assert_eq!(
            component.module_script.as_deref().unwrap().trim(),
            "export const y = 2;"
        );
        assert_eq!(
            component.style.as_deref().unwrap().trim(),
            "div { color: red; }"
        );
        assert_eq!(component.nodes.len(), 1);
        if let Node::Element(el) = &component.nodes[0] {
            assert_eq!(el.tag, "div");
        } else {
            panic!("Expected Element");
        }
    }

    #[test]
    fn test_trailing_markup_error() {
        let src = "<div></div></foo>";
        let err = parse_svelte_component_raw(src).unwrap_err();
        assert_eq!(err.message, "unexpected trailing markup in Svelte template");
    }
}
