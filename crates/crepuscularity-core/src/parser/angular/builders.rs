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
fn literal_expr(attr: &JsxAttr) -> Result<String, RawParseError> {
    match &attr.value {
        JsxAttrValue::Str(s) => {
            let trimmed = s.trim();
            if let Some(inner) = trimmed
                .strip_prefix("{{")
                .and_then(|r| r.strip_suffix("}}"))
            {
                if !inner.contains("{{") {
                    return Ok(inner.trim().to_string());
                }
            }
            if trimmed.contains("{{") {
                return Err(unsupported(format!(
                    "interpolation mixed with literal text in attribute value {s:?} is not \
                     supported; use a `[prop]=\"…\"` binding instead"
                )));
            }
            Ok(format!("\"{}\"", s.replace('"', "\\\"")))
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

        if let Some(name) = key.strip_prefix('*') {
            match name {
                "ngIf" => {
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
                "ngFor" => for_spec = Some(parse_for_head(&directive_expr(attr))?),
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
            bindings.push(Binding {
                prop: "bind".to_string(),
                value: directive_expr(attr),
            });
            continue;
        }

        if let Some(inner) = key.strip_prefix('[').and_then(|k| k.strip_suffix(']')) {
            let expr = directive_expr(attr);
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
                bindings.push(Binding {
                    prop: name.to_string(),
                    value: expr,
                });
                continue;
            }
            if inner == "innerHTML" {
                replacement_children = Some(vec![Node::RawHtml(expr)]);
                continue;
            }
            if inner == "innerText" || inner == "textContent" {
                replacement_children = Some(vec![Node::Text(vec![TextPart::Expr(expr)])]);
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
                prop: inner.to_string(),
                value: expr,
            });
            continue;
        }

        if let Some(inner) = key.strip_prefix('(').and_then(|k| k.strip_suffix(')')) {
            if let Some((event, pseudo)) = inner.split_once('.') {
                return Err(unsupported(format!(
                    "pseudo-event `({event}.{pseudo})` is not supported: the shared \
                     EventHandler has no key-filter modifiers"
                )));
            }
            if inner.is_empty() {
                return Err(unsupported("`()` is missing an event name"));
            }
            event_handlers.push(EventHandler {
                event: inner.to_string(),
                modifiers: Vec::new(),
                handler: directive_expr(attr),
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

        bindings.push(Binding {
            prop: key.to_string(),
            value: literal_expr(attr)?,
        });
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
        if extra.starts_with("track") {
            continue;
        }
        return Err(unsupported(format!(
            "`{extra}` in a for-loop head is not supported: the shared ForBlock binds \
             only the item variable"
        )));
    }

    let lhs_rhs = [" of ", " in "]
        .iter()
        .find_map(|kw| main.find(kw).map(|pos| (&main[..pos], &main[pos + 4..])));
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
    Ok((pattern.to_string(), iterator.to_string()))
}

fn split_top_level_semicolons(src: &str) -> Vec<&str> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::jsx::{JsxAttr, JsxAttrValue};

    #[test]
    fn test_directive_expr() {
        assert_eq!(
            directive_expr(&JsxAttr {
                key: "k".into(),
                value: JsxAttrValue::Str("   value   ".into()),
            }),
            "value"
        );
        assert_eq!(
            directive_expr(&JsxAttr {
                key: "k".into(),
                value: JsxAttrValue::Expr("   x + 1   ".into()),
            }),
            "x + 1"
        );
        assert_eq!(
            directive_expr(&JsxAttr {
                key: "k".into(),
                value: JsxAttrValue::Bool(true),
            }),
            ""
        );
        assert_eq!(
            directive_expr(&JsxAttr {
                key: "k".into(),
                value: JsxAttrValue::Bool(false),
            }),
            ""
        );
    }
}
