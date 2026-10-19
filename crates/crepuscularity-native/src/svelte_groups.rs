//! Resolve group targets before allocating native controllers.
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType, Span};

#[derive(Debug, PartialEq)]
pub(crate) struct Group {
    pub target: String,
    pub kind: String,
    pub symbols: Vec<String>,
    pub scope: Span,
    pub members: Vec<Span>,
}

fn input_kind(element: &JSXElement<'_>) -> Result<String, String> {
    if !matches!(&element.opening_element.name, JSXElementName::Identifier(name) if name.name=="input")
    {
        return Err("group binding requires an input".into());
    }
    let kind = element.opening_element.attributes.iter().find_map(|item| match item {
        JSXAttributeItem::Attribute(attr) if matches!(&attr.name,JSXAttributeName::Identifier(name) if name.name=="type") => match &attr.value {
        Some(JSXAttributeValue::StringLiteral(value)) => Some(value.value.as_str()),
        Some(JSXAttributeValue::ExpressionContainer(value)) => value.expression.as_expression().and_then(|value| match value.get_inner_expression() {Expression::StringLiteral(value)=>Some(value.value.as_str()),_=>None}),
        _=>None,
        }, _=>None,
    }).filter(|kind| matches!(*kind,"radio"|"checkbox")).ok_or("group binding requires a static radio or checkbox type")?;
    Ok(kind.to_owned())
}

pub(crate) fn plan(source: &str, marker: &str) -> Result<Vec<Group>, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid group binding module".into());
    }
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err("invalid group binding scope".into());
    }
    let nodes = built.semantic.nodes();
    let scopes = built.semantic.scoping();
    let mut groups: Vec<Group> = Vec::new();
    for node in nodes.iter() {
        let AstKind::JSXAttribute(attribute) = node.kind() else {
            continue;
        };
        if !matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name==marker) {
            continue;
        }
        let element = nodes
            .ancestor_kinds(node.id())
            .find_map(|kind| match kind {
                AstKind::JSXElement(element) => Some(element),
                _ => None,
            })
            .ok_or("group binding outside an element")?;
        let kind = input_kind(element)?;
        let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
            return Err("bind:group requires an assignable target".into());
        };
        let Some(expression) = container.expression.as_expression() else {
            return Err("bind:group requires an assignable target".into());
        };
        let root = crate::svelte_bindings::binding_root(expression)
            .ok_or("bind:group requires an assignable target")?;
        let resolved = root
            .reference_id
            .get()
            .and_then(|id| scopes.get_reference(id).symbol_id());
        let store_symbol = if resolved.is_none() {
            root.name
                .strip_prefix('$')
                .and_then(|name| scopes.find_binding(node.scope_id(), name.into()))
        } else {
            None
        };
        let root_symbol = resolved
            .or(store_symbol)
            .ok_or("bind:group requires a declared target")?;
        if store_symbol.is_none()
            && matches!(expression.get_inner_expression(), Expression::Identifier(_))
        {
            let flags = scopes.symbol_flags(root_symbol);
            if flags.is_const_variable() || flags.is_import() {
                return Err("bind:group cannot assign to a constant binding".into());
            }
        }
        let span = expression.span();
        let mut symbols = Vec::new();
        let mut declarations = Vec::new();
        for reference in nodes.iter() {
            let AstKind::IdentifierReference(id) = reference.kind() else {
                continue;
            };
            if id.span.start < span.start || id.span.end > span.end {
                continue;
            }
            if let Some(symbol) = id
                .reference_id
                .get()
                .and_then(|id| scopes.get_reference(id).symbol_id())
                .or_else(|| {
                    id.name
                        .strip_prefix('$')
                        .and_then(|name| scopes.find_binding(reference.scope_id(), name.into()))
                })
            {
                symbols.push(format!("{symbol:?}"));
                declarations.push(scopes.symbol_span(symbol));
            }
        }
        let functions = nodes
            .ancestor_kinds(node.id())
            .filter_map(|kind| match kind {
                AstKind::Function(function) => Some(function.span),
                AstKind::ArrowFunctionExpression(function) => Some(function.span),
                _ => None,
            })
            .collect::<Vec<_>>();
        let scope = functions
            .iter()
            .copied()
            .find(|scope| {
                declarations
                    .iter()
                    .any(|decl| decl.start >= scope.start && decl.end <= scope.end)
            })
            .or_else(|| functions.last().copied())
            .ok_or("bind:group requires a component scope")?;
        let mut codegen = Codegen::new();
        codegen.print_expression(expression.get_inner_expression());
        let target = codegen.into_source_text();
        if let Some(group) = groups.iter_mut().find(|group| {
            group.target == target && group.symbols == symbols && group.scope == scope
        }) {
            if group.kind != kind {
                group.kind = "mixed".to_owned();
            }
            group.members.push(attribute.span);
        } else {
            groups.push(Group {
                target,
                kind: kind.to_owned(),
                symbols,
                scope,
                members: vec![attribute.span],
            });
        }
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn siblings_share_targets_but_distinct_aliases_do_not() {
        let groups = plan("function App(){let rows;let i;let j;return <><input type={'checkbox'} Group={rows[i]}/><input type={'checkbox'} Group={rows[ i ]}/><input type={'checkbox'} Group={rows[j]}/></>}", "Group").unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].members.len(), 2);
    }
    #[test]
    fn loop_independent_targets_use_component_scope() {
        let source = "function App(){let selected;return rows.map(row=><input type={'checkbox'} Group={selected}/>)}";
        let groups = plan(source, "Group").unwrap();
        assert_eq!(groups[0].scope.start, 0);
    }
    #[test]
    fn loop_dependent_and_nested_targets_use_their_declaration_scope() {
        let source="function App(){let rows;return rows.map(row=>row.children.map(child=><><input type={'checkbox'} Group={row.selected}/><input type={'checkbox'} Group={child.selected}/></>))}";
        let groups = plan(source, "Group").unwrap();
        assert_eq!(groups.len(), 2);
        assert!(groups[0].scope.start < groups[1].scope.start);
        assert!(groups[0].scope.start > 0);
    }
    #[test]
    fn invalid_targets_fail_without_a_runtime_fallback() {
        for source in [
            "function App(){return <input type={'checkbox'} Group={missing}/>}",
            "function App(){const value=[];return <input type={'checkbox'} Group={value}/>}",
            "function App(){return <input type={'checkbox'} Group={read()}/>}",
        ] {
            assert!(plan(source, "Group").is_err(), "{source}");
        }
    }
}

/// Emit controller declarations in the owning lexical function, before member setup.
#[cfg(test)]
pub(crate) fn allocate(
    source: &str,
    marker: &str,
    globals: &crate::source_globals::SourceGlobals,
) -> Result<String, String> {
    allocate_with_spreads(source, marker, &[], globals)
}

pub(crate) fn allocate_with_spreads(
    source: &str,
    marker: &str,
    element_spread_markers: &[String],
    globals: &crate::source_globals::SourceGlobals,
) -> Result<String, String> {
    let types = &globals.types;
    let groups = plan(source, marker)?;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    let built = SemanticBuilder::new().build(&parsed.program);
    let mut used = std::collections::HashSet::new();
    for node in built.semantic.nodes().iter() {
        match node.kind() {
            AstKind::BindingIdentifier(id) => {
                used.insert(id.name.to_string());
            }
            AstKind::IdentifierReference(id) => {
                used.insert(id.name.to_string());
            }
            _ => {}
        }
    }
    let mut factory = "__createInputGroup".to_string();
    while !used.insert(factory.clone()) {
        factory.push('_');
    }
    let mut replacements = Vec::new();
    let mut declarations = std::collections::BTreeMap::<(u32, u32), String>::new();
    for (index, group) in groups.iter().enumerate() {
        let mut name = format!("__groupController{index}");
        while !used.insert(name.clone()) {
            name.push('_');
        }
        let mut value = "__groupValue".to_string();
        while !used.insert(value.clone()) {
            value.push('_');
        }
        let mut input = "__groupInput".to_string();
        while !used.insert(input.clone()) {
            input.push('_');
        }
        declarations
            .entry((group.scope.start, group.scope.end))
            .or_default()
            .push_str(&format!("const {name}={factory}({kind:?},()=>({target}),({value}:any)=>({target}={value}));",kind=group.kind,target=group.target));
        for member in &group.members {
            let watch = built.semantic.nodes().iter().find_map(|node| {
                let AstKind::JSXElement(element) = node.kind() else {
                    return None;
                };
                if !element.opening_element.attributes.iter().any(|item| {
                    matches!(item, JSXAttributeItem::Attribute(attr) if attr.span == *member)
                }) {
                    return None;
                }
                let has_spread = element.opening_element.attributes.iter().any(|item| {
                    matches!(item, JSXAttributeItem::Attribute(attr)
                        if matches!(&attr.name, JSXAttributeName::Identifier(id)
                            if element_spread_markers.iter().any(|m| m == id.name.as_str())))
                });
                if !has_spread {
                    return None;
                }
                element.opening_element.attributes.iter().find_map(|item| {
                    let JSXAttributeItem::Attribute(attr) = item else {
                        return None;
                    };
                    if !matches!(&attr.name, JSXAttributeName::Identifier(id) if id.name == "value")
                    {
                        return None;
                    }
                    match &attr.value {
                        Some(JSXAttributeValue::ExpressionContainer(value)) => {
                            let span = value.expression.span();
                            Some(&source[span.start as usize..span.end as usize])
                        }
                        _ => None,
                    }
                })
            });
            let bind_call = match watch {
                Some(watch) => format!("{name}.bind({input},()=>({watch}\n))"),
                None => format!("{name}.bind({input})"),
            };
            replacements.push((
                member.start as usize,
                member.end as usize,
                format!("{marker}={{({input}:{types}.HTMLInputElement)=>{bind_call}}}"),
            ));
        }
    }
    let mut edits = std::collections::BTreeMap::<usize, String>::new();
    for ((start, end), declaration) in declarations {
        let mut body = None;
        for node in built.semantic.nodes().iter() {
            match node.kind() {
                AstKind::Function(function) if function.span == Span::new(start, end) => {
                    body = function.body.as_ref().map(|body| (body.span, false));
                }
                AstKind::ArrowFunctionExpression(function)
                    if function.span == Span::new(start, end) =>
                {
                    body = Some((function.body.span, function.expression));
                }
                _ => {}
            }
        }
        let (span, expression) = body.ok_or("missing group allocation function")?;
        if expression {
            edits
                .entry(span.start as usize)
                .or_default()
                .push_str(&format!("{{{declaration}return ("));
            edits
                .entry(span.end as usize)
                .or_default()
                .insert_str(0, ");}");
        } else {
            edits
                .entry(span.start as usize + 1)
                .or_default()
                .push_str(&declaration);
        }
    }
    replacements.extend(
        edits
            .into_iter()
            .map(|(position, text)| (position, position, text)),
    );
    replacements.sort_by_key(|(start, end, _)| (*start, *end));
    let mut output = source.to_owned();
    for (start, end, text) in replacements.into_iter().rev() {
        output.replace_range(start..end, &text);
    }
    if !groups.is_empty() {
        output=format!("import{{createInputGroup as {factory}}}from\"@tschk/moonshine-svelte/binding\";\n{output}");
    }
    Ok(output)
}

#[cfg(test)]
mod allocation_tests {
    use super::*;
    #[test]
    fn controllers_are_allocated_per_component_or_dependent_row() {
        let source = "function App(){let selected;let rows;return rows.map(row=><><input type={'checkbox'} Group={selected}/><input type={'checkbox'} Group={row.selected}/><input type={'checkbox'} Group={row.selected}/></>)}";
        let output = allocate(source, "Group", &Default::default()).unwrap();
        assert!(
            output.contains("function App(){const __groupController0=__createInputGroup("),
            "{output}"
        );
        assert!(
            output.contains("row=>{const __groupController1=__createInputGroup("),
            "{output}"
        );
        assert_eq!(output.matches("const __groupController").count(), 2);
        let allocator = Allocator::default();
        assert!(
            Parser::new(&allocator, &output, SourceType::tsx())
                .parse()
                .errors
                .is_empty(),
            "{output}"
        );
    }
    #[test]
    fn multiple_groups_wrap_a_row_once_and_avoid_decoded_authored_names() {
        let source = r"function App(){let __groupControll\u0065r0;let rows;return rows.map(row=><><input type={'checkbox'} Group={row.first}/><input type={'checkbox'} Group={row.second}/></>)}";
        let output = allocate(source, "Group", &Default::default()).unwrap();
        assert!(
            output.contains("row=>{const __groupController0_=__createInputGroup(")
                && output.contains("const __groupController1=__createInputGroup("),
            "{output}"
        );
        assert_eq!(output.matches("return (").count(), 1);
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{output}");
        assert!(
            SemanticBuilder::new()
                .with_check_syntax_error(true)
                .build(&parsed.program)
                .errors
                .is_empty(),
            "{output}"
        );
    }
    #[test]
    fn generated_setter_does_not_capture_an_authored_target_name() {
        let source = "function App(){let __groupValue;return <input type={'checkbox'} Group={__groupValue}/>}";
        let output = allocate(source, "Group", &Default::default()).unwrap();
        assert!(
            output.contains("(__groupValue_:any)=>(__groupValue=__groupValue_)"),
            "{output}"
        );
        assert!(output.contains(".bind(__groupInput)"), "{output}");
    }
    #[test]
    fn authored_change_handler_installs_after_group_binding_and_is_cleaned_up() {
        let source="function App(){let selected;return <input type={'checkbox'} Group={selected} onChange={event=>selected}/> }";
        let allocated = allocate(source, "Group", &Default::default()).unwrap();
        let output = install_events(&allocated, "Group", &Default::default()).unwrap();
        assert!(!output.contains(" onChange="), "{output}");
        assert!(output.contains("=>__installGroupMember("), "{output}");
        assert!(
            output.contains("group.bind(input);if(!options?.capture)listen();"),
            "{output}"
        );
        assert!(
            output.contains("removeEventListener('change',handler"),
            "{output}"
        );
        let allocator = Allocator::default();
        assert!(
            Parser::new(&allocator, &output, SourceType::tsx())
                .parse()
                .errors
                .is_empty(),
            "{output}"
        );
    }
    #[test]
    fn spread_group_members_keep_change_handlers_in_spread_layers_and_watch_authored_dynamic_values(
    ) {
        let source = "function App(){let selected;let dynamicVal;let attrs;return <><input type={'checkbox'} value={dynamicVal} __crepusSvelteElementSpread0={attrs} onChange={event=>selected} Group={selected}/><input type={'checkbox'} __crepusSvelteElementSpread1={attrs} Group={selected}/></> }";
        let spread_markers = vec![
            "__crepusSvelteElementSpread0".to_string(),
            "__crepusSvelteElementSpread1".to_string(),
        ];
        let allocated =
            allocate_with_spreads(source, "Group", &spread_markers, &Default::default()).unwrap();
        assert!(
            allocated.contains(".bind(__groupInput,()=>(dynamicVal\n))"),
            "{allocated}"
        );
        assert!(allocated.contains(".bind(__groupInput)}"), "{allocated}");
        let output =
            install_events_with_spreads(&allocated, "Group", &spread_markers, &Default::default())
                .unwrap();
        assert!(output.contains("onChange={event=>selected}"), "{output}");
        assert!(!output.contains("__installGroupMember"), "{output}");
    }
}

/// Server group bindings emit ordered lazy attributes, without allocating controllers.
pub(crate) fn ssr(
    source: &str,
    marker: &str,
    namespace: Option<&str>,
) -> Result<(String, Option<String>), String> {
    let groups = plan(source, marker)?;
    if groups.is_empty() {
        return Ok((source.to_owned(), None));
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    let built = SemanticBuilder::new().build(&parsed.program);
    let mut used = std::collections::HashSet::new();
    for node in built.semantic.nodes().iter() {
        match node.kind() {
            AstKind::BindingIdentifier(id) => {
                used.insert(id.name.to_string());
            }
            AstKind::IdentifierReference(id) => {
                used.insert(id.name.to_string());
            }
            _ => {}
        }
    }
    let mut fresh = |base: &str| {
        let mut name = base.to_owned();
        while !used.insert(name.clone()) {
            name.push('_');
        }
        name
    };
    let helper = fresh("__GroupServerInput");
    let entries_factory = fresh("__groupSourceEntries");
    let assigned = fresh("__groupSsrValue");
    let mut helper_used = false;
    let mut edits = Vec::new();
    for node in built.semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        let binding = element.opening_element.attributes.iter().find_map(|item| match item {
            JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,JSXAttributeName::Identifier(name) if name.name==marker)=>match &attribute.value {
                Some(JSXAttributeValue::ExpressionContainer(value))=>Some((attribute.span, value.expression.span())),_=>None,
            },_=>None,
        });
        let Some((marker_span, target)) = binding else {
            continue;
        };
        let kind = input_kind(element)?;
        let target = &source[target.start as usize..target.end as usize];
        let model =
            format!("(false?(({assigned}:any)=>({target}\n={assigned}))(undefined):({target}\n))");
        let decode_attr_value = |attribute: &JSXAttribute<'_>| -> Result<String, String> {
            match &attribute.value {
                None => Ok("true".to_owned()),
                Some(JSXAttributeValue::StringLiteral(value)) => serde_json::to_string(
                    &crate::svelte_entities::decode_entities(value.value.as_str(), true),
                )
                .map_err(|error| error.to_string()),
                Some(JSXAttributeValue::ExpressionContainer(value)) => {
                    if value.expression.as_expression().is_none() {
                        return Err("invalid group SSR attribute expression".into());
                    }
                    let span = value.expression.span();
                    Ok(format!(
                        "({}\n)",
                        &source[span.start as usize..span.end as usize]
                    ))
                }
                _ => Err("unsupported group SSR attribute".into()),
            }
        };
        let authored_value = element
            .opening_element
            .attributes
            .iter()
            .find_map(|item| match item {
                JSXAttributeItem::Attribute(attribute)
                    if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "value") =>
                {
                    Some(decode_attr_value(attribute))
                }
                _ => None,
            })
            .transpose()?;
        let has_spread = element
            .opening_element
            .attributes
            .iter()
            .any(|item| matches!(item, JSXAttributeItem::SpreadAttribute(_)));
        if has_spread {
            let replacement = if let Some(value) = authored_value {
                let checked = if kind == "checkbox" {
                    format!("{model}.includes({value})")
                } else {
                    format!("{model}===({value})")
                };
                format!("{{...{{checked:({checked})}}}}")
            } else {
                format!("{{...(false?{{checked:(({assigned}:any)=>({target}\n={assigned}))(undefined)}}:{{}})}}")
            };
            edits.push((
                marker_span.start as usize,
                marker_span.end as usize,
                replacement,
            ));
            continue;
        }
        helper_used = true;
        let mut entries = Vec::new();
        for item in &element.opening_element.attributes {
            let JSXAttributeItem::Attribute(attribute) = item else {
                return Err("group SSR spreads require ordered source spread lowering".into());
            };
            let JSXAttributeName::Identifier(name) = &attribute.name else {
                return Err("invalid group SSR attribute name".into());
            };
            let name = name.name.as_str();
            if name == marker {
                if let Some(value) = &authored_value {
                    let checked = if kind == "checkbox" {
                        format!("{model}.includes({value})")
                    } else {
                        format!("{model}===({value})")
                    };
                    entries.push(format!("[\"checked\",()=>({checked})]"));
                }
                continue;
            }
            if Some(name) == namespace
                || name.starts_with("on")
                || matches!(name, "ref" | "children")
            {
                continue;
            }
            let live = match name {
                "defaultChecked" => Some("checked"),
                "defaultValue" => Some("value"),
                _ => None,
            };
            if live.is_some_and(|live| element.opening_element.attributes.iter().any(|item| matches!(item, JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,JSXAttributeName::Identifier(name) if name.name==live)))) {
                continue;
            }
            let key = match name {
                "defaultChecked" => "checked",
                "defaultValue" => "value",
                _ => name,
            };
            let value = decode_attr_value(attribute)?;
            let key = serde_json::to_string(key).map_err(|error| error.to_string())?;
            let lazy = matches!(
                &attribute.value,
                Some(JSXAttributeValue::ExpressionContainer(_))
            );
            entries.push(format!(
                "[{key},{}]",
                if lazy {
                    format!("()=>({value})")
                } else {
                    value.clone()
                }
            ));
        }
        edits.push((
            element.span.start as usize,
            element.span.end as usize,
            format!("<{helper} entries={{[{}]}}/>", entries.join(",")),
        ));
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    for pair in edits.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err("nested JSX group values require non-overlapping SSR lowering".into());
        }
    }
    let mut output = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        output.replace_range(start..end, &text);
    }
    if !helper_used {
        return Ok((output, None));
    }
    output=format!("import{{hSourceEntries as {entries_factory}}}from\"@tschk/moonshine-dom/ssr\";\n{output}\nfunction {helper}(props:{{entries:readonly(readonly[string,unknown])[]}}){{return {entries_factory}(\"input\",props.entries);}}\n");
    Ok((output, Some(helper)))
}

#[cfg(test)]
mod ssr_tests {
    use super::*;
    #[test]
    fn ordered_group_attributes_keep_duplicate_checked_and_repeat_value_reads() {
        let source = r#"function App(){let selected;return <input type="checkbox" checked={true} value={value()} Group={selected} data-after={after()}/>;}"#;
        let (output, helper) = ssr(source, "Group", None).unwrap();
        assert!(helper.is_some());
        assert!(output.contains("hSourceEntries as"), "{output}");
        assert_eq!(output.matches("value()").count(), 2);
        assert_eq!(output.matches("[\"checked\",").count(), 2);
        assert!(output.find(".includes(").unwrap() < output.find("after()").unwrap());
        let allocator = Allocator::default();
        assert!(
            Parser::new(&allocator, &output, SourceType::tsx())
                .parse()
                .errors
                .is_empty(),
            "{output}"
        );
    }
    #[test]
    fn missing_values_do_not_read_group_and_client_handlers_are_erased() {
        let source = r#"function App(){let model;return <input type="radio" Group={model.value} onChange={handler()}/>;}"#;
        let (output, _) = ssr(source, "Group", None).unwrap();
        assert!(!output.contains("model.value"), "{output}");
        assert!(!output.contains("handler()"), "{output}");
        assert!(!output.contains("[\"checked\","), "{output}");
    }
    #[test]
    fn synthetic_reset_defaults_do_not_duplicate_live_source_attributes() {
        let source = r#"function App(){let selected;return <input type="checkbox" value="a" defaultValue="a" checked={true} defaultChecked={true} Group={selected}/>;}"#;
        let (output, _) = ssr(source, "Group", None).unwrap();
        assert_eq!(output.matches("[\"value\",").count(), 1, "{output}");
        assert_eq!(output.matches("[\"checked\",").count(), 2, "{output}");
    }
    #[test]
    fn spread_group_ssr_emits_positioned_checked_layer_only_when_value_is_authored() {
        let source = r#"function App(){let selected;let attrs;return <><input type="checkbox" {...attrs} Group={selected}/><input type="checkbox" value="a" {...attrs} Group={selected}/><input type="radio" value={choice()} Group={selected} {...attrs}/></>;}"#;
        let (output, helper) = ssr(source, "Group", None).unwrap();
        assert!(helper.is_none());
        assert!(!output.contains("hSourceEntries"), "{output}");
        assert!(output.contains("{...(false?{checked:"), "{output}");
        assert!(
            output.contains("{...attrs} {...{checked:((false?"),
            "{output}"
        );
        assert!(
            output.contains("===((choice()\n)))}} {...attrs}/>"),
            "{output}"
        );
    }
}

#[cfg(test)]
mod combined_binding_tests {
    #[test]
    fn group_ssr_consumes_checked_markers_before_ordered_attribute_emission() {
        let source = r#"<script lang="ts">let selected=$state(['a']);let checked=$state(false);</script><input type="checkbox" value="a" bind:group={selected} bind:checked={checked}/>"#;
        let output = crate::svelte_module::compile_moonshine_svelte(
            source,
            std::path::Path::new("Combined.svelte"),
            crate::MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert!(
            !output.source.contains("__crepusCheckedBinding"),
            "{}",
            output.source
        );
        assert!(
            output.source.matches("\"checked\"").count() >= 2,
            "{}",
            output.source
        );
        assert!(
            output.source.contains("hSourceEntries"),
            "{}",
            output.source
        );
    }
}

/// Install authored handlers after the group binder, following source event lowering.
#[cfg(test)]
pub(crate) fn install_events(
    source: &str,
    marker: &str,
    globals: &crate::source_globals::SourceGlobals,
) -> Result<String, String> {
    install_events_with_spreads(source, marker, &[], globals)
}

pub(crate) fn install_events_with_spreads(
    source: &str,
    marker: &str,
    element_spread_markers: &[String],
    globals: &crate::source_globals::SourceGlobals,
) -> Result<String, String> {
    let types = &globals.types;
    let realm = &globals.realm;
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid group event module".into());
    }
    let built = SemanticBuilder::new().build(&parsed.program);
    let mut used = std::collections::HashSet::new();
    for node in built.semantic.nodes().iter() {
        match node.kind() {
            AstKind::BindingIdentifier(id) => {
                used.insert(id.name.to_string());
            }
            AstKind::IdentifierReference(id) => {
                used.insert(id.name.to_string());
            }
            _ => {}
        }
    }
    let mut fresh = |base: &str| {
        let mut name = base.to_owned();
        while !used.insert(name.clone()) {
            name.push('_');
        }
        name
    };
    let installer = fresh("__installGroupMember");
    let cleanup = fresh("__groupCleanup");
    let input = fresh("__groupEventInput");
    let mut edits = Vec::new();
    for node in built.semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        if element.opening_element.attributes.iter().any(|item| {
            matches!(item, JSXAttributeItem::Attribute(attribute)
                if matches!(&attribute.name, JSXAttributeName::Identifier(name)
                    if element_spread_markers.iter().any(|marker| marker == name.name.as_str())))
        }) {
            continue;
        }
        let find = |key: &str| {
            element.opening_element.attributes.iter().find_map(|item|match item{JSXAttributeItem::Attribute(attribute)if matches!(&attribute.name,JSXAttributeName::Identifier(name)if name.name==key)=>Some(attribute),_=>None})
        };
        let (Some(binding), Some(handler)) = (find(marker), find("onChange")) else {
            continue;
        };
        let (
            Some(JSXAttributeValue::ExpressionContainer(binder)),
            Some(JSXAttributeValue::ExpressionContainer(value)),
        ) = (&binding.value, &handler.value)
        else {
            return Err("group event and binder require expressions".into());
        };
        let span = binder.expression.span();
        let binder = &source[span.start as usize..span.end as usize];
        let expression = crate::svelte_bindings::typed_event_expression(
            source,
            &value.expression,
            globals,
            "HTMLInputElement",
            "change",
        );
        edits.push((binding.span.start as usize,binding.span.end as usize,format!("{marker}={{({input}:{types}.HTMLInputElement)=>{installer}({input},{{bind:({binder})}},{expression})}}")));
        edits.push((
            handler.span.start as usize,
            handler.span.end as usize,
            String::new(),
        ));
    }
    if edits.is_empty() {
        return Ok(source.to_owned());
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut output = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        output.replace_range(start..end, &text);
    }
    {
        output=format!("import{{onCleanup as {cleanup}}}from\"@tschk/moonshine-dom/reactive\";\n{output}\nfunction {installer}(input:{types}.HTMLInputElement,group:{{bind:(input:{types}.HTMLInputElement)=>void}},handler:any):void{{const object=typeof handler==='object'&&handler!==null?handler:undefined;if(handler!=null&&typeof handler!=='function'&&typeof object?.handleEvent!=='function')throw new {realm}.TypeError('onChange must be a handler');const options=object?{{capture:!!object.capture,once:object.once,passive:object.passive,signal:object.signal}}:undefined;const listen=()=>{{if(handler==null)return;input.addEventListener('change',handler,options);{cleanup}(()=>input.removeEventListener('change',handler,options?.capture));}};if(options?.capture)listen();group.bind(input);if(!options?.capture)listen();}}\n");
    }
    Ok(output)
}

#[cfg(test)]
mod store_group_tests {
    #[test]
    fn store_group_assignments_reach_the_existing_store_write_lowering() {
        let source = r#"<script lang="ts">import{writable}from'svelte/store';const selected=writable<string[]>([]);</script><input type="checkbox" value="a" bind:group={$selected}/>"#;
        for target in [
            crate::MoonshineModuleTarget::Dom,
            crate::MoonshineModuleTarget::Ssr,
        ] {
            let output = crate::svelte_module::compile_moonshine_svelte(
                source,
                std::path::Path::new("StoreGroup.svelte"),
                target,
            )
            .unwrap();
            assert!(output.source.contains("setStoreValue"), "{}", output.source);
            assert!(
                !output.source.contains("from \"svelte/store\""),
                "{}",
                output.source
            );
        }
    }
}

#[cfg(test)]
mod mixed_kind_tests {
    use super::*;
    #[test]
    fn one_target_shares_a_mixed_controller_across_input_kinds() {
        let source = r#"function App(){let selected;return <><input type="checkbox" value="a" Group={selected}/><input type="radio" value={choice} Group={selected}/></>}"#;
        let groups = plan(source, "Group").unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].kind, "mixed");
        assert_eq!(groups[0].members.len(), 2);
        let output = allocate(source, "Group", &Default::default()).unwrap();
        assert_eq!(output.matches("const __groupController").count(), 1);
        assert!(output.contains("__createInputGroup(\"mixed\""), "{output}");
        let (output, _) = ssr(source, "Group", None).unwrap();
        assert_eq!(output.matches(".includes(").count(), 1, "{output}");
        assert_eq!(output.matches("===(").count(), 1, "{output}");
    }
}
