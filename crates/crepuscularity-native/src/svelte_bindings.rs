//! Connect authored Svelte form bindings after native element construction.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BindingKind {
    Checked,
    Open,
    Files,
    Value,
    Select,
}

pub(crate) fn typed_event_expression(
    source: &str,
    expression: &JSXExpression<'_>,
    globals: &crate::source_globals::SourceGlobals,
    element: &str,
    event: &str,
) -> String {
    let span = expression.span();
    let text = &source[span.start as usize..span.end as usize];
    let types = &globals.types;
    format!("(({text}) satisfies (((this:{types}.{element},event:{types}.GlobalEventHandlersEventMap[\"{event}\"] & {{currentTarget:{types}.{element}}})=>unknown)|({types}.EventListenerObject & {types}.AddEventListenerOptions)|null|undefined))")
}

fn assignable(value: &Expression<'_>) -> bool {
    binding_root(value).is_some()
}
pub(crate) fn binding_root<'a, 'b>(
    value: &'b Expression<'a>,
) -> Option<&'b IdentifierReference<'a>> {
    match value.get_inner_expression() {
        Expression::Identifier(id) => Some(id),
        Expression::StaticMemberExpression(member) if !member.optional => {
            binding_root(&member.object)
        }
        Expression::ComputedMemberExpression(member) if !member.optional => {
            binding_root(&member.object)
        }
        // Svelte requires an identifier root; computed keys may still be arbitrary expressions.
        _ => None,
    }
}

#[cfg(test)]
pub(crate) fn transform(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    marker: &str,
    kind: BindingKind,
) -> Result<(String, Option<String>), String> {
    transform_template(source, path, target, marker, kind, false)
}

#[cfg(test)]
pub(crate) fn transform_template(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    marker: &str,
    kind: BindingKind,
    grouped: bool,
) -> Result<(String, Option<String>), String> {
    transform_template_with_policy(
        source,
        path,
        target,
        marker,
        kind,
        grouped,
        (
            crate::svelte_javascript::IrPolicy::Typed,
            &Default::default(),
        ),
    )
}

pub(crate) fn transform_template_with_policy(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    marker: &str,
    kind: BindingKind,
    grouped: bool,
    (policy, globals): (
        crate::svelte_javascript::IrPolicy,
        &crate::source_globals::SourceGlobals,
    ),
) -> Result<(String, Option<String>), String> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    let select = kind == BindingKind::Select;
    let files = kind == BindingKind::Files;
    let directive = match kind {
        BindingKind::Checked => "bind:checked",
        BindingKind::Open => "bind:open",
        BindingKind::Files => "bind:files",
        _ => "bind:value",
    };
    let fail = |message: &str| format!("{}: {message}", path.display());
    if !parsed.errors.is_empty() {
        return Err(fail("invalid Svelte binding syntax"));
    }
    policy.configure(&mut parsed.program);
    let built = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !built.errors.is_empty() {
        return Err(fail("invalid Svelte binding scope"));
    }
    let semantic = built.semantic;
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name)
            || semantic.nodes().iter().any(|node| match node.kind() {
                AstKind::BindingIdentifier(id) => id.name == name.as_str(),
                AstKind::IdentifierReference(id) => id.name == name.as_str(),
                _ => false,
            })
        {
            name.push('_');
        }
        name
    };
    let checked = kind == BindingKind::Checked;
    let open = kind == BindingKind::Open;
    let helper = fresh(match kind {
        BindingKind::Checked => "__CrepusCheckedInput",
        BindingKind::Open => "__CrepusOpenDetails",
        BindingKind::Files => "__CrepusFilesInput",
        BindingKind::Value => "__CrepusValueInput",
        BindingKind::Select => "__CrepusSelectValue",
    });
    let bind = fresh(match kind {
        BindingKind::Checked => "__crepusBindChecked",
        BindingKind::Open => "__crepusBindOpen",
        BindingKind::Files => "__crepusBindFiles",
        BindingKind::Value => "__crepusBindValue",
        BindingKind::Select => "__crepusBindSelect",
    });
    let value = fresh(match kind {
        BindingKind::Checked => "__crepusCheckedValue",
        BindingKind::Open => "__crepusOpenValue",
        BindingKind::Files => "__crepusFilesValue",
        BindingKind::Value => "__crepusInputValue",
        BindingKind::Select => "__crepusSelectValue",
    });
    let cleanup = fresh(match kind {
        BindingKind::Checked => "__crepusCheckedCleanup",
        BindingKind::Open => "__crepusOpenCleanup",
        BindingKind::Files => "__crepusFilesCleanup",
        BindingKind::Value => "__crepusValueCleanup",
        BindingKind::Select => "__crepusSelectCleanup",
    });
    let event_prop = if open {
        "onToggle"
    } else if checked || files || select {
        "onChange"
    } else {
        "onInput"
    };
    let event_name = if open {
        "toggle"
    } else if checked || files || select {
        "change"
    } else {
        "input"
    };
    let property = if open {
        "open"
    } else if checked {
        "checked"
    } else if files {
        "files"
    } else {
        "value"
    };
    let value_type = if files {
        format!("{}.FileList|null", globals.types)
    } else {
        (if checked || open { "boolean" } else { "any" }).to_owned()
    };
    let read_type = if files {
        format!("{value_type}|undefined")
    } else {
        (if checked || open {
            "boolean|null|undefined"
        } else {
            "any"
        })
        .to_owned()
    };
    let native_binding = match kind {
        BindingKind::Checked => "bindChecked",
        BindingKind::Open => "bindOpen",
        BindingKind::Files => "bindFiles",
        BindingKind::Value => "bindValue",
        BindingKind::Select => "bindSelectValue",
    };
    let element_type = match kind {
        BindingKind::Checked => "HTMLInputElement",
        BindingKind::Open => "HTMLDetailsElement",
        BindingKind::Files => "HTMLInputElement",
        BindingKind::Value => "HTMLInputElement|HTMLTextAreaElement",
        BindingKind::Select => "HTMLSelectElement",
    };
    let handler_type = if select {
        "any"
    } else {
        "EventListener|(EventListenerObject & AddEventListenerOptions)|null"
    };
    let mut ssr_helper = false;
    let mut edits = Vec::new();
    let mut found = false;
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        let is_select = matches!(&element.opening_element.name,JSXElementName::Identifier(name) if name.name=="select");
        if (select && !is_select) || (kind == BindingKind::Value && is_select) {
            continue;
        }
        let event_element = if select {
            "HTMLSelectElement"
        } else if open {
            "HTMLDetailsElement"
        } else if matches!(&element.opening_element.name, JSXElementName::Identifier(name) if name.name=="textarea")
        {
            "HTMLTextAreaElement"
        } else {
            "HTMLInputElement"
        };
        let event_expression = |expression: &JSXExpression<'_>| {
            if policy == crate::svelte_javascript::IrPolicy::Typed
                && (select
                    || expression.as_expression().is_some_and(|value| {
                        matches!(
                            value.get_inner_expression(),
                            Expression::ArrowFunctionExpression(_)
                                | Expression::FunctionExpression(_)
                        )
                    }))
            {
                let typed =
                    typed_event_expression(source, expression, globals, event_element, event_name);
                if select {
                    typed
                } else {
                    format!(
                        "({typed} as unknown as {}.EventListenerOrEventListenerObject|null|undefined)",
                        globals.types
                    )
                }
            } else {
                let span = expression.span();
                source[span.start as usize..span.end as usize].to_owned()
            }
        };
        let attrs = &element.opening_element.attributes;
        let markers: Vec<_> = attrs
            .iter()
            .filter_map(|item| {
                if let JSXAttributeItem::Attribute(attr) = item {
                    if matches!(&attr.name,JSXAttributeName::Identifier(name) if name.name==marker)
                    {
                        Some(attr.as_ref())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        if markers.is_empty() {
            continue;
        }
        if markers.len() != 1 {
            return Err(fail("duplicate binding marker"));
        }
        let input = matches!(&element.opening_element.name,JSXElementName::Identifier(name) if name.name=="input");
        let textarea = matches!(&element.opening_element.name,JSXElementName::Identifier(name) if name.name=="textarea");
        let details = matches!(&element.opening_element.name,JSXElementName::Identifier(name) if name.name=="details");
        if open != details
            || (!open && !select && !input && !(kind == BindingKind::Value && textarea))
        {
            return Err(fail(if open {
                "bind:open requires a details element"
            } else {
                "binding requires a supported input or textarea"
            }));
        }
        if textarea && !element.children.is_empty() {
            return Err(fail(
                "bind:value textarea bodies are not supported yet; use defaultValue",
            ));
        }
        let has_spread = attrs
            .iter()
            .any(|item| matches!(item, JSXAttributeItem::SpreadAttribute(_)));
        let mut input_type = "text".to_owned();
        let mut default_value = None;
        for item in attrs {
            let JSXAttributeItem::Attribute(attr) = item else {
                continue;
            };
            if matches!(&attr.name,JSXAttributeName::Identifier(name) if name.name==property) {
                return Err(fail(&format!(
                    "explicit {property} conflicts with its binding"
                )));
            }
            if select
                && matches!(&attr.name,JSXAttributeName::Identifier(name) if name.name=="multiple")
                && !matches!(
                    &attr.value,
                    None | Some(JSXAttributeValue::StringLiteral(_))
                )
                && !matches!(&attr.value, Some(JSXAttributeValue::ExpressionContainer(container))
                    if matches!(container.expression.as_expression().map(Expression::get_inner_expression), Some(Expression::BooleanLiteral(_) | Expression::StringLiteral(_))))
            {
                return Err(fail("select multiple must be static"));
            }
            if matches!(&attr.name,JSXAttributeName::Identifier(name) if name.name=="defaultValue")
            {
                default_value = Some(attr.as_ref());
            }
            if !open && matches!(&attr.name,JSXAttributeName::Identifier(name) if name.name=="type")
            {
                let Some(JSXAttributeValue::StringLiteral(value)) = &attr.value else {
                    return Err(fail("binding requires a static input type"));
                };
                input_type = value.value.to_ascii_lowercase();
            }
        }
        if checked && input_type != "checkbox" {
            return Err(fail(&format!("{directive} requires static type=checkbox")));
        }
        if files && input_type != "file" {
            return Err(fail("bind:files requires static type=file"));
        }
        if !checked
            && !files
            && input
            && !matches!(
                input_type.as_str(),
                "text"
                    | "search"
                    | "tel"
                    | "url"
                    | "email"
                    | "password"
                    | "number"
                    | "range"
                    | "date"
                    | "datetime-local"
                    | "month"
                    | "week"
                    | "time"
                    | "color"
            )
        {
            return Err(fail(
                "bind:value supports text-like, numeric, date/time, and color inputs; other control types are unsupported",
            ));
        }
        let attr = markers[0];
        let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value else {
            return Err(fail(&format!(
                "{directive} requires an assignable expression"
            )));
        };
        let Some(expression) = container.expression.as_expression() else {
            return Err(fail(&format!(
                "{directive} requires an assignable expression"
            )));
        };
        if !assignable(expression) {
            return Err(fail(&format!(
                "{directive} requires an identifier or member target; function bindings and optional chains are unsupported"
            )));
        }
        if let Expression::Identifier(id) = expression.get_inner_expression() {
            let reference = semantic
                .scoping()
                .get_reference(id.reference_id.get().unwrap());
            let store_symbol = if open && reference.symbol_id().is_none() {
                id.name.strip_prefix('$').and_then(|name| {
                    semantic
                        .scoping()
                        .find_binding(node.scope_id(), name.into())
                })
            } else {
                None
            };
            let Some(symbol) = reference.symbol_id().or(store_symbol) else {
                return Err(fail(&format!("{directive} target must be declared")));
            };
            let flags = semantic.scoping().symbol_flags(symbol);
            if store_symbol.is_none() && (flags.is_const_variable() || flags.is_import()) {
                return Err(fail(&format!(
                    "{directive} cannot assign to a constant binding"
                )));
            }
        }
        let span = expression.span();
        let expression = &source[span.start as usize..span.end as usize];
        found = true;
        if matches!(target, MoonshineModuleTarget::Ssr) {
            if files {
                // Retain type checks without reading the model or invoking server handlers.
                let mut event_check = String::new();
                if policy == crate::svelte_javascript::IrPolicy::Typed {
                    for item in attrs {
                        if let JSXAttributeItem::Attribute(attribute) = item {
                            if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == event_prop)
                            {
                                if let Some(JSXAttributeValue::ExpressionContainer(container)) =
                                    &attribute.value
                                {
                                    event_check = format!(
                                        "void ({}), ",
                                        typed_event_expression(
                                            source,
                                            &container.expression,
                                            globals,
                                            "HTMLInputElement",
                                            "change"
                                        )
                                    );
                                }
                            }
                        }
                    }
                }
                let replacement = format!("files={{false ? ({event_check}(({value}:{value_type})=>({expression}={value}))(null)) : undefined}}");
                if has_spread {
                    // File bindings have no SSR value; retain only unreachable assignment checks.
                    // An empty spread leaves authored/spread files attributes untouched.
                    let check = format!("{{...(false ? {{files:({event_check}(({value}:{value_type})=>({expression}={value}))(null))}} : {{}})}}");
                    edits.push((attr.span.start as usize, attr.span.end as usize, check));
                } else {
                    edits.push((
                        attr.span.start as usize,
                        attr.span.end as usize,
                        replacement,
                    ));
                }
                continue;
            }
            // Check authored assignment targets without constructing or calling a server setter.
            let argument = if checked || open {
                "false"
            } else {
                "undefined"
            };
            let expression = format!(
                "(false ? (({value}:{value_type})=>({expression}={value}))({argument}) : {expression})"
            );
            let expression = if open {
                format!("!!{expression}")
            } else {
                expression
            };
            if select && !has_spread {
                if policy == crate::svelte_javascript::IrPolicy::Typed {
                    for item in attrs {
                        if let JSXAttributeItem::Attribute(attribute) = item {
                            if let (
                                JSXAttributeName::Identifier(name),
                                Some(JSXAttributeValue::ExpressionContainer(container)),
                            ) = (&attribute.name, &attribute.value)
                            {
                                if name.name.starts_with("on") {
                                    let span = container.expression.span();
                                    edits.push((
                                        span.start as usize,
                                        span.end as usize,
                                        typed_event_expression(
                                            source,
                                            &container.expression,
                                            globals,
                                            "HTMLSelectElement",
                                            &name.name[2..].to_ascii_lowercase(),
                                        ),
                                    ));
                                }
                            }
                        }
                    }
                }
                let span = element.opening_element.name.span();
                edits.push((span.start as usize, span.end as usize, helper.clone()));
                if let Some(closing) = &element.closing_element {
                    let span = closing.name.span();
                    edits.push((span.start as usize, span.end as usize, helper.clone()));
                }
                edits.push((
                    attr.span.start as usize,
                    attr.span.end as usize,
                    format!("{value}={{{{value:({expression})}}}}"),
                ));
            } else if has_spread {
                let end = attrs.last().unwrap().span().end as usize;
                edits.push((
                    attr.span.start as usize,
                    attr.span.end as usize,
                    String::new(),
                ));
                edits.push((end, end, format!(" {property}={{{expression}}}")));
            } else if !checked && input {
                ssr_helper = true;
                let mut render =
                    source[element.span.start as usize..element.span.end as usize].to_owned();
                let mut empty = render.clone();
                let mut local = vec![(
                    attr.span.start as usize,
                    attr.span.end as usize,
                    format!("value={{{value}}}"),
                    String::new(),
                )];
                let default_prop = if let Some(default) = default_value {
                    local.push((
                        default.span.start as usize,
                        default.span.end as usize,
                        String::new(),
                        String::new(),
                    ));
                    format!(
                        " {}",
                        &source[default.span.start as usize..default.span.end as usize]
                    )
                } else {
                    String::new()
                };
                local.sort_by_key(|e| (e.0, e.1));
                for (start, end, replacement, absent) in local.into_iter().rev() {
                    let start = start - element.span.start as usize;
                    let end = end - element.span.start as usize;
                    render.replace_range(start..end, &replacement);
                    empty.replace_range(start..end, &absent);
                }
                edits.push((element.span.start as usize,element.span.end as usize,format!("<{helper} get={{()=>({expression})}}{default_prop} render={{({value}:any)=>({render})}} empty={{()=>({empty})}}/>")));
            } else {
                edits.push((
                    attr.span.start as usize,
                    attr.span.end as usize,
                    format!("{property}={{{expression}}}"),
                ));
            }
        } else if grouped {
            let node = fresh("__crepusBoundElement");
            let mut event = String::new();
            for item in attrs {
                if has_spread {
                    continue;
                }
                if let JSXAttributeItem::Attribute(attribute) = item {
                    if matches!(&attribute.name,JSXAttributeName::Identifier(name) if name.name==event_prop)
                    {
                        let Some(JSXAttributeValue::ExpressionContainer(value)) = &attribute.value
                        else {
                            return Err(fail("binding event requires an expression"));
                        };
                        event = if open {
                            format!(" {event_prop}={{{}}}", event_expression(&value.expression))
                        } else {
                            format!(",{event_prop}:({})", event_expression(&value.expression))
                        };
                        edits.push((
                            attribute.span.start as usize,
                            attribute.span.end as usize,
                            String::new(),
                        ));
                    }
                }
            }
            // Keep open handlers in JSX until the shared late-event pass has visited them.
            let install = if open {
                format!("<{helper} get={{()=>({expression})}} set={{({value}:{value_type})=>({expression}={value})}} render={{()=>{node}}}{event}/>")
            } else {
                format!("{helper}({{get:()=>({expression}),set:({value}:{value_type})=>({expression}={value}),render:()=>{node}{event}}})")
            };
            edits.push((
                attr.span.start as usize,
                attr.span.end as usize,
                format!("{marker}={{({node}:{element_type})=>{install}}}"),
            ));
        } else {
            edits.push((
                attr.span.start as usize,
                attr.span.end as usize,
                String::new(),
            ));
            let mut change = String::new();
            for item in attrs {
                if let JSXAttributeItem::Attribute(event) = item {
                    if matches!(&event.name,JSXAttributeName::Identifier(name) if name.name==event_prop)
                    {
                        if select || policy == crate::svelte_javascript::IrPolicy::Typed {
                            let Some(JSXAttributeValue::ExpressionContainer(container)) =
                                &event.value
                            else {
                                return Err(fail("binding event requires an expression"));
                            };
                            change.push_str(&format!(
                                " {event_prop}={{{}}}",
                                event_expression(&container.expression)
                            ));
                        } else {
                            change.push(' ');
                            change.push_str(
                                &source[event.span.start as usize..event.span.end as usize],
                            );
                        }
                        edits.push((
                            event.span.start as usize,
                            event.span.end as usize,
                            String::new(),
                        ));
                    }
                }
            }
            edits.push((element.span.start as usize,element.span.start as usize,format!("<{helper} get={{()=>({expression})}} set={{({value}:{value_type})=>({expression}={value})}}{change} render={{()=>(")));

            edits.push((
                element.span.end as usize,
                element.span.end as usize,
                ")}/>".into(),
            ));
        }
    }
    if !found {
        return Ok((source.to_owned(), None));
    }
    edits.sort_by_key(|(start, end, _)| (*start, *end));
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    if matches!(target, MoonshineModuleTarget::Dom) {
        let binding_source = if checked || open {
            "@tschk/moonshine-dom/binding"
        } else {
            "@tschk/moonshine-svelte/binding"
        };
        out = format!("import {{{native_binding} as {bind}}} from {binding_source:?};\n{out}");
        out = format!(
            "import {{onCleanup as {cleanup}}} from \"@tschk/moonshine-dom/reactive\";\n{out}"
        );
        let realm = &globals.realm;
        let listen_before = if open {
            "listen();"
        } else {
            "if(options?.capture)listen();"
        };
        let listen_after = if open {
            ""
        } else {
            "if(!options?.capture)listen();"
        };
        out.push_str(&format!(r#"
function {helper}(props:{{get:()=>{read_type};set:(value:{value_type})=>void;render:()=>any;{event_prop}?:{handler_type}}}):any{{
 const input=props.render() as {element_type};
 const value=props.{event_prop};
 const object=typeof value==='object'&&value!==null?value as EventListenerObject & AddEventListenerOptions:undefined;
 if(value!=null&&typeof value!=='function'&&typeof object?.handleEvent!=='function')throw new {realm}.TypeError('{event_prop} must be an event handler or handleEvent object');
 const options=object?{{capture:!!object.capture,once:object.once,passive:object.passive,signal:object.signal}}:undefined;
 const listen=()=>{{if(value==null)return;input.addEventListener('{event_name}',value,options);{cleanup}(()=>input.removeEventListener('{event_name}',value,options?.capture));}};
 {listen_before}
 {bind}(input,props.get,props.set);
 {listen_after}
 return input;
}}
"#));
        Ok((out, Some(helper)))
    } else if select {
        let realm = &globals.realm;
        let factory = fresh("__crepusSelectFactory");
        let symbol = fresh("__crepusSelectBinding");
        out = format!("import {{hSource as {factory},selectBinding as {symbol}}} from \"@tschk/moonshine-dom/ssr\";\n{out}\nfunction {helper}(props:any):any{{const attrs={realm}.Object.create(null);for(const key of {realm}.Object.keys(props)){{if(key!==\"children\"&&key!==\"{value}\")attrs[key]=props[key];}}return {factory}(\"select\",{{...attrs,[{symbol}]:{{...props.{value},preserveSelected:true}}}},props.children);}}\n");
        Ok((out, Some(helper)))
    } else if ssr_helper {
        out.push_str(&format!("\nfunction {helper}(props:{{get:()=>any;defaultValue?:any;render:(value:any)=>any;empty:()=>any}}):any{{void props.defaultValue;const value=props.get();return value==null?props.empty():props.render(value);}}\n"));
        Ok((out, Some(helper)))
    } else {
        Ok((out, None))
    }
}

/// Give each independently compiled source option its literal comparison identity.
pub(crate) fn transform_ssr_options(
    source: &str,
    path: &Path,
    globals: &crate::source_globals::SourceGlobals,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<(String, Option<String>), String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(format!("{}: invalid SSR option syntax", path.display()));
    }
    let fresh = |base: &str| {
        let mut value = base.to_owned();
        while source.contains(&value) {
            value.push('_');
        }
        value
    };
    let helper = fresh("__CrepusSourceOption");
    let read = fresh("__crepusOptionBox");
    let factory = fresh("__crepusOptionFactory");
    let symbol = fresh("__crepusOptionBinding");
    struct Options<'s> {
        source: &'s str,
        helper: &'s str,
        read: &'s str,
        globals: &'s crate::source_globals::SourceGlobals,
        policy: crate::svelte_javascript::IrPolicy,
        edits: Vec<(usize, usize, String)>,
    }
    impl<'a> oxc_ast_visit::Visit<'a> for Options<'_> {
        fn visit_jsx_element(&mut self, element: &JSXElement<'a>) {
            if matches!(&element.opening_element.name, JSXElementName::Identifier(name) if name.name == "option")
            {
                if let Some(attribute) = element.opening_element.attributes.iter().find_map(|item| {
                    match item {
                        JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "value") => Some(attribute),
                        _ => None,
                    }
                }) {
                    let expression = match &attribute.value {
                        Some(JSXAttributeValue::ExpressionContainer(container)) => {
                            let span = container.expression.span();
                            self.source[span.start as usize..span.end as usize].to_owned()
                        }
                        Some(JSXAttributeValue::StringLiteral(value)) => serde_json::to_string(&crate::svelte_entities::decode_entities(value.value.as_str(), true)).unwrap(),
                        None => "true".to_owned(),
                        _ => return,
                    };
                    if self.policy == crate::svelte_javascript::IrPolicy::Typed {
                        for item in &element.opening_element.attributes {
                            if let JSXAttributeItem::Attribute(attribute) = item {
                                if let (JSXAttributeName::Identifier(name), Some(JSXAttributeValue::ExpressionContainer(container))) = (&attribute.name, &attribute.value) {
                                    if name.name.starts_with("on") {
                                        let span=container.expression.span();
                                        self.edits.push((span.start as usize,span.end as usize,typed_event_expression(self.source,&container.expression,self.globals,"HTMLOptionElement",&name.name[2..].to_ascii_lowercase())));
                                    }
                                }
                            }
                        }
                    }
                    let span = element.opening_element.name.span();
                    self.edits.push((span.start as usize, span.end as usize, self.helper.to_owned()));
                    if let Some(closing) = &element.closing_element {
                        let span = closing.name.span();
                        self.edits.push((span.start as usize, span.end as usize, self.helper.to_owned()));
                    }
                    self.edits.push((attribute.span.start as usize, attribute.span.end as usize, format!("{}={{{{value:({expression})}}}}", self.read)));
                }
            }
            oxc_ast_visit::walk::walk_jsx_element(self, element);
        }
    }
    let mut options = Options {
        source,
        helper: &helper,
        read: &read,
        globals,
        policy,
        edits: Vec::new(),
    };
    oxc_ast_visit::Visit::visit_program(&mut options, &parsed.program);
    if options.edits.is_empty() {
        return Ok((source.to_owned(), None));
    }
    options.edits.sort_by_key(|(start, end, _)| (*start, *end));
    if options.edits.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(format!(
            "{}: nested JSX inside an option value is unsupported",
            path.display()
        ));
    }
    let mut output = source.to_owned();
    for (start, end, value) in options.edits.into_iter().rev() {
        output.replace_range(start..end, &value);
    }
    let realm = &globals.realm;
    output = format!("import {{hSource as {factory},optionBinding as {symbol}}} from \"@tschk/moonshine-dom/ssr\";\n{output}\nfunction {helper}(props:any):any{{const attrs={realm}.Object.create(null);for(const key of {realm}.Object.keys(props)){{if(key!==\"children\")attrs[key===\"{read}\"?\"value\":key]=props[key];}}const box=attrs.value;return {factory}(\"option\",{{...attrs,[{symbol}]:box}},props.children);}}\n");
    Ok((output, Some(helper)))
}

pub(crate) fn erase_factory_client_attributes(
    source: &str,
    namespace: &str,
    helpers: &[String],
) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid SSR selection factory syntax".into());
    }
    struct Erase<'s> {
        namespace: &'s str,
        helpers: &'s [String],
        spans: Vec<oxc_span::Span>,
    }
    impl<'a> oxc_ast_visit::Visit<'a> for Erase<'_> {
        fn visit_jsx_element(&mut self, element: &JSXElement<'a>) {
            if matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if self.helpers.iter().any(|helper| helper == name.name.as_str()))
            {
                for item in &element.opening_element.attributes {
                    if let JSXAttributeItem::Attribute(attribute) = item {
                        if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == self.namespace)
                        {
                            self.spans.push(attribute.span);
                        }
                    }
                }
            }
            oxc_ast_visit::walk::walk_jsx_element(self, element);
        }
    }
    let mut erase = Erase {
        namespace,
        helpers,
        spans: Vec::new(),
    };
    oxc_ast_visit::Visit::visit_program(&mut erase, &parsed.program);
    let mut output = source.to_owned();
    erase.spans.sort_by_key(|span| span.start);
    for span in erase.spans.into_iter().rev() {
        output.replace_range(span.start as usize..span.end as usize, "");
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lower(
        source: &str,
        target: MoonshineModuleTarget,
    ) -> Result<(String, Option<String>), String> {
        transform(
            source,
            Path::new("input.svelte"),
            target,
            "marker",
            BindingKind::Checked,
        )
        .inspect(|out| {
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &out.0, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}\n{}", parsed.errors, out.0);
        })
    }
    fn lower_value(
        source: &str,
        target: MoonshineModuleTarget,
    ) -> Result<(String, Option<String>), String> {
        transform(
            source,
            Path::new("value.svelte"),
            target,
            "marker",
            BindingKind::Value,
        )
        .inspect(|out| {
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &out.0, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}\n{}", parsed.errors, out.0);
        })
    }
    fn lower_select(
        source: &str,
        target: MoonshineModuleTarget,
    ) -> Result<(String, Option<String>), String> {
        transform(
            source,
            Path::new("select.svelte"),
            target,
            "marker",
            BindingKind::Select,
        )
        .inspect(|out| {
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &out.0, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}\n{}", parsed.errors, out.0);
        })
    }
    #[test]
    fn select_binding_imports_helper_and_source_handlers_follow_bind() {
        let (out, helper) = lower_select(
            "export default function C(){let v;return <select multiple onChange={()=>after()} marker={v}><option value=\"a\">a</option></select>}",
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert_eq!(helper.as_deref(), Some("__CrepusSelectValue"));
        assert!(out.contains("import {bindSelectValue as __crepusBindSelect} from \"@tschk/moonshine-svelte/binding\""));
        assert!(out.contains("as HTMLSelectElement"));
        assert!(out.contains("addEventListener('change',value,options)"));
        assert!(out.contains(
            " __crepusBindSelect(input,props.get,props.set);\n if(!options?.capture)listen();"
        ));
        assert!(out.contains("onChange={((()=>after()) satisfies"));
        assert!(out.contains("multiple onChange") || out.contains("<select multiple"));
    }
    #[test]
    fn select_binding_rejects_bad_targets_and_emits_typed_ssr() {
        for source in [
            "function C(){let v;return <select multiple={m} marker={v}/>}",
            "function C(){let v;return <select value={x} marker={v}/>}",
            "function C(){const v=1;return <select marker={v}/>}",
        ] {
            assert!(
                lower_select(source, MoonshineModuleTarget::Dom).is_err(),
                "{source}"
            );
        }
        assert!(lower_select(
            "function C(){let v;return <select {...p} marker={v}/>}",
            MoonshineModuleTarget::Dom,
        )
        .is_ok());
        let (output, helper) = lower_select(
            "function C(){let v;return <select marker={v}/>}",
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert!(helper.is_some());
        assert!(output.contains("selectBinding as"));
        assert!(output.contains("preserveSelected:true"));
        assert!(output.contains("value:("));
    }
    #[test]
    fn normalized_static_multiple_literals_are_accepted() {
        for value in ["true", "false", "\"\""] {
            let source = format!(
                "function C(){{let v;return <select multiple={{{value}}} marker={{v}}/>;}}"
            );
            assert!(lower_select(&source, MoonshineModuleTarget::Dom).is_ok());
        }
    }

    #[test]
    fn select_and_input_passes_leave_each_others_markers_untouched() {
        let input = "function C(){let v;return <input marker={v}/>;}";
        assert_eq!(
            lower_select(input, MoonshineModuleTarget::Dom).unwrap(),
            (input.to_owned(), None)
        );
        let select = "function C(){let v;return <select marker={v}/>;}";
        assert_eq!(
            lower_value(select, MoonshineModuleTarget::Dom).unwrap(),
            (select.to_owned(), None)
        );
    }

    #[test]
    fn server_options_box_raw_values_across_independent_components() {
        let source = "function Child(){return <option value={fn}>text</option>}";
        let (output, helper) = transform_ssr_options(
            source,
            Path::new("Child.svelte"),
            &Default::default(),
            crate::svelte_javascript::IrPolicy::Typed,
        )
        .unwrap();
        assert!(helper.is_some());
        assert!(output.contains("optionBinding as"));
        assert!(output.contains("__crepusOptionBox={{value:(fn)}}"));
        assert!(output.contains("const box=attrs.value"));
        assert!(!output.contains("fn()"));
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{output}");
        let plain = "function Child(){return <option>text</option>}";
        assert_eq!(
            transform_ssr_options(
                plain,
                Path::new("Child.svelte"),
                &Default::default(),
                crate::svelte_javascript::IrPolicy::Typed
            )
            .unwrap(),
            (plain.to_owned(), None)
        );
    }

    #[test]
    fn server_option_value_overlap_fails_before_applying_edits() {
        let error = transform_ssr_options(
            "function C(){return <option value={<option value={x}/>}/>}",
            Path::new("C.svelte"),
            &Default::default(),
            crate::svelte_javascript::IrPolicy::Typed,
        )
        .unwrap_err();
        assert!(error.contains("nested JSX inside an option value"));
    }

    #[test]
    fn selection_factory_erasure_keeps_child_and_unrelated_namespace_witnesses() {
        let source = r#"const x=<Factory Witness="html"><mi Witness="math"/></Factory>;const y=<Other Witness="html"/>;"#;
        let output =
            erase_factory_client_attributes(source, "Witness", &["Factory".into()]).unwrap();
        assert!(!output.contains("Factory Witness"));
        assert!(output.contains("mi Witness=\"math\""));
        assert!(output.contains("Other Witness=\"html\""));
    }

    #[test]
    fn server_factory_witness_erasure_preserves_handler_evaluation() {
        let source = "const x=<Factory Witness=\"html\" onChange={readHandler()}/>;const y=<Other onChange={readHandler()}/>;";
        let output =
            erase_factory_client_attributes(source, "Witness", &["Factory".into()]).unwrap();
        assert_eq!(output.matches("readHandler()").count(), 2);
        assert!(output.contains("Other onChange"));
    }

    #[test]
    fn unbound_select_is_untouched() {
        let source = "function C(){return <select><option>a</option></select>}";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert_eq!(
                lower_select(source, target).unwrap(),
                (source.to_owned(), None)
            );
        }
    }
    #[test]
    fn supplied_realm_constructs_helper_error_without_ambient_global() {
        let globals = crate::source_globals::SourceGlobals::with_aliases("__r", "__T", true);
        let (out, _) = transform_template_with_policy(
            "function C(){let on=true;return <input type=\"checkbox\" marker={on}/>;}",
            Path::new("input.svelte"),
            MoonshineModuleTarget::Dom,
            "marker",
            BindingKind::Checked,
            false,
            (crate::svelte_javascript::IrPolicy::Typed, &globals),
        )
        .unwrap();
        assert!(out.contains("throw new (__r()).TypeError("), "{out}");
        assert!(!out.contains("globalThis"), "{out}");
    }
    #[test]
    fn bound_value_handlers_check_precise_input_and_textarea_context() {
        for (tag, element) in [
            ("input", "HTMLInputElement"),
            ("textarea", "HTMLTextAreaElement"),
        ] {
            let source = format!("export default function C(){{let value;return <{tag} marker={{value}} onInput={{event=>event.currentTarget}}/>}}");
            let (output, _) = lower_value(&source, MoonshineModuleTarget::Dom).unwrap();
            assert!(
                output.contains(&format!("currentTarget:globalThis.{element}")),
                "{output}"
            );
            assert!(output.contains("satisfies"), "{output}");
            assert!(
                output.contains("as unknown as globalThis.EventListenerOrEventListenerObject"),
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
    }

    #[test]
    fn value_binding_reuses_owned_factories_and_input_listener_order() {
        for control in [
            "<input marker={text} onInput={()=>after()}/>",
            "<input type=\"number\" marker={text}/>",
            "<input type=\"range\" marker={text}/>",
            "<textarea marker={text} defaultValue=\"start\"/>",
        ] {
            let (out, helper) = lower_value(
                &format!("export default function C(){{let text;return {control}}}"),
                MoonshineModuleTarget::Dom,
            )
            .unwrap();
            assert_eq!(helper.as_deref(), Some("__CrepusValueInput"));
            assert!(out.contains("bindValue as __crepusBindValue"));
            assert!(out.contains("@tschk/moonshine-svelte/binding"));
            assert!(!out.contains("numeric="));
            assert!(out.contains("addEventListener('input',value,options)"));
            assert!(out.contains("if(!options?.capture)listen()"));
        }
    }
    #[test]
    fn server_input_selects_attribute_presence_and_preserves_ignored_default_evaluation() {
        let(out,helper)=lower_value("export default function C(){let text;return <input defaultValue={makeDefault()} title={title()} marker={text}/>}",MoonshineModuleTarget::Ssr).unwrap();
        assert_eq!(helper.as_deref(), Some("__CrepusValueInput"));
        assert!(!out.contains("import"));
        assert_eq!(out.matches("makeDefault()").count(), 1);
        assert!(out.contains("void props.defaultValue;const value=props.get();return value==null?props.empty():props.render(value)"));
        let absent = out
            .split("empty={()=>(")
            .nth(1)
            .unwrap()
            .split(")}/>")
            .next()
            .unwrap();
        assert!(!absent.contains("defaultValue"));
        assert!(!absent.contains("value="));
        let(area,helper)=lower_value("export default function C(){let text;return <textarea defaultValue=\"start\" marker={text}/>}",MoonshineModuleTarget::Ssr).unwrap();
        assert!(helper.is_none());
        assert!(area.contains("value={(false ? (("));
        assert!(area.contains("(__crepusInputValue:any)=>(text=__crepusInputValue)"));
        assert!(!area.contains("import"));
    }
    #[test]
    fn fixed_date_time_and_color_values_reuse_string_binding() {
        for kind in ["date", "datetime-local", "month", "week", "time", "color"] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let source = format!(
                    "function C(){{let text;return <input type=\"{kind}\" marker={{text}}/>}}"
                );
                let (output, _) = lower_value(&source, target).unwrap();
                assert!(output.contains("__crepusInputValue"));
                if matches!(target, MoonshineModuleTarget::Dom) {
                    assert!(output.contains("bindValue as"));
                }
            }
        }
    }

    #[test]
    fn value_binding_unsupported_controls_and_targets_fail_explicitly() {
        for control in [
            "<input type=\"checkbox\" marker={text}/>",
            "<input type=\"file\" marker={text}/>",
            "<input type={kind} marker={text}/>",
            "<input value=\"x\" marker={text}/>",
            "<textarea marker={text}>body</textarea>",
            "<textarea marker={text===true}/>",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(
                    lower_value(
                        &format!("function C(){{let text;return {control}}}"),
                        target
                    )
                    .is_err(),
                    "{control}"
                );
            }
        }
        for source in [
            "function C(){const text='';return <input marker={text}/>}",
            "function C(){return <input marker={missing}/>}",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(lower_value(source, target).is_err(), "{source}");
            }
        }
    }
    #[test]
    fn checkbox_binding_uses_owned_render_factory_and_scope_visible_setter() {
        let(out,helper)=lower("export default function C(){let checked;return <div><input type=\"checkbox\" defaultChecked={true} onChange={()=>before()} marker={checked}/></div>}",MoonshineModuleTarget::Dom).unwrap();
        assert_eq!(helper.as_deref(), Some("__CrepusCheckedInput"));
        assert!(out.contains("get={()=> (checked)}") || out.contains("get={()=>(checked)}"));
        assert!(out.contains("checked=__crepusCheckedValue"));
        assert!(out.contains("render={()=>(<input"));
        assert!(out.contains("()=>before()) satisfies"), "{out}");
        assert!(out.contains("if(options?.capture)listen();\n __crepusBindChecked(input,props.get,props.set);\n if(!options?.capture)listen();"));
        let input = out
            .split("render={()=>(")
            .nth(1)
            .unwrap()
            .split(")}/>")
            .next()
            .unwrap();
        assert!(!input.contains("onChange"));
        assert!(out.contains(
            "__crepusCheckedCleanup(()=>input.removeEventListener('change',value,options?.capture))"
        ));
    }
    #[test]
    fn change_listener_options_and_server_handlers_are_preserved() {
        let source = "export default function C(){let checked;return <input type=\"checkbox\" marker={checked} onChange={{handleEvent(event){},capture:true,once:true,passive:true,signal:abort.signal}}/>}";
        let (out, _) = lower(source, MoonshineModuleTarget::Dom).unwrap();
        assert!(out.contains("onChange={{handleEvent(event){},capture:true,once:true,passive:true,signal:abort.signal}} render"));
        assert!(
            out.contains(
                "onChange?:EventListener|(EventListenerObject & AddEventListenerOptions)|null"
            ),
            "{out}"
        );
        assert!(out.contains("once:object.once,passive:object.passive,signal:object.signal"));
        let (ssr, _) = lower(source, MoonshineModuleTarget::Ssr).unwrap();
        assert!(ssr.contains("onChange={{handleEvent"));
        assert!(!ssr.contains("addEventListener"));
    }
    #[test]
    fn server_checks_assignment_without_executing_setter_or_adopting_default() {
        let(out,helper)=lower("export default function C(){let checked;return <input type=\"checkbox\" defaultChecked={true} marker={checked}/>}",MoonshineModuleTarget::Ssr).unwrap();
        assert!(helper.is_none());
        assert!(out.contains("defaultChecked={true} checked={(false ? (("));
        assert!(!out.contains("import"));
        assert_eq!(out.matches("checked={(false ?").count(), 1);
        assert!(out.contains("(__crepusCheckedValue:boolean)=>(checked=__crepusCheckedValue)"));
    }
    #[test]
    fn setter_parameters_reserve_decoded_authored_names() {
        let source = r#"export default function C(){let \u005f_crepusInputValue='x';return <textarea marker={\u005f_crepusInputValue}/>}"#;
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let (out, _) = lower_value(source, target).unwrap();
            assert!(out.contains("(__crepusInputValue_:any)=>"), "{out}");
            assert!(
                out.contains(r#"\u005f_crepusInputValue=__crepusInputValue_"#),
                "{out}"
            );
        }
    }
    #[test]
    fn member_targets_hygiene_and_no_bindings() {
        let(out,helper)=lower("const __CrepusCheckedInput=0;export default function C(){const state={checked:false};return <input type=\"checkbox\" marker={state.checked}/>}",MoonshineModuleTarget::Dom).unwrap();
        assert_eq!(helper.as_deref(), Some("__CrepusCheckedInput_"));
        assert!(out.contains("state.checked=__crepusCheckedValue"));
        let source = "export default ()=> <input type=\"checkbox\"/>";
        assert_eq!(
            lower(source, MoonshineModuleTarget::Dom).unwrap(),
            (source.into(), None)
        );
    }
    #[test]
    fn member_bindings_require_identifier_roots_but_preserve_computed_keys() {
        for kind in [BindingKind::Checked, BindingKind::Value] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let input_type = if kind == BindingKind::Checked {
                    "checkbox"
                } else {
                    "text"
                };
                for expression in [
                    "get().value",
                    "(new Box()).value",
                    "({value:false}).value",
                    "this.value",
                    "[false][0]",
                ] {
                    let source = format!(
                        "function C(){{return <input type=\"{input_type}\" marker={{{expression}}}/>}}"
                    );
                    assert!(
                        transform(&source, Path::new("input.svelte"), target, "marker", kind)
                            .is_err(),
                        "{expression}"
                    );
                }
                for expression in [
                    "foo.bar",
                    "buckets[choose()].value",
                    "buckets[key?.value]",
                    "missing.member",
                    "namespace.member",
                    "((foo as any)).bar",
                ] {
                    let source = format!(
                        "import * as namespace from './fields';function C(){{return <input type=\"{input_type}\" marker={{{expression}}}/>}}"
                    );
                    let (out, _) =
                        transform(&source, Path::new("input.svelte"), target, "marker", kind)
                            .unwrap_or_else(|error| panic!("{expression}: {error}"));
                    let allocator = Allocator::default();
                    assert!(
                        Parser::new(&allocator, &out, SourceType::tsx())
                            .parse()
                            .errors
                            .is_empty(),
                        "{out}"
                    );
                }
                for expression in ["foo?.bar", "foo?.bar.value", "(foo?.bar).value"] {
                    let source = format!(
                        "function C(){{return <input type=\"{input_type}\" marker={{{expression}}}/>}}"
                    );
                    assert!(
                        transform(&source, Path::new("input.svelte"), target, "marker", kind)
                            .is_err(),
                        "{expression}"
                    );
                }
            }
        }
    }
    #[test]
    fn invalid_targets_and_inputs_reject_for_both_hosts() {
        for source in [
            "function C(){const checked=false;return <input type=\"checkbox\" marker={checked}/>}",
            "function C(){return <input type=\"checkbox\" marker={missing}/>}",
            "function C(){let checked;return <input type=\"text\" marker={checked}/>}",
            "function C(){let checked;return <input type={\"checkbox\"} marker={checked}/>}",
            "function C(){let checked;return <input type=\"checkbox\" checked={true} marker={checked}/>}",
            "function C(){let obj;return <input type=\"checkbox\" marker={obj?.checked}/>}",
            "function C(){let checked;return <input type=\"checkbox\" marker={checked===true}/>}",
            "function C(){let checked;return <input type=\"checkbox\" marker={()=>checked,v=>checked=v}/>}",
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(lower(source, target).is_err(), "{source}");
            }
        }
    }
}

#[cfg(test)]
mod files_tests {
    use super::*;
    #[test]
    fn files_use_the_native_binding_and_change_event_contract() {
        let (out, helper) = transform(
            "export default function C(){let files;return <input type=\"file\" marker={files} onChange={event=>event.currentTarget.files}/>}",
            Path::new("files.svelte"), MoonshineModuleTarget::Dom, "marker", BindingKind::Files,
        ).unwrap();
        assert!(helper.unwrap().contains("FilesInput"));
        assert!(out.contains("bindFiles as"));
        assert!(out.contains("FileList|null"));
        assert!(out.contains("currentTarget:globalThis.HTMLInputElement"));
        assert!(out.contains("GlobalEventHandlersEventMap[\"change\"]"));
        assert!(out.contains("addEventListener('change'"));
        assert!(!out.contains("marker={files}"));
    }
    #[test]
    fn files_server_binding_is_an_unreachable_type_check() {
        let (out, helper) = transform(
            "export default function C(){const model={get files(){throw Error('read')},set files(value:FileList|null){}};return <input type=\"file\" marker={model.files} onChange={event=>event.currentTarget.files}/>}",
            Path::new("files.svelte"), MoonshineModuleTarget::Ssr, "marker", BindingKind::Files,
        ).unwrap();
        assert!(helper.is_none());
        assert!(out.contains("files={false ?"));
        assert!(out.contains(": undefined}"));
        assert!(!out.contains("bindFiles"));
        assert!(out.contains("GlobalEventHandlersEventMap[\"change\"]"));
    }
    #[test]
    fn files_invalid_controls_and_targets_reject_on_both_targets() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for source in [
                "export default function C(){let files;return <input type=\"text\" marker={files}/>}",
                "export default function C(){let files;return <textarea marker={files}/>}",
                "export default function C(){const files=null;return <input type=\"file\" marker={files}/>}",
                "export default function C(){let files;return <input type=\"file\" marker={files} files={null}/>}",
            ] {
                assert!(transform(source,Path::new("files.svelte"),target,"marker",BindingKind::Files).is_err(), "{source}");
            }
        }
    }
}

#[cfg(test)]
mod open_tests {
    use super::*;

    fn lower_open(
        source: &str,
        target: MoonshineModuleTarget,
        grouped: bool,
    ) -> Result<(String, Option<String>), String> {
        transform_template(
            source,
            Path::new("open.svelte"),
            target,
            "marker",
            BindingKind::Open,
            grouped,
        )
        .inspect(|out| {
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &out.0, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}\n{}", parsed.errors, out.0);
        })
    }

    #[test]
    fn details_open_uses_the_native_helper_after_authored_toggle_listeners() {
        let (out, helper) = lower_open(
            "export default function C(){let expanded;return <div><details onToggle={()=>after()} marker={expanded}><summary>x</summary></details></div>}",
            MoonshineModuleTarget::Dom,
            false,
        )
        .unwrap();
        assert_eq!(helper.as_deref(), Some("__CrepusOpenDetails"));
        assert!(out.contains(
            "import {bindOpen as __crepusBindOpen} from \"@tschk/moonshine-dom/binding\";"
        ));
        assert!(!out.contains("@tschk/moonshine-svelte/binding"));
        assert!(out.contains("get={()=>(expanded)}"), "{out}");
        assert!(out.contains("set={(__crepusOpenValue:boolean)=>(expanded=__crepusOpenValue)}"));
        assert!(out.contains("render={()=>(<details"));
        assert!(out.contains("()=>after()) satisfies"), "{out}");
        assert!(out.contains("GlobalEventHandlersEventMap[\"toggle\"]"));
        assert!(out.contains("currentTarget:globalThis.HTMLDetailsElement"));
        assert!(out.contains("const input=props.render() as HTMLDetailsElement;"));
        assert!(out.contains(
            "get:()=>boolean|null|undefined;set:(value:boolean)=>void;render:()=>any;onToggle?:"
        ));
        assert!(out.contains("listen();\n __crepusBindOpen(input,props.get,props.set);"));
        assert!(!out.contains("if(!options?.capture)listen();"));
        assert!(out.contains(
            "__crepusOpenCleanup(()=>input.removeEventListener('toggle',value,options?.capture))"
        ));
        let details = out
            .split("render={()=>(")
            .nth(1)
            .unwrap()
            .split(")}/>")
            .next()
            .unwrap();
        assert!(!details.contains("onToggle"));
        assert!(!details.contains("marker"));
    }

    #[test]
    fn grouped_details_open_calls_the_same_helper_with_a_typed_element() {
        let (out, helper) = lower_open(
            "export default function C(){let expanded;return <details onToggle={()=>after()} marker={expanded}/>}",
            MoonshineModuleTarget::Dom,
            true,
        )
        .unwrap();
        assert_eq!(helper.as_deref(), Some("__CrepusOpenDetails"));
        assert!(
            out.contains(
                "marker={(__crepusBoundElement:HTMLDetailsElement)=><__CrepusOpenDetails get={()=>"
            ),
            "{out}"
        );
        assert!(out.contains(" onToggle={"), "{out}");
    }

    #[test]
    fn server_open_is_a_boolean_snapshot_without_helper_listener_or_import() {
        let (out, helper) = lower_open(
            "export default function C(){let expanded;return <details onToggle={()=>after()} marker={expanded}/>}",
            MoonshineModuleTarget::Ssr,
            false,
        )
        .unwrap();
        assert!(helper.is_none());
        assert!(out.contains("open={!!(false ? ((__crepusOpenValue:boolean)=>(expanded=__crepusOpenValue))(false) : expanded)}"), "{out}");
        assert!(out.contains("onToggle={()=>after()}"), "{out}");
        assert!(!out.contains("import"));
        assert!(!out.contains("bindOpen"));
        assert!(!out.contains("addEventListener"));
        assert!(!out.contains("marker"));
    }

    #[test]
    fn open_helpers_are_hygienic_shared_and_nest_without_crossing_edits() {
        let (out, helper) = lower_open(
            "const __CrepusOpenDetails=0;export default function C(){const state={a:false,b:false};return <><details marker={state.a}><details marker={state.b}></details></details><details marker={state.a}></details></>}",
            MoonshineModuleTarget::Dom,
            false,
        )
        .unwrap();
        assert_eq!(helper.as_deref(), Some("__CrepusOpenDetails_"));
        assert_eq!(out.matches("<__CrepusOpenDetails_ get=").count(), 3);
        assert_eq!(out.matches("function __CrepusOpenDetails_(").count(), 1);
        assert!(out.contains("state.a=__crepusOpenValue"));
        assert!(out.contains("state.b=__crepusOpenValue"));
        let source = "export default ()=> <details/>";
        assert_eq!(
            lower_open(source, MoonshineModuleTarget::Dom, false).unwrap(),
            (source.into(), None)
        );
    }

    #[test]
    fn open_rejects_other_elements_conflicts_and_unassignable_targets() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for source in [
                "export default function C(){let expanded;return <div marker={expanded}/>}",
                "export default function C(){let expanded;return <dialog marker={expanded}/>}",
                "export default function C(){let expanded;return <input marker={expanded}/>}",
                "export default function C(){let expanded;return <Details marker={expanded}/>}",
                "export default function C(){let expanded;return <details marker={expanded} open={true}/>}",
                "export default function C(){let expanded;return <details marker={expanded} {...rest} open={true}/>}",
                "export default function C(){const expanded=false;return <details marker={expanded}/>}",
                "import expanded from 'state';export default function C(){return <details marker={expanded}/>}",
                "export default function C(){return <details marker={missing}/>}",
                "export default function C(){let expanded;return <details marker={expanded||false}/>}",
                "export default function C(){let model;return <details marker={model?.open}/>}",
                "export default function C(){let model;return <details marker={model.get().open}/>}",
                "export default function C(){let expanded;return <details marker={[()=>expanded,(value)=>expanded=value]}/>}",
                "export default function C(){let expanded;return <details marker=\"expanded\"/>}",
                "export default function C(){let expanded;return <details marker/>}",
            ] {
                assert!(
                    lower_open(source, target, false).is_err(),
                    "{source}"
                );
            }
            assert!(lower_open(
                "export default function C(){let expanded;return <details marker={expanded} {...rest}/>}" ,
                target,
                true,
            ).is_ok());
        }
    }
}
