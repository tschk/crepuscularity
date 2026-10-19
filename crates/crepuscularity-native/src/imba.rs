//! Imba component source with JavaScript expressions isolated in TypeScript.

use crepuscularity_core::CrepusError;

use crate::ir::{
    html_input_type, html_tag, implicit_aria_label, implicit_form_name, ViewIr, ViewNode, ViewStyle,
};
use crate::scoped_expression::{
    bound_value_expr, checked_expr, expression_runtime, label_expr, scope_expr, slider_value_expr,
    tab_index_expr, tab_selected_expr, text_expr, LocalBinding,
};

/// Save both files together as `CrepusEmit.imba` and `CrepusEmit.bindings.ts`.
pub struct ImbaComponent {
    pub source: String,
    pub bindings: String,
}

/// Emit the supported core nodes, rejecting unsupported nodes explicitly.
pub fn emit_imba_component(ir: &ViewIr) -> Result<ImbaComponent, CrepusError> {
    let mut emitter = Emitter {
        expressions: Vec::new(),
        has_tabs: false,
        has_pickers: false,
        has_rotation: false,
        has_textareas: false,
    };
    let markup = emitter.nodes(&ir.root, 3, &[])?;
    let tabs_render = if emitter.has_tabs {
        "\t\tbindings.tabsRef(self)\n"
    } else {
        ""
    };
    // Imba sets tag properties before creating option children.
    let mut after_render = String::new();
    if emitter.has_pickers {
        after_render.push_str("\t\tbindings.syncPickers(self)\n");
    }
    if emitter.has_textareas {
        after_render.push_str("\t\tif $node$\n\t\t\tbindings.syncSSRTextareas(self)\n");
    }
    if !after_render.is_empty() {
        after_render.push_str("\t\tself\n");
    }
    let rotation_tag = if emitter.has_rotation {
        "tag CrepusRotatingText < span\n\tprop phrases = []\n\tprop interval = 3200\n\tprop cleanup = null\n\tdef render\n\t\t<self> phrases[0] || \"\"\n\tdef mount\n\t\tself.cleanup! if self.cleanup\n\t\tself.cleanup = bindings.rotate(self,phrases,interval)\n\tdef unmount\n\t\tself.cleanup! if self.cleanup\n\t\tself.cleanup = null\n\n"
    } else {
        ""
    };
    let source = format!(
        "import * as bindings from './CrepusEmit.bindings.ts'\n\n{rotation_tag}export tag CrepusEmit\n\tprop scope = {{}}\n\tprop handlers = {{}}\n\tdef render\n\t\tlet scope = self.scope || {{}}\n\t\tlet handlers = self.handlers || {{}}\n{tabs_render}\t\t<self>\n{markup}{after_render}"
    );
    let mut expressions = emitter.expressions.join("\n");
    if emitter.has_tabs {
        expressions.push_str(
            r#"
export function tabsRef(root: any): void {
  const element = root?.dom ?? root;
  if (typeof element?.querySelectorAll !== "function") return;
  globalThis.queueMicrotask(() => {
    element.querySelectorAll("[data-crepus-tabs]").forEach((group: any) => __crepusTabsRef(group));
  });
}
export function tabsKey(event: any): void { __crepusTabsKey(event); }
"#,
        );
    }
    if emitter.has_pickers {
        expressions.push_str(
            r#"
const pickerValues = new WeakMap<object, string>();
export function syncPickers(root: any): void {
  const element = root?.dom ?? root;
  if (typeof element?.querySelectorAll !== "function") return;
  element.querySelectorAll("select[data-crepus-picker]").forEach((field: any) => {
    if (field.__crepusPickerOwner !== root) return;
    const value = field.__crepusPickerValue;
    if (!pickerValues.has(field) || pickerValues.get(field) !== value) {
      field.value = value;
      pickerValues.set(field, value);
    }
  });
}
"#,
        );
    }
    if emitter.has_textareas {
        expressions.push_str(r#"
export function syncSSRTextareas(owner: any): void {
  const pending = [owner];
  while (pending.length) {
    const node = pending.pop();
    if (node?.__crepusTextareaOwner === owner) {
      const value = Object.prototype.hasOwnProperty.call(node, "value") ? node.value : node.defaultValue;
      // Imba escapes markup in textContent, but leaves ampersands unchanged.
      let text = String(value ?? "").replace(/&/g, "&amp;").replace(/\r/g, "&#13;");
      // HTML parsing removes the first LF immediately after the opening textarea.
      if (text.startsWith("\n")) text = "\n" + text;
      node.textContent = text;
    }
    if (node?.childNodes) {
      for (let i = node.childNodes.length - 1; i >= 0; i--) pending.push(node.childNodes[i]);
    }
  }
}
"#);
    }
    expressions.push_str("\nexport function drop(handlers: any, key: string | null, event: any): void {\n  event.preventDefault();\n  if (key !== null) dispatch(handlers, key, event);\n}\n");
    let mut runtime = expression_runtime(&expressions);
    if emitter.has_rotation {
        runtime.push_str(crate::scoped_expression::ROTATING_TEXT_RUNTIME);
        expressions.push_str("\nexport function rotate(node: HTMLElement, phrases: string[], interval: number) { return __crepusRotate(node, { phrases, interval })?.destroy; }\n");
    }
    let bindings = format!(
        "// Generated expression bindings for CrepusEmit.imba\n{runtime}\nconst toArray = (value: unknown): any[] => Array.isArray(value) ? value : [];\n\nexport function dispatch(handlers: any, key: string, ...args: any[]): void {{\n  if (handlers == null || typeof handlers !== \"object\" || Array.isArray(handlers) || !Object.prototype.hasOwnProperty.call(handlers, key)) return;\n  const handler = handlers[key];\n  if (typeof handler === \"function\") handler.apply(handlers, args);\n}}\n\n{expressions}\n"
    );
    Ok(ImbaComponent { source, bindings })
}

fn literal(value: &str) -> String {
    serde_json::Value::String(value.into())
        .to_string()
        .replace('{', "\\u007b")
        .replace('}', "\\u007d")
}

fn attr(name: &str, value: &str) -> String {
    format!(" {name}=({})", literal(value))
}

fn optional_attr(name: &str, value: Option<&String>) -> String {
    value.map(|value| attr(name, value)).unwrap_or_default()
}

fn identity_attrs(style: Option<&ViewStyle>) -> String {
    let mut attrs = style
        .and_then(|style| style.id.as_ref())
        .map(|id| attr("id", id))
        .unwrap_or_default();
    if let Some(label) = style.and_then(|style| style.aria_label.as_ref()) {
        attrs.push_str(&attr("aria-label", label));
    }
    if let Some(style) = style {
        for (attribute, value) in [
            ("min", &style.form_min),
            ("max", &style.form_max),
            ("step", &style.form_step),
        ] {
            if let Some(value) = value {
                attrs.push_str(&attr(attribute, value));
            }
        }
    }
    if let Some(name) = style.and_then(|style| style.form_name.as_ref()) {
        attrs.push_str(&attr("name", name));
    }
    if style.is_some_and(|style| style.required == Some(true)) {
        attrs.push_str(" required=(true)");
    }
    attrs
}

fn class_tokens(style: Option<&ViewStyle>) -> String {
    style
        .filter(|style| !style.classes.is_empty())
        .map(|style| attr("className", &style.classes.join(" ")))
        .unwrap_or_default()
}

fn classes(style: Option<&ViewStyle>) -> String {
    let mut attrs = identity_attrs(style);
    if let Some(style) = style {
        if style.html_tag.as_deref() == Some("label") {
            if let Some(target) = &style.html_for {
                attrs.push_str(&attr("for", target));
            }
        }
    }
    attrs.push_str(&class_tokens(style));
    if style.is_some_and(|style| style.html_display_contents == Some(true)) {
        attrs.push_str(&attr("style", "display: contents;"));
    }
    attrs
}

fn event(name: &str, action: Option<&String>, with_event: bool) -> String {
    action
        .map(|action| {
            format!(
                " @{name}=(bindings.dispatch(handlers,{}{}))",
                literal(action),
                if with_event { ",e" } else { "" }
            )
        })
        .unwrap_or_default()
}

struct Emitter {
    expressions: Vec<String>,
    has_tabs: bool,
    has_pickers: bool,
    has_rotation: bool,
    has_textareas: bool,
}

impl Emitter {
    fn expression(&mut self, expression: String, locals: &[LocalBinding]) -> String {
        let name = format!("e{}", self.expressions.len());
        let parameters = locals
            .iter()
            .map(|local| format!(", {}: any", local.variable))
            .collect::<String>();
        self.expressions.push(format!(
            "export function {name}(scope: any{parameters}): any {{ return {expression}; }}"
        ));
        let arguments = locals
            .iter()
            .map(|local| format!(",{}", local.variable))
            .collect::<String>();
        format!("bindings.{name}(scope{arguments})")
    }

    fn nodes(
        &mut self,
        nodes: &[ViewNode],
        indent: usize,
        locals: &[LocalBinding],
    ) -> Result<String, CrepusError> {
        if nodes.is_empty() {
            return Ok(format!("{}nil\n", "\t".repeat(indent)));
        }
        nodes
            .iter()
            .map(|node| self.node(node, indent, locals))
            .collect()
    }

    fn element(
        &mut self,
        tag: &str,
        attrs: String,
        children: &[ViewNode],
        indent: usize,
        locals: &[LocalBinding],
    ) -> Result<String, CrepusError> {
        let pad = "\t".repeat(indent);
        if children.is_empty() {
            return Ok(format!("{pad}<{tag}{attrs}>\n"));
        }
        Ok(format!(
            "{pad}<{tag}{attrs}>\n{}",
            self.nodes(children, indent + 1, locals)?
        ))
    }

    fn node(
        &mut self,
        node: &ViewNode,
        indent: usize,
        locals: &[LocalBinding],
    ) -> Result<String, CrepusError> {
        let pad = "\t".repeat(indent);
        Ok(match node {
            ViewNode::Text {
                content,
                bind,
                style,
            } => {
                let body = match bind {
                    Some(bind) => self.expression(text_expr(bind, locals, "scope."), locals),
                    None => literal(content),
                };
                let tag = html_tag(style.as_ref(), "span");
                format!("{pad}<{tag}{}> {body}\n", classes(style.as_ref()))
            }
            ViewNode::Button {
                label,
                label_parts,
                on_click,
                on_long_press,
                style,
            } => {
                let body = label_expr(label_parts, locals, "scope.")
                    .map(|expression| self.expression(expression, locals))
                    .unwrap_or_else(|| literal(label));
                format!(
                    "{pad}<button{}{}{}{}> {body}\n",
                    attr("type", "button"),
                    classes(style.as_ref()),
                    event("click", on_click.as_ref(), false),
                    optional_attr("data-crepus-on-long-press", on_long_press.as_ref())
                )
            }
            ViewNode::Stack {
                children,
                style,
                on_long_press,
                ..
            } => self.element(
                html_tag(style.as_ref(), "div"),
                format!(
                    "{}{}",
                    classes(style.as_ref()),
                    optional_attr("data-crepus-on-long-press", on_long_press.as_ref())
                ),
                children,
                indent,
                locals,
            )?,
            ViewNode::Scroll {
                children,
                style,
                on_long_press,
                ..
            } => self.element(
                html_tag(style.as_ref(), "div"),
                classes(style.as_ref())
                    + &optional_attr("data-crepus-on-long-press", on_long_press.as_ref()),
                children,
                indent,
                locals,
            )?,
            ViewNode::List {
                ordered,
                children,
                style,
            } => self.element(
                if *ordered { "ol" } else { "ul" },
                classes(style.as_ref()),
                children,
                indent,
                locals,
            )?,
            ViewNode::ListItem {
                children,
                style,
                on_long_press,
            } => self.element(
                "li",
                format!(
                    "{}{}",
                    classes(style.as_ref()),
                    optional_attr("data-crepus-on-long-press", on_long_press.as_ref())
                ),
                children,
                indent,
                locals,
            )?,
            ViewNode::Link {
                href,
                href_bind,
                target,
                rel,
                children,
                style,
            } => {
                let href = match href_bind {
                    Some(bind) => format!(
                        " href=({})",
                        self.expression(text_expr(bind, locals, "scope."), locals)
                    ),
                    None => attr("href", href),
                };
                self.element(
                    "a",
                    format!(
                        "{}{}{}{}",
                        href,
                        optional_attr("target", target.as_ref()),
                        optional_attr("rel", rel.as_ref()),
                        classes(style.as_ref())
                    ),
                    children,
                    indent,
                    locals,
                )?
            }
            ViewNode::Image {
                src,
                src_bind,
                alt,
                on_long_press,
                style,
                ..
            } => {
                // Imba rewrites literal image paths into imports; preserve the IR URL at runtime.
                let source = src_bind
                    .as_ref()
                    .map(|bind| text_expr(bind, locals, "scope."))
                    .unwrap_or_else(|| literal(src));
                let src = self.expression(source, locals);
                format!(
                    "{pad}<img src=({src}){}{}{}>\n",
                    attr("alt", alt.as_deref().unwrap_or("")),
                    classes(style.as_ref()),
                    optional_attr("data-crepus-on-long-press", on_long_press.as_ref())
                )
            }
            ViewNode::Dropzone {
                on_drop,
                style,
                children,
                ..
            } => {
                let action = on_drop
                    .as_ref()
                    .map(|action| literal(action))
                    .unwrap_or_else(|| "null".into());
                self.element("div", format!("{} @dragover=(e.preventDefault()) @drop=(bindings.drop(handlers,{action},e))", classes(style.as_ref())), children, indent, locals)?
            }
            ViewNode::WebView { src, style } => {
                let src = self.expression(literal(src), locals);
                format!("{pad}<iframe src=({src}){}>\n", classes(style.as_ref()))
            }
            ViewNode::If {
                condition,
                then_children,
                else_children,
                ..
            } => {
                let condition = self.expression(scope_expr(condition, locals, "scope."), locals);
                let mut result = format!(
                    "{pad}if {condition}\n{}",
                    self.nodes(then_children, indent + 1, locals)?
                );
                if let Some(children) = else_children {
                    result.push_str(&format!(
                        "{pad}else\n{}",
                        self.nodes(children, indent + 1, locals)?
                    ));
                }
                result
            }
            ViewNode::ForEach {
                bind,
                item_name,
                item_body,
                ..
            } => {
                let collection = self.expression(
                    format!("toArray({})", scope_expr(bind, locals, "scope.")),
                    locals,
                );
                let local = LocalBinding::loop_item(item_name, locals.len());
                let mut body_locals = locals.to_vec();
                let variable = local.variable.clone();
                body_locals.push(local);
                format!(
                    "{pad}for {variable} in {collection}\n{pad}\t<>\n{}",
                    self.nodes(item_body, indent + 2, &body_locals)?
                )
            }
            ViewNode::Progress {
                label,
                value,
                max,
                style,
            } => {
                let (value, max) = crate::codegen::shared::progress_values(*value, *max);
                format!(
                    "{pad}<progress{}{} max=({max}) value=({value})>\n",
                    optional_attr(
                        "aria-label",
                        implicit_aria_label(style.as_ref(), label.as_ref())
                    ),
                    classes(style.as_ref())
                )
            }
            ViewNode::Meter {
                label,
                value,
                min,
                max,
                style,
            } => {
                let (value, min, max) = crate::codegen::shared::meter_values(*value, *min, *max);
                format!(
                    "{pad}<meter{}{} min=({min}) max=({max}) value=({value})>\n",
                    optional_attr(
                        "aria-label",
                        implicit_aria_label(style.as_ref(), label.as_ref())
                    ),
                    classes(style.as_ref())
                )
            }
            ViewNode::Slider {
                label,
                bind,
                value,
                min,
                max,
                step,
                on_change,
                style,
            } => {
                let (value, min, max, step) =
                    crate::codegen::shared::slider_values(*value, *min, *max, *step);
                let value = self.expression(
                    slider_value_expr(bind.as_deref(), value, min, max, locals, "scope."),
                    locals,
                );
                let step = step
                    .map(|step| step.to_string())
                    .unwrap_or_else(|| literal("any"));
                let value_attribute = if bind.as_deref().unwrap_or_default().trim().is_empty() {
                    "defaultValue"
                } else {
                    "value"
                };
                // Range setters sanitize the value using the bounds and step already installed.
                format!("{pad}<input{}{}{}{} min=({min}) max=({max}) step=({step}) {value_attribute}=({value}){}>\n", attr("type", "range"), optional_attr("aria-label", implicit_aria_label(style.as_ref(), label.as_ref())), optional_attr("name", implicit_form_name(style.as_ref(), bind.as_ref())), classes(style.as_ref()), event("change", on_change.as_ref(), true))
            }
            ViewNode::Input {
                placeholder,
                bind,
                secure,
                multiline,
                on_input,
                on_change,
                style,
            } => {
                let value = bound_value_expr(bind, locals, "scope.")
                    .map(|expression| format!(" value=({})", self.expression(expression, locals)))
                    .or_else(|| {
                        style
                            .as_ref()
                            .and_then(|style| style.form_value.as_ref())
                            .map(|value| attr("defaultValue", value))
                    })
                    .unwrap_or_default();
                let owner = if *multiline && !value.is_empty() {
                    self.has_textareas = true;
                    " __crepusTextareaOwner=(self)"
                } else {
                    ""
                };
                let attrs = format!(
                    "{}{owner}{}{}{}{}{}",
                    value,
                    attr("placeholder", placeholder),
                    optional_attr("name", implicit_form_name(style.as_ref(), Some(bind))),
                    classes(style.as_ref()),
                    event("change", on_change.as_ref(), true),
                    event("input", on_input.as_ref(), true)
                );
                if *multiline {
                    format!("{pad}<textarea{attrs}>\n")
                } else {
                    format!(
                        "{pad}<input{}{attrs}>\n",
                        attr("type", html_input_type(style.as_ref(), *secure))
                    )
                }
            }
            ViewNode::Tabs {
                bind,
                tabs,
                on_change,
                style,
            } => {
                self.has_tabs = true;
                let mut wrapper_style = style.clone();
                let tablist_name = wrapper_style
                    .as_mut()
                    .and_then(|style| style.aria_label.take());
                let mut attrs = classes(wrapper_style.as_ref());
                if bind.trim().is_empty() {
                    attrs.push_str(&attr("data-crepus-tabs-unbound", "true"));
                }
                let mut result = format!(
                    "{pad}<div{}{}>\n{pad}\t<div{}{} @keydown=(bindings.tabsKey(e))>\n",
                    attrs,
                    attr("data-crepus-tabs", "true"),
                    attr("html:role", "tablist"),
                    attr("aria-label", tablist_name.as_deref().unwrap_or("Tabs"))
                );
                for (index, tab) in tabs.iter().enumerate() {
                    let selected = self.expression(
                        tab_selected_expr(bind, tabs, index, locals, "scope."),
                        locals,
                    );
                    let tab_index = self
                        .expression(tab_index_expr(bind, tabs, index, locals, "scope."), locals);
                    let click = on_change
                        .as_ref()
                        .map(|action| {
                            format!(
                                " @click=(bindings.dispatch(handlers,{},{}))",
                                literal(action),
                                literal(&tab.value)
                            )
                        })
                        .unwrap_or_default();
                    result.push_str(&format!("{pad}\t\t<button{}{} aria-selected=({selected}) tabIndex=({tab_index}){click}> {}\n", attr("type", "button"), attr("html:role", "tab"), literal(&tab.label)));
                }
                for (index, tab) in tabs.iter().enumerate() {
                    let hidden = self.expression(
                        format!(
                            "!({})",
                            tab_selected_expr(bind, tabs, index, locals, "scope.")
                        ),
                        locals,
                    );
                    let hidden_variable = format!("__crepusHidden{}", self.expressions.len());
                    // SSR needs the HTML attribute; the browser property removes its null form.
                    result.push_str(&format!(
                        "{pad}\tlet {hidden_variable} = {hidden}\n{pad}\t<div{} html:hidden=({hidden_variable} ? \"\" : null) hidden=({hidden_variable}) tabIndex=(0)>\n{}",
                        attr("html:role", "tabpanel"),
                        self.nodes(&tab.children, indent + 2, locals)?
                    ));
                }
                result
            }
            ViewNode::Picker {
                bind,
                options,
                on_change,
                style,
            } => {
                let bound_value = bound_value_expr(bind, locals, "scope.");
                let value = bound_value
                    .as_ref()
                    .map(|expression| {
                        self.has_pickers = true;
                        let value =
                            self.expression(format!("globalThis.String({expression})"), locals);
                        format!(
                            "{} __crepusPickerOwner=(self) __crepusPickerValue=({value})",
                            attr("data-crepus-picker", "")
                        )
                    })
                    .unwrap_or_default();
                let mut result = format!(
                    "{pad}<select{}{}{}{value}>\n",
                    optional_attr("name", implicit_form_name(style.as_ref(), Some(bind))),
                    classes(style.as_ref()),
                    event("change", on_change.as_ref(), true)
                );
                let mut seen_values = std::collections::HashSet::new();
                for option in options {
                    // Imba SSR requires an explicit attribute; false must omit it.
                    let selected = if seen_values.insert(&option.value) {
                        bound_value
                            .as_ref()
                            .map(|value| {
                                let expression = self.expression(
                                    format!(
                                        "globalThis.String({value}) === {}",
                                        literal(&option.value)
                                    ),
                                    locals,
                                );
                                format!(" html:selected=({expression} ? \"\" : null)")
                            })
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    result.push_str(&format!(
                        "{pad}\t<option{}{selected}> {}\n",
                        attr("value", &option.value),
                        literal(&option.label)
                    ));
                }
                result
            }
            ViewNode::FilePicker {
                label,
                accept,
                multiple,
                on_pick,
                style,
            } => format!(
                "{pad}<label{}>\n{pad}\t<input{}{}{}{}{}>\n{pad}\t{}\n",
                class_tokens(style.as_ref()),
                attr("type", "file"),
                if accept.is_empty() {
                    String::new()
                } else {
                    attr("accept", &accept.join(","))
                },
                if *multiple { " multiple=(true)" } else { "" },
                identity_attrs(style.as_ref()),
                event("change", on_pick.as_ref(), true),
                literal(label)
            ),
            ViewNode::Checkbox {
                label,
                bind,
                checked,
                on_change,
                style,
            } => {
                let checked_attribute = if bind.as_deref().unwrap_or_default().trim().is_empty() {
                    "defaultChecked"
                } else {
                    "checked"
                };
                let checked = self.expression(
                    checked_expr(bind.as_deref(), *checked, locals, "scope."),
                    locals,
                );
                format!(
                    "{pad}<label{}>\n{pad}\t<input{}{} {checked_attribute}=({checked}){}>\n{pad}\t{}\n",
                    class_tokens(style.as_ref()),
                    attr("type", "checkbox"),
                    identity_attrs(style.as_ref())
                        + &optional_attr(
                            "value",
                            style.as_ref().and_then(|style| style.form_value.as_ref())
                        ),
                    event("change", on_change.as_ref(), true),
                    literal(label)
                )
            }
            ViewNode::Toggle {
                label,
                bind,
                checked,
                on_change,
                on_long_press,
                style,
            } => {
                let checked = self.expression(
                    checked_expr(bind.as_deref(), *checked, locals, "scope."),
                    locals,
                );
                format!(
                    "{pad}<button{}{} aria-checked=({checked}){}{}{}> {}\n",
                    attr("type", "button"),
                    attr("html:role", "switch"),
                    classes(style.as_ref()),
                    event("click", on_change.as_ref(), true),
                    optional_attr("data-crepus-on-long-press", on_long_press.as_ref()),
                    literal(label)
                )
            }
            ViewNode::Badge { label, tone, style } => format!(
                "{pad}<span{}{}> {}\n",
                classes(style.as_ref()),
                optional_attr("data-tone", tone.as_ref()),
                literal(label)
            ),
            ViewNode::Divider { style, .. } => format!("{pad}<hr{}>\n", classes(style.as_ref())),
            ViewNode::Spacer { style, .. } => format!(
                "{pad}<div{}{}>\n",
                attr("aria-hidden", "true"),
                classes(style.as_ref())
            ),
            ViewNode::SlotRotate {
                phrases,
                interval_ms,
                style,
            } => {
                self.has_rotation = true;
                let phrases_value = self.expression(
                    serde_json::to_string(phrases)
                        .unwrap()
                        .replace('<', "\\u003c"),
                    locals,
                );
                format!("{pad}<CrepusRotatingText phrases=({phrases_value}) interval=({interval_ms}){}{}{}>\n",
                    attr("data-crepus-slot-rotate", &phrases.join("|")),
                    attr("data-interval-ms", &interval_ms.to_string()),
                    classes(style.as_ref()))
            }
        })
    }
}

#[cfg(test)]
mod url_binding_tests {
    use super::*;

    #[test]
    fn image_and_link_urls_are_live_in_root_and_loop_scopes() {
        fn urls(prefix: &str) -> Vec<ViewNode> {
            vec![
                ViewNode::Image {
                    src: "snapshot-image".into(),
                    src_bind: Some(format!("{prefix}imageUrl")),
                    alt: None,
                    placeholder: None,
                    on_long_press: None,
                    style: None,
                },
                ViewNode::Link {
                    href: "snapshot-link".into(),
                    href_bind: Some(format!("{prefix}destination")),
                    target: None,
                    rel: None,
                    style: None,
                    children: vec![],
                },
            ]
        }
        let root = emit_imba_component(&ViewIr {
            version: crate::ir::IR_VERSION,
            root: urls(""),
        })
        .unwrap();
        assert!(root.source.contains("src=(bindings.e"), "{}", root.source);
        assert!(root.source.contains("href=(bindings.e"), "{}", root.source);
        assert!(
            root.bindings.contains("__crepusText(scope.imageUrl)"),
            "{}",
            root.bindings
        );
        assert!(
            root.bindings.contains("__crepusText(scope.destination)"),
            "{}",
            root.bindings
        );
        let looped = emit_imba_component(&ViewIr {
            version: crate::ir::IR_VERSION,
            root: vec![ViewNode::ForEach {
                bind: "rows".into(),
                item_name: "row".into(),
                item_body: urls("row."),
                style: None,
            }],
        })
        .unwrap();
        assert!(
            looped
                .bindings
                .contains("__crepusText(_crepusItem0?.imageUrl)"),
            "{}",
            looped.bindings
        );
        assert!(
            looped
                .bindings
                .contains("__crepusText(_crepusItem0?.destination)"),
            "{}",
            looped.bindings
        );
        assert!(!root.bindings.contains("snapshot-") && !looped.bindings.contains("snapshot-"));
    }
}
