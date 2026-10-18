//! JSX AST node builders.

use crate::ast::*;

use super::attrs::{JsxAttr, JsxAttrValue};

/// Canonical DOM events shared by JSX aliases and Svelte 5 event attributes.
pub(crate) const DOM_EVENTS: &[&str] = &[
    "click",
    "dblclick",
    "input",
    "beforeinput",
    "change",
    "submit",
    "reset",
    "select",
    "focus",
    "focusin",
    "focusout",
    "blur",
    "keydown",
    "keyup",
    "keypress",
    "mousedown",
    "mouseup",
    "mousemove",
    "mouseenter",
    "mouseleave",
    "mouseover",
    "mouseout",
    "contextmenu",
    "wheel",
    "scroll",
    "toggle",
    "load",
    "error",
    "copy",
    "cut",
    "paste",
    "gotpointercapture",
    "lostpointercapture",
    "pointerdown",
    "pointerup",
    "pointermove",
    "pointerenter",
    "pointerleave",
    "touchstart",
    "touchend",
    "touchmove",
    "touchcancel",
    "dragstart",
    "dragend",
    "dragover",
    "dragenter",
    "dragleave",
    "drop",
    "animationend",
    "transitionend",
    "abort",
    "animationcancel",
    "animationiteration",
    "animationstart",
    "auxclick",
    "beforematch",
    "beforetoggle",
    "cancel",
    "canplay",
    "canplaythrough",
    "close",
    "compositionend",
    "compositionstart",
    "compositionupdate",
    "cuechange",
    "drag",
    "durationchange",
    "emptied",
    "encrypted",
    "ended",
    "fullscreenchange",
    "fullscreenerror",
    "invalid",
    "loadeddata",
    "loadedmetadata",
    "loadstart",
    "pause",
    "play",
    "playing",
    "pointercancel",
    "progress",
    "ratechange",
    "resize",
    "scrollend",
    "seeked",
    "seeking",
    "stalled",
    "suspend",
    "timeupdate",
    "transitioncancel",
    "transitionrun",
    "transitionstart",
    "volumechange",
    "waiting",
];

pub(crate) fn jsx_build_element(
    tag: &str,
    attrs: Vec<JsxAttr>,
    children: Vec<Node>,
) -> Result<Element, &'static str> {
    jsx_build_element_with_class_literals(tag, attrs, children, true)
}

pub(crate) fn jsx_build_element_with_class_literals(
    tag: &str,
    attrs: Vec<JsxAttr>,
    children: Vec<Node>,
    literal_classes: bool,
) -> Result<Element, &'static str> {
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
                    if literal_classes {
                        classes.extend(s.split_whitespace().map(super::super::literal_class_token));
                    } else {
                        classes.extend(super::super::indent::split_class_tokens(s));
                    }
                }
                JsxAttrValue::Expr(e) => {
                    if e.trim().is_empty() {
                        return Err("class attributes require a nonempty expression");
                    }
                    // Dynamic expression — keep as a single {expr} class token
                    classes.push(format!("{{{}}}", e));
                }
                JsxAttrValue::Bool(_) => {}
            }
            continue;
        }

        if key == "id" {
            if id.is_some()
                || bindings
                    .iter()
                    .any(|binding: &Binding| binding.prop == "id")
            {
                return Err("an element may have only one id attribute");
            }
            match &attr.value {
                JsxAttrValue::Str(value) => id = Some(value.clone()),
                JsxAttrValue::Expr(expression) => {
                    let expression = expression.trim();
                    if expression.is_empty() {
                        return Err("id attributes require a nonempty expression");
                    }
                    if let Some(crate::context::TemplateValue::Str(value)) =
                        crate::eval::eval_literal(expression)
                    {
                        id = Some(value);
                    } else {
                        bindings.push(Binding {
                            origin: crate::ast::BindingOrigin::Expression,
                            prop: "id".to_string(),
                            value: expression.to_string(),
                        });
                    }
                }
                JsxAttrValue::Bool(_) => {
                    return Err("id attributes require a literal or expression")
                }
            }
            continue;
        }

        // class:name={condition}
        if let Some(class_name) = key.strip_prefix("class:") {
            if class_name.is_empty() {
                return Err("conditional class directives require a class name");
            }
            let condition = attr.as_expr().unwrap_or_default();
            if condition.trim().is_empty() {
                return Err("conditional class directives require a nonempty condition");
            }
            conditional_classes.push(ConditionalClass {
                class: class_name.to_string(),
                condition,
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
                return Err("when directives require a nonempty condition");
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
                JsxAttrValue::Expr(_) | JsxAttrValue::Bool(_) => {
                    return Err("when directives require a literal class list");
                }
            }
            continue;
        }

        // @event={handler}
        if let Some(event_part) = key.strip_prefix('@') {
            let event = event_part.split('|').next().unwrap_or("").to_string();
            if event.is_empty() {
                return Err("event directives require an event name");
            }
            let modifiers: Vec<String> = event_part
                .split('|')
                .skip(1)
                .map(|s| s.to_string())
                .collect();
            if modifiers.iter().any(|modifier| modifier.trim().is_empty()) {
                return Err("event directives require nonempty modifier entries");
            }
            event_handlers.push(EventHandler {
                model_before_listener: None,
                event,
                modifiers,
                handler: attr.event_handler()?,
            });
            continue;
        }

        // onEvent={handler} — React-style camelCase
        if key.starts_with("on") && key.len() > 2 {
            let rest = &key[2..];
            if rest.starts_with(|c: char| c.is_ascii_uppercase()) {
                let canonical = |name: &str| {
                    let lower = name.to_ascii_lowercase();
                    if lower == "doubleclick" {
                        "dblclick".to_string()
                    } else {
                        lower
                    }
                };
                let lower = canonical(rest);
                let (event, modifiers) = if DOM_EVENTS.contains(&lower.as_str()) {
                    (lower, vec![])
                } else if let Some(base) = rest
                    .strip_suffix("Capture")
                    .map(canonical)
                    .filter(|base| DOM_EVENTS.contains(&base.as_str()))
                {
                    (base, vec!["capture".to_string()])
                } else {
                    let first = rest.chars().next().unwrap();
                    (
                        format!(
                            "{}{}",
                            first.to_ascii_lowercase(),
                            &rest[first.len_utf8()..]
                        ),
                        vec![],
                    )
                };
                event_handlers.push(EventHandler {
                    model_before_listener: None,
                    event,
                    modifiers,
                    handler: attr.event_handler()?,
                });
                continue;
            }
        }

        // animate:property={duration easing}
        if let Some(prop) = key.strip_prefix("animate:") {
            if prop.is_empty() {
                return Err("animation directives require a property name");
            }
            let val = match &attr.value {
                JsxAttrValue::Str(value) | JsxAttrValue::Expr(value) => value.clone(),
                JsxAttrValue::Bool(_) => attr.as_expr().unwrap_or_default(),
            };
            let parts: Vec<&str> = val.split_whitespace().collect();
            if parts.len() > 3 || parts.get(2).is_some_and(|token| *token != "repeat") {
                return Err("animation settings accept duration, easing, and optional repeat only");
            }
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
            if prop.is_empty() {
                return Err("binding directives require a property name");
            }
            if prop == "id" && (id.is_some() || bindings.iter().any(|binding| binding.prop == "id"))
            {
                return Err("an element may have only one id attribute");
            }
            let value = attr.as_expr().unwrap_or_default();
            if value.trim().is_empty() {
                return Err("binding directives require a nonempty expression");
            }
            bindings.push(Binding {
                origin: crate::ast::BindingOrigin::Expression,
                prop: prop.to_string(),
                value,
            });
            continue;
        }

        // All other attributes with values → binding
        if let Some(value) = attr.as_expr() {
            if value.trim().is_empty() {
                return Err("attribute bindings require a nonempty expression");
            }
            bindings.push(Binding {
                origin: crate::ast::BindingOrigin::Expression,
                prop: key.clone(),
                value,
            });
        }
    }

    Ok(Element {
        tag: tag.to_string(),
        id,
        classes,
        conditional_classes,
        event_handlers,
        bindings,
        animations,
        children,
    })
}

pub(crate) fn jsx_validate_source_attrs(tag: &str, attrs: &[JsxAttr]) -> Result<(), String> {
    for (index, attribute) in attrs.iter().enumerate() {
        if attrs[..index]
            .iter()
            .any(|previous| previous.key == attribute.key)
        {
            return Err(format!("duplicate directive attribute {:?}", attribute.key));
        }
    }
    let mut sources = attrs
        .iter()
        .filter(|attribute| matches!(attribute.key.as_str(), "src" | "path"));
    let source = sources.next().and_then(|attribute| attribute.as_str());
    if sources.next().is_some() || source.is_none_or(|path| path.trim().is_empty()) {
        return Err(format!(
            "<{tag}> requires exactly one literal source path (src or path)"
        ));
    }
    Ok(())
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
