//! Preserve authored Svelte text coercion and single-value caching semantics.
use crate::{
    svelte_entities::{decode_entities, decode_static_dom},
    MoonshineModuleTarget,
};
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::collections::{HashMap, HashSet};

fn text_part<'a>(item: &'a ArrayExpressionElement<'_>) -> Option<(oxc_span::Span, &'a str)> {
    match item {
        ArrayExpressionElement::StringLiteral(value) => Some((value.span, value.value.as_str())),
        ArrayExpressionElement::ObjectExpression(object) if object.properties.len() == 1 => {
            let ObjectPropertyKind::ObjectProperty(property) = &object.properties[0] else {
                return None;
            };
            if property.computed
                || property.shorthand
                || !matches!(&property.key, PropertyKey::StaticIdentifier(name) if name.name == "text")
            {
                return None;
            }
            let Expression::StringLiteral(value) = &property.value else {
                return None;
            };
            Some((object.span(), value.value.as_str()))
        }
        _ => None,
    }
}

#[derive(Default)]
struct Text {
    raw: String,
    data: String,
}

enum FragmentNode {
    Text(u32, Text),
    Expression,
    Other,
}

fn whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\n')
}

fn collapse_start(value: &mut String, replacement: &str) {
    let trimmed = value.trim_start_matches(whitespace);
    if trimmed.len() != value.len() {
        *value = format!("{replacement}{trimmed}");
    }
}

fn collapse_end(value: &mut String, replacement: &str) {
    let trimmed = value.trim_end_matches(whitespace);
    if trimmed.len() != value.len() {
        *value = format!("{trimmed}{replacement}");
    }
}

fn clean_fragment(nodes: &mut [FragmentNode], remove_space: bool) {
    let empty = |node: &FragmentNode| matches!(node, FragmentNode::Text(_, text) if text.data.chars().all(whitespace));
    let start = nodes
        .iter()
        .position(|node| !empty(node))
        .unwrap_or(nodes.len());
    let end = nodes
        .iter()
        .rposition(|node| !empty(node))
        .map_or(start, |at| at + 1);
    for (index, node) in nodes.iter_mut().enumerate() {
        if index < start || index >= end {
            if let FragmentNode::Text(_, text) = node {
                *text = Text::default();
            }
        }
    }
    let nodes = &mut nodes[start..end];
    if let Some(FragmentNode::Text(_, text)) = nodes.first_mut() {
        collapse_start(&mut text.raw, "");
        collapse_start(&mut text.data, "");
    }
    if let Some(FragmentNode::Text(_, text)) = nodes.last_mut() {
        collapse_end(&mut text.raw, "");
        collapse_end(&mut text.data, "");
    }
    for i in 0..nodes.len() {
        let previous_expression = i > 0 && matches!(nodes[i - 1], FragmentNode::Expression);
        let previous_space = i > 0
            && matches!(&nodes[i - 1], FragmentNode::Text(_, text) if text.data.ends_with(whitespace));
        let next_expression = matches!(nodes.get(i + 1), Some(FragmentNode::Expression));
        if let FragmentNode::Text(_, text) = &mut nodes[i] {
            if !previous_expression {
                let replacement = if previous_space { "" } else { " " };
                collapse_start(&mut text.raw, replacement);
                collapse_start(&mut text.data, replacement);
            }
            if !next_expression {
                collapse_end(&mut text.raw, " ");
                collapse_end(&mut text.data, " ");
            }
        }
    }
    for node in nodes {
        if let FragmentNode::Text(_, text) = node {
            if text.data.is_empty() || (remove_space && text.data == " ") {
                *text = Text::default();
            }
        }
    }
}

fn fragment_text(
    semantic: &oxc_semantic::Semantic<'_>,
    marker: &str,
    hoisted: &[&str],
) -> HashMap<u32, Text> {
    let mut result = HashMap::new();
    for node in semantic.nodes().iter() {
        let (children, parent) = match node.kind() {
            AstKind::JSXElement(element) => {
                (&element.children, Some(&element.opening_element.name))
            }
            AstKind::JSXFragment(fragment) => (&fragment.children, None),
            _ => continue,
        };
        let mut names: Vec<_> = semantic
            .nodes()
            .ancestors(node.id())
            .filter_map(|ancestor| {
                if let AstKind::JSXElement(element) = ancestor.kind() {
                    Some(&element.opening_element.name)
                } else {
                    None
                }
            })
            .collect();
        names.extend(parent);
        let preserve = names.iter().any(|name| {
            matches!(name,
            JSXElementName::Identifier(name) if matches!(name.name.as_str(), "pre" | "textarea"))
        });
        let pre = matches!(parent, Some(JSXElementName::Identifier(name)) if name.name == "pre");
        if preserve && !pre {
            continue;
        }
        let remove_space = matches!(parent,
            Some(JSXElementName::Identifier(name)) if matches!(name.name.as_str(),
            "select" | "tr" | "table" | "tbody" | "thead" | "tfoot" | "colgroup" | "datalist"));
        let mut nodes = Vec::new();
        for child in children {
            let JSXChild::Element(element) = child else {
                nodes.push(FragmentNode::Other);
                continue;
            };
            if matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if hoisted.contains(&name.name.as_str()))
            {
                continue;
            }
            if !matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if name.name == marker)
            {
                nodes.push(FragmentNode::Other);
                continue;
            }
            for attribute in &element.opening_element.attributes {
                let JSXAttributeItem::Attribute(attribute) = attribute else {
                    continue;
                };
                if !matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "parts")
                {
                    continue;
                }
                let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value
                else {
                    continue;
                };
                let Some(Expression::ArrayExpression(array)) = container.expression.as_expression()
                else {
                    continue;
                };
                for item in &array.elements {
                    nodes.push(if let Some((span, raw)) = text_part(item) {
                        FragmentNode::Text(
                            span.start,
                            Text {
                                raw: raw.to_owned(),
                                data: if matches!(item, ArrayExpressionElement::StringLiteral(_)) {
                                    raw.to_owned()
                                } else {
                                    decode_entities(raw, false)
                                },
                            },
                        )
                    } else {
                        FragmentNode::Expression
                    });
                }
            }
        }
        if preserve {
            if let Some(FragmentNode::Text(span, text)) = nodes.first() {
                if matches!(text.data.as_str(), "\n" | "\r\n") {
                    result.insert(*span, Text::default());
                }
            }
            continue;
        }
        clean_fragment(&mut nodes, remove_space);
        for node in nodes {
            if let FragmentNode::Text(span, text) = node {
                result.insert(span, text);
            }
        }
    }
    result
}

fn decode_parts(
    source: &str,
    marker: &str,
    hoisted: &[&str],
    normalize_text: bool,
    value_marker: Option<&str>,
    target: MoonshineModuleTarget,
) -> Result<(String, bool, Option<String>, bool), String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid authored Svelte text syntax".into());
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    let cleaned = if normalize_text {
        fragment_text(&built.semantic, marker, hoisted)
    } else {
        HashMap::new()
    };
    let mut edits = Vec::new();
    let mut dynamic = false;
    let mut has_calls = false;
    let mut anchor = "__crepusSveltePreAnchor".to_owned();
    while source.contains(&anchor) {
        anchor.push('_');
    }
    let mut anchor_used = false;
    let mut default_helper = "__crepusTextareaUpdate".to_owned();
    while source.contains(&default_helper) {
        default_helper.push('_');
    }
    let mut default_runtime = "__crepusTextareaUpdateEffect".to_owned();
    while source.contains(&default_runtime) {
        default_runtime.push('_');
    }
    let mut default_used = false;
    let mut ssr_value = "__crepusTextareaSsrValue".to_owned();
    while source.contains(&ssr_value) {
        ssr_value.push('_');
    }
    let mut ssr_value_used = false;
    for node in built.semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name,JSXElementName::IdentifierReference(name) if name.name==marker)
        {
            continue;
        }
        has_calls |= element.opening_element.attributes.iter().any(|item| {
            matches!(item,
            JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,
                JSXAttributeName::Identifier(name) if name.name == "calls"))
        });
        let textarea = matches!(built.semantic.nodes().parent_kind(node.id()), AstKind::JSXElement(parent) if matches!(&parent.opening_element.name, JSXElementName::Identifier(name) if name.name == "textarea"));
        let first_pre_text = matches!(built.semantic.nodes().parent_kind(node.id()), AstKind::JSXElement(parent)
            if matches!(&parent.opening_element.name, JSXElementName::Identifier(name) if name.name == "pre")
                && matches!(parent.children.first(), Some(JSXChild::Element(first)) if first.span == element.span));
        let literal = element.opening_element.attributes.iter().any(|item| matches!(item, JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "literal")));
        let mut raw_body = None;
        let mut leading_stripped = false;
        let mut restore_cr = false;
        let mut leading_prefix = "";
        if textarea {
            for item in &element.opening_element.attributes {
                if let JSXAttributeItem::Attribute(attribute) = item {
                    if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "leadingNewlineStripped")
                    {
                        if let Some(JSXAttributeValue::ExpressionContainer(container)) =
                            &attribute.value
                        {
                            if let Some(Expression::StringLiteral(value)) =
                                container.expression.as_expression()
                            {
                                leading_prefix = value.value.as_str();
                                leading_stripped =
                                    literal || matches!(value.value.as_str(), "\n" | "\r\n");
                                restore_cr = !literal && value.value == "\r";
                            }
                        }
                        edits.push((
                            attribute.span.start as usize,
                            attribute.span.end as usize,
                            String::new(),
                        ));
                    }
                }
            }
        }
        let mut retained_newline = false;
        let mut literal_text = String::new();
        let mut literal_raw = String::new();
        for item in &element.opening_element.attributes {
            let JSXAttributeItem::Attribute(attribute) = item else {
                continue;
            };
            if !matches!(&attribute.name,JSXAttributeName::Identifier(name) if name.name=="parts") {
                continue;
            }
            let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
                continue;
            };
            let Some(Expression::ArrayExpression(array)) = container.expression.as_expression()
            else {
                continue;
            };
            // Normalize the restored literal CR separately from a leading expression.
            let restored_prefix = restore_cr
                && !matches!(
                    array.elements.first(),
                    Some(
                        ArrayExpressionElement::StringLiteral(_)
                            | ArrayExpressionElement::ObjectExpression(_)
                    )
                );
            if restored_prefix {
                let at = array.span.start as usize + 1;
                edits.push((at, at, "\"\\n\",".into()));
            }
            let mut part_values = HashMap::new();
            let mut removed_text = false;
            for (index, item) in array.elements.iter().enumerate() {
                let Some((span, raw)) = text_part(item) else {
                    continue;
                };
                let entity = matches!(item, ArrayExpressionElement::ObjectExpression(_));
                let clean = cleaned.get(&span.start);
                let raw = clean.map_or(raw, |text| text.raw.as_str());
                if literal && matches!(target, MoonshineModuleTarget::Dom) {
                    literal_raw.push_str(raw);
                    continue;
                }
                let mut decoded = if let Some(clean) = clean {
                    clean.data.clone()
                } else if entity {
                    decode_entities(raw, textarea)
                } else {
                    raw.to_owned()
                };
                if textarea && index == 0 {
                    if restore_cr {
                        decoded.insert(0, '\r');
                    }
                    if !literal && !leading_stripped {
                        if let Some(rest) = decoded
                            .strip_prefix("\r\n")
                            .or_else(|| decoded.strip_prefix('\n'))
                        {
                            decoded = rest.to_owned();
                        }
                    }
                    retained_newline = decoded.starts_with('\n') || decoded.starts_with("\r\n");
                }
                // Decide prefix retention before normalizing literal CRs, independently of expressions.
                decoded = decoded.replace("\r\n", "\n").replace('\r', "\n");
                if textarea && literal && index == 0 && !leading_stripped {
                    if let Some(rest) = decoded.strip_prefix('\n') {
                        decoded = rest.to_owned();
                    }
                }
                literal_text.push_str(&decoded);
                if !literal {
                    removed_text |= decoded.is_empty();
                    part_values.insert(
                        span.start,
                        (!decoded.is_empty())
                            .then(|| serde_json::to_string(&decoded).expect("text serialization")),
                    );
                }
                if matches!(item, ArrayExpressionElement::StringLiteral(value) if value.value == decoded)
                {
                    continue;
                }
                edits.push((
                    span.start as usize,
                    span.end as usize,
                    serde_json::to_string(&decoded).expect("text serialization"),
                ));
            }
            if textarea
                && !literal
                && matches!(target, MoonshineModuleTarget::Dom)
                && !restored_prefix
            {
                let mut retained = array
                    .elements
                    .iter()
                    .filter(|item| !matches!(part_values.get(&item.span().start), Some(None)));
                if let (Some(ArrayExpressionElement::ArrowFunctionExpression(reader)), None) =
                    (retained.next(), retained.next())
                {
                    if let Some(expression) = reader.get_expression() {
                        let span = expression.span();
                        raw_body = Some(source[span.start as usize..span.end as usize].to_owned());
                    }
                }
            }
            if removed_text {
                let mut values = Vec::new();
                if restored_prefix {
                    values.push("\"\\n\"".to_owned());
                }
                for item in &array.elements {
                    let span = item.span();
                    match part_values.get(&span.start) {
                        Some(Some(value)) => values.push(value.clone()),
                        Some(None) => {}
                        None => {
                            values.push(source[span.start as usize..span.end as usize].to_owned())
                        }
                    }
                }
                edits.retain(|(start, end, _)| {
                    !(*start >= array.span.start as usize && *end <= array.span.end as usize)
                });
                edits.push((
                    array.span.start as usize,
                    array.span.end as usize,
                    format!("[{}]", values.join(",")),
                ));
            }
        }
        if let Some(value) = raw_body {
            let AstKind::JSXElement(parent) = built.semantic.nodes().parent_kind(node.id()) else {
                unreachable!()
            };
            edits.retain(|(start, end, _)| {
                !(*start >= element.span.start as usize
                    && *end <= element.span.end as usize
                    && *start < element.span.end as usize
                    && *end > element.span.start as usize)
            });
            edits.push((
                element.span.start as usize,
                element.span.end as usize,
                String::new(),
            ));
            let parent_id = built.semantic.nodes().parent_id(node.id());
            let jsx = matches!(
                built.semantic.nodes().parent_kind(parent_id),
                AstKind::JSXElement(_) | AstKind::JSXFragment(_)
            );
            edits.push((
                parent.span.start as usize,
                parent.span.start as usize,
                format!("{}{default_helper}(", if jsx { "{" } else { "" }),
            ));
            edits.push((
                parent.span.end as usize,
                parent.span.end as usize,
                format!(",()=>({value}\n),true){}", if jsx { "}" } else { "" }),
            ));
            default_used = true;
            continue;
        }
        dynamic |= !literal;
        if literal {
            let mut preserve_default = false;
            if matches!(target, MoonshineModuleTarget::Dom) {
                let raw = format!("{leading_prefix}{literal_raw}");
                literal_text = decode_static_dom(&raw);
                if textarea || first_pre_text {
                    if let Some(rest) = literal_text.strip_prefix('\n') {
                        literal_text = rest.to_owned();
                    }
                }
                if textarea {
                    if let AstKind::JSXElement(parent) =
                        built.semantic.nodes().parent_kind(node.id())
                    {
                        let attribute = parent.opening_element.attributes.iter().find_map(|item| match item {
                            JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,
                                JSXAttributeName::Identifier(name) if name.name == "defaultValue"
                                    || (name.name.eq_ignore_ascii_case("defaultvalue") && matches!(attribute.value, Some(JSXAttributeValue::ExpressionContainer(_))))) => Some(attribute),
                            _ => None,
                        });
                        if let Some(attribute) = attribute {
                            // A static body establishes the live value before Svelte sets its reset default.
                            let at = parent.opening_element.span.end as usize - 1;
                            edits.push((
                                at,
                                at,
                                format!(
                                    " value={{{}}}",
                                    serde_json::to_string(&literal_text)
                                        .expect("literal serialization")
                                ),
                            ));
                            let value = match &attribute.value {
                                None => "true".to_owned(),
                                Some(JSXAttributeValue::StringLiteral(value)) => {
                                    serde_json::to_string(&decode_entities(&value.value, true))
                                        .expect("default serialization")
                                }
                                Some(JSXAttributeValue::ExpressionContainer(value)) => {
                                    let span = value.expression.span();
                                    source[span.start as usize..span.end as usize].to_owned()
                                }
                                _ => return Err("unsupported textarea reset default".into()),
                            };
                            edits.push((
                                attribute.span.start as usize,
                                attribute.span.end as usize,
                                String::new(),
                            ));
                            let parent_id = built.semantic.nodes().parent_id(node.id());
                            let jsx = matches!(
                                built.semantic.nodes().parent_kind(parent_id),
                                AstKind::JSXElement(_) | AstKind::JSXFragment(_)
                            );
                            edits.push((
                                parent.span.start as usize,
                                parent.span.start as usize,
                                format!("{}{default_helper}(", if jsx { "{" } else { "" }),
                            ));
                            edits.push((
                                parent.span.end as usize,
                                parent.span.end as usize,
                                format!(",()=>({value}\n)){}", if jsx { "}" } else { "" }),
                            ));
                            default_used = true;
                            preserve_default = true;
                        }
                    }
                }
            }
            edits.retain(|(start, end, _)| {
                // A sibling wrapper may insert at this marker's boundary.
                !(*start >= element.span.start as usize
                    && *end <= element.span.end as usize
                    && *start < element.span.end as usize
                    && *end > element.span.start as usize)
            });
            edits.push((
                element.span.start as usize,
                element.span.end as usize,
                if preserve_default || literal_text.is_empty() {
                    String::new()
                } else {
                    format!(
                        "{{{}}}",
                        serde_json::to_string(&literal_text).expect("literal serialization")
                    )
                },
            ));
            continue;
        }
        if textarea && matches!(target, MoonshineModuleTarget::Ssr) && !retained_newline {
            let at = element.opening_element.span.end as usize
                - if element.closing_element.is_none() {
                    2
                } else {
                    1
                };
            edits.push((at, at, " stripLeadingNewline".into()));
        }
    }
    for node in built.semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name, JSXElementName::Identifier(name) if name.name == "textarea")
        {
            continue;
        }
        let provenance = element.opening_element.attributes.iter().find_map(|item| match item {
            JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name, JSXAttributeName::Identifier(name) if Some(name.name.as_str()) == value_marker) => Some(attribute),
            _ => None,
        });
        let Some(provenance) = provenance else {
            continue;
        };
        edits.push((
            provenance.span.start as usize,
            provenance.span.end as usize,
            String::new(),
        ));
        if !matches!(target, MoonshineModuleTarget::Ssr) {
            continue;
        }
        let Some(JSXAttributeValue::ExpressionContainer(prefix)) = &provenance.value else {
            return Err("invalid textarea value provenance".into());
        };
        let Expression::StringLiteral(prefix) = prefix.expression.to_expression() else {
            return Err("invalid textarea value prefix".into());
        };
        let prefix = decode_entities(&prefix.value, true);
        // The host preserves leading newlines; Svelte only does so for literal LF/CRLF prefixes.
        if prefix.starts_with('\n') || prefix.starts_with("\r\n") {
            continue;
        }
        for item in &element.opening_element.attributes {
            let JSXAttributeItem::Attribute(attribute) = item else {
                continue;
            };
            if !matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "value")
            {
                continue;
            }
            if matches!(&attribute.value, Some(JSXAttributeValue::ExpressionContainer(value)) if matches!(value.expression, JSXExpression::BooleanLiteral(_) | JSXExpression::NumericLiteral(_) | JSXExpression::NullLiteral(_)))
            {
                continue;
            }
            let expression = match &attribute.value {
                Some(JSXAttributeValue::ExpressionContainer(value)) => {
                    let span = value.expression.span();
                    source[span.start as usize..span.end as usize].to_owned()
                }
                Some(JSXAttributeValue::StringLiteral(value)) => {
                    serde_json::to_string(&decode_entities(&value.value, true))
                        .expect("value serialization")
                }
                None => "true".to_owned(),
                _ => return Err("unsupported textarea value".into()),
            };
            edits.push((
                attribute.span.start as usize,
                attribute.span.end as usize,
                format!("value={{{ssr_value}(({expression}\n))}}"),
            ));
            ssr_value_used = true;
        }
    }
    if matches!(target, MoonshineModuleTarget::Dom) {
        for node in built.semantic.nodes().iter() {
            let AstKind::JSXElement(element) = node.kind() else {
                continue;
            };
            if !matches!(&element.opening_element.name, JSXElementName::Identifier(name) if name.name == "textarea")
            {
                continue;
            }
            for item in &element.opening_element.attributes {
                let JSXAttributeItem::Attribute(attribute) = item else {
                    continue;
                };
                if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "value")
                {
                    if let Some(JSXAttributeValue::ExpressionContainer(value)) = &attribute.value {
                        let span = value.expression.span();
                        let value = &source[span.start as usize..span.end as usize];
                        edits.push((
                            attribute.span.start as usize,
                            attribute.span.end as usize,
                            String::new(),
                        ));
                        let jsx = matches!(
                            built.semantic.nodes().parent_kind(node.id()),
                            AstKind::JSXElement(_) | AstKind::JSXFragment(_)
                        );
                        edits.push((
                            element.span.start as usize,
                            element.span.start as usize,
                            format!("{}{default_helper}(", if jsx { "{" } else { "" }),
                        ));
                        edits.push((
                            element.span.end as usize,
                            element.span.end as usize,
                            format!(",()=>({value}\n),true){}", if jsx { "}" } else { "" }),
                        ));
                        default_used = true;
                    }
                    continue;
                }
                if !matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name.eq_ignore_ascii_case("defaultvalue"))
                {
                    continue;
                }
                if edits.iter().any(|(start, end, text)| {
                    *start == attribute.span.start as usize
                        && *end == attribute.span.end as usize
                        && text.is_empty()
                }) {
                    continue;
                }
                let Some(JSXAttributeValue::ExpressionContainer(value)) = &attribute.value else {
                    continue;
                };
                let span = value.expression.span();
                let value = &source[span.start as usize..span.end as usize];
                // DOMString assignment preserves nullish spellings and rejects Symbols.
                edits.push((
                    span.start as usize,
                    span.end as usize,
                    format!("`${{({value}\n) as unknown}}`"),
                ));
                let name = attribute.name.span();
                edits.push((
                    name.start as usize,
                    name.end as usize,
                    "defaultValue".into(),
                ));
            }
        }
    }
    if matches!(target, MoonshineModuleTarget::Ssr) {
        for node in built.semantic.nodes().iter() {
            let AstKind::JSXElement(element) = node.kind() else {
                continue;
            };
            if !matches!(&element.opening_element.name, JSXElementName::Identifier(name) if name.name == "pre")
            {
                continue;
            }
            let first = element.children.iter().find(|child| {
                !edits.iter().any(|(start, end, text)| {
                    *start == child.span().start as usize
                        && *end == child.span().end as usize
                        && text.is_empty()
                })
            });
            let structural = match first {
                Some(JSXChild::Element(child)) => match &child.opening_element.name {
                    JSXElementName::IdentifierReference(name) => name.name != marker,
                    JSXElementName::MemberExpression(_) => true,
                    _ => false,
                },
                Some(JSXChild::ExpressionContainer(child)) => matches!(
                    child.expression.as_expression(),
                    Some(Expression::ConditionalExpression(_))
                ),
                _ => false,
            };
            if structural {
                // Svelte's structural anchors prevent HTML from consuming a leading pre newline.
                let at = element.opening_element.span.end as usize;
                edits.push((at, at, format!("{{{anchor}(\"<!---->\")}}")));
                anchor_used = true;
            }
        }
    }
    edits.sort_by_key(|edit| (edit.0, edit.1));
    let mut output = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        output.replace_range(start..end, &text);
    }
    if anchor_used {
        output =
            format!("import {{rawHTML as {anchor}}} from \"@tschk/moonshine-dom/ssr\";\n{output}");
    }
    if ssr_value_used {
        output = format!("{output}\nfunction {ssr_value}(value:unknown):string{{const text=value==null?\"\":typeof value===\"symbol\"?value.toString():[value].join(\"\");return text.replace(/^\\r\\n|^[\\r\\n]/,\"\");}}\n");
    }
    if default_used {
        output = format!(
            "import {{createRenderEffect as {default_runtime}}} from \"@tschk/moonshine-dom/reactive\";\n{output}\nfunction {default_helper}<T>(element:T,read:()=>unknown,live=false):T{{const node=element as T&{{value:unknown;defaultValue:unknown}};let previous:unknown;{default_runtime}(()=>{{const value=read();if(live){{const next=value??void 0;if(previous===next)return;previous=next;if(node.value!==value)node.value=value??\"\";}}else{{const previous=node.value;node.defaultValue=value;node.value=previous;}}}});return element;}}\n"
        );
    }
    Ok((
        output,
        dynamic,
        default_used.then_some(default_helper),
        has_calls,
    ))
}

#[cfg(test)]
pub(crate) fn transform(
    source: &str,
    marker: &str,
    value_marker: Option<&str>,
    target: MoonshineModuleTarget,
) -> Result<(String, Option<String>), String> {
    transform_named(source, marker, marker, value_marker, &[], true, target)
}

pub(crate) fn transform_named(
    source: &str,
    marker: &str,
    function_name: &str,
    value_marker: Option<&str>,
    hoisted: &[&str],
    normalize_text: bool,
    target: MoonshineModuleTarget,
) -> Result<(String, Option<String>), String> {
    let (source, dynamic, default_helper, has_calls) = decode_parts(
        source,
        marker,
        hoisted,
        normalize_text,
        value_marker,
        target,
    )?;
    if !dynamic {
        return Ok((source, default_helper));
    }
    // The source is already validated compiler IR. Reserve decoded names so
    // escaped authored references cannot capture generated consumer imports.
    let reserved = if matches!(target, MoonshineModuleTarget::Ssr) {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &source, SourceType::tsx()).parse();
        if !parsed.errors.is_empty() {
            return Err("invalid source text helper syntax".into());
        }
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        semantic
            .nodes()
            .iter()
            .filter_map(|node| match node.kind() {
                AstKind::BindingIdentifier(id) => Some(id.name.to_string()),
                AstKind::IdentifierReference(id) => Some(id.name.to_string()),
                _ => None,
            })
            .collect::<HashSet<_>>()
    } else {
        HashSet::new()
    };
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name) || reserved.contains(&name) {
            name.push('_');
        }
        name
    };
    let runtime = fresh("__crepusSvelteTextRuntime");
    let peek = fresh("__crepusSvelteTextConsumer");
    let enter = fresh("__crepusSvelteTextWithConsumer");
    Ok((
        match target {
            MoonshineModuleTarget::Dom => {
                let part_type = if has_calls {
                    "string|(()=>unknown)|{readonly value:unknown}"
                } else {
                    "string|(()=>unknown)"
                };
                let call_prop = if has_calls { ",calls?:boolean" } else { "" };
                let snapshot = if has_calls {
                    "  let callValues:unknown[]|undefined;\n  if(props.calls){\n   callValues=[];\n   for(const part of parts){if(typeof part==='object')callValues.push(part.value); }\n  }\n"
                } else {
                    ""
                };
                let call_index = if has_calls {
                    "   let callIndex=0;\n"
                } else {
                    ""
                };
                let part_value = if has_calls {
                    "typeof part==='function'?`${part()??''}`:typeof part==='object'?`${callValues![callIndex++]??''}`:part"
                } else {
                    "typeof part==='function'?`${part()??''}`:part"
                };
                format!(
                    r#"{source}
function {function_name}(props:{{parts:({part_type})[]{call_prop}}}):any{{
 const parts=props.parts;
 let previous:unknown='';
 let text='';
 const read=parts.length===1&&typeof parts[0]==='function'?parts[0]:undefined;
 return ()=>{{
{snapshot}  if(read){{
   let value:any=read()??'';
   if(typeof value==='object')value=`${{value}}`;
   if(previous!==value){{previous=value;text=`${{value}}`;}}
  }}else{{
   let value='';
{call_index}   for(const part of parts){{value+={part_value};}}
   if(previous!==value){{previous=value;text=value;}}
  }}
  return text;
 }};
}}
"#
                )
            }
            // Per-part joins preserve string-hint coercion without capturing authored String bindings.
            MoonshineModuleTarget::Ssr => format!(
                r#"import {{scopeSSRChild as {runtime}}} from "@tschk/moonshine-dom/ssr";
import {{peekComponentContext as {peek},withComponentConsumer as {enter}}} from "@tschk/moonshine-dom/component";
{source}
function {function_name}(props:{{parts:(string|(()=>unknown))[],stripLeadingNewline?:boolean}}):any{{
 const parts=props.parts;
 const consumer={peek}();
 const read=()=>{{const text=parts.map(part=>{{const value=typeof part==='function'?(part()??''):part;return typeof value==='symbol'?value.toString():[value].join('');}}).join('');return props.stripLeadingNewline?text.replace(/^\r\n|^[\r\n]/,''):text;}};
 return {runtime}(()=>consumer?{enter}(consumer,read):read());
}}
"#
            ),
        },
        default_helper,
    ))
}

/// Memoize only owned DOM text readers whose original body contains a call Svelte would
/// cache as a template-effect argument. Plain property/getter readers stay live and uncached.
/// Wrapped readers become calls, so repeated preparation skips them. Textarea-specific
/// readers are intentionally left unchanged (documented bounded gap).
pub(crate) fn prepare_call_readers(
    source: String,
    marker: &str,
    target: MoonshineModuleTarget,
    policy: crate::svelte_javascript::IrPolicy,
    fresh: &dyn Fn(&str) -> String,
) -> Result<String, String> {
    if marker.is_empty() || !matches!(target, MoonshineModuleTarget::Dom) {
        return Ok(source);
    }
    let (ranges, occupied, call_groups) = {
        let allocator = Allocator::default();
        let mut parsed = Parser::new(&allocator, &source, SourceType::tsx()).parse();
        if !parsed.errors.is_empty() {
            return Err("invalid authored Svelte text syntax".into());
        }
        policy.configure(&mut parsed.program);
        let built = SemanticBuilder::new().build(&parsed.program);
        let mut ranges = Vec::new();
        let mut call_groups = Vec::new();
        let mut occupied = HashSet::new();
        for node in built.semantic.nodes().iter() {
            match node.kind() {
                AstKind::BindingIdentifier(id) => {
                    occupied.insert(id.name.to_string());
                }
                AstKind::IdentifierReference(id) => {
                    occupied.insert(id.name.to_string());
                }
                AstKind::IdentifierName(id) => {
                    occupied.insert(id.name.to_string());
                }
                AstKind::JSXElement(element) if matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if name.name == marker) =>
                {
                    let textarea = matches!(built.semantic.nodes().parent_kind(node.id()), AstKind::JSXElement(parent) if matches!(&parent.opening_element.name, JSXElementName::Identifier(name) if name.name == "textarea"));
                    if textarea {
                        continue;
                    }
                    let initial_ranges = ranges.len();
                    for item in &element.opening_element.attributes {
                        let JSXAttributeItem::Attribute(attribute) = item else {
                            continue;
                        };
                        if !matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "parts")
                        {
                            continue;
                        }
                        let Some(JSXAttributeValue::ExpressionContainer(container)) =
                            &attribute.value
                        else {
                            continue;
                        };
                        let Some(Expression::ArrayExpression(array)) =
                            container.expression.as_expression()
                        else {
                            continue;
                        };
                        for part in &array.elements {
                            let ArrayExpressionElement::ArrowFunctionExpression(reader) = part
                            else {
                                continue;
                            };
                            if reader.r#async
                                || !reader.params.items.is_empty()
                                || reader.params.rest.is_some()
                            {
                                continue;
                            }
                            let Some(body) = reader.get_expression() else {
                                continue;
                            };
                            if crate::svelte_expression::metadata(body, &built.semantic).0 {
                                ranges.push((reader.span.start as usize, reader.span.end as usize));
                            }
                        }
                    }
                    if ranges.len() > initial_ranges
                        && !element.opening_element.attributes.iter().any(|item| {
                            matches!(item,
                            JSXAttributeItem::Attribute(attr) if matches!(&attr.name,
                                JSXAttributeName::Identifier(name) if name.name == "calls"))
                        })
                    {
                        call_groups.push(element.opening_element.name.span().end as usize);
                    }
                }
                _ => {}
            }
        }
        ranges.sort_unstable();
        (ranges, occupied, call_groups)
    };
    if ranges.is_empty() {
        return Ok(source);
    }
    let unique = |prefix: &str| {
        let mut name = fresh(prefix);
        while source.contains(&name) || occupied.iter().any(|id| id.starts_with(&name)) {
            name.push('_');
        }
        name
    };
    let helper = unique("__crepusSvelteTextReader");
    let derived = unique("__crepusSvelteTextMemo");
    let mut out = source;
    let mut edits = Vec::new();
    for (start, end) in ranges {
        edits.push((start, format!("{helper}(")));
        edits.push((end, ")".to_owned()));
    }
    for at in call_groups {
        edits.push((at, " calls".to_owned()));
    }
    edits.sort_by_key(|(at, _)| *at);
    for (at, text) in edits.into_iter().rev() {
        out.insert_str(at, &text);
    }
    out.push_str(&format!(
        "\nfunction {helper}<T>(read:()=>T):{{readonly value:T}}{{return {derived}(read);}}\n"
    ));
    out.insert_str(
        0,
        &format!("import {{createDerived as {derived}}} from \"@tschk/moonshine-svelte/runes\";\n"),
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    fn lower(source: &str, target: MoonshineModuleTarget) -> String {
        let out = transform(source, "TextParts", None, target).unwrap().0;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &out, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{out}", parsed.errors);
        out
    }

    #[test]
    fn static_textarea_body_keeps_live_value_separate_from_reset_default() {
        for (parts, metadata, expected) in [
            ("'BODY'", "", "BODY"),
            ("''", " leadingNewlineStripped={\"\\n\"}", ""),
        ] {
            let source = format!(
                "function C(){{return <textarea defaultValue={{readDefault()}}><TextParts literal parts={{[{parts}]}}{metadata}/></textarea>}}"
            );
            let out = lower(&source, MoonshineModuleTarget::Dom);
            let value = serde_json::to_string(expected).unwrap();
            assert!(out.contains(&format!("value={{{value}}}")), "{out}");
            assert!(out.contains("__crepusTextareaUpdate(<textarea"), "{out}");
            assert_eq!(out.matches("readDefault()").count(), 1);
            assert!(!out.contains("defaultValue={"));
            assert!(out.contains("node.defaultValue=value;node.value=previous;"));
            assert!(!out.contains("function TextParts"));
        }
        let out = lower(
            "function C(){return <textarea defaultValue={readDefault()}><TextParts parts={[()=>body]}/></textarea>}",
            MoonshineModuleTarget::Dom,
        );
        assert!(!out.contains("<TextParts"));
        assert!(out.contains("__crepusTextareaUpdateEffect"));
        assert_eq!(out.matches("readDefault()").count(), 1);
    }

    #[test]
    fn adjacent_textarea_wrappers_keep_their_closing_edits_across_formatting_text() {
        let source = "function C(){return <form><textarea defaultValue='A'><TextParts literal parts={['BODY']}/></textarea><TextParts literal parts={['\\n  ']}/><textarea defaultValue='B'><TextParts literal parts={['BODY']}/></textarea><TextParts literal parts={['\\n']}/></form>}";
        let out = lower(source, MoonshineModuleTarget::Dom);
        assert_eq!(
            out.matches("{__crepusTextareaUpdate(<textarea").count(),
            2,
            "{out}"
        );
        assert!(!out.contains("function TextParts"));
    }

    #[test]
    fn sole_textarea_body_expressions_keep_raw_values_after_empty_literal_cleanup() {
        for parts in ["()=>readBody()", "'',()=>readBody()", "()=>readBody(),''"] {
            let source = format!(
                "function C(){{return <textarea><TextParts parts={{[{parts}]}}/></textarea>}}"
            );
            let out = lower(&source, MoonshineModuleTarget::Dom);
            assert!(out.contains("__crepusTextareaUpdate(<textarea"), "{out}");
            assert!(out.contains("()=>(readBody()\n),true)"), "{out}");
            assert!(!out.contains("function TextParts"), "{out}");
        }
        let source = "function C(){return <p><TextParts parts={[()=>readBody()]}/></p>}";
        assert!(!lower(source, MoonshineModuleTarget::Dom).contains("TextareaUpdate"));
        let source="function C(){return <textarea><TextParts parts={['prefix',()=>readBody()]}/></textarea>}";
        assert!(!lower(source, MoonshineModuleTarget::Dom).contains("TextareaUpdate"));
    }

    #[test]
    fn explicit_textarea_value_expressions_use_raw_value_updates() {
        for source in [
            "function C(){return <textarea value={readValue()}/>}",
            "function C(){return <form><textarea value={readValue()}/><input value={other()}/></form>}",
        ] {
            let out = lower(source, MoonshineModuleTarget::Dom);
            assert!(out.contains("__crepusTextareaUpdate(<textarea"), "{out}");
            assert!(out.contains("()=>(readValue()\n),true)"), "{out}");
            assert!(!out.contains("value={readValue()}"), "{out}");
            assert!(out.contains("if(previous===next)return;"), "{out}");
            assert!(out.contains("if(node.value!==value)node.value=value??"), "{out}");
        }
        let source = "function C(){return <textarea value='literal'/>}";
        assert_eq!(lower(source, MoonshineModuleTarget::Dom), source);
        let source = "function C(){return <input value={readValue()}/>}";
        assert_eq!(lower(source, MoonshineModuleTarget::Dom), source);
    }

    #[test]
    fn textarea_default_expressions_follow_dom_string_assignment() {
        for key in ["defaultValue", "defaultvalue", "DEFAULTVALUE"] {
            let out = lower(
                &format!("function C(){{return <textarea {key}={{readDefault()}}/>}}"),
                MoonshineModuleTarget::Dom,
            );
            assert!(out.contains("defaultValue={"), "{out}");
            assert!(out.contains("`${(readDefault()\n) as unknown}`"), "{out}");
            assert_eq!(out.matches("readDefault()").count(), 1);
        }
        let source = "function C(){return <textarea defaultvalue='literal'/>}";
        assert_eq!(lower(source, MoonshineModuleTarget::Dom), source);
    }

    #[test]
    fn fragment_cleanup_uses_decoded_edges_and_expression_neighbors() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = lower("function C(){return <p><TextParts parts={[{text:'&#13;&#10;'},()=>value,'  tail  ']}/></p>}", target);
            assert!(out.contains("parts={[()=>value,\"  tail\"]}"), "{out}");
            let out = lower(
                "function C(){return <p><TextParts parts={['a  ','  ',()=>value,'  b']}/></p>}",
                target,
            );
            assert!(out.contains("parts={[\"a \",()=>value,\"  b\"]}"), "{out}");
        }
    }

    #[test]
    fn fragment_cleanup_collapses_structural_edges_but_keeps_non_ascii_whitespace() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = lower("function C(){return <p><TextParts literal parts={['  a  ']}/><b/><TextParts literal parts={['  b  ']}/></p>}", target);
            assert_eq!(out, "function C(){return <p>{\"a \"}<b/>{\" b\"}</p>}");
            let out = lower("function C(){return <p><TextParts literal parts={['\\u00a0\\u000b\\u000c\\u2028\\u2029\\ufeff']}/></p>}", target);
            assert!(out.contains('\u{a0}'), "{out}");
            assert!(out.contains("\\u000b\\f"), "{out}");
        }
    }

    #[test]
    fn fragment_cleanup_respects_preserved_and_table_contexts() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for tag in ["pre", "textarea"] {
                let source = format!("function C(){{return <{tag}><TextParts literal parts={{['  a  ']}}/></{tag}>}}");
                assert!(lower(&source, target).contains("{\"  a  \"}"));
            }
            let out = lower(
                "function C(){return <pre><TextParts literal parts={['\\n']}/><b/></pre>}",
                target,
            );
            assert_eq!(out, "function C(){return <pre><b/></pre>}");
            for tag in ["table", "select"] {
                let source = format!("function C(){{return <{tag}><b/><TextParts literal parts={{['  ']}}/><b/></{tag}>}}");
                assert!(!lower(&source, target).contains("{\" \"}"));
            }
        }
    }

    #[test]
    fn static_pre_dom_consumes_one_html_line_break_without_changing_ssr_or_later_text() {
        for (raw, dom) in [
            ("\\nplain", "plain"),
            ("\\n\\nplain", "\nplain"),
            ("\\r\\nplain", "plain"),
        ] {
            let source = format!(
                "function C(){{return <pre><TextParts literal parts={{['{raw}']}}/></pre>}}"
            );
            let out = lower(&source, MoonshineModuleTarget::Dom);
            let expected = serde_json::to_string(dom).unwrap();
            assert_eq!(
                out,
                format!("function C(){{return <pre>{{{expected}}}</pre>}}")
            );
            let out = lower(&source, MoonshineModuleTarget::Ssr);
            assert!(out.contains("{\"\\n"), "{out}");
        }
        let out = lower(
            "function C(){return <pre><b/><TextParts literal parts={['\\nplain']}/></pre>}",
            MoonshineModuleTarget::Dom,
        );
        assert!(out.contains("{\"\\nplain\"}"));
    }

    #[test]
    fn ssr_pre_structural_anchors_do_not_change_direct_expression_line_breaks() {
        for child in ["<Child/>", "<UI.Child/>", "{value ? <>body</> : null}"] {
            let source = format!("function C(){{return <pre>{child}</pre>}}");
            let out = lower(&source, MoonshineModuleTarget::Ssr);
            assert!(
                out.contains("<pre>{__crepusSveltePreAnchor(\"<!---->\")}"),
                "{out}"
            );
            assert!(!lower(&source, MoonshineModuleTarget::Dom).contains("PreAnchor"));
        }
        let out = lower(
            "function C(){return <pre><TextParts literal parts={['\\n']}/><Child/></pre>}",
            MoonshineModuleTarget::Ssr,
        );
        assert!(
            out.contains("<pre>{__crepusSveltePreAnchor(\"<!---->\")}<Child/></pre>"),
            "{out}"
        );
        let out = lower(
            "function C(){return <pre><TextParts parts={[()=>value]}/></pre>}",
            MoonshineModuleTarget::Ssr,
        );
        assert!(!out.contains("PreAnchor"));
        let out = transform(
            "function C(){return <pre><Child/></pre>}",
            "",
            None,
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert!(out.0.contains("PreAnchor"));
    }

    #[test]
    fn literal_markers_use_host_entity_rules_without_a_live_text_helper() {
        for (target, expected) in [
            (MoonshineModuleTarget::Dom, "≂\u{338}"),
            (MoonshineModuleTarget::Ssr, "≂"),
        ] {
            let out = lower("function C(){return <p><TextParts literal parts={[{text:'&NotEqualTilde;'}]}/></p>}", target);
            let expected = serde_json::to_string(expected).unwrap();
            assert_eq!(out, format!("function C(){{return <p>{{{expected}}}</p>}}"));
            assert!(!out.contains("function TextParts"));
        }
        for (target, expected) in [
            (MoonshineModuleTarget::Dom, "\r\n"),
            (MoonshineModuleTarget::Ssr, ""),
        ] {
            let out = lower("function C(){return <textarea><TextParts literal parts={[{text:'&#13;&#10;'}]}/></textarea>}", target);
            let expected = serde_json::to_string(expected).unwrap();
            assert_eq!(
                out,
                if expected == "\"\"" {
                    "function C(){return <textarea></textarea>}".to_owned()
                } else {
                    format!("function C(){{return <textarea>{{{expected}}}</textarea>}}")
                }
            );
        }
    }

    #[test]
    fn static_dom_rejoins_raw_chunks_while_ssr_decodes_before_joining() {
        let source =
            "function C(){return <p><TextParts literal parts={[{text:'&#1'},'28;']}/></p>}";
        assert!(lower(source, MoonshineModuleTarget::Dom).contains("€"));
        assert!(lower(source, MoonshineModuleTarget::Ssr).contains("\\u000128;"));
    }

    #[test]
    fn textarea_newlines_keep_literal_provenance_and_gate_ssr_stripping() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for (parts, metadata, expected, strip) in [
                ("{text:'&#10;prefix'},()=>value", "", "prefix", true),
                ("{text:'&#13;prefix'},()=>value", "", "\\nprefix", true),
                (
                    "'\\nretained',()=>value",
                    " leadingNewlineStripped={\"\\n\"}",
                    "\\nretained",
                    false,
                ),
                (
                    "'\\r\\nretained',()=>value",
                    " leadingNewlineStripped={\"\\r\\n\"}",
                    "\\nretained",
                    false,
                ),
                ("'raw\\rretained',()=>value", "", "raw\\nretained", true),
                (
                    "{text:'&#10;'},()=>value",
                    " leadingNewlineStripped={\"\\r\"}",
                    "parts={[()=>value]}",
                    true,
                ),
                (
                    "()=>value",
                    " leadingNewlineStripped={\"\\r\"}",
                    "parts={[\"\\n\",()=>value]}",
                    true,
                ),
                ("()=>value", "", "()=>value", true),
            ] {
                let source = format!("function C(){{return <textarea><TextParts parts={{[{parts}]}}{metadata}/></textarea>}}");
                let out = lower(&source, target);
                let expected = if matches!(target, MoonshineModuleTarget::Dom)
                    && matches!(expected, "parts={[()=>value]}" | "()=>value")
                {
                    "()=>(value\n),true)"
                } else {
                    expected
                };
                assert!(out.contains(expected), "{out}");
                assert!(!out.contains(" leadingNewlineStripped"), "{out}");
                assert_eq!(
                    out.contains(" stripLeadingNewline"),
                    strip && matches!(target, MoonshineModuleTarget::Ssr),
                    "{out}"
                );
            }
        }
        let source = "function C(){return <div><TextParts parts={['\\r',()=>value]}/></div>}";
        let out = lower(source, MoonshineModuleTarget::Ssr);
        assert!(out.contains("parts={[()=>value]}"));
        assert!(!out.contains(" stripLeadingNewline"));
    }

    #[test]
    fn dom_single_value_coerces_objects_before_raw_cache_comparison() {
        let out = lower(
            "function C(){return <TextParts parts={[()=>value]}/>}",
            MoonshineModuleTarget::Dom,
        );
        assert!(out.contains("const parts=props.parts;"));
        assert!(out.contains("let text=''"));
        assert!(out.contains("return text;"));
        assert!(!out.contains("globalThis"));
        assert!(out.contains("let previous:unknown=''"));
        assert!(out.contains("let value:any=read()??'';"));
        assert!(
            out.contains("if(typeof value==='object')value=`${value}`;\n   if(previous!==value)")
        );
        assert!(out.contains("previous=value;text=`${value}`"));
        assert!(!out.contains("value()"));
        assert!(out.contains("return ()=>{\n  if(read)"));
        assert!(!out.contains("createRenderEffect"));
        assert!(!out.contains("moonshine-dom/reactive"));
    }

    #[test]
    fn dom_snapshots_call_values_before_plain_readers() {
        let out = lower(
            "function C(){return <TextParts calls parts={[()=>first,{value:second}]}/>}",
            MoonshineModuleTarget::Dom,
        );
        let snapshots = out.find("callValues.push(part.value)").unwrap();
        let ordinary = out.find("let value:any=read()").unwrap();
        assert!(snapshots < ordinary, "{out}");
        assert!(out.contains("callValues![callIndex++]"), "{out}");
    }

    #[test]
    fn dom_mixed_values_read_and_coerce_in_part_order_before_comparing() {
        let out = lower(
            "function C(){return <TextParts parts={['a',()=>first,'b',()=>second]}/>}",
            MoonshineModuleTarget::Dom,
        );
        assert!(out.contains(
            "for(const part of parts){value+=typeof part==='function'?`${part()??''}`:part;}"
        ));
        assert!(out.contains("if(previous!==value){previous=value;text=value;}"));
        assert!(!out.contains("parts.map"));
        assert!(!out.contains("globalThis.String"));
    }

    #[test]
    fn ssr_defers_interleaved_read_and_string_coercion() {
        let out = lower(
            "function C(){return <TextParts parts={['a',()=>value]}/>}",
            MoonshineModuleTarget::Ssr,
        );
        assert!(out.contains("scopeSSRChild as __crepusSvelteTextRuntime"));
        assert!(out.contains("const parts=props.parts;"));
        assert!(out.contains(
            "const text=parts.map(part=>{const value=typeof part==='function'?(part()??''):part;return typeof value==='symbol'?value.toString():[value].join('');}).join('');"
        ));
        assert!(!out.contains("globalThis"));
        assert!(!out.contains("escapeHtml"));
        assert!(!out.contains("rawHTML"));
        assert!(!out.contains("defer as"));
        assert!(!out.contains("createTextNode"));
        assert!(!out.contains("createRenderEffect"));
    }

    #[test]
    fn ssr_text_captures_call_consumer_around_the_entire_deferred_read() {
        let out = lower(
            "function C(){return <TextParts parts={[()=>read(),()=>coerce()]}/>}",
            MoonshineModuleTarget::Ssr,
        );
        assert!(
            out.contains("const consumer=__crepusSvelteTextConsumer();"),
            "{out}"
        );
        assert!(out.contains("return __crepusSvelteTextRuntime(()=>consumer?__crepusSvelteTextWithConsumer(consumer,read):read());"), "{out}");
        assert!(!out.contains("runWithOwner"), "{out}");
        let dom = lower(
            "function C(){return <TextParts parts={[()=>read()]}/>}",
            MoonshineModuleTarget::Dom,
        );
        assert!(!dom.contains("peekComponentContext"), "{dom}");
        assert!(!dom.contains("withComponentConsumer"), "{dom}");
    }

    #[test]
    fn ssr_text_consumer_aliases_reserve_decoded_escaped_references() {
        let source = r"const \u005f\u005fcrepusSvelteTextConsumer=1; function C(){return <TextParts parts={[()=>\u005f\u005fcrepusSvelteTextWithConsumer]}/>};";
        let out = lower(source, MoonshineModuleTarget::Ssr);
        assert!(
            out.contains("peekComponentContext as __crepusSvelteTextConsumer_"),
            "{out}"
        );
        assert!(
            out.contains("withComponentConsumer as __crepusSvelteTextWithConsumer_"),
            "{out}"
        );
        assert!(out.contains(source), "{out}");
    }

    #[test]
    fn static_ssr_text_adds_no_consumer_imports() {
        let source = "function C(){return <p><TextParts literal parts={['static']}/></p>};";
        let out = lower(source, MoonshineModuleTarget::Ssr);
        assert!(!out.contains("peekComponentContext"), "{out}");
    }

    #[test]
    fn ssr_owned_plain_text_reads_each_part_once_and_handles_symbols_without_globals() {
        let source = "const String=0,globalThis=0;function C(){return <TextParts parts={['<a&',()=>first(),()=>symbol]}/>};";
        let out = lower(source, MoonshineModuleTarget::Ssr);
        assert!(out.contains(source));
        assert!(out.contains("const value=typeof part==='function'?(part()??''):part;"));
        assert_eq!(out.matches("part()").count(), 1);
        assert!(out.contains("typeof value==='symbol'?value.toString():[value].join('')"));
        assert!(!out.contains("String(value)"));
        assert!(!out.contains("globalThis.String"));
        assert!(!out.contains("rawHTML as"));
        assert!(!out.contains("escapeHtml as"));
    }

    #[test]
    fn authored_global_this_binding_does_not_capture_helper_globals() {
        let source="const globalThis={value:true};function C(){return <TextParts parts={[()=>globalThis.value]}/>};";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = lower(source, target);
            assert_eq!(out.matches("globalThis").count(), 2);
        }
        let out=lower("const __crepusSvelteTextEscape=0;const __crepusSvelteTextHTML=0;function C(){return <TextParts parts={[()=>value]}/>}",MoonshineModuleTarget::Ssr);
        assert!(out.contains("scopeSSRChild as __crepusSvelteTextRuntime"));
        assert!(!out.contains("as __crepusSvelteTextEscape"));
        assert!(!out.contains("as __crepusSvelteTextHTML"));
    }
    #[test]
    fn named_common_and_numeric_entities_decode_once_at_compile_time() {
        assert_eq!(
            decode_entities("&copy;&amp;&lt;&gt;&quot;&apos;&nbsp;", true),
            "©&<>\"'\u{a0}"
        );
        assert_eq!(decode_entities("&#65;&#x41;&#128512;", true), "AA😀");
        assert_eq!(
            decode_entities("&amp;copy; &amp;amp;", true),
            "&copy; &amp;"
        );
        assert_eq!(
            decode_entities("&unknown; &#x110000; &#xD800; &copy", true),
            "&unknown; \0 \0 ©"
        );
        assert_eq!(
            decode_entities("literal & prefix &copy;", true),
            "literal & prefix ©"
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out=lower("function C(){return <TextParts parts={[{text:'&copy; &amp;copy; &#65; &unknown;'},()=>value]}/>}",target);
            assert!(out.contains("parts={[\"© &copy; A &unknown;\",()=>value]}"));
            assert!(!out.contains("{text:"));
        }
    }
    #[test]
    fn only_direct_text_wrappers_inside_marker_parts_are_decoded() {
        let source="const raw={text:'&copy;'};function C(){return <><Other parts={[{text:'&copy;'}]}/><TextParts parts={['&copy;',()=>({text:'&copy;'}),{text:'&copy;',extra:true}]}/></>}";
        let out = lower(source, MoonshineModuleTarget::Dom);
        assert!(out.contains(source));
        assert!(transform(
            "const broken=;",
            "TextParts",
            None,
            MoonshineModuleTarget::Dom
        )
        .is_err());
    }
    #[test]
    fn runtime_alias_hygiene_and_source_preservation() {
        let source = "const __crepusSvelteTextRuntime=0;function C(){return <TextParts parts={[()=>value]}/>}; // keep source\n";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = lower(source, target);
            assert!(out.contains(source));
            if matches!(target, MoonshineModuleTarget::Ssr) {
                assert!(out.contains("as __crepusSvelteTextRuntime_"));
            } else {
                assert!(!out.contains("createRenderEffect"));
                assert!(!out.contains("moonshine-dom/reactive"));
            }
        }
    }

    fn prepare(source: &str, target: MoonshineModuleTarget) -> String {
        let out = prepare_call_readers(
            source.to_owned(),
            "TextParts",
            target,
            crate::svelte_javascript::IrPolicy::Typed,
            &|base| base.to_owned(),
        )
        .unwrap();
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &out, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{out}", parsed.errors);
        out
    }

    #[test]
    fn member_call_readers_memoize_but_plain_getters_stay_live() {
        let source = "let obj:any={};let tag:any=(s:any)=>s;function C(){return <TextParts parts={[()=>obj.method(),'-',()=>obj.name,()=>obj?.run?.(),()=>tag`x`]}/>}";
        let out = prepare(source, MoonshineModuleTarget::Dom);
        assert!(
            out.contains("__crepusSvelteTextReader(()=>obj.method())"),
            "{out}"
        );
        assert!(
            out.contains("__crepusSvelteTextReader(()=>obj?.run?.())"),
            "{out}"
        );
        assert!(
            out.contains("__crepusSvelteTextReader(()=>tag`x`)"),
            "{out}"
        );
        assert!(out.contains(",()=>obj.name,"), "{out}");
        assert!(!out.contains("Reader(()=>obj.name)"), "{out}");
        assert!(
            out.contains("createDerived as __crepusSvelteTextMemo"),
            "{out}"
        );
        let plain = "let obj:any={};function C(){return <TextParts parts={[()=>obj.name]}/>}";
        assert_eq!(prepare(plain, MoonshineModuleTarget::Dom), plain);
    }

    #[test]
    fn repeated_preparation_does_not_cache_twice() {
        let source = "let obj:any={};function C(){return <TextParts parts={[()=>obj.method()]}/>}";
        let once = prepare(source, MoonshineModuleTarget::Dom);
        assert_eq!(prepare(&once, MoonshineModuleTarget::Dom), once);
        assert_eq!(once.matches("function __crepusSvelteTextReader").count(), 1);
    }

    #[test]
    fn ssr_and_textarea_readers_are_unchanged() {
        let source = "let obj:any={};function C(){return <TextParts parts={[()=>obj.method()]}/>}";
        assert_eq!(prepare(source, MoonshineModuleTarget::Ssr), source);
        let textarea = "let obj:any={};function C(){return <textarea><TextParts parts={[()=>obj.method()]}/></textarea>}";
        assert_eq!(prepare(textarea, MoonshineModuleTarget::Dom), textarea);
    }

    #[test]
    fn helper_names_avoid_plain_and_decoded_bindings() {
        let source = "const __crepusSvelteTextRead\\u0065r=0;let obj:any={};function C(){return <TextParts parts={[()=>obj.method()]}/>}";
        let out = prepare(source, MoonshineModuleTarget::Dom);
        assert!(
            out.contains("function __crepusSvelteTextReader_<T>"),
            "{out}"
        );
        assert!(
            out.contains("__crepusSvelteTextReader_(()=>obj.method())"),
            "{out}"
        );
    }
}

#[cfg(test)]
mod head_whitespace_tests {
    use super::*;
    #[test]
    fn hoisted_head_does_not_keep_body_edge_whitespace() {
        let source = "function C(){return <><Text literal parts={[\"\\n\"]}/><Head><title>t</title></Head><Text literal parts={[\"\\n\"]}/><p>x</p></>}";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let (out, _) =
                transform_named(source, "Text", "Text", None, &["Head"], true, target).unwrap();
            assert!(!out.contains("{\" \"}"), "{out}");
            assert!(out.contains("<Head>"), "{out}");
        }
    }
}
