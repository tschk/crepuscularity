use std::path::Path;
use std::sync::Arc;

use crepuscularity_core::ast::*;
use crepuscularity_core::context::{value_to_str, TemplateContext, TemplateValue};
use crepuscularity_core::eval::eval_expr;
use crepuscularity_core::include_paths::resolve_include_path;
use crepuscularity_core::parser::{parse_component_file, parse_template};
use crepuscularity_core::virtual_files::lookup_virtual_file;
pub use crepuscularity_core::CrepusError;

/// Maximum include nesting depth to prevent infinite recursion / stack overflow.
const MAX_INCLUDE_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LvglOptions {
    pub name: String,
    pub root: LvglRoot,
}

impl Default for LvglOptions {
    fn default() -> Self {
        Self {
            name: "CrepusView".into(),
            root: LvglRoot::Component,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LvglRoot {
    Component,
    Screen,
}

pub fn render_template_to_lvgl_xml(
    template: &str,
    ctx: &TemplateContext,
) -> Result<String, CrepusError> {
    render_template_to_lvgl_xml_with_options(template, ctx, &LvglOptions::default())
}

pub fn render_template_to_lvgl_xml_with_options(
    template: &str,
    ctx: &TemplateContext,
    options: &LvglOptions,
) -> Result<String, CrepusError> {
    let nodes = parse_template(template)?;
    render_nodes_to_lvgl_xml(&nodes, ctx, options)
}

pub fn render_component_file_to_lvgl_xml(
    content: &str,
    component_name: &str,
    ctx: &TemplateContext,
) -> Result<String, CrepusError> {
    let file = parse_component_file(content)?;
    let component = file
        .components
        .get(component_name)
        .ok_or_else(|| CrepusError::render(format!("component not found: {component_name}")))?;
    let mut child_ctx = lvgl_context(ctx);
    for (key, expr) in &component.meta.defaults {
        if !child_ctx.vars.contains_key(key) {
            child_ctx
                .vars
                .insert(key.clone(), eval_expr(expr, &TemplateContext::new())?);
        }
    }
    render_nodes_to_lvgl_xml(
        &component.nodes,
        &child_ctx,
        &LvglOptions {
            name: component_name.into(),
            root: LvglRoot::Component,
        },
    )
}

pub fn render_nodes_to_lvgl_xml(
    nodes: &[Node],
    ctx: &TemplateContext,
    options: &LvglOptions,
) -> Result<String, CrepusError> {
    let mut out = String::new();
    let tag = match options.root {
        LvglRoot::Component => "component",
        LvglRoot::Screen => "screen",
    };
    out.push('<');
    out.push_str(tag);
    out.push_str(" name=\"");
    push_xml_attr(&mut out, &options.name);
    out.push_str("\">\n  <view>\n");
    let ctx = lvgl_context(ctx);
    for node in render_nodes_list(nodes, &ctx, 0)? {
        write_node(&mut out, &node, 4);
    }
    out.push_str("  </view>\n</");
    out.push_str(tag);
    out.push_str(">\n");
    Ok(out)
}

pub fn lvgl_context(ctx: &TemplateContext) -> TemplateContext {
    let mut c = ctx.clone();
    c.vars
        .insert("crepus_target".into(), TemplateValue::Str("lvgl".into()));
    c.vars.insert("is_lvgl".into(), TemplateValue::Bool(true));
    c.vars
        .insert("is_embedded".into(), TemplateValue::Bool(true));
    c.vars.insert("is_tui".into(), TemplateValue::Bool(false));
    c.vars.insert("is_web".into(), TemplateValue::Bool(false));
    c.vars.insert("is_gui".into(), TemplateValue::Bool(false));
    c
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct XmlNode {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<XmlNode>,
}

fn render_nodes_list(
    nodes: &[Node],
    ctx: &TemplateContext,
    depth: usize,
) -> Result<Vec<XmlNode>, CrepusError> {
    let mut ctx = ctx.clone();
    let mut out = Vec::new();
    for node in nodes {
        match node {
            Node::LetDecl(decl) => {
                if !(decl.is_default && ctx.vars.contains_key(&decl.name)) {
                    ctx.vars
                        .insert(decl.name.clone(), eval_expr(&decl.expr, &ctx)?);
                }
            }
            Node::If(block) => {
                let body = if ctx.eval_condition(&block.condition)? {
                    &block.then_children
                } else {
                    block.else_children.as_deref().unwrap_or(&[])
                };
                out.extend(render_nodes_list(body, &ctx, depth)?);
            }
            Node::For(block) => {
                for item_ctx in ctx.get_list_ref(&block.iterator) {
                    let mut loop_ctx = ctx.clone();
                    for (key, value) in &item_ctx.vars {
                        loop_ctx.vars.insert(key.clone(), value.clone());
                    }
                    let pattern = block.pattern.trim();
                    if !pattern.is_empty() {
                        let item_value = item_ctx.get_str("value");
                        if !item_value.is_empty() {
                            loop_ctx
                                .vars
                                .insert(pattern.to_string(), TemplateValue::Str(item_value));
                        }
                    }
                    out.extend(render_nodes_list(&block.body, &loop_ctx, depth)?);
                }
            }
            Node::Match(block) => out.extend(render_match(block, &ctx, depth)?),
            Node::Element(el) => out.push(render_element(el, &ctx, depth)?),
            Node::Text(parts) => out.push(label_node(render_text_inline(parts, &ctx)?)),
            Node::RawText(expr) | Node::RawHtml(expr) => {
                out.push(label_node(value_to_str(&eval_expr(expr, &ctx)?)));
            }
            Node::Include(inc) => {
                if depth >= MAX_INCLUDE_DEPTH {
                    return Err(CrepusError::render(format!(
                        "maximum include depth ({MAX_INCLUDE_DEPTH}) exceeded; possible circular include involving '{}'",
                        inc.path
                    )));
                }
                let (inner, inner_ctx) = expand_include(inc, &ctx, depth)?;
                out.extend(render_nodes_list(&inner, &inner_ctx, depth + 1)?);
            }
            Node::Embed(_) => {}
        }
    }
    Ok(out)
}

fn render_match(
    block: &MatchBlock,
    ctx: &TemplateContext,
    depth: usize,
) -> Result<Vec<XmlNode>, CrepusError> {
    let value = value_to_str(&eval_expr(&block.expr, ctx)?);
    for arm in &block.arms {
        let pattern = arm.pattern.trim();
        if pattern == "_"
            || (pattern.starts_with('"')
                && pattern.ends_with('"')
                && value == pattern[1..pattern.len() - 1])
            || value == pattern
        {
            return render_nodes_list(&arm.body, ctx, depth);
        }
    }
    Ok(Vec::new())
}

fn render_element(
    el: &Element,
    ctx: &TemplateContext,
    depth: usize,
) -> Result<XmlNode, CrepusError> {
    if el.tag == "slot" {
        let children = if let Some((nodes, slot_ctx)) = &ctx.slot {
            render_nodes_list(nodes, slot_ctx, depth)?
        } else {
            render_nodes_list(&el.children, ctx, depth)?
        };
        return Ok(XmlNode {
            tag: "lv_obj".into(),
            attrs: Vec::new(),
            children,
        });
    }

    let mut classes = el.classes.clone();
    for class in &el.conditional_classes {
        if ctx.eval_condition(&class.condition)? {
            classes.push(class.class.clone());
        }
    }

    let mut children = render_nodes_list(&el.children, ctx, depth)?;
    let text = take_text_child(&mut children);
    let mut attrs = Vec::new();
    if let Some(id) = &el.id {
        attrs.push(("id".into(), id.clone()));
    }
    apply_class_attrs(&classes, &mut attrs);
    for binding in &el.bindings {
        attrs.push((
            lvgl_attr_name(&binding.prop),
            eval_binding(&binding.value, ctx)?,
        ));
    }
    for handler in &el.event_handlers {
        attrs.push((
            format!("data_on_{}", handler.event),
            handler.handler.clone(),
        ));
    }

    let tag = map_tag(&el.tag, text.as_deref(), children.is_empty());
    if let Some(text) = text {
        if tag == "lv_button" {
            let mut new_children = Vec::with_capacity(children.len() + 1);
            new_children.push(label_node(text));
            new_children.extend(children);
            children = new_children;
        } else {
            attrs.push(("text".into(), text));
        }
    }
    Ok(XmlNode {
        tag,
        attrs,
        children,
    })
}

fn render_text_inline(parts: &[TextPart], ctx: &TemplateContext) -> Result<String, CrepusError> {
    let mut out = String::new();
    for part in parts {
        match part {
            TextPart::Literal(s) => out.push_str(&ctx.interpolate(s)?),
            TextPart::Expr(expr) => {
                out.push_str(&value_to_str(&eval_expr(expr, ctx)?));
            }
        }
    }
    Ok(out)
}

fn read_file(ctx: &TemplateContext, path: &Path) -> Result<String, CrepusError> {
    if let Some(content) = lookup_virtual_file(ctx, path) {
        return Ok(content);
    }
    std::fs::read_to_string(path)
        .map_err(|e| CrepusError::render(format!("include error: {:?}: {}", path, e)))
}

fn expand_include(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    depth: usize,
) -> Result<(Vec<Node>, TemplateContext), CrepusError> {
    if depth >= MAX_INCLUDE_DEPTH {
        return Err(CrepusError::render(format!(
            "maximum include depth ({MAX_INCLUDE_DEPTH}) exceeded; possible circular include involving '{}'",
            inc.path
        )));
    }

    if let Some((file_part, comp_name)) = inc.path.split_once('#') {
        return expand_named_component(inc, ctx, file_part, comp_name, depth);
    }

    let file_path = resolve_include_path(ctx.base_dir.as_deref(), &inc.path)?;
    let content = read_file(ctx, &file_path)?;
    let nodes = parse_template(&content)
        .map_err(|e| CrepusError::render(format!("include parse error: {e}")))?;

    let mut child_ctx = TemplateContext::new();
    child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());
    child_ctx.virtual_files = ctx.virtual_files.clone();
    for (key, expr) in &inc.props {
        child_ctx.vars.insert(key.clone(), eval_expr(expr, ctx)?);
    }
    if !inc.slot.is_empty() {
        child_ctx.slot = Some((inc.slot.clone(), Arc::new(ctx.clone())));
    }

    Ok((nodes, lvgl_context(&child_ctx)))
}

fn expand_named_component(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    file_part: &str,
    comp_name: &str,
    depth: usize,
) -> Result<(Vec<Node>, TemplateContext), CrepusError> {
    if depth >= MAX_INCLUDE_DEPTH {
        return Err(CrepusError::render(format!(
            "maximum include depth ({MAX_INCLUDE_DEPTH}) exceeded; possible circular include involving '{file_part}#{comp_name}'"
        )));
    }

    let file_path = resolve_include_path(ctx.base_dir.as_deref(), file_part)?;
    let content = read_file(ctx, &file_path)?;
    let comp_file = parse_component_file(&content)
        .map_err(|e| CrepusError::render(format!("component file parse error: {e}")))?;
    let comp = comp_file.components.get(comp_name).ok_or_else(|| {
        CrepusError::render(format!("component '{comp_name}' not found in {file_part}"))
    })?;

    let mut child_ctx = TemplateContext::new();
    child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());
    child_ctx.virtual_files = ctx.virtual_files.clone();
    for (key, expr) in &comp.meta.defaults {
        if !child_ctx.vars.contains_key(key) {
            child_ctx
                .vars
                .insert(key.clone(), eval_expr(expr, &TemplateContext::new())?);
        }
    }
    for (key, expr) in &inc.props {
        child_ctx.vars.insert(key.clone(), eval_expr(expr, ctx)?);
    }
    if !inc.slot.is_empty() {
        child_ctx.slot = Some((inc.slot.clone(), Arc::new(ctx.clone())));
    }

    Ok((comp.nodes.clone(), lvgl_context(&child_ctx)))
}

fn map_tag(tag: &str, text: Option<&str>, childless: bool) -> String {
    match tag {
        "button" => "lv_button",
        "input" | "textarea" => "lv_textarea",
        "img" | "image" => "lv_image",
        "progress" | "meter" => "lv_bar",
        "slider" => "lv_slider",
        "checkbox" => "lv_checkbox",
        "switch" => "lv_switch",
        "select" | "dropdown" => "lv_dropdown",
        "canvas" => "lv_canvas",
        "span" | "p" | "label" | "h1" | "h2" | "h3" if text.is_some() && childless => "lv_label",
        _ => "lv_obj",
    }
    .into()
}

fn label_node(text: String) -> XmlNode {
    XmlNode {
        tag: "lv_label".into(),
        attrs: vec![("text".into(), text)],
        children: Vec::new(),
    }
}

fn take_text_child(children: &mut Vec<XmlNode>) -> Option<String> {
    if children.len() == 1 && children[0].tag == "lv_label" && children[0].children.is_empty() {
        if let Some(pos) = children[0].attrs.iter().position(|(key, _)| key == "text") {
            let (_, text) = children[0].attrs.remove(pos);
            children.clear();
            return Some(text);
        }
    }
    None
}

fn eval_binding(value: &str, ctx: &TemplateContext) -> Result<String, CrepusError> {
    let trimmed = value.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') && trimmed.len() >= 2 {
        Ok(value_to_str(&eval_expr(
            &trimmed[1..trimmed.len() - 1],
            ctx,
        )?))
    } else if ctx.get(trimmed).is_some() {
        Ok(value_to_str(&eval_expr(trimmed, ctx)?))
    } else {
        ctx.interpolate(trimmed)
            .map_err(|e| CrepusError::render(e.to_string()))
    }
}

fn lvgl_attr_name(name: &str) -> String {
    match name {
        "class" => "class".into(),
        "src" => "src".into(),
        "value" => "value".into(),
        "placeholder" => "placeholder_text".into(),
        "disabled" => "disabled".into(),
        other => other.replace('-', "_"),
    }
}

fn apply_class_attrs(classes: &[String], attrs: &mut Vec<(String, String)>) {
    for class in classes {
        match class.as_str() {
            "flex" => attrs.push(("layout".into(), "flex".into())),
            "flex-row" => attrs.push(("flex_flow".into(), "row".into())),
            "flex-col" => attrs.push(("flex_flow".into(), "column".into())),
            "items-center" => attrs.push(("flex_cross_place".into(), "center".into())),
            "items-end" => attrs.push(("flex_cross_place".into(), "end".into())),
            "justify-center" => attrs.push(("flex_main_place".into(), "center".into())),
            "justify-end" => attrs.push(("flex_main_place".into(), "end".into())),
            "justify-between" => attrs.push(("flex_track_place".into(), "space_between".into())),
            "hidden" => attrs.push(("hidden".into(), "true".into())),
            "font-bold" => attrs.push(("text_font".into(), "montserrat_16".into())),
            "text-center" => attrs.push(("text_align".into(), "center".into())),
            "rounded" | "rounded-md" => attrs.push(("radius".into(), "6".into())),
            "rounded-lg" => attrs.push(("radius".into(), "8".into())),
            "rounded-full" => attrs.push(("radius".into(), "100%".into())),
            "border" => attrs.push(("border_width".into(), "1".into())),
            "w-full" => attrs.push(("width".into(), "100%".into())),
            "h-full" => attrs.push(("height".into(), "100%".into())),
            _ => apply_prefixed_class(class, attrs),
        }
    }
}

fn apply_prefixed_class(class: &str, attrs: &mut Vec<(String, String)>) {
    if let Some(value) = class.strip_prefix("w-") {
        if let Some(px) = spacing_px(value) {
            attrs.push(("width".into(), px));
        }
    } else if let Some(value) = class.strip_prefix("h-") {
        if let Some(px) = spacing_px(value) {
            attrs.push(("height".into(), px));
        }
    } else if let Some(value) = class.strip_prefix("p-") {
        if let Some(px) = spacing_px(value) {
            attrs.push(("pad_all".into(), px));
        }
    } else if let Some(value) = class.strip_prefix("px-") {
        if let Some(px) = spacing_px(value) {
            attrs.push(("pad_left".into(), px.clone()));
            attrs.push(("pad_right".into(), px));
        }
    } else if let Some(value) = class.strip_prefix("py-") {
        if let Some(px) = spacing_px(value) {
            attrs.push(("pad_top".into(), px.clone()));
            attrs.push(("pad_bottom".into(), px));
        }
    } else if let Some(value) = class.strip_prefix("gap-") {
        if let Some(px) = spacing_px(value) {
            attrs.push(("style_pad_gap".into(), px));
        }
    } else if let Some(value) = class.strip_prefix("bg-") {
        if let Some(color) = color_value(value) {
            attrs.push(("bg_color".into(), color));
        }
    } else if let Some(value) = class.strip_prefix("text-") {
        if let Some(color) = color_value(value) {
            attrs.push(("text_color".into(), color));
        } else if let Some(size) = font_size(value) {
            attrs.push(("text_font".into(), size));
        }
    } else if let Some(value) = class.strip_prefix("border-") {
        if let Some(color) = color_value(value) {
            attrs.push(("border_color".into(), color));
        }
    }
}

fn spacing_px(value: &str) -> Option<String> {
    match value {
        "0" => Some("0".into()),
        "1" => Some("4".into()),
        "2" => Some("8".into()),
        "3" => Some("12".into()),
        "4" => Some("16".into()),
        "5" => Some("20".into()),
        "6" => Some("24".into()),
        "8" => Some("32".into()),
        "10" => Some("40".into()),
        "12" => Some("48".into()),
        "16" => Some("64".into()),
        "20" => Some("80".into()),
        "24" => Some("96".into()),
        "32" => Some("128".into()),
        _ if value.ends_with("px") => Some(value.trim_end_matches("px").into()),
        _ => None,
    }
}

fn font_size(value: &str) -> Option<String> {
    match value {
        "xs" | "sm" => Some("montserrat_12".into()),
        "base" => Some("montserrat_14".into()),
        "lg" | "xl" | "2xl" => Some("montserrat_16".into()),
        _ => None,
    }
}

fn color_value(value: &str) -> Option<String> {
    match value {
        "black" => Some("#000000".into()),
        "white" => Some("#ffffff".into()),
        "transparent" => Some("#000000".into()),
        "red-500" => Some("#ef4444".into()),
        "green-500" => Some("#22c55e".into()),
        "blue-500" => Some("#3b82f6".into()),
        "yellow-500" => Some("#eab308".into()),
        "zinc-50" => Some("#fafafa".into()),
        "zinc-100" => Some("#f4f4f5".into()),
        "zinc-400" => Some("#a1a1aa".into()),
        "zinc-500" => Some("#71717a".into()),
        "zinc-800" => Some("#27272a".into()),
        "zinc-900" => Some("#18181b".into()),
        "zinc-950" => Some("#09090b".into()),
        _ if value.starts_with("[#") && value.ends_with(']') => {
            Some(value[1..value.len() - 1].into())
        }
        _ => None,
    }
}

fn write_node(out: &mut String, node: &XmlNode, indent: usize) {
    push_indent(out, indent);
    out.push('<');
    out.push_str(&node.tag);
    for (key, value) in &node.attrs {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        push_xml_attr(out, value);
        out.push('"');
    }
    if node.children.is_empty() {
        out.push_str("/>\n");
    } else {
        out.push_str(">\n");
        for child in &node.children {
            write_node(out, child, indent + 2);
        }
        push_indent(out, indent);
        out.push_str("</");
        out.push_str(&node.tag);
        out.push_str(">\n");
    }
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push(' ');
    }
}

fn push_xml_attr(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_lvgl_component_xml() {
        let template = r##"
div #root w-full h-full flex flex-col gap-2 bg-[#101820] p-4
  h1 text-white text-lg
    "Temp {temp}"
  button #refresh bg-blue-500 text-white rounded @click="refresh"
    "Refresh"
"##;
        let mut ctx = TemplateContext::new();
        ctx.set("temp", 24);
        let xml = render_template_to_lvgl_xml(template, &ctx).unwrap();
        assert!(xml.contains(r#"<component name="CrepusView">"#));
        assert!(xml.contains(r#"<lv_obj id="root" width="100%" height="100%""#));
        assert!(xml.contains(
            r##"<lv_label text_color="#ffffff" text_font="montserrat_16" text="Temp 24"/>"##
        ));
        assert!(xml.contains(r##"<lv_button id="refresh" bg_color="#3b82f6""##));
        assert!(xml.contains(r#"<lv_label text="Refresh"/>"#));
    }

    #[test]
    fn evaluates_dynamic_binding_values() {
        let template = r#"
progress #cpu value={cpu}
"#;
        let mut ctx = TemplateContext::new();
        ctx.set("cpu", 68);
        let xml = render_template_to_lvgl_xml(template, &ctx).unwrap();
        assert!(xml.contains(r#"<lv_bar id="cpu" value="68"/>"#));
    }

    #[test]
    fn applies_control_flow_before_xml_generation() {
        let template = r#"
if {ok}
  div
    "Ready"
else
  div
    "Offline"
"#;
        let mut ctx = TemplateContext::new();
        ctx.set("ok", true);
        let xml = render_template_to_lvgl_xml(template, &ctx).unwrap();
        assert!(xml.contains(r#"text="Ready""#));
        assert!(!xml.contains("Offline"));
    }

    #[test]
    fn escapes_xml_attribute_values() {
        let template = r#"
div
  "A&B <C>"
"#;
        let xml = render_template_to_lvgl_xml(template, &TemplateContext::new()).unwrap();
        assert!(xml.contains("A&amp;B &lt;C&gt;"));
    }

    #[test]
    fn expands_virtual_file_include_with_slot() {
        let template = r#"
include card.crepus title="Vitals"
  span
    "OK"
"#;
        let mut ctx = TemplateContext::new();
        std::sync::Arc::make_mut(&mut ctx.virtual_files).insert(
            "card.crepus".into(),
            r#"
div #card p-2
  h2
    "{title}"
  slot
    span
      "fallback"
"#
            .into(),
        );
        let xml = render_template_to_lvgl_xml(template, &ctx).unwrap();
        assert!(xml.contains(r#"<lv_obj id="card" pad_all="8">"#));
        assert!(xml.contains(r#"<lv_label text="Vitals"/>"#));
        assert!(xml.contains(r#"<lv_label text="OK"/>"#));
        assert!(!xml.contains("fallback"));
    }

    #[test]
    fn maps_unhandled_tags_to_lv_obj() {
        let template = r#"
unknown_tag #custom
  div
    span
      "Hello"
"#;
        let xml = render_template_to_lvgl_xml(template, &TemplateContext::new()).unwrap();
        // Since tag is "unknown_tag", it should map to "lv_obj"
        assert!(xml.contains(r#"<lv_obj id="custom">"#));
        // The unhandled child tag should also map to "lv_obj"
        assert!(xml.contains(r#"<lv_obj text="Hello"/>"#));
    }
}
