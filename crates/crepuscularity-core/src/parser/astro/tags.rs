//! Astro element and attribute parsing.

use crate::ast::*;

use super::super::jsx::attrs::{
    jsx_parse_attrs_with_expression_boundary, jsx_parse_authored_attrs_with_expression_boundary,
    JsxAttr, JsxAttrItem, JsxAttrValue,
};
use super::super::jsx::builders::jsx_build_element;
use super::super::jsx::text::jsx_close;
use super::super::vue::builders::apply_dynamic_class;
use super::super::RawParseError;
use super::{astro_err, parse_astro_nodes, ParseMode};

pub(super) const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Directives whose runtime semantics this frontend cannot honour.
const UNSUPPORTED_DIRECTIVES: &[&str] = &["transition:", "define:vars", "is:raw", "is:inline"];

/// Attributes extracted from an Astro tag that do not map onto a JSX attribute.
#[derive(Default)]
struct AstroExtras {
    classes: Vec<String>,
    conditional_classes: Vec<ConditionalClass>,
    replacement_children: Option<Vec<Node>>,
}

pub(crate) fn parse_astro_tag<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
    mode: ParseMode,
) -> Result<(Vec<Node>, &'a str), RawParseError> {
    let (tag, attrs, after_gt, self_closing) = read_opening(root, src, depth, mode)?;

    if tag.eq_ignore_ascii_case("script") || tag.eq_ignore_ascii_case("style") {
        return Err(astro_err(
            root,
            src,
            "script/style raw-text elements are not supported by the shared Astro frontend",
        ));
    }

    if tag == "slot" && !mode.components {
        return Err(astro_err(
            root,
            src,
            "`<slot />` is not supported by the Astro frontend \
             (use crepuscularity `include` slots instead)",
        ));
    }
    if !mode.components && tag != "Fragment" && tag.starts_with(|c: char| c.is_ascii_uppercase()) {
        return Err(astro_err(
            root,
            src,
            format!(
                "component tag `<{tag}>` is not supported: this frontend compiles markup only \
                 and performs no module resolution"
            ),
        ));
    }

    let (attrs, extras) = rewrite_astro_attrs(root, src, attrs, mode.components)?;

    let (children, rest) = if self_closing || VOID_TAGS.contains(&tag) {
        (Vec::new(), after_gt)
    } else {
        let (children, rest) = parse_astro_nodes(
            root,
            after_gt,
            depth + 1,
            ParseMode {
                preserve_whitespace: mode.preserve_whitespace
                    || (mode.components && matches!(tag, "pre" | "textarea")),
                ..mode
            },
        )?;
        let rest = jsx_close(root, rest, tag)?;
        (children, rest)
    };

    let children = extras.replacement_children.unwrap_or(children);

    // `<Fragment>` is a structural wrapper: it contributes its children rather
    // than an element of its own.
    let slot_wrapper = mode.components
        && tag == "Fragment"
        && matches!(&attrs[..], [attribute] if attribute.key == "slot")
        && extras.classes.is_empty()
        && extras.conditional_classes.is_empty();
    if tag == "Fragment" && !slot_wrapper {
        if !attrs.is_empty() || !extras.classes.is_empty() || !extras.conditional_classes.is_empty()
        {
            return Err(astro_err(
                root,
                src,
                "`<Fragment>` accepts only `set:html` / `set:text`: it produces no element, \
                 so other attributes would be dropped",
            ));
        }
        return Ok((children, rest));
    }

    if mode.components {
        let component = tag.starts_with(|character: char| character.is_ascii_uppercase());
        let bindings = attrs
            .into_iter()
            .map(|attribute| {
                let (origin, value) = match attribute.value {
                    JsxAttrValue::Str(value) => (
                        BindingOrigin::Literal,
                        serde_json::Value::String(value).to_string(),
                    ),
                    JsxAttrValue::Expr(value) => (BindingOrigin::Expression, value),
                    JsxAttrValue::Bool(value) if component => {
                        (BindingOrigin::Expression, value.to_string())
                    }
                    JsxAttrValue::Bool(_) => (BindingOrigin::Literal, "\"\"".to_owned()),
                };
                Binding {
                    origin,
                    prop: attribute.key,
                    value,
                }
            })
            .collect();
        return Ok((
            vec![Node::Element(Element {
                tag: tag.to_owned(),
                id: None,
                classes: extras.classes,
                conditional_classes: extras.conditional_classes,
                event_handlers: Vec::new(),
                bindings,
                animations: Vec::new(),
                children,
            })],
            rest,
        ));
    }
    let mut element =
        jsx_build_element(tag, attrs, children).map_err(|message| astro_err(root, src, message))?;
    element.classes.extend(extras.classes);
    element
        .conditional_classes
        .extend(extras.conditional_classes);
    Ok((vec![Node::Element(element)], rest))
}

/// Rewrite Astro attribute syntax into the keys `jsx_build_element` understands.
///
/// `client:*` hydration directives are recorded verbatim as bindings rather than
/// dropped: the shared IR has no hydration concept, but keeping the directive on
/// the element means no information is lost silently.
fn rewrite_astro_attrs(
    root: &str,
    src: &str,
    attrs: Vec<JsxAttr>,
    authored: bool,
) -> Result<(Vec<JsxAttr>, AstroExtras), RawParseError> {
    let mut out = Vec::with_capacity(attrs.len());
    let mut extras = AstroExtras::default();

    for attr in attrs {
        let key = attr.key.clone();
        let key = key.as_str();

        // `{value}` shorthand and `{...spread}`.
        if let Some(inner) = key.strip_prefix('{') {
            let Some(name) = inner.strip_suffix('}') else {
                return Err(astro_err(
                    root,
                    src,
                    format!("malformed shorthand attribute `{key}`"),
                ));
            };
            let name = name.trim();
            if name.starts_with("...") {
                return Err(astro_err(
                    root,
                    src,
                    "spread attributes `{...props}` are not supported: the shared AST \
                     records named bindings only",
                ));
            }
            if !crate::eval::is_variable_name(name) {
                return Err(astro_err(
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
            return Err(astro_err(
                root,
                src,
                format!(
                    "`{prefix}` directives are not supported by the Astro frontend \
                     (found `{key}`)"
                ),
            ));
        }

        if key == "class:list" && authored {
            if matches!(&attr.value, JsxAttrValue::Bool(_))
                || matches!(&attr.value, JsxAttrValue::Expr(value) if value.trim().is_empty())
            {
                return Err(astro_err(
                    root,
                    src,
                    "class:list requires a literal class list or expression",
                ));
            }
            out.push(attr);
            continue;
        }
        if key == "class:list" {
            let expr = match &attr.value {
                JsxAttrValue::Expr(e) => e.trim().to_string(),
                JsxAttrValue::Str(s) => {
                    extras
                        .classes
                        .extend(s.split_whitespace().map(super::super::literal_class_token));
                    continue;
                }
                JsxAttrValue::Bool(_) => {
                    return Err(astro_err(
                        root,
                        src,
                        "class:list requires a literal class list or expression",
                    ));
                }
            };
            if expr.is_empty() {
                return Err(astro_err(
                    root,
                    src,
                    "class:list requires a nonempty expression",
                ));
            }
            apply_dynamic_class(&expr, &mut extras.classes, &mut extras.conditional_classes)
                .map_err(|e| astro_err(root, src, e.message))?;
            continue;
        }

        if matches!(key, "set:html" | "set:text") {
            if extras.replacement_children.is_some() {
                return Err(astro_err(
                    root,
                    src,
                    "an element may have only one content directive (set:html or set:text)",
                ));
            }
            let expression = match &attr.value {
                JsxAttrValue::Str(value) => {
                    let decoded = super::super::svelte_entities::decode_static_attribute(value);
                    if authored {
                        serde_json::Value::String(decoded).to_string()
                    } else {
                        super::super::quote_expression_string(&decoded)
                    }
                }
                _ => directive_expr(&attr),
            };
            if expression.is_empty() {
                return Err(astro_err(
                    root,
                    src,
                    format!("{key} requires an expression"),
                ));
            }
            extras.replacement_children = Some(vec![if key == "set:html" {
                Node::RawHtml(expression)
            } else {
                Node::Text(vec![TextPart::Expr(expression)])
            }]);
            continue;
        }

        if key.starts_with("class:") {
            return Err(astro_err(
                root,
                src,
                format!("`{key}` is not an Astro directive (Astro spells it `class:list`)"),
            ));
        }

        out.push(attr);
    }

    Ok((out, extras))
}

/// Read expression operands after literal directive values have been handled.
fn directive_expr(attr: &JsxAttr) -> String {
    match &attr.value {
        JsxAttrValue::Str(s) => s.trim().to_string(),
        JsxAttrValue::Expr(e) => e.trim().to_string(),
        JsxAttrValue::Bool(_) => String::new(),
    }
}

/// Shared tag-header lexer for markup parsing and expression boundary scanning.
pub(super) fn read_opening<'a>(
    root: &'a str,
    src: &'a str,
    depth: usize,
    mode: ParseMode,
) -> Result<(&'a str, Vec<JsxAttr>, &'a str, bool), RawParseError> {
    let after_lt = &src[1..];
    let name_end = after_lt
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(after_lt.len());
    let tag = &after_lt[..name_end];

    let attrs_src = after_lt[name_end..].trim_start();
    let boundary = |text: &str| super::scan::match_brace_at(text, depth + 1, mode);
    let (attrs, rest, closed) = if mode.components {
        let (items, rest, closed) =
            jsx_parse_authored_attrs_with_expression_boundary(root, attrs_src, boundary)?;
        let attrs = items
            .into_iter()
            .map(|item| match item {
                JsxAttrItem::Named(attribute) if attribute.key == "..." => Err(astro_err(
                    root,
                    src,
                    "spread attributes require `{...expression}` syntax",
                )),
                JsxAttrItem::Named(attribute) => Ok(attribute),
                JsxAttrItem::Spread(value) => Ok(JsxAttr {
                    key: "...".into(),
                    value: JsxAttrValue::Expr(value),
                }),
            })
            .collect::<Result<Vec<_>, _>>()?;
        (attrs, rest, closed)
    } else {
        jsx_parse_attrs_with_expression_boundary(root, attrs_src, boundary)?
    };
    Ok((tag, attrs, rest, closed))
}
