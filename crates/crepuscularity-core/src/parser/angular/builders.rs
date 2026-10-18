//! Angular element/directive lowering into the shared AST.

use crate::ast::*;

use super::super::jsx::{JsxAttr, JsxAttrValue};
use super::super::vue::builders::apply_dynamic_class;
use super::super::RawParseError;

/// An element plus the structural directives that wrap it.
pub(crate) struct AngularElement {
    pub element: Element,
    pub if_condition: Option<String>,
    pub for_spec: Option<(String, String)>,
    /// Replacement children produced by `[innerHTML]` / `[innerText]`.
    pub replacement_children: Option<Vec<Node>>,
}

fn unsupported(message: impl Into<String>) -> RawParseError {
    RawParseError {
        message: message.into(),
        byte_offset: None,
    }
}

/// Binding and directive attribute values are expression source.
fn directive_expr(attr: &JsxAttr) -> String {
    match &attr.value {
        JsxAttrValue::Str(s) => s.trim().to_string(),
        JsxAttrValue::Expr(e) => e.trim().to_string(),
        JsxAttrValue::Bool(_) => String::new(),
    }
}

/// Plain attributes are literals, except for a whole-value `{{ expr }}`, which
/// is the interpolated form of a property binding.
fn literal_expr(attr: &JsxAttr, decode_entities: bool) -> Result<String, RawParseError> {
    match &attr.value {
        JsxAttrValue::Str(s) => {
            let trimmed = s.trim();
            if let Some(inner) = trimmed
                .strip_prefix("{{")
                .and_then(|r| r.strip_suffix("}}"))
            {
                if !inner.contains("{{") {
                    let expression = inner.trim();
                    if expression.is_empty() {
                        return Err(unsupported(
                            "attribute interpolation requires an expression",
                        ));
                    }
                    return Ok(expression.to_string());
                }
            }
            if trimmed.contains("{{") {
                return Err(unsupported(format!(
                    "interpolation mixed with literal text in attribute value {s:?} is not \
                     supported; use a `[prop]=\"…\"` binding instead"
                )));
            }
            let value = if decode_entities {
                super::super::svelte_entities::decode_html_entities(s, true)
            } else {
                s.clone()
            };
            Ok(super::super::quote_expression_string(&value))
        }
        JsxAttrValue::Expr(e) => Ok(e.clone()),
        JsxAttrValue::Bool(b) => Ok(b.to_string()),
    }
}

pub(crate) fn angular_build_element(
    tag: &str,
    attrs: Vec<JsxAttr>,
    children: Vec<Node>,
) -> Result<AngularElement, RawParseError> {
    if attrs
        .iter()
        .filter(|attribute| matches!(attribute.key.as_str(), "id" | "[id]" | "[attr.id]"))
        .take(2)
        .count()
        > 1
    {
        return Err(unsupported(
            "an element may have only one id attribute or binding",
        ));
    }
    let mut id = None;
    let mut classes: Vec<String> = Vec::new();
    let mut conditional_classes: Vec<ConditionalClass> = Vec::new();
    let mut event_handlers: Vec<EventHandler> = Vec::new();
    let mut bindings: Vec<Binding> = Vec::new();
    let mut if_condition = None;
    let mut for_spec = None;
    let mut replacement_children = None;

    for attr in &attrs {
        let key = attr.key.as_str();

        if key == "class" {
            if matches!(&attr.value, JsxAttrValue::Expr(_)) {
                return Err(unsupported(
                    "class expressions are not supported here; use `[class]=\"…\"` instead",
                ));
            }
            if let JsxAttrValue::Str(value) = &attr.value {
                if value.contains("{{") {
                    classes.push(format!("{{{}}}", literal_expr(attr, false)?));
                } else {
                    classes.extend(
                        value
                            .split_whitespace()
                            .map(super::super::literal_class_token),
                    );
                }
            }
            continue;
        }
        if key == "id" {
            if let JsxAttrValue::Str(value) = &attr.value {
                if value.contains("{{") {
                    bindings.push(Binding {
                        origin: crate::ast::BindingOrigin::Expression,
                        prop: "id".to_string(),
                        value: literal_expr(attr, false)?,
                    });
                } else {
                    id = Some(value.clone());
                }
            } else {
                let expression = directive_expr(attr);
                if expression.is_empty() {
                    return Err(unsupported(
                        "id attributes require a literal or nonempty expression",
                    ));
                }
                bindings.push(Binding {
                    origin: crate::ast::BindingOrigin::Expression,
                    prop: "id".to_string(),
                    value: expression,
                });
            }
            continue;
        }

        if let Some(name) = key.strip_prefix('*') {
            match name {
                "ngIf" => {
                    if if_condition.is_some() {
                        return Err(unsupported("an element may have only one *ngIf directive"));
                    }
                    let expr = directive_expr(attr);
                    if expr.contains(';') {
                        return Err(unsupported(
                            "`*ngIf` with `; else` or `; as` is not supported: template \
                             reference variables are outside this frontend's scope",
                        ));
                    }
                    if expr.is_empty() {
                        return Err(unsupported("`*ngIf` requires an expression"));
                    }
                    if_condition = Some(expr);
                }
                "ngFor" => {
                    if for_spec.is_some() {
                        return Err(unsupported("an element may have only one *ngFor directive"));
                    }
                    for_spec = Some(parse_for_head(&directive_expr(attr))?);
                }
                other => {
                    return Err(unsupported(format!(
                        "unsupported structural directive `*{other}` \
                         (supported: `*ngIf`, `*ngFor`)"
                    )))
                }
            }
            continue;
        }

        if let Some(inner) = key.strip_prefix("[(").and_then(|k| k.strip_suffix(")]")) {
            if inner != "ngModel" {
                return Err(unsupported(format!(
                    "two-way binding `[({inner})]` is not supported (only `[(ngModel)]` is)"
                )));
            }
            let expr = directive_expr(attr);
            if expr.is_empty() {
                return Err(unsupported("`[(ngModel)]` requires an expression"));
            }
            bindings.push(Binding {
                origin: crate::ast::BindingOrigin::Expression,
                prop: "bind".to_string(),
                value: expr,
            });
            continue;
        }

        if let Some(inner) = key.strip_prefix('[').and_then(|k| k.strip_suffix(']')) {
            let expr = directive_expr(attr);
            if inner.is_empty() || expr.is_empty() {
                return Err(unsupported(format!(
                    "{key} requires a property name and a nonempty expression"
                )));
            }
            if let Some(class) = inner.strip_prefix("class.") {
                if class.is_empty() {
                    return Err(unsupported("`[class.]` is missing a class name"));
                }
                conditional_classes.push(ConditionalClass {
                    class: class.to_string(),
                    condition: expr,
                });
                continue;
            }
            if inner.starts_with("style.") || inner == "ngStyle" {
                return Err(unsupported(format!(
                    "`[{inner}]` is not supported; use utility classes instead"
                )));
            }
            if inner == "ngClass" || inner == "class" {
                apply_dynamic_class(&expr, &mut classes, &mut conditional_classes)?;
                continue;
            }
            if let Some(name) = inner.strip_prefix("attr.") {
                if name.is_empty() {
                    return Err(unsupported("`[attr.]` is missing an attribute name"));
                }
                bindings.push(Binding {
                    origin: crate::ast::BindingOrigin::Attribute,
                    prop: name.to_string(),
                    value: expr,
                });
                continue;
            }
            if matches!(inner, "innerHTML" | "innerText" | "textContent") {
                if replacement_children.is_some() {
                    return Err(unsupported(
                        "an element may have only one content binding (innerHTML, innerText, or textContent)",
                    ));
                }
                replacement_children = Some(vec![if inner == "innerHTML" {
                    Node::RawHtml(expr)
                } else {
                    Node::Text(vec![TextPart::Expr(expr)])
                }]);
                continue;
            }
            if matches!(inner, "ngIf" | "ngFor" | "ngForOf" | "ngSwitch") {
                return Err(unsupported(format!(
                    "`[{inner}]` is not supported; write the structural form `*{inner}` instead"
                )));
            }
            if inner.starts_with('@') {
                return Err(unsupported(format!(
                    "animation binding `[{inner}]` is not supported"
                )));
            }
            bindings.push(Binding {
                origin: crate::ast::BindingOrigin::Expression,
                prop: inner.to_string(),
                value: expr,
            });
            continue;
        }

        if let Some(inner) = key.strip_prefix('(').and_then(|k| k.strip_suffix(')')) {
            if let Some((target, _)) = inner.split_once(':') {
                if matches!(target, "window" | "document" | "body") {
                    return Err(unsupported(format!(
                        "global event target `({inner})` is not supported: the shared EventHandler has no target field"
                    )));
                }
            }
            if let Some((event, pseudo)) = inner.split_once('.') {
                return Err(unsupported(format!(
                    "pseudo-event `({event}.{pseudo})` is not supported: the shared \
                     EventHandler has no key-filter modifiers"
                )));
            }
            if inner.is_empty() {
                return Err(unsupported("`()` is missing an event name"));
            }
            let handler = directive_expr(attr);
            if handler.is_empty() {
                return Err(unsupported(format!("{key} requires a handler expression")));
            }
            event_handlers.push(EventHandler {
                model_before_listener: None,
                event: inner.to_string(),
                modifiers: Vec::new(),
                handler,
            });
            continue;
        }

        if key.starts_with('#') {
            return Err(unsupported(format!(
                "template reference variable `{key}` is not supported: it names a node \
                 the shared AST cannot address"
            )));
        }
        if key.starts_with('@') {
            return Err(unsupported(format!(
                "animation trigger `{key}` is not supported"
            )));
        }

        let value = literal_expr(attr, true)?;
        if value.trim().is_empty() {
            return Err(unsupported(
                "attribute bindings require a nonempty expression",
            ));
        }
        bindings.push(Binding {
            origin: crate::ast::BindingOrigin::Expression,
            prop: key.to_string(),
            value,
        });
    }

    for (index, binding) in bindings.iter().enumerate() {
        if binding.origin != BindingOrigin::Attribute {
            continue;
        }
        if bindings.iter().enumerate().any(|(other_index, other)| {
            other_index != index && other.prop.eq_ignore_ascii_case(&binding.prop)
        }) || (binding.prop.eq_ignore_ascii_case("id") && id.is_some())
            || (binding.prop.eq_ignore_ascii_case("class")
                && (!classes.is_empty() || !conditional_classes.is_empty()))
        {
            return Err(unsupported(format!(
                "attribute binding `{}` cannot be combined with another binding or class list for the same attribute",
                binding.prop
            )));
        }
    }

    if if_condition.is_some() && for_spec.is_some() {
        return Err(unsupported(
            "`*ngIf` and `*ngFor` on the same element are not supported; \
             wrap one of them in an `<ng-container>`",
        ));
    }

    Ok(AngularElement {
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
        if_condition,
        for_spec,
        replacement_children,
    })
}

/// `let item of items; trackBy: fn`, `item of items; track item.id`.
pub(crate) fn parse_for_head(src: &str) -> Result<(String, String), RawParseError> {
    let segments = split_top_level_semicolons(src);
    let main = segments.first().copied().unwrap_or("").trim();

    for extra in segments.iter().skip(1) {
        let extra = extra.trim();
        if extra.is_empty() {
            continue;
        }
        // `track` / `trackBy` are pure DOM-diffing concerns the shared IR does
        // not model, so they are accepted and dropped. Everything else in the
        // head binds a variable that would be undefined at render time.
        let tracking = extra.strip_prefix("trackBy:").or_else(|| {
            extra
                .strip_prefix("track")
                .filter(|tail| tail.starts_with(char::is_whitespace))
        });
        if let Some(expression) = tracking {
            if expression.trim().is_empty() {
                return Err(unsupported("loop tracking requires an expression"));
            }
            continue;
        }
        return Err(unsupported(format!(
            "`{extra}` in a for-loop head is not supported: the shared ForBlock binds \
             only the item variable"
        )));
    }

    let lhs_rhs = super::super::scan_top_level(main, |i, _| {
        main[i..].starts_with(" of ") || main[i..].starts_with(" in ")
    })
    .map(|pos| (&main[..pos], &main[pos + 4..]));
    let Some((lhs, iterator)) = lhs_rhs else {
        return Err(unsupported(format!(
            "could not parse for-loop head {src:?} (expected `let item of items`)"
        )));
    };

    let pattern = lhs.trim().strip_prefix("let ").unwrap_or(lhs).trim();
    if pattern.is_empty() {
        return Err(unsupported(format!(
            "for-loop head {src:?} has no item binding"
        )));
    }
    if pattern.starts_with('{') || pattern.starts_with('[') {
        return Err(unsupported(
            "destructuring a for-loop item binding is not supported: the shared \
             ForBlock binds a single item variable",
        ));
    }
    if pattern.contains(',') {
        return Err(unsupported(
            "multiple for-loop bindings are not supported: the shared ForBlock has no \
             index variable, so it would be undefined at render time",
        ));
    }

    let iterator = iterator.trim();
    if iterator.is_empty() {
        return Err(unsupported(format!(
            "for-loop head {src:?} has no iterable expression"
        )));
    }
    if !crate::eval::is_variable_name(pattern) {
        return Err(unsupported("loop requires a supported item variable name"));
    }
    Ok((pattern.to_string(), iterator.to_string()))
}

fn split_top_level_semicolons(src: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut start = 0usize;
    for (i, c) in src.char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth -= 1,
            ';' if depth == 0 => {
                parts.push(&src[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&src[start..]);
    parts
}
