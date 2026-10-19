//! Svelte 5 single-file-component emitter, structured after [`crate::moonshine`]'s
//! JSX emitter but targeting Svelte 5 runes/markup instead of React/JSX.

use crate::ir::{
    html_input_type, implicit_aria_label, implicit_form_name, ViewIr, ViewNode, ViewStyle,
};
use crate::scoped_expression::{
    bound_value_expr, checked_expr, expression_runtime, label_expr, property_access,
    slider_value_expr, tab_index_expr, tab_selected_expr, text_expr, LocalBinding,
};

/// Emit a complete Svelte 5 `.svelte` single-file component from `ir`.
///
/// The component takes `scope` and `handlers` as runes props (`$props()`),
/// mirroring the `scope`/`handlers` contract used by the other emitters.
pub fn emit_svelte_component(ir: &ViewIr) -> String {
    let locals: Vec<LocalBinding> = Vec::new();
    let markup: String = ir
        .root
        .iter()
        .map(|n| emit_node(n, 0, &locals))
        .collect::<Vec<_>>()
        .join("\n");
    let mut expression_runtime = expression_runtime(&markup);
    if markup.contains("use:__crepusRotate=") {
        expression_runtime.push_str(crate::scoped_expression::ROTATING_TEXT_RUNTIME);
    }
    format!(
        r#"<script lang="ts">
  let {{ scope = {{}}, handlers = {{}} }}: {{ scope?: Record<string, any>; handlers?: Record<string, any> }} = $props();

  function toArray(v: unknown): any[] {{
    return Array.isArray(v) ? v : [];
  }}
{expression_runtime}
</script>

{markup}
"#
    )
}

/// Keep whitespace-sensitive and NUL data out of static HTML normalization.
/// Ordinary literal text escapes braces so data never becomes template syntax.
fn html_text(s: &str) -> String {
    let sensitive_whitespace = s.starts_with(char::is_whitespace)
        || s.ends_with(char::is_whitespace)
        || s.contains("  ")
        || s.chars().any(|c| c != ' ' && c.is_whitespace());
    if sensitive_whitespace || s.contains('\0') {
        let literal = serde_json::Value::String(s.into())
            .to_string()
            .replace('<', "\\u003c");
        return format!("{{{literal}}}");
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '{' => out.push_str("&#123;"),
            '}' => out.push_str("&#125;"),
            c => out.push(c),
        }
    }
    out
}

/// Escape static attribute values, including Svelte interpolation delimiters.
fn html_attr_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '{' => out.push_str("&#123;"),
            '}' => out.push_str("&#125;"),
            c => out.push(c),
        }
    }
    out
}

/// Preserve values that cannot round-trip through Svelte's static HTML path.
fn attr(name: &str, value: &str) -> String {
    if value.contains(['\0', '\r']) {
        let literal = serde_json::Value::String(value.into())
            .to_string()
            .replace('<', "\\u003c");
        return format!(" {name}={{{literal}}}");
    }
    format!(" {name}=\"{}\"", html_attr_value(value))
}

fn opt_attr(name: &str, value: Option<&String>) -> String {
    match value {
        Some(v) => attr(name, v),
        None => String::new(),
    }
}

/// Source identity and class tokens preserved on the node.
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
        attrs.push_str(" required");
    }
    attrs
}

fn class_tokens(style: Option<&ViewStyle>) -> String {
    style
        .filter(|style| !style.classes.is_empty())
        .map(|style| attr("class", &style.classes.join(" ")))
        .unwrap_or_default()
}

fn class_attr(style: Option<&ViewStyle>) -> String {
    let mut attrs = identity_attrs(style);
    if let Some(style) = style {
        if style.html_tag.as_deref() == Some("label") {
            if let Some(target) = &style.html_for {
                attrs.push_str(&attr("for", target));
            }
        }
    }
    if style.is_some_and(|style| style.html_display_contents == Some(true)) {
        attrs.push_str(" style=\"display: contents;\"");
    }
    attrs.push_str(&class_tokens(style));
    attrs
}

/// Rewrite expression references using the shared framework lexer.
fn scope_expr(expr: &str, locals: &[LocalBinding]) -> String {
    crate::js_expr::scope_expr(expr, locals, "scope.")
}

/// A text child: literal (HTML-escaped) unless bound, in which case `{expr}`.
fn text_child(content: &str, bind: Option<&String>, locals: &[LocalBinding]) -> String {
    match bind {
        Some(b) => format!("{{{}}}", text_expr(b, locals, "scope.")),
        None => html_text(content),
    }
}

fn on_click_attr(name: &str) -> String {
    format!(
        " onclick={{() => {}?.()}}",
        property_access("handlers", name)
    )
}

fn on_click_arg_attr(name: &str, arg: &str) -> String {
    format!(
        " onclick={{() => {}?.({arg})}}",
        property_access("handlers", name)
    )
}

fn dom_event_attr(svelte_event: &str, name: &str) -> String {
    format!(
        " {svelte_event}={{(event) => {}?.(event)}}",
        property_access("handlers", name)
    )
}

fn opt_on_click(v: Option<&String>) -> String {
    v.map(|n| on_click_attr(n)).unwrap_or_default()
}

fn opt_dom_event(svelte_event: &str, v: Option<&String>) -> String {
    v.map(|n| dom_event_attr(svelte_event, n))
        .unwrap_or_default()
}

fn opt_on_long_press(v: Option<&String>) -> String {
    v.map(|n| attr("data-crepus-on-long-press", n))
        .unwrap_or_default()
}

fn emit_children(children: &[ViewNode], indent: usize, locals: &[LocalBinding]) -> String {
    children
        .iter()
        .map(|c| emit_node(c, indent, locals))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wrap children in a nonvoid container, including an explicit closing tag.
fn element(
    tag: &str,
    attrs: &str,
    children: &[ViewNode],
    indent: usize,
    locals: &[LocalBinding],
) -> String {
    let pad = "  ".repeat(indent);
    if children.is_empty() {
        return format!("{pad}<{tag}{attrs}></{tag}>");
    }
    let inner = emit_children(children, indent + 1, locals);
    format!("{pad}<{tag}{attrs}>\n{inner}\n{pad}</{tag}>")
}

/// `{#if COND} ... {:else} ... {/if}`, omitting `{:else}` when there is none.
fn emit_if(
    condition: &str,
    then_children: &[ViewNode],
    else_children: Option<&Vec<ViewNode>>,
    indent: usize,
    locals: &[LocalBinding],
) -> String {
    let pad = "  ".repeat(indent);
    let cond = scope_expr(condition, locals);
    let then_body = emit_children(then_children, indent + 1, locals);
    match else_children {
        None => format!("{pad}{{#if {cond}}}\n{then_body}\n{pad}{{/if}}"),
        Some(ec) => {
            let else_body = emit_children(ec, indent + 1, locals);
            format!("{pad}{{#if {cond}}}\n{then_body}\n{pad}{{:else}}\n{else_body}\n{pad}{{/if}}")
        }
    }
}

/// `{#each toArray(scope.BIND) as ITEM, index (index)} ... {/each}`.
fn emit_for_each(
    bind: &str,
    item_name: &str,
    item_body: &[ViewNode],
    indent: usize,
    locals: &[LocalBinding],
) -> String {
    let pad = "  ".repeat(indent);
    let bind_expr = scope_expr(bind, locals);
    let binding = LocalBinding::loop_item(item_name, locals.len());
    let item_name = binding.variable.clone();
    let index_name = format!("_crepusIndex{}", locals.len());
    let mut inner_locals = locals.to_vec();
    inner_locals.push(binding);
    let inner = emit_children(item_body, indent + 1, &inner_locals);
    format!(
        "{pad}{{#each toArray({bind_expr}) as {item_name}, {index_name} ({index_name})}}\n{inner}\n{pad}{{/each}}"
    )
}

pub(crate) fn emit_node(node: &ViewNode, indent: usize, locals: &[LocalBinding]) -> String {
    let pad = "  ".repeat(indent);
    match node {
        ViewNode::Text {
            content,
            bind,
            style,
        } => {
            let tag = crate::ir::html_tag(style.as_ref(), "span");
            format!(
                "{pad}<{tag}{}>{}</{tag}>",
                class_attr(style.as_ref()),
                text_child(content, bind.as_ref(), locals)
            )
        }
        ViewNode::Link {
            href,
            href_bind,
            target,
            rel,
            style,
            children,
        } => {
            let attrs = format!(
                "{}{}{}{}",
                href_bind.as_ref().map_or_else(|| attr("href", href), |bind| format!(" href={{{}}}", text_expr(bind, locals, "scope."))),
                opt_attr("target", target.as_ref()),
                opt_attr("rel", rel.as_ref()),
                class_attr(style.as_ref())
            );
            element("a", &attrs, children, indent, locals)
        }
        ViewNode::Stack {
            on_long_press,
            style,
            children,
            ..
        } => {
            let attrs = format!(
                "{}{}",
                class_attr(style.as_ref()),
                opt_on_long_press(on_long_press.as_ref())
            );
            element(
                crate::ir::html_tag(style.as_ref(), "div"),
                &attrs,
                children,
                indent,
                locals,
            )
        }
        ViewNode::Scroll {
            style,
            children,
            on_long_press,
            ..
        } => element(
            crate::ir::html_tag(style.as_ref(), "div"),
            &format!(
                "{}{}",
                class_attr(style.as_ref()),
                opt_on_long_press(on_long_press.as_ref())
            ),
            children,
            indent,
            locals,
        ),
        ViewNode::Dropzone {
            on_drop,
            style,
            children,
            ..
        } => {
            let dispatch = on_drop
                .as_ref()
                .map(|name| format!("{}?.(event);", property_access("handlers", name)))
                .unwrap_or_default();
            let attrs = format!(
                "{} ondragover={{(event) => event.preventDefault()}} ondrop={{(event) => {{ event.preventDefault(); {dispatch} }}}}",
                class_attr(style.as_ref())
            );
            element("div", &attrs, children, indent, locals)
        }
        ViewNode::List {
            ordered,
            style,
            children,
        } => element(
            if *ordered { "ol" } else { "ul" },
            &class_attr(style.as_ref()),
            children,
            indent,
            locals,
        ),
        ViewNode::ListItem {
            on_long_press,
            style,
            children,
        } => {
            let attrs = format!(
                "{}{}",
                class_attr(style.as_ref()),
                opt_on_long_press(on_long_press.as_ref())
            );
            element("li", &attrs, children, indent, locals)
        }
        ViewNode::Button {
            label,
            label_parts,
            on_click,
            on_long_press,
            style,
        } => format!(
            "{pad}<button type=\"button\"{}{}{}>{}</button>",
            class_attr(style.as_ref()),
            opt_on_click(on_click.as_ref()),
            opt_on_long_press(on_long_press.as_ref()),
            label_expr(label_parts, locals, "scope.")
                .map(|expr| format!("{{{expr}}}"))
                .unwrap_or_else(|| html_text(label))
        ),
        ViewNode::Badge {
            label, tone, style, ..
        } => format!(
            "{pad}<span{}{}>{}</span>",
            class_attr(style.as_ref()),
            opt_attr("data-tone", tone.as_ref()),
            html_text(label)
        ),
        ViewNode::Divider { style, .. } => {
            format!("{pad}<hr{} />", class_attr(style.as_ref()))
        }
        ViewNode::Spacer { style, .. } => {
            format!(
                "{pad}<div aria-hidden=\"true\"{} />",
                class_attr(style.as_ref())
            )
        }
        ViewNode::Image {
            src,
            src_bind,
            alt,
            on_long_press,
            style,
            ..
        } => format!(
            "{pad}<img{}{}{}{} />",
            src_bind.as_ref().map_or_else(|| attr("src", src), |bind| format!(" src={{{}}}", text_expr(bind, locals, "scope."))),
            attr("alt", alt.as_deref().unwrap_or("")),
            class_attr(style.as_ref()),
            opt_on_long_press(on_long_press.as_ref())
        ),
        ViewNode::WebView { src, style } => format!(
            "{pad}<iframe{}{} />",
            attr("src", src),
            class_attr(style.as_ref())
        ),
        ViewNode::Toggle {
            label,
            bind,
            checked,
            on_change,
            on_long_press,
            style,
            ..
        } => format!(
            "{pad}<button type=\"button\" role=\"switch\" aria-checked={{{}}}{}{}{}>{}</button>",
            checked_expr(bind.as_deref(), *checked, locals, "scope."),
            class_attr(style.as_ref()),
            opt_dom_event("onclick", on_change.as_ref()),
            opt_on_long_press(on_long_press.as_ref()),
            html_text(label)
        ),
        ViewNode::Checkbox {
            label,
            bind,
            checked,
            on_change,
            style,
            ..
        } => format!(
            "{pad}<label{}><input type=\"checkbox\" checked={{{}}}{}{} />{}</label>",
            class_tokens(style.as_ref()),
            checked_expr(bind.as_deref(), *checked, locals, "scope."),
            // Keep checked for older Svelte SSR compilers that omit defaultChecked.
            if bind.as_deref().unwrap_or_default().trim().is_empty() {
                format!(" defaultChecked={{{checked}}}")
            } else {
                String::new()
            },
            identity_attrs(style.as_ref())
                + &opt_attr(
                    "value",
                    style.as_ref().and_then(|style| style.form_value.as_ref())
                )
                + &opt_dom_event("onchange", on_change.as_ref()),
            html_text(label)
        ),
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
            let value_expr = slider_value_expr(bind.as_deref(), value, min, max, locals, "scope.");
            let default_value = if bind.as_deref().unwrap_or_default().trim().is_empty() {
                format!(" defaultValue={{{value}}}")
            } else {
                String::new()
            };
            let step = step
                .map(|step| format!(" step={{{step}}}"))
                .unwrap_or_else(|| attr("step", "any"));
            // Set bounds before value to avoid the browser's initial 0-100 clamp.
            format!(
                "{pad}<input type=\"range\" min={{{min}}} max={{{max}}}{step} value={{{value_expr}}}{default_value}{}{}{} />",
                opt_attr("aria-label", implicit_aria_label(style.as_ref(), label.as_ref())),
                class_attr(style.as_ref()),
                opt_dom_event("onchange", on_change.as_ref())
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
                "{pad}<progress max={{{max}}} value={{{value}}}{}{}></progress>",
                opt_attr(
                    "aria-label",
                    implicit_aria_label(style.as_ref(), label.as_ref())
                ),
                class_attr(style.as_ref())
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
                "{pad}<meter min={{{min}}} max={{{max}}} value={{{value}}}{}{}></meter>",
                opt_attr(
                    "aria-label",
                    implicit_aria_label(style.as_ref(), label.as_ref())
                ),
                class_attr(style.as_ref())
            )
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
            let bound_value =
                bound_value_expr(bind, locals, "scope.").map(|expr| format!(" value={{{expr}}}"));
            let initial = style.as_ref().and_then(|style| style.form_value.as_ref());
            let body = if bound_value.is_none() && *multiline {
                initial.map(|value| html_text(value)).unwrap_or_default()
            } else {
                String::new()
            };
            let value = bound_value
                .or_else(|| {
                    if *multiline {
                        initial.map(|value| {
                            let literal = serde_json::Value::String(value.clone())
                                .to_string()
                                .replace('<', "\\u003c");
                            format!(" defaultValue={{{literal}}}")
                        })
                    } else {
                        initial.map(|value| attr("value", value))
                    }
                })
                .unwrap_or_default();
            let cls = class_attr(style.as_ref());
            let ph = attr("placeholder", placeholder);
            let name = opt_attr("name", implicit_form_name(style.as_ref(), Some(bind)));
            let on_change_attr = opt_dom_event("onchange", on_change.as_ref());
            let on_input_attr = opt_dom_event("oninput", on_input.as_ref());
            if *multiline {
                format!("{pad}<textarea{value}{ph}{name}{cls}{on_change_attr}{on_input_attr}>{body}</textarea>")
            } else {
                let ty = html_input_type(style.as_ref(), *secure);
                format!("{pad}<input type=\"{ty}\"{value}{ph}{name}{cls}{on_change_attr}{on_input_attr} />")
            }
        }
        ViewNode::Picker {
            options,
            bind,
            on_change,
            style,
        } => {
            let value = bound_value_expr(bind, locals, "scope.")
                .map(|expr| format!(" value={{{expr}}}"))
                .unwrap_or_default();
            let opts: String = options
                .iter()
                .map(|o| {
                    format!(
                        "{pad}  <option{}>{}</option>",
                        attr("value", &o.value),
                        html_text(&o.label)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "{pad}<select{value}{}{}{}>\n{opts}\n{pad}</select>",
                opt_attr("name", implicit_form_name(style.as_ref(), Some(bind))),
                class_attr(style.as_ref()),
                opt_dom_event("onchange", on_change.as_ref())
            )
        }
        ViewNode::FilePicker {
            label,
            accept,
            multiple,
            on_pick,
            style,
        } => {
            let accept_attr = if accept.is_empty() {
                String::new()
            } else {
                attr("accept", &accept.join(","))
            };
            let multiple_attr = if *multiple { " multiple" } else { "" };
            format!(
                "{pad}<label{}><input type=\"file\"{accept_attr}{multiple_attr}{} />{}</label>",
                class_tokens(style.as_ref()),
                identity_attrs(style.as_ref()) + &opt_dom_event("onchange", on_pick.as_ref()),
                html_text(label)
            )
        }
        ViewNode::SlotRotate {
            phrases,
            interval_ms,
            style,
        } => format!(
            "{pad}<span{} data-interval-ms={{{interval_ms}}}{} use:__crepusRotate={{{{ phrases: {}, interval: {interval_ms} }}}}>{}</span>",
            attr("data-crepus-slot-rotate", &phrases.join("|")),
            class_attr(style.as_ref()),
            serde_json::to_string(phrases).unwrap().replace('<', "\\u003c"),
            html_text(phrases.first().map(String::as_str).unwrap_or(""))
        ),
        ViewNode::Tabs {
            bind,
            tabs,
            on_change,
            style,
        } => {
            let mut wrapper_style = style.clone();
            let tablist_name = wrapper_style
                .as_mut()
                .and_then(|style| style.aria_label.take());
            let tablist_label = attr("aria-label", tablist_name.as_deref().unwrap_or("Tabs"));
            let mut cls = class_attr(wrapper_style.as_ref());
            if bind.trim().is_empty() {
                cls.push_str(&attr("data-crepus-tabs-unbound", "true"));
            }
            let buttons: String = tabs
                .iter()
                .enumerate()
                .map(|(index, t)| {
                    let onclick = match on_change {
                        Some(name) => on_click_arg_attr(name, &serde_json::Value::String(t.value.clone()).to_string()),
                        None => String::new(),
                    };
                    let selected = tab_selected_expr(bind, tabs, index, locals, "scope.");
                    let tabindex = tab_index_expr(bind, tabs, index, locals, "scope.");
                    format!(
                        "{pad}    <button type=\"button\" role=\"tab\" aria-selected={{{selected}}} tabindex={{{tabindex}}}{onclick}>{}</button>",
                        html_text(&t.label)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let panels: String = tabs
                .iter()
                .enumerate()
                .map(|(index, t)| {
                    let selected = tab_selected_expr(bind, tabs, index, locals, "scope.");
                    format!(
                        "{pad}  <div role=\"tabpanel\" tabindex=\"0\" hidden={{!({selected})}}>\n{}\n{pad}  </div>",
                        emit_children(&t.children, indent + 2, locals)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "{pad}<div{cls} use:__crepusTabsRef>\n{pad}  <div role=\"tablist\"{tablist_label} onkeydown={{__crepusTabsKey}} tabindex=\"-1\">\n{buttons}\n{pad}  </div>\n{panels}\n{pad}</div>"
            )
        }
        ViewNode::If {
            condition,
            then_children,
            else_children,
            ..
        } => emit_if(
            condition,
            then_children,
            else_children.as_ref(),
            indent,
            locals,
        ),
        ViewNode::ForEach {
            bind,
            item_name,
            item_body,
            ..
        } => emit_for_each(bind, item_name, item_body, indent, locals),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::render_template_to_ir;
    use crepuscularity_core::context::TemplateContext;

    #[test]
    fn component_emitter_emits_scope_and_handlers_props_contract() {
        let body = emit_svelte_component(&sample_ir());
        assert!(
            body.contains("let { scope = {}, handlers = {} }: { scope?: Record<string, any>; handlers?: Record<string, any> } = $props();"),
            "{body}"
        );
        assert!(body.contains("<script lang=\"ts\">"), "{body}");
        assert!(body.contains("</script>"), "{body}");
    }

    #[test]
    fn image_attributes_keep_carriage_returns_out_of_static_html() {
        let mut ctx = TemplateContext::new();
        ctx.set("description", "A\rB\r\nC\tD\nE");
        let ir = render_template_to_ir("img src=\"data:,\" alt={description}", &ctx).unwrap();
        let output = emit_svelte_component(&ir);
        assert!(output.contains(r#"alt={"A\rB\r\nC\tD\nE"}"#), "{output}");
        assert!(!output.contains('\r'));
    }

    fn sample_ir() -> ViewIr {
        let source = r#"stack col gap-2
 text "hi"
 button "Go"
"#;
        render_template_to_ir(source, &TemplateContext::new()).expect("ir")
    }

    #[test]
    fn emits_basic_element_with_classes() {
        let body = emit_svelte_component(&sample_ir());
        assert!(body.contains("<script lang=\"ts\">"), "{body}");
        assert!(body.contains("$props()"), "{body}");
        assert!(body.contains("<div class=\"col gap-2\">"), "{body}");
        assert!(body.contains("<span>hi</span>"), "{body}");
        assert!(
            body.contains("<button type=\"button\">Go</button>"),
            "{body}"
        );
    }

    #[test]
    fn if_without_else_omits_else_block() {
        let node = ViewNode::If {
            condition: "count > 0".to_string(),
            then_children: vec![ViewNode::Text {
                content: "yes".to_string(),
                bind: None,
                style: None,
            }],
            else_children: None,
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.starts_with("{#if scope.count > 0}"), "{out}");
        assert!(out.contains("<span>yes</span>"), "{out}");
        assert!(!out.contains("{:else}"), "{out}");
        assert!(out.trim_end().ends_with("{/if}"), "{out}");
    }

    #[test]
    fn if_with_else_renders_both_branches() {
        let node = ViewNode::If {
            condition: "count > 0".to_string(),
            then_children: vec![ViewNode::Text {
                content: "yes".to_string(),
                bind: None,
                style: None,
            }],
            else_children: Some(vec![ViewNode::Text {
                content: "no".to_string(),
                bind: None,
                style: None,
            }]),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("{:else}"), "{out}");
        assert!(out.contains("<span>yes</span>"), "{out}");
        assert!(out.contains("<span>no</span>"), "{out}");
    }

    #[test]
    fn for_each_maps_scope_array_and_leaves_item_unprefixed() {
        let node = ViewNode::ForEach {
            bind: "items".to_string(),
            item_name: "item".to_string(),
            item_body: vec![ViewNode::Text {
                content: "".to_string(),
                bind: Some("item.name".to_string()),
                style: None,
            }],
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(
            out.starts_with(
                "{#each toArray(scope.items) as _crepusItem0, _crepusIndex0 (_crepusIndex0)}"
            ),
            "{out}"
        );
        assert!(out.contains("{__crepusText(_crepusItem0?.name)}"), "{out}");
        assert!(!out.contains("scope.item?."), "{out}");
        assert!(out.trim_end().ends_with("{/each}"), "{out}");
    }

    #[test]
    fn button_click_wired_to_handlers() {
        let node = ViewNode::Button {
            label_parts: Vec::new(),
            label: "Go".to_string(),
            on_click: Some("go".to_string()),
            on_long_press: None,
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(
            out.contains("onclick={() => __crepusOwnHandler(handlers, \"go\")?.go?.()}"),
            "{out}"
        );
    }

    #[test]
    fn text_with_bind_renders_expression_not_literal() {
        let node = ViewNode::Text {
            content: "placeholder".to_string(),
            bind: Some("user.name".to_string()),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert_eq!(out, "<span>{__crepusText(scope.user?.name)}</span>");
    }

    // ── one test per remaining ViewNode variant ────────────────────────────

    #[test]
    fn emits_stack_with_long_press() {
        let node = ViewNode::Stack {
            axis: crate::ir::StackAxis::Column,
            spacing: None,
            align_items: None,
            justify_content: None,
            on_long_press: Some("press".to_string()),
            style: None,
            children: vec![],
        };
        let out = emit_node(&node, 0, &[]);
        assert!(
            out.contains(r#"data-crepus-on-long-press="press""#),
            "{out}"
        );
    }

    #[test]
    fn emits_toggle() {
        let node = ViewNode::Toggle {
            label: "On".to_string(),
            bind: None,
            checked: true,
            on_change: Some("toggle".to_string()),
            on_long_press: None,
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("role=\"switch\""), "{out}");
        assert!(
            out.contains(
                "onclick={(event) => __crepusOwnHandler(handlers, \"toggle\")?.toggle?.(event)}"
            ),
            "{out}"
        );
    }

    #[test]
    fn emits_checkbox() {
        let node = ViewNode::Checkbox {
            label: "Agree".to_string(),
            bind: None,
            checked: false,
            on_change: Some("agree".to_string()),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("type=\"checkbox\""), "{out}");
        assert!(
            out.contains(
                "onchange={(event) => __crepusOwnHandler(handlers, \"agree\")?.agree?.(event)}"
            ),
            "{out}"
        );
    }

    #[test]
    fn emits_slider() {
        let node = ViewNode::Slider {
            label: None,
            bind: None,
            value: 5.0,
            min: 0.0,
            max: 10.0,
            step: Some(1.0),
            on_change: Some("vol".to_string()),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("type=\"range\""), "{out}");
        assert!(
            out.contains(
                "onchange={(event) => __crepusOwnHandler(handlers, \"vol\")?.vol?.(event)}"
            ),
            "{out}"
        );
    }

    #[test]
    fn emits_progress() {
        let node = ViewNode::Progress {
            label: None,
            value: 3.0,
            max: 10.0,
            style: None,
        };
        assert_eq!(
            emit_node(&node, 0, &[]),
            "<progress max={10} value={3}></progress>"
        );
    }

    #[test]
    fn emits_meter() {
        let node = ViewNode::Meter {
            label: None,
            value: 3.0,
            min: 0.0,
            max: 10.0,
            style: None,
        };
        assert_eq!(
            emit_node(&node, 0, &[]),
            "<meter min={0} max={10} value={3}></meter>"
        );
    }

    #[test]
    fn emits_badge() {
        let node = ViewNode::Badge {
            label: "New".to_string(),
            tone: Some("info".to_string()),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("data-tone=\"info\""), "{out}");
    }

    #[test]
    fn emits_divider() {
        let node = ViewNode::Divider {
            axis: crate::ir::StackAxis::Row,
            style: None,
        };
        assert_eq!(emit_node(&node, 0, &[]), "<hr />");
    }

    #[test]
    fn emits_spacer() {
        let node = ViewNode::Spacer {
            size: None,
            style: None,
        };
        assert_eq!(emit_node(&node, 0, &[]), "<div aria-hidden=\"true\" />");
    }

    #[test]
    fn emits_dropzone() {
        let node = ViewNode::Dropzone {
            label: "Drop here".to_string(),
            accept: None,
            on_drop: Some("drop".to_string()),
            style: None,
            children: vec![],
        };
        let out = emit_node(&node, 0, &[]);
        assert!(
            out.contains("__crepusOwnHandler(handlers, \"drop\")?.drop?.(event)"),
            "{out}"
        );
    }

    #[test]
    fn emits_file_picker() {
        let node = ViewNode::FilePicker {
            label: "Upload".to_string(),
            accept: vec!["image/*".to_string()],
            multiple: true,
            on_pick: Some("upload".to_string()),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("type=\"file\""), "{out}");
        assert!(out.contains("accept=\"image/*\""), "{out}");
        assert!(out.contains("multiple"), "{out}");
        assert!(
            out.contains(
                "onchange={(event) => __crepusOwnHandler(handlers, \"upload\")?.upload?.(event)}"
            ),
            "{out}"
        );
    }

    #[test]
    fn emits_image() {
        let node = ViewNode::Image {
            src: "cat.png".to_string(),
            src_bind: None,
            alt: Some("A cat".to_string()),
            placeholder: None,
            on_long_press: Some("press".to_string()),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("src=\"cat.png\""), "{out}");
        assert!(
            out.contains(r#"data-crepus-on-long-press="press""#),
            "{out}"
        );
    }

    #[test]
    fn emits_link() {
        let node = ViewNode::Link {
            href: "/about".to_string(),
            href_bind: None,
            target: None,
            rel: None,
            style: None,
            children: vec![],
        };
        assert_eq!(emit_node(&node, 0, &[]), "<a href=\"/about\"></a>");
    }

    #[test]
    fn emits_web_view() {
        let node = ViewNode::WebView {
            src: "https://example.com".to_string(),
            style: None,
        };
        assert_eq!(
            emit_node(&node, 0, &[]),
            "<iframe src=\"https://example.com\" />"
        );
    }

    #[test]
    fn emits_scroll() {
        let node = ViewNode::Scroll {
            on_long_press: None,
            axis: crate::ir::StackAxis::Column,
            style: None,
            children: vec![],
        };
        assert_eq!(emit_node(&node, 0, &[]), "<div></div>");
    }

    #[test]
    fn emits_list() {
        let node = ViewNode::List {
            ordered: true,
            style: None,
            children: vec![],
        };
        assert_eq!(emit_node(&node, 0, &[]), "<ol></ol>");
    }

    #[test]
    fn emits_list_item() {
        let node = ViewNode::ListItem {
            on_long_press: Some("press".to_string()),
            style: None,
            children: vec![],
        };
        let out = emit_node(&node, 0, &[]);
        assert!(
            out.contains(r#"data-crepus-on-long-press="press""#),
            "{out}"
        );
    }

    #[test]
    fn emits_slot_rotate() {
        let node = ViewNode::SlotRotate {
            phrases: vec!["hi".to_string(), "bye".to_string()],
            interval_ms: 500,
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("data-crepus-slot-rotate=\"hi|bye\""), "{out}");
        assert!(out.contains(">hi</span>"), "{out}");
    }

    #[test]
    fn emits_input() {
        let node = ViewNode::Input {
            placeholder: "Name".to_string(),
            bind: "name".to_string(),
            multiline: false,
            secure: false,
            on_input: None,
            on_change: Some("nameChanged".to_string()),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("placeholder=\"Name\""), "{out}");
        assert!(
            out.contains("onchange={(event) => __crepusOwnHandler(handlers, \"nameChanged\")?.nameChanged?.(event)}"),
            "{out}"
        );
    }

    #[test]
    fn emits_picker() {
        let node = ViewNode::Picker {
            bind: "color".to_string(),
            options: vec![],
            on_change: Some("colorChanged".to_string()),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(
            out.contains("onchange={(event) => __crepusOwnHandler(handlers, \"colorChanged\")?.colorChanged?.(event)}"),
            "{out}"
        );
    }

    #[test]
    fn emits_tabs() {
        let node = ViewNode::Tabs {
            bind: "tab".to_string(),
            tabs: vec![crate::ir::TabItem {
                value: "one".to_string(),
                label: "One".to_string(),
                icon: None,
                children: vec![],
            }],
            on_change: Some("tabChanged".to_string()),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(
            out.contains("onclick={() => __crepusOwnHandler(handlers, \"tabChanged\")?.tabChanged?.(\"one\")}"),
            "{out}"
        );
        assert!(out.contains("__crepusTabsRef"), "{out}");
        assert!(out.contains("__crepusTabsKey"), "{out}");
        assert!(out.contains("aria-label=\"Tabs\""), "{out}");
        assert!(out.contains("tabindex=\"0\""), "{out}");
        assert!(out.contains("aria-selected={"), "{out}");
        assert!(out.contains(" tabindex={"), "{out}");
        assert!(out.contains(" hidden={!("), "{out}");
    }

    #[test]
    fn text_content_is_html_escaped() {
        let node = ViewNode::Text {
            content: "a < b && c > d {raw}".to_string(),
            bind: None,
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert_eq!(
            out,
            "<span>a &lt; b &amp;&amp; c &gt; d &#123;raw&#125;</span>"
        );
    }

    #[test]
    fn attribute_value_is_html_escaped() {
        let node = ViewNode::Link {
            href: "/a\"b{raw}".to_string(),
            href_bind: None,
            target: None,
            rel: None,
            style: None,
            children: vec![],
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("href=\"/a&quot;b&#123;raw&#125;\""), "{out}");
    }
    #[test]
    fn slider_bindings_keep_labels_continuous_steps_and_unbound_behavior() {
        let mut node = ViewNode::Slider {
            label: Some(r#"Level "quoted" &"#.into()),
            bind: Some("prefs.level".into()),
            value: 5.0,
            min: 0.0,
            max: 10.0,
            step: None,
            on_change: None,
            style: None,
        };
        let output = emit_node(&node, 0, &[]);
        assert!(output.contains("value={"), "{output}");
        assert!(output.contains("?.prefs?.level"), "{output}");
        assert!(
            output.contains("aria-label=\"Level &quot;quoted&quot; &amp;\""),
            "{output}"
        );
        assert!(output.contains("step=\"any\""), "{output}");
        if let ViewNode::Slider { bind, .. } = &mut node {
            *bind = Some(" ".into());
        }
        let output = emit_node(&node, 0, &[]);
        assert!(output.contains("value={5}"), "{output}");
    }
    #[test]
    fn progress_and_meter_normalize_invalid_numbers_and_escape_labels() {
        for node in [
            ViewNode::Progress {
                label: Some(r#"Status "quoted" &"#.into()),
                value: f32::NAN,
                max: f32::INFINITY,
                style: None,
            },
            ViewNode::Meter {
                label: Some(r#"Status "quoted" &"#.into()),
                value: f32::NAN,
                min: 10.0,
                max: -10.0,
                style: None,
            },
        ] {
            let output = emit_node(&node, 0, &[]);
            assert!(!output.contains("NaN"), "{output}");
            assert!(!output.contains("Infinity"), "{output}");
            assert!(
                output.contains(r#"aria-label="Status &quot;quoted&quot; &amp;""#),
                "{output}"
            );
        }
    }
    #[test]
    fn every_static_label_caller_preserves_literal_braces() {
        let literal = "{raw} & <tag>";
        for json in [
            serde_json::json!({"kind":"button", "label":literal}),
            serde_json::json!({"kind":"badge", "label":literal}),
            serde_json::json!({"kind":"checkbox", "label":literal, "checked":false}),
            serde_json::json!({"kind":"toggle", "label":literal, "checked":false}),
            serde_json::json!({"kind":"picker", "bind":"choice", "options":[{"value":"one", "label":literal}]}),
            serde_json::json!({"kind":"filePicker", "label":literal}),
            serde_json::json!({"kind":"slotRotate", "phrases":[literal], "intervalMs":1000}),
            serde_json::json!({"kind":"tabs", "bind":"tab", "tabs":[{"value":"one", "label":literal, "children":[]}]}),
        ] {
            let node: ViewNode = serde_json::from_value(json).unwrap();
            let output = emit_node(&node, 0, &[]);
            assert!(
                output.contains("&#123;raw&#125; &amp; &lt;tag&gt;"),
                "{output}"
            );
            assert!(!output.contains(">{raw}"), "{output}");
        }
        let text: ViewNode = serde_json::from_value(
            serde_json::json!({"kind":"text", "content":"snapshot", "bind":"row.label"}),
        )
        .unwrap();
        let button: ViewNode = serde_json::from_value(serde_json::json!({"kind":"button", "label":"snapshot", "labelParts":[{"kind":"expr", "value":"row.label"}]})).unwrap();
        for node in [text, button] {
            let output = emit_node(&node, 0, &[]);
            assert!(
                output
                    .replace("scope?.", "scope.")
                    .contains("{__crepusText(scope.row?.label)}"),
                "{output}"
            );
        }
    }
    #[test]
    fn whitespace_sensitive_text_uses_exact_javascript_string_values() {
        for value in [
            "  a  b\n\t c  ",
            "leading ",
            " trailing",
            "a  b",
            "a\nb",
            "a\tb",
            "a\u{a0}b",
            "  {raw} &amp; <tag> \"quoted\" \\path  ",
            " </script>{raw} ",
        ] {
            let output = html_text(value);
            let literal = output.strip_prefix('{').unwrap().strip_suffix('}').unwrap();
            let decoded: String = serde_json::from_str(literal).unwrap();
            assert_eq!(decoded, value);
            assert!(!literal.contains('<'), "{output}");
            let node = ViewNode::Text {
                content: value.into(),
                bind: None,
                style: None,
            };
            assert_eq!(emit_node(&node, 0, &[]), format!("<span>{output}</span>"));
        }
        assert_eq!(html_text("normal {raw}"), "normal &#123;raw&#125;");
    }
    #[test]
    fn control_label_formatting_does_not_add_text_whitespace() {
        let label = "  a  b\n\t c  ";
        for json in [
            serde_json::json!({"kind":"checkbox", "label":label, "checked":false}),
            serde_json::json!({"kind":"filePicker", "label":label}),
        ] {
            let node: ViewNode = serde_json::from_value(json).unwrap();
            let output = emit_node(&node, 2, &[]);
            assert!(
                output.contains(&format!(" />{}</label>", html_text(label))),
                "{output}"
            );
            assert!(
                !output.contains('\n'),
                "formatting must not add label text: {output}"
            );
        }
    }
    #[test]
    fn text_containers_preserve_allowlisted_source_tags_only() {
        for (tag, expected) in [("h2", "h2"), ("label", "label"), ("script", "span")] {
            let node: ViewNode = serde_json::from_value(
                serde_json::json!({"kind":"text", "content":"Literal", "style":{"htmlTag":tag}}),
            )
            .unwrap();
            let output = emit_node(&node, 0, &[]);
            assert_eq!(output, format!("<{expected}>Literal</{expected}>"));
        }
        for tag in ["h1", "p"] {
            for kind in ["stack", "scroll"] {
                let node: ViewNode = serde_json::from_value(serde_json::json!({"kind":kind, "axis":"column", "style":{"htmlTag":tag}, "children":[]})).unwrap();
                assert_eq!(emit_node(&node, 0, &[]), format!("<{tag}></{tag}>"));
            }
        }
        for kind in ["stack", "scroll"] {
            let node: ViewNode = serde_json::from_value(serde_json::json!({"kind":kind, "axis":"column", "style":{"htmlTag":"h3"}, "children":[{"kind":"text", "content":"Child"}]})).unwrap();
            let output = emit_node(&node, 0, &[]);
            assert!(output.starts_with("<h3>"), "{output}");
            assert!(output.ends_with("</h3>"), "{output}");
            assert!(output.contains("<span>Child</span>"), "{output}");
        }
    }
}

#[cfg(test)]
mod display_contents_tests {
    use super::*;

    #[test]
    fn private_contents_style_requires_explicit_true_and_preserves_classes() {
        for enabled in [None, Some(false), Some(true)] {
            let style = ViewStyle {
                classes: vec!["retained".into()],
                html_display_contents: enabled,
                ..Default::default()
            };
            let attrs = class_attr(Some(&style));
            assert!(attrs.contains("retained"));
            assert_eq!(
                attrs.contains(" style=\"display: contents;\""),
                enabled == Some(true)
            );
        }
        assert!(!class_attr(None).contains("display"));
    }
    #[test]
    fn bound_urls_use_scope_and_loop_locals() {
        let nodes = vec![
            ViewNode::Image {
                src: "snapshot.png".into(),
                src_bind: Some("photo.url".into()),
                alt: None,
                placeholder: None,
                on_long_press: None,
                style: None,
            },
            ViewNode::Link {
                href: "/snapshot".into(),
                href_bind: Some("item.url".into()),
                target: None,
                rel: None,
                style: None,
                children: vec![],
            },
        ];
        let root = emit_node(&nodes[0], 0, &[]);
        assert!(root.contains("scope.photo?.url"), "{root}");
        assert!(!root.contains("snapshot.png"), "{root}");
        let root_link = emit_node(&nodes[1], 0, &[]);
        assert!(root_link.contains("scope.item?.url"), "{root_link}");
        let mut nodes = nodes;
        if let ViewNode::Image { src_bind, .. } = &mut nodes[0] {
            *src_bind = Some("item.image".into());
        }
        let loop_node = ViewNode::ForEach {
            bind: "items".into(),
            item_name: "item".into(),
            item_body: nodes,
            style: None,
        };
        let output = emit_node(&loop_node, 0, &[]);
        assert!(output.contains("_crepusItem0?.url"), "{output}");
        assert!(output.contains("_crepusItem0?.image"), "{output}");
        assert!(!output.contains("scope.item?.url"), "{output}");
        assert!(!output.contains("/snapshot"), "{output}");
    }
}
