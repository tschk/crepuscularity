use crate::ir::{StackAxis, ViewIr, ViewNode, ViewStyle};

use super::shared::{
    bool_literal, finite_opt, indent_str, nonnegative_opt, text_line_limit,
    vertical_divider_height, ScopeBinding, SWIFT,
};

pub(super) fn generate_swiftui(ir: &ViewIr, view_name: &str) -> String {
    let body = swiftui_nodes(&ir.root, 2, &[]);
    let webview = "import WebKit\n#if canImport(UIKit)\npublic struct CrepusWebView: UIViewRepresentable {\n    public let source: String\n\n    public func makeUIView(context: Context) -> WKWebView {\n        WKWebView()\n    }\n\n    public func updateUIView(_ webView: WKWebView, context: Context) {\n        guard webView.url?.absoluteString != source, let url = URL(string: source) else { return }\n        webView.load(URLRequest(url: url))\n    }\n}\n#elseif canImport(AppKit)\npublic struct CrepusWebView: NSViewRepresentable {\n    public let source: String\n\n    public func makeNSView(context: Context) -> WKWebView {\n        WKWebView()\n    }\n\n    public func updateNSView(_ webView: WKWebView, context: Context) {\n        guard webView.url?.absoluteString != source, let url = URL(string: source) else { return }\n        webView.load(URLRequest(url: url))\n    }\n}\n#endif\n\n";
    format!(
        "import Foundation\nimport SwiftUI\n{webview}#if canImport(UIKit)\nimport UIKit\n#endif\n#if canImport(AppKit)\nimport AppKit\n#endif\n\n@MainActor\npublic enum CrepusActions {{\n    public static let model = CrepusStateStore.shared\n    public static var dispatch: (String) -> String = {{ _ in \"{{}}\" }}\n    public static var resultSink: (String) -> Void = {{ _ in }}\n\n    public static func applyResult(_ json: String) {{\n        model.applyResult(json)\n    }}\n\n    private static let actionQueue = DispatchQueue(label: \"dev.crepuscularity.native.actions\", qos: .userInitiated)\n\n    public static func perform(_ action: String) {{\n        let dispatch = dispatch\n        let resultSink = resultSink\n        actionQueue.async {{\n            let result = dispatch(action)\n            DispatchQueue.main.sync {{\n                resultSink(result)\n            }}\n        }}\n    }}\n\n    // Drafts keep controls responsive; the captured sink owns authoritative commits.\n    public static func performChange(_ action: String?, bind: String, value: Any) {{\n        let resultSink = resultSink\n        guard let valueJson = CrepusRustActions.encodeQueuedChangeValue(value),\n              let sequence = model.beginDraft(path: bind, valueJson: valueJson) else {{\n            resultSink(\"{{\\\"ok\\\":false,\\\"error\\\":\\\"json encode failure\\\"}}\")\n            return\n        }}\n        let action = action ?? \"\"\n        actionQueue.async {{\n            let result = CrepusRustActions.dispatchQueuedChange(action, bind: bind, valueJson: valueJson)\n            DispatchQueue.main.sync {{\n                defer {{ model.finishDraft(path: bind, sequence: sequence) }}\n                resultSink(result)\n            }}\n        }}\n    }}\n\n    public static func dismissKeyboard() {{\n#if canImport(UIKit)\n        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)\n#elseif canImport(AppKit)\n        NSApp.keyWindow?.makeFirstResponder(nil)\n#endif\n    }}\n}}\n\n@MainActor\npublic struct {view_name}: View {{\n    @ObservedObject private var model = CrepusActions.model\n\n    public init() {{}}\n\n    public var body: some View {{\n{body}\n        .contentShape(Rectangle())\n        .onTapGesture {{ CrepusActions.dismissKeyboard() }}\n    }}\n}}\n"
    )
}

fn swiftui_nodes(nodes: &[ViewNode], indent: usize, scopes: &[ScopeBinding]) -> String {
    if nodes.len() == 1 {
        swiftui_node(&nodes[0], indent, scopes)
    } else {
        let pad = indent_str(indent);
        let inner = nodes
            .iter()
            .map(|node| swiftui_node(node, indent + 1, scopes))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{pad}Group {{\n{inner}\n{pad}}}")
    }
}

fn swiftui_stack_children(
    _axis: StackAxis,
    justify_content: Option<&str>,
    children: &[ViewNode],
    indent: usize,
    scopes: &[ScopeBinding],
) -> String {
    let child_lines = swiftui_children(children, indent, scopes);
    let spacer = format!("{}Spacer()", indent_str(indent));
    match justify_content {
        Some("end") => format!("{spacer}\n{child_lines}"),
        Some("between") if children.len() > 1 => children
            .iter()
            .map(|child| swiftui_node(child, indent, scopes))
            .collect::<Vec<_>>()
            .join(&format!("\n{spacer}\n")),
        _ => child_lines,
    }
}

fn swiftui_node(node: &ViewNode, indent: usize, scopes: &[ScopeBinding]) -> String {
    let pad = indent_str(indent);
    let mut out = match node {
        ViewNode::Text {
            content,
            bind,
            style,
            ..
        } => {
            let text = SWIFT.text_content(content, bind.as_deref(), scopes);
            let mut out = format!("{pad}Text({text})");
            swiftui_style(&mut out, style.as_ref(), true, indent);
            out
        }
        ViewNode::If {
            condition,
            then_children,
            else_children,
            ..
        } => {
            let then_inner = swiftui_children(then_children, indent + 1, scopes);
            let condition = SWIFT.model_bool(condition, scopes);
            if let Some(else_children) = else_children {
                let else_inner = swiftui_children(else_children, indent + 1, scopes);
                format!(
                    "{pad}if {condition} {{\n{then_inner}\n{pad}}} else {{\n{else_inner}\n{pad}}}"
                )
            } else {
                format!("{pad}if {condition} {{\n{then_inner}\n{pad}}}")
            }
        }
        ViewNode::ForEach {
            bind,
            item_name,
            item_body,
            ..
        } => {
            let binding = SWIFT.loop_binding(item_name, scopes);
            let item_var = &binding.variable;
            let index_var =
                SWIFT.identifier(&format!("{}__crepusIndex", item_var.trim_matches('`')));
            let items = SWIFT.model_items(bind, scopes);
            let path_var = SWIFT.binding_path_var(item_var);
            let path = SWIFT.binding_path(bind, scopes);
            let mut child_scopes = scopes.to_vec();
            child_scopes.push(binding.clone());
            let inner = swiftui_children(item_body, indent + 1, &child_scopes);
            format!(
                "{pad}ForEach(Array({items}.enumerated()), id: \\.offset) {{ {index_var}, {item_var} in\n{}let {path_var} = {path} + \".\" + String({index_var})\n{inner}\n{pad}}}", indent_str(indent + 1)
            )
        }
        ViewNode::Stack {
            axis,
            spacing,
            align_items,
            justify_content,
            style,
            children,
            ..
        } => {
            let align = swiftui_stack_alignment(*axis, align_items.as_deref());
            let gap = finite_opt(*spacing).unwrap_or(8.0);
            let inner = swiftui_stack_children(
                *axis,
                justify_content.as_deref(),
                children,
                indent + 1,
                scopes,
            );
            let mut out = if *axis == StackAxis::Row
                && style.as_ref().and_then(|style| style.flex_wrap) == Some(true)
            {
                let grid_align = swiftui_grid_alignment(align_items.as_deref());
                format!(
                    "{pad}LazyVGrid(columns: [GridItem(.adaptive(minimum: 72), spacing: {gap:.1})], alignment: {grid_align}, spacing: {gap:.1}) {{\n{inner}\n{pad}}}"
                )
            } else {
                let view = match axis {
                    StackAxis::Row => "HStack",
                    StackAxis::Column => "VStack",
                };
                format!("{pad}{view}(alignment: {align}, spacing: {gap:.1}) {{\n{inner}\n{pad}}}")
            };
            swiftui_style_with_alignment(
                &mut out,
                style.as_ref(),
                false,
                indent,
                swiftui_frame_alignment(*axis, justify_content.as_deref(), align_items.as_deref()),
            );
            out
        }
        ViewNode::Button {
            label,
            label_parts,
            on_click,
            style,
            ..
        } => {
            let action = swiftui_action(on_click.as_deref());
            let mut out = format!(
                "{pad}Button(action: {{ {action} }}) {{\n{}Text({})\n{pad}}}",
                indent_str(indent + 1),
                SWIFT.label_text(label, label_parts, scopes)
            );
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Toggle {
            label,
            bind,
            checked,
            on_change,
            style,
            ..
        }
        | ViewNode::Checkbox {
            label,
            bind,
            checked,
            on_change,
            style,
            ..
        } => {
            let binding = bind
                .as_deref()
                .map(|bind| {
                    format!(
                        "Binding(get: {{ {} }}, set: {{ newValue in {} }})",
                        swiftui_draft_getter("Bool", bind, scopes),
                        swiftui_change(on_change.as_deref(), bind, "newValue", scopes)
                    )
                })
                .unwrap_or_else(|| {
                    if on_change.is_some() {
                        format!(
                            "Binding(get: {{ {} }}, set: {{ _ in {} }})",
                            bool_literal(*checked),
                            swiftui_action(on_change.as_deref())
                        )
                    } else {
                        format!(".constant({})", bool_literal(*checked))
                    }
                });
            let mut out = format!("{pad}Toggle(\"{}\", isOn: {binding})", SWIFT.escape(label));
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
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
            ..
        } => {
            let (value, min, max, step) = super::shared::slider_values(*value, *min, *max, *step);
            let step_arg = step.map(|v| format!(", step: {v:?}")).unwrap_or_default();
            let slider_value = bind
                .as_deref()
                .map(|bind| {
                    format!(
                        "Binding(get: {{ Swift.min(Swift.max({}, {min:?}), {max:?}) }}, set: {{ newValue in {} }})",
                        swiftui_draft_getter("Number", bind, scopes),
                        swiftui_change(on_change.as_deref(), bind, "newValue", scopes)
                    )
                })
                .unwrap_or_else(|| {
                    if on_change.is_some() {
                        format!(
                            "Binding(get: {{ {value:?} }}, set: {{ _ in {} }})",
                            swiftui_action(on_change.as_deref())
                        )
                    } else {
                        format!(".constant({value:?})")
                    }
                });
            let control = format!("Slider(value: {slider_value}, in: {min:?}...{max:?}{step_arg})");
            let mut out = if let Some(label) = label {
                format!(
                    "{pad}VStack(alignment: .leading, spacing: 8.0) {{\n{}Text(\"{}\")\n{}{control}\n{pad}}}",
                    indent_str(indent + 1),
                    SWIFT.escape(label),
                    indent_str(indent + 1)
                )
            } else {
                format!("{pad}{control}")
            };
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Progress {
            label,
            value,
            max,
            style,
        } => {
            let (value, max) = super::shared::progress_values(*value, *max);
            let mut out = if let Some(label) = label {
                format!(
                    "{pad}ProgressView(\"{}\", value: {value:?}, total: {max:?})",
                    SWIFT.escape(label)
                )
            } else {
                format!("{pad}ProgressView(value: {value:?}, total: {max:?})")
            };
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Meter {
            label,
            value,
            min,
            max,
            style,
        } => {
            let progress = ViewNode::Progress {
                label: label.clone(),
                value: super::shared::meter_fraction(*value, *min, *max),
                max: 1.0,
                style: style.clone(),
            };
            swiftui_node(&progress, indent, scopes)
        }
        ViewNode::Badge { label, style, .. } => {
            let mut out = format!("{pad}Text(\"{}\")", SWIFT.escape(label));
            swiftui_style(&mut out, style.as_ref(), true, indent);
            out
        }
        ViewNode::Divider { axis, style } => {
            let mut out = match axis {
                StackAxis::Row => {
                    let height = vertical_divider_height(
                        style.as_ref().and_then(|style| style.height),
                        style.as_ref().and_then(|style| style.max_height),
                    )
                    .map(|height| format!("{height:?}"))
                    .unwrap_or_else(|| "nil".into());
                    format!("{pad}HStack(spacing: 0) {{ Divider() }}\n{pad}.frame(width: 1, height: {height})")
                }
                StackAxis::Column => format!("{pad}VStack(spacing: 0) {{ Divider() }}"),
            };
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Spacer { size, style } => {
            let mut out = nonnegative_opt(*size)
                .map(|v| format!("{pad}Spacer(minLength: {v:?})"))
                .unwrap_or_else(|| format!("{pad}Spacer()"));
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Dropzone {
            label,
            style,
            children,
            ..
        } => {
            let inner = if children.is_empty() {
                format!(
                    "{}Text(\"{}\")",
                    indent_str(indent + 1),
                    SWIFT.escape(label)
                )
            } else {
                swiftui_children(children, indent + 1, scopes)
            };
            let mut out =
                format!("{pad}VStack(alignment: .leading, spacing: 8.0) {{\n{inner}\n{pad}}}");
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::FilePicker {
            label,
            on_pick,
            style,
            ..
        } => {
            let action = swiftui_action(on_pick.as_deref());
            let mut out = format!(
                "{pad}Button(action: {{ {action} }}) {{\n{}Text(\"{}\")\n{pad}}}",
                indent_str(indent + 1),
                SWIFT.escape(label)
            );
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Image {
            src,
            src_bind,
            alt,
            style,
            ..
        } => {
            let label = alt.as_deref().unwrap_or(src);
            let value = if alt.is_none() {
                src_bind.as_ref().map(|bind| SWIFT.model_text(bind, scopes))
            } else {
                None
            }
            .unwrap_or_else(|| format!("\"{}\"", SWIFT.escape(label)));
            let mut out = format!("{pad}Text({value})");
            swiftui_style(&mut out, style.as_ref(), true, indent);
            out
        }
        ViewNode::WebView { src, style } => {
            let mut out = format!("{pad}CrepusWebView(source: \"{}\")", SWIFT.escape(src));
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Scroll {
            axis,
            style,
            children,
            ..
        } => {
            let scroll_axis = match axis {
                StackAxis::Row => ".horizontal",
                StackAxis::Column => ".vertical",
            };
            let inner = swiftui_node(
                &ViewNode::Stack {
                    axis: *axis,
                    spacing: Some(8.0),
                    align_items: None,
                    justify_content: None,
                    on_long_press: None,
                    style: None,
                    children: children.clone(),
                },
                indent + 1,
                scopes,
            );
            let mut out = format!("{pad}ScrollView({scroll_axis}) {{\n{inner}\n{pad}}}");
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::List {
            ordered,
            style,
            children,
        } => {
            let rows = children
                .iter()
                .enumerate()
                .map(|(idx, child)| {
                    if *ordered {
                        format!(
                            "{}HStack(alignment: .top, spacing: 8.0) {{\n{}Text(\"{}.\")\n{}\n{}}}",
                            indent_str(indent + 1),
                            indent_str(indent + 2),
                            idx + 1,
                            swiftui_node(child, indent + 2, scopes),
                            indent_str(indent + 1)
                        )
                    } else {
                        swiftui_node(child, indent + 1, scopes)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let mut out = format!("{pad}List {{\n{rows}\n{pad}}}");
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::ListItem {
            style, children, ..
        } => {
            let inner = swiftui_children(children, indent + 1, scopes);
            let mut out =
                format!("{pad}VStack(alignment: .leading, spacing: 4.0) {{\n{inner}\n{pad}}}");
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Link {
            href,
            href_bind,
            style,
            children,
            ..
        } => {
            let mut out = if href_bind.is_none() && href.is_empty() {
                let inner = swiftui_children(children, indent + 1, scopes);
                format!("{pad}VStack(alignment: .leading, spacing: 0.0) {{\n{inner}\n{pad}}}")
            } else {
                let label = swiftui_children(children, indent + 3, scopes);
                let fallback = swiftui_children(children, indent + 2, scopes);
                let (setup, destination, condition) = if let Some(bind) = href_bind {
                    (
                        format!(
                            "let crepusHref = {}\n{}",
                            SWIFT.model_text(bind, scopes),
                            indent_str(indent + 1)
                        ),
                        "crepusHref".to_string(),
                        "!crepusHref.isEmpty, ",
                    )
                } else {
                    (String::new(), format!("\"{}\"", SWIFT.escape(href)), "")
                };
                format!("{pad}Group {{\n{}{setup}if {condition}let crepusDestination = Foundation.URL(string: {destination}) {{\n{}SwiftUI.Link(destination: crepusDestination) {{\n{}VStack(alignment: .leading, spacing: 0.0) {{\n{label}\n{}}}\n{}}}\n{}}} else {{\n{}VStack(alignment: .leading, spacing: 0.0) {{\n{fallback}\n{}}}\n{}}}\n{pad}}}", indent_str(indent+1), indent_str(indent+2), indent_str(indent+3), indent_str(indent+3), indent_str(indent+2), indent_str(indent+1), indent_str(indent+2), indent_str(indent+2), indent_str(indent+1))
            };
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::SlotRotate { phrases, style, .. } => {
            let mut out = format!(
                "{pad}Text(\"{}\")",
                SWIFT.escape(phrases.first().map(String::as_str).unwrap_or(""))
            );
            swiftui_style(&mut out, style.as_ref(), true, indent);
            out
        }
        ViewNode::Input {
            placeholder,
            bind,
            multiline,
            secure,
            on_change,
            on_input,
            style,
            ..
        } => {
            // Native setters expose edits; both actions follow one binding update.
            let mut on_value_change =
                swiftui_change(on_change.as_deref(), bind, "newValue", scopes);
            if let Some(action) = on_input.as_deref() {
                if !on_value_change.is_empty() {
                    on_value_change.push_str("; ");
                }
                on_value_change.push_str(&swiftui_action(Some(action)));
            }
            // Directly supplied IR must not turn secure text into an unmasked editor.
            let mut out = if *multiline && !secure {
                format!(
                    "{pad}TextEditor(text: Binding(get: {{ {} }}, set: {{ newValue in {} }}))",
                    swiftui_draft_getter("Text", bind, scopes),
                    on_value_change
                )
            } else if *secure {
                format!(
                    "{pad}SecureField(\"{}\", text: Binding(get: {{ {} }}, set: {{ newValue in {} }}))",
                    SWIFT.escape(placeholder),
                    swiftui_draft_getter("Text", bind, scopes),
                    on_value_change
                )
            } else {
                format!(
                    "{pad}TextField(\"{}\", text: Binding(get: {{ {} }}, set: {{ newValue in {} }}))",
                    SWIFT.escape(placeholder)
                    ,
                    swiftui_draft_getter("Text", bind, scopes),
                    on_value_change
                )
            };
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Picker {
            bind,
            options,
            on_change,
            style,
            ..
        } => {
            let first = options.first().map(|o| o.value.as_str()).unwrap_or("");
            let rows = options
                .iter()
                .map(|option| {
                    format!(
                        "{}Text(\"{}\").tag(\"{}\")",
                        indent_str(indent + 1),
                        SWIFT.escape(&option.label),
                        SWIFT.escape(&option.value)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let mut out = format!(
                "{pad}Picker(\"\", selection: Binding(get: {{ {} }}, set: {{ newValue in {} }})) {{\n{rows}\n{pad}}}",
                swiftui_draft_getter("Text", bind, scopes),
                swiftui_change(on_change.as_deref(), bind, "newValue", scopes)
            );
            out.push('\n');
            out.push_str(&indent_str(indent));
            out.push_str(".pickerStyle(.segmented)");
            swiftui_style(&mut out, style.as_ref(), false, indent);
            if first.is_empty() {
                out = out.replace("Binding(get: {  }, set: { _ in })", ".constant(\"\")");
            }
            out
        }
        ViewNode::Tabs {
            bind,
            tabs,
            on_change,
            style,
            ..
        } => {
            let first = tabs.first().map(|tab| tab.value.as_str()).unwrap_or("");
            let rows = tabs
                .iter()
                .map(|tab| {
                    let content = swiftui_nodes(&tab.children, indent + 1, scopes);
                    let tab_item = if let Some(icon) = tab.icon.as_deref() {
                        format!(
                            "Label(\"{}\", systemImage: \"{}\")",
                            SWIFT.escape(&tab.label),
                            SWIFT.escape(icon)
                        )
                    } else {
                        format!("Text(\"{}\")", SWIFT.escape(&tab.label))
                    };
                    format!(
                        "{content}\n{}.tabItem {{ {tab_item} }}\n{}.tag(\"{}\")",
                        indent_str(indent + 1),
                        indent_str(indent + 1),
                        SWIFT.escape(&tab.value)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let mut out = format!(
                "{pad}TabView(selection: Binding(get: {{ {} }}, set: {{ newValue in {} }})) {{\n{rows}\n{pad}}}",
                swiftui_draft_getter("Text", bind, scopes),
                swiftui_change(on_change.as_deref(), bind, "newValue", scopes)
            );
            swiftui_style(&mut out, style.as_ref(), false, indent);
            if first.is_empty() {
                out = out.replace("Binding(get: {  }, set: { _ in })", ".constant(\"\")");
            }
            out
        }
    };
    let on_long_press = match node {
        ViewNode::Stack { on_long_press, .. }
        | ViewNode::Scroll { on_long_press, .. }
        | ViewNode::Button { on_long_press, .. }
        | ViewNode::Toggle { on_long_press, .. }
        | ViewNode::Image { on_long_press, .. }
        | ViewNode::ListItem { on_long_press, .. } => on_long_press.as_deref(),
        _ => None,
    };
    if let Some(action) = on_long_press {
        out.push_str(&format!(
            "\n{pad}.highPriorityGesture(LongPressGesture().onEnded {{ _ in {} }})",
            swiftui_action(Some(action))
        ));
    }
    out
}

fn swiftui_action(on_click: Option<&str>) -> String {
    on_click
        .map(|action| format!("CrepusActions.perform(\"{}\")", SWIFT.escape(action)))
        .unwrap_or_default()
}

fn swiftui_draft_getter(kind: &str, bind: &str, scopes: &[ScopeBinding]) -> String {
    let bind = bind.trim();
    let fallback = match kind {
        "Bool" => SWIFT.model_bool(bind, scopes),
        "Number" => SWIFT.model_number(bind, scopes),
        _ => SWIFT.model_text(bind, scopes),
    };
    if bind.is_empty() {
        return fallback;
    }
    format!(
        "model.draft{kind}(path: {}, fallback: {fallback})",
        SWIFT.binding_path(bind, scopes)
    )
}

fn swiftui_change(
    on_change: Option<&str>,
    bind: &str,
    value: &str,
    scopes: &[ScopeBinding],
) -> String {
    let bind = bind.trim();
    if bind.is_empty() {
        return swiftui_action(on_change);
    }
    let action = on_change
        .map(|action| format!("\"{}\"", SWIFT.escape(action)))
        .unwrap_or_else(|| SWIFT.null.to_string());
    format!(
        "CrepusActions.performChange({}, bind: {}, value: {})",
        action,
        SWIFT.binding_path(bind, scopes),
        value
    )
}

fn swiftui_children(children: &[ViewNode], indent: usize, scopes: &[ScopeBinding]) -> String {
    children
        .iter()
        .map(|child| swiftui_node(child, indent, scopes))
        .collect::<Vec<_>>()
        .join("\n")
}

fn swiftui_stack_alignment(axis: StackAxis, align_items: Option<&str>) -> &'static str {
    match axis {
        StackAxis::Column => match align_items {
            Some("center") => ".center",
            Some("end") => ".trailing",
            _ => ".leading",
        },
        StackAxis::Row => match align_items {
            Some("center") => ".center",
            Some("end") => ".bottom",
            _ => ".top",
        },
    }
}

fn swiftui_grid_alignment(align_items: Option<&str>) -> &'static str {
    match align_items {
        Some("center") => ".center",
        Some("end") => ".trailing",
        _ => ".leading",
    }
}

fn swiftui_style(out: &mut String, style: Option<&ViewStyle>, is_text: bool, indent: usize) {
    swiftui_style_with_alignment(out, style, is_text, indent, ".topLeading");
}

fn swiftui_style_with_alignment(
    out: &mut String,
    style: Option<&ViewStyle>,
    is_text: bool,
    indent: usize,
    frame_alignment: &str,
) {
    let Some(style) = style else {
        return;
    };
    let pad = indent_str(indent + 1);
    if let Some(size) = finite_opt(style.font_size) {
        out.push_str(&format!("\n{pad}.font(.system(size: {size:?}))"));
    }
    if let Some(weight) = style.font_weight {
        out.push_str(&format!(
            "\n{pad}.fontWeight({})",
            swiftui_font_weight(weight)
        ));
    }
    let design = match style.font_family.as_deref() {
        Some("sans") => Some(".default"),
        Some("serif") => Some(".serif"),
        Some("mono") => Some(".monospaced"),
        _ => None,
    };
    if let Some(design) = design {
        out.push_str(&format!("\n{pad}.fontDesign({design})"));
    }
    if let Some(color) = &style.foreground_color {
        out.push_str(&format!("\n{pad}.foregroundStyle({})", swift_color(color)));
    }
    if is_text {
        if let Some(spacing) = finite_opt(style.letter_spacing) {
            out.push_str(&format!("\n{pad}.tracking({spacing:?})"));
        }
        if let Some(align) = swiftui_text_align(style.text_align.as_deref()) {
            out.push_str(&format!("\n{pad}.multilineTextAlignment({align})"));
        }
        if style.italic == Some(true) {
            out.push_str(&format!("\n{pad}.italic()"));
        }
        if style.underline == Some(true) {
            out.push_str(&format!("\n{pad}.underline()"));
        }
        if style.strikethrough == Some(true) {
            out.push_str(&format!("\n{pad}.strikethrough()"));
        }
        if let Some(lines) = text_line_limit(style) {
            out.push_str(&format!("\n{pad}.lineLimit({lines})"));
        }
    }
    swiftui_frame(out, style, &pad, frame_alignment);
    swiftui_spacing(out, style, &pad, "padding");
    swiftui_spacing(out, style, &pad, "margin");
    if let Some(opacity) = finite_opt(style.opacity) {
        out.push_str(&format!("\n{pad}.opacity({opacity:.3})"));
    }
    if style.hidden == Some(true) {
        out.push_str(&format!("\n{pad}.opacity(0)"));
    }
    if let Some(background) = &style.background_color {
        out.push_str(&format!("\n{pad}.background({})", swift_color(background)));
    }
    if let Some(radius) = finite_opt(style.corner_radius) {
        out.push_str(&format!(
            "\n{pad}.clipShape(RoundedRectangle(cornerRadius: {radius:.1}))"
        ));
    }
    if let Some(width) = finite_opt(style.border_width) {
        let color = style
            .border_color
            .as_deref()
            .map(swift_color)
            .unwrap_or_else(|| "Color.gray".to_string());
        out.push_str(&format!("\n{pad}.border({color}, width: {width:.1})"));
    }
    if style.overflow_hidden == Some(true) {
        out.push_str(&format!("\n{pad}.clipped()"));
    }
    if let Some(radius) = finite_opt(style.shadow_radius) {
        let color = style
            .shadow_color
            .as_deref()
            .map(swift_color)
            .unwrap_or_else(|| "Color.black.opacity(0.25)".to_string());
        let x = finite_opt(style.shadow_offset_x).unwrap_or(0.0);
        let y = finite_opt(style.shadow_offset_y).unwrap_or(0.0);
        out.push_str(&format!(
            "\n{pad}.shadow(color: {color}, radius: {radius:.1}, x: {x:.1}, y: {y:.1})"
        ));
    }
    let translate_x = finite_opt(style.translate_x);
    let translate_y = finite_opt(style.translate_y);
    if translate_x.is_some() || translate_y.is_some() {
        out.push_str(&format!(
            "\n{pad}.offset(x: {:.1}, y: {:.1})",
            translate_x.unwrap_or(0.0),
            translate_y.unwrap_or(0.0)
        ));
    }
    if let Some(rotate) = finite_opt(style.rotate) {
        out.push_str(&format!("\n{pad}.rotationEffect(.degrees({rotate:.1}))"));
    }
    let scale_x = finite_opt(style.scale_x);
    let scale_y = finite_opt(style.scale_y);
    if scale_x.is_some() || scale_y.is_some() {
        out.push_str(&format!(
            "\n{pad}.scaleEffect(x: {:.3}, y: {:.3})",
            scale_x.unwrap_or(1.0),
            scale_y.unwrap_or(1.0)
        ));
    }
}

fn swiftui_spacing(out: &mut String, style: &ViewStyle, pad: &str, kind: &str) {
    let values = if kind == "padding" {
        [
            (style.padding, ""),
            (style.padding_horizontal, ".horizontal"),
            (style.padding_vertical, ".vertical"),
            (style.padding_top, ".top"),
            (style.padding_bottom, ".bottom"),
            (style.padding_left, ".leading"),
            (style.padding_right, ".trailing"),
        ]
    } else {
        [
            (style.margin, ""),
            (style.margin_horizontal, ".horizontal"),
            (style.margin_vertical, ".vertical"),
            (style.margin_top, ".top"),
            (style.margin_bottom, ".bottom"),
            (style.margin_left, ".leading"),
            (style.margin_right, ".trailing"),
        ]
    };
    let filter: fn(Option<f32>) -> Option<f32> = if kind == "padding" {
        nonnegative_opt
    } else {
        finite_opt
    };
    for (value, edge) in values {
        if let Some(value) = filter(value) {
            if edge.is_empty() {
                out.push_str(&format!("\n{pad}.padding({value})"));
            } else {
                out.push_str(&format!("\n{pad}.padding({edge}, {value})"));
            }
        }
    }
}

fn swiftui_frame(out: &mut String, style: &ViewStyle, pad: &str, alignment: &str) {
    let width = finite_opt(style.width);
    let height = finite_opt(style.height);
    if width.is_some_and(|value| value > 0.0) || height.is_some_and(|value| value > 0.0) {
        let width = width
            .filter(|v| *v > 0.0)
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "nil".to_string());
        let height = height
            .filter(|v| *v > 0.0)
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "nil".to_string());
        out.push_str(&format!(
            "\n{pad}.frame(width: {width}, height: {height}, alignment: {alignment})"
        ));
    }
    if width == Some(-2.0) || height == Some(-2.0) {
        out.push_str(&format!(
            "\n{pad}.fixedSize(horizontal: {}, vertical: {})",
            width == Some(-2.0),
            height == Some(-2.0)
        ));
    }
    if width == Some(-1.0) || height == Some(-1.0) {
        let max_width = if width == Some(-1.0) {
            ".infinity".to_string()
        } else {
            "nil".to_string()
        };
        let max_height = if height == Some(-1.0) {
            ".infinity".to_string()
        } else {
            "nil".to_string()
        };
        out.push_str(&format!(
            "\n{pad}.frame(maxWidth: {max_width}, maxHeight: {max_height}, alignment: {alignment})"
        ));
    }
    let min_width = finite_opt(style.min_width);
    let max_width = finite_opt(style.max_width);
    let min_height = finite_opt(style.min_height);
    let max_height = finite_opt(style.max_height);
    if min_width.is_some() || max_width.is_some() || min_height.is_some() || max_height.is_some() {
        let min_width = swiftui_dimension(min_width);
        let max_width = if max_width == Some(-1.0) {
            ".infinity".into()
        } else {
            swiftui_dimension(max_width)
        };
        let min_height = swiftui_dimension(min_height);
        let max_height = if max_height == Some(-1.0) {
            ".infinity".into()
        } else {
            swiftui_dimension(max_height)
        };
        out.push_str(&format!(
            "\n{pad}.frame(minWidth: {min_width}, maxWidth: {max_width}, minHeight: {min_height}, maxHeight: {max_height}, alignment: {alignment})"
        ));
    }
}

fn swiftui_frame_alignment(
    axis: StackAxis,
    justify_content: Option<&str>,
    align_items: Option<&str>,
) -> &'static str {
    match (axis, justify_content, align_items) {
        (StackAxis::Column, Some("center"), Some("center")) => ".center",
        (StackAxis::Column, Some("center"), Some("end")) => ".trailing",
        (StackAxis::Column, Some("center"), _) => ".leading",
        (StackAxis::Column, Some("end"), Some("center")) => ".bottom",
        (StackAxis::Column, Some("end"), Some("end")) => ".bottomTrailing",
        (StackAxis::Column, Some("end"), _) => ".bottomLeading",
        (StackAxis::Row, Some("center"), Some("center")) => ".center",
        (StackAxis::Row, Some("center"), Some("end")) => ".bottom",
        (StackAxis::Row, Some("center"), _) => ".top",
        (StackAxis::Row, Some("end"), Some("center")) => ".trailing",
        (StackAxis::Row, Some("end"), Some("end")) => ".bottomTrailing",
        (StackAxis::Row, Some("end"), _) => ".topTrailing",
        _ => ".topLeading",
    }
}

fn swiftui_dimension(value: Option<f32>) -> String {
    finite_opt(value)
        .filter(|value| *value > 0.0)
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "nil".to_string())
}

fn swiftui_text_align(value: Option<&str>) -> Option<&'static str> {
    match value {
        Some("center") => Some(".center"),
        Some("right") | Some("end") | Some("trailing") => Some(".trailing"),
        Some("justify") => Some(".leading"),
        Some("left") | Some("start") | Some("leading") => Some(".leading"),
        _ => None,
    }
}

fn swiftui_font_weight(weight: u16) -> &'static str {
    match weight {
        0..=199 => ".ultraLight",
        200..=299 => ".thin",
        300..=399 => ".light",
        400..=499 => ".regular",
        500..=599 => ".medium",
        600..=699 => ".semibold",
        700..=799 => ".bold",
        800..=899 => ".heavy",
        _ => ".black",
    }
}

fn swift_color(color: &str) -> String {
    let Some([r, g, b, a]) = crate::colors::resolve_rgba(color)
        .or_else(|| crate::colors::resolve_rgba(&format!("#{color}")))
    else {
        return "Color.gray".to_string();
    };
    format!("Color(red: {r:.3}, green: {g:.3}, blue: {b:.3}, opacity: {a:.3})")
}

#[cfg(test)]
mod dynamic_url_tests {
    use super::*;
    #[test]
    fn links_and_image_placeholders_use_scoped_model_reads() {
        let scopes = [ScopeBinding {
            name: "row".into(),
            variable: "rowValue".into(),
        }];
        for node in [
            ViewNode::Image {
                src: "snapshot.png".into(),
                src_bind: Some("row.url".into()),
                alt: None,
                placeholder: None,
                on_long_press: None,
                style: None,
            },
            ViewNode::Link {
                href: "snapshot.html".into(),
                href_bind: Some("row.url".into()),
                target: None,
                rel: None,
                style: None,
                children: vec![],
            },
        ] {
            let output = swiftui_node(&node, 0, &scopes);
            assert!(
                output.contains(".text(\"row.url\"") && output.contains("rowValue"),
                "{output}"
            );
            assert!(!output.contains("snapshot."), "{output}");
        }
        let node = ViewNode::Link {
            href: String::new(),
            href_bind: Some("url".into()),
            target: None,
            rel: None,
            style: None,
            children: vec![],
        };
        let output = swiftui_node(&node, 0, &[]);
        assert!(output.contains(".text(\"url\""), "{output}");
        assert!(
            output.contains("isNotEmpty") || output.contains("!crepusHref.isEmpty"),
            "{output}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::StackAxis;

    #[test]
    fn test_swiftui_frame_alignment() {
        // Test Column axis
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Column, Some("center"), Some("center")),
            ".center"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Column, Some("center"), Some("end")),
            ".trailing"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Column, Some("center"), Some("start")),
            ".leading"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Column, Some("center"), None),
            ".leading"
        );

        assert_eq!(
            swiftui_frame_alignment(StackAxis::Column, Some("end"), Some("center")),
            ".bottom"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Column, Some("end"), Some("end")),
            ".bottomTrailing"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Column, Some("end"), Some("start")),
            ".bottomLeading"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Column, Some("end"), None),
            ".bottomLeading"
        );

        // Fallback cases for Column (start, other)
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Column, Some("start"), Some("center")),
            ".topLeading"
        ); // Matches fallback _
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Column, None, None),
            ".topLeading"
        );

        // Test Row axis
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Row, Some("center"), Some("center")),
            ".center"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Row, Some("center"), Some("end")),
            ".bottom"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Row, Some("center"), Some("start")),
            ".top"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Row, Some("center"), None),
            ".top"
        );

        assert_eq!(
            swiftui_frame_alignment(StackAxis::Row, Some("end"), Some("center")),
            ".trailing"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Row, Some("end"), Some("end")),
            ".bottomTrailing"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Row, Some("end"), Some("start")),
            ".topTrailing"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Row, Some("end"), None),
            ".topTrailing"
        );

        // Fallback cases for Row (start, other)
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Row, Some("start"), Some("center")),
            ".topLeading"
        );
        assert_eq!(
            swiftui_frame_alignment(StackAxis::Row, None, None),
            ".topLeading"
        );
    }
}
