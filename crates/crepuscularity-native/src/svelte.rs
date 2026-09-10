//! Svelte 5 single-file-component emitter, structured after [`crate::moonshine`]'s
//! JSX emitter but targeting Svelte 5 runes/markup instead of React/JSX.

use crate::ir::{ViewIr, ViewNode, ViewStyle};

/// Emit a complete Svelte 5 `.svelte` single-file component from `ir`.
///
/// The component takes `scope` and `handlers` as runes props (`$props()`),
/// mirroring the `scope`/`handlers` contract used by the other emitters.
pub fn emit_svelte_component(ir: &ViewIr) -> String {
    let locals: Vec<String> = Vec::new();
    let markup: String = ir
        .root
        .iter()
        .map(|n| emit_node(n, 0, &locals))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"<script lang="ts">
  let {{ scope = {{}}, handlers = {{}} }}: {{ scope?: Record<string, any>; handlers?: Record<string, any> }} = $props();

  function toArray(v: unknown): any[] {{
    return Array.isArray(v) ? v : [];
  }}
</script>

{markup}
"#
    )
}

/// HTML-escape text content: `&`, `<`, `>`.
fn html_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

/// HTML-escape an attribute value: `&`, `<`, `>`, `"`.
fn html_attr_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// A static `name="value"` attribute, HTML-escaped.
fn attr(name: &str, value: &str) -> String {
    format!(" {name}=\"{}\"", html_attr_value(value))
}

fn opt_attr(name: &str, value: Option<&String>) -> String {
    match value {
        Some(v) => attr(name, v),
        None => String::new(),
    }
}

/// `class="..."` from the class tokens the parser preserved on the node.
fn class_attr(style: Option<&ViewStyle>) -> String {
    let classes = match style {
        Some(s) if !s.classes.is_empty() => s.classes.join(" "),
        _ => return String::new(),
    };
    attr("class", &classes)
}

/// Whether `c` can start a JS identifier.
fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

/// Whether `c` can continue a JS identifier.
fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// Rewrite a template expression string into valid JS reading from `scope`.
/// Ported from [`crate::moonshine::scope_expr`]: bare identifiers are
/// prefixed with `scope.` except those in `locals` (enclosing `ForEach` item
/// names) and JS literal keywords; only the first segment of a dotted path is
/// prefixed; string literals are untouched; empty becomes `undefined`.
fn scope_expr(expr: &str, locals: &[String]) -> String {
    if expr.trim().is_empty() {
        return "undefined".to_string();
    }
    let chars: Vec<char> = expr.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' || c == '"' {
            let quote = c;
            out.push(c);
            i += 1;
            while i < chars.len() {
                let cc = chars[i];
                out.push(cc);
                i += 1;
                if cc == '\\' && i < chars.len() {
                    out.push(chars[i]);
                    i += 1;
                    continue;
                }
                if cc == quote {
                    break;
                }
            }
            continue;
        }
        if is_ident_start(c) {
            let prev_is_dot = out.trim_end().ends_with('.');
            let start = i;
            while i < chars.len() && is_ident_continue(chars[i]) {
                i += 1;
            }
            let ident: String = chars[start..i].iter().collect();
            if prev_is_dot
                || locals.iter().any(|l| l == &ident)
                || matches!(ident.as_str(), "true" | "false" | "null" | "undefined")
            {
                out.push_str(&ident);
            } else {
                out.push_str("scope.");
                out.push_str(&ident);
            }
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// A text child: literal (HTML-escaped) unless bound, in which case `{expr}`.
fn text_child(content: &str, bind: Option<&String>, locals: &[String]) -> String {
    match bind {
        Some(b) => format!("{{{}}}", scope_expr(b, locals)),
        None => html_text(content),
    }
}

fn on_click_attr(name: &str) -> String {
    format!(" onclick={{() => handlers.{name}?.()}}")
}

fn on_click_arg_attr(name: &str, arg: &str) -> String {
    format!(" onclick={{() => handlers.{name}?.({arg})}}")
}

fn dom_event_attr(svelte_event: &str, name: &str) -> String {
    format!(" {svelte_event}={{(event) => handlers.{name}?.(event)}}")
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

fn emit_children(children: &[ViewNode], indent: usize, locals: &[String]) -> String {
    children
        .iter()
        .map(|c| emit_node(c, indent, locals))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Wrap `children` in `tag`, or self-close when there are none.
fn element(
    tag: &str,
    attrs: &str,
    children: &[ViewNode],
    indent: usize,
    locals: &[String],
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
    locals: &[String],
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
    locals: &[String],
) -> String {
    let pad = "  ".repeat(indent);
    let bind_expr = scope_expr(bind, locals);
    let mut inner_locals = locals.to_vec();
    inner_locals.push(item_name.to_string());
    let inner = emit_children(item_body, indent + 1, &inner_locals);
    format!(
        "{pad}{{#each toArray({bind_expr}) as {item_name}, index (index)}}\n{inner}\n{pad}{{/each}}"
    )
}

fn emit_node(node: &ViewNode, indent: usize, locals: &[String]) -> String {
    let pad = "  ".repeat(indent);
    match node {
        ViewNode::Text { .. } => emit_text(node, &pad, indent, locals),
        ViewNode::Link { .. } => emit_link(node, &pad, indent, locals),
        ViewNode::Stack { .. } => emit_stack(node, &pad, indent, locals),
        ViewNode::Scroll { .. } => emit_scroll(node, &pad, indent, locals),
        ViewNode::Dropzone { .. } => emit_dropzone(node, &pad, indent, locals),
        ViewNode::List { .. } => emit_list(node, &pad, indent, locals),
        ViewNode::ListItem { .. } => emit_list_item(node, &pad, indent, locals),
        ViewNode::Button { .. } => emit_button(node, &pad, indent, locals),
        ViewNode::Badge { .. } => emit_badge(node, &pad, indent, locals),
        ViewNode::Divider { .. } => emit_divider(node, &pad, indent, locals),
        ViewNode::Spacer { .. } => emit_spacer(node, &pad, indent, locals),
        ViewNode::Image { .. } => emit_image(node, &pad, indent, locals),
        ViewNode::WebView { .. } => emit_web_view(node, &pad, indent, locals),
        ViewNode::Toggle { .. } => emit_toggle(node, &pad, indent, locals),
        ViewNode::Checkbox { .. } => emit_checkbox(node, &pad, indent, locals),
        ViewNode::Slider { .. } => emit_slider(node, &pad, indent, locals),
        ViewNode::Progress { .. } => emit_progress(node, &pad, indent, locals),
        ViewNode::Meter { .. } => emit_meter(node, &pad, indent, locals),
        ViewNode::Input { .. } => emit_input(node, &pad, indent, locals),
        ViewNode::Picker { .. } => emit_picker(node, &pad, indent, locals),
        ViewNode::FilePicker { .. } => emit_file_picker(node, &pad, indent, locals),
        ViewNode::SlotRotate { .. } => emit_slot_rotate(node, &pad, indent, locals),
        ViewNode::Tabs { .. } => emit_tabs(node, &pad, indent, locals),
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

fn emit_text(node: &ViewNode, pad: &str, _indent: usize, locals: &[String]) -> String {
    let ViewNode::Text {
        content,
        bind,
        style,
    } = node
    else {
        unreachable!()
    };
    format!(
        "{pad}<span{}>{}</span>",
        class_attr(style.as_ref()),
        text_child(content, bind.as_ref(), locals)
    )
}

fn emit_link(node: &ViewNode, _pad: &str, indent: usize, locals: &[String]) -> String {
    let ViewNode::Link {
        href,
        target,
        rel,
        style,
        children,
    } = node
    else {
        unreachable!()
    };
    {
        let attrs = format!(
            "{}{}{}{}",
            attr("href", href),
            opt_attr("target", target.as_ref()),
            opt_attr("rel", rel.as_ref()),
            class_attr(style.as_ref())
        );
        element("a", &attrs, children, indent, locals)
    }
}

fn emit_stack(node: &ViewNode, _pad: &str, indent: usize, locals: &[String]) -> String {
    let ViewNode::Stack {
        on_long_press,
        style,
        children,
        ..
    } = node
    else {
        unreachable!()
    };
    {
        let attrs = format!(
            "{}{}",
            class_attr(style.as_ref()),
            opt_on_long_press(on_long_press.as_ref())
        );
        element("div", &attrs, children, indent, locals)
    }
}

fn emit_scroll(node: &ViewNode, _pad: &str, indent: usize, locals: &[String]) -> String {
    let ViewNode::Scroll {
        style, children, ..
    } = node
    else {
        unreachable!()
    };
    {
        element("div", &class_attr(style.as_ref()), children, indent, locals)
    }
}

fn emit_dropzone(node: &ViewNode, _pad: &str, indent: usize, locals: &[String]) -> String {
    let ViewNode::Dropzone {
        on_drop,
        style,
        children,
        ..
    } = node
    else {
        unreachable!()
    };
    {
        let attrs = format!(
            "{}{}",
            class_attr(style.as_ref()),
            opt_dom_event("ondrop", on_drop.as_ref())
        );
        element("div", &attrs, children, indent, locals)
    }
}

fn emit_list(node: &ViewNode, _pad: &str, indent: usize, locals: &[String]) -> String {
    let ViewNode::List {
        ordered,
        style,
        children,
    } = node
    else {
        unreachable!()
    };
    element(
        if *ordered { "ol" } else { "ul" },
        &class_attr(style.as_ref()),
        children,
        indent,
        locals,
    )
}

fn emit_list_item(node: &ViewNode, _pad: &str, indent: usize, locals: &[String]) -> String {
    let ViewNode::ListItem {
        on_long_press,
        style,
        children,
    } = node
    else {
        unreachable!()
    };
    {
        let attrs = format!(
            "{}{}",
            class_attr(style.as_ref()),
            opt_on_long_press(on_long_press.as_ref())
        );
        element("li", &attrs, children, indent, locals)
    }
}

fn emit_button(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Button {
        label,
        on_click,
        on_long_press,
        style,
    } = node
    else {
        unreachable!()
    };
    format!(
        "{pad}<button type=\"button\"{}{}{}>{}</button>",
        class_attr(style.as_ref()),
        opt_on_click(on_click.as_ref()),
        opt_on_long_press(on_long_press.as_ref()),
        html_text(label)
    )
}

fn emit_badge(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Badge {
        label, tone, style, ..
    } = node
    else {
        unreachable!()
    };
    format!(
        "{pad}<span{}{}>{}</span>",
        class_attr(style.as_ref()),
        opt_attr("data-tone", tone.as_ref()),
        html_text(label)
    )
}

fn emit_divider(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Divider { style, .. } = node else {
        unreachable!()
    };
    {
        format!("{pad}<hr{} />", class_attr(style.as_ref()))
    }
}

fn emit_spacer(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Spacer { style, .. } = node else {
        unreachable!()
    };
    {
        format!(
            "{pad}<div aria-hidden=\"true\"{} />",
            class_attr(style.as_ref())
        )
    }
}

fn emit_image(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Image {
        src,
        alt,
        on_long_press,
        style,
        ..
    } = node
    else {
        unreachable!()
    };
    format!(
        "{pad}<img{}{}{}{} />",
        attr("src", src),
        attr("alt", alt.as_deref().unwrap_or("")),
        class_attr(style.as_ref()),
        opt_on_long_press(on_long_press.as_ref())
    )
}

fn emit_web_view(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::WebView { src, style } = node else {
        unreachable!()
    };
    format!(
        "{pad}<iframe{}{} />",
        attr("src", src),
        class_attr(style.as_ref())
    )
}

fn emit_toggle(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Toggle {
        label,
        checked,
        on_change,
        on_long_press,
        style,
        ..
    } = node
    else {
        unreachable!()
    };
    format!(
        "{pad}<button type=\"button\" role=\"switch\" aria-checked={{{}}}{}{}{}>{}</button>",
        checked,
        class_attr(style.as_ref()),
        opt_dom_event("onchange", on_change.as_ref()),
        opt_on_long_press(on_long_press.as_ref()),
        html_text(label)
    )
}

fn emit_checkbox(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Checkbox {
        label,
        checked,
        on_change,
        style,
        ..
    } = node
    else {
        unreachable!()
    };
    format!(
            "{pad}<label{}>\n{pad}  <input type=\"checkbox\" checked={{{}}}{} />\n{pad}  {}\n{pad}</label>",
            class_attr(style.as_ref()),
            checked,
            opt_dom_event("onchange", on_change.as_ref()),
            html_text(label)
        )
}

fn emit_slider(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Slider {
        value,
        min,
        max,
        step,
        on_change,
        style,
        ..
    } = node
    else {
        unreachable!()
    };
    format!(
        "{pad}<input type=\"range\" value={{{value}}} min={{{min}}} max={{{max}}}{}{}{} />",
        step.map(|s| format!(" step={{{s}}}")).unwrap_or_default(),
        class_attr(style.as_ref()),
        opt_dom_event("onchange", on_change.as_ref())
    )
}

fn emit_progress(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Progress {
        value, max, style, ..
    } = node
    else {
        unreachable!()
    };
    format!(
        "{pad}<progress value={{{value}}} max={{{max}}}{} />",
        class_attr(style.as_ref())
    )
}

fn emit_meter(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Meter {
        value,
        min,
        max,
        style,
        ..
    } = node
    else {
        unreachable!()
    };
    format!(
        "{pad}<meter value={{{value}}} min={{{min}}} max={{{max}}}{} />",
        class_attr(style.as_ref())
    )
}

fn emit_input(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Input {
        placeholder,
        bind,
        secure,
        multiline,
        on_change,
        style,
    } = node
    else {
        unreachable!()
    };
    {
        let cls = class_attr(style.as_ref());
        let ph = attr("placeholder", placeholder);
        let name = attr("name", bind);
        let on_change_attr = opt_dom_event("onchange", on_change.as_ref());
        if *multiline {
            format!("{pad}<textarea{ph}{name}{cls}{on_change_attr}></textarea>")
        } else {
            let ty = if *secure { "password" } else { "text" };
            format!("{pad}<input type=\"{ty}\"{ph}{name}{cls}{on_change_attr} />")
        }
    }
}

fn emit_picker(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::Picker {
        options,
        bind,
        on_change,
        style,
    } = node
    else {
        unreachable!()
    };
    {
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
            "{pad}<select{}{}{}>\n{opts}\n{pad}</select>",
            attr("name", bind),
            class_attr(style.as_ref()),
            opt_dom_event("onchange", on_change.as_ref())
        )
    }
}

fn emit_file_picker(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::FilePicker {
        label,
        accept,
        multiple,
        on_pick,
        style,
    } = node
    else {
        unreachable!()
    };
    {
        let accept_attr = if accept.is_empty() {
            String::new()
        } else {
            attr("accept", &accept.join(","))
        };
        let multiple_attr = if *multiple { " multiple" } else { "" };
        format!(
                "{pad}<label{}>\n{pad}  <input type=\"file\"{accept_attr}{multiple_attr}{} />\n{pad}  {}\n{pad}</label>",
                class_attr(style.as_ref()),
                opt_dom_event("onchange", on_pick.as_ref()),
                html_text(label)
            )
    }
}

fn emit_slot_rotate(node: &ViewNode, pad: &str, _indent: usize, _locals: &[String]) -> String {
    let ViewNode::SlotRotate {
        phrases,
        interval_ms,
        style,
    } = node
    else {
        unreachable!()
    };
    format!(
        "{pad}<span{} data-interval-ms={{{interval_ms}}}{}>{}</span>",
        attr("data-crepus-slot-rotate", &phrases.join("|")),
        class_attr(style.as_ref()),
        html_text(phrases.first().map(String::as_str).unwrap_or(""))
    )
}

fn emit_tabs(node: &ViewNode, pad: &str, indent: usize, locals: &[String]) -> String {
    let ViewNode::Tabs {
        tabs,
        on_change,
        style,
        ..
    } = node
    else {
        unreachable!()
    };
    {
        let cls = class_attr(style.as_ref());
        let buttons: String = tabs
            .iter()
            .map(|t| {
                let onclick = match on_change {
                    Some(name) => on_click_arg_attr(name, &format!("\"{}\"", t.value)),
                    None => String::new(),
                };
                format!(
                    "{pad}    <button type=\"button\" role=\"tab\"{onclick}>{}</button>",
                    html_text(&t.label)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let panels: String = tabs
            .iter()
            .map(|t| {
                format!(
                    "{pad}  <div role=\"tabpanel\">\n{}\n{pad}  </div>",
                    emit_children(&t.children, indent + 2, locals)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
                "{pad}<div{cls}>\n{pad}  <div role=\"tablist\">\n{buttons}\n{pad}  </div>\n{panels}\n{pad}</div>"
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::render_template_to_ir;
    use crepuscularity_core::context::TemplateContext;

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
            out.starts_with("{#each toArray(scope.items) as item, index (index)}"),
            "{out}"
        );
        assert!(out.contains("{item.name}"), "{out}");
        assert!(!out.contains("scope.item.name"), "{out}");
        assert!(out.trim_end().ends_with("{/each}"), "{out}");
    }

    #[test]
    fn button_click_wired_to_handlers() {
        let node = ViewNode::Button {
            label: "Go".to_string(),
            on_click: Some("go".to_string()),
            on_long_press: None,
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("onclick={() => handlers.go?.()}"), "{out}");
    }

    #[test]
    fn text_with_bind_renders_expression_not_literal() {
        let node = ViewNode::Text {
            content: "placeholder".to_string(),
            bind: Some("user.name".to_string()),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert_eq!(out, "<span>{scope.user.name}</span>");
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
            out.contains("onchange={(event) => handlers.toggle?.(event)}"),
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
            out.contains("onchange={(event) => handlers.agree?.(event)}"),
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
            out.contains("onchange={(event) => handlers.vol?.(event)}"),
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
        assert_eq!(emit_node(&node, 0, &[]), "<progress value={3} max={10} />");
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
            "<meter value={3} min={0} max={10} />"
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
            out.contains("ondrop={(event) => handlers.drop?.(event)}"),
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
            out.contains("onchange={(event) => handlers.upload?.(event)}"),
            "{out}"
        );
    }

    #[test]
    fn emits_image() {
        let node = ViewNode::Image {
            src: "cat.png".to_string(),
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
            on_change: Some("nameChanged".to_string()),
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("placeholder=\"Name\""), "{out}");
        assert!(
            out.contains("onchange={(event) => handlers.nameChanged?.(event)}"),
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
            out.contains("onchange={(event) => handlers.colorChanged?.(event)}"),
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
            out.contains("onclick={() => handlers.tabChanged?.(\"one\")}"),
            "{out}"
        );
    }

    #[test]
    fn text_content_is_html_escaped() {
        let node = ViewNode::Text {
            content: "a < b && c > d".to_string(),
            bind: None,
            style: None,
        };
        let out = emit_node(&node, 0, &[]);
        assert_eq!(out, "<span>a &lt; b &amp;&amp; c &gt; d</span>");
    }

    #[test]
    fn attribute_value_is_html_escaped() {
        let node = ViewNode::Link {
            href: "/a\"b".to_string(),
            target: None,
            rel: None,
            style: None,
            children: vec![],
        };
        let out = emit_node(&node, 0, &[]);
        assert!(out.contains("href=\"/a&quot;b\""), "{out}");
    }
}
