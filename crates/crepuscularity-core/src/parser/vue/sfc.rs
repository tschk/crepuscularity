//! Vue single-file-component block splitting.

use super::super::jsx::{jsx_err, JsxAttr, JsxAttrValue};
use super::super::RawParseError;
use std::collections::HashSet;

/// One `<script>` block of a Vue SFC. The contents are never executed or parsed.
#[derive(Debug, Clone, Default)]
pub struct VueScriptBlock {
    /// True when the block was declared as `<script setup>`.
    pub setup: bool,
    /// The `lang` attribute, if any (`ts`, `js`, …).
    pub lang: Option<String>,
    /// Raw source between the tags.
    pub content: String,
}

/// One `<style>` block of a Vue SFC, preserved verbatim for a caller to emit.
#[derive(Debug, Clone, Default)]
pub struct VueStyleBlock {
    pub scoped: bool,
    pub module: bool,
    pub lang: Option<String>,
    pub content: String,
}

/// A split Vue single-file component.
#[derive(Debug, Clone, Default)]
pub struct VueSfc {
    /// Raw contents of the `<template>` block, if present.
    pub template: Option<String>,
    pub scripts: Vec<VueScriptBlock>,
    pub styles: Vec<VueStyleBlock>,
}

/// Extra source metadata, kept separate from the existing public SFC block types.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct VueSfcMetadata {
    /// One entry per script block, in source order.
    pub script_generics: Vec<Option<String>>,
    /// One attribute-name list per style block, in source order.
    pub style_attributes: Vec<Vec<String>>,
}

pub(crate) fn split_sfc(src: &str) -> Result<VueSfc, RawParseError> {
    split_sfc_with_template_offset(src).map(|(sfc, _, _)| sfc)
}

pub(super) fn split_sfc_with_template_offset(
    src: &str,
) -> Result<(VueSfc, usize, VueSfcMetadata), RawParseError> {
    let mut sfc = VueSfc::default();
    let mut metadata = VueSfcMetadata::default();
    let mut template_offset = 0;
    let mut i = if src.starts_with('\u{feff}') {
        '\u{feff}'.len_utf8()
    } else {
        0
    };

    while i < src.len() {
        let rest = src[i..].trim_start();
        i = src.len() - rest.len();
        if rest.is_empty() {
            break;
        }
        if src[i..].starts_with("<!--") {
            i = match src[i + 4..].find("-->") {
                Some(end) => i + 4 + end + 3,
                None => {
                    return Err(RawParseError {
                        message: "unclosed markup comment".into(),
                        byte_offset: Some(i),
                    })
                }
            };
            continue;
        }
        let Some(name) = block_name(rest) else {
            return Err(RawParseError {
                message: "unsupported top-level content in Vue SFC; expected <template>, <script>, or <style>".into(),
                byte_offset: Some(i),
            });
        };

        let (attrs, after_open, self_closing) = parse_sfc_attrs(src, &rest[1 + name.len()..])?;
        let mut names = HashSet::new();
        for attribute in &attrs {
            if !names.insert(attribute.key.as_str()) {
                return Err(RawParseError {
                    message: format!("duplicate Vue SFC {name} attribute {:?}", attribute.key),
                    byte_offset: Some(i),
                });
            }
        }
        if attrs.iter().any(|attribute| attribute.key == "src") {
            return Err(RawParseError {
                message: format!("external Vue SFC {name} blocks are not supported"),
                byte_offset: Some(i),
            });
        }
        if name == "template"
            && attrs.iter().any(|attribute| {
                attribute.key == "lang"
                    && !matches!(attribute.as_str(), None | Some("") | Some("html"))
            })
        {
            return Err(RawParseError {
                message: "unsupported Vue template language; only HTML templates are supported"
                    .into(),
                byte_offset: Some(i),
            });
        }
        if name == "script"
            && attrs
                .iter()
                .any(|attribute| attribute.key == "generic" && attribute.as_str().is_none())
        {
            return Err(RawParseError {
                message: "Vue script generic attribute requires a string".into(),
                byte_offset: Some(i),
            });
        }
        let body_start = src.len() - after_open.len();
        let (body, next) = if self_closing {
            ("", body_start)
        } else {
            let body_end = find_block_end(src, body_start, name)?.ok_or_else(|| RawParseError {
                message: format!("missing </{name}> in Vue SFC"),
                byte_offset: Some(i),
            })?;
            let next = body_end
                + src[body_end..]
                    .find('>')
                    .expect("closing tag checked above")
                + 1;
            (&src[body_start..body_end], next)
        };

        match name {
            "template" => {
                if sfc.template.is_some() {
                    return Err(RawParseError {
                        message: "Vue SFC has more than one top-level <template> block".to_string(),
                        byte_offset: Some(i),
                    });
                }
                sfc.template = Some(body.to_string());
                template_offset = body_start;
            }
            "script" => {
                metadata.script_generics.push(attr_value(&attrs, "generic"));
                sfc.scripts.push(VueScriptBlock {
                    setup: has_flag(&attrs, "setup"),
                    lang: attr_value(&attrs, "lang"),
                    content: body.to_string(),
                });
            }
            _ => {
                metadata.style_attributes.push(
                    attrs
                        .iter()
                        .map(|attribute| attribute.key.clone())
                        .collect(),
                );
                sfc.styles.push(VueStyleBlock {
                    scoped: has_flag(&attrs, "scoped"),
                    module: has_flag(&attrs, "module"),
                    lang: attr_value(&attrs, "lang"),
                    content: body.to_string(),
                });
            }
        }

        i = next;
    }

    Ok((sfc, template_offset, metadata))
}

fn block_name(rest: &str) -> Option<&'static str> {
    for name in ["template", "script", "style"] {
        if let Some(after) = rest
            .strip_prefix('<')
            .and_then(|r| r.strip_prefix(name))
            .filter(|after| after.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/'))
        {
            let _ = after;
            return Some(name);
        }
    }
    None
}

/// Find the byte offset of the matching `</name>` starting at `from`, honouring
/// nested template openings. Script and style bodies are raw text.
fn find_block_end(src: &str, from: usize, name: &str) -> Result<Option<usize>, RawParseError> {
    let close = format!("</{name}");
    let mut depth = 1usize;
    let mut i = from;
    while i < src.len() {
        let tail = &src[i..];
        if name == "template" {
            if let Some(after) = tail.strip_prefix("<!--") {
                i += 4 + after.find("-->").map(|e| e + 3).unwrap_or(after.len());
                continue;
            }
        }
        if let Some(after) = tail.strip_prefix(&close).filter(|after| {
            after.starts_with('>') || after.starts_with(|ch: char| ch.is_whitespace())
        }) {
            if !after.trim_start().starts_with('>') {
                return Ok(None);
            }
            depth -= 1;
            if depth == 0 {
                return Ok(Some(i));
            }
            i += close.len();
            continue;
        }
        if name == "template" && tail.starts_with("{{") {
            // Leave unclosed moustaches to the template parser for its precise diagnostic.
            i += super::super::match_delimiter(tail, '{', '}').map_or(2, |end| end + 1);
            continue;
        }
        if name == "template" && tail.starts_with('<') {
            if tail.starts_with("</") {
                let Some(end) = tail.find('>') else {
                    return Ok(None);
                };
                i += end + 1;
            } else {
                let tag_tail = &tail[1..];
                let end = tag_tail
                    .find(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
                    .unwrap_or(tag_tail.len());
                let (_, remaining, self_closing) =
                    super::super::jsx::jsx_parse_attrs(src, &tag_tail[end..])?;
                if &tag_tail[..end] == "template" && !self_closing {
                    depth += 1;
                }
                i = src.len() - remaining.len();
            }
            continue;
        }
        i += tail.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
    }
    Ok(None)
}

// SFC block metadata uses literal HTML values, never JSX expressions or escapes.
fn parse_sfc_attrs<'a>(
    root: &'a str,
    mut rest: &'a str,
) -> Result<(Vec<JsxAttr>, &'a str, bool), RawParseError> {
    let mut attrs = Vec::new();
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("/>") {
            return Ok((attrs, after, true));
        }
        if let Some(after) = rest.strip_prefix('>') {
            return Ok((attrs, after, false));
        }
        let end = rest
            .find(|ch: char| ch.is_whitespace() || matches!(ch, '=' | '>' | '/'))
            .unwrap_or(rest.len());
        if end == 0 || rest[..end].contains(['\"', '\'', '`', '<']) {
            return Err(jsx_err(root, rest, "invalid or unclosed Vue SFC attribute"));
        }
        let key = rest[..end].to_owned();
        rest = rest[end..].trim_start();
        let value = if let Some(after) = rest.strip_prefix('=') {
            rest = after.trim_start();
            let (value, after) = if rest.starts_with(['\"', '\'']) {
                let quote = rest.as_bytes()[0] as char;
                let end = rest[1..]
                    .find(quote)
                    .ok_or_else(|| jsx_err(root, rest, "unclosed quoted Vue SFC attribute"))?
                    + 1;
                (&rest[1..end], &rest[end + 1..])
            } else {
                let end = rest
                    .find(|ch: char| ch.is_whitespace() || ch == '>')
                    .unwrap_or(rest.len());
                let value = &rest[..end];
                if value.is_empty() || value.contains(['\"', '\'', '`', '<', '=']) {
                    return Err(jsx_err(
                        root,
                        rest,
                        "invalid unquoted Vue SFC attribute value",
                    ));
                }
                (value, &rest[end..])
            };
            if !after.is_empty()
                && !after.starts_with(|ch: char| ch.is_whitespace() || ch == '>' || ch == '/')
            {
                return Err(jsx_err(
                    root,
                    after,
                    "Vue SFC attributes require whitespace separation",
                ));
            }
            rest = after;
            JsxAttrValue::Str(value.to_owned())
        } else {
            JsxAttrValue::Bool(true)
        };
        attrs.push(JsxAttr { key, value });
    }
}

fn has_flag(attrs: &[super::super::jsx::JsxAttr], flag: &str) -> bool {
    attrs.iter().any(|attribute| attribute.key == flag)
}

fn attr_value(attrs: &[super::super::jsx::JsxAttr], key: &str) -> Option<String> {
    attrs
        .iter()
        .find(|attribute| attribute.key == key)
        .and_then(|attribute| attribute.as_str())
        .map(str::to_string)
}
