//! Authored component bodies are callable children snippets, not JSX values.
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, SourceType};

pub(crate) fn transform(
    source: &str,
    helpers: &[&str],
    children_binding: Option<&str>,
) -> Result<String, String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    if !parsed.errors.is_empty() {
        return Err("invalid authored Svelte children syntax".into());
    }
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let mut edits = Vec::new();
    for node in semantic.nodes().iter() {
        let AstKind::JSXElement(element) = node.kind() else {
            continue;
        };
        let component = match &element.opening_element.name {
            JSXElementName::IdentifierReference(name) => !helpers.contains(&name.name.as_str()),
            JSXElementName::MemberExpression(_) => true,
            _ => false,
        };
        if !component || element.children.is_empty() {
            continue;
        }
        if element.opening_element.attributes.iter().any(|attribute| {
            matches!(attribute, JSXAttributeItem::Attribute(attribute) if matches!(&attribute.name, JSXAttributeName::Identifier(name) if name.name == "children" || Some(name.name.as_str()) == children_binding))
        }) {
            return Err("explicit children cannot be combined with an authored component body".into());
        }
        let Some(closing) = &element.closing_element else {
            return Err("unclosed authored Svelte component body".into());
        };
        let end = element.opening_element.span.end as usize;
        edits.push((end - 1, end, " children={()=><>"));
        let span = closing.span();
        edits.push((span.start as usize, span.end as usize, "</>}/>"));
    }
    edits.sort_by_key(|edit| edit.0);
    let mut out = source.to_owned();
    for (start, end, text) in edits.into_iter().rev() {
        out.replace_range(start..end, text);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_bodies_become_snippets_without_changing_intrinsics_or_helpers() {
        let source = "function View(){return <Key><Child {...props}><UI.Other><p>{value}</p></UI.Other></Child></Key>}";
        let out = transform(source, &["Key"], None).unwrap();
        assert!(out.contains("<Key><Child {...props} children={()=><>"));
        assert!(out.contains("<UI.Other children={()=><><p>{value}</p></>}/>"));
        assert!(out.ends_with("</>}/></Key>}"));
        let allocator = Allocator::default();
        assert!(Parser::new(&allocator, &out, SourceType::tsx())
            .parse()
            .errors
            .is_empty());
        assert!(transform(
            "const view=<Child children={snippet}><p/></Child>;",
            &[],
            None
        )
        .is_err());
        assert!(transform(
            "const view=<Child mark={snippet}><p/></Child>;",
            &[],
            Some("mark")
        )
        .is_err());
    }
}
