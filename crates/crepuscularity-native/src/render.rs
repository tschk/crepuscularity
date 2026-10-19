//! AST + context → [`crate::ir::ViewIr`] lowering.

use std::borrow::Cow;
use std::collections::HashMap;

use crepuscularity_core::ast::*;
use crepuscularity_core::context::{value_to_str, TemplateContext, TemplateValue};
use crepuscularity_core::eval::{eval_binding, eval_expr};
use crepuscularity_core::preprocess::slot_rotate_child_phrases;
use crepuscularity_core::CrepusError;

use crate::include_expand;
use crate::ir::{
    LabelPart, PickerOption, StackAxis, TabItem, ViewIr, ViewNode, ViewStyle, HTML_INPUT_TYPES,
    IR_VERSION,
};
use crate::style;

thread_local! {
    static MIXED_TEXT_SNAPSHOT_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct MixedTextSnapshot;

impl MixedTextSnapshot {
    fn enter() -> Self {
        MIXED_TEXT_SNAPSHOT_DEPTH.with(|depth| depth.set(depth.get() + 1));
        Self
    }
}

impl Drop for MixedTextSnapshot {
    fn drop(&mut self) {
        MIXED_TEXT_SNAPSHOT_DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

/// Render from a virtual file map (`entry` may use `#Component` suffix).
pub fn render_from_files(
    files: &HashMap<String, String>,
    entry: &str,
    ctx: &TemplateContext,
) -> Result<ViewIr, CrepusError> {
    let mut ctx = ctx.clone();
    ctx.virtual_files = std::sync::Arc::new(files.clone());
    let entry_path = entry.split_once('#').map_or(entry, |(path, _)| path);
    ctx.base_dir = std::path::Path::new(entry_path)
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(std::path::Path::to_path_buf);

    if let Some((file_part, comp_name)) = entry.split_once('#') {
        let content = files.get(file_part).ok_or_else(|| {
            CrepusError::render(format!("file not found in virtual fs: {file_part}"))
        })?;
        return render_component_file_to_ir(content, comp_name, &ctx);
    }

    let content = files
        .get(entry)
        .ok_or_else(|| CrepusError::render(format!("file not found in virtual fs: {entry}")))?;
    render_template_to_ir_with_path(content, &ctx, Some(std::path::Path::new(entry)))
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
        |nodes, child_ctx| {
            let _snapshot = MixedTextSnapshot::enter();
            render_nodes_to_ir(nodes, child_ctx)
        },
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
    // Reserve enough for the large IR-producing frames before the next recursive check.
    // Grow on the same thread so the include depth guard remains active.
    stacker::maybe_grow(512 * 1024, 2 * 1024 * 1024, || {
        render_nodes_list_inner(nodes, ctx)
    })
}

fn render_nodes_list_inner(
    nodes: &[Node],
    ctx: &TemplateContext,
) -> Result<Vec<ViewNode>, CrepusError> {
    let mut ctx = Cow::Borrowed(ctx);
    let mut out = Vec::new();
    let mut declaration_snapshot = None;
    for node in nodes {
        if let Node::LetDecl(decl) = node {
            if decl.is_default && ctx.vars.contains_key(&decl.name) {
                continue;
            }
            let val = eval_expr(&decl.expr, &ctx)?;
            ctx.to_mut().vars.insert(decl.name.clone(), val);
            declaration_snapshot.get_or_insert_with(MixedTextSnapshot::enter);
            continue;
        }
        if let Node::Include(inc) = node {
            let _snapshot = MixedTextSnapshot::enter();
            let (inner_nodes, inner_ctx, _include_guard) =
                include_expand::expand_include(inc, &ctx)?;
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
        Node::Embed(_) => Err(CrepusError::render(
            "View IR does not support embedded islands; use the web site build or adapter=\"webview\"",
        )),
        Node::RawText(expr) => Ok(ViewNode::Text {
            content: value_to_str(&eval_expr(expr, ctx)?),
            bind: None,
            style: None,
        }),
        Node::RawHtml(_) => Err(CrepusError::render("View IR does not support raw HTML")),
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
    reject_unsupported_control_state(el, ctx)?;
    let picker = matches!(
        el.tag.to_ascii_lowercase().as_str(),
        "file-picker" | "filepicker" | "media-picker"
    );
    let mut events = std::collections::HashSet::new();
    for handler in &el.event_handlers {
        if !handler.modifiers.is_empty() {
            return Err(CrepusError::render(format!(
                "View IR does not support event modifiers on `{}` handlers for `<{}>`",
                handler.event, el.tag
            )));
        }
        let event = if picker && handler.event == "click" {
            "pick"
        } else if handler.event == "longPress" {
            "longpress"
        } else {
            handler.event.as_str()
        };
        if !events.insert(event) {
            return Err(CrepusError::render(format!(
                "View IR supports only one `{}` handler per element; multiple handlers on `<{}>`",
                event, el.tag
            )));
        }
    }
    let mut node = render_element_inner(el, ctx)?;
    if let Some(handler) = event_handler(el, "longpress") {
        match &mut node {
            ViewNode::Stack { on_long_press, .. }
            | ViewNode::Scroll { on_long_press, .. }
            | ViewNode::Button { on_long_press, .. }
            | ViewNode::Toggle { on_long_press, .. }
            | ViewNode::Image { on_long_press, .. }
            | ViewNode::ListItem { on_long_press, .. } => *on_long_press = Some(handler),
            _ => {
                return Err(CrepusError::render(format!(
                    "View IR does not support long-press handlers on `<{}>`",
                    el.tag
                )))
            }
        }
    }
    for handler in &el.event_handlers {
        let supported = match handler.event.as_str() {
            "longpress" | "longPress" => true,
            "click" => {
                matches!(node, ViewNode::Button { .. })
                    || (picker && matches!(node, ViewNode::FilePicker { .. }))
            }
            "pick" => picker && matches!(node, ViewNode::FilePicker { .. }),
            "drop" => matches!(node, ViewNode::Dropzone { .. }),
            "input" => matches!(node, ViewNode::Input { .. }),
            "change" => {
                matches!(
                    node,
                    ViewNode::Toggle { .. }
                        | ViewNode::Checkbox { .. }
                        | ViewNode::Slider { .. }
                        | ViewNode::Input { .. }
                        | ViewNode::Picker { .. }
                        | ViewNode::Tabs { .. }
                ) || (!picker && matches!(node, ViewNode::FilePicker { .. }))
            }
            _ => false,
        };
        if !supported {
            return Err(CrepusError::render(format!(
                "View IR does not support `{}` handlers on `<{}>`",
                handler.event, el.tag
            )));
        }
    }
    match &node {
        ViewNode::Stack { .. }
        | ViewNode::Dropzone { .. }
        | ViewNode::Link { .. }
        | ViewNode::Scroll { .. }
        | ViewNode::List { .. }
        | ViewNode::ListItem { .. } => {}
        ViewNode::Tabs { .. } => {
            for child in &el.children {
                if let Node::Element(inner) = child {
                    if matches!(inner.tag.to_ascii_lowercase().as_str(), "tab" | "page") {
                        reject_metadata_events(inner, &el.tag, ctx)?;
                        continue;
                    }
                }
                reject_unsupported_flattened_nodes(std::slice::from_ref(child), &el.tag, ctx)?;
                if !matches!(child, Node::Text(parts) if parts.iter().all(|part| {
                    matches!(part, TextPart::Literal(text) if text.trim().is_empty())
                })) {
                    return Err(CrepusError::render(format!(
                        "unsupported View IR tabs child in `<{}>`; use direct tab or page elements",
                        el.tag
                    )));
                }
            }
        }
        _ => reject_unsupported_flattened_nodes(&el.children, &el.tag, ctx)?,
    }
    if let Some(id) = &el.id {
        node.set_id(id.clone());
    } else if let Some(binding) = el
        .bindings
        .iter()
        .find(|binding| binding.matches_prop("id"))
    {
        if let Some(value) = eval_binding(binding, ctx)? {
            node.set_id(value_to_str(&value));
        }
    }
    if el
        .bindings
        .iter()
        .any(|binding| binding.matches_prop("hidden") && binding.origin == BindingOrigin::Attribute)
        && single_binding_value(el, "hidden", ctx)?.is_some()
    {
        node.style_mut().get_or_insert_with(Default::default).hidden = Some(true);
    }
    let tag = el.tag.to_ascii_lowercase();
    let mut names = el
        .bindings
        .iter()
        .filter(|binding| binding.matches_prop("aria-label"));
    if let Some(name) = names.next() {
        if names.next().is_some() {
            return Err(CrepusError::render(
                "an element may have only one aria-label attribute",
            ));
        }
        node.style_mut()
            .get_or_insert_with(Default::default)
            .aria_label = eval_binding(name, ctx)?.map(|value| value_to_str(&value));
    }
    if tag == "label" {
        let mut targets = el
            .bindings
            .iter()
            .filter(|binding| binding.matches_prop("for") || binding.matches_prop("htmlFor"));
        if let Some(target) = targets.next() {
            if targets.next().is_some() {
                return Err(CrepusError::render(
                    "a label may have only one for/htmlFor attribute",
                ));
            }
            node.style_mut()
                .get_or_insert_with(Default::default)
                .html_for = eval_binding(target, ctx)?.map(|value| value_to_str(&value));
        }
    }
    if crate::ir::HTML_TEXT_TAGS.contains(&tag.as_str()) && tag != "span" {
        node.style_mut()
            .get_or_insert_with(Default::default)
            .html_tag = Some(tag);
    }
    Ok(node)
}

fn render_element_inner(el: &Element, ctx: &TemplateContext) -> Result<ViewNode, CrepusError> {
    let tag = el.tag.to_ascii_lowercase();
    let tag = tag.as_str();

    if tag == "slot-rotate" {
        let phrases = slot_rotate_child_phrases(&el.children).map_err(CrepusError::render)?;
        let mut intervals = el.bindings.iter().filter(|binding| {
            binding.prop == "interval" && binding.origin != BindingOrigin::Attribute
        });
        let interval = intervals.next();
        if intervals.next().is_some() {
            return Err(CrepusError::render("duplicate interval binding"));
        }
        let interval_ms = interval
            .map(|binding| eval_binding(binding, ctx))
            .transpose()?
            .flatten()
            .map(|value| value_to_str(&value))
            .map(|value| {
                value
                    .trim_matches('"')
                    .trim()
                    .parse::<u64>()
                    .unwrap_or(3200)
            })
            .unwrap_or(3200);
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

    if tag != "slot" && el.children.is_empty() {
        if let Some(bind) = binding_raw(el, "bind")? {
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
    let scroll_axis = style::scroll_axis(&classes, axis);
    let hints = style::extract_stack_hints(classes, Some(ctx));
    let children = if tag == "slot" {
        if let Some((nodes, caller_ctx)) = &ctx.slot {
            render_nodes_list(nodes, caller_ctx)?
        } else {
            render_nodes_list(&el.children, ctx)?
        }
    } else {
        render_nodes_list(&el.children, ctx)?
    };

    if let Some(scroll_axis) = scroll_axis {
        return Ok(ViewNode::Scroll {
            axis: scroll_axis,
            on_long_press: None,
            style: hints.style.opt(),
            children: vec![ViewNode::Stack {
                axis,
                spacing,
                align_items: hints.align_items,
                justify_content: hints.justify_content,
                on_long_press: None,
                style: Some(ViewStyle {
                    html_display_contents: Some(true),
                    ..Default::default()
                }),
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
    let input_type = if ["input", "textfield", "textinput", "textarea"].contains(&tag) {
        single_binding_value(el, "type", ctx)?.map(|value| {
            let kind = value_to_str(&value).trim().to_ascii_lowercase();
            if kind.is_empty() {
                "text".into()
            } else {
                kind
            }
        })
    } else {
        None
    };
    if tag == "button" {
        if let Some(value) = single_binding_value(el, "type", ctx)? {
            let kind = value_to_str(&value).trim().to_ascii_lowercase();
            if kind != "button" {
                return Err(CrepusError::render(format!(
                    "unsupported View IR button type {kind:?}; only ordinary buttons are supported"
                )));
            }
        }
        let label = component_label(el, ctx)?;
        let on_click = el
            .event_handlers
            .iter()
            .find(|e| e.event == "click")
            .map(|e| e.handler.clone());
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Button {
            label,
            label_parts: button_label_parts(el, ctx)?,
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
            bind: binding_raw(el, "bind")?,
            checked: input_initial_checked(el, ctx, false)?,
            on_change: event_handler(el, "change"),
            on_long_press: None,
            style: hints.style.opt(),
        }));
    }

    if tag == "checkbox" || (tag == "input" && input_type.as_deref() == Some("checkbox")) {
        let label = component_label(el, ctx)?;
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        let mut form_style = form_style(el, ctx, hints.style, true)?;
        form_style.form_value = input_initial_value(el, ctx)?;
        return Ok(Some(ViewNode::Checkbox {
            label,
            bind: binding_raw(el, "bind")?,
            checked: input_initial_checked(el, ctx, tag == "input")?,
            on_change: event_handler(el, "change"),
            style: form_style.opt(),
        }));
    }

    if tag == "slider" {
        let min = binding_f32(el, "min", ctx)?.unwrap_or(0.0);
        let max = binding_f32(el, "max", ctx)?.unwrap_or(100.0);
        let step = binding_f32(el, "step", ctx)?;
        if min > max {
            return Err(CrepusError::render("slider min must not exceed max"));
        }
        if step.is_some_and(|step| step <= 0.0) {
            return Err(CrepusError::render("slider step must be positive"));
        }
        if !(max - min).is_finite() {
            return Err(CrepusError::render("slider range width must be finite"));
        }
        let value = binding_f32(el, "value", ctx)?
            .unwrap_or(0.0)
            .clamp(min, max);
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Slider {
            label: optional_component_label(el, ctx)?,
            bind: binding_raw(el, "bind")?,
            value,
            min,
            max,
            step,
            on_change: event_handler(el, "change"),
            style: form_style(el, ctx, hints.style, false)?.opt(),
        }));
    }

    if tag == "progress" {
        let max = binding_f32(el, "max", ctx)?.unwrap_or(100.0);
        if max <= 0.0 {
            return Err(CrepusError::render("progress max must be positive"));
        }
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Progress {
            label: optional_component_label(el, ctx)?,
            value: binding_f32(el, "value", ctx)?.unwrap_or(0.0),
            max,
            style: hints.style.opt(),
        }));
    }

    if tag == "meter" {
        let min = binding_f32(el, "min", ctx)?.unwrap_or(0.0);
        let max = binding_f32(el, "max", ctx)?.unwrap_or(100.0);
        if min > max {
            return Err(CrepusError::render("meter min must not exceed max"));
        }
        if !(max - min).is_finite() {
            return Err(CrepusError::render("meter range width must be finite"));
        }
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Meter {
            label: optional_component_label(el, ctx)?,
            value: binding_f32(el, "value", ctx)?.unwrap_or(0.0),
            min,
            max,
            style: hints.style.opt(),
        }));
    }

    if tag == "dropzone" {
        let label = component_label(el, ctx)?;
        let accept = binding_value_string(el, "accept", ctx)?;
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

    let html_file = tag == "input" && input_type.as_deref() == Some("file");
    if matches!(tag, "file-picker" | "filepicker" | "media-picker") || html_file {
        if html_file
            && el.bindings.iter().any(|binding| {
                binding.origin != BindingOrigin::Attribute
                    && matches!(
                        binding.prop.as_str(),
                        "value" | "defaultValue" | "default-value" | "bind"
                    )
            })
        {
            return Err(CrepusError::render(
                "file inputs do not support value or bind attributes (including defaultValue and default-value)",
            ));
        }
        let label = component_label(el, ctx)?;
        let accept = binding_value_string(el, "accept", ctx)?
            .map(|value| split_accept(&value))
            .unwrap_or_default();
        let multiple = if html_file {
            single_binding_value(el, "multiple", ctx)?.is_some_and(|value| {
                !matches!(value, TemplateValue::Bool(false) | TemplateValue::Null)
            })
        } else if el.bindings.iter().any(|binding| {
            binding.matches_prop("multiple") && binding.origin == BindingOrigin::Expression
        }) {
            // Portable picker expressions retain the existing authored-presence
            // contract. Newly distinguished literal false values keep coercion.
            single_binding_value(el, "multiple", ctx)?
                .is_some_and(|value| !matches!(value, TemplateValue::Null))
        } else {
            binding_bool(el, "multiple", ctx)?
                .unwrap_or_else(|| classes.iter().any(|c| c == "multiple"))
        };
        let on_pick = if html_file {
            event_handler(el, "change")
        } else {
            el.event_handlers
                .iter()
                .find(|e| e.event == "pick" || e.event == "click")
                .map(|e| e.handler.clone())
        };
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::FilePicker {
            label,
            accept,
            multiple,
            on_pick,
            style: form_style(el, ctx, hints.style, true)?.opt(),
        }));
    }

    if tag == "input" || tag == "textfield" || tag == "textinput" || tag == "textarea" {
        if tag == "input"
            && input_type
                .as_deref()
                .is_some_and(|kind| !HTML_INPUT_TYPES.contains(&kind))
        {
            return Err(CrepusError::render(format!(
                "unsupported input type {:?}",
                input_type.as_deref().unwrap()
            )));
        }
        let bind = binding_raw(el, "bind")?.unwrap_or_default();
        let placeholder = binding_value_string(el, "placeholder", ctx)?.unwrap_or_default();
        let multiline = classes.iter().any(|c| c == "multiline") || tag == "textarea";
        let secure =
            input_type.as_deref() == Some("password") || classes.iter().any(|c| c == "secure");
        if secure && multiline {
            return Err(CrepusError::render(
                "secure multiline inputs are not supported",
            ));
        }
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        let mut form_style = form_style(el, ctx, hints.style, true)?;
        form_style.form_value = input_initial_value(el, ctx)?;
        if multiline {
            let mut child_text = String::new();
            for child in &el.children {
                match child {
                    Node::Text(parts) => child_text.push_str(&render_text_inline(parts, ctx)?),
                    Node::RawText(expression) => {
                        child_text.push_str(&value_to_str(&eval_expr(expression, ctx)?))
                    }
                    _ => {
                        return Err(CrepusError::render(
                            "textarea defaults support only text and expressions",
                        ))
                    }
                }
            }
            if !child_text.is_empty() {
                if form_style.form_value.is_some() {
                    return Err(CrepusError::render(
                        "textarea defaults cannot combine child text with a value binding",
                    ));
                }
                form_style.form_value = Some(child_text);
            }
        }
        if !multiline {
            form_style.input_type =
                input_type.filter(|kind| HTML_INPUT_TYPES.contains(&kind.as_str()));
        }
        let supports_constraints = !multiline
            && !secure
            && matches!(
                form_style.input_type.as_deref(),
                Some("number" | "date" | "datetime-local" | "month" | "time" | "week")
            );
        for (name, target) in [
            ("min", &mut form_style.form_min),
            ("max", &mut form_style.form_max),
            ("step", &mut form_style.form_step),
        ] {
            let value = single_binding_value(el, name, ctx)?;
            if value.is_some() && !supports_constraints {
                return Err(CrepusError::render(format!(
                    "{name} requires a numeric or date/time input type"
                )));
            }
            *target = value.map(|value| value_to_str(&value));
        }
        return Ok(Some(ViewNode::Input {
            placeholder,
            bind,
            multiline,
            secure,
            on_input: event_handler(el, "input"),
            on_change: event_handler(el, "change"),
            style: form_style.opt(),
        }));
    }

    if tag == "picker" || tag == "select" {
        if let Some(binding) = el.bindings.iter().find(|binding| {
            binding.origin != BindingOrigin::Attribute
                && matches!(
                    binding.prop.as_str(),
                    "value" | "defaultValue" | "default-value"
                )
        }) {
            return Err(CrepusError::render(format!(
                "View IR pickers do not support `{}` selection defaults; bind the picker value instead",
                binding.prop
            )));
        }
        let multiple = if tag == "select" {
            single_binding_value(el, "multiple", ctx)?.is_some_and(|value| {
                !matches!(value, TemplateValue::Bool(false) | TemplateValue::Null)
            })
        } else {
            binding_bool(el, "multiple", ctx)?.unwrap_or(false)
        };
        if multiple {
            return Err(CrepusError::render(
                "View IR pickers do not support multiple selection",
            ));
        }
        let bind = binding_raw(el, "bind")?.unwrap_or_default();
        let mut options = Vec::new();
        for child in &el.children {
            if let Node::Element(inner) = child {
                let inner_tag = inner.tag.to_ascii_lowercase();
                if !matches!(inner_tag.as_str(), "span" | "button" | "option") {
                    return Err(CrepusError::render(format!(
                        "unsupported View IR picker child `<{inner_tag}>`"
                    )));
                }
                if single_binding_value(inner, "selected", ctx)?.is_some_and(|value| {
                    !matches!(value, TemplateValue::Bool(false) | TemplateValue::Null)
                }) {
                    return Err(CrepusError::render(
                        "View IR picker options do not support selected defaults; bind the picker value instead",
                    ));
                }
                let (value, label) = if tag == "select" && inner_tag == "option" {
                    let value = binding_value_string(inner, "value", ctx)?;
                    let label = binding_value_string(inner, "label", ctx)?
                        .filter(|label| !label.is_empty());
                    let text = if value.is_none() || label.is_none() {
                        option_text_value(&inner.children, ctx)?
                    } else {
                        String::new()
                    };
                    (value.unwrap_or_else(|| text.clone()), label.unwrap_or(text))
                } else {
                    let label = component_label(inner, ctx)?;
                    let value = binding_value_string(inner, "value", ctx)?
                        .unwrap_or_else(|| slug_option_value(&label));
                    (value, label)
                };
                options.push(PickerOption { value, label });
            }
        }
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Picker {
            bind,
            options,
            on_change: event_handler(el, "change"),
            style: form_style(el, ctx, hints.style, true)?.opt(),
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
            tone: binding_string(el, "tone", ctx)?,
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
        let size = binding_value_string(el, "size", ctx)?
            .map(|value| {
                value
                    .trim()
                    .parse::<f32>()
                    .ok()
                    .filter(|size| size.is_finite() && *size >= 0.0)
                    .ok_or_else(|| {
                        CrepusError::render("spacer size must be finite and nonnegative")
                    })
            })
            .transpose()?
            .or(gap);
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Spacer {
            size,
            style: hints.style.opt(),
        }));
    }

    if tag == "tabs" || tag == "tabview" || tag == "page-switcher" {
        let bind = binding_raw(el, "bind")?.unwrap_or_default();
        let mut tabs = Vec::new();
        for child in &el.children {
            if let Node::Element(inner) = child {
                let inner_tag = inner.tag.to_ascii_lowercase();
                if matches!(inner_tag.as_str(), "tab" | "page") {
                    let label = binding_value_string(inner, "label", ctx)?
                        .map(|label| label.trim().to_string())
                        .filter(|label| !label.is_empty());
                    let value = binding_value_string(inner, "value", ctx)?.unwrap_or_else(|| {
                        label.as_deref().map(slug_option_value).unwrap_or_default()
                    });
                    let label = label.unwrap_or_else(|| value.replace('-', " "));
                    let icon = binding_value_string(inner, "icon", ctx)?
                        .map(|icon| icon.trim().to_string())
                        .filter(|icon| !icon.is_empty());
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
            src: binding_value_string(el, "src", ctx)?
                .map(|source| {
                    if el.bindings.iter().any(|binding| {
                        binding.origin == BindingOrigin::Attribute && binding.matches_prop("src")
                    }) {
                        source
                    } else {
                        source.trim().to_string()
                    }
                })
                .unwrap_or_default(),
            style: hints.style.opt(),
        }));
    }

    if tag == "img" || tag == "image" {
        let src = binding_value_string(el, "src", ctx)?.unwrap_or_default();
        let alt = binding_value_string(el, "alt", ctx)?;
        let placeholder = binding_value_string(el, "placeholder", ctx)?;
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Image {
            src,
            src_bind: url_binding(el, "src"),
            alt,
            placeholder,
            on_long_press: None,
            style: hints.style.opt(),
        }));
    }

    if tag == "a" || tag == "link" {
        let hints = style::extract_stack_hints(classes.to_vec(), Some(ctx));
        return Ok(Some(ViewNode::Link {
            href: binding_value_string(el, "href", ctx)?.unwrap_or_default(),
            href_bind: url_binding(el, "href"),
            target: binding_string(el, "target", ctx)?,
            rel: binding_string(el, "rel", ctx)?,
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

fn option_text_value(children: &[Node], ctx: &TemplateContext) -> Result<String, CrepusError> {
    let mut text = String::new();
    let mut pending = children.iter().rev().collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        match node {
            Node::Text(parts) => text.push_str(&render_text_inline(parts, ctx)?),
            Node::RawText(expr) => text.push_str(&value_to_str(&eval_expr(expr, ctx)?)),
            Node::Element(element)
                if !element.tag.eq_ignore_ascii_case("script")
                    && !element.tag.eq_ignore_ascii_case("template") =>
            {
                pending.extend(element.children.iter().rev());
            }
            _ => {}
        }
    }
    Ok(text
        .split(['\t', '\n', '\u{000c}', '\r', ' '])
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" "))
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

fn button_label_parts(el: &Element, ctx: &TemplateContext) -> Result<Vec<LabelPart>, CrepusError> {
    // Nonempty attribute labels retain their precedence and snapshot semantics.
    if binding_value_string(el, "label", ctx)?.is_some_and(|label| !label.trim().is_empty()) {
        return Ok(Vec::new());
    }
    Ok(primary_text_parts(&el.children, ctx)?
        .filter(|parts| parts.iter().any(|part| matches!(part, TextPart::Expr(_))))
        .map(|parts| {
            parts
                .into_iter()
                .map(|part| match part {
                    TextPart::Literal(value) => LabelPart::Literal(value),
                    TextPart::Expr(value) => LabelPart::Expr(value),
                })
                .collect()
        })
        .unwrap_or_default())
}

fn component_label(el: &Element, ctx: &TemplateContext) -> Result<String, CrepusError> {
    optional_component_label(el, ctx).map(|label| label.unwrap_or_default())
}

fn optional_component_label(
    el: &Element,
    ctx: &TemplateContext,
) -> Result<Option<String>, CrepusError> {
    if let Some(label) = binding_value_string(el, "label", ctx)? {
        let label = label.trim().to_string();
        if !label.is_empty() {
            return Ok(Some(label));
        }
    }
    let label = collect_primary_text(el.children.as_slice(), ctx)?;
    Ok((!label.is_empty()).then_some(label))
}

fn binding_raw(el: &Element, prop: &str) -> Result<Option<String>, CrepusError> {
    let mut bindings = el
        .bindings
        .iter()
        .filter(|binding| binding.prop == prop && binding.origin != BindingOrigin::Attribute);
    let first = bindings.next();
    if bindings.next().is_some() {
        return Err(CrepusError::render(format!("duplicate {prop} binding")));
    }
    Ok(first
        .map(|binding| {
            binding
                .value
                .trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .trim()
                .to_string()
        })
        .filter(|value| !value.is_empty()))
}

fn single_binding_value(
    el: &Element,
    prop: &str,
    ctx: &TemplateContext,
) -> Result<Option<TemplateValue>, CrepusError> {
    let mut bindings = el
        .bindings
        .iter()
        .filter(|binding| binding.matches_prop(prop));
    let first = bindings.next();
    if bindings.next().is_some() {
        return Err(CrepusError::render(format!("duplicate {prop} binding")));
    }
    first
        .map(|binding| eval_binding(binding, ctx))
        .transpose()
        .map(Option::flatten)
}

fn input_initial_checked(
    el: &Element,
    ctx: &TemplateContext,
    html_presence: bool,
) -> Result<bool, CrepusError> {
    let mut authored = ["checked", "defaultChecked", "default-checked"]
        .into_iter()
        .filter(|name| {
            el.bindings.iter().any(|binding| {
                binding.matches_prop(name)
                    && (binding.origin != BindingOrigin::Attribute || *name == "checked")
            })
        });
    let Some(name) = authored.next() else {
        return Ok(false);
    };
    if authored.next().is_some() {
        return Err(CrepusError::render(
            "checked defaults cannot combine checked, defaultChecked or default-checked attributes",
        ));
    }
    if html_presence {
        Ok(single_binding_value(el, name, ctx)?.is_some_and(|value| {
            !matches!(value, TemplateValue::Bool(false) | TemplateValue::Null)
        }))
    } else {
        Ok(binding_bool(el, name, ctx)?.unwrap_or(false))
    }
}

// View IR stores one initial value and cannot represent separate live/reset defaults.
fn input_initial_value(el: &Element, ctx: &TemplateContext) -> Result<Option<String>, CrepusError> {
    let mut authored = ["value", "defaultValue", "default-value"]
        .into_iter()
        .filter(|name| {
            el.bindings.iter().any(|binding| {
                binding.matches_prop(name)
                    && (binding.origin != BindingOrigin::Attribute
                        || (*name == "value" && !el.tag.eq_ignore_ascii_case("textarea")))
            })
        });
    let Some(name) = authored.next() else {
        return Ok(None);
    };
    if authored.next().is_some() {
        return Err(CrepusError::render(
            "input defaults cannot combine value, defaultValue or default-value attributes",
        ));
    }
    Ok(single_binding_value(el, name, ctx)?.map(|value| value_to_str(&value)))
}

fn form_style(
    el: &Element,
    ctx: &TemplateContext,
    mut style: ViewStyle,
    supports_required: bool,
) -> Result<ViewStyle, CrepusError> {
    style.form_name = single_binding_value(el, "name", ctx)?.map(|value| value_to_str(&value));
    let required = single_binding_value(el, "required", ctx)?;
    if !supports_required && required.is_some() {
        return Err(CrepusError::render("required is not supported on sliders"));
    }
    style.required = required
        .filter(|value| !matches!(value, TemplateValue::Bool(false) | TemplateValue::Null))
        .map(|_| true);
    Ok(style)
}

fn url_binding(el: &Element, prop: &str) -> Option<String> {
    // shortcut: captured URLs stay snapshots until IR carries lexical environments.
    if MIXED_TEXT_SNAPSHOT_DEPTH.with(|depth| depth.get() > 0) {
        return None;
    }
    el.bindings
        .iter()
        .find(|binding| {
            binding.prop == prop
                && binding.origin == BindingOrigin::Expression
                && crepuscularity_core::eval::eval_literal(&binding.value).is_none()
        })
        .map(|binding| binding.value.clone())
}

fn binding_value_string(
    el: &Element,
    prop: &str,
    ctx: &TemplateContext,
) -> Result<Option<String>, CrepusError> {
    Ok(single_binding_value(el, prop, ctx)?.map(|value| value_to_str(&value)))
}

fn binding_string(
    el: &Element,
    prop: &str,
    ctx: &TemplateContext,
) -> Result<Option<String>, CrepusError> {
    Ok(binding_value_string(el, prop, ctx)?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty()))
}

fn binding_f32(
    el: &Element,
    prop: &str,
    ctx: &TemplateContext,
) -> Result<Option<f32>, CrepusError> {
    Ok(binding_string(el, prop, ctx)?
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite()))
}

fn binding_bool(
    el: &Element,
    prop: &str,
    ctx: &TemplateContext,
) -> Result<Option<bool>, CrepusError> {
    let attribute = el
        .bindings
        .iter()
        .any(|binding| binding.matches_prop(prop) && binding.origin == BindingOrigin::Attribute);
    Ok(single_binding_value(el, prop, ctx)?.and_then(|value| {
        if attribute {
            return Some(true);
        }
        if matches!(value, TemplateValue::Null) {
            return Some(false);
        }
        match value_to_str(&value).trim() {
            "" | "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" => Some(false),
            _ => None,
        }
    }))
}

fn event_handler(el: &Element, event: &str) -> Option<String> {
    el.event_handlers
        .iter()
        .find(|handler| {
            handler.event == event || (event == "longpress" && handler.event == "longPress")
        })
        .map(|handler| handler.handler.clone())
}

fn is_text_tag(tag: &str) -> bool {
    matches!(
        tag,
        "span" | "text" | "p" | "label" | "caption" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
    )
}

fn reject_unsupported_control_state(
    el: &Element,
    ctx: &TemplateContext,
) -> Result<(), CrepusError> {
    for binding in el.bindings.iter().filter(|binding| {
        binding.prop.eq_ignore_ascii_case("disabled")
            || binding.prop.eq_ignore_ascii_case("readonly")
    }) {
        if binding.origin == BindingOrigin::Attribute && eval_binding(binding, ctx)?.is_none() {
            continue;
        }
        return Err(CrepusError::render(format!(
            "unsupported View IR control state `{}` on `<{}>`",
            binding.prop, el.tag
        )));
    }
    Ok(())
}

fn reject_metadata_events(
    el: &Element,
    owner: &str,
    ctx: &TemplateContext,
) -> Result<(), CrepusError> {
    reject_unsupported_control_state(el, ctx)?;
    if let Some(handler) = el.event_handlers.first() {
        return Err(CrepusError::render(format!(
            "View IR cannot preserve `{}` handlers on `<{}>` in metadata-only children of `<{}>`",
            handler.event, el.tag, owner
        )));
    }
    Ok(())
}

fn reject_unsupported_flattened_nodes(
    nodes: &[Node],
    owner: &str,
    ctx: &TemplateContext,
) -> Result<(), CrepusError> {
    let mut pending = nodes.iter().rev().collect::<Vec<_>>();
    while let Some(node) = pending.pop() {
        match node {
            Node::Embed(_) => {
                return Err(CrepusError::render(format!(
                    "View IR does not support embedded islands in metadata-only children of `<{owner}>`"
                )));
            }
            Node::RawHtml(_) => {
                return Err(CrepusError::render(format!(
                    "View IR does not support raw HTML in metadata-only children of `<{owner}>`"
                )));
            }
            Node::Element(el) => {
                reject_metadata_events(el, owner, ctx)?;
                pending.extend(el.children.iter().rev());
            }
            Node::If(_) | Node::For(_) | Node::Match(_) => {
                return Err(CrepusError::render(format!(
                    "View IR does not support control flow in metadata-only children of `<{owner}>`"
                )));
            }
            Node::Include(_) => {
                return Err(CrepusError::render(format!(
                    "View IR does not support includes in metadata-only children of `<{owner}>`"
                )));
            }
            _ => {}
        }
    }
    Ok(())
}

fn primary_text_parts(
    children: &[Node],
    ctx: &TemplateContext,
) -> Result<Option<Vec<TextPart>>, CrepusError> {
    for child in children {
        match child {
            Node::Text(parts) => return Ok(Some(parts.clone())),
            Node::RawText(expr) => return Ok(Some(vec![TextPart::Expr(expr.clone())])),
            Node::Element(inner) => {
                if let Some(parts) = primary_text_parts(&inner.children, ctx)? {
                    if parts.iter().any(|part| matches!(part, TextPart::Expr(_)))
                        || !render_text_inline(&parts, ctx)?.is_empty()
                    {
                        return Ok(Some(parts));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(None)
}

fn collect_primary_text(children: &[Node], ctx: &TemplateContext) -> Result<String, CrepusError> {
    primary_text_parts(children, ctx)?
        .map(|parts| render_text_inline(&parts, ctx))
        .unwrap_or_else(|| Ok(String::new()))
}

fn active_classes(el: &Element, ctx: &TemplateContext) -> Result<Vec<String>, CrepusError> {
    let mut expanded = Vec::new();
    for c in el.classes.iter() {
        expanded.extend(ctx.interpolate(c)?.split_whitespace().map(str::to_string));
    }
    if el
        .bindings
        .iter()
        .any(|binding| binding.matches_prop("class") && binding.origin == BindingOrigin::Attribute)
    {
        if let Some(value) = single_binding_value(el, "class", ctx)? {
            expanded.extend(value_to_str(&value).split_whitespace().map(str::to_string));
        }
    }
    for cc in &el.conditional_classes {
        if ctx.eval_condition(&cc.condition)? {
            expanded.extend(
                ctx.interpolate(&cc.class)?
                    .split_whitespace()
                    .map(str::to_string),
            );
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
    // shortcut: lexical scopes retain mixed snapshots until IR carries their provenance.
    if !matches!(parts, [TextPart::Expr(_)])
        && MIXED_TEXT_SNAPSHOT_DEPTH.with(|depth| depth.get() > 0)
    {
        return None;
    }
    crepuscularity_core::text_binding::encode_text_binding(parts)
}

fn render_if(block: &IfBlock, ctx: &TemplateContext) -> Result<ViewNode, CrepusError> {
    crepuscularity_core::eval::validate_expr(&block.condition)?;
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
    crate::scoped_expression::validate_collection_expr(&block.iterator)?;
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
        if let Some(literal) = pattern.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            let lit = crepuscularity_core::parser::unescape_crepus_text_literal(literal);
            if value == lit {
                let children = render_nodes_list(&arm.body, ctx)?;
                return Ok(stack_column_raw(children));
            }
        } else if value == pattern {
            let children = render_nodes_list(&arm.body, ctx)?;
            return Ok(stack_column_raw(children));
        }
    }
    Ok(stack_column_raw(vec![]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crepuscularity_core::ast::TextPart;
    use crepuscularity_core::context::TemplateContext;

    #[test]
    fn test_render_text_inline_literal_only() {
        let parts = vec![TextPart::Literal("Hello world".to_string())];
        let ctx = TemplateContext::new();
        let result = render_text_inline(&parts, &ctx).unwrap();
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_render_text_inline_expr_only() {
        let parts = vec![TextPart::Expr("name".to_string())];
        let mut ctx = TemplateContext::new();
        ctx.set("name", "Alice");
        let result = render_text_inline(&parts, &ctx).unwrap();
        assert_eq!(result, "Alice");
    }

    #[test]
    fn test_render_text_inline_mixed() {
        let parts = vec![
            TextPart::Literal("Hello ".to_string()),
            TextPart::Expr("name".to_string()),
            TextPart::Literal("!".to_string()),
        ];
        let mut ctx = TemplateContext::new();
        ctx.set("name", "Bob");
        let result = render_text_inline(&parts, &ctx).unwrap();
        assert_eq!(result, "Hello Bob!");
    }

    #[test]
    fn test_render_text_inline_missing_var_error() {
        let parts = vec![TextPart::Expr("missing".to_string())];
        let ctx = TemplateContext::new();
        let result = render_text_inline(&parts, &ctx);
        // By default TemplateContext returns an empty string for missing variables in crepuscularity_core
        // So we just expect the output to be empty.
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_render_text_inline_number_eval() {
        let parts = vec![
            TextPart::Literal("Score: ".to_string()),
            TextPart::Expr("score".to_string()),
        ];
        let mut ctx = TemplateContext::new();
        ctx.set("score", 42);
        let result = render_text_inline(&parts, &ctx).unwrap();
        assert_eq!(result, "Score: 42");
    }

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

        // Mixed parts remain live and preserve literal provenance.
        let parts = vec![
            TextPart::Literal("hello ".to_string()),
            TextPart::Expr("name".to_string()),
        ];
        let encoded = text_bind(&parts).unwrap();
        assert_eq!(encoded, "\"\" + \"hello \" + (name)");
        assert_eq!(
            crepuscularity_core::text_binding::decode_text_binding(&encoded),
            Some(parts)
        );

        // Multiple expressions are concatenated as display text.
        let parts = vec![
            TextPart::Expr("a".to_string()),
            TextPart::Expr("b".to_string()),
        ];
        let encoded = text_bind(&parts).unwrap();
        assert_eq!(encoded, "\"\" + (a) + (b)");
        assert_eq!(
            crepuscularity_core::text_binding::decode_text_binding(&encoded),
            Some(parts)
        );
    }
}
