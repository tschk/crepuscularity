//! AST + context → [`EmbeddedDocument`], then layout and framebuffer paint.

use std::borrow::Cow;
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
    let nodes = crepuscularity_core::parse_template_with_path(&content, Some(path))?;
    render_parsed_nodes_to_framebuffer(&nodes, &child_ctx, screen, fb)
}

struct SlotFrame<'a> {
    nodes: &'a [Node],
    context: &'a TemplateContext,
    caller_slot: Option<&'a SlotFrame<'a>>,
}

pub fn render_nodes_to_document(
    nodes: &[Node],
    ctx: &TemplateContext,
    screen: ScreenSize,
) -> Result<EmbeddedDocument, CrepusError> {
    Ok(EmbeddedDocument::new(
        render_nodes_list(
            nodes,
            ctx,
            &mut include_expand::IncludeCache::default(),
            None,
        )?,
        screen,
    ))
}

fn render_nodes_list(
    nodes: &[Node],
    ctx: &TemplateContext,
    cache: &mut include_expand::IncludeCache,
    slot: Option<&SlotFrame<'_>>,
) -> Result<Vec<EmbeddedNode>, CrepusError> {
    let mut ctx = Cow::Borrowed(ctx);
    let mut out = Vec::new();
    for node in nodes {
        if let Node::LetDecl(decl) = node {
            if !(decl.is_default && ctx.vars.contains_key(&decl.name)) {
                let val = eval_expr(&decl.expr, &ctx)?;
                ctx.to_mut().vars.insert(decl.name.clone(), val);
            }
            continue;
        }
        if let Node::Include(inc) = node {
            let (inner, inner_ctx, _include_guard) =
                include_expand::expand_include(inc, &ctx, cache)?;
            // Includes render synchronously, so their slots can borrow the caller.
            let frame = SlotFrame {
                nodes: &inc.slot,
                context: &ctx,
                caller_slot: slot,
            };
            let child_slot = (!inc.slot.is_empty()).then_some(&frame);
            out.extend(render_nodes_list(&inner, &inner_ctx, cache, child_slot)?);
            continue;
        }
        let mut rendered = render_node(node, &ctx, cache, slot)?;
        if matches!(node, Node::If(_) | Node::For(_) | Node::Match(_))
            && rendered.children.iter().all(|child| child.style.hidden)
        {
            rendered.style.hidden = true;
        }
        out.push(rendered);
    }
    Ok(out)
}

fn render_node(
    node: &Node,
    ctx: &TemplateContext,
    cache: &mut include_expand::IncludeCache,
    slot: Option<&SlotFrame<'_>>,
) -> Result<EmbeddedNode, CrepusError> {
    match node {
        Node::Element(el) => render_element(el, ctx, cache, slot),
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
                render_nodes_list(body, ctx, cache, slot)?,
                None,
                None,
            ))
        }
        Node::For(block) => {
            let mut children = Vec::new();
            let items = match eval_expr(&block.iterator, ctx)? {
                TemplateValue::List(items) => items,
                _ => Vec::new(),
            };
            let pattern = block.pattern.trim();
            let has_pattern = !pattern.is_empty();
            let mut loop_ctx = ctx.clone();
            let mut overwritten = Vec::new();
            for item_ctx in items {
                for (key, value) in &item_ctx.vars {
                    overwritten.push((
                        key.clone(),
                        loop_ctx.vars.insert(key.clone(), value.clone()),
                    ));
                }
                if has_pattern {
                    let value = item_ctx
                        .get("value")
                        .cloned()
                        .unwrap_or_else(|| TemplateValue::Scope(item_ctx.clone()));
                    overwritten.push((
                        pattern.to_string(),
                        loop_ctx.vars.insert(pattern.to_string(), value),
                    ));
                }
                let rendered = render_nodes_list(&block.body, &loop_ctx, cache, slot);
                // Reverse restores duplicate writes when an item field is also the alias.
                for (key, value) in overwritten.drain(..).rev() {
                    match value {
                        Some(value) => {
                            loop_ctx.vars.insert(key, value);
                        }
                        None => {
                            loop_ctx.vars.remove(&key);
                        }
                    }
                }
                children.extend(rendered?);
            }
            Ok(container(
                "for",
                EmbeddedStyle::default(),
                children,
                None,
                None,
            ))
        }
        Node::Match(block) => render_match(block, ctx, cache, slot),
        Node::LetDecl(_) => Ok(container(
            "let",
            EmbeddedStyle::default(),
            vec![],
            None,
            None,
        )),
        Node::Include(_) => Err(CrepusError::render("include not expanded")),
        Node::Embed(embed) => Err(CrepusError::render(format!(
            "embed '{}' is not supported by the embedded target; guard web islands with is_web",
            embed.src
        ))),
        Node::RawText(expr) => Ok(text_node(value_to_str(&eval_expr(expr, ctx)?))),
        Node::RawHtml(expr) => Ok(text_node(value_to_str(&eval_expr(expr, ctx)?))),
    }
}

fn render_match(
    block: &MatchBlock,
    ctx: &TemplateContext,
    cache: &mut include_expand::IncludeCache,
    slot: Option<&SlotFrame<'_>>,
) -> Result<EmbeddedNode, CrepusError> {
    let value = value_to_str(&eval_expr(&block.expr, ctx)?);
    for arm in &block.arms {
        let pattern = arm.pattern.trim();
        let matches = if pattern == "_" {
            true
        } else if let Some(literal) = pattern.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            value == crepuscularity_core::parser::unescape_crepus_text_literal(literal)
        } else {
            value == pattern
        };
        if matches {
            return Ok(container(
                "match",
                EmbeddedStyle::default(),
                render_nodes_list(&arm.body, ctx, cache, slot)?,
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

fn render_element(
    el: &Element,
    ctx: &TemplateContext,
    cache: &mut include_expand::IncludeCache,
    slot: Option<&SlotFrame<'_>>,
) -> Result<EmbeddedNode, CrepusError> {
    let id = if let Some(id) = &el.id {
        Some(id.clone())
    } else {
        el.bindings
            .iter()
            .find(|binding| binding.prop == "id")
            .map(|binding| {
                crepuscularity_core::eval::eval_binding(binding, ctx)
                    .map(|value| value.map(|value| value_to_str(&value)))
            })
            .transpose()?
            .flatten()
    };
    if el.tag == "slot" {
        let children = if let Some(frame) = slot {
            render_nodes_list(frame.nodes, frame.context, cache, frame.caller_slot)?
        } else if let Some((nodes, slot_ctx)) = &ctx.slot {
            render_nodes_list(nodes, slot_ctx, cache, None)?
        } else {
            render_nodes_list(&el.children, ctx, cache, slot)?
        };
        return Ok(container(
            &el.tag,
            style(el, ctx)?,
            children,
            id.clone(),
            None,
        ));
    }

    if el.tag == "button" {
        for binding in &el.bindings {
            if binding.prop.eq_ignore_ascii_case("disabled") {
                let value = crepuscularity_core::eval::eval_binding(binding, ctx)?;
                if !matches!(
                    value,
                    None | Some(TemplateValue::Bool(false) | TemplateValue::Null)
                ) {
                    return Err(CrepusError::render(
                        "disabled buttons are not supported by the embedded target",
                    ));
                }
            }
        }
        let children = render_nodes_list(&el.children, ctx, cache, slot)?;
        let on_click = el
            .event_handlers
            .iter()
            .find(|e| e.event == "click")
            .map(|e| e.handler.clone());
        let label = collect_primary_text(&children)
            .unwrap_or_default()
            .to_owned();
        let mut n = text_node(label);
        n.id = id.clone();
        n.tag = "button".into();
        n.style = style(el, ctx)?;
        n.on_click = on_click;
        return Ok(n);
    }

    if el.tag == "span" && el.children.len() == 1 {
        if let Node::Text(parts) = &el.children[0] {
            let mut n = text_node(render_text_inline(parts, ctx)?);
            n.id = id.clone();
            n.tag = el.tag.clone();
            n.style = style(el, ctx)?;
            return Ok(n);
        }
    }

    Ok(container(
        &el.tag,
        style(el, ctx)?,
        render_nodes_list(&el.children, ctx, cache, slot)?,
        id.clone(),
        None,
    ))
}

fn collect_primary_text(children: &[EmbeddedNode]) -> Option<&str> {
    for child in children {
        if child.style.hidden || child.style.invisible || child.style.opacity_percent == Some(0) {
            continue;
        }
        if matches!(
            child.tag.as_str(),
            "text" | "span" | "if" | "for" | "match" | "slot"
        ) {
            if let Some(text) = child
                .text
                .as_deref()
                .or_else(|| collect_primary_text(&child.children))
            {
                return Some(text);
            }
        }
    }
    None
}

fn style(el: &Element, ctx: &TemplateContext) -> Result<EmbeddedStyle, CrepusError> {
    let mut classes = Vec::new();
    for c in &el.classes {
        classes.extend(ctx.interpolate(c)?.split_whitespace().map(str::to_string));
    }
    for cc in &el.conditional_classes {
        if ctx.eval_condition(&cc.condition)? {
            classes.extend(
                ctx.interpolate(&cc.class)?
                    .split_whitespace()
                    .map(str::to_string),
            );
        }
    }
    for binding in &el.bindings {
        if binding.origin == BindingOrigin::Attribute && binding.matches_prop("class") {
            if let Some(value) = crepuscularity_core::eval::eval_binding(binding, ctx)? {
                classes.extend(value_to_str(&value).split_whitespace().map(str::to_string));
            }
        }
    }
    let mut style = style_from_classes_with_context(&classes, Some(ctx));
    for binding in &el.bindings {
        if binding.prop.eq_ignore_ascii_case("hidden") {
            let value = crepuscularity_core::eval::eval_binding(binding, ctx)?;
            style.hidden |= !matches!(
                value,
                None | Some(TemplateValue::Bool(false) | TemplateValue::Null)
            );
        }
    }
    Ok(style)
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
