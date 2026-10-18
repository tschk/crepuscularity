//! Vue element/directive lowering into the shared AST.

use crate::ast::*;

use super::super::jsx::{JsxAttr, JsxAttrValue};
use super::super::RawParseError;

/// Which `v-if` chain slot an element occupies.
#[derive(Debug, Clone)]
pub(crate) enum VueBranch {
    If(String),
    ElseIf(String),
    Else,
}

/// An element plus the structural directives that wrap it.
pub(crate) struct VueElement {
    pub element: Element,
    pub branch: Option<VueBranch>,
    pub for_spec: Option<(String, String)>,
    /// Replacement children produced by `v-html` / `v-text`.
    pub replacement_children: Option<Vec<Node>>,
}

const SUPPORTED_EVENT_MODIFIERS: [&str; 5] = ["prevent", "stop", "self", "once", "capture"];

fn unsupported(message: impl Into<String>) -> RawParseError {
    RawParseError {
        message: message.into(),
        byte_offset: None,
    }
}

/// Directive attribute values are expression source; plain attributes are literals.
fn directive_expr(attr: &JsxAttr) -> String {
    match &attr.value {
        JsxAttrValue::Str(s) => s.trim().to_string(),
        JsxAttrValue::Expr(e) => e.trim().to_string(),
        JsxAttrValue::Bool(_) => String::new(),
    }
}

fn literal_expr(attr: &JsxAttr) -> String {
    match &attr.value {
        JsxAttrValue::Str(s) => format!("\"{}\"", s.replace('"', "\\\"")),
        JsxAttrValue::Expr(e) => e.clone(),
        JsxAttrValue::Bool(b) => b.to_string(),
    }
}

pub(crate) fn vue_build_element(
    tag: &str,
    attrs: Vec<JsxAttr>,
    children: Vec<Node>,
) -> Result<VueElement, RawParseError> {
    let mut id = None;
    let mut classes: Vec<String> = Vec::new();
    let mut conditional_classes: Vec<ConditionalClass> = Vec::new();
    let mut event_handlers: Vec<EventHandler> = Vec::new();
    let mut bindings: Vec<Binding> = Vec::new();
    let mut branch = None;
    let mut for_spec = None;
    let mut replacement_children = None;

    for attr in &attrs {
        let key = attr.key.as_str();

        if key == "class" {
            if let JsxAttrValue::Str(s) = &attr.value {
                classes.extend(s.split_whitespace().map(|c| c.to_string()));
            }
            continue;
        }
        if key == "id" {
            if let JsxAttrValue::Str(s) = &attr.value {
                id = Some(s.clone());
            }
            continue;
        }
        if key == "v-if" {
            branch = Some(VueBranch::If(directive_expr(attr)));
            continue;
        }
        if key == "v-else-if" {
            branch = Some(VueBranch::ElseIf(directive_expr(attr)));
            continue;
        }
        if key == "v-else" {
            branch = Some(VueBranch::Else);
            continue;
        }
        if key == "v-for" {
            for_spec = Some(parse_v_for(&directive_expr(attr))?);
            continue;
        }
        if key == "v-show" {
            let expr = directive_expr(attr);
            if expr.is_empty() {
                return Err(unsupported("v-show requires an expression"));
            }
            conditional_classes.push(ConditionalClass {
                class: "hidden".to_string(),
                condition: format!("!({expr})"),
            });
            continue;
        }
        if key == "v-html" {
            replacement_children = Some(vec![Node::RawHtml(directive_expr(attr))]);
            continue;
        }
        if key == "v-text" {
            replacement_children =
                Some(vec![Node::Text(vec![TextPart::Expr(directive_expr(attr))])]);
            continue;
        }
        if key == "v-model" || key.starts_with("v-model.") {
            if key != "v-model" {
                return Err(unsupported(format!(
                    "unsupported v-model modifiers in {key:?} (only plain v-model is supported)"
                )));
            }
            bindings.push(Binding {
                origin: crate::ast::BindingOrigin::Expression,
                prop: "bind".to_string(),
                value: directive_expr(attr),
            });
            continue;
        }
        if key == "v-bind" {
            return Err(unsupported(
                "object spread `v-bind=\"obj\"` is not supported; bind properties individually",
            ));
        }
        if let Some(arg) = key
            .strip_prefix("v-bind:")
            .or_else(|| key.strip_prefix(':'))
        {
            if arg.contains('.') {
                return Err(unsupported(format!(
                    "unsupported v-bind modifiers in {key:?}"
                )));
            }
            if arg.starts_with('[') {
                return Err(unsupported("dynamic binding arguments are not supported"));
            }
            let expr = directive_expr(attr);
            match arg {
                "class" => apply_dynamic_class(&expr, &mut classes, &mut conditional_classes)?,
                "id" => bindings.push(Binding {
                    origin: crate::ast::BindingOrigin::Expression,
                    prop: "id".to_string(),
                    value: expr,
                }),
                "style" => {
                    return Err(unsupported(
                        ":style is not supported; use utility classes instead",
                    ))
                }
                _ => bindings.push(Binding {
                    origin: crate::ast::BindingOrigin::Expression,
                    prop: arg.to_string(),
                    value: expr,
                }),
            }
            continue;
        }
        if let Some(arg) = key.strip_prefix("v-on:").or_else(|| key.strip_prefix('@')) {
            if arg.starts_with('[') {
                return Err(unsupported("dynamic event names are not supported"));
            }
            let mut parts = arg.split('.');
            let event = parts.next().unwrap_or("").to_string();
            let mut modifiers = Vec::new();
            for m in parts {
                if !SUPPORTED_EVENT_MODIFIERS.contains(&m) {
                    return Err(unsupported(format!(
                        "unsupported event modifier {m:?} in {key:?} (supported: {})",
                        SUPPORTED_EVENT_MODIFIERS.join(", ")
                    )));
                }
                modifiers.push(m.to_string());
            }
            event_handlers.push(EventHandler {
                event,
                modifiers,
                handler: directive_expr(attr),
            });
            continue;
        }
        if let Some(rest) = key.strip_prefix("v-") {
            let name = rest.split(['.', ':']).next().unwrap_or(rest);
            return Err(unsupported(format!("unsupported Vue directive `v-{name}`")));
        }
        if key.starts_with('#') {
            return Err(unsupported(
                "slot shorthand `#name` is not supported (slots are out of scope)",
            ));
        }

        bindings.push(Binding {
            origin: crate::ast::BindingOrigin::Expression,
            prop: key.to_string(),
            value: literal_expr(attr),
        });
    }

    Ok(VueElement {
        element: Element {
            tag: tag.to_string(),
            id,
            classes,
            conditional_classes,
            event_handlers,
            bindings,
            animations: Vec::new(),
            children,
        },
        branch,
        for_spec,
        replacement_children,
    })
}

/// `item in items`, `(item, i) in items`, `item of items`.
fn parse_v_for(src: &str) -> Result<(String, String), RawParseError> {
    let (lhs, iterator) = split_for_parts(src)
        .ok_or_else(|| unsupported(format!("could not parse v-for expression {src:?}")))?;
    let lhs = lhs.trim();
    let inner = lhs
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(lhs);
    let pattern = inner.split(',').next().unwrap_or("").trim().to_string();
    if pattern.is_empty() {
        return Err(unsupported(format!(
            "v-for expression {src:?} has no item binding"
        )));
    }
    Ok((pattern, iterator.trim().to_string()))
}

fn split_for_parts(src: &str) -> Option<(&str, &str)> {
    for keyword in [" in ", " of "] {
        if let Some(pos) = src.find(keyword) {
            return Some((&src[..pos], &src[pos + keyword.len()..]));
        }
    }
    None
}

/// `:class` supports plain expressions, object syntax and array syntax.
pub(crate) fn apply_dynamic_class(
    expr: &str,
    classes: &mut Vec<String>,
    conditional_classes: &mut Vec<ConditionalClass>,
) -> Result<(), RawParseError> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Ok(());
    }
    if let Some(inner) = expr.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        for entry in split_top_level(inner) {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let colon = entry.rfind(':').ok_or_else(|| {
                unsupported(format!(":class object entry {entry:?} is missing a value"))
            })?;
            let name = entry[..colon].trim().trim_matches(['"', '\'']);
            let condition = entry[colon + 1..].trim();
            if name.is_empty() || condition.is_empty() {
                return Err(unsupported(format!(
                    ":class object entry {entry:?} is invalid"
                )));
            }
            for class in name.split_whitespace() {
                conditional_classes.push(ConditionalClass {
                    class: class.to_string(),
                    condition: condition.to_string(),
                });
            }
        }
        return Ok(());
    }
    if let Some(inner) = expr.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        for entry in split_top_level(inner) {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            if entry.starts_with('{') {
                apply_dynamic_class(entry, classes, conditional_classes)?;
            } else if (entry.starts_with('"') && entry.ends_with('"'))
                || (entry.starts_with('\'') && entry.ends_with('\''))
            {
                classes.extend(
                    entry[1..entry.len() - 1]
                        .split_whitespace()
                        .map(|c| c.to_string()),
                );
            } else {
                classes.push(format!("{{{entry}}}"));
            }
        }
        return Ok(());
    }
    if (expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2)
        || (expr.starts_with('\'') && expr.ends_with('\'') && expr.len() >= 2)
    {
        classes.extend(
            expr[1..expr.len() - 1]
                .split_whitespace()
                .map(|c| c.to_string()),
        );
        return Ok(());
    }
    classes.push(format!("{{{expr}}}"));
    Ok(())
}

/// Split on commas that are not nested inside brackets, braces or quotes.
pub(crate) fn split_top_level(src: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut start = 0usize;
    for (i, c) in src.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&src[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&src[start..]);
    parts
}
