use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crepuscularity_core::ast::*;
use crepuscularity_core::context::{value_to_str, TemplateContext, TemplateValue};
use crepuscularity_core::eval::{eval_expr, eval_literal};
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
    render_component_file_to_lvgl_xml_with_options(
        content,
        component_name,
        ctx,
        &LvglOptions {
            name: component_name.into(),
            root: LvglRoot::Component,
        },
    )
}

pub fn render_component_file_to_lvgl_xml_with_options(
    content: &str,
    component_name: &str,
    ctx: &TemplateContext,
    options: &LvglOptions,
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
    render_nodes_to_lvgl_xml(&component.nodes, &child_ctx, options)
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
    push_xml_attr(&mut out, &options.name)?;
    out.push_str("\">\n  <view>\n");
    let ctx = lvgl_context(ctx);
    for node in render_nodes_list(nodes, &ctx, 0, None, &mut IncludeCache::default())? {
        write_node(&mut out, &node, 4)?;
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

// One render owns the snapshot; captured slots may use distinct virtual bundles.
#[derive(Default)]
struct IncludeCache {
    bundles: Vec<BundleCache>,
}

struct BundleCache {
    virtual_files: Arc<HashMap<String, String>>,
    resolutions: HashMap<(Option<PathBuf>, String), PathBuf>,
    files: HashMap<PathBuf, CachedFile>,
}

struct CachedFile {
    source: String,
    nodes: HashMap<PathBuf, Arc<Vec<Node>>>,
    components: Option<HashMap<String, CachedComponent>>,
}

struct CachedComponent {
    nodes: Arc<Vec<Node>>,
    defaults: HashMap<String, String>,
}

impl IncludeCache {
    fn bundle(&mut self, ctx: &TemplateContext) -> &mut BundleCache {
        let index = self
            .bundles
            .iter()
            .position(|bundle| Arc::ptr_eq(&bundle.virtual_files, &ctx.virtual_files))
            .unwrap_or_else(|| {
                self.bundles.push(BundleCache {
                    virtual_files: Arc::clone(&ctx.virtual_files),
                    resolutions: HashMap::new(),
                    files: HashMap::new(),
                });
                self.bundles.len() - 1
            });
        &mut self.bundles[index]
    }

    fn resolve(&mut self, ctx: &TemplateContext, literal: &str) -> Result<PathBuf, CrepusError> {
        let key = (ctx.base_dir.clone(), literal.to_owned());
        let resolutions = &mut self.bundle(ctx).resolutions;
        if let Some(path) = resolutions.get(&key) {
            return Ok(path.clone());
        }
        let path = resolve_include_path(ctx.base_dir.as_deref(), literal)?;
        resolutions.insert(key, path.clone());
        Ok(path)
    }

    fn file(&mut self, ctx: &TemplateContext, path: &Path) -> Result<&mut CachedFile, CrepusError> {
        use std::collections::hash_map::Entry;
        Ok(match self.bundle(ctx).files.entry(path.to_path_buf()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(CachedFile {
                source: read_file(ctx, path)?,
                nodes: HashMap::new(),
                components: None,
            }),
        })
    }
}

// Include rendering is synchronous, so supplied slots can borrow their caller.
struct SlotFrame<'a> {
    nodes: &'a [Node],
    context: &'a TemplateContext,
    caller_slot: Option<&'a SlotFrame<'a>>,
}

fn render_nodes_list(
    nodes: &[Node],
    ctx: &TemplateContext,
    depth: usize,
    slot: Option<&SlotFrame<'_>>,
    cache: &mut IncludeCache,
) -> Result<Vec<XmlNode>, CrepusError> {
    let mut ctx = std::borrow::Cow::Borrowed(ctx);
    let mut out = Vec::new();
    for node in nodes {
        match node {
            Node::LetDecl(decl) => {
                if !(decl.is_default && ctx.vars.contains_key(&decl.name)) {
                    let value = eval_expr(&decl.expr, &ctx)?;
                    ctx.to_mut().vars.insert(decl.name.clone(), value);
                }
            }
            Node::If(block) => {
                let body = if ctx.eval_condition(&block.condition)? {
                    &block.then_children
                } else {
                    block.else_children.as_deref().unwrap_or(&[])
                };
                out.extend(render_nodes_list(body, &ctx, depth, slot, cache)?);
            }
            Node::For(block) => {
                let TemplateValue::List(items) = eval_expr(&block.iterator, &ctx)? else {
                    continue;
                };
                let mut loop_ctx = ctx.as_ref().clone();
                let mut overwritten = Vec::new();
                let pattern = block.pattern.trim();
                for item_ctx in items {
                    for (key, value) in &item_ctx.vars {
                        overwritten.push((
                            key.clone(),
                            loop_ctx.vars.insert(key.clone(), value.clone()),
                        ));
                    }
                    if !pattern.is_empty() {
                        let value = item_ctx
                            .get("value")
                            .cloned()
                            .unwrap_or_else(|| TemplateValue::Scope(item_ctx.clone()));
                        overwritten.push((
                            pattern.to_string(),
                            loop_ctx.vars.insert(pattern.to_string(), value),
                        ));
                    }
                    let rendered = render_nodes_list(&block.body, &loop_ctx, depth, slot, cache);
                    // Reverse restores item fields that share the loop alias.
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
                    out.extend(rendered?);
                }
            }
            Node::Match(block) => out.extend(render_match(block, &ctx, depth, slot, cache)?),
            Node::Element(el) => out.push(render_element(el, &ctx, depth, slot, cache)?),
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
                let (inner, inner_ctx) = expand_include(inc, &ctx, depth, cache)?;
                let frame = SlotFrame {
                    nodes: &inc.slot,
                    context: &ctx,
                    caller_slot: slot,
                };
                out.extend(render_nodes_list(
                    &inner,
                    &inner_ctx,
                    depth + 1,
                    (!inc.slot.is_empty()).then_some(&frame),
                    cache,
                )?);
            }
            Node::Embed(embed) => {
                return Err(CrepusError::render(format!(
                    "embed island '{}' is not supported by the LVGL target; guard web-only islands with is_web",
                    embed.src
                )));
            }
        }
    }
    Ok(out)
}

fn render_match(
    block: &MatchBlock,
    ctx: &TemplateContext,
    depth: usize,
    slot: Option<&SlotFrame<'_>>,
    cache: &mut IncludeCache,
) -> Result<Vec<XmlNode>, CrepusError> {
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
            return render_nodes_list(&arm.body, ctx, depth, slot, cache);
        }
    }
    Ok(Vec::new())
}

fn render_element(
    el: &Element,
    ctx: &TemplateContext,
    depth: usize,
    slot: Option<&SlotFrame<'_>>,
    cache: &mut IncludeCache,
) -> Result<XmlNode, CrepusError> {
    let mut classes = Vec::new();
    for class in &el.classes {
        classes.extend(
            ctx.interpolate(class)?
                .split_whitespace()
                .map(str::to_string),
        );
    }
    for class in &el.conditional_classes {
        if ctx.eval_condition(&class.condition)? {
            classes.extend(
                ctx.interpolate(&class.class)?
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

    let mut children = if el.tag == "slot" {
        if let Some(frame) = slot {
            render_nodes_list(frame.nodes, frame.context, depth, frame.caller_slot, cache)?
        } else if let Some((nodes, slot_ctx)) = &ctx.slot {
            render_nodes_list(nodes, slot_ctx, depth, None, cache)?
        } else {
            render_nodes_list(&el.children, ctx, depth, slot, cache)?
        }
    } else {
        render_nodes_list(&el.children, ctx, depth, slot, cache)?
    };
    let text = take_text_child(&mut children);
    let mut attrs = Vec::new();
    if let Some(id) = &el.id {
        attrs.push(("id".into(), id.clone()));
    }
    apply_class_attrs(&classes, &mut attrs);
    for binding in &el.bindings {
        if binding.origin == BindingOrigin::Attribute {
            if binding.matches_prop("class") {
                continue;
            }
            if let Some(value) = crepuscularity_core::eval::eval_binding(binding, ctx)? {
                let mut name = lvgl_html_attribute_name(&binding.prop);
                let value = if matches!(
                    name.to_ascii_lowercase().as_str(),
                    "hidden" | "disabled" | "checked"
                ) {
                    name.make_ascii_lowercase();
                    "true".to_owned()
                } else {
                    value_to_str(&value)
                };
                attrs.push((name, value));
            }
            continue;
        }

        if binding.prop.eq_ignore_ascii_case("hidden")
            || binding.prop.eq_ignore_ascii_case("disabled")
        {
            let value = eval_expr(&binding.value, ctx)?;
            let present = !matches!(value, TemplateValue::Bool(false) | TemplateValue::Null);
            attrs.push((binding.prop.to_ascii_lowercase(), present.to_string()));
            continue;
        }
        if binding.prop.eq_ignore_ascii_case("checked")
            && matches!(el.tag.as_str(), "checkbox" | "switch")
        {
            let value = eval_expr(&binding.value, ctx)?;
            // Match native custom controls rather than XML string truthiness.
            let checked = !matches!(value, TemplateValue::Null)
                && matches!(
                    value_to_str(&value).trim(),
                    "" | "true" | "1" | "yes" | "on"
                );
            attrs.push(("checked".into(), checked.to_string()));
            continue;
        }
        if matches!(binding.prop.as_str(), "min" | "max")
            && matches!(el.tag.as_str(), "slider" | "progress" | "meter")
        {
            attrs.push((
                lvgl_attr_name(&el.tag, &binding.prop),
                eval_integer_attribute(&el.tag, &binding.prop, &binding.value, ctx)?,
            ));
            continue;
        }
        if binding.prop == "step" && matches!(el.tag.as_str(), "slider" | "progress" | "meter") {
            let step = eval_integer_attribute(&el.tag, "step", &binding.value, ctx)?;
            if step != "1" {
                return Err(CrepusError::render(format!(
                    "LVGL {} step supports only 1; nonunit steps require unsupported quantization",
                    el.tag
                )));
            }
            continue;
        }
        if binding.prop == "bind" {
            let (name, value) = eval_form_binding(&el.tag, &binding.value, ctx)?;
            attrs.push((name.into(), value));
        } else {
            attrs.push((
                lvgl_attr_name(&el.tag, &binding.prop),
                eval_binding(&binding.value, ctx)?,
            ));
        }
    }
    for handler in &el.event_handlers {
        attrs.push((
            format!("data_on_{}", handler.event),
            handler.handler.clone(),
        ));
    }

    let tag = map_tag(&el.tag, text.as_deref(), children.is_empty());
    if let Some(text) = text {
        if matches!(
            tag.as_str(),
            "lv_label" | "lv_textarea" | "lv_checkbox" | "lv_dropdown"
        ) {
            attrs.push(("text".into(), text));
        } else {
            let mut new_children = Vec::with_capacity(children.len() + 1);
            new_children.push(label_node(text));
            new_children.extend(children);
            children = new_children;
        }
    }
    let mut seen = std::collections::HashSet::new();
    attrs.reverse();
    attrs.retain(|(name, _)| seen.insert(name.clone()));
    attrs.reverse();
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
            TextPart::Literal(s) => out.push_str(s),
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
    cache: &mut IncludeCache,
) -> Result<(Arc<Vec<Node>>, TemplateContext), CrepusError> {
    if depth >= MAX_INCLUDE_DEPTH {
        return Err(CrepusError::render(format!(
            "maximum include depth ({MAX_INCLUDE_DEPTH}) exceeded; possible circular include involving '{}'",
            inc.path
        )));
    }

    if let Some((file_part, comp_name)) = inc.path.split_once('#') {
        return expand_named_component(inc, ctx, file_part, comp_name, depth, cache);
    }

    let file_path = cache.resolve(ctx, &inc.path)?;
    let file = cache.file(ctx, &file_path)?;
    let frontend_path =
        crepuscularity_core::include_paths::include_frontend_path(&file_path, &inc.path);
    let nodes = if let Some(nodes) = file.nodes.get(&frontend_path) {
        Arc::clone(nodes)
    } else {
        let nodes = Arc::new(
            crepuscularity_core::parse_template_with_path(&file.source, Some(&frontend_path))
                .map_err(|e| CrepusError::render(format!("include parse error: {e}")))?,
        );
        file.nodes.insert(frontend_path, Arc::clone(&nodes));
        nodes
    };

    let mut child_ctx = TemplateContext::new();
    child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());
    child_ctx.virtual_files = ctx.virtual_files.clone();
    for (key, expr) in &inc.props {
        child_ctx.vars.insert(key.clone(), eval_expr(expr, ctx)?);
    }

    Ok((nodes, lvgl_context(&child_ctx)))
}

fn expand_named_component(
    inc: &IncludeNode,
    ctx: &TemplateContext,
    file_part: &str,
    comp_name: &str,
    depth: usize,
    cache: &mut IncludeCache,
) -> Result<(Arc<Vec<Node>>, TemplateContext), CrepusError> {
    if depth >= MAX_INCLUDE_DEPTH {
        return Err(CrepusError::render(format!(
            "maximum include depth ({MAX_INCLUDE_DEPTH}) exceeded; possible circular include involving '{file_part}#{comp_name}'"
        )));
    }

    let file_path = cache.resolve(ctx, file_part)?;
    let file = cache.file(ctx, &file_path)?;
    if file.components.is_none() {
        let parsed = parse_component_file(&file.source)
            .map_err(|e| CrepusError::render(format!("component file parse error: {e}")))?;
        file.components = Some(
            parsed
                .components
                .into_iter()
                .map(|(name, component)| {
                    (
                        name,
                        CachedComponent {
                            nodes: Arc::new(component.nodes),
                            defaults: component.meta.defaults,
                        },
                    )
                })
                .collect(),
        );
    }
    let comp = file
        .components
        .as_ref()
        .and_then(|components| components.get(comp_name))
        .ok_or_else(|| {
            CrepusError::render(format!("component '{comp_name}' not found in {file_part}"))
        })?;

    let mut child_ctx = TemplateContext::new();
    child_ctx.base_dir = file_path.parent().map(|p| p.to_path_buf());
    child_ctx.virtual_files = ctx.virtual_files.clone();
    for (key, expr) in &comp.defaults {
        if !child_ctx.vars.contains_key(key) {
            child_ctx
                .vars
                .insert(key.clone(), eval_expr(expr, &TemplateContext::new())?);
        }
    }
    for (key, expr) in &inc.props {
        child_ctx.vars.insert(key.clone(), eval_expr(expr, ctx)?);
    }

    Ok((Arc::clone(&comp.nodes), lvgl_context(&child_ctx)))
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
    if children.len() == 1
        && children[0].tag == "lv_label"
        && children[0].children.is_empty()
        && children[0].attrs.len() == 1
    {
        if let Some(pos) = children[0].attrs.iter().position(|(key, _)| key == "text") {
            let (_, text) = children[0].attrs.remove(pos);
            children.clear();
            return Some(text);
        }
    }
    None
}

fn eval_binding(value: &str, ctx: &TemplateContext) -> Result<String, CrepusError> {
    Ok(value_to_str(&eval_expr(value, ctx)?))
}

fn eval_integer_attribute(
    tag: &str,
    name: &str,
    source: &str,
    ctx: &TemplateContext,
) -> Result<String, CrepusError> {
    let value = eval_expr(source, ctx)?;
    let integer = match value {
        TemplateValue::Int(value) => i32::try_from(value).ok(),
        TemplateValue::Float(value)
            if value.is_finite()
                && value.fract() == 0.0
                && value >= f64::from(i32::MIN)
                && value <= f64::from(i32::MAX) =>
        {
            Some(value as i32)
        }
        TemplateValue::Str(value) => {
            let value = value.trim();
            value.parse::<i32>().ok().or_else(|| {
                let (whole, fraction) = value.split_once('.')?;
                (!fraction.is_empty() && fraction.bytes().all(|byte| byte == b'0'))
                    .then(|| whole.parse::<i32>().ok())
                    .flatten()
            })
        }
        _ => None,
    };
    integer.map(|value| value.to_string()).ok_or_else(|| {
        CrepusError::render(format!(
            "LVGL {tag} {name} requires a representable 32-bit integer"
        ))
    })
}

fn eval_form_binding(
    tag: &str,
    source: &str,
    ctx: &TemplateContext,
) -> Result<(&'static str, String), CrepusError> {
    let literal = eval_literal(source);
    // Bare/quoted bind paths are encoded as string literals in the shared AST.
    let expression = match &literal {
        Some(TemplateValue::Str(path)) => path.as_str(),
        _ => source,
    };
    let value = eval_expr(expression, ctx)?;
    match tag {
        "input" | "textarea" => Ok(("text", value_to_str(&value))),
        "checkbox" | "switch" => match value {
            TemplateValue::Bool(value) => Ok(("checked", value.to_string())),
            _ => Err(CrepusError::render(format!(
                "LVGL {tag} bind requires a boolean snapshot"
            ))),
        },
        "select" | "dropdown" | "slider" => {
            let TemplateValue::Int(value) = value else {
                return Err(CrepusError::render(format!(
                    "LVGL {tag} bind requires an integer snapshot"
                )));
            };
            let value = i32::try_from(value).map_err(|_| {
                CrepusError::render(format!(
                    "LVGL {tag} bind requires a 32-bit integer snapshot"
                ))
            })?;
            if tag != "slider" && value < 0 {
                return Err(CrepusError::render(format!(
                    "LVGL {tag} bind requires a nonnegative option index"
                )));
            }
            Ok((
                if tag == "slider" { "value" } else { "selected" },
                value.to_string(),
            ))
        }
        _ => Err(CrepusError::render(format!(
            "LVGL bind is unsupported on {tag}"
        ))),
    }
}

// Only known HTML attribute mappings are case-insensitive. Custom XML names
// retain their authored case, and ordinary property bindings retain exact matching.
fn lvgl_html_attribute_name(name: &str) -> String {
    for known in ["class", "src", "value", "placeholder", "disabled"] {
        if name.eq_ignore_ascii_case(known) {
            // Attribute-origin values keep the HTML attribute contract; only
            // ordinary property bindings apply tag-specific control lowering.
            return lvgl_attr_name("", known);
        }
    }
    lvgl_attr_name("", name)
}

fn lvgl_attr_name(tag: &str, name: &str) -> String {
    match name {
        "class" => "class".into(),
        "src" => "src".into(),
        "value" if matches!(tag, "input" | "textarea") => "text".into(),
        "value" => "value".into(),
        "min" if matches!(tag, "slider" | "progress" | "meter") => "min_value".into(),
        "max" if matches!(tag, "slider" | "progress" | "meter") => "max_value".into(),
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
            "justify-between" => attrs.push(("flex_main_place".into(), "space_between".into())),
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
        _ if value.starts_with("[#") && value.ends_with(']') => {
            Some(value[1..value.len() - 1].into())
        }
        _ => crepuscularity_colors::lookup_named_color(value).map(str::to_owned),
    }
}

fn write_node(out: &mut String, node: &XmlNode, indent: usize) -> Result<(), CrepusError> {
    validate_xml_chars(&node.tag)?;
    push_indent(out, indent);
    out.push('<');
    out.push_str(&node.tag);
    for (key, value) in &node.attrs {
        validate_xml_name(key)?;
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        push_xml_attr(out, value)?;
        out.push('"');
    }
    if node.children.is_empty() {
        out.push_str("/>\n");
    } else {
        out.push_str(">\n");
        for child in &node.children {
            write_node(out, child, indent + 2)?;
        }
        push_indent(out, indent);
        out.push_str("</");
        out.push_str(&node.tag);
        out.push_str(">\n");
    }
    Ok(())
}

fn push_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push(' ');
    }
}

// XML 1.0 (Fifth Edition), productions NameStartChar and NameChar.
fn xml_name_start(ch: char) -> bool {
    matches!(ch, ':' | 'A'..='Z' | '_' | 'a'..='z'
        | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}'
        | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
}

fn validate_xml_name(name: &str) -> Result<(), CrepusError> {
    validate_xml_chars(name)?;
    let mut chars = name.chars();
    let valid = chars.next().is_some_and(xml_name_start)
        && chars.all(|ch| xml_name_start(ch)
            || matches!(ch, '-' | '.' | '0'..='9' | '\u{B7}' | '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}'));
    if !valid {
        return Err(CrepusError::render(format!(
            "LVGL XML attribute name {name:?} is not a valid XML Name"
        )));
    }
    Ok(())
}

fn validate_xml_chars(value: &str) -> Result<(), CrepusError> {
    for ch in value.chars() {
        if !matches!(ch, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
        {
            return Err(CrepusError::render(format!(
                "LVGL XML cannot contain character U+{:04X}",
                ch as u32
            )));
        }
    }
    Ok(())
}

fn push_xml_attr(out: &mut String, value: &str) -> Result<(), CrepusError> {
    validate_xml_chars(value)?;
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\n' => out.push_str("&#10;"),
            '\t' => out.push_str("&#9;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(ch),
        }
    }
    Ok(())
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
        assert!(xml.contains(r##"<lv_button id="refresh" bg_color="#2b7fff""##));
        assert!(xml.contains(r#"<lv_label text="Refresh"/>"#));
    }

    #[test]
    fn justification_uses_main_axis_and_last_alignment_wins() {
        for direction in ["flex-row", "flex-col"] {
            for (classes, expected) in [
                ("justify-between", "space_between"),
                ("justify-center justify-between", "space_between"),
                ("justify-end justify-between", "space_between"),
                ("justify-between justify-center", "center"),
                ("justify-between justify-end", "end"),
            ] {
                let source =
                    format!("div flex {direction} {classes}\n  span \"Left\"\n  span \"Right\"");
                let xml = render_template_to_lvgl_xml(&source, &TemplateContext::new()).unwrap();
                assert!(
                    xml.contains(&format!("flex_main_place=\"{expected}\"")),
                    "{source}: {xml}"
                );
                assert_eq!(xml.matches("flex_main_place=").count(), 1, "{xml}");
                assert!(!xml.contains("flex_track_place="), "{xml}");
            }
        }
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
unknown-tag
  "Text"
custom-tag
"#;
        let xml = render_template_to_lvgl_xml(template, &TemplateContext::new()).unwrap();
        assert!(xml.contains("<lv_obj>"), "{xml}");
        assert!(xml.contains(r#"<lv_label text="Text"/>"#), "{xml}");
        assert!(!xml.contains(r#"<lv_obj text="Text""#), "{xml}");
        assert!(xml.contains(r#"<lv_obj/>"#));
    }
}

#[cfg(test)]
mod include_cache_tests;

#[cfg(test)]
mod binding_expression_tests {
    use super::*;

    #[test]
    fn extra_binding_braces_preserve_the_original_evaluator_error() {
        let mut ctx = TemplateContext::new();
        ctx.set("name", "VISIBLE");
        for (source, expression) in [
            ("input value={{name}}", "{name}"),
            ("div hidden={{false}}", "{false}"),
            ("button disabled={{false}}", "{false}"),
            ("checkbox checked={{false}}", "{false}"),
            ("slider min={{0}}", "{0}"),
            ("slider max={{10}}", "{10}"),
            ("slider step={{1}}", "{1}"),
            ("input bind={{name}}", "{name}"),
        ] {
            let nodes = parse_template(source).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("element")
            };
            assert_eq!(element.bindings[0].value, expression, "{source}");
            let expected = eval_expr(expression, &ctx).unwrap_err().to_string();
            let actual = render_nodes_to_lvgl_xml(&nodes, &ctx, &LvglOptions::default())
                .unwrap_err()
                .to_string();
            assert_eq!(actual, expected, "{source}");
            assert!(render_template_to_lvgl_xml(&format!("if false\n  {source}"), &ctx).is_ok());
        }
    }

    #[test]
    fn single_brace_bindings_and_literal_bind_paths_keep_existing_meaning() {
        let mut ctx = TemplateContext::new();
        ctx.set("name", "VISIBLE");
        for (source, attribute) in [
            ("input value={name}", "text=\"VISIBLE\""),
            ("div hidden={false}", "hidden=\"false\""),
            ("button disabled={false}", "disabled=\"false\""),
            ("checkbox checked={false}", "checked=\"false\""),
            ("slider min={0}", "min_value=\"0\""),
            ("slider max={10}", "max_value=\"10\""),
            ("input bind={name}", "text=\"VISIBLE\""),
            ("input bind=name", "text=\"VISIBLE\""),
        ] {
            let xml = render_template_to_lvgl_xml(source, &ctx).unwrap();
            assert!(xml.contains(attribute), "{source}: {xml}");
        }
        let xml = render_template_to_lvgl_xml("slider step={1}", &ctx).unwrap();
        assert!(!xml.contains("step="), "{xml}");
    }
}
