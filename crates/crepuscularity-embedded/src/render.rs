//! AST + context → [`EmbeddedDocument`], then layout and framebuffer paint.

use std::path::Path;

use crepuscularity_core::ast::*;
use crepuscularity_core::context::{value_to_str, TemplateContext, TemplateValue};
use crepuscularity_core::eval::eval_expr;
use crepuscularity_core::CrepusError;

use crate::document::{EmbeddedDocument, EmbeddedNode, EmbeddedStyle};
use crate::framebuffer::Framebuffer;
use crate::layout::layout_document;
use crate::paint::paint_document;
use crate::screen::ScreenSize;
use crate::style::style_from_classes_with_context;

use crate::include_expand;

/// Inject embedded-target flags and screen dimensions into a child context.
pub fn with_embedded_target(ctx: &TemplateContext, screen: ScreenSize) -> TemplateContext {
    let mut c = ctx.clone();
    c.vars.insert(
        "crepus_target".into(),
        TemplateValue::Str("embedded".into()),
    );
    c.vars
        .insert("is_embedded".into(), TemplateValue::Bool(true));
    c.vars.insert("is_tui".into(), TemplateValue::Bool(false));
    c.vars.insert("is_web".into(), TemplateValue::Bool(false));
    c.vars.insert("is_gui".into(), TemplateValue::Bool(false));
    c.vars.insert(
        "screen_width".into(),
        TemplateValue::Int(i64::from(screen.width)),
    );
    c.vars.insert(
        "screen_height".into(),
        TemplateValue::Int(i64::from(screen.height)),
    );
    c
}

pub fn render_template_to_framebuffer(
    template: &str,
    ctx: &TemplateContext,
    screen: ScreenSize,
    fb: &mut impl Framebuffer,
) -> Result<EmbeddedDocument, CrepusError> {
    crepuscularity_core::render_entry::render_template(template, ctx, |nodes, ctx| {
        render_parsed_nodes_to_framebuffer(nodes, ctx, screen, fb)
    })
}

/// Lower parsed AST → layout → paint (skips parse; use with a cached [`Vec<Node>`]).
pub fn render_parsed_nodes_to_framebuffer(
    nodes: &[Node],
    ctx: &TemplateContext,
    screen: ScreenSize,
    fb: &mut impl Framebuffer,
) -> Result<EmbeddedDocument, CrepusError> {
    let ctx = with_embedded_target(ctx, screen);
    let mut doc = render_nodes_to_document(nodes, &ctx, screen)?;
    layout_document(&mut doc);
    paint_document(fb, &doc);
    Ok(doc)
}

pub fn render_component_file_to_framebuffer(
    content: &str,
    component_name: &str,
    ctx: &TemplateContext,
    screen: ScreenSize,
    fb: &mut impl Framebuffer,
) -> Result<EmbeddedDocument, CrepusError> {
    let ctx = with_embedded_target(ctx, screen);
    crepuscularity_core::render_entry::render_component_file(
        content,
        component_name,
        &ctx,
        |nodes, ctx| {
            let mut doc = render_nodes_to_document(nodes, ctx, screen)?;
            layout_document(&mut doc);
            paint_document(fb, &doc);
            Ok(doc)
        },
    )
}

pub fn render_file_to_framebuffer(
    path: impl AsRef<Path>,
    ctx: &TemplateContext,
    screen: ScreenSize,
    fb: &mut impl Framebuffer,
) -> Result<EmbeddedDocument, CrepusError> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .map_err(|e| CrepusError::render(format!("read {}: {}", path.display(), e)))?;
    let mut child_ctx = ctx.clone();
    child_ctx.base_dir = path.parent().map(|p| p.to_path_buf());
    render_template_to_framebuffer(&content, &child_ctx, screen, fb)
}

pub fn render_nodes_to_document(
    nodes: &[Node],
    ctx: &TemplateContext,
    screen: ScreenSize,
) -> Result<EmbeddedDocument, CrepusError> {
    Ok(EmbeddedDocument::new(
        render_nodes_list(nodes, ctx)?,
        screen,
    ))
}

fn render_nodes_list(
    nodes: &[Node],
    ctx: &TemplateContext,
) -> Result<Vec<EmbeddedNode>, CrepusError> {
    let mut ctx = ctx.clone();
    let mut out = Vec::new();
    for node in nodes {
        if let Node::LetDecl(decl) = node {
            if !(decl.is_default && ctx.vars.contains_key(&decl.name)) {
                let val = eval_expr(&decl.expr, &ctx)?;
                if let Some(v) = ctx.vars.get_mut(&decl.name) {
                    *v = val;
                } else {
                    ctx.vars.insert(decl.name.clone(), val);
                }
            }
            continue;
        }
        if let Node::Include(inc) = node {
            let (inner, inner_ctx) = include_expand::expand_include(inc, &ctx)?;
            out.extend(render_nodes_list(&inner, &inner_ctx)?);
            continue;
        }
        out.push(render_node(node, &ctx)?);
    }
    Ok(out)
}

fn render_node(node: &Node, ctx: &TemplateContext) -> Result<EmbeddedNode, CrepusError> {
    match node {
        Node::Element(el) => render_element(el, ctx),
        Node::Text(parts) => Ok(text_node(render_text_inline(parts, ctx)?)),
        Node::If(block) => {
            let body = if ctx.eval_condition(&block.condition)? {
                &block.then_children
            } else {
                block.else_children.as_deref().unwrap_or(&[])
            };
            Ok(container(
                "if",
                EmbeddedStyle::default(),
                render_nodes_list(body, ctx)?,
                None,
                None,
            ))
        }
        Node::For(block) => {
            let mut children = Vec::new();
            let items = ctx.get_list_ref(&block.iterator);
            let pattern = block.pattern.trim();
            let has_pattern = !pattern.is_empty();
            for item_ctx in items {
                let s = if has_pattern {
                    item_ctx.get_str("value")
                } else {
                    String::new()
                };
                let mut vars = item_ctx.vars.clone();
                if has_pattern && !s.is_empty() {
                    vars.insert(pattern.to_string(), TemplateValue::Str(s));
                }
                let loop_ctx = ctx.child_with_vars(vars);
                children.extend(render_nodes_list(&block.body, &loop_ctx)?);
            }
            Ok(container(
                "for",
                EmbeddedStyle::default(),
                children,
                None,
                None,
            ))
        }
        Node::Match(block) => render_match(block, ctx),
        Node::LetDecl(_) => Ok(container(
            "let",
            EmbeddedStyle::default(),
            vec![],
            None,
            None,
        )),
        Node::Include(_) => Err(CrepusError::render("include not expanded")),
        Node::Embed(_) => Ok(container(
            "embed",
            EmbeddedStyle::default(),
            vec![],
            None,
            None,
        )),
        Node::RawText(expr) => Ok(text_node(value_to_str(&eval_expr(expr, ctx)?))),
        Node::RawHtml(expr) => Ok(text_node(value_to_str(&eval_expr(expr, ctx)?))),
    }
}

fn render_match(block: &MatchBlock, ctx: &TemplateContext) -> Result<EmbeddedNode, CrepusError> {
    let value = value_to_str(&eval_expr(&block.expr, ctx)?);
    for arm in &block.arms {
        let pattern = arm.pattern.trim();
        if pattern == "_"
            || (pattern.starts_with('"')
                && pattern.ends_with('"')
                && value == pattern[1..pattern.len() - 1])
            || value == pattern
        {
            return Ok(container(
                "match",
                EmbeddedStyle::default(),
                render_nodes_list(&arm.body, ctx)?,
                None,
                None,
            ));
        }
    }
    Ok(container(
        "match",
        EmbeddedStyle::default(),
        vec![],
        None,
        None,
    ))
}

fn render_element(el: &Element, ctx: &TemplateContext) -> Result<EmbeddedNode, CrepusError> {
    if el.tag == "slot" {
        let children = if let Some((nodes, slot_ctx)) = &ctx.slot {
            render_nodes_list(nodes, slot_ctx)?
        } else {
            render_nodes_list(&el.children, ctx)?
        };
        return Ok(container(
            &el.tag,
            style(el, ctx)?,
            children,
            el.id.clone(),
            None,
        ));
    }

    if el.tag == "button" {
        let on_click = el
            .event_handlers
            .iter()
            .find(|e| e.event == "click")
            .map(|e| e.handler.clone());
        let label = collect_primary_text(&el.children, ctx)?;
        let mut n = text_node(label);
        n.id = el.id.clone();
        n.tag = "button".into();
        n.style = style(el, ctx)?;
        n.on_click = on_click;
        return Ok(n);
    }

    if el.tag == "span" && el.children.len() == 1 {
        if let Node::Text(parts) = &el.children[0] {
            let mut n = text_node(render_text_inline(parts, ctx)?);
            n.id = el.id.clone();
            n.tag = el.tag.clone();
            n.style = style(el, ctx)?;
            return Ok(n);
        }
    }

    Ok(container(
        &el.tag,
        style(el, ctx)?,
        render_nodes_list(&el.children, ctx)?,
        el.id.clone(),
        None,
    ))
}

fn collect_primary_text(children: &[Node], ctx: &TemplateContext) -> Result<String, CrepusError> {
    for child in children {
        match child {
            Node::Text(parts) => return render_text_inline(parts, ctx),
            Node::Element(el) if el.tag == "span" => {
                return collect_primary_text(&el.children, ctx);
            }
            _ => {}
        }
    }
    Ok(String::new())
}

fn style(el: &Element, ctx: &TemplateContext) -> Result<EmbeddedStyle, CrepusError> {
    let mut classes = Vec::new();
    for c in &el.classes {
        classes.push(ctx.interpolate(c)?);
    }
    for cc in &el.conditional_classes {
        if ctx.eval_condition(&cc.condition)? {
            classes.push(ctx.interpolate(&cc.class)?);
        }
    }
    for binding in &el.bindings {
        if binding.origin == BindingOrigin::Attribute && binding.matches_prop("class") {
            if let Some(value) = crepuscularity_core::eval::eval_binding(binding, ctx)? {
                classes.extend(value_to_str(&value).split_whitespace().map(str::to_string));
            }
        }
    }
    Ok(style_from_classes_with_context(&classes, Some(ctx)))
}

fn container(
    tag: &str,
    style: EmbeddedStyle,
    children: Vec<EmbeddedNode>,
    id: Option<String>,
    on_click: Option<String>,
) -> EmbeddedNode {
    EmbeddedNode {
        id,
        tag: tag.into(),
        text: None,
        on_click,
        style,
        bounds: Default::default(),
        children,
    }
}

fn text_node(text: String) -> EmbeddedNode {
    EmbeddedNode {
        id: None,
        tag: "text".into(),
        text: Some(text),
        on_click: None,
        style: EmbeddedStyle::default(),
        bounds: Default::default(),
        children: vec![],
    }
}

fn render_text_inline(parts: &[TextPart], ctx: &TemplateContext) -> Result<String, CrepusError> {
    let mut out = String::new();
    for part in parts {
        match part {
            TextPart::Literal(s) => out.push_str(s),
            TextPart::Expr(e) => {
                out.push_str(&value_to_str(&eval_expr(e, ctx)?));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod binding_origin_tests {
    use super::*;

    fn element(source: &str) -> Element {
        let nodes =
            crepuscularity_core::parse_template_with_path(source, Some(Path::new("view.ng")))
                .unwrap();
        let Node::Element(el) = nodes.into_iter().next().unwrap() else {
            panic!("element expected")
        };
        el
    }

    #[test]
    fn attribute_class_is_literal_and_null_is_absent() {
        let mut ctx = TemplateContext::new();
        ctx.set("classes", "bg-red-500 {missing}");
        let attr = element("<div [attr.CLASS]=\"classes\"></div>");
        let expected = style_from_classes_with_context(&["bg-red-500".into()], Some(&ctx));
        assert_eq!(style(&attr, &ctx).unwrap().bg, expected.bg);
        assert_eq!(
            style(&element("<div [attr.CLASS]=\"null\"></div>"), &ctx)
                .unwrap()
                .bg,
            None
        );
    }

    #[test]
    fn malformed_attribute_class_expression_propagates() {
        let mut el = element("<div></div>");
        el.bindings.push(Binding {
            origin: BindingOrigin::Attribute,
            prop: "class".into(),
            value: "true foo".into(),
        });
        assert!(crepuscularity_core::eval::eval_expr("true foo", &TemplateContext::new()).is_err());
        assert!(style(&el, &TemplateContext::new()).is_err());
    }
}
