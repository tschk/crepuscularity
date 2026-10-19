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

fn native_model_argument(name: &str) -> Option<String> {
    let mut parts = name.split('-');
    let first = parts.next()?;
    let valid = |value: &str| {
        !value.is_empty()
            && value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$'))
    };
    if !valid(first)
        || !first.starts_with(|ch: char| ch.is_ascii_alphabetic() || matches!(ch, '_' | '$'))
    {
        return None;
    }
    let mut result = first.to_owned();
    for part in parts {
        if !valid(part) || part.starts_with('$') {
            return None;
        }
        let mut chars = part.chars();
        result.push(chars.next()?.to_ascii_uppercase());
        result.extend(chars);
    }
    Some(result)
}
fn model_attribute_name(key: &str) -> Option<String> {
    let head = key.split('.').next()?;
    if head == "v-model" {
        Some("modelValue".into())
    } else {
        native_model_argument(head.strip_prefix("v-model:")?)
    }
}

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

fn literal_expr(attr: &JsxAttr, native_props: bool) -> String {
    match &attr.value {
        JsxAttrValue::Str(s) if !native_props => super::super::quote_expression_string(
            &super::super::svelte_entities::decode_html_entities(s, true),
        ),
        JsxAttrValue::Str(s) => super::super::quote_expression_string(s),
        JsxAttrValue::Expr(e) => e.clone(),
        JsxAttrValue::Bool(b) => b.to_string(),
    }
}

pub(crate) fn vue_build_element(
    tag: &str,
    attrs: Vec<JsxAttr>,
    children: Vec<Node>,
) -> Result<VueElement, RawParseError> {
    vue_build_element_with_mode(tag, attrs, children, false)
}

pub(crate) fn vue_build_element_with_mode(
    tag: &str,
    attrs: Vec<JsxAttr>,
    children: Vec<Node>,
    native_props: bool,
) -> Result<VueElement, RawParseError> {
    if attrs
        .iter()
        .filter(|attribute| matches!(attribute.key.as_str(), "id" | ":id" | "v-bind:id"))
        .take(2)
        .count()
        > 1
    {
        return Err(unsupported(
            "an element may have only one id attribute or binding",
        ));
    }
    if native_props {
        for name in ["class", "style"] {
            if attrs
                .iter()
                .filter(|attr| {
                    attr.key == name
                        || attr.key == format!(":{name}")
                        || attr.key == format!("v-bind:{name}")
                })
                .take(2)
                .count()
                > 1
            {
                return Err(unsupported(
                    "native Vue combined class/style attributes are unsupported",
                ));
            }
        }
    }
    let mut id = None;
    let mut classes: Vec<String> = Vec::new();
    let mut conditional_classes: Vec<ConditionalClass> = Vec::new();
    let mut event_handlers: Vec<EventHandler> = Vec::new();
    let mut bindings: Vec<Binding> = Vec::new();
    let mut branch = None;
    let mut for_spec = None;
    let mut replacement_children = None;

    for (position, attr) in attrs.iter().enumerate() {
        let key = attr.key.as_str();

        if matches!(key, "v-if" | "v-else-if" | "v-else") && branch.is_some() {
            return Err(unsupported(
                "an element may have only one branch directive (v-if, v-else-if, or v-else)",
            ));
        }

        if native_props && matches!(key, "class" | "style") {
            let JsxAttrValue::Str(value) = &attr.value else {
                return Err(unsupported(
                    "native Vue class/style attributes require literals",
                ));
            };
            let value = if key == "class" {
                value
                    .split([' ', '\t', '\r', '\n', '\u{c}'])
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim_matches(|ch| matches!(ch, '\t'..='\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'))
                    .to_owned()
            } else {
                value.clone()
            };
            bindings.push(Binding {
                origin: BindingOrigin::Literal,
                prop: key.to_owned(),
                value: super::super::quote_expression_string(&value),
            });
            continue;
        }
        if key == "class" {
            if matches!(&attr.value, JsxAttrValue::Expr(_)) {
                return Err(unsupported(
                    "class expressions are not supported here; use `:class=\"…\"` instead",
                ));
            }
            if let JsxAttrValue::Str(s) = &attr.value {
                classes.extend(s.split_whitespace().map(super::super::literal_class_token));
            }
            continue;
        }
        if key == "id" {
            if let JsxAttrValue::Str(s) = &attr.value {
                id = Some(s.clone());
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
        if matches!(key, "v-if" | "v-else-if") {
            let expr = directive_expr(attr);
            if expr.is_empty() {
                return Err(unsupported(format!("{key} requires an expression")));
            }
            branch = Some(if key == "v-if" {
                VueBranch::If(expr)
            } else {
                VueBranch::ElseIf(expr)
            });
            continue;
        }
        if key == "v-else" {
            branch = Some(VueBranch::Else);
            continue;
        }
        if key == "v-for" {
            if for_spec.is_some() {
                return Err(unsupported("an element may have only one v-for directive"));
            }
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
        if matches!(key, "v-html" | "v-text") {
            if replacement_children.is_some() {
                return Err(unsupported(
                    "an element may have only one content directive (v-text or v-html)",
                ));
            }
            let expr = directive_expr(attr);
            if expr.is_empty() {
                return Err(unsupported(format!("{key} requires an expression")));
            }
            replacement_children = Some(vec![if key == "v-html" {
                Node::RawHtml(expr)
            } else {
                Node::Text(vec![TextPart::Expr(expr)])
            }]);
            continue;
        }
        if key == "v-model" || key.starts_with("v-model.") || key.starts_with("v-model:") {
            let named = key.starts_with("v-model:");
            if named && !native_props {
                return Err(unsupported("named models require native source mode"));
            }
            let name = model_attribute_name(key)
                .ok_or_else(|| unsupported("native Vue models require a static ASCII argument"))?;
            let component = native_props && tag.chars().next().is_some_and(char::is_uppercase);
            let mut modifiers: Vec<&str> = Vec::new();
            for modifier in key.split('.').skip(1) {
                if component {
                    if modifier.is_empty() || modifier == "__proto__" {
                        return Err(unsupported(format!(
                            "unsupported v-model modifiers in {key:?}"
                        )));
                    }
                    if !modifiers.contains(&modifier) {
                        modifiers.push(modifier);
                    }
                } else if !native_props
                    || !matches!(modifier, "lazy" | "trim" | "number")
                    || modifiers.contains(&modifier)
                {
                    return Err(unsupported(format!(
                        "unsupported v-model modifiers in {key:?}"
                    )));
                } else {
                    modifiers.push(modifier);
                }
            }
            if !component {
                modifiers.sort_unstable();
            }
            let expr = directive_expr(attr);
            if expr.is_empty() {
                return Err(unsupported("v-model requires an expression"));
            }
            bindings.push(Binding {
                origin: crate::ast::BindingOrigin::Expression,
                // Native-only metadata; the shared View IR still accepts plain models only.
                prop: std::iter::once(if named {
                    format!("bind:{name}")
                } else {
                    "bind".into()
                })
                .chain(modifiers.into_iter().map(str::to_owned))
                .collect::<Vec<_>>()
                .join("."),
                value: expr,
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
            if arg.is_empty() || expr.is_empty() {
                return Err(unsupported(format!(
                    "{key} requires a property name and a nonempty expression"
                )));
            }
            if native_props && matches!(arg, "class" | "style") {
                bindings.push(Binding {
                    origin: BindingOrigin::Expression,
                    prop: arg.to_owned(),
                    value: expr,
                });
                continue;
            }
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
            if event.is_empty() {
                return Err(unsupported(format!("{key} requires an event name")));
            }
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
                model_before_listener: if native_props {
                    event
                        .strip_prefix("update:")
                        .and_then(native_model_argument)
                        .and_then(|name| {
                            attrs.iter().position(|attr| {
                                model_attribute_name(&attr.key).as_ref() == Some(&name)
                            })
                        })
                        .map(|model| model < position)
                } else {
                    None
                },
                event,
                modifiers,
                handler: attr.event_handler().map_err(unsupported)?,
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

        let value = literal_expr(attr, native_props);
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

/// `item in items`, `(item) in items`, `item of items`.
fn parse_v_for(src: &str) -> Result<(String, String), RawParseError> {
    let (lhs, iterator) = split_for_parts(src)
        .ok_or_else(|| unsupported(format!("could not parse v-for expression {src:?}")))?;
    let lhs = lhs.trim();
    let inner = lhs
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(lhs);
    if inner.contains(',') {
        return Err(unsupported(
            "v-for index and key bindings are not supported: the shared ForBlock binds only the item variable",
        ));
    }
    let pattern = inner.trim().to_string();
    if pattern.is_empty() {
        return Err(unsupported(format!(
            "v-for expression {src:?} has no item binding"
        )));
    }
    if !crate::eval::is_variable_name(&pattern) {
        return Err(unsupported("v-for requires a supported item variable name"));
    }
    Ok((pattern, iterator.trim().to_string()))
}

fn split_for_parts(src: &str) -> Option<(&str, &str)> {
    let pos = super::super::scan_top_level(src, |i, _| {
        src[i..].starts_with(" in ") || src[i..].starts_with(" of ")
    })?;
    Some((&src[..pos], &src[pos + 4..]))
}

/// `:class` supports plain expressions, object syntax and array syntax.
pub(crate) fn apply_dynamic_class(
    expr: &str,
    classes: &mut Vec<String>,
    conditional_classes: &mut Vec<ConditionalClass>,
) -> Result<(), RawParseError> {
    apply_dynamic_class_at_depth(expr, classes, conditional_classes, 0)
}

fn apply_dynamic_class_at_depth(
    expr: &str,
    classes: &mut Vec<String>,
    conditional_classes: &mut Vec<ConditionalClass>,
    depth: usize,
) -> Result<(), RawParseError> {
    if depth > 64 {
        return Err(unsupported(
            "maximum class-list nesting depth (64) exceeded",
        ));
    }
    let expr = expr.trim();
    if expr.is_empty() || matches!(expr, "false" | "null") {
        return Ok(());
    }
    if let Some(inner) = expr.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        let mut indices = std::collections::HashMap::<String, usize>::new();
        let mut properties: Vec<(String, &str)> = Vec::new();
        for entry in split_top_level(inner) {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let fields = split_top_level_on(entry, ':');
            if fields.len() < 2 {
                return Err(unsupported(format!(
                    ":class object entry {entry:?} is missing a value"
                )));
            }
            let colon = fields[0].len();
            let name = entry[..colon].trim();
            if name.starts_with('[') {
                return Err(unsupported(
                    "computed class object keys are not supported; use static keys",
                ));
            }
            let quoted = name.starts_with(['\'', '"']);
            let literal = crate::eval::eval_literal(name);
            let valid_key = if quoted {
                matches!(literal, Some(crate::context::TemplateValue::Str(_)))
            } else {
                // Bare keys admit identifiers, numeric literals and the existing
                // hyphenated utility extension. Other punctuation needs quoting.
                name.chars().all(|character| {
                    character.is_alphanumeric() || matches!(character, '_' | '$' | '-')
                }) || matches!(
                    literal,
                    Some(
                        crate::context::TemplateValue::Int(_)
                            | crate::context::TemplateValue::Float(_)
                    )
                )
            };
            if !valid_key {
                return Err(unsupported(format!(
                    "invalid static class object key {name:?}; use one quoted literal or a bare identifier, number, or hyphenated utility"
                )));
            }
            let name = if let Some(literal) = name
                .strip_prefix('"')
                .and_then(|name| name.strip_suffix('"'))
                .or_else(|| {
                    name.strip_prefix('\'')
                        .and_then(|name| name.strip_suffix('\''))
                }) {
                super::super::unescape_crepus_text_literal(literal)
            } else {
                name.to_string()
            };
            let condition = entry[colon + 1..].trim();
            if name.is_empty() || condition.is_empty() {
                return Err(unsupported(format!(
                    ":class object entry {entry:?} is invalid"
                )));
            }
            if let Some(&index) = indices.get(&name) {
                properties[index].1 = condition;
            } else {
                indices.insert(name.clone(), properties.len());
                properties.push((name, condition));
            }
        }
        for (name, condition) in properties {
            for class in name.split_whitespace() {
                conditional_classes.push(ConditionalClass {
                    class: super::super::literal_class_token(class),
                    condition: condition.to_string(),
                });
            }
        }
        return Ok(());
    }
    if let Some(inner) = expr.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        for entry in split_top_level(inner) {
            let entry = entry.trim();
            if entry.is_empty() || matches!(entry, "false" | "null") {
                continue;
            }
            let literal = matches!(
                crate::eval::eval_literal(entry),
                Some(crate::context::TemplateValue::Str(_))
            );
            if entry.starts_with(['\'', '"'])
                && !literal
                && crate::eval::validate_expr(entry).is_err()
            {
                return Err(unsupported(
                    "class-list string or expression is malformed; check closing quotes",
                ));
            }
            if entry.starts_with('{') || entry.starts_with('[') {
                apply_dynamic_class_at_depth(entry, classes, conditional_classes, depth + 1)?;
            } else if literal {
                classes.extend(
                    super::super::unescape_crepus_text_literal(&entry[1..entry.len() - 1])
                        .split_whitespace()
                        .map(super::super::literal_class_token),
                );
            } else {
                classes.push(format!("{{{entry}}}"));
            }
        }
        return Ok(());
    }
    if matches!(
        crate::eval::eval_literal(expr),
        Some(crate::context::TemplateValue::Str(_))
    ) {
        classes.extend(
            super::super::unescape_crepus_text_literal(&expr[1..expr.len() - 1])
                .split_whitespace()
                .map(super::super::literal_class_token),
        );
        return Ok(());
    }
    classes.push(format!("{{{expr}}}"));
    Ok(())
}

/// Split on commas that are not nested inside brackets, braces or quotes.
pub(crate) fn split_top_level(src: &str) -> Vec<&str> {
    split_top_level_on(src, ',')
}

fn split_top_level_on(src: &str, separator: char) -> Vec<&str> {
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
            delimiter if delimiter == separator && depth == 0 => {
                parts.push(&src[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&src[start..]);
    parts
}
