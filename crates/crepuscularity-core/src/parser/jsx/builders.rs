//! JSX AST node builders.

use crate::ast::*;

use super::attrs::{JsxAttr, JsxAttrValue};

pub(crate) fn jsx_build_element(tag: &str, attrs: Vec<JsxAttr>, children: Vec<Node>) -> Element {
    let mut id = None;
    let mut classes = Vec::new();
    let mut conditional_classes = Vec::new();
    let mut event_handlers = Vec::new();
    let mut bindings = Vec::new();
    let mut animations = Vec::new();

    for attr in attrs {
        let key = &attr.key;

        // class / className → split into individual class tokens
        if key == "class" || key == "className" {
            match &attr.value {
                JsxAttrValue::Str(s) => {
                    classes.extend(s.split_whitespace().map(|c| c.to_string()));
                }
                JsxAttrValue::Expr(e) => {
                    // Dynamic expression — keep as a single {expr} class token
                    classes.push(format!("{{{}}}", e));
                }
                JsxAttrValue::Bool(_) => {}
            }
            continue;
        }

        if key == "id" {
            if let Some(value) = attr.as_expr() {
                let trimmed = value.trim();
                if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
                    id = Some(trimmed[1..trimmed.len() - 1].to_string());
                }
            }
            continue;
        }

        // class:name={condition}
        if let Some(class_name) = key.strip_prefix("class:") {
            conditional_classes.push(ConditionalClass {
                class: class_name.to_string(),
                condition: attr.as_expr().unwrap_or_default(),
            });
            continue;
        }

        // when:{condition}="class1 class2 …"
        if let Some(cond_src) = key.strip_prefix("when:") {
            let condition = if cond_src.starts_with('{') && cond_src.ends_with('}') {
                cond_src[1..cond_src.len() - 1].trim().to_string()
            } else {
                cond_src.trim().to_string()
            };
            if condition.is_empty() {
                continue;
            }
            match &attr.value {
                JsxAttrValue::Str(s) => {
                    let mut iter = s.split_whitespace();
                    if let Some(first) = iter.next() {
                        let mut prev = first;
                        for next in iter {
                            if prev.is_empty() {
                                prev = next;
                                continue;
                            }
                            conditional_classes.push(ConditionalClass {
                                class: prev.to_string(),
                                condition: condition.clone(),
                            });
                            prev = next;
                        }
                        if !prev.is_empty() {
                            conditional_classes.push(ConditionalClass {
                                class: prev.to_string(),
                                condition,
                            });
                        }
                    }
                }
                JsxAttrValue::Expr(_) | JsxAttrValue::Bool(_) => {}
            }
            continue;
        }

        // @event={handler}
        if let Some(event_part) = key.strip_prefix('@') {
            let event = event_part.split('|').next().unwrap_or("").to_string();
            let modifiers = event_part
                .split('|')
                .skip(1)
                .map(|s| s.to_string())
                .collect();
            event_handlers.push(EventHandler {
                event,
                modifiers,
                handler: attr.as_expr().unwrap_or_default(),
            });
            continue;
        }

        // onEvent={handler} — React-style camelCase
        if key.starts_with("on") && key.len() > 2 {
            let rest = &key[2..];
            if rest.starts_with(|c: char| c.is_ascii_uppercase()) {
                let first = rest.chars().next().unwrap();
                let event = format!(
                    "{}{}",
                    first.to_ascii_lowercase(),
                    &rest[first.len_utf8()..]
                );
                event_handlers.push(EventHandler {
                    event,
                    modifiers: vec![],
                    handler: attr.as_expr().unwrap_or_default(),
                });
                continue;
            }
        }

        // animate:property={duration easing}
        if let Some(prop) = key.strip_prefix("animate:") {
            let val = attr.as_expr().unwrap_or_default();
            let parts: Vec<&str> = val.split_whitespace().collect();
            animations.push(AnimationSpec {
                property: prop.to_string(),
                duration_expr: parts.first().unwrap_or(&"300ms").to_string(),
                easing: parts.get(1).unwrap_or(&"linear").to_string(),
                repeat: parts.get(2).map(|s| *s == "repeat").unwrap_or(false),
            });
            continue;
        }

        // bind:prop={expr}
        if let Some(prop) = key.strip_prefix("bind:") {
            bindings.push(Binding {
                origin: crate::ast::BindingOrigin::Expression,
                prop: prop.to_string(),
                value: attr.as_expr().unwrap_or_default(),
            });
            continue;
        }

        // All other attributes with values → binding
        if let Some(value) = attr.as_expr() {
            bindings.push(Binding {
                origin: crate::ast::BindingOrigin::Expression,
                prop: key.clone(),
                value,
            });
        }
    }

    Element {
        tag: tag.to_string(),
        id,
        classes,
        conditional_classes,
        event_handlers,
        bindings,
        animations,
        children,
    }
}

pub(crate) fn jsx_build_include(attrs: Vec<JsxAttr>, slot: Vec<Node>) -> Node {
    let path = attrs
        .iter()
        .find(|a| matches!(a.key.as_str(), "src" | "path"))
        .and_then(|a| a.as_str())
        .unwrap_or("")
        .to_string();
    let props = attrs
        .iter()
        .filter(|a| !matches!(a.key.as_str(), "src" | "path"))
        .filter_map(|a| a.as_expr().map(|v| (a.key.clone(), v)))
        .collect();
    Node::Include(IncludeNode { path, props, slot })
}

pub(crate) fn jsx_build_embed(attrs: Vec<JsxAttr>) -> Node {
    let src = attrs
        .iter()
        .find(|a| matches!(a.key.as_str(), "src" | "path"))
        .and_then(|a| a.as_str())
        .unwrap_or("")
        .to_string();
    let adapter = attrs
        .iter()
        .find(|a| a.key == "adapter")
        .and_then(|a| a.as_str())
        .map(|s| s.to_string());
    let props = attrs
        .iter()
        .filter(|a| !matches!(a.key.as_str(), "src" | "path" | "adapter"))
        .filter_map(|a| a.as_expr().map(|v| (a.key.clone(), v)))
        .collect();
    Node::Embed(EmbedNode {
        src,
        adapter,
        props,
    })
}

pub(crate) fn jsx_build_let(attrs: Vec<JsxAttr>, is_default: bool) -> LetDecl {
    let name = attrs
        .iter()
        .find(|a| a.key == "name")
        .and_then(|a| a.as_str())
        .unwrap_or("")
        .to_string();
    let expr = attrs
        .iter()
        .find(|a| a.key == "value")
        .and_then(|a| a.as_expr())
        .unwrap_or_default();
    LetDecl {
        name,
        expr,
        is_default,
    }
}
