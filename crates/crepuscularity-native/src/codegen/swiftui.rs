use crate::ir::{StackAxis, ViewIr, ViewNode, ViewStyle};

use super::shared::{bool_literal, indent_str, SWIFT};

pub(super) fn generate_swiftui(ir: &ViewIr, view_name: &str) -> String {
    let body = swiftui_nodes(&ir.root, 2, None, None);
    let webview = "import WebKit\n#if canImport(UIKit)\npublic struct CrepusWebView: UIViewRepresentable {\n    public let source: String\n\n    public func makeUIView(context: Context) -> WKWebView {\n        WKWebView()\n    }\n\n    public func updateUIView(_ webView: WKWebView, context: Context) {\n        guard webView.url?.absoluteString != source, let url = URL(string: source) else { return }\n        webView.load(URLRequest(url: url))\n    }\n}\n#elseif canImport(AppKit)\npublic struct CrepusWebView: NSViewRepresentable {\n    public let source: String\n\n    public func makeNSView(context: Context) -> WKWebView {\n        WKWebView()\n    }\n\n    public func updateNSView(_ webView: WKWebView, context: Context) {\n        guard webView.url?.absoluteString != source, let url = URL(string: source) else { return }\n        webView.load(URLRequest(url: url))\n    }\n}\n#endif\n\n";
    format!(
        "import Foundation\nimport SwiftUI\n{webview}#if canImport(UIKit)\nimport UIKit\n#endif\n#if canImport(AppKit)\nimport AppKit\n#endif\n\n@MainActor\npublic enum CrepusActions {{\n    public static let model = CrepusStateStore.shared\n    public static var dispatch: (String) -> String = {{ _ in \"{{}}\" }}\n    public static var resultSink: (String) -> Void = {{ _ in }}\n\n    public static func applyResult(_ json: String) {{\n        model.applyResult(json)\n    }}\n\n    public static func perform(_ action: String) {{\n        let dispatch = dispatch\n        let resultSink = resultSink\n        DispatchQueue.global(qos: .userInitiated).async {{\n            let result = dispatch(action)\n            DispatchQueue.main.async {{\n                resultSink(result)\n            }}\n        }}\n    }}\n\n    public static func performChange(_ action: String?, bind: String, value: Any) {{\n        resultSink(CrepusRustActions.dispatchChangeStored(action ?? \"\", bind: bind, value: value))\n    }}\n\n    public static func dismissKeyboard() {{\n#if canImport(UIKit)\n        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)\n#elseif canImport(AppKit)\n        NSApp.keyWindow?.makeFirstResponder(nil)\n#endif\n    }}\n}}\n\n@MainActor\npublic struct {view_name}: View {{\n    @ObservedObject private var model = CrepusActions.model\n\n    public init() {{}}\n\n    public var body: some View {{\n{body}\n        .contentShape(Rectangle())\n        .onTapGesture {{ CrepusActions.dismissKeyboard() }}\n    }}\n}}\n"
    )
}

fn swiftui_nodes(
    nodes: &[ViewNode],
    indent: usize,
    scope_name: Option<&str>,
    scope_var: Option<&str>,
) -> String {
    if nodes.len() == 1 {
        swiftui_node(&nodes[0], indent, scope_name, scope_var)
    } else {
        let pad = indent_str(indent);
        let inner = nodes
            .iter()
            .map(|node| swiftui_node(node, indent + 1, scope_name, scope_var))
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
    scope_name: Option<&str>,
    scope_var: Option<&str>,
) -> String {
    let child_lines = swiftui_children(children, indent, scope_name, scope_var);
    let spacer = format!("{}Spacer()", indent_str(indent));
    match justify_content {
        Some("end") => format!("{spacer}\n{child_lines}"),
        Some("between") if children.len() > 1 => children
            .iter()
            .map(|child| swiftui_node(child, indent, scope_name, scope_var))
            .collect::<Vec<_>>()
            .join(&format!("\n{spacer}\n")),
        _ => child_lines,
    }
}

fn swiftui_node(
    node: &ViewNode,
    indent: usize,
    scope_name: Option<&str>,
    scope_var: Option<&str>,
) -> String {
    let pad = indent_str(indent);
    match node {
        ViewNode::Text {
            content,
            bind,
            style,
            ..
        } => {
            let text = bind
                .as_deref()
                .map(|bind| SWIFT.model_text(bind, scope_name, scope_var))
                .unwrap_or_else(|| format!("\"{}\"", SWIFT.escape(content)));
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
            let then_inner = swiftui_children(then_children, indent + 1, scope_name, scope_var);
            let condition = SWIFT.model_bool(condition, scope_name, scope_var);
            if let Some(else_children) = else_children {
                let else_inner = swiftui_children(else_children, indent + 1, scope_name, scope_var);
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
            let item_var = SWIFT.identifier(item_name);
            let items = SWIFT.model_items(bind, scope_name, scope_var);
            let inner = swiftui_children(item_body, indent + 1, Some(item_name), Some(&item_var));
            format!(
                "{pad}ForEach(Array({items}.enumerated()), id: \\.offset) {{ _, {item_var} in\n{inner}\n{pad}}}"
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
            let gap = spacing.unwrap_or(8.0);
            let inner = swiftui_stack_children(
                *axis,
                justify_content.as_deref(),
                children,
                indent + 1,
                scope_name,
                scope_var,
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
            on_click,
            style,
            ..
        } => {
            let action = swiftui_action(on_click.as_deref());
            let mut out = format!(
                "{pad}Button(action: {{ {action} }}) {{\n{}Text(\"{}\")\n{pad}}}",
                indent_str(indent + 1),
                SWIFT.escape(label)
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
        } => {
            let binding = bind
                .as_deref()
                .map(|bind| {
                    format!(
                        "Binding(get: {{ {} }}, set: {{ newValue in {} }})",
                        SWIFT.model_bool(bind, scope_name, scope_var),
                        swiftui_change(on_change.as_deref(), bind, "newValue")
                    )
                })
                .unwrap_or_else(|| format!(".constant({})", bool_literal(*checked)));
            let mut out = format!("{pad}Toggle(\"{}\", isOn: {binding})", SWIFT.escape(label));
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Checkbox {
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
                        SWIFT.model_bool(bind, scope_name, scope_var),
                        swiftui_change(on_change.as_deref(), bind, "newValue")
                    )
                })
                .unwrap_or_else(|| format!(".constant({})", bool_literal(*checked)));
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
            let step_arg = step.map(|v| format!(", step: {v:.1}")).unwrap_or_default();
            let slider_value = bind
                .as_deref()
                .map(|bind| {
                    format!(
                        "Binding(get: {{ {} }}, set: {{ newValue in {} }})",
                        SWIFT.model_number(bind, scope_name, scope_var),
                        swiftui_change(on_change.as_deref(), bind, "newValue")
                    )
                })
                .unwrap_or_else(|| format!(".constant({value:.3})"));
            let control =
                format!("Slider(value: {slider_value}, in: {min:.3}...{max:.3}{step_arg})");
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
            let mut out = if let Some(label) = label {
                format!(
                    "{pad}ProgressView(\"{}\", value: {value:.3}, total: {max:.3})",
                    SWIFT.escape(label)
                )
            } else {
                format!("{pad}ProgressView(value: {value:.3}, total: {max:.3})")
            };
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Meter {
            label,
            value,
            max,
            style,
            ..
        } => {
            let text = label
                .as_deref()
                .map(|label| format!("{label}: {value:.1}/{max:.1}"))
                .unwrap_or_else(|| format!("{value:.1}/{max:.1}"));
            let mut out = format!("{pad}Text(\"{}\")", SWIFT.escape(&text));
            swiftui_style(&mut out, style.as_ref(), true, indent);
            out
        }
        ViewNode::Badge { label, style, .. } => {
            let mut out = format!("{pad}Text(\"{}\")", SWIFT.escape(label));
            swiftui_style(&mut out, style.as_ref(), true, indent);
            out
        }
        ViewNode::Divider { style, .. } => {
            let mut out = format!("{pad}Divider()");
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Spacer { size, style } => {
            let mut out = size
                .map(|v| format!("{pad}Spacer(minLength: {v:.1})"))
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
                swiftui_children(children, indent + 1, scope_name, scope_var)
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
            src, alt, style, ..
        } => {
            let label = alt.as_deref().unwrap_or(src);
            let mut out = format!("{pad}Text(\"{}\")", SWIFT.escape(label));
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
                scope_name,
                scope_var,
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
                            swiftui_node(child, indent + 2, scope_name, scope_var),
                            indent_str(indent + 1)
                        )
                    } else {
                        swiftui_node(child, indent + 1, scope_name, scope_var)
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
            let inner = swiftui_children(children, indent + 1, scope_name, scope_var);
            let mut out =
                format!("{pad}VStack(alignment: .leading, spacing: 4.0) {{\n{inner}\n{pad}}}");
            swiftui_style(&mut out, style.as_ref(), false, indent);
            out
        }
        ViewNode::Link {
            style, children, ..
        } => {
            let inner = swiftui_children(children, indent + 1, scope_name, scope_var);
            let mut out =
                format!("{pad}VStack(alignment: .leading, spacing: 0.0) {{\n{inner}\n{pad}}}");
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
            style,
            ..
        } => {
            let mut out = if *multiline {
                format!(
                    "{pad}TextEditor(text: Binding(get: {{ {} }}, set: {{ newValue in {} }}))",
                    SWIFT.model_text(bind, scope_name, scope_var),
                    swiftui_change(on_change.as_deref(), bind, "newValue")
                )
            } else if *secure {
                format!(
                    "{pad}SecureField(\"{}\", text: Binding(get: {{ {} }}, set: {{ newValue in {} }}))",
                    SWIFT.escape(placeholder),
                    SWIFT.model_text(bind, scope_name, scope_var),
                    swiftui_change(on_change.as_deref(), bind, "newValue")
                )
            } else {
                format!(
                    "{pad}TextField(\"{}\", text: Binding(get: {{ {} }}, set: {{ newValue in {} }}))",
                    SWIFT.escape(placeholder)
                    ,
                    SWIFT.model_text(bind, scope_name, scope_var),
                    swiftui_change(on_change.as_deref(), bind, "newValue")
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
                SWIFT.model_text(bind, scope_name, scope_var),
                swiftui_change(on_change.as_deref(), bind, "newValue")
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
                    let content = swiftui_nodes(&tab.children, indent + 1, scope_name, scope_var);
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
                SWIFT.model_text(bind, scope_name, scope_var),
                swiftui_change(on_change.as_deref(), bind, "newValue")
            );
            swiftui_style(&mut out, style.as_ref(), false, indent);
            if first.is_empty() {
                out = out.replace("Binding(get: {  }, set: { _ in })", ".constant(\"\")");
            }
            out
        }
    }
}

fn swiftui_action(on_click: Option<&str>) -> String {
    on_click
        .map(|action| format!("CrepusActions.perform(\"{}\")", SWIFT.escape(action)))
        .unwrap_or_default()
}

fn swiftui_change(on_change: Option<&str>, bind: &str, value: &str) -> String {
    let action = on_change
        .map(|action| format!("\"{}\"", SWIFT.escape(action)))
        .unwrap_or_else(|| SWIFT.null.to_string());
    format!(
        "CrepusActions.performChange({}, bind: \"{}\", value: {})",
        action,
        SWIFT.escape(bind),
        value
    )
}

fn swiftui_children(
    children: &[ViewNode],
    indent: usize,
    scope_name: Option<&str>,
    scope_var: Option<&str>,
) -> String {
    children
        .iter()
        .map(|child| swiftui_node(child, indent, scope_name, scope_var))
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
    if let Some(size) = style.font_size {
        out.push_str(&format!("\n{pad}.font(.system(size: {size:.1}))"));
    }
    if let Some(weight) = style.font_weight {
        out.push_str(&format!(
            "\n{pad}.fontWeight({})",
            swiftui_font_weight(weight)
        ));
    }
    if let Some(color) = &style.foreground_color {
        out.push_str(&format!("\n{pad}.foregroundStyle({})", swift_color(color)));
    }
    if is_text {
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
        if let Some(lines) = style.line_clamp {
            out.push_str(&format!("\n{pad}.lineLimit({lines})"));
        }
    }
    swiftui_frame(out, style, &pad, frame_alignment);
    swiftui_spacing(out, style, &pad, "padding");
    swiftui_spacing(out, style, &pad, "margin");
    if let Some(opacity) = style.opacity {
        out.push_str(&format!("\n{pad}.opacity({opacity:.3})"));
    }
    if style.hidden == Some(true) {
        out.push_str(&format!("\n{pad}.opacity(0)"));
    }
    if let Some(background) = &style.background_color {
        out.push_str(&format!("\n{pad}.background({})", swift_color(background)));
    }
    if let Some(radius) = style.corner_radius {
        out.push_str(&format!(
            "\n{pad}.clipShape(RoundedRectangle(cornerRadius: {radius:.1}))"
        ));
    }
    if let Some(width) = style.border_width {
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
    if let Some(radius) = style.shadow_radius {
        let color = style
            .shadow_color
            .as_deref()
            .map(swift_color)
            .unwrap_or_else(|| "Color.black.opacity(0.25)".to_string());
        let x = style.shadow_offset_x.unwrap_or(0.0);
        let y = style.shadow_offset_y.unwrap_or(0.0);
        out.push_str(&format!(
            "\n{pad}.shadow(color: {color}, radius: {radius:.1}, x: {x:.1}, y: {y:.1})"
        ));
    }
    if style.translate_x.is_some() || style.translate_y.is_some() {
        out.push_str(&format!(
            "\n{pad}.offset(x: {:.1}, y: {:.1})",
            style.translate_x.unwrap_or(0.0),
            style.translate_y.unwrap_or(0.0)
        ));
    }
    if let Some(rotate) = style.rotate {
        out.push_str(&format!("\n{pad}.rotationEffect(.degrees({rotate:.1}))"));
    }
    if style.scale_x.is_some() || style.scale_y.is_some() {
        out.push_str(&format!(
            "\n{pad}.scaleEffect(x: {:.3}, y: {:.3})",
            style.scale_x.unwrap_or(1.0),
            style.scale_y.unwrap_or(1.0)
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
    for (value, edge) in values {
        if let Some(value) = value {
            if edge.is_empty() {
                out.push_str(&format!("\n{pad}.padding({value:.0})"));
            } else {
                out.push_str(&format!("\n{pad}.padding({edge}, {value:.0})"));
            }
        }
    }
}

fn swiftui_frame(out: &mut String, style: &ViewStyle, pad: &str, alignment: &str) {
    if style.width == Some(-1.0) || style.height == Some(-1.0) {
        let max_width = if style.width == Some(-1.0) {
            ".infinity".to_string()
        } else {
            "nil".to_string()
        };
        let max_height = if style.height == Some(-1.0) {
            ".infinity".to_string()
        } else {
            "nil".to_string()
        };
        out.push_str(&format!(
            "\n{pad}.frame(maxWidth: {max_width}, maxHeight: {max_height}, alignment: {alignment})"
        ));
    } else if style.width.is_some() || style.height.is_some() {
        let width = style
            .width
            .filter(|v| *v > 0.0)
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "nil".to_string());
        let height = style
            .height
            .filter(|v| *v > 0.0)
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "nil".to_string());
        out.push_str(&format!(
            "\n{pad}.frame(width: {width}, height: {height}, alignment: {alignment})"
        ));
    }
    if style.min_width.is_some()
        || style.max_width.is_some()
        || style.min_height.is_some()
        || style.max_height.is_some()
    {
        let min_width = swiftui_dimension(style.min_width);
        let max_width = swiftui_dimension(style.max_width);
        let min_height = swiftui_dimension(style.min_height);
        let max_height = swiftui_dimension(style.max_height);
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
    value
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
        0..=299 => ".thin",
        300..=399 => ".light",
        400..=499 => ".regular",
        500..=599 => ".medium",
        600..=699 => ".semibold",
        700..=799 => ".bold",
        _ => ".heavy",
    }
}

fn swift_color(color: &str) -> String {
    let trimmed = color.trim_start_matches('#');
    let Some((r, g, b, a)) = parse_hex_rgba(trimmed) else {
        return "Color.gray".to_string();
    };
    format!(
        "Color(red: {:.3}, green: {:.3}, blue: {:.3}, opacity: {:.3})",
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0
    )
}

fn parse_hex_rgba(hex: &str) -> Option<(u8, u8, u8, u8)> {
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    if hex.len() == 8 {
        Some((
            ((value & 0x00ff0000) >> 16) as u8,
            ((value & 0x0000ff00) >> 8) as u8,
            (value & 0x000000ff) as u8,
            ((value & 0xff000000) >> 24) as u8,
        ))
    } else {
        Some((
            ((value & 0xff0000) >> 16) as u8,
            ((value & 0x00ff00) >> 8) as u8,
            (value & 0x0000ff) as u8,
            255,
        ))
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
