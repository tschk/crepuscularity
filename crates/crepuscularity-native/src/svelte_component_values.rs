//! Carry component value bindings as descriptors through ordinary prop lowering.
use crate::MoonshineModuleTarget;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

#[cfg(test)]
pub(crate) fn prepare(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    bindings: &[(String, String)],
    component: &str,
) -> Result<(String, Option<String>), String> {
    prepare_with_policy(
        source,
        path,
        target,
        bindings,
        component,
        crate::svelte_javascript::IrPolicy::Typed,
    )
}

pub(crate) fn prepare_with_policy(
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
    bindings: &[(String, String)],
    component: &str,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<(String, Option<String>), String> {
    let fail = |message: &str| format!("{}: component binding {message}", path.display());
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err(fail("has invalid syntax"));
    }
    policy.configure(&mut parsed.program);
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    if !semantic.errors.is_empty() {
        return Err(fail("has invalid scope"));
    }
    let semantic = semantic.semantic;
    let fresh = |base: &str| {
        let mut name = base.to_owned();
        while source.contains(&name) {
            name.push('_');
        }
        name
    };
    let value = fresh("__crepusComponentValue");
    let invalidate = fresh("__crepusBindingInvalidate");
    let next = fresh("__crepusBindingNext");
    let settle = fresh("__crepusBindingSettle");
    let untrack = fresh("__crepusBindingUntrack");
    let capture = fresh("__CrepusBindingCapture");
    let defer = fresh("__crepusBindingDefer");
    let mut pair_number = 0;
    let mut wrapper_prefixes = Vec::new();
    let mut edits = Vec::new();
    let mut count = 0;
    let mut ordinary_count = 0;
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        let mut captures = String::new();
        for item in &element.opening_element.attributes {
            let JSXAttributeItem::Attribute(attribute) = item else {
                continue;
            };
            let JSXAttributeName::Identifier(name) = &attribute.name else {
                continue;
            };
            let Some((marker, _)) = bindings
                .iter()
                .find(|(marker, _)| name.name == marker.as_str())
            else {
                continue;
            };
            if !matches!(
                &element.opening_element.name,
                JSXElementName::IdentifierReference(_) | JSXElementName::MemberExpression(_)
            ) {
                return Err(fail("requires a component"));
            }
            let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
                return Err(fail("requires an assignable expression"));
            };
            let Some(expression) = container.expression.as_expression() else {
                return Err(fail("requires an assignable expression"));
            };
            // Core contributes exactly one transparent parenthesized layer.
            // Preserve authored extra parentheses as an invalid pair, matching Svelte.
            let inner = match expression {
                Expression::ParenthesizedExpression(parenthesized) => &parenthesized.expression,
                _ => expression,
            };
            let pair = match inner {
                Expression::SequenceExpression(pair) => {
                    if pair.expressions.len() != 2 {
                        return Err(fail("function pairs require exactly a getter and setter"));
                    }
                    Some(pair)
                }
                _ if matches!(
                    inner.get_inner_expression(),
                    Expression::SequenceExpression(_)
                ) =>
                {
                    return Err(fail(
                        "function pairs must not have authored surrounding parentheses",
                    ));
                }
                _ => None,
            };
            let (span, descriptor) = if let Some(pair) = pair {
                if semantic.nodes().iter().any(|node| {
                    matches!(
                        node.kind(),
                        AstKind::JSXElement(_) | AstKind::JSXFragment(_)
                    ) && node.kind().span().start >= pair.span.start
                        && node.kind().span().end <= pair.span.end
                }) {
                    return Err(fail("function binding expressions cannot contain JSX"));
                }
                // Capture outside the eventual dynamic-selection renderer.
                // Only if/key remounting creates a new occurrence and recaptures.
                let pair_get = fresh(&format!("__crepusBindingGet{pair_number}"));
                let pair_set = fresh(&format!("__crepusBindingSet{pair_number}"));
                pair_number += 1;
                let getter = pair.expressions[0].span();
                let setter = pair.expressions[1].span();
                for comment in &parsed.program.comments {
                    if comment.span.start >= expression.span().start
                        && comment.span.end <= expression.span().end
                        && !(comment.span.start >= getter.start && comment.span.end <= getter.end)
                        && !(comment.span.start >= setter.start && comment.span.end <= setter.end)
                    {
                        captures.push_str(
                            &source[comment.span.start as usize..comment.span.end as usize],
                        );
                        captures.push('\n');
                    }
                }
                captures.push_str(&format!(
                    "const {pair_get}:any=({}\n);const {pair_set}:any=({}\n);",
                    &source[getter.start as usize..getter.end as usize],
                    &source[setter.start as usize..setter.end as usize]
                ));
                (
                    expression.span(),
                    format!(
                        "{{enumerable:true,configurable:true,get:()=>{pair_get}(),set:({value}:any)=>{{{pair_set}({value});}}}}"
                    ),
                )
            } else {
                let Some(root) = crate::svelte_bindings::binding_root(expression) else {
                    return Err(fail(
                        "requires an identifier or member target without optional chains",
                    ));
                };
                if matches!(expression.get_inner_expression(), Expression::Identifier(_)) {
                    let reference = semantic
                        .scoping()
                        .get_reference(root.reference_id.get().unwrap());
                    if reference.symbol_id().is_some_and(|id| {
                        semantic.scoping().symbol_flags(id).is_const_variable()
                            || semantic.scoping().symbol_flags(id).is_import()
                    }) {
                        return Err(fail("cannot assign a constant or import"));
                    }
                }
                ordinary_count += 1;
                let span = expression.span();
                let read = &source[span.start as usize..span.end as usize];
                let notify = if matches!(target, MoonshineModuleTarget::Ssr) {
                    format!(";{invalidate}()")
                } else {
                    String::new()
                };
                (
                    span,
                    format!(
                        "{{enumerable:true,configurable:true,get:()=>({read}\n),set:({value}:any)=>{{({read}\n)={value}{notify};}}}}"
                    ),
                )
            };
            let trivia = parsed
                .program
                .comments
                .iter()
                .filter(|comment| {
                    comment.span.start >= attribute.span.start
                        && comment.span.end <= attribute.span.end
                        && !(comment.span.start >= span.start && comment.span.end <= span.end)
                })
                .map(|comment| {
                    format!(
                        "{}\n",
                        &source[comment.span.start as usize..comment.span.end as usize]
                    )
                })
                .collect::<String>();
            let span = attribute.span();
            edits.push((span.start as usize, span.end as usize, String::new()));
            // Svelte bindings override every spread, regardless of source order.
            let opening = &element.opening_element;
            let at = opening.span.end as usize
                - if element.closing_element.is_none() {
                    2
                } else {
                    1
                };
            edits.push((at, at, format!(" {marker}={{{trivia}{descriptor}}}")));
            count += 1;
        }
        if !captures.is_empty() {
            let (prefix, suffix) = match target {
                MoonshineModuleTarget::Dom => (
                    format!("<{capture} render={{()=>{{{captures}return (\n"),
                    "\n);}}/>".to_owned(),
                ),
                MoonshineModuleTarget::Ssr => {
                    let markup = matches!(
                        semantic.nodes().parent_kind(node.id()),
                        AstKind::JSXElement(_) | AstKind::JSXFragment(_) | AstKind::JSXAttribute(_)
                    );
                    (
                        format!(
                            "{}{defer}(()=> <{capture} render={{()=>{{{captures}return (\n",
                            if markup { "{" } else { "" }
                        ),
                        format!("\n);}}}}/>){}", if markup { "}" } else { "" }),
                    )
                }
            };
            wrapper_prefixes.push((
                element.span.start as usize,
                element.span.start as usize,
                prefix,
            ));
            // Suffixes precede the SSR settlement tail at identical offsets;
            // prefixes follow its head, leaving captures inside each attempt.
            edits.push((element.span.end as usize, element.span.end as usize, suffix));
        }
    }
    if count == 0 {
        return Err(fail("marker was not found"));
    }
    if ordinary_count > 0 && matches!(target, MoonshineModuleTarget::Ssr) {
        let function = semantic
            .nodes()
            .iter()
            .find_map(|node| match node.kind() {
                AstKind::Function(function)
                    if function.id.as_ref().is_some_and(|id| id.name == component) =>
                {
                    Some(function)
                }
                _ => None,
            })
            .ok_or_else(|| fail("component was not found"))?;
        let body = function
            .body
            .as_ref()
            .ok_or_else(|| fail("component body was not found"))?;
        let argument = body
            .statements
            .iter()
            .find_map(|statement| match statement {
                Statement::ReturnStatement(statement) => statement.argument.as_ref(),
                _ => None,
            })
            .ok_or_else(|| fail("component return was not found"))?;
        let at = body.span.start as usize + 1;
        edits.push((at, at, format!("let {invalidate}:()=>void=()=>{{}};")));
        let span = argument.span();
        edits.push((
            span.start as usize,
            span.start as usize,
            format!("{settle}(({next}:()=>void)=>{{{invalidate}={next};return ("),
        ));
        edits.push((span.end as usize, span.end as usize, ");})".into()));
    }
    edits.extend(wrapper_prefixes);
    edits.sort_by_key(|edit| (edit.0, edit.1));
    let mut out = source.to_owned();
    for (start, end, value) in edits.into_iter().rev() {
        out.replace_range(start..end, &value);
    }
    if ordinary_count > 0 && matches!(target, MoonshineModuleTarget::Ssr) {
        out = format!("import {{settle as {settle}}} from \"@tschk/moonshine-svelte/ssr\";\n{out}");
    }
    if pair_number > 0 {
        out = format!(
            "import {{untrack as {untrack}}} from \"@tschk/moonshine-dom/reactive\";\n{out}"
        );
        if matches!(target, MoonshineModuleTarget::Ssr) {
            out = format!("import {{defer as {defer}}} from \"@tschk/moonshine-dom/ssr\";\n{out}");
        }
        out.push_str(&format!("\nfunction {capture}(props:{{render:()=>any}}):any{{return {untrack}(props.render);}}\n"));
    }
    Ok((out, (pair_number > 0).then_some(capture)))
}

pub(crate) fn finish(source: &str, bindings: &[(String, String)]) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid compiled component binding syntax".into());
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut edits = Vec::new();
    for node in semantic.nodes().iter() {
        let AstKind::ObjectProperty(property) = node.kind() else {
            continue;
        };
        let PropertyKey::StaticIdentifier(name) = &property.key else {
            continue;
        };
        let Some((_, prop)) = bindings
            .iter()
            .find(|(marker, _)| !property.computed && name.name == marker.as_str())
        else {
            continue;
        };
        let Expression::ObjectExpression(descriptor) = property.value.get_inner_expression() else {
            return Err("invalid compiled component binding descriptor".into());
        };
        let getter = descriptor.properties.iter().find_map(|property| match property {
            ObjectPropertyKind::ObjectProperty(property) if matches!(&property.key, PropertyKey::StaticIdentifier(name) if !property.computed && name.name == "get") => Some(&property.value),
            _ => None,
        }).ok_or("compiled component binding getter was not found")?;
        let key = property.key.span();
        // Computed keys retain ordinary props such as __proto__ without changing a prototype.
        edits.push((
            key.start as usize,
            key.end as usize,
            format!(
                "[{}]",
                serde_json::to_string(prop).map_err(|error| error.to_string())?
            ),
        ));
        let span = getter.span();
        edits.push((
            descriptor.span.start as usize,
            span.start as usize,
            "(".into(),
        ));
        edits.push((
            span.end as usize,
            descriptor.span.end as usize,
            ")()".into(),
        ));
    }
    if edits.is_empty() {
        return Err("compiled component binding marker was not lowered".into());
    }
    edits.sort_by_key(|edit| edit.0);
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, &text);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lowered(
        source: &str,
        path: &Path,
        target: MoonshineModuleTarget,
        bindings: &[(String, String)],
        component: &str,
    ) -> Result<String, String> {
        prepare(source, path, target, bindings, component).map(|(source, _)| source)
    }
    #[test]
    fn component_bindings_validate_targets_and_settle_only_server_templates() {
        let source = "export default function App(){let value;return <Child mark={value}/>;}";
        let bindings = vec![("mark".into(), "value".into())];
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = lowered(source, Path::new("Test.svelte"), target, &bindings, "App").unwrap();
            assert!(out.contains("get:()=>(value"));
            assert!(out.contains("set:(__crepusComponentValue:any)"));
            assert_eq!(
                out.contains("settle as"),
                matches!(target, MoonshineModuleTarget::Ssr)
            );
            let allocator = Allocator::default();
            assert!(
                Parser::new(&allocator, &out, SourceType::tsx())
                    .parse()
                    .errors
                    .is_empty(),
                "{out}"
            );
            assert!(lowered(
                "function App(){const value=1;return <Child mark={value}/>}",
                Path::new("Bad.svelte"),
                target,
                &bindings,
                "App"
            )
            .is_err());
            assert!(lowered(
                "function App(){return <Child mark={value?.x}/>}",
                Path::new("Bad.svelte"),
                target,
                &bindings,
                "App"
            )
            .is_err());
        }
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let ordered = lowered(
                "function App(){let value;return <Child mark={value} {...props}/>}",
                Path::new("Order.svelte"),
                target,
                &bindings,
                "App",
            )
            .unwrap();
            assert!(ordered.find("...props").unwrap() < ordered.find("mark=").unwrap());
        }
        let out = finish("const props={mark:{enumerable:true,get:cached(()=>({enumerable:true,get:()=>value,set:v=>value=v}),false)}};", &bindings).unwrap();
        assert!(out.contains("[\"value\"]:(cached("));
        let allocator = Allocator::default();
        assert!(Parser::new(&allocator, &out, SourceType::tsx())
            .parse()
            .errors
            .is_empty());
    }

    #[test]
    fn prop_keys_are_computed_including_prototype_and_hyphen_names() {
        let bindings = vec![
            ("mark".into(), "__proto__".into()),
            ("other".into(), "aria-label".into()),
        ];
        let out = finish("const props={mark:{enumerable:true,get:()=>descriptor},other:{enumerable:true,get:()=>another}};", &bindings).unwrap();
        assert!(out.contains("[\"__proto__\"]:"));
        assert!(out.contains("[\"aria-label\"]:"));
        let allocator = Allocator::default();
        assert!(Parser::new(&allocator, &out, SourceType::tsx())
            .parse()
            .errors
            .is_empty());
    }
    fn pair_lower(source: &str, target: MoonshineModuleTarget) -> Result<String, String> {
        let bindings = vec![
            ("mark".into(), "open".into()),
            ("other".into(), "count".into()),
        ];
        let output = lowered(source, Path::new("Pair.svelte"), target, &bindings, "App")?;
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
        Ok(output)
    }
    #[test]
    fn pairs_capture_expressions_once_with_unbound_identifier_calls() {
        let source = "export default function App(){return <Child mark={(makeGet(),object.set)} {...props}/>;}";
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = pair_lower(source, target).unwrap();
            assert_eq!(output.matches("makeGet()").count(), 1);
            assert_eq!(output.matches("object.set").count(), 1);
            assert!(output.contains("get:()=>__crepusBindingGet0()"));
            assert!(output.contains("__crepusBindingSet0(__crepusComponentValue);"));
            assert!(output.find("...props").unwrap() < output.find("mark=").unwrap());
            assert!(!output.contains("settle as"));
            assert!(!output.contains("__crepusBindingInvalidate"));
        }
    }
    #[test]
    fn pair_callbacks_keep_arrows_types_comments_and_fresh_parameter_names() {
        let output=pair_lower("const __crepusBindingGet0=0;const __crepusBindingSet0=0;const __crepusComponentValue=0;export default function App(){let value;return <Child mark={(/*head*/ (()=>value) as ()=>boolean, /*mid*/ ((next:boolean)=>value=next) /*tail*/ // line\n)}/>}",MoonshineModuleTarget::Dom).unwrap();
        assert!(output.contains("const __crepusBindingGet0_:any="));
        assert!(output.contains("const __crepusBindingSet0_:any="));
        assert!(output.contains("set:(__crepusComponentValue_:any)"));
        for comment in ["/*head*/", "/*mid*/", "/*tail*/", "// line"] {
            assert!(output.contains(comment), "{output}");
        }
        assert!(output.contains("as ()=>boolean"));
    }
    #[test]
    fn pair_arity_authored_parentheses_and_ordinary_target_guards() {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            for source in [
                "export default function App(){return <Child mark={(get,set,extra)}/>}",
                "export default function App(){return <Child mark={((get,set))}/>}",
                "export default function App(){return <Child mark={((get,set) as any)}/>}",
                "export default function App(){return <Child mark={(object?.value)}/>}",
                "export default function App(){const value=1;return <Child mark={(value)}/>}",
            ] {
                assert!(pair_lower(source, target).is_err(), "{source}");
            }
            // Svelte accepts these values and lets invocation fail if reached.
            pair_lower(
                "export default function App(){return <Child mark={(null,1)}/>}",
                target,
            )
            .unwrap();
        }
    }
    #[test]
    fn server_pairs_do_not_invalidate_but_mixed_ordinary_bindings_settle_once() {
        let output=pair_lower("export default function App(){let value;return <><Child mark={(get,set)}/><Other other={(value)}/></>}",MoonshineModuleTarget::Ssr).unwrap();
        assert_eq!(output.matches("settle as").count(), 1);
        assert_eq!(output.matches("__crepusBindingSettle((").count(), 1);
        assert!(output.contains("__crepusBindingSet0(__crepusComponentValue);"));
        assert!(output.contains("defer as __crepusBindingDefer"));
        assert!(output.contains("return __crepusBindingUntrack(props.render)"));
        assert!(output.contains("{__crepusBindingDefer(()=> <__CrepusBindingCapture render={"));
        assert!(!output.contains("__crepusBindingDefer(()=>__crepusBindingUntrack"));
        assert!(!output
            .contains("__crepusBindingSet0(__crepusComponentValue);__crepusBindingInvalidate"));
        assert!(output.contains("=__crepusComponentValue;__crepusBindingInvalidate()"));
    }
    #[test]
    fn occurrence_wrappers_enclose_dynamic_selection_and_nested_children() {
        let output=pair_lower("export default function App(){return <><Current mark={(makeGet(),makeSet())}/><div><Other mark={(get,set)}/></div></>}",MoonshineModuleTarget::Dom).unwrap();
        assert_eq!(
            output.matches("<__CrepusBindingCapture render={").count(),
            2
        );
        assert!(output.contains("const __crepusBindingGet0:any=(makeGet()\n);"));
        assert!(output.contains("const __crepusBindingGet1:any=(get\n);"));
        let dynamic = crate::svelte_components::transform(
            &output,
            Path::new("Pair.svelte"),
            &["__CrepusBindingCapture".into()],
        )
        .unwrap();
        let allocator = Allocator::default();
        assert!(
            Parser::new(&allocator, &dynamic, SourceType::tsx())
                .parse()
                .errors
                .is_empty(),
            "{dynamic}"
        );
        assert!(
            dynamic.find("const __crepusBindingGet0").unwrap()
                < dynamic.find("<__crepusSvelteDynamic").unwrap()
        );
    }
    #[test]
    fn mixed_root_pair_capture_stays_inside_server_settlement() {
        let output=pair_lower("export default function App(){let value;return <Current mark={(get,set)} other={(value)}/>}",MoonshineModuleTarget::Ssr).unwrap();
        assert!(
            output.contains("return (__crepusBindingDefer(()=> <__CrepusBindingCapture render={"),
            "{output}"
        );
        assert!(
            output.find("__crepusBindingSettle((").unwrap()
                < output.find("const __crepusBindingGet0").unwrap()
        );
    }
    #[test]
    fn capture_helper_metadata_is_fresh_and_absent_for_ordinary_bindings() {
        let bindings = vec![("mark".into(), "open".into())];
        let(source,helper)=prepare("const __CrepusBindingCapture=0;export default function App(){return <Current mark={(get,set)}/>}",Path::new("Pair.svelte"),MoonshineModuleTarget::Dom,&bindings,"App").unwrap();
        assert_eq!(helper.as_deref(), Some("__CrepusBindingCapture_"));
        assert!(source.contains("<__CrepusBindingCapture_ render={"));
        assert!(source.contains("function __CrepusBindingCapture_(props:{render:()=>any}):any{return __crepusBindingUntrack(props.render);}"));
        let (_, helper) = prepare(
            "export default function App(){let open;return <Current mark={(open)}/>}",
            Path::new("Pair.svelte"),
            MoonshineModuleTarget::Dom,
            &bindings,
            "App",
        )
        .unwrap();
        assert_eq!(helper, None);
    }
    #[test]
    fn server_defer_constructs_capture_with_a_live_component_frame() {
        for source in [
            "export default function App(){return <Current mark={(get,set)}/>}",
            "export default function App(){return <><Current mark={(get,set)}/></>}",
            "export default function App(){return <div><Current mark={(get,set)}/></div>}",
        ] {
            let output = pair_lower(source, MoonshineModuleTarget::Ssr).unwrap();
            assert!(
                output.contains("__crepusBindingDefer(()=> <__CrepusBindingCapture render={"),
                "{output}"
            );
            assert!(output.contains("function __CrepusBindingCapture(props:{render:()=>any}):any{return __crepusBindingUntrack(props.render);}"),"{output}");
            assert!(!output.contains("return __crepusBindingDefer(()=>__crepusBindingUntrack"));
        }
    }
}
