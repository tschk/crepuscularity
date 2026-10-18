//! Erase compiler-only const scopes into existing callback bodies after JSX lowering.
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType, Span};

fn marker_call<'s, 'a>(
    expression: &'s Expression<'a>,
    marker: &str,
    host: &str,
    fragment: &str,
) -> Option<&'s CallExpression<'a>> {
    let Expression::CallExpression(call) = expression.get_inner_expression() else {
        return None;
    };
    if call.callee.is_specific_id(marker) {
        return Some(call);
    }
    // Transparent compiler fragments introduce no new lexical scope. Only a
    // sole marker child can be collapsed; no unrelated sibling is discarded.
    let child = if call.callee.is_specific_id(host)
        && matches!(call.arguments.first(),Some(Argument::Identifier(id)) if id.name==fragment)
        && call.arguments.len() == 3
    {
        call.arguments.get(2)
    } else if call.callee.is_specific_id(fragment)
        && call.arguments.len() == 2
        && matches!(call.arguments.first(), Some(Argument::NullLiteral(_)))
    {
        call.arguments.get(1)
    } else {
        None
    }?;
    marker_call(child.as_expression()?, marker, host, fragment)
}
fn callback<'s, 'a>(
    call: &'s CallExpression<'a>,
) -> Result<&'s ArrowFunctionExpression<'a>, String> {
    let Some(Argument::ArrowFunctionExpression(arrow)) = call.arguments.first() else {
        return Err("invalid compiler-only const body".into());
    };
    if arrow.expression || arrow.r#async || !arrow.params.items.is_empty() {
        return Err("invalid emitted const scope callback shape".into());
    }
    Ok(arrow)
}
fn text(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

fn body_text(
    source: &str,
    call: &CallExpression<'_>,
    host: &str,
    fragment: &str,
    block_alias: Option<&str>,
) -> Result<String, String> {
    let arrow = callback(call)?;
    let mut body = text(source, arrow.body.span).to_string();
    let component = matches!(call.arguments.get(1),Some(Argument::StringLiteral(kind)) if kind.value=="component");
    if component {
        // Implicit component children did not previously have a fragment owner.
        // Return its native child array directly rather than adding h(Fragment).
        let Some(Statement::ReturnStatement(returned)) = arrow.body.statements.last() else {
            return Err("missing const component return".into());
        };
        let Some(Expression::CallExpression(children)) = returned
            .argument
            .as_ref()
            .map(Expression::get_inner_expression)
        else {
            return Err("invalid const component return".into());
        };
        if block_alias.is_some_and(|alias| children.callee.is_specific_id(alias)) {
            // The compiler's known block already returns the native child array.
            return Ok(body);
        }
        let start = if children.callee.is_specific_id(host)
            && matches!(children.arguments.first(),Some(Argument::Identifier(id)) if id.name==fragment)
        {
            2
        } else if children.callee.is_specific_id(fragment) {
            1
        } else {
            return Err("missing const component fragment".into());
        };
        let values = children
            .arguments
            .iter()
            .skip(start)
            .map(|arg| text(source, arg.span()))
            .collect::<Vec<_>>()
            .join(",");
        body.replace_range(
            (children.span.start - arrow.body.span.start) as usize
                ..(children.span.end - arrow.body.span.start) as usize,
            &format!("[{values}]"),
        );
    }
    Ok(body)
}

/// Intercept only after JSX choice classification, before generic JSX-to-h.
/// The call is compiler IR encoded in the existing Oxc tree, never published.
pub(crate) fn prepare(
    source: &str,
    marker: &str,
    source_type: SourceType,
) -> Result<String, String> {
    let mut output = source.to_string();
    loop {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, source_type).parse();
        if !parsed.errors.is_empty() {
            return Err("invalid const scope preparation syntax".into());
        }
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let selected=semantic.nodes().iter().filter_map(|node|match node.kind(){
            AstKind::JSXElement(element) if matches!(&element.opening_element.name,JSXElementName::IdentifierReference(id) if id.name==marker)=>Some((element,matches!(semantic.nodes().parent_kind(node.id()),AstKind::JSXElement(_)|AstKind::JSXFragment(_)))),_=>None
        }).min_by_key(|(element,_)|element.span.size());
        let Some((element, jsx_child)) = selected else {
            return Ok(output);
        };
        let mut arrow = None;
        let mut kind = None;
        for attr in &element.opening_element.attributes {
            let JSXAttributeItem::Attribute(attr) = attr else {
                return Err("invalid const scope spread".into());
            };
            let JSXAttributeName::Identifier(name) = &attr.name else {
                return Err("invalid const scope attribute".into());
            };
            match (name.name.as_str(), attr.value.as_ref()) {
                ("body", Some(JSXAttributeValue::ExpressionContainer(value))) => {
                    arrow = Some(value.expression.span())
                }
                ("kind", Some(JSXAttributeValue::StringLiteral(value))) => {
                    kind = Some(value.value.as_str())
                }
                ("conflicts", Some(JSXAttributeValue::ExpressionContainer(_))) => {}
                _ => return Err("invalid const scope metadata".into()),
            }
        }
        let replacement = format!(
            "{marker}({},{} )",
            text(&output, arrow.ok_or("missing const scope callback")?),
            serde_json::to_string(kind.ok_or("missing const scope kind")?)
                .map_err(|error| format!("cannot encode const scope kind: {error}"))?
        );
        let replacement = if jsx_child {
            format!("{{{replacement}}}")
        } else {
            replacement
        };
        output.replace_range(
            element.span.start as usize..element.span.end as usize,
            &replacement,
        );
    }
}

/// The body moves, rather than runs: no invocation, owner, or callback is added.
pub(crate) fn finish(
    source: &str,
    marker: &str,
    host: &str,
    fragment: &str,
    block_alias: Option<&str>,
    source_type: SourceType,
) -> Result<String, String> {
    let mut output = source.to_string();
    loop {
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, &output, source_type).parse();
        if !parsed.errors.is_empty() {
            return Err("invalid emitted const scope syntax".into());
        }
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let mut replacements = Vec::new();
        let mut markers = 0;
        for node in semantic.nodes().iter() {
            match node.kind() {
                AstKind::CallExpression(call) if call.callee.is_specific_id(marker) => {
                    markers += 1;
                }
                AstKind::ReturnStatement(statement) => {
                    let Some(expression) = statement.argument.as_ref() else {
                        continue;
                    };
                    let Some(call) = marker_call(expression, marker, host, fragment) else {
                        continue;
                    };
                    // Nested lexical block permits snippet-param shadowing and
                    // retains await's already-emitted payload prelude outside it.
                    replacements.push((
                        statement.span,
                        body_text(&output, call, host, fragment, block_alias)?,
                    ));
                }
                AstKind::ArrowFunctionExpression(arrow) if arrow.expression => {
                    let Some(Statement::ExpressionStatement(statement)) =
                        arrow.body.statements.first()
                    else {
                        continue;
                    };
                    let Some(call) = marker_call(&statement.expression, marker, host, fragment)
                    else {
                        continue;
                    };
                    replacements.push((
                        arrow.body.span,
                        body_text(&output, call, host, fragment, block_alias)?,
                    ));
                }
                _ => {}
            }
        }
        if markers == 0 {
            return Ok(output);
        }
        // Inner scopes first; an outer replacement would otherwise discard an
        // overlapping inner edit. Reparse only compiler-authored finite markers.
        let Some((span, replacement)) =
            replacements.into_iter().min_by_key(|(span, _)| span.size())
        else {
            return Err("const scope is not returned by an existing callback".into());
        };
        output.replace_range(span.start as usize..span.end as usize, &replacement);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prepare_intercepts_jsx_without_losing_child_expression_or_choice_shape() {
        let source="const render=()=>yes?<Scope kind=\"if\" body={()=>{const x=1;return <b>{x}</b>;}}/>:<i/>;const children=<Card><Scope kind=\"component\" body={()=>{const x=1;return <b>{x}</b>;}}/></Card>;";
        let output = prepare(source, "Scope", SourceType::tsx()).unwrap();
        assert!(!output.contains("<Scope"), "{output}");
        assert!(output.contains("<Card>{Scope("), "{output}");
        assert!(output.contains("yes?Scope("), "{output}");
    }
    #[test]
    fn returned_marker_moves_into_the_existing_arrow_without_invocation() {
        let source="const render=()=>h(F,null,Scope(()=>{const cell=derive(()=>read());return h(F,null,h('p',null,()=>cell.value));},'each'));";
        let output = finish(source, "Scope", "h", "F", None, SourceType::tsx()).unwrap();
        assert!(!output.contains("Scope("), "{output}");
        assert!(output.contains("const cell=derive"), "{output}");
        assert_eq!(
            output.matches("()=>").count(),
            source.matches("()=>").count() - 1,
            "{output}"
        );
        assert!(!output.contains("})()"));
    }
    #[test]
    fn await_payload_prelude_and_snippet_parameter_shadow_use_nested_block() {
        let source="const render=(value)=>{const payload=read();return Scope(()=>{const value=derive(()=>payload);return h(F,null,()=>value.value);},'snippet');};";
        let output = finish(source, "Scope", "h", "F", None, SourceType::tsx()).unwrap();
        assert!(output.contains("const payload=read();{"), "{output}");
        assert!(output.contains("const value=derive"));
        assert!(!output.contains("Scope("));
    }
    #[test]
    fn implicit_children_return_array_without_a_new_fragment_owner() {
        let source="const children=()=>Scope(()=>{const x=derive(read);return h(F,null,h('b',null,()=>x.value),h('i',null,()=>x.value));},'component');";
        let output = finish(source, "Scope", "h", "F", None, SourceType::tsx()).unwrap();
        assert!(output.contains("return [h('b'"), "{output}");
        assert!(!output.contains("h(F"));
        assert!(!output.contains("Scope("));
    }
    #[test]
    fn nested_markers_erase_inside_out_and_unsupported_placement_cannot_publish() {
        let source="const render=()=>Scope(()=>{const x=1;return h(F,null,()=>Scope(()=>{const x=2;return h('p',null,x);},'if'));},'if');";
        let output = finish(source, "Scope", "h", "F", None, SourceType::tsx()).unwrap();
        assert!(!output.contains("Scope("), "{output}");
        assert!(finish(
            "const wrong=Scope(()=>{return 1;},'if');",
            "Scope",
            "h",
            "F",
            None,
            SourceType::tsx()
        )
        .is_err());
    }
    #[test]
    fn component_scope_accepts_only_the_explicit_generated_array_block() {
        let source = "function C(){return Scope(()=>{const value=1;return Block(retain=>{return [value]});},'component')}";
        let output = finish(source, "Scope", "h", "F", Some("Block"), SourceType::tsx()).unwrap();
        assert!(output.contains("return Block"), "{output}");
        assert!(!output.contains("Scope("), "{output}");
        assert!(finish(source, "Scope", "h", "F", None, SourceType::tsx()).is_err());
        assert!(finish(source, "Scope", "h", "F", Some("Other"), SourceType::tsx()).is_err());
    }

    #[test]
    fn no_marker_is_byte_identical() {
        let source = "export const render=()=>h('p',null,'same'); // keep\n";
        assert_eq!(
            finish(source, "Scope", "h", "F", None, SourceType::tsx()).unwrap(),
            source
        );
        assert_eq!(prepare(source, "Scope", SourceType::tsx()).unwrap(), source);
    }
}
