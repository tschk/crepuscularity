//! Svelte element and attribute parsing.

use crate::ast::Node;

use super::super::jsx::attrs::{jsx_parse_attrs, JsxAttr, JsxAttrValue};
use super::super::jsx::builders::{
    jsx_build_element_with_class_literals, jsx_build_include, jsx_validate_source_attrs, DOM_EVENTS,
};
use super::super::jsx::text::jsx_close;
use super::super::RawParseError;
use super::{parse_svelte_nodes, svelte_err};

pub(super) const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Directives whose runtime semantics this frontend cannot honour.
const UNSUPPORTED_DIRECTIVES: &[&str] = &[
    "use:",
    "transition:",
    "in:",
    "out:",
    "animate:",
    "let:",
    "style:",
];

pub(crate) fn parse_svelte_tag<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
) -> Result<(Node, &'a str), RawParseError> {
    let src = src.trim_start();
    let after_lt = &src[1..];
    let name_end = after_lt
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(after_lt.len());
    let tag = &after_lt[..name_end];

    if tag.starts_with("svelte:") {
        return Err(svelte_err(
            root,
            src,
            format!("`<{tag}>` special elements are not supported by the Svelte frontend"),
        ));
    }
    if tag == "slot" {
        return Err(svelte_err(
            root,
            src,
            "`<slot>` is not supported by the Svelte frontend \
             (use crepuscularity `include` slots instead)",
        ));
    }
    if tag.starts_with(|c: char| c.is_ascii_uppercase()) {
        return Err(svelte_err(
            root,
            src,
            format!(
                "component tag `<{tag}>` is not supported: this frontend compiles markup only \
                 and performs no module resolution"
            ),
        ));
    }

    let rest = after_lt[name_end..].trim_start();
    let (attrs, after_gt, self_closing) = jsx_parse_attrs(root, rest)?;
    let attrs = rewrite_svelte_attrs(root, src, attrs)?;

    if tag == "include" {
        jsx_validate_source_attrs(tag, &attrs).map_err(|message| svelte_err(root, src, message))?;
    }
    let (children, rest) = if self_closing || VOID_TAGS.contains(&tag) {
        (vec![], after_gt)
    } else {
        let (children, rest) = parse_svelte_nodes(root, after_gt, depth + 1)?;
        (children, jsx_close(root, rest, tag)?)
    };
    let node = if tag == "include" {
        jsx_build_include(attrs, children)
    } else {
        Node::Element(
            jsx_build_element_with_class_literals(tag, attrs, children, false)
                .map_err(|message| svelte_err(root, src, message))?,
        )
    };
    Ok((node, rest))
}

/// Rewrite Svelte attribute syntax into the keys `jsx_build_element` understands.
fn rewrite_svelte_attrs(
    root: &str,
    src: &str,
    attrs: Vec<JsxAttr>,
) -> Result<Vec<JsxAttr>, RawParseError> {
    let mut out = Vec::with_capacity(attrs.len());

    for attr in attrs {
        let key = attr.key.clone();
        let key = key.as_str();

        // `{value}` shorthand and `{...spread}`.
        if let Some(inner) = key.strip_prefix('{') {
            let Some(name) = inner.strip_suffix('}') else {
                return Err(svelte_err(
                    root,
                    src,
                    format!("malformed shorthand attribute `{key}`"),
                ));
            };
            let name = name.trim();
            if name.starts_with("...") {
                return Err(svelte_err(
                    root,
                    src,
                    "spread attributes `{...props}` are not supported: the shared AST \
                     records named bindings only",
                ));
            }
            if !crate::eval::is_variable_name(name) {
                return Err(svelte_err(
                    root,
                    src,
                    "shorthand attributes require a supported variable name",
                ));
            }
            out.push(JsxAttr {
                key: name.to_string(),
                value: JsxAttrValue::Expr(name.to_string()),
            });
            continue;
        }

        if let Some(prefix) = UNSUPPORTED_DIRECTIVES.iter().find(|p| key.starts_with(**p)) {
            return Err(svelte_err(
                root,
                src,
                format!(
                    "`{prefix}` directives are not supported by the Svelte frontend \
                     (found `{key}`)"
                ),
            ));
        }

        // `on:click={h}` / `on:click|once={h}` → `@click` / `@click|once`.
        if let Some(event) = key.strip_prefix("on:") {
            if event.split('|').next().is_none_or(str::is_empty) {
                return Err(svelte_err(root, src, "on directives require an event name"));
            }
            if matches!(attr.value, JsxAttrValue::Bool(_)) {
                return Err(svelte_err(
                    root,
                    src,
                    "Svelte event forwarding is not supported",
                ));
            }
            let mut parts = event.split('|');
            let mut translated = format!("@{}", parts.next().unwrap_or_default());
            for modifier in parts {
                let modifier = match modifier {
                    "preventDefault" => "prevent",
                    "stopPropagation" => "stop",
                    "stopImmediatePropagation" => "immediate",
                    "self" | "once" | "capture" | "trusted" => modifier,
                    _ => {
                        return Err(svelte_err(
                            root,
                            src,
                            format!("unsupported Svelte event modifier `{modifier}`"),
                        ));
                    }
                };
                translated.push('|');
                translated.push_str(modifier);
            }
            out.push(JsxAttr {
                key: translated,
                value: attr.value,
            });
            continue;
        }

        // Svelte 5 `onclick={h}` → `@click`.
        if let Some(event) = key.strip_prefix("on") {
            if DOM_EVENTS.contains(&event) {
                out.push(JsxAttr {
                    key: format!("@{event}"),
                    value: attr.value,
                });
                continue;
            }
        }

        if let Some(name) = key
            .strip_prefix("class:")
            .or_else(|| key.strip_prefix("bind:"))
        {
            if name.is_empty() {
                return Err(svelte_err(
                    root,
                    src,
                    "class and bind directives require a name",
                ));
            }
            if matches!(&attr.value, JsxAttrValue::Expr(expression) if expression.trim().is_empty())
            {
                return Err(svelte_err(
                    root,
                    src,
                    "class and bind directives require a nonempty expression",
                ));
            }
        }

        // `class:foo` / `bind:value` shorthand — the bare attribute means the
        // expression is the directive name itself.
        let mut attr = attr;
        if (key.starts_with("class:") || key.starts_with("bind:"))
            && matches!(attr.value, JsxAttrValue::Bool(true))
        {
            let name = key
                .split_once(':')
                .map(|(_, n)| n)
                .unwrap_or("")
                .to_string();
            attr.value = JsxAttrValue::Expr(name);
        }

        // Svelte's two-way form bindings are the shared AST's `bind=` binding,
        // which every renderer already wires up for inputs, toggles and pickers.
        if let Some(name) = attr.key.strip_prefix("bind:") {
            if !matches!(name, "value" | "checked" | "group" | "files") {
                return Err(svelte_err(
                    root,
                    src,
                    format!(
                        "unsupported Svelte binding `bind:{name}` in the shared template frontend"
                    ),
                ));
            }
            attr.key = "bind:bind".to_string();
        }

        out.push(attr);
    }

    Ok(out)
}
