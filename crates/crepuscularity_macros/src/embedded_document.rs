//! `embedded_document!`: lower a `.crepus` file to `crepuscularity_embedded::compiled` statics.

use crepuscularity_core::ast::{Element, Node, TextPart};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

pub(crate) fn expand(source: &str, path: &std::path::Path) -> Result<TokenStream2, String> {
    let nodes = crepuscularity_core::parse_template_with_path(source, Some(path))
        .map_err(|e| e.to_string())?;
    let nodes = lower_nodes(&nodes)?;
    let path = path.to_string_lossy();
    Ok(quote! {{
        const _: &str = ::core::include_str!(#path);
        static TEMPLATE: ::crepuscularity_embedded::compiled::CompiledTemplate =
            ::crepuscularity_embedded::compiled::CompiledTemplate { nodes: #nodes };
        &TEMPLATE
    }})
}

fn lower_nodes(nodes: &[Node]) -> Result<TokenStream2, String> {
    let nodes = nodes
        .iter()
        .map(lower_node)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote! { &[#(#nodes),*] })
}

fn lower_node(node: &Node) -> Result<TokenStream2, String> {
    let c = quote! { ::crepuscularity_embedded::compiled };
    Ok(match node {
        Node::Element(el) => lower_element(el)?,
        Node::Text(parts) => {
            let parts = lower_text(parts)?;
            quote! { #c::CNode::Text(#parts) }
        }
        Node::If(block) => {
            let cond = lower_cond(&block.condition)?;
            let then = lower_nodes(&block.then_children)?;
            let otherwise = lower_nodes(block.else_children.as_deref().unwrap_or(&[]))?;
            quote! { #c::CNode::If { cond: #cond, then: #then, otherwise: #otherwise } }
        }
        Node::For(_) => return Err(unsupported("`for` loops")),
        Node::Match(_) => return Err(unsupported("`match` blocks")),
        Node::LetDecl(_) => return Err(unsupported("`$: let` declarations")),
        Node::Include(inc) => return Err(unsupported(&format!("include `{}`", inc.path))),
        Node::Embed(embed) => return Err(unsupported(&format!("embed `{}`", embed.src))),
        Node::RawText(_) | Node::RawHtml(_) => return Err(unsupported("raw expressions")),
    })
}

fn lower_element(el: &Element) -> Result<TokenStream2, String> {
    let c = quote! { ::crepuscularity_embedded::compiled };
    if el.tag == "slot" {
        return Err(unsupported("`slot`"));
    }
    if let Some(binding) = el.bindings.first() {
        return Err(unsupported(&format!(
            "binding `{}` on <{}>",
            binding.prop, el.tag
        )));
    }
    let tag = &el.tag;
    let id = match &el.id {
        Some(id) => quote! { ::core::option::Option::Some(#id) },
        None => quote! { ::core::option::Option::None },
    };
    let mut classes = Vec::new();
    for class in &el.classes {
        if class.contains('{') {
            return Err(unsupported(&format!("interpolated class `{class}`")));
        }
        classes.extend(class.split_whitespace().map(str::to_owned));
    }
    let mut conditional = Vec::new();
    for cc in &el.conditional_classes {
        if cc.class.contains('{') {
            return Err(unsupported(&format!("interpolated class `{}`", cc.class)));
        }
        let cond = lower_cond(&cc.condition)?;
        conditional.extend(
            cc.class
                .split_whitespace()
                .map(|class| quote! { (#cond, #class) }),
        );
    }
    // Same handler string the std renderer stores on buttons.
    let on_click = match el.event_handlers.iter().find(|e| e.event == "click") {
        Some(handler) if el.tag == "button" => {
            let handler = &handler.handler;
            quote! { ::core::option::Option::Some(#handler) }
        }
        _ => quote! { ::core::option::Option::None },
    };
    let children = lower_nodes(&el.children)?;
    Ok(quote! {
        #c::CNode::Element(#c::CElement {
            tag: #tag,
            id: #id,
            classes: &[#(#classes),*],
            conditional_classes: &[#(#conditional),*],
            on_click: #on_click,
            children: #children,
        })
    })
}

fn lower_text(parts: &[TextPart]) -> Result<TokenStream2, String> {
    let c = quote! { ::crepuscularity_embedded::compiled };
    let parts = parts
        .iter()
        .map(|part| match part {
            TextPart::Literal(s) => Ok(quote! { #c::CText::Lit(#s) }),
            TextPart::Expr(expr) => {
                let name = variable(expr)
                    .ok_or_else(|| unsupported(&format!("text expression `{{{expr}}}`")))?;
                Ok(quote! { #c::CText::Var(#name) })
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(quote! { &[#(#parts),*] })
}

fn lower_cond(condition: &str) -> Result<TokenStream2, String> {
    let c = quote! { ::crepuscularity_embedded::compiled };
    let trimmed = condition.trim();
    let trimmed = trimmed
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(trimmed)
        .trim();
    let (negated, name) = match trimmed.strip_prefix('!') {
        Some(rest) => (true, rest.trim()),
        None => (false, trimmed),
    };
    let name = variable(name).ok_or_else(|| unsupported(&format!("condition `{condition}`")))?;
    Ok(if negated {
        quote! { #c::Cond::Not(#name) }
    } else {
        quote! { #c::Cond::Var(#name) }
    })
}

/// A bare variable name as the core expression tokenizer reads it.
fn variable(expr: &str) -> Option<&str> {
    let name = expr.trim();
    let mut chars = name.chars();
    let first = chars.next()?;
    let valid = (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !matches!(name, "true" | "false" | "null" | "none" | "None" | "Null");
    valid.then_some(name)
}

fn unsupported(what: &str) -> String {
    format!(
        "embedded_document!: {what} is not supported in no_std compiled templates \
         (supported: elements, static/conditional classes, static ids, `{{name}}` text, \
         button @click, and if/else on `name` or `!name`)"
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn constructs_the_compiled_renderer_cannot_evaluate_are_errors() {
        let path = std::path::Path::new("ui.crepus");
        for source in [
            "for item in {items}\n  div",
            "match {mode}\n  \"a\" =>\n    div",
            "include child.crepus",
            "div\n  \"{cpu + 1}\"",
            "if cpu > 3\n  div",
            "div class:hidden={a && b}",
            "div bg-{theme}",
            "div id={name}",
        ] {
            let error = super::expand(source, path).unwrap_err();
            assert!(error.contains("not supported"), "{source}: {error}");
        }
        assert!(super::expand("div #a class:hidden={!ok}\n  \"{x}\"", path).is_ok());
    }
}
