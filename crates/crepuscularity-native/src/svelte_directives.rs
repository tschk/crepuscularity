//! Authored intrinsic directives, carried through semantic passes as explicit markers.
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

#[derive(Debug, Clone)]
pub(crate) struct Directive {
    pub attribute: String,
    /// None is an attachment; Some records an action's parameter presence.
    pub action_parameter: Option<bool>,
}

/// Erase directive closures before SSR semantic passes. Script declarations keep
/// their ordinary lifetime, but neither action lookup nor attachment/param reads
/// become SSR expressions, including when they contain nested JSX.
pub(crate) fn erase(source: &str, directives: &[Directive]) -> Result<String, String> {
    if directives.is_empty() {
        return Ok(source.to_owned());
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid authored Svelte directive expressions".into());
    }
    struct Erase<'s> {
        directives: &'s [Directive],
        spans: Vec<oxc_span::Span>,
    }
    impl<'a> Visit<'a> for Erase<'_> {
        fn visit_jsx_attribute(&mut self, attribute: &JSXAttribute<'a>) {
            if matches!(&attribute.name, JSXAttributeName::Identifier(name)
                if self.directives.iter().any(|directive| directive.attribute == name.name.as_str()))
            {
                self.spans.push(attribute.span);
            } else {
                walk::walk_jsx_attribute(self, attribute);
            }
        }
    }
    let mut erase = Erase {
        directives,
        spans: Vec::new(),
    };
    erase.visit_program(&parsed.program);
    let mut output = source.to_owned();
    for span in erase.spans.iter().rev() {
        output.replace_range(span.start as usize..span.end as usize, "");
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_erasure_removes_only_owned_markers_without_reading_closures() {
        let directives = [Directive {
            attribute: "privateMarker".into(),
            action_parameter: Some(true),
        }];
        let source = "const action=()=>{};function C(){return <p privateMarker={[(node,value)=>action(node,value),()=> <Unknown/>]} data-keep={read()}/>;}";
        let output = erase(source, &directives).unwrap();
        assert_eq!(
            output,
            "const action=()=>{};function C(){return <p  data-keep={read()}/>;}"
        );
        assert!(!output.contains("Unknown"));
        assert_eq!(erase(source, &[]).unwrap(), source);
    }
}

/// Mark authored template boundaries after CSS ownership analysis, before the
/// semantic passes rewrite branches and snippet/each parameters. Plain nested
/// fragments remain in their containing block.
pub(crate) fn mark_groups(
    source: &str,
    group: &str,
    functions: &[&str],
    controls: &[&str],
    const_scope: Option<&str>,
    dynamic_map: Option<&str>,
) -> Result<String, String> {
    use oxc_ast::AstKind;
    use oxc_semantic::SemanticBuilder;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid authored Svelte directive template".into());
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    if !built.errors.is_empty() {
        return Err("invalid authored Svelte directive template scope".into());
    }
    let nodes = built.semantic.nodes();
    let mut roots = Vec::new();
    let mut component_bodies = Vec::new();
    for node in nodes.iter() {
        let AstKind::ReturnStatement(statement) = node.kind() else {
            continue;
        };
        let Some(Expression::JSXFragment(fragment)) = &statement.argument else {
            continue;
        };
        let function = nodes
            .ancestors(node.id())
            .find_map(|ancestor| match ancestor.kind() {
                AstKind::Function(function) => {
                    Some(function.id.as_ref().map(|id| id.name.as_str()))
                }
                AstKind::ArrowFunctionExpression(_) => Some(None),
                _ => None,
            })
            .flatten();
        let const_body = const_scope.is_some_and(|marker| {
            let attribute = nodes.ancestors(node.id()).find_map(|ancestor| match ancestor.kind() {
                AstKind::JSXAttribute(attribute) => Some(attribute), _ => None,
            });
            let element = nodes.ancestors(node.id()).find_map(|ancestor| match ancestor.kind() {
                AstKind::JSXElement(element) => Some(element), _ => None,
            });
            let nearest_arrow = nodes.ancestors(node.id()).find_map(|ancestor| match ancestor.kind() {
                AstKind::ArrowFunctionExpression(arrow) => Some(Some(arrow.span)),
                AstKind::Function(_) => Some(None), _ => None,
            }).flatten();
            let direct_body = attribute.is_some_and(|attribute| matches!(&attribute.value,
                Some(JSXAttributeValue::ExpressionContainer(value)) if value.expression.as_expression()
                    .is_some_and(|value| matches!(value.get_inner_expression(), Expression::ArrowFunctionExpression(arrow) if Some(arrow.span) == nearest_arrow))));
            let matched = direct_body && matches!(attribute.map(|a| &a.name), Some(JSXAttributeName::Identifier(name)) if name.name == "body")
                && element.is_some_and(|element| matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if name.name == marker));
            if matched && element.is_some_and(|element| element.opening_element.attributes.iter().any(|item| matches!(item, JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "kind") && matches!(&attribute.value, Some(JSXAttributeValue::StringLiteral(value)) if value.value == "component")))) {
                component_bodies.push(fragment.span);
            }
            matched
        });
        if function.is_some_and(|name| functions.contains(&name)) || const_body {
            roots.push(fragment.span);
        }
    }
    let mut spans = roots.clone();
    for node in nodes.iter() {
        let AstKind::JSXFragment(fragment) = node.kind() else {
            continue;
        };
        if !roots
            .iter()
            .any(|root| root.start <= fragment.span.start && root.end >= fragment.span.end)
        {
            continue;
        }
        let parent = nodes.parent_kind(node.id());
        if matches!(parent, AstKind::ConditionalExpression(_)) {
            spans.push(fragment.span);
            continue;
        }
        if !matches!(
            parent,
            AstKind::ExpressionStatement(_) | AstKind::ArrowFunctionExpression(_)
        ) {
            continue;
        }
        if dynamic_map.is_some_and(|name| nodes.ancestors(node.id()).any(|ancestor| matches!(ancestor.kind(), AstKind::CallExpression(call) if call.callee.is_specific_id(name)))) {
            spans.push(fragment.span);
            continue;
        }
        // Each/key and authored await arms are separate template callbacks.
        let attribute = nodes
            .ancestors(node.id())
            .find_map(|ancestor| match ancestor.kind() {
                AstKind::JSXAttribute(attribute) => Some(attribute),
                _ => None,
            });
        let Some(attribute) = attribute else {
            continue;
        };
        if !matches!(&attribute.name, JSXAttributeName::Identifier(name) if matches!(name.name.as_str(), "children" | "fallback" | "initial" | "then" | "catch"))
        {
            continue;
        }
        let control = nodes
            .ancestors(node.id())
            .find_map(|ancestor| match ancestor.kind() {
                AstKind::JSXElement(element) => Some(element),
                _ => None,
            });
        if control.is_some_and(|element| {
            matches!(&element.opening_element.name,
            JSXElementName::IdentifierReference(name) if controls.contains(&name.name.as_str()))
        }) && nodes
            .ancestors(node.id())
            .take_while(|ancestor| !matches!(ancestor.kind(), AstKind::JSXAttribute(_)))
            .any(|ancestor| matches!(ancestor.kind(), AstKind::ArrowFunctionExpression(_)))
        {
            spans.push(fragment.span);
        }
    }
    if let Some(marker) = const_scope {
        // The scope must remain the existing callback's returned expression.
        // Only its body fragment is a template; wrapping the transparent outer
        // fragment would turn the scope into an owned component/range child.
        fn sole_scope(fragment: &JSXFragment<'_>, marker: &str) -> bool {
            let mut children = fragment.children.iter().filter(
                |child| !matches!(child, JSXChild::Text(text) if text.value.trim().is_empty()),
            );
            let child = children.next();
            if children.next().is_some() {
                return false;
            }
            match child {
                Some(JSXChild::Element(element)) => {
                    matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if name.name == marker)
                }
                Some(JSXChild::Fragment(fragment)) => sole_scope(fragment, marker),
                _ => false,
            }
        }
        spans.retain(|span| !nodes.iter().any(|node| matches!(node.kind(), AstKind::JSXFragment(fragment) if fragment.span == *span && sole_scope(fragment, marker))));
    }
    spans.sort_by_key(|span| (span.start, span.end));
    spans.dedup();
    let mut edits = Vec::new();
    let mut spread_bodies = Vec::new();
    for node in nodes.iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if !matches!(&element.opening_element.name, JSXElementName::Identifier(_))
            || !element
                .opening_element
                .attributes
                .iter()
                .any(|item| matches!(item, JSXAttributeItem::SpreadAttribute(_)))
            || !roots
                .iter()
                .any(|root| root.start <= element.span.start && root.end >= element.span.end)
        {
            continue;
        }
        let parent = nodes
            .ancestors(node.id())
            .find_map(|ancestor| match ancestor.kind() {
                AstKind::JSXElement(parent)
                    if matches!(
                        &parent.opening_element.name,
                        JSXElementName::IdentifierReference(_)
                            | JSXElementName::MemberExpression(_)
                    ) =>
                {
                    Some(parent)
                }
                _ => None,
            });
        let Some(parent) = parent else {
            continue;
        };
        let Some(closing) = &parent.closing_element else {
            continue;
        };
        let span = (
            parent.opening_element.span.end as usize,
            closing.span.start as usize,
        );
        if span.0 < span.1 && !spread_bodies.contains(&span) {
            spread_bodies.push(span);
        }
    }
    // Implicit component children are lazy callbacks too; preserve a template owner
    // for spread intrinsics before the component-prop pass moves their source.
    for (start, end) in spread_bodies {
        edits.push((start, start, format!("<{group}>")));
        edits.push((end, end, format!("</{group}>")));
    }
    for span in spans {
        edits.push((
            span.start as usize,
            span.start as usize + 2,
            if component_bodies.contains(&span) {
                format!("<{group} constComponent>")
            } else {
                format!("<{group}>")
            },
        ));
        edits.push((
            span.end as usize - 3,
            span.end as usize,
            format!("</{group}>"),
        ));
    }
    edits.sort_by_key(|edit| edit.0);
    let mut output = source.to_owned();
    for (start, end, replacement) in edits.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    Ok(output)
}

#[cfg(test)]
mod group_tests {
    use super::*;

    #[test]
    fn group_provenance_keeps_plain_fragments_in_their_block() {
        let source = "function Component(){return <><div><><b/></></div>{ok ? <><p/></> : <><i/></>}<Each children={()=> <><span/><><em/></></>} fallback={()=> <><u/></>}/></>;}function other(){return <><aside/></>;}";
        let output = mark_groups(source, "Group", &["Component"], &["Each"], None, None).unwrap();
        assert_eq!(output.matches("<Group>").count(), 5, "{output}");
        assert!(output.contains("<div><><b/></></div>"));
        assert!(output.contains("<span/><><em/></>"));
        assert!(output.contains("function other(){return <><aside/></>;}"));
    }
    #[test]
    fn authored_await_arm_groups_require_a_known_control_identity() {
        let source="function Component(){return <><Await initial={()=> <><p/></>} then={()=> <><b/></>} catch={()=> <><i/></>}/><Other then={()=> <><u/></>}/></>;}";
        let output = mark_groups(source, "Group", &["Component"], &["Await"], None, None).unwrap();
        assert_eq!(output.matches("<Group>").count(), 4, "{output}");
        assert!(
            output.contains("<Other then={()=> <><u/></>}/>"),
            "{output}"
        );
    }
    #[test]
    fn const_scope_outer_fragment_stays_transparent_and_body_keeps_component_shape() {
        let source="function Component(){return <><Scope kind='component' body={()=>{const x=1;return <><b>{x}</b></>}}/></>;}";
        let output =
            mark_groups(source, "Group", &["Component"], &[], Some("Scope"), None).unwrap();
        assert!(output.contains("return <><Scope"), "{output}");
        assert!(
            output.contains("return <Group constComponent><b>{x}</b></Group>"),
            "{output}"
        );
        assert_eq!(output.matches("<Group").count(), 1, "{output}");
    }
}

pub(crate) struct GroupHints<'s> {
    pub group: &'s str,
    pub bindings: &'s [String],
    pub intrinsic_reference: Option<&'s str>,
    pub dynamic_tag_attribute: Option<&'s str>,
    pub dynamic_namespace_attribute: Option<&'s str>,
    pub dynamic_map: Option<&'s str>,
    pub class_directive_helper: Option<&'s str>,
    pub each: Option<(&'s str, &'s str)>,
    pub directives: &'s [Directive],
    pub text: Option<&'s str>,
    pub textarea_default_helper: Option<&'s str>,
    pub choose: &'s str,
    pub once: &'s str,
    pub ordered: bool,
    pub property_keys: &'s [(&'s str, &'s str)],
    pub intrinsic_namespace: Option<&'s str>,
    pub element_spread_markers: &'s [String],
}

/// Lower only explicitly tagged authored blocks. Ordinary JSX keeps the native
/// factory path. All ordinary JSX validation and coercion runs before this seam.
pub(crate) struct LoweredGroups {
    pub source: String,
    pub block_alias: String,
}
pub(crate) fn lower_groups(
    source: &str,
    hints: GroupHints<'_>,
    globals: &crate::source_globals::SourceGlobals,
) -> Result<LoweredGroups, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid grouped Svelte directive template".into());
    }
    struct Collect<'a> {
        group: String,
        nodes: Vec<&'a JSXElement<'a>>,
    }
    impl<'a> Visit<'a> for Collect<'a> {
        fn visit_jsx_element(&mut self, node: &JSXElement<'a>) {
            if matches!(&node.opening_element.name, JSXElementName::IdentifierReference(name) if name.name.as_str() == self.group)
            {
                // AST storage outlives this visitor; use the visitor's arena lifetime.
                self.nodes.push(self.alloc(node));
            }
            walk::walk_jsx_element(self, node);
        }
    }
    let mut collect = Collect {
        group: hints.group.to_owned(),
        nodes: Vec::new(),
    };
    collect.visit_program(&parsed.program);
    if collect.nodes.is_empty() {
        return Err("missing authored directive template boundary".into());
    }
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&parsed.program)
        .semantic;
    let decoded = semantic
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            oxc_ast::AstKind::BindingIdentifier(id) => Some(id.name.as_str()),
            oxc_ast::AstKind::IdentifierReference(id) => Some(id.name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let prefix = (0usize..)
        .map(|index| format!("__crepusDirectiveBlock{index}_"))
        .find(|prefix| {
            !source.contains(prefix)
                && !decoded.iter().any(|name| name.starts_with(prefix.as_str()))
        })
        .ok_or("directive block allocation failed")?;
    let jsx_group_children: std::collections::HashSet<u32> = semantic.nodes().iter().filter_map(|node| {
        let oxc_ast::AstKind::JSXElement(element) = node.kind() else { return None; };
        if matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if name.name == hints.group)
            && matches!(semantic.nodes().parent_kind(node.id()), oxc_ast::AstKind::JSXElement(_) | oxc_ast::AstKind::JSXFragment(_))
        { Some(element.span.start) } else { None }
    }).collect();
    struct Emitter<'a, 's> {
        source: &'s str,
        hints: GroupHints<'s>,
        globals: &'s crate::source_globals::SourceGlobals,
        groups: Vec<&'a JSXElement<'a>>,
        jsx_group_children: std::collections::HashSet<u32>,
        prefix: String,
        serial: usize,
        late: Vec<Vec<String>>,
        declarations: Vec<Vec<String>>,
        references_used: bool,
        spreads_used: bool,
    }
    impl Emitter<'_, '_> {
        fn fresh(&mut self, name: &str) -> String {
            let next = format!("{}{name}{}", self.prefix, self.serial);
            self.serial += 1;
            next
        }
        fn slice(&self, span: oxc_span::Span) -> &str {
            &self.source[span.start as usize..span.end as usize]
        }
        fn nested(&mut self, span: oxc_span::Span) -> Result<String, String> {
            let mut groups = self
                .groups
                .iter()
                .copied()
                .filter(|group| group.span.start >= span.start && group.span.end <= span.end)
                .collect::<Vec<_>>();
            groups.sort_by_key(|group| (group.span.start, std::cmp::Reverse(group.span.end)));
            let mut last = span.start;
            let mut output = String::new();
            for group in groups {
                if group.span.start < last {
                    continue;
                }
                output.push_str(&self.source[last as usize..group.span.start as usize]);
                let jsx_child = self.jsx_group_children.contains(&group.span.start);
                if jsx_child {
                    output.push('{');
                }
                output.push_str(&self.group(group)?);
                if jsx_child {
                    output.push('}');
                }
                last = group.span.end;
            }
            output.push_str(&self.source[last as usize..span.end as usize]);
            Ok(output)
        }
        fn class_inputs(
            &mut self,
            expression: &JSXExpression<'_>,
            statements: &mut Vec<String>,
        ) -> Result<String, String> {
            struct Inputs<'s> {
                helper: &'s str,
                spans: Vec<oxc_span::Span>,
            }
            impl<'a> Visit<'a> for Inputs<'_> {
                fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
                    if !call.callee.is_specific_id(self.helper) {
                        walk::walk_call_expression(self, call);
                        return;
                    }
                    if call.arguments.len() != 3 {
                        return;
                    }
                    self.spans.push(call.arguments[0].span());
                    if let Some(Expression::ObjectExpression(flags)) =
                        call.arguments[2].as_expression()
                    {
                        for property in &flags.properties {
                            if let ObjectPropertyKind::ObjectProperty(property) = property {
                                self.spans.push(property.value.span());
                            }
                        }
                    }
                }
            }
            let Some(helper) = self.hints.class_directive_helper else {
                return self.nested(expression.span());
            };
            let mut inputs = Inputs {
                helper,
                spans: Vec::new(),
            };
            inputs.visit_jsx_expression(expression);
            let span = expression.span();
            let mut output = self.slice(span).to_owned();
            let mut edits = Vec::new();
            for input in inputs.spans {
                let value = self.nested(input)?;
                let read = self.fresh("classInput");
                // Keep each input independent and evaluate it before child creation.
                statements.push(format!(
                    "const {read}={p}memo(()=>({value}));{read}();",
                    p = self.prefix
                ));
                edits.push((input, format!("{read}()")));
            }
            edits.sort_by_key(|(span, _)| span.start);
            for (input, replacement) in edits.into_iter().rev() {
                output.replace_range(
                    (input.start - span.start) as usize..(input.end - span.start) as usize,
                    &replacement,
                );
            }
            Ok(output)
        }
        fn staged(&mut self, body: String, statements: &mut Vec<String>) -> Result<String, String> {
            if !self.hints.ordered {
                return Ok(body);
            }
            let update = self.fresh("update");
            self.declarations
                .last_mut()
                .ok_or("update outside authored fragment")?
                .push(format!("let {update}:(()=>void)|undefined;"));
            statements.push(format!("{update}=()=>{{{body}}};"));
            Ok(format!("if(!{update})throw {}.Error(\"Authored fragment setup did not complete\");{update}();",self.globals.realm))
        }
        fn update(
            &mut self,
            body: String,
            statements: &mut Vec<String>,
            updates: &mut Vec<String>,
        ) -> Result<(), String> {
            updates.push(self.staged(body, statements)?);
            Ok(())
        }
        fn range(&mut self, value: String, retain: &str, statements: &mut Vec<String>) -> String {
            let range = self.fresh("range");
            statements.push(format!(
                "const {range}={p}range(()=>({value}),{p}memo,{retain});{p}prepare({range});",
                p = self.prefix
            ));
            range
        }
        fn append(&mut self, parent: &str, child: &str, range: bool, statements: &mut Vec<String>) {
            if range {
                let anchor = self.fresh("anchor");
                statements.push(format!("const {anchor}={realm}.document.createComment(\"\");{parent}.appendChild({anchor});{p}move({child},{anchor});{anchor}.remove();",p=self.prefix,realm=self.globals.realm));
            } else {
                statements.push(format!("{parent}.appendChild({child});"));
            }
        }
        fn children(
            &mut self,
            children: &[JSXChild<'_>],
            retain: &str,
            statements: &mut Vec<String>,
            updates: &mut Vec<String>,
        ) -> Result<Vec<(String, bool)>, String> {
            let mut roots = Vec::new();
            for child in children {
                match child {
                    JSXChild::Fragment(fragment) => roots.extend(self.children(
                        &fragment.children,
                        retain,
                        statements,
                        updates,
                    )?),
                    JSXChild::Element(element) => {
                        roots.push(self.element(element, retain, statements, updates)?)
                    }
                    JSXChild::ExpressionContainer(container) => {
                        if matches!(container.expression, JSXExpression::EmptyExpression(_)) {
                            continue;
                        }
                        let mut expression = self.nested(container.expression.span())?;
                        let eager = container.expression.as_expression().is_some_and(|expression| {
                            matches!(expression.get_inner_expression(), Expression::CallExpression(call)
                                if (self.hints.textarea_default_helper.is_some_and(|helper| call.callee.is_specific_id(helper)) || self.hints.dynamic_map.is_some_and(|helper| call.callee.is_specific_id(helper))))
                        });
                        if eager {
                            // Inspect deliberately leaves this node-producing helper eager.
                            // Construct it once in sibling order, outside the range reader.
                            let child = self.fresh("child");
                            statements.push(format!("const {child}={expression};"));
                            let range = self.range(child, retain, statements);
                            roots.push((range, true));
                            continue;
                        }
                        if let Some(Expression::CallExpression(choice)) = container
                            .expression
                            .as_expression()
                            .map(Expression::get_inner_expression)
                        {
                            if choice.callee.is_specific_id(self.hints.choose)
                                || choice.callee.is_specific_id(self.hints.once)
                            {
                                let read = self.fresh("choice");
                                statements.push(format!("const {read}={expression};"));
                                expression = read;
                            }
                        }

                        if let Some(Expression::ArrowFunctionExpression(arrow)) = container
                            .expression
                            .as_expression()
                            .map(Expression::get_inner_expression)
                        {
                            if let Some(Statement::ExpressionStatement(statement)) =
                                arrow.body.statements.first()
                            {
                                if let Expression::CallExpression(call) =
                                    statement.expression.get_inner_expression()
                                {
                                    if let Expression::CallExpression(choice) =
                                        call.callee.get_inner_expression()
                                    {
                                        if choice.callee.is_specific_id(self.hints.choose)
                                            && call.arguments.is_empty()
                                        {
                                            let read = self.fresh("choice");
                                            let value = self.nested(choice.span)?;
                                            statements.push(format!("const {read}={value};"));
                                            expression = format!("()=>{read}()");
                                        }
                                    }
                                }
                            }
                        }
                        // Inspect wraps intrinsic children in a reader; the native range
                        // already accepts that reader and owns all text conversion.
                        let range = self.range(format!("({expression})()"), retain, statements);
                        roots.push((range, true));
                    }
                    JSXChild::Text(text) if text.value.trim().is_empty() => {}
                    JSXChild::Text(_) => {
                        return Err("unsupported unnormalized directive template text".into())
                    }
                    JSXChild::Spread(_) => {
                        return Err("unsupported directive template spread child".into())
                    }
                }
            }
            Ok(roots)
        }
        fn element(
            &mut self,
            element: &JSXElement<'_>,
            retain: &str,
            statements: &mut Vec<String>,
            updates: &mut Vec<String>,
        ) -> Result<(String, bool), String> {
            use oxc_span::GetSpan;
            if matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if Some(name.name.as_str()) == self.hints.text)
            {
                let JSXElementName::IdentifierReference(name) = &element.opening_element.name
                else {
                    return Err("invalid text helper".into());
                };
                let mut props = Vec::new();
                for item in &element.opening_element.attributes {
                    let JSXAttributeItem::Attribute(attribute) = item else {
                        return Err("invalid grouped text helper attributes".into());
                    };
                    let JSXAttributeName::Identifier(key) = &attribute.name else {
                        return Err("invalid grouped text helper attribute".into());
                    };
                    let value = match &attribute.value {
                        Some(JSXAttributeValue::ExpressionContainer(value)) => {
                            self.nested(value.expression.span())?
                        }
                        Some(value) => self.slice(value.span()).to_owned(),
                        None => "true".into(),
                    };
                    props.push(format!("{}:{value}", key.name));
                }
                let read = self.fresh("textRead");
                let node = self.fresh("textNode");
                let previous = self.fresh("textPrevious");
                let next = self.fresh("textNext");
                statements.push(format!("const {read}={}({{{}}});const {node}={}.document.createTextNode(\"\");let {previous}=\"\";",name.name,props.join(","),self.globals.realm));
                // Compare authored output, not live DOM text: unrelated group updates
                // must neither publish mutations nor overwrite outside text edits.
                self.update(format!("const {next}={read}();if({previous}!=={next}){{{previous}={next};{node}.data={next};}}"), statements, updates)?;
                return Ok((node, false));
            }
            let JSXElementName::Identifier(tag) = &element.opening_element.name else {
                let mut expression = self.nested(element.span)?;
                if let Some((marker, property)) = self.hints.each {
                    if matches!(&element.opening_element.name,JSXElementName::IdentifierReference(name) if name.name==marker)
                    {
                        let offset =
                            (element.opening_element.name.span().end - element.span.start) as usize;
                        expression.insert_str(offset, &format!(" {property}={{{retain}}}"));
                    }
                }
                let value = self.fresh("child");
                statements.push(format!("const {value}={expression};"));
                return Ok((self.range(value, retain, statements), true));
            };
            let tag = tag.name.as_str();
            let has_spread = self.hints.ordered && element.opening_element.attributes.iter().any(|item| matches!(item,
                JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name, JSXAttributeName::Identifier(name) if self.hints.element_spread_markers.iter().any(|marker| marker == name.name.as_str()))));
            if has_spread {
                self.spreads_used = true;
                let node = self.fresh("spreadNode");
                let mut layers = Vec::new();
                let mut namespace = String::new();
                let mut bindings = Vec::new();
                for item in &element.opening_element.attributes {
                    let JSXAttributeItem::Attribute(attribute) = item else {
                        return Err("unprepared intrinsic Svelte spread".into());
                    };
                    let JSXAttributeName::Identifier(name) = &attribute.name else {
                        return Err("unsupported namespaced Svelte spread attribute".into());
                    };
                    let name = name.name.as_str();
                    if self.hints.intrinsic_namespace == Some(name) {
                        namespace = self.slice(attribute.span).to_owned();
                        continue;
                    }
                    if self.hints.bindings.iter().any(|marker| marker == name) {
                        let Some(JSXAttributeValue::ExpressionContainer(value)) = &attribute.value
                        else {
                            return Err("invalid spread binding installer".into());
                        };
                        bindings.push(self.nested(value.expression.span())?);
                        continue;
                    }
                    let spread = self
                        .hints
                        .element_spread_markers
                        .iter()
                        .any(|marker| marker == name);
                    let value = match &attribute.value {
                        Some(JSXAttributeValue::ExpressionContainer(value)) => {
                            let read = self.nested(value.expression.span())?;
                            if spread {
                                layers.push(format!("({read})"));
                                continue;
                            }
                            if name.starts_with("on") {
                                read
                            } else {
                                format!("({read})()")
                            }
                        }
                        Some(JSXAttributeValue::StringLiteral(value)) => serde_json::to_string(
                            &crate::svelte_entities::decode_entities(value.value.as_str(), true),
                        )
                        .map_err(|error| error.to_string())?,
                        None => "true".into(),
                        _ => {
                            return Err("unsupported intrinsic Svelte spread attribute value".into())
                        }
                    };
                    let authored = if name == "className" {
                        "class".to_owned()
                    } else if name.starts_with("on") {
                        name.to_lowercase()
                    } else {
                        name.to_owned()
                    };
                    let name =
                        serde_json::to_string(&authored).map_err(|error| error.to_string())?;
                    layers.push(format!("()=>({{[{name}]:{value}}})"));
                }
                statements.push(format!("const {node}=<{tag} {namespace}/>;"));
                if bindings.is_empty() {
                    statements.push(format!(
                        "{p}spread({node},{});",
                        layers.join(","),
                        p = self.prefix
                    ));
                }
                for (child, range) in
                    self.children(&element.children, retain, statements, updates)?
                {
                    self.append(&node, &child, range, statements);
                }
                if !bindings.is_empty() {
                    statements.push(format!(
                        "{p}boundSpread({node},[{}],{});",
                        bindings.join(","),
                        layers.join(","),
                        p = self.prefix
                    ));
                }
                return Ok((node, false));
            }
            let typed_input = tag == "input" && element.opening_element.attributes.iter().any(|item| matches!(item,
                JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,JSXAttributeName::Identifier(name) if name.name=="type") && matches!(&attribute.value,Some(JSXAttributeValue::StringLiteral(value)) if matches!(value.value.as_str(),"checkbox"|"radio"))));
            let node = self.fresh("node");
            let creation_at = statements.len();
            let source_namespace = self.hints.ordered && element.opening_element.attributes.iter().any(|item| matches!(item,JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,JSXAttributeName::Identifier(name) if Some(name.name.as_str())==self.hints.dynamic_namespace_attribute)));
            let mut props = Vec::new();
            let mut actions = Vec::new();
            let mut references = Vec::new();
            let mut select_cache = None;

            let has_action=element.opening_element.attributes.iter().any(|item| matches!(item,JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,JSXAttributeName::Identifier(name) if self.hints.directives.iter().any(|directive| directive.attribute==name.name.as_str() && directive.action_parameter.is_some()))));
            for item in &element.opening_element.attributes {
                let JSXAttributeItem::Attribute(attribute) = item else {
                    return Err("unsupported directive intrinsic spread".into());
                };
                let JSXAttributeName::Identifier(name) = &attribute.name else {
                    return Err("unsupported directive namespaced attribute".into());
                };
                let name = name.name.as_str();
                if self.hints.dynamic_tag_attribute == Some(name)
                    || self.hints.dynamic_namespace_attribute == Some(name)
                {
                    let Some(JSXAttributeValue::ExpressionContainer(value)) = &attribute.value
                    else {
                        return Err("invalid dynamic tag metadata".into());
                    };
                    props.push(format!(
                        "{name}={{{}}}",
                        self.nested(value.expression.span())?
                    ));
                    continue;
                }
                if self.hints.intrinsic_reference == Some(name) {
                    let Some(JSXAttributeValue::ExpressionContainer(value)) = &attribute.value
                    else {
                        return Err("invalid intrinsic reference metadata".into());
                    };
                    let Some(Expression::ObjectExpression(metadata)) =
                        value.expression.as_expression()
                    else {
                        return Err("invalid intrinsic reference metadata".into());
                    };
                    let property = |key: &str| {
                        metadata
                            .properties
                            .iter()
                            .find_map(|property| match property {
                                ObjectPropertyKind::ObjectProperty(property)
                                    if property
                                        .key
                                        .static_name()
                                        .is_some_and(|name| name == key) =>
                                {
                                    Some(&property.value)
                                }
                                _ => None,
                            })
                    };
                    let Some(Expression::StringLiteral(node_type)) = property("nodeType") else {
                        return Err("missing intrinsic reference node type".into());
                    };
                    let Some(Expression::StringLiteral(parts_type)) = property("partsType") else {
                        return Err("missing intrinsic reference parts type".into());
                    };
                    let Some(Expression::BooleanLiteral(pair)) = property("pair") else {
                        return Err("invalid intrinsic reference form".into());
                    };
                    let parts = self
                        .slice(
                            property("parts")
                                .ok_or("missing intrinsic reference parts")?
                                .span(),
                        )
                        .to_owned();
                    let get = self
                        .slice(
                            property("get")
                                .ok_or("missing intrinsic reference getter")?
                                .span(),
                        )
                        .to_owned();
                    let set = self
                        .slice(
                            property("set")
                                .ok_or("missing intrinsic reference setter")?
                                .span(),
                        )
                        .to_owned();
                    let node_type = node_type.value.as_str();
                    let parts_type = parts_type.value.as_str();
                    let read = self.fresh("referenceParts");
                    let infer = self.fresh("referenceTuple");
                    let mut install = format!("const {read}={parts};type {node_type}=typeof {node};type {parts_type}=typeof {read} extends ()=>infer {infer}?{infer}:never;");
                    if pair.value {
                        let getter = self.fresh("referenceGet");
                        let setter = self.fresh("referenceSet");
                        let value = self.fresh("referenceValue");
                        let args = self.fresh("referenceArgs");
                        install.push_str(&format!("const {getter}={get};const {setter}:(value:{node_type},...parts:{parts_type})=>unknown={set};{p}bindThis({node},{{cleanup:\"commit\",publication:\"post\",parts:{parts},get:(...{args})=>{getter}(...{args}),set:({value},...{args})=>{setter}({value}!,...{args})}});",p=self.prefix));
                    } else {
                        install.push_str(&format!("{p}bindThis({node},{{cleanup:\"commit\",publication:\"post\",parts:{parts},get:{get},set:{set}}});",p=self.prefix));
                    }
                    references.push(install);
                    self.references_used = true;
                    continue;
                }
                if self.hints.bindings.iter().any(|marker| marker == name) {
                    let Some(JSXAttributeValue::ExpressionContainer(value)) = &attribute.value
                    else {
                        return Err("invalid template binding marker".into());
                    };
                    let install = format!("({})({node})", self.slice(value.expression.span()));
                    if has_action {
                        actions.push(format!("{p}bind(()=>{{{install};}});", p = self.prefix));
                    } else {
                        let install = self.staged(format!("{install};"), statements)?;
                        self.late
                            .last_mut()
                            .ok_or("binding outside template block")?
                            .push(install);
                    }
                    continue;
                }
                if let Some(directive) = self
                    .hints
                    .directives
                    .iter()
                    .find(|directive| directive.attribute == name)
                {
                    let Some(JSXAttributeValue::ExpressionContainer(value)) = &attribute.value
                    else {
                        return Err("invalid directive marker value".into());
                    };
                    let value = value
                        .expression
                        .as_expression()
                        .ok_or("empty directive marker")?
                        .get_inner_expression();
                    // This closure is compiler metadata, excluded from Inspect's
                    // attribute thunking. Its semantic identifiers are already lowered.
                    if let Some(parameter) = directive.action_parameter {
                        let Expression::ArrayExpression(parts) = value else {
                            return Err("invalid action marker".into());
                        };
                        let expected = if parameter { 2 } else { 1 };
                        if parts.elements.len() != expected {
                            return Err("invalid action marker arity".into());
                        }
                        let invoke = self.slice(parts.elements[0].span()).to_owned();
                        let parameter = if parameter {
                            format!(",{}", self.slice(parts.elements[1].span()))
                        } else {
                            String::new()
                        };
                        actions.push(format!(
                            "{p}action({node},{invoke}{parameter});",
                            p = self.prefix
                        ));
                    } else {
                        let read = self.slice(value.span()).to_owned();
                        actions.push(format!("{p}attach({node},{read});", p = self.prefix));
                    }
                    continue;
                }
                if tag == "select" && name == "value" {
                    let cache = self.fresh("selection");
                    let expression = match &attribute.value {
                        Some(JSXAttributeValue::ExpressionContainer(value)) => {
                            format!("({})()", self.nested(value.expression.span())?)
                        }
                        Some(JSXAttributeValue::StringLiteral(value)) => serde_json::to_string(
                            &crate::svelte_entities::decode_entities(value.value.as_str(), true),
                        )
                        .map_err(|error| error.to_string())?,
                        None => "true".to_owned(),
                        _ => return Err("unsupported select value expression".into()),
                    };
                    self.update(
                        format!(
                            "{cache}={expression};{p}attribute({node},\"value\",{cache});",
                            p = self.prefix
                        ),
                        statements,
                        updates,
                    )?;
                    select_cache = Some(cache);
                    continue;
                }
                let property = self
                    .hints
                    .property_keys
                    .iter()
                    .find(|(marker, _)| *marker == name)
                    .map(|(_, key)| *key);
                let key = serde_json::to_string(name).map_err(|error| error.to_string())?;
                match &attribute.value {
                    Some(JSXAttributeValue::ExpressionContainer(value))
                        if property.is_none() && !name.starts_with("on") =>
                    {
                        let expression =
                            if self.hints.ordered && matches!(name, "class" | "className") {
                                self.class_inputs(&value.expression, statements)?
                            } else if self.hints.ordered && name == "xmlns" && element.opening_element.attributes.iter().any(|item| matches!(item, JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,JSXAttributeName::Identifier(name) if Some(name.name.as_str())==self.hints.dynamic_tag_attribute))) {
                                let expression = self.nested(value.expression.span())?;
                                let read = self.fresh("xmlnsInput");
                                statements.push(format!("const {read}={p}memo(()=>({expression})());{read}();",p=self.prefix));
                                format!("()=>{read}()")
                            } else {
                                self.nested(value.expression.span())?
                            };
                        let update = if source_namespace && name == "xmlns" {
                            let next = self.fresh("xmlnsValue");
                            format!("const {next}=({expression})();if({next}==null){node}.removeAttribute(\"xmlns\");else {node}.setAttribute(\"xmlns\",{next} as string);")
                        } else if self.hints.ordered && tag == "option" && name == "value" {
                            format!("{p}option({node},({expression})());", p = self.prefix)
                        } else if self.hints.ordered && typed_input && name == "value" {
                            format!("{p}input({node},({expression})());", p = self.prefix)
                        } else if self.hints.ordered
                            && name == "value"
                            && matches!(tag, "input" | "textarea")
                        {
                            let previous = self.fresh("valuePrevious");
                            let next = self.fresh("valueNext");
                            self.declarations
                                .last_mut()
                                .ok_or("value cache outside authored fragment")?
                                .push(format!("let {previous}:unknown;"));
                            // Nullish values share the initial undefined cache. Commit
                            // the cache only after application so failed coercions retry.
                            format!("const {next}=({expression})();if({previous}!==({next}??undefined)){{{p}attribute({node},{key},{next});{previous}={next}??undefined;}}",p=self.prefix)
                        } else {
                            format!(
                                "{p}attribute({node},{key},({expression})());",
                                p = self.prefix
                            )
                        };
                        self.update(update, statements, updates)?;
                    }
                    Some(_) | None => props.push(self.slice(attribute.span).to_owned()),
                }
            }
            let ordered_children = if source_namespace {
                None
            } else if self.hints.ordered && tag == "textarea" && !element.children.is_empty() {
                // Preserve the direct Text child for the existing textarea-value
                // lowering. Ordinary DOM children would change textContent and
                // bypass its value memo/default protocol.
                Some(
                    element
                        .children
                        .iter()
                        .map(|child| self.nested(child.span()))
                        .collect::<Result<Vec<_>, _>>()?
                        .join(""),
                )
            } else if self.hints.ordered && !element.children.is_empty() {
                let mut child_statements = Vec::new();
                let roots =
                    self.children(&element.children, retain, &mut child_statements, updates)?;
                let roots = roots
                    .into_iter()
                    .map(|(node, _)| node)
                    .collect::<Vec<_>>()
                    .join(",");
                Some(format!(
                    "{{{}(()=>{{{}return [{roots}];}})}}",
                    self.hints.once,
                    child_statements.join("")
                ))
            } else {
                None
            };
            if source_namespace {
                // Namespace conversion/creation must fail before authored attribute reads.
                statements.insert(
                    creation_at,
                    format!("const {node}=<{tag} {}/>;", props.join(" ")),
                );
            } else if let Some(children) = &ordered_children {
                statements.push(format!(
                    "const {node}=<{tag} {}>{children}</{tag}>;",
                    props.join(" ")
                ));
            } else {
                statements.push(format!("const {node}=<{tag} {}/>;", props.join(" ")));
            }
            if let Some(cache) = select_cache {
                statements.push(format!(
                    "let {cache}:unknown;{p}select({node},()=>{cache});",
                    p = self.prefix
                ));
            }
            if ordered_children.is_none() {
                for (child, is_range) in
                    self.children(&element.children, retain, statements, updates)?
                {
                    self.append(&node, &child, is_range, statements);
                }
            }
            statements.extend(references);
            statements.extend(actions);
            Ok((node, false))
        }
        fn group(&mut self, group: &JSXElement<'_>) -> Result<String, String> {
            let retain = self.fresh("retain");
            let mut statements = Vec::new();
            let mut updates = Vec::new();
            self.late.push(Vec::new());
            self.declarations.push(Vec::new());
            let roots = self.children(&group.children, &retain, &mut statements, &mut updates)?;
            let late = self.late.pop().ok_or("missing template binding list")?;
            let consumer = if self.hints.ordered && (!updates.is_empty() || !late.is_empty()) {
                let consumer = self.fresh("consumer");
                statements.insert(
                    0,
                    format!("const {consumer}={p}consumer();", p = self.prefix),
                );
                Some(consumer)
            } else {
                None
            };
            if !updates.is_empty() {
                if let Some(consumer) = &consumer {
                    // Register after all authored setup, in source order, but keep
                    // each updater's dependencies independent. A text dependency
                    // must not rewrite an unrelated dirty form control.
                    for update in &updates {
                        let run = self.fresh("readUpdates");
                        statements.push(format!("const {run}=()=>{{{update}}};{p}effect(()=>{consumer}?{p}withConsumer({consumer},{run}):{run}());",p=self.prefix));
                    }
                } else {
                    statements.push(format!(
                        "{p}effect(()=>{{{}}});",
                        updates.join(""),
                        p = self.prefix
                    ));
                }
            }
            if !late.is_empty() {
                if let Some(consumer) = &consumer {
                    let run = self.fresh("lateBindings");
                    statements.push(format!("const {run}=()=>{{{}}};{consumer}?{p}withConsumer({consumer},{run}):{run}();",late.join(""),p=self.prefix));
                } else {
                    statements.extend(late);
                }
            }
            let roots = roots.into_iter().map(|(node, _)| node).collect::<Vec<_>>();
            let force_array = group.opening_element.attributes.iter().any(|item| matches!(item, JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "constComponent")));
            let result = if roots.len() == 1 && !force_array {
                roots[0].clone()
            } else {
                format!("[{}]", roots.join(","))
            };
            let declarations = self
                .declarations
                .pop()
                .ok_or("missing fragment declarations")?;
            Ok(format!(
                "{p}block(({retain})=>{{{}{}return {result};}})",
                declarations.join(""),
                statements.join(""),
                p = self.prefix
            ))
        }
    }
    let mut emitter = Emitter {
        source,
        hints,
        globals,
        groups: collect.nodes,
        jsx_group_children,
        prefix,
        serial: 0,
        late: Vec::new(),
        declarations: Vec::new(),
        references_used: false,
        spreads_used: false,
    };
    let body = emitter.nested(oxc_span::Span::new(0, source.len() as u32))?;
    let p = &emitter.prefix;
    let realm = &emitter.globals.realm;
    let types = &emitter.globals.types;
    let consumer_import = if emitter.hints.ordered {
        format!("import{{getRuneConsumer as {p}consumer}}from\"@tschk/moonshine-svelte/pre\";import{{withComponentConsumer as {p}withConsumer}}from\"@tschk/moonshine-dom/component\";\n")
    } else {
        String::new()
    };
    let reference_import = if emitter.references_used {
        format!("import{{bindThis as {p}bindThis}}from\"@tschk/moonshine-svelte/binding\";\n")
    } else {
        String::new()
    };
    let spread_import = if emitter.spreads_used {
        format!("import{{installElementSpread as {p}spread,installBoundElementSpread as {p}boundSpread}}from\"@tschk/moonshine-svelte/runes\";\n")
    } else {
        String::new()
    };
    let block_alias = format!("{p}block");
    let source = format!("{spread_import}{reference_import}{consumer_import}import{{templateBlock as {p}block,createInsertionRange as {p}range,prepareInsertionRange as {p}prepare,moveInsertionRange as {p}move,setAttributeValue as {p}attribute}}from\"@tschk/moonshine-dom/dom\";import{{createRenderEffect as {p}effect,createRenderMemo as {p}memo,getRenderScheduler as {p}scheduler,createComponentScope as {p}scope,onCleanup as {p}cleanup}}from\"@tschk/moonshine-dom/reactive\";import{{setOptionValue as {p}option,setInputValue as {p}input}}from\"@tschk/moonshine-svelte/binding\";import{{createRuneEffect as {p}post}}from\"@tschk/moonshine-svelte/runes\";import{{installAction as {p}action,installAttachment as {p}attach}}from\"@tschk/moonshine-svelte/directives\";\nfunction {p}select(element:{types}.HTMLSelectElement,read:()=>unknown):void{{const observer=new {realm}.MutationObserver(entries=>{{if(entries.every(entry=>{{if(entry.target instanceof {realm}.Element&&entry.target.closest(\"selectedcontent\"))return true;if(entry.type!==\"childList\")return false;const nodes=[...entry.addedNodes,...entry.removedNodes];return nodes.length>0&&nodes.every(node=>node.nodeName===\"SELECTEDCONTENT\");}}))return;{p}attribute(element,\"value\",read());}});observer.observe(element,{{childList:true,subtree:true,attributes:true,attributeFilter:[\"value\"]}});{p}cleanup(()=>observer.disconnect());}}\nfunction {p}bind(install:()=>void):void{{const scheduler={p}scheduler();if(!scheduler)throw {realm}.Error(\"Template binding requires rune scheduler\");{p}scope(()=>scheduler.branch(()=>{p}post(install)));}}\n{body}");
    Ok(LoweredGroups {
        source,
        block_alias,
    })
}

#[cfg(test)]
mod source_tests {
    use super::{lower_groups, GroupHints};
    use crate::{compile_moonshine_svelte, MoonshineModuleTarget};
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    use std::path::Path;

    fn lower(source: &str) -> String {
        lower_mode(source, false)
    }
    fn lower_mode(source: &str, ordered: bool) -> String {
        let output = lower_groups(
            source,
            GroupHints {
                group: "Group",
                bindings: &[],
                intrinsic_reference: None,
                dynamic_tag_attribute: Some("DynTag"),
                dynamic_namespace_attribute: Some("DynNs"),
                dynamic_map: None,
                class_directive_helper: Some("Classes"),
                each: None,
                directives: &[],
                text: Some("Text"),
                textarea_default_helper: Some("ResetDefault"),
                choose: "Choose",
                once: "Once",
                ordered,
                property_keys: &[],
                intrinsic_namespace: None,
                element_spread_markers: &[],
            },
            &Default::default(),
        )
        .unwrap()
        .source;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
        output
    }

    #[test]
    fn authored_option_expression_values_keep_raw_identity() {
        let output = lower_mode(
            "function App(){return <Group><option value={()=>raw}/></Group>;}",
            true,
        );
        assert!(output.contains("setOptionValue as"), "{output}");
        assert!(output.contains("option("), "{output}");
        assert!(
            !output.contains("attribute(__crepusDirective_node0,\"value\""),
            "{output}"
        );
    }

    #[test]
    fn source_namespace_creation_precedes_attribute_reads_and_uses_browser_setters() {
        let output=lower_mode("function C(){return <Group><div DynTag={tag} DynNs={ns} xmlns={()=>readNs()}><Child/></div></Group>}",true);
        let creation = output.find("=<div").unwrap();
        let read = output.find("readNs()").unwrap();
        let child = output.find("<Child").unwrap();
        assert!(creation < read && read < child, "{output}");
        assert!(output.contains(".setAttribute(\"xmlns\","), "{output}");
        assert!(output.contains(".removeAttribute(\"xmlns\")"), "{output}");
    }

    #[test]
    fn class_inputs_are_separate_memos_before_child_setup() {
        let output = lower_mode("function C(){return <Group><div className={()=>Classes(base(),null,{active:flag()})}><Child/></div></Group>}",true);
        let base = output.find("memo(()=>(base()))").unwrap();
        let flag = output.find("memo(()=>(flag()))").unwrap();
        let child = output.find("<Child").unwrap();
        assert!(base < flag && flag < child, "{output}");
        assert_eq!(output.matches("base()").count(), 1);
        assert_eq!(output.matches("flag()").count(), 1);
    }

    #[test]
    fn supplied_globals_replace_every_compiler_owned_access() {
        let source = "function Component(){return <Group><Text parts={[()=>before]}/></Group>;}";
        let globals = crate::source_globals::SourceGlobals::with_aliases("__r", "__T", true);
        let output = lower_groups(
            source,
            GroupHints {
                group: "Group",
                bindings: &[],
                intrinsic_reference: None,
                dynamic_tag_attribute: None,
                dynamic_namespace_attribute: None,
                dynamic_map: None,
                class_directive_helper: Some("Classes"),
                each: None,
                directives: &[],
                text: Some("Text"),
                textarea_default_helper: None,
                choose: "Choose",
                once: "Once",
                ordered: true,
                property_keys: &[],
                intrinsic_namespace: None,
                element_spread_markers: &[],
            },
            &globals,
        )
        .unwrap()
        .source;
        assert!(!output.contains("globalThis"), "{output}");
        assert!(output.contains("new (__r()).MutationObserver"), "{output}");
        assert!(output.contains("element:__T.HTMLSelectElement"), "{output}");
        assert!(
            output.contains("throw (__r()).Error(\"Authored fragment setup"),
            "{output}"
        );
        assert!(
            output.contains("=(__r()).document.createTextNode"),
            "{output}"
        );
    }

    #[test]
    fn ordered_intrinsics_construct_children_inside_namespace_callback_and_stage_updates() {
        let output = lower_mode("function Component(){return <Group><svg title={()=>label}><Text parts={[()=>before]}/><Child/><g><Text parts={[()=>after]}/></g></svg><Read/></Group>;}", true);
        assert!(output.contains("<svg >{Once(()=>{"), "{output}");
        assert!(output.contains("<g >{Once(()=>{"), "{output}");
        assert!(output.contains(":(()=>void)|undefined;"), "{output}");
        assert!(output.contains("=()=>{const"), "{output}");
        assert!(output.contains("_effect(()=>"), "{output}");
        let child = output.find("=<Read/>").unwrap();
        let effects = output.rfind("_effect(()=>").unwrap();
        assert!(child < effects, "{output}");
        assert!(
            !output.contains("let __crepusDirectiveBlock0_node"),
            "{output}"
        );
    }

    #[test]
    fn ordered_updates_have_independent_effects_after_all_setup() {
        let output = lower_mode("function Component(){return <Group><p><Text parts={[()=>first()]}/></p><Read/><input value={()=>second()}/></Group>;}", true);
        assert_eq!(output.matches("_effect(()=>").count(), 2, "{output}");
        let effects = output
            .match_indices("_effect(()=>")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert!(output.find("=<Read/>").unwrap() < effects[0], "{output}");
        assert!(output.find("<input").unwrap() < effects[0], "{output}");
        // Each wrapper restores the same captured consumer and invokes exactly
        // one readiness-checked updater; throwing/disposal cannot merge peers.
        let runs = output
            .split("const ")
            .filter(|part| part.starts_with("__crepusDirectiveBlock0_readUpdates"))
            .collect::<Vec<_>>();
        assert_eq!(runs.len(), 2, "{output}");
        for run in runs {
            let body = run.split("};").next().unwrap();
            assert_eq!(
                body.matches("Authored fragment setup did not complete")
                    .count(),
                1,
                "{output}"
            );
            assert!(run.contains("_withConsumer("), "{output}");
        }
    }

    #[test]
    fn ordered_textarea_keeps_direct_text_for_existing_value_lowering() {
        let output = lower_mode("function Component(){return <Group><textarea><Text parts={[()=>value]}/></textarea><Read/></Group>;}", true);
        assert!(
            output.contains("<textarea ><Text parts={[()=>value]}/></textarea>"),
            "{output}"
        );
        assert!(!output.contains("createTextNode"), "{output}");
    }

    #[test]
    fn eager_textarea_child_is_constructed_once_before_its_range() {
        let output = lower("function Component(){return <Group>{ResetDefault(<textarea/>,()=>defaultValue)}{()=>ordinary()}</Group>;}");
        assert!(
            output.contains(
                "const __crepusDirectiveBlock0_child1=ResetDefault(<textarea/>,()=>defaultValue);"
            ),
            "{output}"
        );
        assert!(
            output.contains("range(()=> (__crepusDirectiveBlock0_child1)")
                || output.contains("range(()=>(__crepusDirectiveBlock0_child1)"),
            "{output}"
        );
        assert!(!output.contains("defaultValue)()"), "{output}");
        assert_eq!(output.matches("ResetDefault(").count(), 1);
        assert!(output.contains("(()=>ordinary())()"), "{output}");
    }

    #[test]
    fn known_once_accessor_creation_is_hoisted_but_initializer_stays_lazy() {
        let output = lower(
            "function Component(){return <Group>{Once(()=>setup())}{()=>arbitrary()}</Group>;}",
        );
        assert!(
            output.contains("const __crepusDirectiveBlock0_choice1=Once(()=>setup());"),
            "{output}"
        );
        assert!(
            output.contains("(__crepusDirectiveBlock0_choice1)()"),
            "{output}"
        );
        assert_eq!(output.matches("setup()").count(), 1, "{output}");
        assert!(output.contains("(()=>arbitrary())()"), "{output}");
        assert!(!output.contains("=arbitrary();"), "{output}");
    }

    #[test]
    fn grouped_text_compares_cached_output_before_writing_dom() {
        let output = lower("function Component(){return <Group><p title={()=>revision}><Text parts={[()=>value]}/></p></Group>;}");
        assert!(
            output.contains("let __crepusDirectiveBlock0_textPrevious4=\"\";"),
            "{output}"
        );
        assert!(output.contains("if(__crepusDirectiveBlock0_textPrevious4!==__crepusDirectiveBlock0_textNext5){__crepusDirectiveBlock0_textPrevious4=__crepusDirectiveBlock0_textNext5;__crepusDirectiveBlock0_textNode3.data=__crepusDirectiveBlock0_textNext5;}"), "{output}");
        assert_eq!(output.matches("textRead2()").count(), 1);
        assert!(!output.contains(".data=__crepusDirectiveBlock0_textRead"));
    }
    #[test]
    fn authored_action_emits_owned_group_and_server_erases_its_reads() {
        let source = "<script lang='ts'>let count=$state(1);function action(node:HTMLDivElement,value:number){return{update(value:number){node.dataset.observed=String(value)}}}</script><div use:action={count} data-count={count}><span>{count}</span></div>";
        let dom = compile_moonshine_svelte(
            source,
            Path::new("Action.svelte"),
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert!(dom.source.contains("templateBlock as"), "{}", dom.source);
        assert!(dom.source.contains("installAction as"), "{}", dom.source);
        assert!(
            dom.source.contains(".data=") || dom.source.contains(".data ="),
            "{}",
            dom.source
        );
        assert!(!dom.source.contains("__crepusDirective0"), "{}", dom.source);
        assert!(
            !dom.source.contains("__CrepusDirectiveTemplate"),
            "{}",
            dom.source
        );
        let ssr = compile_moonshine_svelte(
            source,
            Path::new("Action.svelte"),
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert!(!ssr.source.contains("installAction"));
        assert!(!ssr.source.contains("__crepusDirective0"));
    }
}
