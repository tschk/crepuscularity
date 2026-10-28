//! AST-owned selector participation and compile-only intrinsic scope classes.
use raffia::ast::*;
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug)]
pub struct Element {
    pub tag: Option<String>,
    pub classes: Option<Vec<String>>,
    pub id: Option<Option<String>>,
    pub parent: Option<usize>,
    pub previous: Option<usize>,
    pub optional: bool,
}
#[derive(Debug)]
pub struct StylePlan {
    pub css: String,
    pub participants: BTreeSet<usize>,
}
#[derive(Debug)]
enum Test<'s> {
    Tag(&'s str),
    Class(&'s str),
    Id(&'s str),
    Any,
}
#[derive(Debug)]
struct Part<'s> {
    tests: Vec<Test<'s>>,
    global: bool,
    end: usize,
    universal: Option<(usize, usize)>,
}
fn ident<'a>(name: &'a InterpolableIdent<'_>) -> Result<&'a str, String> {
    match name {
        InterpolableIdent::Literal(name) => Ok(name.name.as_ref()),
        _ => Err("interpolated selector unsupported".into()),
    }
}
fn part<'a>(compound: &'a CompoundSelector<'_>) -> Result<Part<'a>, String> {
    let mut result = Part {
        tests: Vec::new(),
        global: false,
        end: compound.span.start,
        universal: None,
    };
    if let [SimpleSelector::PseudoClass(pseudo)] = compound.children.as_slice() {
        if ident(&pseudo.name)? == "global" {
            let Some(arg) = &pseudo.arg else {
                return Err("global block handled separately".into());
            };
            let PseudoClassSelectorArgKind::SelectorList(list) = &arg.kind else {
                return Err("global argument unsupported".into());
            };
            let [selector] = list.selectors.as_slice() else {
                return Err("global selector list unsupported".into());
            };
            let [ComplexSelectorChild::CompoundSelector(inner)] = selector.children.as_slice()
            else {
                return Err("complex global argument not in initial matcher".into());
            };
            result = part(inner)?;
            result.global = true;
            return Ok(result);
        }
    }
    for simple in &compound.children {
        // Pseudo predicates may change as the DOM changes.
        // Their ordinary type/class/id prefix still controls participation.
        match simple {
            SimpleSelector::PseudoClass(pseudo)
                if pseudo.arg.is_none()
                    && [
                        "hover",
                        "active",
                        "focus",
                        "focus-visible",
                        "focus-within",
                        "checked",
                        "disabled",
                        "enabled",
                        "required",
                        "optional",
                        "read-only",
                        "read-write",
                        "valid",
                        "invalid",
                        "indeterminate",
                        "placeholder-shown",
                        "first-child",
                        "last-child",
                        "only-child",
                        "first-of-type",
                        "last-of-type",
                        "only-of-type",
                        "empty",
                        "before",
                        "after",
                    ]
                    .iter()
                    .any(|name| {
                        ident(&pseudo.name).is_ok_and(|actual| actual.eq_ignore_ascii_case(name))
                    }) =>
            {
                continue;
            }
            // Nth filters include foreign siblings. Keep their selector bytes
            // unscoped, as Svelte does; only the owned subject participates.
            SimpleSelector::PseudoClass(pseudo)
                if pseudo
                    .arg
                    .as_ref()
                    .is_some_and(|arg| matches!(&arg.kind, PseudoClassSelectorArgKind::Nth(_)))
                    && [
                        "nth-child",
                        "nth-last-child",
                        "nth-of-type",
                        "nth-last-of-type",
                    ]
                    .iter()
                    .any(|name| {
                        ident(&pseudo.name).is_ok_and(|actual| actual.eq_ignore_ascii_case(name))
                    }) =>
            {
                continue;
            }
            SimpleSelector::PseudoElement(pseudo)
                if pseudo.arg.is_none()
                    && ["before", "after"].iter().any(|name| {
                        ident(&pseudo.name).is_ok_and(|actual| actual.eq_ignore_ascii_case(name))
                    }) =>
            {
                continue;
            }
            _ => {}
        }
        result.universal = None;
        result.tests.push(match simple {
            SimpleSelector::Class(value) => {
                result.end = value.span.end;
                Test::Class(ident(&value.name)?)
            }
            SimpleSelector::Id(value) => {
                result.end = value.span.end;
                Test::Id(ident(&value.name)?)
            }
            SimpleSelector::Type(TypeSelector::TagName(value)) if value.name.prefix.is_none() => {
                result.end = value.span.end;
                Test::Tag(ident(&value.name.name)?)
            }
            SimpleSelector::Type(TypeSelector::Universal(value)) if value.prefix.is_none() => {
                result.end = value.span.end;
                result.universal = Some((value.span.start, value.span.end));
                Test::Any
            }
            _ => return Err("unsupported scoped CSS selector form (analysis pending)".into()),
        });
    }
    Ok(result)
}
fn matches(part: &Part<'_>, element: &Element) -> bool {
    part.tests.iter().all(|test| match test {
        Test::Tag(tag) => element
            .tag
            .as_deref()
            .is_none_or(|value| tag.eq_ignore_ascii_case(value)),
        Test::Class(class) => element
            .classes
            .as_ref()
            .is_none_or(|classes| classes.iter().any(|value| value == class)),
        Test::Id(id) => element
            .id
            .as_ref()
            .is_none_or(|value| value.as_deref() == Some(*id)),
        Test::Any => true,
    })
}
#[cfg(test)]
thread_local! {static PREFIX_EVALUATIONS:std::cell::Cell<usize>=const {std::cell::Cell::new(0)};}
// Cache successful/failed prefixes, rather than enumerating every matching
// ancestor path. Every reached state already has a matching right-hand chain.
fn prefix_matches(
    parts: &[Part<'_>],
    combinators: &[CombinatorKind],
    at: usize,
    index: usize,
    graph: &[Element],
    memo: &mut HashMap<(usize, usize), bool>,
    found: &mut BTreeSet<usize>,
) -> bool {
    if let Some(success) = memo.get(&(at, index)) {
        return *success;
    }
    #[cfg(test)]
    PREFIX_EVALUATIONS.with(|counter| counter.set(counter.get() + 1));
    let mut success = matches(&parts[at], &graph[index]);
    if success && at > 0 {
        let mut candidates = Vec::new();
        match combinators[at - 1] {
            CombinatorKind::Child => candidates.extend(graph[index].parent),
            CombinatorKind::NextSibling => {
                let mut next = graph[index].previous;
                while let Some(index) = next {
                    candidates.push(index);
                    if !graph[index].optional {
                        break;
                    }
                    next = graph[index].previous
                }
            }
            CombinatorKind::Descendant => {
                let mut next = graph[index].parent;
                while let Some(index) = next {
                    candidates.push(index);
                    next = graph[index].parent
                }
            }
            CombinatorKind::LaterSibling => {
                let mut next = graph[index].previous;
                while let Some(index) = next {
                    candidates.push(index);
                    next = graph[index].previous
                }
            }
            CombinatorKind::Column => unreachable!("validated before graph traversal"),
        }
        let external_prefix = parts[..at].iter().all(|part| part.global);
        let may_reach_external = match combinators[at - 1] {
            CombinatorKind::Descendant => true,
            CombinatorKind::Child => graph[index].parent.is_none(),
            // Root elements may have global siblings outside this component.
            CombinatorKind::NextSibling | CombinatorKind::LaterSibling => {
                graph[index].parent.is_none()
            }
            CombinatorKind::Column => false,
        };
        success = external_prefix && may_reach_external;
        // Do not short-circuit: every successful candidate owns participation,
        // even when a different ancestor already satisfied this selector.
        for candidate in candidates {
            success |= prefix_matches(parts, combinators, at - 1, candidate, graph, memo, found);
        }
    }
    if success {
        found.insert(index);
    }
    memo.insert((at, index), success);
    success
}
/// Caller supplies a deterministic target-independent hash and an acyclic intrinsic graph.
fn plan(
    source: &str,
    sheet: &Stylesheet<'_>,
    hash: &str,
    graph: &[Element],
) -> Result<StylePlan, String> {
    for (index, element) in graph.iter().enumerate() {
        if element.parent.is_some_and(|value| value >= index)
            || element.previous.is_some_and(|value| value >= index)
        {
            return Err("invalid element graph".into());
        }
        if element
            .previous
            .is_some_and(|value| graph[value].parent != element.parent)
        {
            return Err("invalid sibling relation".into());
        }
    }
    let mut participants = BTreeSet::new();
    let mut edits = Vec::new();
    for statement in &sheet.statements {
        let edit_start = edits.len();
        let mut rule_used = false;
        let Statement::QualifiedRule(rule) = statement else {
            return Err("unsupported scoped CSS at-rule (analysis pending)".into());
        };
        if rule
            .block
            .statements
            .iter()
            .any(|statement| !matches!(statement, Statement::Declaration(_)))
        {
            return Err("unsupported scoped CSS nesting (analysis pending)".into());
        }
        super::declarations(&rule.block)?;
        for selector in &rule.selector.selectors {
            let mut parts = Vec::new();
            let mut combinators = Vec::new();
            for child in &selector.children {
                match child {
                    ComplexSelectorChild::CompoundSelector(compound) => parts.push(part(compound)?),
                    ComplexSelectorChild::Combinator(combinator) => {
                        if matches!(combinator.kind, CombinatorKind::Column) {
                            return Err("column combinator pending".into());
                        }
                        combinators.push(combinator.kind.clone())
                    }
                }
            }
            if parts.is_empty() || combinators.len() + 1 != parts.len() {
                return Err("relative selector pending".into());
            }
            let first = parts.iter().position(|part| !part.global);
            let last = parts.iter().rposition(|part| !part.global);
            if first.is_none() {
                rule_used = true;
            }
            if let (Some(first), Some(last)) = (first, last) {
                if parts[first..=last].iter().any(|part| part.global) {
                    return Err("global placement in selector middle unsupported".into());
                }
                let local = &parts[first..=last];
                let relations = &combinators[..last];
                let mut found = BTreeSet::new();
                let mut memo = HashMap::new();
                for index in 0..graph.len() {
                    prefix_matches(
                        &parts[..=last],
                        relations,
                        last,
                        index,
                        graph,
                        &mut memo,
                        &mut found,
                    );
                }
                // Partially used selector lists retain scoped barriers; entirely unused
                // rules are removed below without touching declarations.
                rule_used |= !found.is_empty();
                participants.extend(found);
                for (ordinal, part) in local.iter().enumerate() {
                    let modifier = if ordinal == 0 {
                        format!(".{hash}")
                    } else {
                        format!(":where(.{hash})")
                    };
                    if let Some((start, end)) = part.universal {
                        edits.push((start, end, modifier))
                    } else {
                        edits.push((part.end, part.end, modifier))
                    }
                }
            }
            for child in &selector.children {
                let ComplexSelectorChild::CompoundSelector(compound) = child else {
                    continue;
                };
                if let [SimpleSelector::PseudoClass(pseudo)] = compound.children.as_slice() {
                    if ident(&pseudo.name)? == "global" {
                        let arg = pseudo.arg.as_ref().ok_or("global block pending")?;
                        edits.push((pseudo.span.start, arg.l_paren.end, String::new()));
                        edits.push((arg.r_paren.start, pseudo.span.end, String::new()));
                    }
                }
            }
        }
        if !rule_used {
            edits.truncate(edit_start);
            edits.push((rule.span.start, rule.span.end, String::new()));
        }
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut css = source.to_owned();
    for (start, end, replacement) in edits.into_iter().rev() {
        css.replace_range(start..end, &replacement)
    }
    Ok(StylePlan { css, participants })
}

use oxc_allocator::Allocator;
use oxc_ast::{ast as js, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser as JsParser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};

struct Collected {
    element: Element,
    insertion: usize,
    uncertain_relation: bool,
}
struct Collector {
    elements: Vec<Collected>,
    parent: Option<usize>,
    previous: Option<usize>,
    uncertain: bool,
    dynamic: Option<String>,
    error: Option<String>,
}
impl<'a> Visit<'a> for Collector {
    fn visit_jsx_element(&mut self, node: &js::JSXElement<'a>) {
        if matches!(
            node.opening_element.name,
            js::JSXElementName::NamespacedName(_)
        ) {
            self.error =
                Some("scoped CSS dynamic/namespaced intrinsic requires owned provenance".into());
            return;
        }
        let parent = self.parent;
        let previous = self.previous;
        let uncertain = self.uncertain;
        let name = &node.opening_element.name;
        let dynamic = match name {
            js::JSXElementName::Identifier(id) => Some(id.name.as_str()),
            js::JSXElementName::IdentifierReference(id) => Some(id.name.as_str()),
            _ => None,
        }
        .is_some_and(|value| self.dynamic.as_deref() == Some(value));
        let tag = if dynamic {
            Some(None)
        } else if let js::JSXElementName::Identifier(name) = name {
            Some(Some(name.name.to_string()))
        } else {
            None
        };
        if let Some(tag) = tag {
            let mut classes = Some(Vec::new());
            let mut id = Some(None);
            for attribute in &node.opening_element.attributes {
                let js::JSXAttributeItem::Attribute(attribute) = attribute else {
                    self.error = Some(
                        "scoped Svelte intrinsic spreads are unsupported without owned class merging".into(),
                    );
                    continue;
                };
                let js::JSXAttributeName::Identifier(name) = &attribute.name else {
                    continue;
                };
                if name.name == "className" {
                    classes = match &attribute.value {
                        Some(js::JSXAttributeValue::StringLiteral(value)) => {
                            Some(value.value.split_whitespace().map(str::to_owned).collect())
                        }
                        _ => None,
                    };
                }
                if name.name == "id" {
                    id = match &attribute.value {
                        Some(js::JSXAttributeValue::StringLiteral(value)) => {
                            Some(Some(value.value.to_string()))
                        }
                        _ => None,
                    };
                }
            }
            let index = self.elements.len();
            self.elements.push(Collected {
                element: Element {
                    tag,
                    classes,
                    id,
                    parent,
                    previous,
                    optional: dynamic,
                },
                insertion: node.opening_element.name.span().end as usize,
                uncertain_relation: uncertain,
            });
            self.parent = Some(index);
            self.previous = None;
            for child in &node.children {
                self.visit_jsx_child(child)
            }
            self.previous = Some(index);
        } else {
            // Authored child content can be placed by its consumer; helper/branch
            // wrappers can repeat or remove rows. A combinator needs that analysis.
            self.parent = None;
            self.previous = None;
            self.uncertain = true;
            // Authored each bodies/fallbacks and component snippets are serialized
            // as JSX-valued attributes, not only element children.
            for attribute in &node.opening_element.attributes {
                self.visit_jsx_attribute_item(attribute);
            }

            for child in &node.children {
                self.visit_jsx_child(child)
            }
            self.previous = previous;
        }
        self.parent = parent;
        self.uncertain = uncertain;
    }
    fn visit_jsx_expression_container(&mut self, node: &js::JSXExpressionContainer<'a>) {
        let uncertain = self.uncertain;
        self.uncertain = true;
        walk::walk_jsx_expression_container(self, node);
        self.uncertain = uncertain;
    }
    fn visit_jsx_fragment(&mut self, node: &js::JSXFragment<'a>) {
        for child in &node.children {
            self.visit_jsx_child(child)
        }
    }
    fn visit_function(&mut self, node: &js::Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        let parent = self.parent;
        let previous = self.previous;
        self.parent = None;
        self.previous = None;
        walk::walk_function(self, node, flags);
        self.parent = parent;
        self.previous = previous;
    }
}
/// Mark authored intrinsics before synthetic component/control transformations.
pub(crate) fn prepare(
    source: &str,
    css: &str,
    sheet: &Stylesheet<'_>,
    marker: &str,
    hash: &str,
    dynamic_marker: Option<&str>,
) -> Result<(String, String), String> {
    let allocator = Allocator::default();
    let parsed = JsParser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid Svelte scoped CSS markup".into());
    }
    let mut collector = Collector {
        elements: Vec::new(),
        parent: None,
        previous: None,
        uncertain: false,
        dynamic: dynamic_marker.map(str::to_owned),
        error: None,
    };
    collector.visit_program(&parsed.program);
    if let Some(error) = collector.error {
        return Err(error);
    }
    let graph = collector
        .elements
        .iter()
        .map(|entry| entry.element.clone())
        .collect::<Vec<_>>();
    let styled = stacker::grow(8 * 1024 * 1024, || plan(css, sheet, hash, &graph))?;
    // Simple selectors are sound across unknown component placement; relational
    // selectors require the branch/slot relation graph in the following slice.
    let relational=sheet.statements.iter().any(|statement|matches!(statement,Statement::QualifiedRule(rule) if rule.selector.selectors.iter().any(|selector|selector.children.len()>1)));
    if relational
        && collector
            .elements
            .iter()
            .any(|entry| entry.uncertain_relation)
    {
        return Err("unsupported scoped CSS combinators across Svelte branches or component children (relation analysis pending)".into());
    }
    let mut output = source.to_owned();
    for index in styled.participants.iter().rev() {
        output.insert_str(collector.elements[*index].insertion, &format!(" {marker}"))
    }
    Ok((output, styled.css))
}
/// Consume compile-only attributes before generic factory props are created.
#[cfg(test)]
fn consume(source: &str, marker: &str, hash: &str) -> Result<Option<String>, String> {
    consume_with_directives(source, marker, hash, None)
}

pub(crate) fn consume_with_directives(
    source: &str,
    marker: &str,
    hash: &str,
    directive_helper: Option<&str>,
) -> Result<Option<String>, String> {
    let allocator = Allocator::default();
    let parsed = JsParser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid Svelte scoped-class JSX".into());
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut helper = "__crepusSvelteScopedClass".to_owned();
    while source.contains(&helper) {
        helper.push('_')
    }
    let mut edits = Vec::new();
    let mut used = false;
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        let attributes = &element.opening_element.attributes;
        let Some(scope)=attributes.iter().find_map(|attribute|match attribute{js::JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,js::JSXAttributeName::Identifier(name) if name.name==marker)=>Some(attribute),_=>None})else{continue};
        if !matches!(
            element.opening_element.name,
            js::JSXElementName::Identifier(_)
        ) {
            return Err("scoped CSS marker reached non-intrinsic component".into());
        }
        if attributes
            .iter()
            .any(|attribute| matches!(attribute, js::JSXAttributeItem::SpreadAttribute(_)))
        {
            return Err("scoped CSS intrinsic spread merge unsupported".into());
        }
        let class=attributes.iter().find_map(|attribute|match attribute{js::JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,js::JSXAttributeName::Identifier(name) if name.name=="className"||name.name=="class")=>Some(attribute),_=>None});
        if let Some(class) = class {
            edits.push((
                scope.span.start as usize,
                scope.span.end as usize,
                String::new(),
            ));
            let replacement = match &class.value {
                Some(js::JSXAttributeValue::StringLiteral(value)) => {
                    serde_json::to_string(format!("{} {hash}", value.value).trim())
                        .map_err(|error| error.to_string())?
                }
                Some(js::JSXAttributeValue::ExpressionContainer(container)) => {
                    if let js::JSXExpression::CallExpression(call) = &container.expression {
                        if matches!(&call.callee, js::Expression::Identifier(name) if Some(name.name.as_str()) == directive_helper)
                        {
                            if call.arguments.len() != 3
                                || !matches!(
                                    call.arguments[1].as_expression(),
                                    Some(js::Expression::NullLiteral(_))
                                )
                            {
                                return Err("invalid Svelte class directive scope arguments".into());
                            }
                            // The scope hash precedes flags so a false flag can remove it.
                            let span = call.arguments[1].span();
                            edits.push((
                                span.start as usize,
                                span.end as usize,
                                serde_json::to_string(hash).map_err(|error| error.to_string())?,
                            ));
                            continue;
                        }
                    }
                    used = true;
                    let span = container.expression.span();
                    format!(
                        "{{((value:string|undefined)=>value?value+\" {hash}\":\"{hash}\")({helper}({}))}}",
                        &source[span.start as usize..span.end as usize]
                    )
                }
                None => format!("\"true {hash}\""),
                _ => return Err("scoped CSS class value unsupported".into()),
            };
            let start = class
                .value
                .as_ref()
                .map_or(class.span.end, |value| value.span().start)
                as usize;
            edits.push((
                start,
                class.span.end as usize,
                if class.value.is_none() {
                    format!("={replacement}")
                } else {
                    replacement
                },
            ));
        } else {
            edits.push((
                scope.span.start as usize,
                scope.span.end as usize,
                format!("className=\"{hash}\""),
            ));
        }
    }
    if edits.is_empty() {
        return Ok(None);
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut output = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        output.replace_range(start..end, &text)
    }
    if used {
        output.push_str(&format!(
            "\nimport{{classValue as {helper}}}from\"@tschk/moonshine-svelte/class\";\n"
        ))
    }
    Ok(Some(output))
}

#[cfg(test)]
mod oxc_tests {
    use super::*;
    fn prepare(
        source: &str,
        css: &str,
        marker: &str,
        hash: &str,
    ) -> Result<(String, String), String> {
        let sheet = super::super::parse_stylesheet(css)?;
        super::prepare(source, css, &sheet, marker, hash, None)
    }
    fn prepare_dynamic(
        source: &str,
        css: &str,
        marker: &str,
        hash: &str,
    ) -> Result<(String, String), String> {
        let sheet = super::super::parse_stylesheet(css)?;
        super::prepare(source, css, &sheet, marker, hash, Some("Dyn"))
    }
    #[test]
    fn class_directives_receive_hash_before_flags_without_outer_normalization() {
        let source = r#"const v=<div Marker className={Classes(base,null,{active:flag})}/>;"#;
        let output = consume_with_directives(source, "Marker", "scope", Some("Classes"))
            .unwrap()
            .unwrap();
        assert!(
            output.contains(r#"Classes(base,"scope",{active:flag})"#),
            "{output}"
        );
        assert!(!output.contains("Marker"));
        assert!(!output.contains("classValue"));
        assert!(!output.contains("import"));
    }

    #[test]
    fn class_directive_scope_rejects_malformed_generated_call() {
        for args in ["base", "base,hash,{}", "base,null,{},extra"] {
            let source = format!("const v=<div Marker className={{Classes({args})}}/>;");
            assert!(consume_with_directives(&source, "Marker", "scope", Some("Classes")).is_err());
        }
    }

    #[test]
    fn dynamic_marker_matches_any_type_selector_and_stays_scoped() {
        for source in ["const v=<Dyn/>;", "const v=<Dyn this={\"div\"}/>;"] {
            let (marked, css) =
                prepare_dynamic(source, "p{color:red}", "data-scope", "scope").unwrap();
            assert_eq!(css, "p.scope{color:red}");
            assert!(marked.contains("<Dyn data-scope"), "{marked}");
        }
        let (marked, css) =
            prepare_dynamic("const v=<Dyn/>;", "*{color:red}", "data-scope", "scope").unwrap();
        assert_eq!(css, ".scope{color:red}");
        assert_eq!(marked.matches("data-scope").count(), 1);
    }
    #[test]
    fn dynamic_marker_without_option_is_an_ordinary_component() {
        let (marked, css) =
            prepare("const v=<Dyn/>;", "p{color:red}", "data-scope", "scope").unwrap();
        assert!(css.is_empty());
        assert!(!marked.contains("data-scope"));
    }
    #[test]
    fn dynamic_marker_class_and_id_retain_static_and_dynamic_matching() {
        let (_, css) = prepare_dynamic(
            "const v=<Dyn className=\"a\"/>;",
            ".a{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert_eq!(css, ".a.scope{color:red}");
        let (marked, css) = prepare_dynamic(
            "const v=<Dyn className=\"b\"/>;",
            ".a{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert!(css.is_empty());
        assert!(!marked.contains(" m"));
        let (_, css) = prepare_dynamic(
            "const v=<Dyn className=\"b\"/>;",
            "p.a{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert!(css.is_empty());
        let (_, css) = prepare_dynamic(
            "const v=<Dyn className={c}/>;",
            ".a{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert_eq!(css, ".a.scope{color:red}");
        let (_, css) =
            prepare_dynamic("const v=<Dyn id=\"i\"/>;", "#i{color:red}", "m", "scope").unwrap();
        assert_eq!(css, "#i.scope{color:red}");
        let (_, css) =
            prepare_dynamic("const v=<Dyn id=\"i\"/>;", "#j{color:red}", "m", "scope").unwrap();
        assert!(css.is_empty());
        let (_, css) =
            prepare_dynamic("const v=<Dyn id={i}/>;", "#j{color:red}", "m", "scope").unwrap();
        assert_eq!(css, "#j.scope{color:red}");
    }
    #[test]
    fn dynamic_marker_is_a_parent_and_child_for_combinators() {
        let (marked, css) = prepare_dynamic(
            "const v=<div><Dyn/></div>;",
            "div > p{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert_eq!(css, "div.scope > p:where(.scope){color:red}");
        assert_eq!(marked.matches(" m").count(), 2);
        let (_, css) = prepare_dynamic(
            "const v=<section><Dyn/></section>;",
            "div > p{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert!(css.is_empty());
        let (marked, css) = prepare_dynamic(
            "const v=<Dyn><span/></Dyn>;",
            "div span{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert_eq!(css, "div.scope span:where(.scope){color:red}");
        assert_eq!(marked.matches(" m").count(), 2);
        let (_, css) = prepare_dynamic(
            "const v=<Dyn><span/></Dyn>;",
            "div > span{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert_eq!(css, "div.scope > span:where(.scope){color:red}");
        let (_, css) = prepare_dynamic(
            "const v=<Dyn><b><span/></b></Dyn>;",
            "div > span{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert!(css.is_empty());
        let (_, css) = prepare_dynamic(
            "const v=<Dyn className=\"w\"><span/></Dyn>;",
            ".z span{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert!(css.is_empty());
    }
    #[test]
    fn owned_global_prefix_matches_receive_scope_without_rewriting_global_selector() {
        for source in [
            "const v=<><i className=\"outside\"/><b/></>;",
            "const v=<div><i className=\"outside\"/><b/></div>;",
        ] {
            let (marked, css) =
                prepare_dynamic(source, ":global(.outside) + b{color:red}", "m", "scope").unwrap();
            assert!(marked.contains("<i m className=\"outside\""), "{marked}");
            assert!(marked.contains("<b m"), "{marked}");
            assert_eq!(css, ".outside + b.scope{color:red}");
        }
        let (marked, css) = prepare_dynamic(
            "const v=<i className=\"outside\"/>;",
            ":global(.outside){color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert!(!marked.contains(" m"));
        assert_eq!(css, ".outside{color:red}");
    }

    #[test]
    fn global_sibling_prefix_is_conservative_for_every_root_only() {
        for combinator in ["+", "~"] {
            let selector = format!(":global(.outside) {combinator} b{{color:red}}");
            for source in [
                "const v=<><i/><b/></>;",
                "const v=<><i/><Dyn/><b/></>;",
                "const v=<><Dyn/><i/><b/></>;",
            ] {
                let (marked, css) = prepare_dynamic(source, &selector, "m", "scope").unwrap();
                assert_eq!(css, format!(".outside {combinator} b.scope{{color:red}}"));
                assert!(marked.contains("<b m"), "{marked}");
                assert!(!marked.contains("<i m"), "{marked}");
            }
            let (marked, css) =
                prepare_dynamic("const v=<div><Dyn/><b/></div>;", &selector, "m", "scope").unwrap();
            assert!(css.is_empty());
            assert!(!marked.contains(" m"));
        }
    }

    #[test]
    fn dynamic_marker_is_optional_for_adjacent_siblings_until_a_definite_sibling() {
        let (marked, css) = prepare_dynamic(
            "const v=<><a/><Dyn/><b/></>;",
            "a + b{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert_eq!(css, "a.scope + b:where(.scope){color:red}");
        assert_eq!(marked.matches(" m").count(), 3);
        let (marked, css) = prepare_dynamic(
            "const v=<><a/><Dyn/><i/><b/></>;",
            "a + b{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert_eq!(css, "a.scope + b:where(.scope){color:red}");
        assert_eq!(marked.matches(" m").count(), 2);
        assert!(
            marked.contains("<i/>") && marked.contains("<b/>"),
            "{marked}"
        );
        let (_, css) = prepare_dynamic(
            "const v=<><a/><i/><Dyn/></>;",
            "a + p{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert!(css.is_empty());
        let (_, css) = prepare_dynamic(
            "const v=<><a/><Dyn className=\"x\"/></>;",
            "a + .y{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert!(css.is_empty());
        let (marked, _) = prepare_dynamic(
            "const v=<><s/><Dyn/><Dyn/><b/></>;",
            "s + b{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert_eq!(marked.matches(" m").count(), 4);
        let (marked, _) = prepare_dynamic(
            "const v=<><Dyn/><Dyn/></>;",
            "p + p{color:red}",
            "m",
            "scope",
        )
        .unwrap();
        assert_eq!(marked.matches(" m").count(), 2);
    }
    #[test]
    fn state_and_generated_content_keep_owned_participation() {
        let source = "const view=<><button/><input/><p/><aside/></>;";
        let (marked, css) = prepare(source, "button:hover{color:red}input:checked{color:blue}p::before{content:'B'}p:after{content:'A'}", "data-scope", "scope").unwrap();
        assert_eq!(marked.matches("data-scope").count(), 3);
        assert_eq!(
            css,
            "button.scope:hover{color:red}input.scope:checked{color:blue}p.scope::before{content:'B'}p.scope:after{content:'A'}"
        );
        let (marked, css) = prepare(
            "const view=<p/>;",
            "button:hover{color:red}",
            "data-scope",
            "scope",
        )
        .unwrap();
        assert!(!marked.contains("data-scope"));
        assert!(css.is_empty());
        let (marked, css) = prepare(
            "const view=<p/>;",
            "p:checked{color:red}",
            "data-scope",
            "scope",
        )
        .unwrap();
        assert_eq!(marked.matches("data-scope").count(), 1);
        assert_eq!(css, "p.scope:checked{color:red}");
    }
    #[test]
    fn verified_state_names_remain_potential_matches() {
        for name in [
            "HOVER",
            "fOcUs",
            "hover",
            "active",
            "focus",
            "focus-visible",
            "focus-within",
            "checked",
            "disabled",
            "enabled",
            "required",
            "optional",
            "read-only",
            "read-write",
            "valid",
            "invalid",
            "indeterminate",
            "placeholder-shown",
        ] {
            let (_, css) = prepare(
                "const view=<p/>;",
                &format!("p:{name}{{color:red}}"),
                "data-scope",
                "scope",
            )
            .unwrap();
            assert_eq!(css, format!("p.scope:{name}{{color:red}}"));
        }
    }
    #[test]
    fn scope_anchor_is_last_nonpseudo_simple_selector() {
        let source = "const view=<div className='outer'><p className='x'/></div>;";
        let (_, css) = prepare(source, ".outer:hover > p:focus.x::before{content:'B'}*.x:hover{color:red}:hover{color:blue}*::after{content:'A'}", "data-scope", "scope").unwrap();
        assert_eq!(
            css,
            ".outer.scope:hover > p:focus.x:where(.scope)::before{content:'B'}*.x.scope:hover{color:red}.scope:hover{color:blue}.scope::after{content:'A'}"
        );
    }
    #[test]
    fn pseudo_case_is_preserved_without_normalizing_global_macro() {
        let (_, css) = prepare(
            "const view=<p/>;",
            "p:HOVER::BEFORE{content:'B'}p:AFTER{content:'A'}",
            "data-scope",
            "scope",
        )
        .unwrap();
        assert_eq!(
            css,
            "p.scope:HOVER::BEFORE{content:'B'}p.scope:AFTER{content:'A'}"
        );
        assert!(prepare(
            "const view=<p/>;",
            ":GLOBAL(.x){color:red}",
            "data-scope",
            "scope"
        )
        .is_err());
    }
    #[test]
    fn structural_subjects_remain_potential_matches() {
        for name in [
            "first-child",
            "last-child",
            "only-child",
            "first-of-type",
            "last-of-type",
            "only-of-type",
            "empty",
            "FIRST-CHILD",
            "EMPTY",
        ] {
            let (marked, css) = prepare(
                "const view=<><p>text</p><p>more text</p><aside/></>;",
                &format!("p:{name}{{color:red}}"),
                "Marker",
                "scope",
            )
            .unwrap();
            assert_eq!(marked.matches("Marker").count(), 2);
            assert_eq!(css, format!("p.scope:{name}{{color:red}}"));
        }
        let (marked, css) = prepare(
            "const view=<aside/>;",
            "p:empty{color:red}",
            "Marker",
            "scope",
        )
        .unwrap();
        assert!(!marked.contains("Marker"));
        assert!(css.is_empty());
    }
    #[test]
    fn nth_arguments_remain_unscoped_and_source_bytes_preserved() {
        for nth in [
            "nth-child(2n + 1)",
            "nth-last-child(2)",
            "nth-of-type(odd)",
            "nth-last-of-type(2n)",
            "NTH-CHILD(2n + 1 of .foreign)",
            "nth-child(2 of :global(.foreign))",
        ] {
            let (marked, css) = prepare(
                "const view=<><p/><Child/><aside/></>;",
                &format!("p:{nth}{{color:red}}"),
                "Marker",
                "scope",
            )
            .unwrap();
            assert_eq!(marked.matches("Marker").count(), 1);
            assert_eq!(css, format!("p.scope:{nth}{{color:red}}"));
        }
        let (_, css) = prepare(
            "const view=<p/>;",
            ":nth-child(2 of .foreign){color:red}",
            "Marker",
            "scope",
        )
        .unwrap();
        assert_eq!(css, ".scope:nth-child(2 of .foreign){color:red}");
    }
    #[test]
    fn each_subjects_are_owned_but_opaque_siblings_are_not() {
        let source="function App(){return <ul><Child/><Each children={()=> <li className=\"row\"/>}/></ul>}";
        let (marked, css) = prepare(
            source,
            ".row:nth-child(2 of .row){color:red}",
            "Marker",
            "scope",
        )
        .unwrap();
        assert_eq!(marked.matches("Marker").count(), 1);
        assert_eq!(css, ".row.scope:nth-child(2 of .row){color:red}");
        assert!(prepare(
            source,
            "ul > .row:nth-child(2){color:red}",
            "Marker",
            "scope"
        )
        .is_err());
    }
    #[test]
    fn nth_forms_do_not_admit_unowned_functional_analysis() {
        for selector in [
            "p:nth-child",
            "p:nth-last-child",
            "p:nth-of-type",
            "p:nth-last-of-type",
            "p:nth-col(2)",
            "p:is(.x)",
            "p:has(.x)",
            "p:not(.x)",
            "p:where(.x)",
        ] {
            assert!(
                prepare(
                    "const view=<p/>;",
                    &format!("{selector}{{color:red}}"),
                    "Marker",
                    "scope"
                )
                .is_err(),
                "{selector}"
            );
        }
    }
    #[test]
    fn leaf_pseudos_do_not_admit_other_selector_analysis() {
        for selector in [
            "p:first-child(x)",
            "p:empty(x)",
            "p:root",
            "p:host",
            "p:unknown",
            "p:is(.x)",
            "p:hover(x)",
            "p::before(x)",
            "p::slotted(.x)",
            ".outer:hover :global(.foreign)::before",
        ] {
            let error = prepare(
                "const view=<div className='outer'><p className='x'/></div>;",
                &format!("{selector}{{color:red}}"),
                "data-scope",
                "scope",
            )
            .unwrap_err();
            assert!(error.contains("unsupported"), "{selector}: {error}");
        }
    }
    #[test]
    fn repeated_descendant_prefixes_are_bounded_and_all_participants_remain() {
        let count = 64usize;
        let width = 32usize;
        let source = (0..width).map(|_| ".x").collect::<Vec<_>>().join(" ") + "{color:red}";
        let sheet = super::super::parse_stylesheet(&source).unwrap();
        let Statement::QualifiedRule(rule) = &sheet.statements[0] else {
            panic!("qualified rule")
        };
        let mut parts = Vec::new();
        let mut combinators = Vec::new();
        for child in &rule.selector.selectors[0].children {
            match child {
                ComplexSelectorChild::CompoundSelector(compound) => {
                    parts.push(part(compound).unwrap())
                }
                ComplexSelectorChild::Combinator(combinator) => {
                    combinators.push(combinator.kind.clone())
                }
            }
        }
        let graph = (0..count)
            .map(|index| Element {
                tag: Some("div".into()),
                classes: Some(vec!["x".into()]),
                id: Some(None),
                parent: index.checked_sub(1),
                previous: None,
                optional: false,
            })
            .collect::<Vec<_>>();
        let mut memo = HashMap::new();
        let mut found = BTreeSet::new();
        PREFIX_EVALUATIONS.with(|counter| counter.set(0));
        for index in 0..count {
            prefix_matches(
                &parts,
                &combinators,
                width - 1,
                index,
                &graph,
                &mut memo,
                &mut found,
            );
        }
        assert!(
            memo.len() <= count * width,
            "one evaluation per prefix/node state"
        );
        PREFIX_EVALUATIONS.with(|counter| {
            assert!(
                counter.get() <= count * width,
                "each prefix/node is evaluated once, got {}",
                counter.get()
            )
        });
        assert_eq!(found, (0..count).collect());
    }
    #[test]
    fn every_successful_prefix_participates_not_only_nearest_ancestor() {
        let source="function App(){return <><div className=\"a\"><section className=\"a\"><p className=\"b\"/></section></div><aside className=\"a\"/></>}";
        let (marked, _) = prepare(source, ".a .b{color:red}", "Marker", "x").unwrap();
        assert_eq!(marked.matches("Marker").count(), 3);
        assert!(marked.contains("<aside className=\"a\"/>"));
    }
    #[test]
    fn nested_authored_graphs_keep_every_scope_marker() {
        for (count, width) in [(16usize, 8), (24, 12), (32, 16)] {
            let markup = "<div className=\"x\">".repeat(count) + "text" + &"</div>".repeat(count);
            let source = format!("function App(){{return {markup}}}");
            let css = (0..width).map(|_| ".x").collect::<Vec<_>>().join(" ") + "{color:red}";
            let (marked, _) = prepare(&source, &css, "Marker", "x").unwrap();
            assert_eq!(marked.matches("Marker").count(), count);
        }
    }
    #[test]
    fn authored_intrinsics_only_marker_and_class_normalization() {
        let source="export default function App(){return <><div className=\"outer\"><span className={classes}>local</span><Child/></div><aside>unrelated</aside></>}";
        let (marked, css) = prepare(
            source,
            ".outer > span{color:red}",
            "ScopeMarker",
            "scope-v1",
        )
        .unwrap();
        assert_eq!(marked.matches("ScopeMarker").count(), 2);
        assert_eq!(css, ".outer.scope-v1 > span:where(.scope-v1){color:red}");
        let output = consume(&marked, "ScopeMarker", "scope-v1")
            .unwrap()
            .unwrap();
        assert!(!output.contains("ScopeMarker"));
        assert!(output.contains("className=\"outer scope-v1\""));
        assert!(output.contains("__crepusSvelteScopedClass(classes)"));
        assert!(output.contains("<Child/>") && output.contains("<aside>"));
    }
    #[test]
    fn provably_unused_rule_pruned_but_unknown_dynamic_class_retained() {
        let source = "function App(){return <div className=\"present\"/>}";
        let (marked, css) = prepare(source, ".absent{color:red}", "Marker", "x").unwrap();
        assert_eq!(marked, source);
        assert_eq!(css, "");
        let (marked, css) = prepare(
            "function App(){return <div className={classes}/>} ",
            ".absent{color:red}",
            "Marker",
            "x",
        )
        .unwrap();
        assert!(marked.contains("Marker"));
        assert_eq!(css, ".absent.x{color:red}");
    }
    #[test]
    fn global_suffix_reaches_opaque_child_but_local_selector_does_not() {
        let source = "function App(){return <div className=\"outer\"><Child/></div>}";
        let (marked, css) =
            prepare(source, ".outer :global(.inner){color:red}", "Marker", "x").unwrap();
        assert_eq!(marked.matches("Marker").count(), 1);
        assert_eq!(css, ".outer.x .inner{color:red}");
        let (marked, css) = prepare(source, ".outer .inner{color:red}", "Marker", "x").unwrap();
        assert_eq!(marked, source);
        assert_eq!(css, "");
    }
    #[test]
    fn global_prefix_checks_known_owned_parent_before_external_boundary() {
        let source = "function App(){return <div><span/></div>}";
        let (marked, css) =
            prepare(source, ":global(body) > span{color:red}", "Marker", "x").unwrap();
        assert_eq!(marked, source);
        assert_eq!(css, "");
        let (marked, css) = prepare(
            "function App(){return <span/>}",
            ":global(body) > span{color:red}",
            "Marker",
            "x",
        )
        .unwrap();
        assert!(marked.contains("Marker"));
        assert_eq!(css, "body > span.x{color:red}");
    }
    #[test]
    fn raw_html_is_opaque_but_global_descendant_remains_possible() {
        let source =
            "function App(){return <div className=\"outer\"><RawHtml value={html}/></div>}";
        let (marked, css) = prepare(source, ".outer .inner{color:red}", "Marker", "x").unwrap();
        assert_eq!(marked, source);
        assert!(css.is_empty());
        let (marked, css) =
            prepare(source, ".outer :global(.inner){color:red}", "Marker", "x").unwrap();
        assert_eq!(marked.matches("Marker").count(), 1);
        assert_eq!(css, ".outer.x .inner{color:red}");
        assert!(prepare(
            "function App(){return <svelte:element this={tag}/>} ",
            ".inner{color:red}",
            "Marker",
            "x"
        )
        .unwrap_err()
        .contains("provenance"));
    }
    #[test]
    fn html_type_selector_is_ascii_case_insensitive() {
        let (marked, css) = prepare(
            "function App(){return <div/>}",
            "DIV{color:red}",
            "Marker",
            "x",
        )
        .unwrap();
        assert!(marked.contains("Marker"));
        assert_eq!(css, "DIV.x{color:red}");
    }
    #[test]
    fn spreads_cannot_overwrite_scope() {
        assert!(prepare(
            "function App(){return <div {...props}/>} ",
            "div{color:red}",
            "Marker",
            "x"
        )
        .unwrap_err()
        .contains("spreads"));
        assert!(consume(
            "function App(){return <div Marker {...props}/>} ",
            "Marker",
            "x"
        )
        .unwrap_err()
        .contains("spread"));
    }
    #[test]
    fn each_attribute_body_and_fallback_keep_scope_markers() {
        let source="function App(){return <Each header={\"rows as row\"} children={()=> <p className=\"row\"/>} fallback={()=> <p className=\"row\"/>}/>}";
        let (marked, _) = prepare(source, ".row{color:red}", "Marker", "x").unwrap();
        assert_eq!(marked.matches("Marker").count(), 2);
        let consumed = consume(&marked, "Marker", "x").unwrap().unwrap();
        assert_eq!(consumed.matches("row x").count(), 2);
        assert!(!consumed.contains("Marker"));
    }
    #[test]
    fn marker_survives_wrapped_each_and_function_scopes() {
        let source = "function App(){return <Each>{()=> <div className=\"row\"/>}</Each>}";
        let (marked, _) = prepare(source, ".row{color:red}", "Marker", "x").unwrap();
        assert_eq!(marked.matches("Marker").count(), 1);
        assert!(consume(&marked, "Marker", "x")
            .unwrap()
            .unwrap()
            .contains("row x"));
    }
}
