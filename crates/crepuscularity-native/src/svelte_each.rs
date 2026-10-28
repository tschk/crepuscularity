//! Authored each blocks retain their header until Oxc can validate bindings and keys.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType, Span};

#[derive(Debug, PartialEq)]
pub(crate) struct Header {
    pub iterable: String,
    pub item: String,
    pub object: bool,
    /// Authored type expression only; its colon is not part of this span.
    pub annotation: Option<String>,
    pub index: Option<String>,
    pub key: Option<String>,
}

// Svelte erases TS expression wrappers before classifying `(index)` as unkeyed.
fn index_identity_key(header: &Header) -> bool {
    let (Some(index), Some(key)) = (&header.index, &header.key) else {
        return false;
    };
    let allocator = Allocator::default();
    let wrapped = format!("({key}\n)");
    Parser::new(&allocator, &wrapped, SourceType::ts())
        .parse_expression()
        .is_ok_and(|expression| matches!(expression.get_inner_expression(), Expression::Identifier(id) if id.name.as_str() == index))
}

fn expression(source: &str, policy: crate::svelte_javascript::IrPolicy) -> bool {
    let allocator = Allocator::default();
    let wrapped = format!("({source}\n)");
    let source_type = match policy {
        crate::svelte_javascript::IrPolicy::Typed => SourceType::tsx(),
        crate::svelte_javascript::IrPolicy::JavaScript => SourceType::jsx(),
    };
    if !Parser::new(&allocator, &wrapped, source_type)
        .parse_expression()
        .is_ok_and(|value| value.span().end as usize == wrapped.len())
    {
        return false;
    }
    let program = format!("const __crepusEachExpression={wrapped};");
    let parsed = Parser::new(&allocator, &program, source_type).parse();
    parsed.errors.is_empty()
        && SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program)
            .errors
            .is_empty()
}

// Preserve leaf symbols and static access paths without eagerly reading intermediate objects.
fn object_fields<'s, 'a>(
    pattern: &'s ObjectPattern<'a>,
) -> Option<Vec<(&'s BindingIdentifier<'a>, Vec<String>)>> {
    let mut fields = Vec::new();
    let mut pending = vec![(pattern, Vec::new())];
    while let Some((pattern, path)) = pending.pop() {
        if pattern.rest.is_some() {
            return None;
        }
        for property in &pattern.properties {
            if property.computed {
                return None;
            }
            let key = match &property.key {
                PropertyKey::StaticIdentifier(key) => key.name.to_string(),
                PropertyKey::StringLiteral(key) => key.value.to_string(),
                _ => return None,
            };
            let mut path = path.clone();
            path.push(key);
            match &property.value {
                BindingPattern::BindingIdentifier(id) => fields.push((id.as_ref(), path)),
                BindingPattern::ObjectPattern(nested) => pending.push((nested.as_ref(), path)),
                _ => return None,
            }
        }
    }
    Some(fields)
}

// The span belongs to Oxc's TSTypeAnnotation, including its leading colon.
// Keep comments between that punctuation and the type rather than reparsing a type.
fn annotation_text(source: &str, span: Span) -> Option<&str> {
    source
        .get(span.start as usize..span.end as usize)?
        .strip_prefix(':')
}

struct Parameters {
    item: String,
    annotation: Option<String>,
    index: Option<String>,
    object: bool,
    supported: bool,
}

struct ParsedHeader {
    header: Header,
    supported: bool,
}

impl Parameters {
    fn with_source(self, iterable: &str, key: Option<String>) -> ParsedHeader {
        ParsedHeader {
            header: Header {
                iterable: iterable.into(),
                item: self.item,
                annotation: self.annotation,
                object: self.object,
                index: self.index,
                key,
            },
            supported: self.supported,
        }
    }
}

fn parameters(source: &str, policy: crate::svelte_javascript::IrPolicy) -> Option<Parameters> {
    let allocator = Allocator::default();
    let wrapped = format!("({source}\n)=>0");
    let source_type = match policy {
        crate::svelte_javascript::IrPolicy::Typed => SourceType::tsx(),
        crate::svelte_javascript::IrPolicy::JavaScript => SourceType::jsx(),
    };
    let Expression::ArrowFunctionExpression(arrow) = Parser::new(&allocator, &wrapped, source_type)
        .parse_expression()
        .ok()?
    else {
        return None;
    };
    if arrow.span.end as usize != wrapped.len()
        || arrow.r#async
        || arrow.return_type.is_some()
        || arrow.type_parameters.is_some()
        || arrow.params.rest.is_some()
        || !(1..=2).contains(&arrow.params.items.len())
    {
        return None;
    }
    // Svelte permits a type on the item, but not on its index or a whole-binding default.
    if arrow
        .params
        .items
        .iter()
        .any(|parameter| parameter.initializer.is_some() || parameter.optional)
        || arrow
            .params
            .items
            .get(1)
            .is_some_and(|parameter| parameter.type_annotation.is_some())
    {
        return None;
    }
    let parameter = &arrow.params.items[0];
    let first = &parameter.pattern;
    let names = first.get_binding_identifiers();
    let supported = match first {
        BindingPattern::BindingIdentifier(_) => true,
        _ => crate::svelte_pattern::supported(first),
    };
    let item_span = first.span();
    let item = match first {
        BindingPattern::BindingIdentifier(id) => id.name.to_string(),
        _ => wrapped[item_span.start as usize..item_span.end as usize].to_owned(),
    };
    let annotation = if let Some(annotation) = &parameter.type_annotation {
        Some(annotation_text(&wrapped, annotation.span)?.to_owned())
    } else {
        None
    };
    let index = if let Some(parameter) = arrow.params.items.get(1) {
        let BindingPattern::BindingIdentifier(id) = &parameter.pattern else {
            return None;
        };
        if names.iter().any(|name| name.name == id.name) {
            return None;
        }
        Some(id.name.to_string())
    } else {
        None
    };
    Some(Parameters {
        item,
        annotation,
        index,
        object: !matches!(first, BindingPattern::BindingIdentifier(_)),
        supported,
    })
}

const UNSUPPORTED_HEADER: &str = "each requires a collection, supported identifier/object/array pattern, optional identifier index and optional key; computed/numeric pattern keys and index annotations are unsupported";

/// Discover the complete grammar even when lowering does not yet own the pattern.
/// In particular inline object types must not leave their closing brace as markup.
pub(crate) fn header_boundary(source: &str, policy: crate::svelte_javascript::IrPolicy) -> bool {
    parse_header(source, policy).is_some()
}

/// Svelte supplies the `as` separator; Oxc validates both surrounding grammars.
/// Trying candidate token boundaries also handles TS assertions in the collection.
#[cfg(test)]
pub(crate) fn header(source: &str) -> Result<Header, String> {
    header_with_policy(source, crate::svelte_javascript::IrPolicy::Typed)
}

pub(crate) fn header_with_policy(
    source: &str,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<Header, String> {
    let parsed = parse_header(source, policy).ok_or(UNSUPPORTED_HEADER)?;
    if !parsed.supported {
        return Err(UNSUPPORTED_HEADER.into());
    }
    Ok(parsed.header)
}

fn parse_header(source: &str, policy: crate::svelte_javascript::IrPolicy) -> Option<ParsedHeader> {
    for (at, _) in source.match_indices("as") {
        if !source[..at].ends_with(char::is_whitespace)
            || !source[at + 2..].starts_with(char::is_whitespace)
        {
            continue;
        }
        let iterable = source[..at].trim();
        if !expression(iterable, policy) {
            continue;
        }
        let tail = source[at + 2..].trim();
        if let Some(parameters) = parameters(tail, policy) {
            return Some(parameters.with_source(iterable, None));
        }
        for (open, _) in tail.match_indices('(') {
            let Some(parameters) = parameters(tail[..open].trim(), policy) else {
                continue;
            };
            let key = tail[open..].trim();
            if key.starts_with('(') && key.ends_with(')') && expression(key, policy) {
                let key = key[1..key.len() - 1].trim();
                if !key.is_empty() {
                    return Some(parameters.with_source(iterable, Some(key.into())));
                }
            }
        }
    }
    None
}

fn insert_view(
    child: &ArrowFunctionExpression<'_>,
    edits: &mut Vec<(usize, usize, String)>,
    declaration: String,
) {
    if child.expression {
        edits.push((
            child.body.span.start as usize,
            child.body.span.start as usize,
            format!("{{{declaration}return ("),
        ));
        edits.push((
            child.body.span.end as usize,
            child.body.span.end as usize,
            ");}".into(),
        ));
    } else {
        let at = child
            .body
            .directives
            .last()
            .map_or(child.body.span.start as usize + 1, |directive| {
                directive.span.end as usize
            });
        edits.push((at, at, declaration));
    }
}

fn attr<'s, 'a>(element: &'s JSXElement<'a>, name: &str) -> Option<&'s JSXAttribute<'a>> {
    element
        .opening_element
        .attributes
        .iter()
        .find_map(|attribute| {
            let JSXAttributeItem::Attribute(attribute) = attribute else {
                return None;
            };
            matches!(&attribute.name, JSXAttributeName::Identifier(id) if id.name == name)
                .then_some(attribute.as_ref())
        })
}

fn arrow<'s, 'a>(attribute: &'s JSXAttribute<'a>) -> Option<&'s ArrowFunctionExpression<'a>> {
    let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
        return None;
    };
    match &container.expression {
        JSXExpression::ArrowFunctionExpression(arrow) => Some(arrow.as_ref()),
        JSXExpression::CallExpression(call) => {
            call.arguments
                .last()
                .and_then(|argument| match argument.as_expression()? {
                    Expression::ArrowFunctionExpression(arrow) => Some(arrow.as_ref()),
                    _ => None,
                })
        }
        _ => None,
    }
}

/// Materialize only name-bearing header syntax for early decoded-name reservation.
/// The returned programs are never executed or substituted into the component.
pub(crate) fn name_sources(
    source: &str,
    marker: &str,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<Vec<String>, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid authored each template".into());
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut sources = Vec::new();
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if name.name == marker)
        {
            continue;
        }
        let metadata = attr(element, "header").ok_or("missing authored each header")?;
        let Some(JSXAttributeValue::ExpressionContainer(value)) = &metadata.value else {
            return Err("invalid each header metadata".into());
        };
        let JSXExpression::StringLiteral(text) = &value.expression else {
            return Err("invalid each header metadata".into());
        };
        let parsed = header_with_policy(text.value.as_str(), policy)?;
        let item = parsed.annotation.as_ref().map_or_else(
            || parsed.item.clone(),
            |annotation| format!("{}:{annotation}", parsed.item),
        );
        let index = parsed
            .index
            .as_ref()
            .map(|name| format!(",{name}"))
            .unwrap_or_default();
        let key = parsed.key.as_deref().unwrap_or("0");
        // Keep trailing line comments separate from synthesized punctuation.
        sources.push(format!(
            "({}\n);({} {})=>({key}\n);",
            parsed.iterable, item, index
        ));
    }
    Ok(sources)
}

#[cfg(test)]
pub(crate) fn transform(
    source: &str,
    marker: &str,
    target: MoonshineModuleTarget,
    binding_markers: &[&str],
) -> Result<String, String> {
    transform_retained(source, marker, target, binding_markers, None)
}

#[cfg(test)]
pub(crate) fn transform_retained(
    source: &str,
    marker: &str,
    target: MoonshineModuleTarget,
    binding_markers: &[&str],
    retention: Option<&str>,
) -> Result<String, String> {
    transform_retained_with_policy(
        source,
        marker,
        target,
        binding_markers,
        retention,
        crate::svelte_javascript::IrPolicy::Typed,
    )
}

#[cfg(test)]
pub(crate) fn transform_retained_with_policy(
    source: &str,
    marker: &str,
    target: MoonshineModuleTarget,
    binding_markers: &[&str],
    retention: Option<&str>,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<String, String> {
    transform_retained_with_references(
        source,
        marker,
        target,
        binding_markers,
        retention,
        (policy, &Default::default()),
        (None, &[]),
    )
}

pub(crate) fn transform_retained_with_references(
    source: &str,
    marker: &str,
    target: MoonshineModuleTarget,
    binding_markers: &[&str],
    retention: Option<&str>,
    (policy, globals): (
        crate::svelte_javascript::IrPolicy,
        &crate::source_globals::SourceGlobals,
    ),
    (references, text_markers): (Option<&crate::svelte_intrinsics::ReferenceMarkers>, &[&str]),
) -> Result<String, String> {
    let (gr, gt) = (&globals.realm, &globals.types);
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid authored each template".into());
    }
    policy.configure(&mut parsed.program);
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut occupied = std::collections::HashSet::new();
    let mut reserve = |kind: AstKind<'_>| {
        let name = match kind {
            AstKind::BindingIdentifier(id) => id.name.as_str(),
            AstKind::IdentifierReference(id) => id.name.as_str(),
            AstKind::IdentifierName(id) => id.name.as_str(),
            _ => return,
        };
        occupied.insert(name.to_owned());
    };
    for node in semantic.nodes().iter() {
        reserve(node.kind());
    }
    // Header bindings remain string metadata until the each transform runs.
    for scope in name_sources(source, marker, policy)? {
        let parsed = Parser::new(&allocator, &scope, SourceType::tsx()).parse();
        if !parsed.errors.is_empty() {
            return Err("invalid each name metadata".into());
        }
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        for node in semantic.nodes().iter() {
            reserve(node.kind());
        }
    }
    let fresh_source = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name) || occupied.iter().any(|id| id.starts_with(&name)) {
            name.push('_');
        }
        name
    };
    let context_type = fresh_source("__crepusEachCollection");
    let context_reader = fresh_source("__crepusEachReader");
    let typed_children = [
        fresh_source("__crepusEachChildren"),
        fresh_source("__crepusEachAnnotatedChildren"),
    ];
    let typed_key = [
        fresh_source("__crepusEachKey"),
        fresh_source("__crepusEachAnnotatedKey"),
    ];
    let typed_snapshot = [
        fresh_source("__crepusEachSnapshotChildren"),
        fresh_source("__crepusEachAnnotatedSnapshotChildren"),
    ];
    let mut child_helpers = [false; 2];
    let mut key_helpers = [false; 2];
    let mut snapshot_helpers = [false; 2];
    let mut edits = Vec::new();
    let mut count = 0;
    let mut has_key = false;
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if name.name == marker)
        {
            continue;
        }
        let metadata = attr(element, "header").ok_or("missing authored each header")?;
        let Some(JSXAttributeValue::ExpressionContainer(value)) = &metadata.value else {
            return Err("invalid each header metadata".into());
        };
        let JSXExpression::StringLiteral(text) = &value.expression else {
            return Err("invalid each header metadata".into());
        };
        let parsed_header = header_with_policy(text.value.as_str(), policy)?;
        let index_key = index_identity_key(&parsed_header);
        has_key |= parsed_header.key.is_some() && !index_key;
        let child = arrow(attr(element, "children").ok_or("missing each body")?)
            .ok_or("invalid each body")?;
        let unused_index = fresh_source("__crepusUnusedEachIndex");
        let index = parsed_header.index.as_deref().unwrap_or(&unused_index);
        let value_parameter = parsed_header.annotation.as_ref().map_or_else(
            || parsed_header.item.clone(),
            |annotation| format!("{}:{annotation}", parsed_header.item),
        );
        let annotation_mode = usize::from(parsed_header.annotation.is_some());
        // A concrete generic constraint contextually types literals without
        // const inference's readonly arrays or excess-property checking.
        let reader = parsed_header.annotation.as_ref().map_or_else(
            || format!("()=>({}\n)", parsed_header.iterable),
            |annotation| format!("(<{context_type} extends {gt}.Iterable<{annotation}>|{gt}.ArrayLike<{annotation}>|null|undefined>({context_reader}:()=>{context_type})=>{context_reader})(()=>({}\n))", parsed_header.iterable),
        );
        let key_helper = &typed_key[annotation_mode];
        let key = parsed_header
            .key
            .as_ref()
            .map(|key| {
                key_helpers[annotation_mode] = true;
                let callback = format!(
                    "{key_helper}({reader},({}, {})=>({key}\n))",
                    value_parameter, index
                );
                // Retain authored key types without executing or allocating server callbacks.
                if matches!(target, MoonshineModuleTarget::Ssr) || index_key {
                    format!("false && {callback}")
                } else {
                    format!(" key={{{callback}}}")
                }
            })
            .unwrap_or_default();
        edits.push((
            metadata.span.start as usize,
            metadata.span.end as usize,
            if (matches!(target, MoonshineModuleTarget::Ssr) || index_key) && !key.is_empty() {
                format!("each={{({key}, {reader})}}")
            } else {
                format!("each={{{reader}}}{key}")
            },
        ));
        edits.push((
            child.params.span.start as usize,
            child.params.span.end as usize,
            format!("({}, {})", value_parameter, index),
        ));
        let child_helper = if parsed_header.object && matches!(target, MoonshineModuleTarget::Ssr) {
            snapshot_helpers[annotation_mode] = true;
            &typed_snapshot[annotation_mode]
        } else {
            child_helpers[annotation_mode] = true;
            &typed_children[annotation_mode]
        };
        edits.push((
            child.span.start as usize,
            child.span.start as usize,
            format!("{child_helper}({reader},"),
        ));
        edits.push((child.span.end as usize, child.span.end as usize, ")".into()));
        count += 1;
    }
    if count == 0 {
        return Ok(source.into());
    }
    edits.sort_by_key(|edit| (edit.0, edit.1));
    let mut output = source.to_owned();
    for (start, end, replacement) in edits.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    for text in text_markers {
        output = crate::svelte_text::prepare_call_readers(
            std::mem::take(&mut output),
            text,
            target,
            policy,
            &fresh_source,
        )?;
    }
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid each callback binding".into());
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err("invalid each binding scope".into());
    }
    let semantic = built.semantic;
    // Only bindings rooted in a row member need an identifier-rooted getter view.
    // Keep ordinary scalar rows on their existing accessor path.
    let member_bound: std::collections::HashSet<_> = semantic
        .nodes()
        .iter()
        .filter_map(|node| {
            let AstKind::IdentifierReference(id) = node.kind() else {
                return None;
            };
            let reference = id.reference_id.get()?;
            let symbol = semantic.scoping().get_reference(reference).symbol_id()?;
            let mut span = id.span;
            let mut member = false;
            let mut container = false;
            for parent in semantic.nodes().ancestors(node.id()) {
                match parent.kind() {
                    AstKind::ParenthesizedExpression(_)
                    | AstKind::TSAsExpression(_)
                    | AstKind::TSSatisfiesExpression(_)
                    | AstKind::TSNonNullExpression(_)
                    | AstKind::TSTypeAssertion(_) => {
                        span = parent.kind().span();
                    }
                    AstKind::StaticMemberExpression(value)
                        if !value.optional && value.object.span() == span =>
                    {
                        member = true;
                        span = value.span;
                    }
                    AstKind::ComputedMemberExpression(value)
                        if !value.optional && value.object.span() == span =>
                    {
                        member = true;
                        span = value.span;
                    }
                    AstKind::JSXExpressionContainer(value)
                        if member && value.expression.span() == span =>
                    {
                        container = true;
                    }
                    AstKind::JSXAttribute(value) if container => {
                        return matches!(&value.name, JSXAttributeName::Identifier(name)
                            if binding_markers.contains(&name.name.as_str()))
                        .then_some(symbol);
                    }
                    _ => return None,
                }
            }
            None
        })
        .collect();
    let mut symbols = Vec::new();
    let mut key_symbols = Vec::new();
    let mut projections = Vec::new();
    let mut parameter_edits = Vec::new();
    let mut reserved = semantic
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::BindingIdentifier(id) => Some(id.name.to_string()),
            AstKind::IdentifierReference(id) => Some(id.name.to_string()),
            _ => None,
        })
        .collect::<std::collections::HashSet<_>>();
    let mut fresh_pattern = |base: &str| {
        let mut name = base.to_owned();
        while reserved.contains(&name) || output.contains(&name) {
            name.push('_');
        }
        reserved.insert(name.clone());
        name
    };
    let source_effect = fresh_pattern("__crepusEachSourceEffect");
    let row_factory = fresh_pattern("__crepusEachRowFactory");
    let fallback_factory = fresh_pattern("__crepusEachFallbackFactory");
    let fallback_read = fresh_pattern("__crepusEachFallback");
    let pattern_memo = fresh_pattern("__crepusEachPatternMemo");
    let pattern_rest = fresh_pattern("__crepusEachPatternRest");
    let pattern_rest_shape = fresh_pattern("__crepusEachPatternRestShape");
    let mut pending_patterns = Vec::new();
    let mut pattern_count = 0;
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if name.name == marker)
        {
            continue;
        }
        let child = arrow(attr(element, "children").ok_or("missing each body")?)
            .ok_or("invalid each body")?;
        let snapshot = matches!(target, MoonshineModuleTarget::Ssr)
            && matches!(
                child
                    .params
                    .items
                    .first()
                    .map(|parameter| &parameter.pattern),
                Some(BindingPattern::ObjectPattern(_) | BindingPattern::ArrayPattern(_))
            );
        for parameter in &child.params.items {
            match &parameter.pattern {
                BindingPattern::BindingIdentifier(id) => {
                    let symbol = id.symbol_id.get().ok_or("unresolved each binding")?;
                    if !snapshot {
                        if let Some(annotation) = &parameter.type_annotation {
                            let declared = annotation_text(&output, annotation.span)
                                .ok_or("invalid each type annotation span")?;
                            parameter_edits.push((
                                parameter.span.start as usize,
                                parameter.span.end as usize,
                                format!("{}:()=>({declared})", id.name),
                            ));
                        }
                    }
                    if snapshot {
                        key_symbols.push(symbol);
                    } else if parameter.span == child.params.items[0].span
                        && member_bound.contains(&symbol)
                    {
                        let mut view = fresh_source(&format!("__crepusEachBinding{pattern_count}"));
                        pattern_count += 1;
                        while semantic.nodes().iter().any(|node| match node.kind() {
                            AstKind::BindingIdentifier(id) => id.name.as_str() == view.as_str(),
                            AstKind::IdentifierReference(id) => id.name.as_str() == view.as_str(),
                            _ => false,
                        }) {
                            view.push('_');
                        }
                        projections.push((symbol, format!("{view}.value")));
                        insert_view(
                            child,
                            &mut parameter_edits,
                            format!("const {view}={{get value(){{return {}()}}}};", id.name),
                        );
                    } else {
                        symbols.push(symbol);
                    }
                }
                BindingPattern::ObjectPattern(pattern) if object_fields(pattern).is_some() => {
                    let fields = object_fields(pattern).ok_or("unsupported each object binding")?;
                    let raw = fresh_source(&format!("__crepusEachItem{pattern_count}"));
                    pattern_count += 1;
                    if !snapshot {
                        let (span, replacement) =
                            if let Some(annotation) = &parameter.type_annotation {
                                let declared = annotation_text(&output, annotation.span)
                                    .ok_or("invalid each type annotation span")?;
                                (parameter.span, format!("{raw}:()=>({declared})"))
                            } else {
                                (pattern.span, raw.clone())
                            };
                        parameter_edits.push((span.start as usize, span.end as usize, replacement));
                    }
                    let view = fresh_source(&format!("__crepusEachFields{pattern_count}"));
                    let mut getters = Vec::new();
                    for (id, path) in fields {
                        let symbol = id.symbol_id.get().ok_or("unresolved each object binding")?;
                        if snapshot {
                            key_symbols.push(symbol);
                        } else {
                            let path = path
                                .iter()
                                .map(|key| {
                                    serde_json::to_string(key)
                                        .map(|key| format!("[{key}]"))
                                        .map_err(|error| {
                                            format!("cannot encode each property key: {error}")
                                        })
                                })
                                .collect::<Result<String, _>>()?;
                            let alias_key = serde_json::to_string(id.name.as_str())
                                .map_err(|error| format!("cannot encode each alias: {error}"))?;
                            getters.push(format!("get [{alias_key}](){{return {raw}(){path}}}"));
                            projections.push((symbol, format!("{view}.{}", id.name)));
                        }
                    }
                    if !snapshot {
                        let comments = parsed
                            .program
                            .comments
                            .iter()
                            .filter(|comment| {
                                comment.span.start >= pattern.span.start
                                    && comment.span.end <= pattern.span.end
                            })
                            .map(|comment| {
                                format!(
                                    "{}\n",
                                    &output[comment.span.start as usize..comment.span.end as usize]
                                )
                            })
                            .collect::<String>();
                        let declaration =
                            format!("{comments}const {view}={{{}}};", getters.join(","));
                        insert_view(child, &mut parameter_edits, declaration);
                    }
                }
                pattern @ (BindingPattern::ObjectPattern(_) | BindingPattern::ArrayPattern(_)) => {
                    if snapshot {
                        for id in pattern.get_binding_identifiers() {
                            key_symbols.push(
                                id.symbol_id
                                    .get()
                                    .ok_or("unresolved each snapshot binding")?,
                            );
                        }
                        continue;
                    }
                    if semantic.nodes().iter().any(|node|matches!(node.kind(),AstKind::BindingIdentifier(id) if id.name=="globalThis")) && !globals.is_active() {
                        return Err("array/rest each patterns with a shadowed globalThis are unsupported".into());
                    }
                    let raw = fresh_pattern("__crepusEachPatternItem");
                    let view = fresh_pattern("__crepusEachPatternFields");
                    let replacement = if let Some(annotation) = &parameter.type_annotation {
                        format!(
                            "{raw}:()=>({})",
                            annotation_text(&output, annotation.span)
                                .ok_or("invalid each annotation")?
                        )
                    } else {
                        raw.clone()
                    };
                    parameter_edits.push((
                        parameter.span.start as usize,
                        parameter.span.end as usize,
                        replacement,
                    ));
                    let plan = crate::svelte_pattern::Plan::build(
                        pattern,
                        &raw,
                        &mut fresh_pattern,
                        globals,
                    )?;
                    for leaf in &plan.leaves {
                        projections.push((
                            leaf.id
                                .symbol_id
                                .get()
                                .ok_or("unresolved each pattern binding")?,
                            format!("{view}.{}", leaf.id.name),
                        ));
                    }
                    pending_patterns.push((plan, child, view, parameter.span));
                }
                _ => return Err("unsupported each binding".into()),
            }
        }
        if let Some(key) = attr(element, "key").and_then(arrow) {
            for parameter in &key.params.items {
                match &parameter.pattern {
                    BindingPattern::BindingIdentifier(id) => {
                        key_symbols.push(id.symbol_id.get().ok_or("unresolved each key binding")?)
                    }
                    pattern => {
                        for id in pattern.get_binding_identifiers() {
                            key_symbols
                                .push(id.symbol_id.get().ok_or("unresolved each key binding")?);
                        }
                    }
                }
            }
        }
    }
    let mut reads = Vec::new();
    for node in semantic.nodes().iter() {
        let AstKind::IdentifierReference(id) = node.kind() else {
            continue;
        };
        let reference = semantic
            .scoping()
            .get_reference(id.reference_id.get().ok_or("unresolved each reference")?);
        let Some(symbol) = reference.symbol_id() else {
            continue;
        };
        let projection = projections.iter().find(|(binding, _)| *binding == symbol);
        if !symbols.contains(&symbol) && !key_symbols.contains(&symbol) && projection.is_none() {
            continue;
        }
        let mut reference_span = id.span;
        let mut direct_call = false;
        let mut tagged_call = false;
        let mut binding_container = false;
        for parent in semantic.nodes().ancestors(node.id()) {
            match parent.kind() {
                AstKind::ParenthesizedExpression(_)
                | AstKind::TSAsExpression(_)
                | AstKind::TSSatisfiesExpression(_)
                | AstKind::TSNonNullExpression(_)
                | AstKind::TSTypeAssertion(_)
                | AstKind::TSInstantiationExpression(_) => {
                    reference_span = parent.kind().span();
                    continue;
                }
                AstKind::CallExpression(call) => direct_call = call.callee.span() == reference_span,
                AstKind::TaggedTemplateExpression(tag) => {
                    tagged_call = tag.tag.span() == reference_span
                }
                AstKind::JSXExpressionContainer(container)
                    if container.expression.span() == reference_span =>
                {
                    binding_container = true;
                    continue;
                }
                AstKind::JSXAttribute(attribute) if binding_container => {
                    if matches!(&attribute.name, JSXAttributeName::Identifier(name) if binding_markers.contains(&name.name.as_str()))
                    {
                        return Err("each bindings cannot be bound directly; bind a member or collection index instead".into());
                    }
                }
                _ => {}
            }
            break;
        }
        if reference.is_write() || reference.is_type() || reference.flags().is_value_as_type() {
            return Err("each bindings cannot be reassigned or used as type queries".into());
        }
        if !symbols.contains(&symbol) && projection.is_none() {
            continue;
        }
        let read = projection.map_or_else(
            || format!("{}()", id.name),
            |(_, read)| {
                if tagged_call {
                    // Preserve an unbound tag reference through production minification.
                    format!("(()=>{read})()")
                } else if direct_call {
                    format!("(0,{read})")
                } else {
                    read.clone()
                }
            },
        );
        let replacement = match semantic.nodes().parent_kind(node.id()) {
            AstKind::ObjectProperty(property) if property.shorthand => {
                crate::svelte_expression::shorthand_property(id.name.as_str(), &read)
            }
            _ => read,
        };
        reads.push((id.span.start as usize, id.span.end as usize, replacement));
    }
    reads.sort_by_key(|edit| edit.0);
    let mut pattern_runtime = String::new();
    if !pending_patterns.is_empty() {
        let mut rest_used = false;
        let mut memo_used = false;
        for (plan, child, view, _) in &pending_patterns {
            let default = |span: Span| -> Result<String, String> {
                let mut value = output[span.start as usize..span.end as usize].to_owned();
                for (start, end, replacement) in reads
                    .iter()
                    .filter(|(start, end, _)| {
                        *start >= span.start as usize && *end <= span.end as usize
                    })
                    .rev()
                {
                    value.replace_range(
                        start - span.start as usize..end - span.start as usize,
                        replacement,
                    );
                }
                Ok(value)
            };
            let defaults = plan.default_spans();
            let pattern_span = child.params.items[0].pattern.span();
            let mut declaration = parsed
                .program
                .comments
                .iter()
                .filter(|comment| {
                    comment.span.start >= pattern_span.start
                        && comment.span.end <= pattern_span.end
                        && !defaults.iter().any(|default| {
                            comment.span.start >= default.start && comment.span.end <= default.end
                        })
                })
                .map(|comment| {
                    format!(
                        "{}\n",
                        &output[comment.span.start as usize..comment.span.end as usize]
                    )
                })
                .collect::<String>();
            declaration.push_str(&plan.declarations(&pattern_memo, &pattern_rest, &default)?);
            let getters = plan
                .leaves
                .iter()
                .enumerate()
                .map(|(i, leaf)| {
                    let key = serde_json::to_string(leaf.id.name.as_str())
                        .map_err(|e| format!("cannot encode each alias: {e}"))?;
                    Ok(format!(
                        "get [{key}](){{return {}}}",
                        plan.read(i, &pattern_rest, &default)?
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?;
            declaration.push_str(&format!("const {view}={{{}}};", getters.join(",")));
            insert_view(child, &mut parameter_edits, declaration);
            rest_used |= plan.needs_rest;
            memo_used |= plan.needs_memo();
        }
        reads.retain(|(start, end, _)| {
            !pending_patterns
                .iter()
                .any(|(_, _, _, span)| *start >= span.start as usize && *end <= span.end as usize)
        });
        if memo_used {
            pattern_runtime = format!(
            "import {{createDerived as {pattern_memo}}} from '@tschk/moonshine-svelte/runes';\n"
        );
        }
        if rest_used {
            pattern_runtime.push_str(&crate::svelte_pattern::rest_helper(
                &pattern_rest,
                &pattern_rest_shape,
                globals,
            ));
        }
    }
    if let Some(markers) = references {
        let mut contexts = symbols
            .iter()
            .map(|symbol| {
                (
                    *symbol,
                    format!("{}()", semantic.scoping().symbol_name(*symbol)),
                )
            })
            .collect::<Vec<_>>();
        contexts.extend(projections.iter().cloned());
        let planned = crate::svelte_intrinsics::reference_edits(
            &output, &semantic, markers, target, &contexts,
        )?;
        for (span, edits) in planned {
            reads.retain(|(start, end, _)| {
                !(*start >= span.start as usize && *end <= span.end as usize)
            });
            reads.extend(edits);
        }
    }
    reads.extend(parameter_edits);
    reads.sort_by_key(|edit| edit.0);
    for (start, end, replacement) in reads.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while output.contains(&name) || occupied.iter().any(|id| id.starts_with(&name)) {
            name.push('_');
        }
        name
    };
    let memo = fresh("__crepusEachMemo");
    let keyed = fresh("__crepusEachKeyed");
    let child_type = fresh("__crepusEachChild");
    let indexed = fresh("__crepusEachIndex");
    let show = fresh("__crepusEachShow");
    let dom_keyed = has_key && matches!(target, MoonshineModuleTarget::Dom);
    // This helper is emitted only for native authored Svelte DOM, whose
    // intrinsic children now establish the ordered namespace callback.
    let prepare = fresh("__crepusEachPrepare");
    let prepared = fresh("__crepusEachPrepared");
    let dispatch = if dom_keyed {
        let retain = retention
            .map(|name| format!(",props.{name}"))
            .unwrap_or_default();
        // Prepare before later sibling setup; manual keyedEach callers retain
        // their existing lazy insertion contract.
        format!("if(props.key){{const {prepared}={keyed}(props.each,props.key,{row_factory},{fallback_factory}{retain});{prepare}({prepared});return {prepared};}}")
    } else {
        String::new()
    };
    let inline_schedule = if retention.is_some() {
        ",read=>read"
    } else {
        ""
    };
    let body = if matches!(target, MoonshineModuleTarget::Ssr) {
        format!(
            "return {memo}(()=>{{const raw=props.each(),items:readonly T[]={gr}.Array.isArray(raw)?raw:{gr}.Array.from(raw??[]);if(!items.length)return props.fallback?.();const values:U[]=[];for(let index=0;index<items.length;index++){{const item=items[index]!;values.push(props.children(()=>item,()=>index));}}return values;}});"
        )
    } else {
        format!(
            "const {row_factory}=(item:()=>T,index:()=>number):U=>{{{source_effect}();return props.children(item,index);}};const {fallback_read}=props.fallback;const {fallback_factory}={fallback_read}?()=>{{{source_effect}();return {fallback_read}();}}:undefined;{dispatch}const items={memo}(()=>{{const raw=props.each();const values:readonly T[]={gr}.Array.isArray(raw)?raw:{gr}.Array.from(raw??[]);return values;}});return {show}<number,U|(()=>U[])>({{when:()=>items().length,children:()=>{indexed}(items,(item,index)=>{row_factory}(item,()=>index){inline_schedule}),fallback:{fallback_factory}}}{inline_schedule});"
        )
    };
    let imports = if matches!(target, MoonshineModuleTarget::Ssr) {
        format!("import {{createMemo as {memo}}} from '@tschk/moonshine-dom/ssr';\n")
    } else {
        let optional = if dom_keyed {
            format!(
                "import {{keyedEach as {keyed}}} from '@tschk/moonshine-svelte/each';\nimport type {{DOMChild as {child_type}}} from '@tschk/moonshine-dom/dom';\n"
            )
        } else {
            String::new()
        };
        let preparation = if dom_keyed {
            format!(
                "import {{prepareInsertionRange as {prepare}}} from '@tschk/moonshine-dom/dom';\n"
            )
        } else {
            String::new()
        };
        format!(
            "import {{markRuneSourceEffect as {source_effect}}} from '@tschk/moonshine-svelte/pre';\n{optional}{preparation}import {{createRenderMemo as {memo}}} from '@tschk/moonshine-dom/reactive';\nimport {{indexArray as {indexed},Show as {show}}} from '@tschk/moonshine-dom/control';\n"
        )
    };
    let output_type = if dom_keyed {
        format!("U extends {child_type}")
    } else {
        "U".into()
    };
    // Annotated reader constraints preserve literal/tuple context and structural
    // extra properties without const inference making arrays readonly. This is native typing
    // policy: the official Svelte checker currently erases these annotations.
    // Unannotated typed readers select any only for an actually-any collection; JS IR retains
    // one implementation binding so its original lexical validation stays strict.
    let mut types = String::new();
    for (mode, parameter) in ["T", "T"].into_iter().enumerate() {
        if child_helpers[mode] {
            let helper = &typed_children[mode];
            if policy == crate::svelte_javascript::IrPolicy::Typed && mode == 0 {
                types.push_str(&format!("function {helper}<R extends ()=>unknown,U>(_read:R & (0 extends 1 & {gt}.ReturnType<R> ? unknown : never),fn:(item:()=>any,index:()=>number)=>U):typeof fn;\nfunction {helper}<{parameter},U>(_read:()=>{gt}.Iterable<T>|{gt}.ArrayLike<T>|null|undefined,fn:(item:()=>T,index:()=>number)=>U):typeof fn;\n"));
            }
            types.push_str(&format!("function {helper}<{parameter},U>(_read:()=>{gt}.Iterable<T>|{gt}.ArrayLike<T>|null|undefined,fn:(item:()=>T,index:()=>number)=>U){{return fn}}\n"));
        }
        if key_helpers[mode] {
            let helper = &typed_key[mode];
            if policy == crate::svelte_javascript::IrPolicy::Typed && mode == 0 {
                types.push_str(&format!("function {helper}<R extends ()=>unknown>(_read:R & (0 extends 1 & {gt}.ReturnType<R> ? unknown : never),fn:(item:any,index:number)=>unknown):typeof fn;\nfunction {helper}<{parameter}>(_read:()=>{gt}.Iterable<T>|{gt}.ArrayLike<T>|null|undefined,fn:(item:T,index:number)=>unknown):typeof fn;\n"));
            }
            types.push_str(&format!("function {helper}<{parameter}>(_read:()=>{gt}.Iterable<T>|{gt}.ArrayLike<T>|null|undefined,fn:(item:T,index:number)=>unknown){{return fn}}\n"));
        }
        if snapshot_helpers[mode] {
            let helper = &typed_snapshot[mode];
            if policy == crate::svelte_javascript::IrPolicy::Typed && mode == 0 {
                types.push_str(&format!("function {helper}<R extends ()=>unknown,U>(_read:R & (0 extends 1 & {gt}.ReturnType<R> ? unknown : never),fn:(item:any,index:number)=>U):(item:()=>any,index:()=>number)=>U;\nfunction {helper}<{parameter},U>(_read:()=>{gt}.Iterable<T>|{gt}.ArrayLike<T>|null|undefined,fn:(item:T,index:number)=>U):(item:()=>T,index:()=>number)=>U;\n"));
            }
            types.push_str(&format!("function {helper}<{parameter},U>(_read:()=>{gt}.Iterable<T>|{gt}.ArrayLike<T>|null|undefined,fn:(item:T,index:number)=>U){{return(item:()=>T,index:()=>number)=>fn(item(),index())}}\n"));
        }
    }
    let retain_type = retention
        .map(|name| format!(";{name}?:(node:{gt}.Node)=>boolean"))
        .unwrap_or_default();
    Ok(format!(
        "{pattern_runtime}{imports}{output}\n{types}\nfunction {marker}<T,{output_type}>(props:{{each:()=>{gt}.Iterable<T>|{gt}.ArrayLike<T>|null|undefined;key?:(item:T,index:number)=>unknown;children:(item:()=>T,index:()=>number)=>U;fallback?:()=>U{retain_type}}}){{{body}}}\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::{header, Header};
    fn transform(
        source: &str,
        marker: &str,
        target: crate::MoonshineModuleTarget,
    ) -> Result<String, String> {
        super::transform(source, marker, target, &[])
    }
    use crate::MoonshineModuleTarget;
    fn checked_output(source: &str, target: MoonshineModuleTarget) -> String {
        let output = transform(source, "Each", target).unwrap();
        let allocator = oxc_allocator::Allocator::default();
        let parsed =
            oxc_parser::Parser::new(&allocator, &output, oxc_span::SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
        let semantic = oxc_semantic::SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        assert!(
            semantic.errors.is_empty(),
            "{:?}\n{output}",
            semantic.errors
        );
        output
    }

    #[test]
    fn explicit_annotations_contextualize_readers_without_changing_ordinary_inference() {
        let source = r#"function App(){const sample=1;return <><Each header={"[1] as item:typeof sample,index (item)"} children={()=> <p>{item}</p>}/><Each header={"[1] as ordinary,index (ordinary)"} children={()=> <p>{(()=>{let value=ordinary;value=2;return value.toFixed()})()}</p>}/></>;}"#;
        let output = checked_output(source, MoonshineModuleTarget::Dom);
        assert!(
            output.contains("function __crepusEachAnnotatedChildren<T,U>"),
            "{output}"
        );
        assert!(
            output.contains("function __crepusEachAnnotatedKey<T>"),
            "{output}"
        );
        assert!(
            output.contains("function __crepusEachChildren<T,U>"),
            "{output}"
        );
        assert!(output.contains("function __crepusEachKey<T>"), "{output}");
        assert!(!output.contains("SnapshotChildren"), "{output}");
        assert!(output.contains("let value=ordinary();value=2"), "{output}");
    }

    #[test]
    fn annotated_snapshot_and_reader_helpers_are_emitted_only_for_actual_uses() {
        let source = r#"function App(){return <Each header={"[{value:1,extra:2}] as {value}:{value:number}"} children={()=> <p>{value}</p>}/>;}"#;
        let dom = checked_output(source, MoonshineModuleTarget::Dom);
        assert!(
            dom.contains("function __crepusEachAnnotatedChildren<T,U>"),
            "{dom}"
        );
        assert!(!dom.contains("function __crepusEachChildren<"), "{dom}");
        assert!(!dom.contains("function __crepusEachAnnotatedKey<"), "{dom}");
        let ssr = checked_output(source, MoonshineModuleTarget::Ssr);
        assert!(
            ssr.contains("function __crepusEachAnnotatedSnapshotChildren<T,U>"),
            "{ssr}"
        );
        assert!(
            !ssr.contains("function __crepusEachAnnotatedChildren<"),
            "{ssr}"
        );
        assert!(!ssr.contains("function __crepusEachKey<"), "{ssr}");
    }

    #[test]
    fn annotation_reader_context_is_qualified_and_decoded_name_hygienic() {
        let source = r#"type Iterable<T>=never;type ArrayLike<T>=never;function App(){const \u005f_crepusEachReader=1;type \u005f_crepusEachCollection=number;return <Each header={"[[1,2]] as row:number[]"} children={()=> <p>{row[0]}</p>}/>;}"#;
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = checked_output(source, target);
            assert!(output.contains("<__crepusEachCollection_ extends globalThis.Iterable<number[]>|globalThis.ArrayLike<number[]>|null|undefined>"), "{output}");
            assert!(
                output.contains(
                    "(__crepusEachReader_:()=>__crepusEachCollection_)=>__crepusEachReader_"
                ),
                "{output}"
            );
            assert!(!output.contains("const T"), "{output}");
        }
    }

    #[test]
    fn unannotated_reader_has_no_context_wrapper() {
        let output = checked_output(
            r#"function App(){return <Each header={"rows as row"} children={()=> <p>{row}</p>}/>;}"#,
            MoonshineModuleTarget::Dom,
        );
        assert!(output.contains("each={()=>"), "{output}");
        assert!(!output.contains("__crepusEachCollection"), "{output}");
        assert!(!output.contains("__crepusEachReader"), "{output}");
        assert!(
            output.contains("_read:()=>globalThis.Iterable<T>|globalThis.ArrayLike<T>"),
            "{output}"
        );
    }

    #[test]
    fn index_identity_keys_retain_types_without_enabling_keyed_runtime() {
        for key in ["i", "(i)", "i as number", "i!", "i satisfies number"] {
            let source = format!(
                "function App(){{return <Each header={{\"rows as [value=fallback()],i({key})\"}} children={{()=> <p>{{value}}</p>}}/>;}}"
            );
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let output = checked_output(&source, target);
                assert!(output.contains("false && __crepusEachKey("), "{output}");
                assert!(!output.contains(" key={"), "{output}");
                assert!(
                    !output.contains("from '@tschk/moonshine-svelte/each'"),
                    "{output}"
                );
            }
        }
    }
    #[test]
    fn index_identity_keys_skip_native_pattern_evaluation_only_after_ts_erasure() {
        for key in [
            "i",
            "(i)",
            "((i))",
            "\\u0069",
            "i as number",
            "i!",
            "i satisfies number",
            "<number>i",
            "i<number>",
        ] {
            let mut parsed = header("rows as [value=fallback()],i(i)").unwrap();
            parsed.key = Some(key.into());
            assert!(super::index_identity_key(&parsed), "{key}");
        }
        for key in ["i+0", "key(i)", "0", "value"] {
            let mut parsed = header("rows as [value=fallback()],i(i)").unwrap();
            parsed.key = Some(key.into());
            assert!(!super::index_identity_key(&parsed), "{key}");
        }
        let source = r#"function App(){return <Each header={"rows as [value=fallback()],i(i)"} children={()=> <p>{value}</p>}/>;}"#;
        let output = checked_output(source, MoonshineModuleTarget::Dom);
        assert!(
            !output.contains("from '@tschk/moonshine-svelte/each'"),
            "{output}"
        );
        assert!(!output.contains(" key={"), "{output}");
        assert!(output.contains("__crepusEachIndex"), "{output}");
    }

    #[test]
    fn annotation_context_does_not_erase_self_type_query_guard() {
        let source = r#"function App(){return <Each header={"[1] as item:typeof item"} children={()=> <p>{item}</p>}/>;}"#;
        assert!(transform(source, "Each", MoonshineModuleTarget::Dom).is_err());
        assert!(transform(source, "Each", MoonshineModuleTarget::Ssr).is_err());
    }

    #[test]
    fn typed_item_annotations_keep_the_original_pattern_and_type() {
        let scalar = header("rows as item: /*keep*/ Box<number>, index (item.id)").unwrap();
        assert_eq!(scalar.item, "item");
        assert_eq!(
            scalar.annotation.as_deref().map(str::trim),
            Some("/*keep*/ Box<number>")
        );
        assert_eq!(scalar.index.as_deref(), Some("index"));
        assert_eq!(scalar.key.as_deref(), Some("item.id"));
        let object = header("rows as {value:renamed}: {value?:number}, index").unwrap();
        assert_eq!(object.item, "{value:renamed}");
        assert_eq!(
            object.annotation.as_deref().map(str::trim),
            Some("{value?:number}")
        );
        assert!(object.object);
        for text in [
            "rows as item:typeof sample",
            "rows as item:import('./types').Row",
            "rows as item:()=>number (item())",
        ] {
            assert!(header(text).is_ok(), "{text}");
        }
    }

    #[test]
    fn boundary_grammar_owns_inline_types_even_for_unlowerable_patterns() {
        let typed = crate::svelte_javascript::IrPolicy::Typed;
        for text in [
            "rows as {[field]:value}:Row",
            "rows as {0:value}:Row",
            "rows as {nested:{[field]:value}}:Row",
        ] {
            assert!(super::header_boundary(text, typed), "{text}");
            assert!(
                header(text)
                    .unwrap_err()
                    .contains("computed/numeric pattern keys"),
                "{text}"
            );
        }
        let optional_prefix = "{#each rows as {value}:{value?:number}}";
        assert_eq!(
            crate::svelte_boundary::boundary(&format!("{optional_prefix}<p/>{{/each}}")),
            Some(optional_prefix.len() - 1)
        );
        let prefix = "{#each rows as {nested:{value}}:{nested:{value?:number}}}";
        assert_eq!(
            crate::svelte_boundary::boundary(&format!("{prefix}<p/>{{/each}}")),
            Some(prefix.len() - 1)
        );
        for text in [
            "rows as item,index:number",
            "rows as item:number,index:number",
            "rows as item:number=3",
        ] {
            assert!(!super::header_boundary(text, typed), "{text}");
        }
    }

    #[test]
    fn original_javascript_does_not_admit_typed_contexts() {
        let js = crate::svelte_javascript::IrPolicy::JavaScript;
        for text in [
            "rows as item:number",
            "rows as {value}:{value:number}",
            "(rows as Row[]) as item",
        ] {
            assert!(super::header_with_policy(text, js).is_err(), "{text}");
        }
        assert!(super::header_with_policy("rows as {value},index (value)", js).is_ok());
    }

    #[test]
    fn typed_callbacks_distinguish_reader_and_value_parameters() {
        let scalar = r#"function App(){type Row={value:number};const rows:Row[]=[];return <Each header={"rows as row:Row,index (row.value)"} children={()=> <p>{row.value}:{index}</p>}/>;}"#;
        let dom = checked_output(scalar, MoonshineModuleTarget::Dom);
        assert!(dom.contains("row:()=>(Row)"), "{dom}");
        assert!(dom.contains("(row:Row, index)=>(row.value"), "{dom}");
        assert!(dom.contains("row().value"), "{dom}");
        let commented = scalar.replace("row:Row", "row:/*keep type comment*/Row");
        assert!(checked_output(&commented, MoonshineModuleTarget::Dom)
            .contains("/*keep type comment*/"));
        let object = r#"function App(){type Row={value:number};const rows:Row[]=[];return <Each header={"rows as {value}:Row,index (value)"} children={()=> <p>{value}:{index}</p>}/>;}"#;
        let dom = checked_output(object, MoonshineModuleTarget::Dom);
        assert!(dom.contains("__crepusEachItem0:()=>(Row)"), "{dom}");
        assert!(!dom.contains("__crepusEachItem0:Row"), "{dom}");
        assert!(
            dom.contains("get [\"value\"](){return __crepusEachItem0()[\"value\"]}"),
            "{dom}"
        );
        let ssr = checked_output(object, MoonshineModuleTarget::Ssr);
        assert!(ssr.contains("({value}:Row, index)"), "{ssr}");
        assert!(!ssr.contains("key={"), "{ssr}");
    }

    #[test]
    fn scalar_member_bindings_get_a_lexical_view_only_when_needed() {
        for context in ["row", "row:Row"] {
            let source = format!(
                r#"function App(){{type Row={{nested:{{value:number}},fn:()=>number}};const rows:Row[]=[];return <Each header={{"rows as {context}"}} children={{()=> <><input BindValue={{row.nested.value}}/><p>{{row.fn()}}:{{row.nested.value}}</p></>}}/>;}}"#
            );
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let output = super::transform(&source, "Each", target, &["BindValue"]).unwrap();
                assert!(output.contains("get value(){return row()}"), "{output}");
                assert!(
                    output.contains("BindValue={__crepusEachBinding0.value.nested.value}"),
                    "{output}"
                );
                assert!(
                    output.contains("__crepusEachBinding0.value.fn()"),
                    "{output}"
                );
                let allocator = oxc_allocator::Allocator::default();
                let parsed =
                    oxc_parser::Parser::new(&allocator, &output, oxc_span::SourceType::tsx())
                        .parse();
                assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
            }
        }
        let direct = r#"function App(){return <Each header={"rows as row"} children={()=> <input BindValue={row}/>}/>;}"#;
        assert!(
            super::transform(direct, "Each", MoonshineModuleTarget::Dom, &["BindValue"])
                .unwrap_err()
                .contains("cannot be bound directly")
        );
        let ordinary = r#"function App(){return <Each header={"rows as row"} children={()=> <p>{row.value}</p>}/>;}"#;
        assert!(
            !super::transform(ordinary, "Each", MoonshineModuleTarget::Dom, &["BindValue"])
                .unwrap()
                .contains("__crepusEachBinding")
        );
    }

    #[test]
    fn enabled_scalar_views_do_not_relax_direct_binding_or_assignment_guards() {
        for context in ["row", "row:Row"] {
            for (body, diagnostic) in [
                (
                    "<><input BindValue={row.value}/><input BindValue={row}/></>",
                    "cannot be bound directly",
                ),
                (
                    "<><input BindValue={row.value}/><p>{row={value:2}}</p></>",
                    "cannot be reassigned",
                ),
            ] {
                let source = format!(
                    r#"function App(){{type Row={{value:number}};const rows:Row[]=[];return <Each header={{"rows as {context}"}} children={{()=> {body}}}/>;}}"#
                );
                for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                    let error =
                        super::transform(&source, "Each", target, &["BindValue"]).unwrap_err();
                    assert!(error.contains(diagnostic), "{context}: {error}");
                }
            }
        }
    }

    #[test]
    fn scalar_binding_views_follow_symbols_and_avoid_decoded_name_collisions() {
        let nested = r#"function App(){const __crepusEachBind\u0069ng0=1;return <Each header={"rows as row"} children={()=> <><p>{row.value}</p><Each header={"row.children as row"} children={()=> <input BindValue={row.value}/>}/></>}/>;}"#;
        let output =
            super::transform(nested, "Each", MoonshineModuleTarget::Dom, &["BindValue"]).unwrap();
        assert!(
            output.contains("const __crepusEachBinding0_={get value(){return row()}};"),
            "{output}"
        );
        assert_eq!(
            output.matches("get value(){return row()}").count(),
            1,
            "{output}"
        );
        assert!(output.contains("<p>{row().value}</p>"), "{output}");
        let shadow = r#"function App(){return <Each header={"rows as row"} children={()=> <>{((row)=> <input BindValue={row.value}/>)(other)}<p>{row.value}</p></>}/>;}"#;
        let output =
            super::transform(shadow, "Each", MoonshineModuleTarget::Dom, &["BindValue"]).unwrap();
        assert!(!output.contains("__crepusEachBinding"), "{output}");
        assert!(output.contains("BindValue={row.value}"), "{output}");
        assert!(output.contains("<p>{row().value}</p>"), "{output}");
    }

    #[test]
    fn typed_binding_guards_still_allow_member_writes() {
        let allowed = r#"function App(){type Row={value:number};const rows:Row[]=[];return <Each header={"rows as row:Row"} children={()=> <p>{row.value++}</p>}/>;}"#;
        assert!(checked_output(allowed, MoonshineModuleTarget::Dom).contains("row().value++"));
        let rejected = r#"function App(){type Row={value:number};const rows:Row[]=[];return <Each header={"rows as row:Row"} children={()=> <p>{row={value:2}}</p>}/>;}"#;
        assert!(transform(rejected, "Each", MoonshineModuleTarget::Dom)
            .unwrap_err()
            .contains("cannot be reassigned"));
        let rejected_object = r#"function App(){type Row={value:number};const rows:Row[]=[];return <Each header={"rows as {value}:Row"} children={()=> <p>{value=2}</p>}/>;}"#;
        assert!(
            transform(rejected_object, "Each", MoonshineModuleTarget::Dom)
                .unwrap_err()
                .contains("cannot be reassigned")
        );
    }

    #[test]
    fn headers_preserve_expression_key_and_index() {
        assert_eq!(
            header("rows as row, index (row.id)").unwrap(),
            Header {
                iterable: "rows".into(),
                item: "row".into(),
                object: false,
                annotation: None,
                index: Some("index".into()),
                key: Some("row.id".into())
            }
        );
        assert_eq!(
            header("(rows as Row[]) as row").unwrap().iterable,
            "(rows as Row[])"
        );
        assert_eq!(
            header("rows // collection\n as row // binding")
                .unwrap()
                .item,
            "row"
        );
        for supported in [
            "rows as {id=1}",
            "rows as {nested:{id=1}}",
            "rows as {id,...rest}",
            "rows as [item]",
        ] {
            assert!(header(supported).is_ok(), "{supported}");
        }
        for invalid in [
            "rows",
            "rows as {[key]:value}",
            "rows as {0:value}",
            "rows as item,...rest",
            "rows as item=1",
            "rows as item,index,third",
        ] {
            assert!(header(invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn row_reads_are_scope_aware_and_server_never_evaluates_keys() {
        let source = r#"function App(){return <Each header={"rows as row, index (key(row))"} children={()=> <div title={row.label}>{(()=>{const row=1;return row})()}{index}{({row})}</div>} fallback={()=> <p>empty</p>}/>;}"#;
        let dom = transform(source, "Each", MoonshineModuleTarget::Dom).unwrap();
        assert!(dom.contains("row().label"), "{dom}");
        assert!(dom.contains("(row, index)=>(key(row)\n)"), "{dom}");
        assert!(dom.contains("const row=1;return row"), "{dom}");
        assert!(dom.contains("row: row()"), "{dom}");
        assert!(dom.contains("createRenderMemo"));
        let ssr = transform(source, "Each", MoonshineModuleTarget::Ssr).unwrap();
        assert!(ssr.contains("false && __crepusEachKey("), "{ssr}");
        assert!(ssr.contains("key(row)"), "{ssr}");
        assert!(!ssr.contains(" key={"), "{ssr}");
        assert!(!ssr.contains("mapArray"));
    }
    #[test]
    fn source_each_marks_dom_rows_and_fallback_without_marking_server() {
        let source = r#"function App(){return <Each header={"rows as row"} children={()=> <p>{row}</p>} fallback={()=> <i/>}/>;}"#;
        let dom = transform(source, "Each", MoonshineModuleTarget::Dom).unwrap();
        assert!(
            dom.contains("markRuneSourceEffect as __crepusEachSourceEffect"),
            "{dom}"
        );
        assert!(
            dom.contains("__crepusEachSourceEffect();return props.children(item,index)"),
            "{dom}"
        );
        assert!(
            dom.contains("__crepusEachSourceEffect();return __crepusEachFallback()"),
            "{dom}"
        );
        let server = transform(source, "Each", MoonshineModuleTarget::Ssr).unwrap();
        assert!(!server.contains("markRuneSourceEffect"), "{server}");
        assert!(!server.contains("__crepusEachRowFactory"), "{server}");
    }

    #[test]
    fn keyed_dom_uses_optional_runtime_but_unkeyed_and_server_do_not() {
        let keyed = r#"function App(){return <Each header={"rows as row,i (getKey(row,i))"} children={()=> <p>{row}{i}</p>}/>;}"#;
        let dom = transform(keyed, "Each", MoonshineModuleTarget::Dom).unwrap();
        assert!(dom.contains("from '@tschk/moonshine-svelte/each'"), "{dom}");
        assert!(dom.contains("keyedEach as __crepusEachKeyed"), "{dom}");
        assert!(dom.contains("U extends __crepusEachChild"), "{dom}");
        assert!(
            dom.contains("props.each,props.key,__crepusEachRowFactory,__crepusEachFallbackFactory"),
            "{dom}"
        );
        assert!(!dom.contains("mapArray"), "{dom}");
        assert!(!dom.contains("new globalThis.Map"), "{dom}");
        let unkeyed = transform(
            &keyed.replace(" (getKey(row,i))", ""),
            "Each",
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert!(
            !unkeyed.contains("@tschk/moonshine-svelte/each"),
            "{unkeyed}"
        );
        assert!(!unkeyed.contains("DOMChild"), "{unkeyed}");
        assert!(unkeyed.contains("indexArray"), "{unkeyed}");
        let ssr = transform(keyed, "Each", MoonshineModuleTarget::Ssr).unwrap();
        assert!(ssr.contains("false && __crepusEachKey("), "{ssr}");
        assert!(ssr.contains("getKey(row,i)"), "{ssr}");
        assert!(!ssr.contains("@tschk/moonshine-svelte/each"), "{ssr}");
        assert!(!ssr.contains("DOMChild"), "{ssr}");
    }
    #[test]
    fn optional_runtime_imports_do_not_collide_with_authored_bindings() {
        let source = r#"const __crepusEachKeyed=1;type __crepusEachChild=number;function App(){return <Each header={"rows as row (row.id)"} children={()=> <p>{row.id}</p>}/>;}"#;
        let dom = transform(source, "Each", MoonshineModuleTarget::Dom).unwrap();
        assert!(dom.contains("keyedEach as __crepusEachKeyed_"), "{dom}");
        assert!(dom.contains("DOMChild as __crepusEachChild_"), "{dom}");
    }
    #[test]
    fn keyed_preparation_names_do_not_capture_authored_bindings() {
        let source = r#"const __crepusEachPrepare=1,__crepusEachPrepared=2;function App(){return <Each header={"rows as row (row.id)"} children={()=> <p>{row.id}</p>}/>;}"#;
        let dom = transform(source, "Each", MoonshineModuleTarget::Dom).unwrap();
        assert!(
            dom.contains("prepareInsertionRange as __crepusEachPrepare_"),
            "{dom}"
        );
        assert!(dom.contains("const __crepusEachPrepared_"), "{dom}");
    }
}

#[cfg(test)]
mod early_name_regression_tests {
    #[test]
    fn early_name_sources_expose_decoded_item_index_key_and_iterable_names() {
        use super::*;
        let source = r#"<><Each header={"\\u005f_crepusRows as {value:\\u005f_CrepusEachBlock0}, \\u005f_crepusIndex (\\u005f_crepusKey(value))"} children={()=>null}/><Other header={"invalid"}/></>"#;
        let sources =
            name_sources(source, "Each", crate::svelte_javascript::IrPolicy::Typed).unwrap();
        assert_eq!(sources.len(), 1);
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &sources[0], SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{}", sources[0]);
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let names: Vec<_> = semantic
            .nodes()
            .iter()
            .filter_map(|node| match node.kind() {
                AstKind::BindingIdentifier(id) => Some(id.name.as_str()),
                AstKind::IdentifierReference(id) => Some(id.name.as_str()),
                _ => None,
            })
            .collect();
        for name in [
            "__crepusRows",
            "__CrepusEachBlock0",
            "__crepusIndex",
            "__crepusKey",
        ] {
            assert!(names.contains(&name), "{names:?}");
        }
        assert!(name_sources(
            r#"<Each header={"rows as [value]"}/>"#,
            "Each",
            crate::svelte_javascript::IrPolicy::Typed
        )
        .is_ok());
        assert!(
            name_sources("<Each/>", "Each", crate::svelte_javascript::IrPolicy::Typed).is_err()
        );
    }
}
