//! Compiler-owned authored intrinsic metadata; never public element props.
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

pub(crate) fn rename_attribute(
    source: &str,
    previous: &str,
    replacement: &str,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid authored intrinsic metadata".into());
    }
    struct Names<'s> {
        previous: &'s str,
        spans: Vec<Span>,
    }
    impl<'a> Visit<'a> for Names<'_> {
        fn visit_jsx_attribute(&mut self, attribute: &JSXAttribute<'a>) {
            if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == self.previous)
            {
                self.spans.push(attribute.name.span());
            }
            walk::walk_jsx_attribute(self, attribute);
        }
    }
    let mut names = Names {
        previous,
        spans: Vec::new(),
    };
    names.visit_program(&parsed.program);
    let mut output = source.to_owned();
    for span in names.spans.into_iter().rev() {
        output.replace_range(span.start as usize..span.end as usize, replacement);
    }
    Ok(output)
}

/// Resolve physical template fragments before control/snippet relocation. The core
/// witness describes authored semantic SVG ancestry; a static HTML fragment does
/// not enter SVG merely because it contains a finite SVG tag such as `g`.
pub(crate) fn resolve_namespaces(
    source: &str,
    marker: &str,
    controls: &[&str],
) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid intrinsic namespace source".into());
    }
    fn witness<'b, 'a>(element: &'b JSXElement<'a>, marker: &str) -> Option<&'b StringLiteral<'a>> {
        element.opening_element.attributes.iter().find_map(|attribute| {
            let JSXAttributeItem::Attribute(attribute) = attribute else { return None };
            if !matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == marker) {
                return None;
            }
            match &attribute.value {
                Some(JSXAttributeValue::StringLiteral(value)) => Some(&**value),
                _ => None,
            }
        })
    }
    fn is_svg(value: &str) -> bool {
        value == "http://www.w3.org/2000/svg"
    }
    fn is_mathml(value: &str) -> bool {
        value == "http://www.w3.org/1998/Math/MathML"
    }
    struct Roots<'s> {
        marker: &'s str,
        controls: &'s [&'s str],
        namespace: Option<bool>,
        html: bool,
        bare_math: bool,
        depth: usize,
    }
    impl<'a> Visit<'a> for Roots<'_> {
        fn visit_jsx_element(&mut self, element: &JSXElement<'a>) {
            if let Some(value) = witness(element, self.marker) {
                let svg = is_svg(value.value.as_str());
                self.namespace = Some(self.namespace.unwrap_or(true) && svg);
                if self.depth == 0 {
                    if is_mathml(value.value.as_str()) {
                        self.bare_math |= !matches!(&element.opening_element.name, JSXElementName::Identifier(name) if name.name == "math");
                    } else if !svg {
                        self.html = true;
                    }
                }
                // A regular element is one static root, not another fragment.
                return;
            }
            if matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if self.controls.contains(&name.name.as_str()))
            {
                self.depth += 1;
                walk::walk_jsx_element(self, element);
                self.depth -= 1;
            }
            // Imported components are opaque. Their private intrinsic roots do
            // not participate in the containing component's parser template.
        }
    }
    struct Physical<'s> {
        marker: &'s str,
        controls: &'s [&'s str],
        namespace: Option<bool>,
        mixed: bool,
        html_math: bool,
        edits: Vec<(Span, &'static str)>,
    }
    impl<'a> Visit<'a> for Physical<'_> {
        fn visit_jsx_fragment(&mut self, fragment: &JSXFragment<'a>) {
            let previous = self.namespace;
            let previous_mixed = self.mixed;
            let mut roots = Roots {
                marker: self.marker,
                controls: self.controls,
                namespace: None,
                html: false,
                bare_math: false,
                depth: 0,
            };
            roots.visit_jsx_fragment(fragment);
            self.namespace = roots.namespace.or(previous);
            self.mixed = roots.html && roots.bare_math;
            walk::walk_jsx_fragment(self, fragment);
            self.namespace = previous;
            self.mixed = previous_mixed;
        }
        fn visit_jsx_element(&mut self, element: &JSXElement<'a>) {
            let previous = self.namespace;
            let previous_html_math = self.html_math;
            let root_mixed = self.mixed;
            if witness(element, self.marker).is_some_and(|v| is_mathml(v.value.as_str())) {
                let is_math = matches!(&element.opening_element.name, JSXElementName::Identifier(name) if name.name == "math");
                if is_math {
                    self.html_math = false;
                    self.namespace = None;
                } else if root_mixed || self.html_math {
                    let value = witness(element, self.marker).unwrap();
                    self.edits
                        .push((value.span, "\"http://www.w3.org/1999/xhtml\""));
                    self.html_math = true;
                    self.namespace = Some(false);
                } else {
                    self.namespace = None;
                }
            } else if let Some(value) = witness(element, self.marker) {
                let tag = match &element.opening_element.name {
                    JSXElementName::Identifier(name) => name.name.as_str(),
                    _ => return,
                };
                let svg = tag == "svg"
                    || self
                        .namespace
                        .unwrap_or_else(|| is_svg(value.value.as_str()));
                self.edits.push((
                    value.span,
                    if svg {
                        "\"http://www.w3.org/2000/svg\""
                    } else {
                        "\"http://www.w3.org/1999/xhtml\""
                    },
                ));
                self.namespace = Some(svg && tag != "foreignObject");
            } else if !matches!(&element.opening_element.name, JSXElementName::IdentifierReference(name) if self.controls.contains(&name.name.as_str()))
            {
                self.namespace = None;
            }
            self.mixed = false;
            walk::walk_jsx_element(self, element);
            self.namespace = previous;
            self.html_math = previous_html_math;
            self.mixed = root_mixed;
        }
    }
    let mut physical = Physical {
        marker,
        controls,
        namespace: None,
        mixed: false,
        html_math: false,
        edits: Vec::new(),
    };
    physical.visit_program(&parsed.program);
    let mut output = source.to_owned();
    physical.edits.sort_by_key(|(span, _)| span.start);
    for (span, value) in physical.edits.into_iter().rev() {
        output.replace_range(span.start as usize..span.end as usize, value);
    }
    Ok(output)
}

#[derive(Clone)]
pub(crate) struct ReferenceMarkers {
    pub raw: String,
    pub prepared: String,
}
pub(crate) type Edit = (usize, usize, String);

/// Original each symbols are supplied by its existing semantic pass, before
/// readonly checks/projections lose their authored lvalue shape.
pub(crate) fn reference_edits(
    source: &str,
    semantic: &oxc_semantic::Semantic<'_>,
    markers: &ReferenceMarkers,
    target: crate::MoonshineModuleTarget,
    contexts: &[(oxc_semantic::SymbolId, String)],
) -> Result<Vec<(Span, Vec<Edit>)>, String> {
    use oxc_ast::AstKind;
    use std::collections::HashSet;
    let mut reserved = semantic
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::BindingIdentifier(id) => Some(id.name.to_string()),
            AstKind::IdentifierReference(id) => Some(id.name.to_string()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut fresh = |base: &str| {
        let mut name = base.to_owned();
        while reserved.contains(&name) || source.contains(&name) {
            name.push('_');
        }
        reserved.insert(name.clone());
        name
    };
    let mut result = Vec::new();
    let mut asserted = HashSet::new();
    let mut declaration_edits = Vec::new();
    for node in semantic.nodes().iter() {
        let AstKind::JSXAttribute(attribute) = node.kind() else {
            continue;
        };
        if !matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == markers.raw)
        {
            continue;
        }
        let Some(JSXAttributeValue::ExpressionContainer(container)) = &attribute.value else {
            return Err("bind:this requires an explicit reference target".into());
        };
        let Some(expression) = container.expression.as_expression() else {
            return Err("bind:this requires a nonempty reference target".into());
        };
        let expression = expression.get_inner_expression();
        let pair = match expression {
            Expression::SequenceExpression(pair) if pair.expressions.len() == 2 => {
                Some((&pair.expressions[0], &pair.expressions[1]))
            }
            Expression::SequenceExpression(_) => {
                return Err(
                    "bind:this function binding requires exactly a getter and setter".into(),
                )
            }
            _ => None,
        };
        let mut scalar_symbol = None;
        if pair.is_none() {
            let root = crate::svelte_bindings::binding_root(expression)
                .ok_or("bind:this requires an identifier-rooted target or getter/setter pair")?;
            let symbol = semantic
                .scoping()
                .get_reference(
                    root.reference_id
                        .get()
                        .ok_or("unresolved bind:this target")?,
                )
                .symbol_id()
                .ok_or("bind:this target must be declared")?;
            if matches!(expression, Expression::Identifier(_)) {
                let flags = semantic.scoping().symbol_flags(symbol);
                if flags.is_const_variable() || flags.is_import() {
                    return Err("bind:this cannot assign to a constant binding".into());
                }
                scalar_symbol = Some(symbol);
            }
        }
        if matches!(target, crate::MoonshineModuleTarget::Ssr) {
            if let Some(symbol) = scalar_symbol {
                let declaration_node = semantic.symbol_declaration(symbol);
                if let AstKind::VariableDeclarator(declaration) = declaration_node.kind() {
                    if let BindingPattern::BindingIdentifier(id) = &declaration.id {
                        let ambient = matches!(semantic.nodes().parent_kind(declaration_node.id()), AstKind::VariableDeclaration(parent) if parent.declare);
                        if matches!(
                            declaration.kind,
                            VariableDeclarationKind::Let | VariableDeclarationKind::Var
                        ) && declaration.type_annotation.is_some()
                            && declaration.init.is_none()
                            && !declaration.definite
                            && !ambient
                            && semantic.scoping().symbol_declarations(symbol).count() == 1
                            && asserted.insert(symbol)
                        {
                            // SSR erases the binding-generated possible assignment.
                            // Keep its authored type and runtime undefined value.
                            declaration_edits.push((
                                id.span.end as usize,
                                id.span.end as usize,
                                "!".into(),
                            ));
                        }
                    }
                }
            }
            result.push((
                attribute.span,
                vec![(
                    attribute.span.start as usize,
                    attribute.span.end as usize,
                    String::new(),
                )],
            ));
            continue;
        }
        let mut captures = Vec::new();
        let capture_span = pair.map_or(expression.span(), |(getter, _)| getter.span());
        for reference_node in semantic.nodes().iter() {
            let AstKind::IdentifierReference(id) = reference_node.kind() else {
                continue;
            };
            if id.span.start < capture_span.start || id.span.end > capture_span.end {
                continue;
            }
            let Some(reference) = id.reference_id.get() else {
                continue;
            };
            let Some(symbol) = semantic.scoping().get_reference(reference).symbol_id() else {
                continue;
            };
            if let Some((_, read)) = contexts.iter().find(|(candidate, _)| *candidate == symbol) {
                if !captures.iter().any(|(captured, _, _)| *captured == symbol) {
                    captures.push((symbol, fresh("__crepusReferencePart"), read.clone()));
                }
            }
        }
        let node_type = fresh("__crepusReferenceNode");
        let parts_type = fresh("__crepusReferenceParts");
        let parts = captures
            .iter()
            .map(|(_, _, read)| read.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let parameters = captures
            .iter()
            .enumerate()
            .map(|(i, (_, name, _))| format!("{name}:{parts_type}[{i}]"))
            .collect::<Vec<_>>();
        let render_with = |span: Span, optional: bool, live: bool| -> String {
            let mut edits: Vec<Edit> = Vec::new();
            for reference_node in semantic.nodes().iter() {
                let AstKind::IdentifierReference(id) = reference_node.kind() else {
                    continue;
                };
                if id.span.start < span.start || id.span.end > span.end {
                    continue;
                }
                let Some(reference) = id.reference_id.get() else {
                    continue;
                };
                let Some(symbol) = semantic.scoping().get_reference(reference).symbol_id() else {
                    continue;
                };
                if let Some((_, name, _)) = captures
                    .iter()
                    .find(|(candidate, _, _)| *candidate == symbol)
                {
                    let replacement = if live {
                        contexts
                            .iter()
                            .find(|(candidate, _)| *candidate == symbol)
                            .unwrap()
                            .1
                            .clone()
                    } else {
                        name.clone()
                    };
                    let text = if matches!(semantic.nodes().parent_kind(reference_node.id()), AstKind::ObjectProperty(property) if property.shorthand)
                    {
                        crate::svelte_expression::shorthand_property(id.name.as_str(), &replacement)
                    } else {
                        replacement
                    };
                    edits.push((id.span.start as usize, id.span.end as usize, text));
                }
            }
            if optional {
                let mut member = expression;
                loop {
                    // Optionalize the complete authored receiver chain even
                    // when parentheses or erased TS wrappers separate members.
                    member = match member.get_inner_expression() {
                        Expression::StaticMemberExpression(value) => {
                            edits.push((
                                value.object.span().end as usize,
                                value.object.span().end as usize,
                                "?".into(),
                            ));
                            &value.object
                        }
                        Expression::ComputedMemberExpression(value) => {
                            edits.push((
                                value.object.span().end as usize,
                                value.object.span().end as usize,
                                "?.".into(),
                            ));
                            &value.object
                        }
                        _ => break,
                    };
                }
            }
            edits.sort_by_key(|edit| (edit.0, edit.1));
            let mut text = source[span.start as usize..span.end as usize].to_owned();
            for (start, end, value) in edits.into_iter().rev() {
                text.replace_range(
                    start - span.start as usize..end - span.start as usize,
                    &value,
                );
            }
            text
        };
        let render = |span, optional| render_with(span, optional, false);
        let value = fresh("__crepusReferenceValue");
        let (get, set, paired) = if let Some((get, set)) = pair {
            // Inline function bindings receive captured each values as real
            // parameters, matching Svelte's build_bind_this transformation.
            let function = |expression: &Expression<'_>, setter: bool| -> Result<String, String> {
                let params = match expression.get_inner_expression() {
                    Expression::ArrowFunctionExpression(arrow) => Some(&arrow.params),
                    Expression::FunctionExpression(function) => Some(&function.params),
                    _ => None,
                };
                let text = if let Some(params) = params {
                    let mut next = Vec::new();
                    if setter {
                        next.push(params.items.first().map_or_else(
                            || value.clone(),
                            |parameter| {
                                source[parameter.span.start as usize..parameter.span.end as usize]
                                    .to_owned()
                            },
                        ));
                    }
                    next.extend(parameters.iter().cloned());
                    // Re-render the function body separately so parameter edits
                    // cannot shift reference offsets inside the original span.
                    let before =
                        render(Span::new(expression.span().start, params.span.start), false);
                    let after = render(Span::new(params.span.end, expression.span().end), false);
                    format!("{before}({}){after}", next.join(","))
                } else {
                    render_with(expression.span(), false, true)
                };
                Ok(text)
            };
            (function(get, false)?, function(set, true)?, true)
        } else {
            let get = format!(
                "({})=>({})",
                captures
                    .iter()
                    .map(|(_, name, _)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                render(expression.span(), true)
            );
            let mut set_parameters = vec![value.clone()];
            set_parameters.extend(captures.iter().map(|(_, name, _)| name.clone()));
            let set = format!(
                "({})=>({}={value}!)",
                set_parameters.join(","),
                render(expression.span(), false)
            );
            (get, set, false)
        };
        let metadata = format!("{{nodeType:{node_type:?},partsType:{parts_type:?},pair:{paired},parts:()=>[{parts}] as const,get:{get},set:{set}}}");
        result.push((
            attribute.span,
            vec![
                (
                    attribute.name.span().start as usize,
                    attribute.name.span().end as usize,
                    markers.prepared.clone(),
                ),
                (
                    container.expression.span().start as usize,
                    container.expression.span().end as usize,
                    metadata,
                ),
            ],
        ));
    }
    for edit in declaration_edits {
        result.push((Span::new(edit.0 as u32, edit.1 as u32), vec![edit]));
    }
    Ok(result)
}

pub(crate) fn prepare_references(
    source: &str,
    markers: &ReferenceMarkers,
    target: crate::MoonshineModuleTarget,
    policy: crate::svelte_javascript::IrPolicy,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let mut parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid intrinsic bind:this syntax".into());
    }
    policy.configure(&mut parsed.program);
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&parsed.program)
        .semantic;
    let edits = reference_edits(source, &semantic, markers, target, &[])?;
    let mut output = source.to_owned();
    if matches!(target, crate::MoonshineModuleTarget::Dom) {
        // Preserve the established DOM edit order byte-for-byte.
        for (_, edits) in edits.into_iter().rev() {
            for (start, end, value) in edits.into_iter().rev() {
                output.replace_range(start..end, &value);
            }
        }
    } else {
        let mut edits = edits
            .into_iter()
            .flat_map(|(_, edits)| edits)
            .collect::<Vec<_>>();
        edits.sort_by_key(|(start, end, _)| (*start, *end));
        for (start, end, value) in edits.into_iter().rev() {
            output.replace_range(start..end, &value);
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    const HTML: &str = "http://www.w3.org/1999/xhtml";
    const SVG: &str = "http://www.w3.org/2000/svg";

    fn resolve(source: &str) -> String {
        let output =
            resolve_namespaces(source, "Witness", &["Each", "Await", "Key", "Const"]).unwrap();
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
        assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
        output
    }

    fn references(source: &str) -> Result<String, String> {
        prepare_references(
            source,
            &ReferenceMarkers {
                raw: "Raw".into(),
                prepared: "Prepared".into(),
            },
            crate::MoonshineModuleTarget::Dom,
            crate::svelte_javascript::IrPolicy::Typed,
        )
    }

    fn references_ssr(source: &str) -> Result<String, String> {
        prepare_references(
            source,
            &ReferenceMarkers {
                raw: "Raw".into(),
                prepared: "Prepared".into(),
            },
            crate::MoonshineModuleTarget::Ssr,
            crate::svelte_javascript::IrPolicy::Typed,
        )
    }

    #[test]
    fn ssr_bound_typed_scalar_declarations_keep_runtime_undefined_and_declared_type() {
        let source = r"function App(){let \u006eode:HTMLDivElement;let untouched:HTMLDivElement;var nullable:HTMLSpanElement|null;return <><div Raw={node}/><div Raw={node}/><span Raw={nullable}/></>}";
        let output = references_ssr(source).unwrap();
        assert!(
            output.contains(r"let \u006eode!:HTMLDivElement"),
            "{output}"
        );
        assert!(
            output.contains("var nullable!:HTMLSpanElement|null"),
            "{output}"
        );
        assert!(output.contains("let untouched:HTMLDivElement"), "{output}");
        assert_eq!(output.matches('!').count(), 2, "{output}");
        assert!(!output.contains("Raw="), "{output}");
        assert!(!output.contains("=undefined"), "{output}");
    }

    #[test]
    fn ssr_assertion_is_limited_to_validated_uninitialized_scalar_declarators() {
        for source in [
            "function App(){let node:HTMLDivElement|null=null;return <div Raw={node}/>}",
            "function App(){let node!:HTMLDivElement;return <div Raw={node}/>}",
            "function App(){let node;return <div Raw={node}/>}",
            "function App(node:HTMLDivElement){return <div Raw={node}/>}",
            "function App(){let box:{node:HTMLDivElement};return <div Raw={box.node}/>}",
            "function App(){let get:()=>HTMLDivElement,set:(node:HTMLDivElement)=>void;return <div Raw={get,set}/>}",
            "declare let node:HTMLDivElement;function App(){return <div Raw={node}/>}",
        ] {
            let output=references_ssr(source).unwrap();
            assert_eq!(output.matches('!').count(),source.matches('!').count(),"{output}");
        }
        for source in [
            "function App(){return <div Raw={unknown}/>}",
            "const node=null;function App(){return <div Raw={node}/>}",
            "import node from 'ref';function App(){return <div Raw={node}/>} ",
        ] {
            assert!(references_ssr(source).is_err(), "{source}");
        }
    }

    #[test]
    fn reference_getter_optionalizes_every_wrapped_member_receiver() {
        for target in [
            "(cells.a).b",
            "(cells.a as Box).b",
            "(cells.a!).b",
            "((cells['a'])!).b",
        ] {
            let source = format!("let cells:{{a:Box}};type Box={{b:HTMLDivElement|null}};function App(){{return <div Raw={{{target}}}/>}}");
            let output = references(&source).unwrap();
            assert_eq!(output.matches("?.").count(), 2, "{target}: {output}");
            assert!(
                output.contains(&format!("({target}=")),
                "setter must preserve authored target: {output}"
            );
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
        }
    }

    #[test]
    fn wrapped_receivers_do_not_admit_call_roots_or_constant_cells() {
        for (source, expected) in [
            (
                "function get(){return {a:null}};function App(){return <div Raw={(get()).a}/>} ",
                "identifier-rooted",
            ),
            (
                "const cell=null;function App(){return <div Raw={cell}/>} ",
                "constant binding",
            ),
        ] {
            let error = references(source).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn static_html_svg_tags_do_not_enter_svg_but_branch_fragments_do() {
        let output = resolve(&format!("function App(){{return <><div Witness=\"{HTML}\"><g Witness=\"{SVG}\"><a Witness=\"{SVG}\"/></g>{{shown ? <><g Witness=\"{SVG}\"><a Witness=\"{SVG}\"/></g></> : null}}</div></>}}"));
        assert_eq!(
            output.matches(&format!("Witness=\"{HTML}\"")).count(),
            3,
            "{output}"
        );
        assert_eq!(
            output.matches(&format!("Witness=\"{SVG}\"")).count(),
            2,
            "{output}"
        );
    }

    #[test]
    fn foreign_object_static_children_reset_and_nested_svg_reenters() {
        let output = resolve(&format!("function App(){{return <><svg Witness=\"{SVG}\"><foreignObject Witness=\"{SVG}\"><g Witness=\"{SVG}\"><a Witness=\"{SVG}\"/></g><svg Witness=\"{SVG}\"><title Witness=\"{SVG}\"/></svg></foreignObject></svg></>}}"));
        assert_eq!(
            output.matches(&format!("Witness=\"{HTML}\"")).count(),
            2,
            "{output}"
        );
        assert_eq!(
            output.matches(&format!("Witness=\"{SVG}\"")).count(),
            4,
            "{output}"
        );
    }

    #[test]
    fn original_snippet_witness_survives_extraction_and_components_are_opaque() {
        let output = resolve(&format!("function row(){{return <><a Witness=\"{SVG}\"/></>}} function App(){{return <><Child/><a Witness=\"{HTML}\"/></>}}"));
        assert_eq!(
            output.matches(&format!("Witness=\"{SVG}\"")).count(),
            1,
            "{output}"
        );
        assert_eq!(
            output.matches(&format!("Witness=\"{HTML}\"")).count(),
            1,
            "{output}"
        );
    }

    #[test]
    fn compiler_owned_controls_contribute_roots_without_component_traversal() {
        let output = resolve(&format!("function App(){{return <><Each children={{()=> <><g Witness=\"{SVG}\"/></>}}/><Child><div Witness=\"{HTML}\"/></Child></>}}"));
        assert_eq!(
            output.matches(&format!("Witness=\"{SVG}\"")).count(),
            1,
            "{output}"
        );
        assert_eq!(
            output.matches(&format!("Witness=\"{HTML}\"")).count(),
            1,
            "{output}"
        );
    }

    const MATH: &str = "http://www.w3.org/1998/Math/MathML";

    #[test]
    fn mathml_witnesses_are_preserved_and_children_use_own_witnesses() {
        let output = resolve(&format!("function App(){{return <><math Witness=\"{MATH}\"><mi Witness=\"{MATH}\"/></math></>}}"));
        assert_eq!(
            output.matches(&format!("Witness=\"{MATH}\"")).count(),
            2,
            "{output}"
        );
        assert!(!output.contains(HTML) && !output.contains(SVG), "{output}");
    }

    #[test]
    fn mtext_html_and_svg_children_keep_their_own_namespaces() {
        let output = resolve(&format!("function App(){{return <><math Witness=\"{MATH}\"><mtext Witness=\"{MATH}\"><span Witness=\"{HTML}\"><g Witness=\"{SVG}\"/></span><svg Witness=\"{SVG}\"><g Witness=\"{SVG}\"/></svg></mtext></math></>}}"));
        assert_eq!(
            output.matches(&format!("Witness=\"{MATH}\"")).count(),
            2,
            "{output}"
        );
        assert_eq!(
            output.matches(&format!("Witness=\"{SVG}\"")).count(),
            2,
            "{output}"
        );
        assert_eq!(
            output.matches(&format!("Witness=\"{HTML}\"")).count(),
            2,
            "{output}"
        );
    }

    fn count(output: &str, ns: &str) -> usize {
        output.matches(&format!("Witness=\"{ns}\"")).count()
    }

    #[test]
    fn bare_mathml_beside_html_root_is_xhtml_in_either_order_with_descendants() {
        for body in [
            format!(
                "<mi Witness=\"{MATH}\"><mn Witness=\"{MATH}\"/></mi><div Witness=\"{HTML}\"/>"
            ),
            format!(
                "<div Witness=\"{HTML}\"/><mi Witness=\"{MATH}\"><mn Witness=\"{MATH}\"/></mi>"
            ),
        ] {
            let output = resolve(&format!("function App(){{return <>{body}</>}}"));
            assert_eq!(count(&output, HTML), 3, "{output}");
            assert_eq!(count(&output, MATH), 0, "{output}");
        }
    }

    #[test]
    fn explicit_math_and_its_descendants_survive_html_siblings_and_nested_math_reenters() {
        let output = resolve(&format!("function App(){{return <><div Witness=\"{HTML}\"/><math Witness=\"{MATH}\"><mi Witness=\"{MATH}\"/></math><mrow Witness=\"{MATH}\"><math Witness=\"{MATH}\"><mi Witness=\"{MATH}\"/></math></mrow></>}}"));
        assert_eq!(count(&output, HTML), 2, "{output}");
        assert_eq!(count(&output, MATH), 4, "{output}");
    }

    #[test]
    fn control_branch_math_stays_separate_from_html_sibling() {
        let output = resolve(&format!("function App(){{return <><Each children={{()=> <><mi Witness=\"{MATH}\"/></>}}/><div Witness=\"{HTML}\"/></>}}"));
        assert_eq!(count(&output, MATH), 1, "{output}");
        assert_eq!(count(&output, HTML), 1, "{output}");
    }

    #[test]
    fn mathml_under_foreign_object_and_controls_is_preserved() {
        let output = resolve(&format!("function App(){{return <><svg Witness=\"{SVG}\"><foreignObject Witness=\"{SVG}\"><math Witness=\"{MATH}\"><mi Witness=\"{MATH}\"/></math></foreignObject></svg><Each children={{()=> <><math Witness=\"{MATH}\"><mi Witness=\"{MATH}\"/></math></>}}/></>}}"));
        assert_eq!(
            output.matches(&format!("Witness=\"{MATH}\"")).count(),
            4,
            "{output}"
        );
        assert_eq!(
            output.matches(&format!("Witness=\"{SVG}\"")).count(),
            2,
            "{output}"
        );
        assert!(!output.contains(HTML), "{output}");
    }

    #[test]
    fn svg_regression_beside_mathml_keeps_svg_context() {
        let output = resolve(&format!("function App(){{return <><svg Witness=\"{SVG}\"><g Witness=\"{SVG}\"/></svg><math Witness=\"{MATH}\"><mi Witness=\"{MATH}\"/></math><div Witness=\"{HTML}\"/></>}}"));
        assert_eq!(
            output.matches(&format!("Witness=\"{SVG}\"")).count(),
            2,
            "{output}"
        );
        assert_eq!(
            output.matches(&format!("Witness=\"{MATH}\"")).count(),
            2,
            "{output}"
        );
        assert_eq!(
            output.matches(&format!("Witness=\"{HTML}\"")).count(),
            1,
            "{output}"
        );
    }
}
