//! AST + context → [`crate::ir::ViewIr`] lowering.

use std::collections::HashMap;

use crepuscularity_core::ast::*;
use crepuscularity_core::context::{value_to_str, TemplateContext};
use crepuscularity_core::eval::eval_expr;
use crepuscularity_core::preprocess::slot_rotate_child_phrases;
use crepuscularity_core::CrepusError;

use crate::include_expand;
use crate::ir::{PickerOption, StackAxis, TabItem, ViewIr, ViewNode, IR_VERSION};
use crate::style;

/// Render from a virtual file map (`entry` may use `#Component` suffix).
pub fn render_from_files(
    files: &HashMap<String, String>,
    entry: &str,
    ctx: &TemplateContext,
) -> Result<ViewIr, CrepusError> {
    let mut ctx = ctx.clone();
    ctx.virtual_files = std::sync::Arc::new(files.clone());

    if let Some((file_part, comp_name)) = entry.split_once('#') {
        let content = files.get(file_part).ok_or_else(|| {
            CrepusError::render(format!("file not found in virtual fs: {file_part}"))
        })?;
        return render_component_file_to_ir(content, comp_name, &ctx);
    }

    let content = files
        .get(entry)
        .ok_or_else(|| CrepusError::render(format!("file not found in virtual fs: {entry}")))?;
    render_template_to_ir(content, &ctx)
}

/// Parse and lower a template string to IR.
pub fn render_template_to_ir(template: &str, ctx: &TemplateContext) -> Result<ViewIr, CrepusError> {
    crepuscularity_core::render_entry::render_template(template, ctx, render_nodes_to_ir)
}

/// Parse and lower a template to IR, choosing the frontend from `path`.
///
/// The dispatcher keys on the file extension, so callers that hold a filename
/// reach every syntax the parser supports rather than only the default one.
pub fn render_template_to_ir_with_path(
    template: &str,
    ctx: &TemplateContext,
    path: Option<&std::path::Path>,
) -> Result<ViewIr, CrepusError> {
    crepuscularity_core::render_entry::render_template_with(
        template,
        ctx,
        |t| crepuscularity_core::parse_template_with_path(t, path),
        render_nodes_to_ir,
    )
}

/// Lower a named component from a multi-component `.crepus` file.
pub fn render_component_file_to_ir(
    content: &str,
    component_name: &str,
    ctx: &TemplateContext,
) -> Result<ViewIr, CrepusError> {
    crepuscularity_core::render_entry::render_component_file(
        content,
        component_name,
        ctx,
        render_nodes_to_ir,
    )
}

/// Lower already-parsed nodes into a [`ViewIr`].
pub fn render_nodes_to_ir(nodes: &[Node], ctx: &TemplateContext) -> Result<ViewIr, CrepusError> {
    let root = render_nodes_list(nodes, ctx)?;
    Ok(ViewIr {
        version: IR_VERSION,
        root,
    })
}

pub(crate) fn render_nodes_list(
    nodes: &[Node],
    ctx: &TemplateContext,
) -> Result<Vec<ViewNode>, CrepusError> {
    let mut ctx = ctx.clone();
    let mut out = Vec::new();
    for node in nodes {
        if let Node::LetDecl(decl) = node {
            if decl.is_default && ctx.vars.contains_key(&decl.name) {
                continue;
            }
            let val = eval_expr(&decl.expr, &ctx)?;
            ctx.vars.insert(decl.name.clone(), val);
            continue;
        }
        if let Node::Include(inc) = node {
            let (inner_nodes, inner_ctx) = include_expand::expand_include(inc, &ctx)?;
            out.extend(render_nodes_list(&inner_nodes, &inner_ctx)?);
            continue;
        }
        out.push(render_node(node, &ctx)?);
    }
    Ok(out)
}

fn render_node(node: &Node, ctx: &TemplateContext) -> Result<ViewNode, CrepusError> {
    match node {
        Node::Element(el) => render_element(el, ctx),
        Node::Text(parts) => Ok(ViewNode::Text {
            content: render_text_inline(parts, ctx)?,
            bind: text_bind(parts),
            style: None,
        }),
        Node::If(block) => render_if(block, ctx),
        Node::For(block) => render_for(block, ctx),
        Node::Match(block) => render_match(block, ctx),
        Node::LetDecl(_) => Ok(stack_column_raw(vec![])),
        Node::Include(_) => Err(CrepusError::render(
            "internal error: include should be expanded in render_nodes_list",
        )),
        Node::Embed(embed) if embed.adapter.as_deref() == Some("webview") => {
            Ok(ViewNode::WebView {
                src: embed.src.clone(),
                style: None,
            })
        }
        Node::Embed(_) => Ok(ViewNode::Text {
            content: String::new(),
            bind: None,
            style: None,
        }),
        Node::RawText(expr) => Ok(ViewNode::Text {
            content: value_to_str(&eval_expr(expr, ctx)?),
            bind: None,
            style: None,
        }),
        Node::RawHtml(expr) => Ok(ViewNode::Text {
            content: value_to_str(&eval_expr(expr, ctx)?),
            bind: None,
            style: None,
        }),
    }
}

fn stack_column_raw(children: Vec<ViewNode>) -> ViewNode {
    ViewNode::Stack {
        axis: StackAxis::Column,
        spacing: None,
        align_items: None,
        justify_content: None,
        on_long_press: None,
        style: None,
        children,
    }
}

fn render_element(el: &Element, ctx: &TemplateContext) -> Result<ViewNode, CrepusError> {
    let mut node = render_element_inner(el, ctx)?;
    if let Some(id) = &el.id {
        node.set_id(id.clone());
    }
    Ok(node)
}

fn render_element_inner(el: &Element, ctx: &TemplateContext) -> Result<ViewNode, CrepusError> {
    let tag = el.tag.to_ascii_lowercase();
    let tag = tag.as_str();

    if tag == "slot" {
        return if let Some((slot_nodes, slot_ctx)) = &ctx.slot {
            let children = render_nodes_list(slot_nodes, slot_ctx)?;
            Ok(stack_column_raw(children))
        } else {
            let children = render_nodes_list(&el.children, ctx)?;
            Ok(stack_column_raw(children))
        };
    }

    if tag == "slot-rotate" {
        let phrases = slot_rotate_child_phrases(&el.children).map_err(CrepusError::render)?;
        let mut interval_ms = 3200u64;
        for b in &el.bindings {
            if b.prop == "interval" {
                let v = value_to_str(&eval_expr(&b.value, ctx)?);
                let v = v.trim_matches('"').trim();
                interval_ms = v.parse().unwrap_or(3200);
            }
        }
        let classes = active_classes(el, ctx)?;
        let style = style::extract_stack_hints(classes, Some(ctx)).style;
        return Ok(ViewNode::SlotRotate {
            phrases,
            interval_ms,
            style: style.opt(),
        });
    }

    let classes = active_classes(el, ctx)?;

    if let Some(node) = render_form_element(tag, el, ctx, &classes)? {
        return Ok(node);
    }
    if let Some(node) = render_layout_element(tag, el, ctx, &classes)? {
        return Ok(node);
    }
    if let Some(node) = render_media_element(tag, el, ctx, &classes)? {
        return Ok(node);
    }
    if let Some(node) = render_list_element(tag, el, ctx, &classes)? {
        return Ok(node);
    }

    if is_text_tag(tag) && el.children.len() == 1 {
        if let Node::Text(parts) = &el.children[0] {
            let txt = render_text_inline(parts, ctx)?;
            let st = style::extract_text_style(classes.clone(), Some(ctx)).opt();
            return Ok(ViewNode::Text {
                content: txt,
                bind: text_bind(parts),
                style: st,
            });
        }
    }

    if el.children.is_empty() {
        if let Some(bind) = binding_raw(el, "bind") {
            let st = style::extract_text_style(classes.clone(), Some(ctx)).opt();
            return Ok(ViewNode::Text {
                content: String::new(),
                bind: Some(bind),
                style: st,
            });
        }
    }

    let axis = if tag == "view" {
        StackAxis::Column
    } else {
        stack_axis(&classes)
    };
    let spacing = parse_gap_spacing(&classes);
    let scroll = style::is_scroll_container(&classes);
    let hints = style::extract_stack_hints(classes, Some(ctx));
    let children = render_nodes_list(&el.children, ctx)?;

    if scroll {
        return Ok(ViewNode::Scroll {
            axis,
            style: hints.style.opt(),
            children: vec![ViewNode::Stack {
                axis,
                spacing,
                align_items: hints.align_items,
                justify_content: hints.justify_content,
                on_long_press: None,
                style: None,
                children,
            }],
        });
    }

    Ok(ViewNode::Stack {
        axis,
        spacing,
        align_items: hints.align_items,
        justify_content: hints.justify_content,
        on_long_press: None,
        style: hints.style.opt(),
        children,
    })
}

fn render_form_element(
    tag: &str,
    el: &Element,
    ctx: &TemplateContext,
    classes: &[String],
) -> Result<Option<ViewNode>, CrepusError> {
    if tag == "button" {
        let label = collect_primary_text(&el.children, ctx)?;
        let on_click = el
            .event_handlers
            .iter()
            .find(|e| e.event == "click")
            .map(|e| e.handler.clone());
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Button {
            label,
            on_click,
            on_long_press: None,
            style: hints.style.opt(),
        }));
    }

    if tag == "toggle" || tag == "switch" {
        let label = component_label(el, ctx)?;
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Toggle {
            label,
            bind: binding_raw(el, "bind"),
            checked: binding_bool(el, "checked", ctx).unwrap_or(false),
            on_change: event_handler(el, "change"),
            on_long_press: None,
            style: hints.style.opt(),
        }));
    }

    if tag == "checkbox" {
        let label = component_label(el, ctx)?;
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Checkbox {
            label,
            bind: binding_raw(el, "bind"),
            checked: binding_bool(el, "checked", ctx).unwrap_or(false),
            on_change: event_handler(el, "change"),
            style: hints.style.opt(),
        }));
    }

    if tag == "slider" {
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Slider {
            label: optional_component_label(el, ctx)?,
            bind: binding_raw(el, "bind"),
            value: binding_f32(el, "value", ctx).unwrap_or(0.0),
            min: binding_f32(el, "min", ctx).unwrap_or(0.0),
            max: binding_f32(el, "max", ctx).unwrap_or(100.0),
            step: binding_f32(el, "step", ctx),
            on_change: event_handler(el, "change"),
            style: hints.style.opt(),
        }));
    }

    if tag == "progress" {
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Progress {
            label: optional_component_label(el, ctx)?,
            value: binding_f32(el, "value", ctx).unwrap_or(0.0),
            max: binding_f32(el, "max", ctx).unwrap_or(100.0),
            style: hints.style.opt(),
        }));
    }

    if tag == "meter" {
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Meter {
            label: optional_component_label(el, ctx)?,
            value: binding_f32(el, "value", ctx).unwrap_or(0.0),
            min: binding_f32(el, "min", ctx).unwrap_or(0.0),
            max: binding_f32(el, "max", ctx).unwrap_or(100.0),
            style: hints.style.opt(),
        }));
    }

    if tag == "dropzone" {
        let label = collect_primary_text(&el.children, ctx).unwrap_or_default();
        let accept = el
            .bindings
            .iter()
            .find(|b| b.prop == "accept")
            .and_then(|b| eval_expr(&b.value, ctx).ok().map(|v| value_to_str(&v)));
        let on_drop = el
            .event_handlers
            .iter()
            .find(|e| e.event == "drop")
            .map(|e| e.handler.clone());
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        let children = render_nodes_list(&el.children, ctx)?;
        return Ok(Some(ViewNode::Dropzone {
            label,
            accept,
            on_drop,
            style: hints.style.opt(),
            children,
        }));
    }

    if tag == "file-picker" || tag == "filepicker" || tag == "media-picker" {
        let label =
            collect_primary_text(&el.children, ctx).unwrap_or_else(|_| "Choose file".to_string());
        let accept = el
            .bindings
            .iter()
            .find(|b| b.prop == "accept")
            .and_then(|b| eval_expr(&b.value, ctx).ok().map(|v| value_to_str(&v)))
            .map(|value| split_accept(&value))
            .unwrap_or_default();
        let multiple = el.bindings.iter().any(|b| b.prop == "multiple")
            || classes.iter().any(|c| c == "multiple");
        let on_pick = el
            .event_handlers
            .iter()
            .find(|e| e.event == "pick" || e.event == "click")
            .map(|e| e.handler.clone());
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::FilePicker {
            label,
            accept,
            multiple,
            on_pick,
            style: hints.style.opt(),
        }));
    }

    if tag == "input" || tag == "textfield" || tag == "textinput" || tag == "textarea" {
        let bind = binding_raw(el, "bind").unwrap_or_default();
        let placeholder = el
            .bindings
            .iter()
            .find(|b| b.prop == "placeholder")
            .map(|b| {
                let v = b.value.trim();
                v.trim_matches(|c| c == '"' || c == '\'').to_string()
            })
            .unwrap_or_default();
        let multiline = classes.iter().any(|c| c == "multiline") || tag == "textarea";
        let secure = binding_raw(el, "type")
            .map(|value| value.eq_ignore_ascii_case("password"))
            .unwrap_or(false)
            || classes.iter().any(|c| c == "secure");
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Input {
            placeholder,
            bind,
            multiline,
            secure,
            on_change: event_handler(el, "change"),
            style: hints.style.opt(),
        }));
    }

    if tag == "picker" || tag == "select" {
        let bind = binding_raw(el, "bind").unwrap_or_default();
        let mut options = Vec::new();
        for child in &el.children {
            if let Node::Element(inner) = child {
                let inner_tag = inner.tag.to_ascii_lowercase();
                if matches!(inner_tag.as_str(), "span" | "button" | "option") {
                    let label = collect_primary_text(&inner.children, ctx)?;
                    let value = inner
                        .bindings
                        .iter()
                        .find(|b| b.prop == "value")
                        .map(|b| {
                            let v = b.value.trim();
                            v.trim_matches(|c| c == '"' || c == '\'').to_string()
                        })
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| slug_option_value(&label));
                    options.push(PickerOption { value, label });
                }
            }
        }
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Picker {
            bind,
            options,
            on_change: event_handler(el, "change"),
            style: hints.style.opt(),
        }));
    }

    Ok(None)
}

fn render_layout_element(
    tag: &str,
    el: &Element,
    ctx: &TemplateContext,
    classes: &[String],
) -> Result<Option<ViewNode>, CrepusError> {
    if tag == "badge" {
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Badge {
            label: component_label(el, ctx)?,
            tone: binding_string(el, "tone", ctx),
            style: hints.style.opt(),
        }));
    }

    if tag == "divider" || tag == "hr" {
        let axis = stack_axis(classes);
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Divider {
            axis,
            style: hints.style.opt(),
        }));
    }

    if tag == "spacer" {
        let gap = parse_gap_spacing(classes);
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Spacer {
            size: binding_f32(el, "size", ctx).or(gap),
            style: hints.style.opt(),
        }));
    }

    if tag == "tabs" || tag == "tabview" || tag == "page-switcher" {
        let bind = binding_raw(el, "bind").unwrap_or_default();
        let mut tabs = Vec::new();
        for child in &el.children {
            if let Node::Element(inner) = child {
                let inner_tag = inner.tag.to_ascii_lowercase();
                if matches!(inner_tag.as_str(), "tab" | "page") {
                    let value = binding_raw(inner, "value").unwrap_or_else(|| {
                        binding_string(inner, "label", ctx)
                            .map(|label| slug_option_value(&label))
                            .unwrap_or_default()
                    });
                    let label = binding_string(inner, "label", ctx)
                        .unwrap_or_else(|| value.replace('-', " "));
                    let icon = binding_string(inner, "icon", ctx);
                    tabs.push(TabItem {
                        value,
                        label,
                        icon,
                        children: render_nodes_list(&inner.children, ctx)?,
                    });
                }
            }
        }
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Tabs {
            bind,
            tabs,
            on_change: event_handler(el, "change"),
            style: hints.style.opt(),
        }));
    }

    Ok(None)
}

fn render_media_element(
    tag: &str,
    el: &Element,
    ctx: &TemplateContext,
    classes: &[String],
) -> Result<Option<ViewNode>, CrepusError> {
    if tag == "iframe" {
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::WebView {
            src: binding_string(el, "src", ctx).unwrap_or_default(),
            style: hints.style.opt(),
        }));
    }

    if tag == "img" || tag == "image" {
        let src = el
            .bindings
            .iter()
            .find(|b| b.prop == "src")
            .and_then(|b| eval_expr(&b.value, ctx).ok().map(|v| value_to_str(&v)))
            .unwrap_or_default();
        let alt = el
            .bindings
            .iter()
            .find(|b| b.prop == "alt")
            .and_then(|b| eval_expr(&b.value, ctx).ok().map(|v| value_to_str(&v)));
        let placeholder = el
            .bindings
            .iter()
            .find(|b| b.prop == "placeholder")
            .and_then(|b| eval_expr(&b.value, ctx).ok().map(|v| value_to_str(&v)));
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Image {
            src,
            alt,
            placeholder,
            on_long_press: None,
            style: hints.style.opt(),
        }));
    }

    if tag == "a" || tag == "link" {
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Link {
            href: binding_string(el, "href", ctx).unwrap_or_default(),
            target: binding_string(el, "target", ctx),
            rel: binding_string(el, "rel", ctx),
            style: hints.style.opt(),
            children: render_nodes_list(&el.children, ctx)?,
        }));
    }

    Ok(None)
}

fn render_list_element(
    tag: &str,
    el: &Element,
    ctx: &TemplateContext,
    classes: &[String],
) -> Result<Option<ViewNode>, CrepusError> {
    if tag == "ul" || tag == "ol" || tag == "list" || tag == "flatlist" {
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::List {
            ordered: tag == "ol",
            style: hints.style.opt(),
            children: render_nodes_list(&el.children, ctx)?,
        }));
    }

    if tag == "li" || tag == "list-item" {
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::ListItem {
            on_long_press: None,
            style: hints.style.opt(),
            children: render_nodes_list(&el.children, ctx)?,
        }));
    }

    Ok(None)
}

fn slug_option_value(label: &str) -> String {
    let mut s = label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    s.trim_matches('_').to_string()
}

fn component_label(el: &Element, ctx: &TemplateContext) -> Result<String, CrepusError> {
    optional_component_label(el, ctx).map(|label| label.unwrap_or_default())
}

fn optional_component_label(
    el: &Element,
    ctx: &TemplateContext,
) -> Result<Option<String>, CrepusError> {
    if let Some(label) = binding_string(el, "label", ctx) {
        return Ok(Some(label));
    }
    match collect_primary_text(el.children.as_slice(), ctx) {
        Ok(label) if !label.is_empty() => Ok(Some(label)),
        Ok(_) => Ok(None),
        Err(_) => Ok(None),
    }
}

fn binding_raw(el: &Element, prop: &str) -> Option<String> {
    el.bindings
        .iter()
        .find(|binding| binding.prop == prop)
        .map(|binding| {
            binding
                .value
                .trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

fn binding_string(el: &Element, prop: &str, ctx: &TemplateContext) -> Option<String> {
    el.bindings
        .iter()
        .find(|binding| binding.prop == prop)
        .and_then(|binding| {
            eval_expr(&binding.value, ctx)
                .ok()
                .map(|v| value_to_str(&v))
        })
        .map(|value| {
            value
                .trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

fn binding_f32(el: &Element, prop: &str, ctx: &TemplateContext) -> Option<f32> {
    binding_string(el, prop, ctx).and_then(|value| value.parse().ok())
}

fn binding_bool(el: &Element, prop: &str, ctx: &TemplateContext) -> Option<bool> {
    binding_string(el, prop, ctx).and_then(|value| match value.as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    })
}

fn event_handler(el: &Element, event: &str) -> Option<String> {
    el.event_handlers
        .iter()
        .find(|handler| handler.event == event)
        .map(|handler| handler.handler.clone())
}

fn is_text_tag(tag: &str) -> bool {
    matches!(
        tag,
        "span" | "text" | "p" | "label" | "caption" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
    )
}

fn collect_primary_text(children: &[Node], ctx: &TemplateContext) -> Result<String, CrepusError> {
    for c in children {
        match c {
            Node::Text(parts) => return render_text_inline(parts, ctx),
            Node::Element(inner) => {
                let s = collect_primary_text(&inner.children, ctx)?;
                if !s.is_empty() {
                    return Ok(s);
                }
            }
            _ => {}
        }
    }
    Ok(String::new())
}

fn active_classes(el: &Element, ctx: &TemplateContext) -> Result<Vec<String>, CrepusError> {
    let mut expanded = Vec::new();
    for c in el.classes.iter() {
        expanded.push(ctx.interpolate(c)?);
    }
    for cc in &el.conditional_classes {
        if ctx.eval_condition(&cc.condition)? {
            expanded.push(ctx.interpolate(&cc.class)?);
        }
    }
    Ok(expanded)
}

fn stack_axis(classes: &[String]) -> StackAxis {
    let set: std::collections::HashSet<&str> = classes.iter().map(|s| s.as_str()).collect();
    if set.contains("flex-col") {
        StackAxis::Column
    } else if set.contains("flex-row") || set.contains("flex") {
        StackAxis::Row
    } else {
        StackAxis::Column
    }
}

fn parse_gap_spacing(classes: &[String]) -> Option<f32> {
    for c in classes {
        if let Some(rest) = c.strip_prefix("gap-") {
            if let Ok(n) = rest.parse::<u32>() {
                return Some((n as f32) * 4.0);
            }
        }
    }
    None
}

fn split_accept(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn render_text_inline(parts: &[TextPart], ctx: &TemplateContext) -> Result<String, CrepusError> {
    let mut result = String::new();
    for part in parts {
        match part {
            TextPart::Literal(text) => result.push_str(text),
            TextPart::Expr(expr) => {
                result.push_str(&value_to_str(&eval_expr(expr, ctx)?));
            }
        }
    }
    Ok(result)
}

fn text_bind(parts: &[TextPart]) -> Option<String> {
    match parts {
        [TextPart::Expr(expr)] => Some(expr.trim().to_string()),
        _ => None,
    }
}

fn render_if(block: &IfBlock, ctx: &TemplateContext) -> Result<ViewNode, CrepusError> {
    Ok(ViewNode::If {
        condition: block.condition.clone(),
        then_children: render_nodes_list(&block.then_children, ctx)?,
        else_children: match &block.else_children {
            Some(children) => Some(render_nodes_list(children, ctx)?),
            None => None,
        },
        style: None,
    })
}

fn render_for(block: &ForBlock, ctx: &TemplateContext) -> Result<ViewNode, CrepusError> {
    Ok(ViewNode::ForEach {
        bind: block.iterator.clone(),
        item_name: block.pattern.trim().to_string(),
        item_body: render_nodes_list(&block.body, ctx)?,
        style: None,
    })
}

fn render_match(block: &MatchBlock, ctx: &TemplateContext) -> Result<ViewNode, CrepusError> {
    let val = eval_expr(&block.expr, ctx)?;
    let value = value_to_str(&val);

    for arm in &block.arms {
        let pattern = arm.pattern.trim();
        if pattern == "_" {
            let children = render_nodes_list(&arm.body, ctx)?;
            return Ok(stack_column_raw(children));
        }
        if pattern.starts_with('"') && pattern.ends_with('"') {
            let lit = &pattern[1..pattern.len() - 1];
            if value == lit {
                let children = render_nodes_list(&arm.body, ctx)?;
                return Ok(stack_column_raw(children));
            }
        }
        if value == pattern {
            let children = render_nodes_list(&arm.body, ctx)?;
            return Ok(stack_column_raw(children));
        }
    }
    Ok(stack_column_raw(vec![]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_bind() {
        // Happy path: Single Expr part
        let parts = vec![TextPart::Expr("  some_expr  ".to_string())];
        assert_eq!(text_bind(&parts), Some("some_expr".to_string()));

        let parts = vec![TextPart::Expr("x".to_string())];
        assert_eq!(text_bind(&parts), Some("x".to_string()));

        // Edge case: Empty Expr
        let parts = vec![TextPart::Expr("".to_string())];
        assert_eq!(text_bind(&parts), Some("".to_string()));

        // Error condition: Empty parts
        let parts = vec![];
        assert_eq!(text_bind(&parts), None);

        // Error condition: Single Literal part
        let parts = vec![TextPart::Literal("hello".to_string())];
        assert_eq!(text_bind(&parts), None);

        // Error condition: Multiple parts (e.g. Literal and Expr)
        let parts = vec![
            TextPart::Literal("hello ".to_string()),
            TextPart::Expr("name".to_string()),
        ];
        assert_eq!(text_bind(&parts), None);

        // Error condition: Multiple Expr parts
        let parts = vec![
            TextPart::Expr("a".to_string()),
            TextPart::Expr("b".to_string()),
        ];
        assert_eq!(text_bind(&parts), None);
    }
}
