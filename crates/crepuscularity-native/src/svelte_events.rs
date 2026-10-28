//! Preserve authored Svelte's late handler reads without changing generic JSX.
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

#[cfg(test)]
pub(crate) fn transform_events(
    source: &str,
    path: &Path,
    binding_helpers: &[String],
) -> Result<String, String> {
    transform_events_with_policy(
        source,
        path,
        binding_helpers,
        crate::svelte_javascript::IrPolicy::Typed,
        None,
        None,
        &Default::default(),
    )
}

pub(crate) fn transform_events_with_policy(
    source: &str,
    path: &Path,
    binding_helpers: &[String],
    policy: crate::svelte_javascript::IrPolicy,
    dynamic_tag_attribute: Option<&str>,
    intrinsic_namespace: Option<&str>,
    globals: &crate::source_globals::SourceGlobals,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("{}: invalid Svelte event syntax", path.display()));
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(format!("{}: invalid Svelte event scope", path.display()));
    }
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name) {
            name.push('_');
        }
        name
    };
    let helper = fresh("__crepusSvelteEvent");
    let derived = fresh("__crepusSvelteEventDerived");
    let mut edits = Vec::new();
    let mut memo_needed = false;
    let mut late_needed = false;
    for node in built.semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        let event_target = match &element.opening_element.name {
            JSXElementName::Identifier(_) => true,
            JSXElementName::IdentifierReference(id) => {
                binding_helpers.iter().any(|name| name == id.name.as_str())
            }
            _ => false,
        };
        if !event_target {
            continue;
        }
        for item in &element.opening_element.attributes {
            let JSXAttributeItem::Attribute(attribute) = item else {
                continue;
            };
            let JSXAttributeName::Identifier(name) = &attribute.name else {
                continue;
            };
            if !matches!(
                name.name.as_str(),
                "onClick"
                    | "onInput"
                    | "onChange"
                    | "onToggle"
                    | "onSubmit"
                    | "onReset"
                    | "onKeyDown"
                    | "onKeyUp"
                    | "onFocus"
                    | "onBlur"
            ) {
                continue;
            }
            let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
                continue;
            };
            let Some(expression) = container.expression.as_expression() else {
                continue;
            };
            let dynamic = dynamic_tag_attribute.is_some_and(|marker| element.opening_element.attributes.iter().any(|item| matches!(item, JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == marker))));
            let static_tag = match &element.opening_element.name {
                JSXElementName::Identifier(tag) if !dynamic => Some(tag.name.as_str()),
                _ => None,
            };
            if (dynamic || static_tag.is_some())
                && policy == crate::svelte_javascript::IrPolicy::Typed
                && matches!(
                    expression.get_inner_expression(),
                    Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
                )
            {
                let span = expression.span();
                let event = name.name.as_str()[2..].to_ascii_lowercase();
                let types = &globals.types;
                let literal;
                let selector = if let Some(tag) = static_tag {
                    literal = format!("{tag:?}");
                    literal.as_str()
                } else {
                    element.opening_element.attributes.iter().find_map(|item| match item {
                        JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name, JSXAttributeName::Identifier(name) if Some(name.name.as_str()) == dynamic_tag_attribute) => match &attribute.value {
                            Some(JSXAttributeValue::ExpressionContainer(value)) => value.expression.as_expression().map(|expression| &source[expression.span().start as usize..expression.span().end as usize]),
                            _ => None,
                        }, _ => None,
                    }).ok_or("missing dynamic event selector")?
                };
                let namespace = intrinsic_namespace.and_then(|marker| element.opening_element.attributes.iter().find_map(|item| match item {
                    JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name,JSXAttributeName::Identifier(name) if name.name==marker)=>match &attribute.value {Some(JSXAttributeValue::StringLiteral(value))=>Some(value.value.as_str()),_=>None},_=>None,
                })).unwrap_or("http://www.w3.org/1999/xhtml");
                let element_type =
                    crate::source_globals::dynamic_element_type(types, selector, namespace);
                let element_type = if static_tag.is_some() {
                    element_type.replacen("typeof ", "", 1)
                } else {
                    element_type
                };
                edits.push((span.start as usize, span.start as usize, "((".into()));
                edits.push((span.end as usize, span.end as usize, format!(") satisfies (this:{element_type},event:{types}.GlobalEventHandlersEventMap[\"{event}\"] & {{currentTarget:{element_type}}})=>unknown)")));
                continue;
            }
            if matches!(
                expression.get_inner_expression(),
                Expression::ArrowFunctionExpression(_)
                    | Expression::FunctionExpression(_)
                    | Expression::ObjectExpression(_)
            ) {
                continue;
            }
            if let Expression::Identifier(id) = expression.get_inner_expression() {
                if built
                    .semantic
                    .scoping()
                    .get_reference(id.reference_id.get().unwrap())
                    .symbol_id()
                    .is_some_and(|symbol| {
                        !built.semantic.scoping().symbol_flags(symbol).is_import()
                    })
                {
                    continue;
                }
            }
            late_needed = true;
            let memoize = crate::svelte_expression::metadata(expression, &built.semantic).0;
            memo_needed |= memoize;
            let span = expression.span();
            let read = if memoize {
                "()=> ("
            } else {
                "function(this:any){return ("
            };
            edits.push((
                span.start as usize,
                span.start as usize,
                format!("{helper}({read}"),
            ));
            edits.push((
                span.end as usize,
                span.end as usize,
                if memoize {
                    "),true)".to_owned()
                } else {
                    ");},false)".to_owned()
                },
            ));
        }
    }
    if edits.is_empty() {
        return Ok(source.to_owned());
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    if !late_needed {
        return Ok(out);
    }
    let (initialize, read) = if memo_needed {
        (
            format!("const memo=memoize?{derived}(read):undefined;"),
            "memo?memo.value:read.apply(this,args)",
        )
    } else {
        (String::new(), "read.apply(this,args)")
    };
    let memo_parameter = if memo_needed { "memoize" } else { "_memoize" };
    out.push_str(&format!("\nfunction {helper}(read:(this:any,...args:any[])=>any,{memo_parameter}:boolean):any{{{initialize}return function(this:any,...args:any[]){{const handler:any={read};return handler?.apply(this,args);}};}}\n"));
    if memo_needed {
        out = format!(
            "import {{createDerived as {derived}}} from \"@tschk/moonshine-svelte/runes\";\n{out}"
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lower(source: &str) -> String {
        let out = transform_events(source, Path::new("event.svelte"), &[]).unwrap();
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &out, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{out}", parsed.errors);
        out
    }
    #[test]
    fn factories_cache_once_while_getters_identifiers_and_nullish_values_are_late() {
        let out = lower(
            "export default function C(){function make(){};return <><button onClick={make()}/><button onClick={obj.handler}/><input onInput={handler}/><button onClick={null}/></>}",
        );
        assert!(out.contains("__crepusSvelteEvent(()=> (make()),true)"));
        assert!(out.contains("function(this:any){return (obj.handler);},false"));
        assert!(out.contains("function(this:any){return (handler);},false"));
        assert!(out.contains("function(this:any){return (null);},false"));
        assert!(out.contains("const memo=memoize?__crepusSvelteEventDerived(read):undefined"));
        assert!(out.contains("return handler?.apply(this,args)"));
    }
    #[test]
    fn unbound_global_calls_remain_per_dispatch_unless_local_dependencies_are_read() {
        let out = lower(
            "export default function C(){let state=1;function local(){};return <><button onClick={globalFactory()}/><button onClick={globalThis.factory()}/><button onClick={globalFactory(state)}/><button onClick={local()}/><button onClick={globalTag`${state}`}/><button onClick={globalFactory() || local}/><button onClick={local || globalFactory()}/></>}",
        );
        assert!(out.contains("function(this:any){return (globalFactory());},false"));
        assert!(out.contains("function(this:any){return (globalThis.factory());},false"));
        assert!(out.contains("()=> (globalFactory(state)),true"));
        assert!(out.contains("()=> (local()),true"));
        assert!(out.contains("function(this:any){return (globalTag`${state}`);},false"));
        assert!(out.contains("function(this:any){return (globalFactory() || local);},false"));
        assert!(out.contains("()=> (local || globalFactory()),true"));
    }
    #[test]
    fn nonmemo_reads_receive_event_this_and_arguments_without_derived_import() {
        let out = lower(
            "export default function C(){return <button onClick={this.handler || arguments[0]}/>}",
        );
        assert!(!out.contains("import"));
        assert!(out.contains("function(this:any){return (this.handler || arguments[0]);},false"));
        assert!(out.contains("read.apply(this,args)"));
    }
    #[test]
    fn ordinary_local_identifier_handlers_are_captured_but_imports_and_cells_are_late() {
        let out = lower(
            "import imported from './handler';export default function C(){let handler=()=>{};let cell={value:()=>{}};return <><button onClick={handler}/><button onClick={imported}/><button onClick={cell.value}/></>}",
        );
        assert!(out.contains("onClick={handler}"));
        assert!(out.contains("function(this:any){return (imported);},false"));
        assert!(out.contains("function(this:any){return (cell.value);},false"));
    }
    #[test]
    fn generated_binding_helpers_receive_late_event_reads_but_other_components_do_not() {
        let source = "export default function C(){let cell={value:()=>{}};return <><CheckedHelper onChange={cell.value}/><ValueHelper onInput={cell.value}/><OpenHelper onToggle={cell.value}/><details onToggle={cell.value}/><Child onClick={cell.value}/></>}";
        let out = transform_events(
            source,
            Path::new("binding.svelte"),
            &[
                "CheckedHelper".into(),
                "ValueHelper".into(),
                "OpenHelper".into(),
            ],
        )
        .unwrap();
        assert_eq!(
            out.matches("function(this:any){return (cell.value);},false")
                .count(),
            4
        );
        assert!(out.contains("<Child onClick={cell.value}/>"));
        let allocator = Allocator::default();
        assert!(Parser::new(&allocator, &out, SourceType::tsx())
            .parse()
            .errors
            .is_empty());
    }
    #[test]
    fn inline_functions_objects_and_component_props_are_preserved() {
        let source = "export default function C(){return <><button onClick={()=>make()}/><button onClick={function(event){return this.handle(event)}}/><input onInput={{handleEvent(){},capture:true,once:true,passive:true,signal:abort.signal}}/><Child onClick={make()}/><UI.Child onClick={make()}/></>}";
        let out = lower(source);
        assert!(out.contains("((()=>make()) satisfies"), "{out}");
        assert!(
            out.contains("((function(event){return this.handle(event)}) satisfies"),
            "{out}"
        );
        for unchanged in ["<input onInput={{handleEvent(){},capture:true,once:true,passive:true,signal:abort.signal}}/>", "<Child onClick={make()}/>", "<UI.Child onClick={make()}/>"] {
            assert!(out.contains(unchanged), "{out}");
        }
        assert!(!out.contains("function __crepusSvelteEvent"), "{out}");
    }
    #[test]
    fn nested_jsx_has_disjoint_wrappers_and_executable_calls_only() {
        let out = lower(
            "export default function C(){function inner(){};function tag(){};return <><button onClick={choose?<button onClick={inner()}/>:handler}/><button onClick={items[()=>later()]}/><button onClick={new Handler()}/><button onClick={tag`hello`}/></>}",
        );
        assert!(out.contains("onClick={__crepusSvelteEvent(()=> (choose?<button"));
        assert!(out.contains("onClick={__crepusSvelteEvent(()=> (inner()),true)}"));
        assert!(out.contains("function(this:any){return (items[()=>later()]);},false"));
        assert!(out.contains("function(this:any){return (new Handler());},false"));
        assert!(out.contains("__crepusSvelteEvent(()=> (tag`hello`),true)"));
    }
    #[test]
    fn helper_names_are_hygienic_and_unknown_or_malformed_attrs_remain_for_validation() {
        let out = lower(
            "const __crepusSvelteEvent=1;const __crepusSvelteEventDerived=2;function make(){};export default()=> <button onClick={make()} onUnsupported={other()}/>;",
        );
        assert!(out.contains("__crepusSvelteEvent_(()=> (make()),true)"));
        assert!(out.contains("createDerived as __crepusSvelteEventDerived_"));
        assert!(out.contains("onUnsupported={other()}"));
        let source = "export default()=> <button onClick='invalid'/>";
        assert_eq!(lower(source), source);
    }
}

#[cfg(test)]
mod dynamic_type_tests {
    use super::*;
    #[test]
    fn dynamic_inline_handlers_get_type_context_without_a_runtime_wrapper() {
        let source = "export default function C(){return <div Tag={tag} onKeyDown={e=>e.key}/>;}";
        let output = transform_events_with_policy(
            source,
            Path::new("Dynamic.svelte"),
            &[],
            crate::svelte_javascript::IrPolicy::Typed,
            Some("Tag"),
            None,
            &Default::default(),
        )
        .unwrap();
        assert!(
            output.contains("GlobalEventHandlersEventMap[\"keydown\"]"),
            "{output}"
        );
        assert!(!output.contains("function __crepusSvelteEvent"), "{output}");
        let allocator = Allocator::default();
        assert!(
            Parser::new(&allocator, &output, SourceType::tsx())
                .parse()
                .errors
                .is_empty(),
            "{output}"
        );
    }
}

#[cfg(test)]
mod static_type_tests {
    use super::*;
    fn typed(source: &str, namespace: Option<&str>) -> String {
        transform_events_with_policy(
            source,
            Path::new("Static.svelte"),
            &[],
            crate::svelte_javascript::IrPolicy::Typed,
            Some("Tag"),
            namespace,
            &Default::default(),
        )
        .unwrap()
    }
    #[test]
    fn static_intrinsic_inline_handlers_get_type_context_without_a_runtime_wrapper() {
        let output = typed(
            "export default function C(){return <option onClick={(event)=>event.currentTarget}/>;}",
            None,
        );
        assert!(output.contains("(\"option\" extends infer"), "{output}");
        assert!(
            output.contains("GlobalEventHandlersEventMap[\"click\"]"),
            "{output}"
        );
        assert!(!output.contains("function __crepusSvelteEvent"), "{output}");
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
    fn static_intrinsic_uses_namespace_witness_and_leaves_objects_untyped() {
        let output = typed(
            "export default function C(){return <><a ns=\"http://www.w3.org/2000/svg\" onClick={()=>1}/><input onInput={{handleEvent(){}}}/></>;}",
            Some("ns"),
        );
        assert!(output.contains("SVGElementTagNameMap"), "{output}");
        assert!(!output.contains("\"input\""), "{output}");
    }
}
