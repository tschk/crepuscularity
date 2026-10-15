use crate::ir::{StackAxis, ViewIr, ViewNode, ViewStyle};

use super::shared::{bool_literal, indent_str, KOTLIN};

pub(super) fn generate_compose(ir: &ViewIr, view_name: &str) -> String {
    let body = compose_nodes(&ir.root, 1, None, None);
    let webview_imports =
        "import android.webkit.WebView\nimport android.webkit.WebViewClient\nimport androidx.compose.ui.viewinterop.AndroidView\n";
    let source = format!(
        "import androidx.compose.foundation.background\nimport androidx.compose.foundation.border\nimport androidx.compose.foundation.horizontalScroll\nimport androidx.compose.foundation.layout.Arrangement\nimport androidx.compose.foundation.layout.Column\nimport androidx.compose.foundation.layout.ExperimentalLayoutApi\nimport androidx.compose.foundation.layout.FlowRow\nimport androidx.compose.foundation.layout.PaddingValues\nimport androidx.compose.foundation.layout.Row\nimport androidx.compose.foundation.layout.Spacer\nimport androidx.compose.foundation.layout.fillMaxHeight\nimport androidx.compose.foundation.layout.fillMaxSize\nimport androidx.compose.foundation.layout.fillMaxWidth\nimport androidx.compose.foundation.layout.height\nimport androidx.compose.foundation.layout.heightIn\nimport androidx.compose.foundation.layout.offset\nimport androidx.compose.foundation.layout.padding\nimport androidx.compose.foundation.layout.width\nimport androidx.compose.foundation.layout.widthIn\nimport androidx.compose.foundation.lazy.LazyColumn\nimport androidx.compose.foundation.rememberScrollState\nimport androidx.compose.foundation.shape.RoundedCornerShape\nimport androidx.compose.foundation.verticalScroll\nimport androidx.compose.material3.Button\nimport androidx.compose.material3.ButtonDefaults\nimport androidx.compose.material3.Divider\nimport androidx.compose.material3.FilterChip\nimport androidx.compose.material3.LinearProgressIndicator\nimport androidx.compose.material3.LocalContentColor\nimport androidx.compose.material3.NavigationBar\nimport androidx.compose.material3.NavigationBarItem\nimport androidx.compose.material3.Scaffold\nimport androidx.compose.material3.Slider\nimport androidx.compose.material3.Switch\nimport androidx.compose.material3.Text\nimport androidx.compose.material3.TextField\nimport androidx.compose.runtime.Composable\nimport androidx.compose.runtime.CompositionLocalProvider\nimport androidx.compose.ui.Alignment\nimport androidx.compose.ui.Modifier\nimport androidx.compose.ui.draw.alpha\nimport androidx.compose.ui.draw.clip\nimport androidx.compose.ui.draw.rotate\nimport androidx.compose.ui.draw.scale\nimport androidx.compose.ui.graphics.Color\nimport androidx.compose.ui.text.font.FontStyle\nimport androidx.compose.ui.text.font.FontWeight\nimport androidx.compose.ui.text.input.PasswordVisualTransformation\nimport androidx.compose.ui.text.style.TextAlign\nimport androidx.compose.ui.text.style.TextDecoration\nimport androidx.compose.ui.unit.dp\nimport androidx.compose.ui.unit.sp\nimport kotlinx.serialization.json.JsonElement\nimport kotlinx.serialization.json.JsonPrimitive\n\nobject CrepusActions {{\n    var dispatch: (String) -> String = {{ \"{{}}\" }}\n    var resultSink: (String) -> Unit = {{}}\n\n    fun applyResult(raw: String) {{\n        CrepusStateStore.applyResult(raw)\n    }}\n\n    fun perform(action: String) {{\n        val dispatch = dispatch\n        val resultSink = resultSink\n        Thread {{\n            val result = dispatch(action)\n            android.os.Handler(android.os.Looper.getMainLooper()).post {{\n                resultSink(result)\n            }}\n        }}.start()\n    }}\n\n    fun performChange(action: String?, bind: String, value: JsonElement) {{\n        resultSink(CrepusRustActions.dispatchChangeJson(action ?: \"\", bind, value.toString()))\n    }}\n}}\n\n@OptIn(ExperimentalLayoutApi::class)\n@Composable\nfun {view_name}(modifier: Modifier = Modifier) {{\n{body}\n}}\n"
    );
    format!("{webview_imports}{source}")
}

fn compose_nodes(
    nodes: &[ViewNode],
    indent: usize,
    scope_name: Option<&str>,
    scope_var: Option<&str>,
) -> String {
    if nodes.len() == 1 {
        compose_node_with_base(
            &nodes[0],
            indent,
            Some("modifier".to_string()),
            scope_name,
            scope_var,
        )
    } else {
        let pad = indent_str(indent);
        let inner = compose_children(nodes, indent + 1, scope_name, scope_var);
        format!("{pad}Column {{\n{inner}\n{pad}}}")
    }
}

fn compose_arrangement(axis: StackAxis, spacing: f32, justify_content: Option<&str>) -> String {
    let alignment = match (axis, justify_content) {
        (StackAxis::Column, Some("center")) => Some("Alignment.CenterVertically"),
        (StackAxis::Column, Some("end")) => Some("Alignment.Bottom"),
        (StackAxis::Column, Some("between")) => return "Arrangement.SpaceBetween".to_string(),
        (StackAxis::Column, Some("around")) => return "Arrangement.SpaceAround".to_string(),
        (StackAxis::Column, Some("evenly")) => return "Arrangement.SpaceEvenly".to_string(),
        (StackAxis::Row, Some("center")) => Some("Alignment.CenterHorizontally"),
        (StackAxis::Row, Some("end")) => Some("Alignment.End"),
        (StackAxis::Row, Some("between")) => return "Arrangement.SpaceBetween".to_string(),
        (StackAxis::Row, Some("around")) => return "Arrangement.SpaceAround".to_string(),
        (StackAxis::Row, Some("evenly")) => return "Arrangement.SpaceEvenly".to_string(),
        _ => None,
    };
    if let Some(alignment) = alignment {
        format!("Arrangement.spacedBy({spacing:.0}.dp, {alignment})")
    } else {
        format!("Arrangement.spacedBy({spacing:.0}.dp)")
    }
}

fn compose_node(
    node: &ViewNode,
    indent: usize,
    scope_name: Option<&str>,
    scope_var: Option<&str>,
) -> String {
    compose_node_with_base(node, indent, None, scope_name, scope_var)
}

fn compose_node_with_base(
    node: &ViewNode,
    indent: usize,
    base_modifier: Option<String>,
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
            let args = compose_text_args(style.as_ref());
            let text = bind
                .as_deref()
                .map(|bind| KOTLIN.model_text(bind, scope_name, scope_var))
                .unwrap_or_else(|| format!("\"{}\"", KOTLIN.escape(content)));
            format!("{pad}Text({text}{args})")
        }
        ViewNode::If {
            condition,
            then_children,
            else_children,
            ..
        } => {
            let condition = KOTLIN.model_bool(condition, scope_name, scope_var);
            let then_inner = compose_children(then_children, indent + 1, scope_name, scope_var);
            if let Some(else_children) = else_children {
                let else_inner = compose_children(else_children, indent + 1, scope_name, scope_var);
                format!("{pad}if ({condition}) {{\n{then_inner}\n{pad}}} else {{\n{else_inner}\n{pad}}}")
            } else {
                format!("{pad}if ({condition}) {{\n{then_inner}\n{pad}}}")
            }
        }
        ViewNode::ForEach {
            bind,
            item_name,
            item_body,
            ..
        } => {
            let item_var = KOTLIN.identifier(item_name);
            let items = KOTLIN.model_items(bind, scope_name, scope_var);
            let inner = compose_children(item_body, indent + 1, Some(item_name), Some(&item_var));
            format!("{pad}{items}.forEachIndexed {{ _, {item_var} ->\n{inner}\n{pad}}}")
        }
        ViewNode::Stack {
            axis,
            spacing,
            justify_content,
            style,
            children,
            ..
        } => {
            let wrap = *axis == StackAxis::Row
                && style.as_ref().and_then(|style| style.flex_wrap) == Some(true);
            let view = match (axis, wrap) {
                (StackAxis::Row, true) => "FlowRow",
                (StackAxis::Row, false) => "Row",
                (StackAxis::Column, _) => "Column",
            };
            let mut args = Vec::new();
            if let Some(modifier) = compose_modifier_chain(base_modifier, style.as_ref()) {
                args.push(format!("modifier = {modifier}"));
            }
            let arrangement = match axis {
                StackAxis::Row => "horizontalArrangement",
                StackAxis::Column => "verticalArrangement",
            };
            args.push(format!(
                "{arrangement} = {}",
                compose_arrangement(*axis, spacing.unwrap_or(8.0), justify_content.as_deref())
            ));
            if wrap {
                args.push(format!(
                    "verticalArrangement = Arrangement.spacedBy({:.0}.dp)",
                    spacing.unwrap_or(8.0)
                ));
            }
            let args = args.join(", ");
            let inner = compose_children(children, indent + 1, scope_name, scope_var);
            let out = format!("{pad}{view}({args}) {{\n{inner}\n{pad}}}");
            compose_content_color_wrapper(out, style.as_ref(), indent)
        }
        ViewNode::Button {
            label,
            on_click,
            style,
            ..
        } => {
            let modifier = compose_button_args(style.as_ref());
            let action = compose_action(on_click.as_deref());
            format!(
                "{pad}Button(onClick = {{ {action} }}{modifier}) {{\n{}Text(\"{}\")\n{pad}}}",
                indent_str(indent + 1),
                KOTLIN.escape(label)
            )
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
            let modifier = compose_modifier_call_args(style.as_ref());
            let checked_value = bind
                .as_deref()
                .map(|bind| KOTLIN.model_bool(bind, scope_name, scope_var))
                .unwrap_or_else(|| bool_literal(*checked).to_string());
            let on_change = bind
                .as_deref()
                .map(|bind| compose_change(on_change.as_deref(), bind, "JsonPrimitive(it)"))
                .unwrap_or_else(|| compose_action(on_change.as_deref()));
            format!(
                "{pad}Row{modifier} {{\n{}Text(\"{}\")\n{}Switch(checked = {checked_value}, onCheckedChange = {{ {on_change} }})\n{pad}}}",
                indent_str(indent + 1),
                KOTLIN.escape(label),
                indent_str(indent + 1)
            )
        }
        ViewNode::Slider {
            label,
            bind,
            value,
            min,
            max,
            on_change,
            style,
            ..
        } => {
            let modifier = compose_modifier_call_args(style.as_ref());
            let label = label
                .as_deref()
                .map(|label| {
                    format!(
                        "{}Text(\"{}\")\n",
                        indent_str(indent + 1),
                        KOTLIN.escape(label)
                    )
                })
                .unwrap_or_default();
            let slider_value = bind
                .as_deref()
                .map(|bind| KOTLIN.model_number(bind, scope_name, scope_var))
                .unwrap_or_else(|| format!("{value:.3}f"));
            let on_value_change = bind
                .as_deref()
                .map(|bind| compose_change(on_change.as_deref(), bind, "JsonPrimitive(it)"))
                .unwrap_or_default();
            format!(
                "{pad}Column{modifier} {{\n{label}{}Slider(value = {slider_value}, onValueChange = {{ {on_value_change} }}, valueRange = {min:.3}f..{max:.3}f)\n{pad}}}",
                indent_str(indent + 1)
            )
        }
        ViewNode::Progress {
            label,
            value,
            max,
            style,
        } => {
            let modifier = compose_modifier_call_args(style.as_ref());
            let label = label
                .as_deref()
                .map(|label| {
                    format!(
                        "{}Text(\"{}\")\n",
                        indent_str(indent + 1),
                        KOTLIN.escape(label)
                    )
                })
                .unwrap_or_default();
            format!(
                "{pad}Column{modifier} {{\n{label}{}LinearProgressIndicator(progress = {value:.3}f / {max:.3}f)\n{pad}}}",
                indent_str(indent + 1)
            )
        }
        ViewNode::Meter {
            label, value, max, ..
        } => {
            let text = label
                .as_deref()
                .map(|label| format!("{label}: {value:.1}/{max:.1}"))
                .unwrap_or_else(|| format!("{value:.1}/{max:.1}"));
            format!("{pad}Text(\"{}\")", KOTLIN.escape(&text))
        }
        ViewNode::Badge { label, style, .. } => {
            let args = compose_text_args(style.as_ref());
            format!("{pad}Text(\"{}\"{args})", KOTLIN.escape(label))
        }
        ViewNode::Divider { .. } => format!("{pad}Divider()"),
        ViewNode::Spacer { size, .. } => {
            format!(
                "{pad}Spacer(modifier = Modifier.height({:.0}.dp))",
                size.unwrap_or(8.0)
            )
        }
        ViewNode::Dropzone {
            label,
            style,
            children,
            ..
        } => {
            let modifier = compose_modifier_call_args(style.as_ref());
            let inner = if children.is_empty() {
                format!(
                    "{}Text(\"{}\")",
                    indent_str(indent + 1),
                    KOTLIN.escape(label)
                )
            } else {
                compose_children(children, indent + 1, scope_name, scope_var)
            };
            format!("{pad}Column{modifier} {{\n{inner}\n{pad}}}")
        }
        ViewNode::FilePicker {
            label,
            on_pick,
            style,
            ..
        } => {
            let modifier = compose_button_args(style.as_ref());
            let action = compose_action(on_pick.as_deref());
            format!(
                "{pad}Button(onClick = {{ {action} }}{modifier}) {{\n{}Text(\"{}\")\n{pad}}}",
                indent_str(indent + 1),
                KOTLIN.escape(label)
            )
        }
        ViewNode::Image {
            src, alt, style, ..
        } => {
            let args = compose_text_args(style.as_ref());
            format!(
                "{pad}Text(\"{}\"{args})",
                KOTLIN.escape(alt.as_deref().unwrap_or(src))
            )
        }
        ViewNode::WebView { src, style } => {
            let modifier = compose_modifier_chain(base_modifier, style.as_ref())
                .unwrap_or_else(|| "Modifier".to_string());
            let src = KOTLIN.escape(src);
            format!(
                "{pad}AndroidView(factory = {{ context -> WebView(context).apply {{ webViewClient = WebViewClient(); settings.javaScriptEnabled = true; loadUrl(\"{src}\") }} }}, update = {{ webView -> if (webView.url != \"{src}\") webView.loadUrl(\"{src}\") }}, modifier = {modifier})"
            )
        }
        ViewNode::Scroll {
            axis,
            style,
            children,
        } => {
            let base = match axis {
                StackAxis::Row => "Modifier.horizontalScroll(rememberScrollState())",
                StackAxis::Column => "Modifier.verticalScroll(rememberScrollState())",
            };
            let modifier = compose_modifier_chain(Some(base.to_string()), style.as_ref());
            let view = match axis {
                StackAxis::Row => "Row",
                StackAxis::Column => "Column",
            };
            let inner = compose_children(children, indent + 1, scope_name, scope_var);
            format!(
                "{pad}{view}(modifier = {}) {{\n{inner}\n{pad}}}",
                modifier.unwrap_or_else(|| "Modifier".to_string())
            )
        }
        ViewNode::List {
            ordered,
            style,
            children,
        } => {
            let modifier = compose_modifier_call_args(style.as_ref());
            let rows = children
                .iter()
                .enumerate()
                .map(|(idx, child)| {
                    if *ordered {
                        format!(
                            "{}item {{\n{}Row {{\n{}Text(\"{}.\")\n{}\n{}}}\n{}}}",
                            indent_str(indent + 1),
                            indent_str(indent + 2),
                            indent_str(indent + 3),
                            idx + 1,
                            compose_node(child, indent + 3, scope_name, scope_var),
                            indent_str(indent + 2),
                            indent_str(indent + 1)
                        )
                    } else {
                        format!(
                            "{}item {{\n{}\n{}}}",
                            indent_str(indent + 1),
                            compose_node(child, indent + 2, scope_name, scope_var),
                            indent_str(indent + 1)
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("{pad}LazyColumn{modifier} {{\n{rows}\n{pad}}}")
        }
        ViewNode::ListItem { children, .. } => {
            let inner = compose_children(children, indent + 1, scope_name, scope_var);
            format!("{pad}Column {{\n{inner}\n{pad}}}")
        }
        ViewNode::Link { children, .. } => {
            let inner = compose_children(children, indent + 1, scope_name, scope_var);
            format!("{pad}Column {{\n{inner}\n{pad}}}")
        }
        ViewNode::SlotRotate { phrases, style, .. } => {
            let args = compose_text_args(style.as_ref());
            format!(
                "{pad}Text(\"{}\"{args})",
                KOTLIN.escape(phrases.first().map(String::as_str).unwrap_or(""))
            )
        }
        ViewNode::Input {
            placeholder,
            bind,
            secure,
            on_change,
            style,
            ..
        } => {
            let on_value_change = compose_change(on_change.as_deref(), bind, "JsonPrimitive(it)");
            let modifier = compose_modifier_param(style.as_ref());
            let secure_arg = if *secure {
                ", visualTransformation = PasswordVisualTransformation()"
            } else {
                ""
            };
            format!(
                "{pad}TextField(value = {}, onValueChange = {{ {on_value_change} }}, placeholder = {{ Text(\"{}\") }}{secure_arg}{modifier})",
                KOTLIN.model_text(bind, scope_name, scope_var),
                KOTLIN.escape(placeholder),
            )
        }
        ViewNode::Picker {
            bind,
            options,
            on_change,
            style,
            ..
        } => {
            let modifier = compose_modifier_call_args(style.as_ref());
            let current = KOTLIN.model_text(bind, scope_name, scope_var);
            let inner = options
                .iter()
                .map(|option| {
                    let click = compose_change(
                        on_change.as_deref(),
                        bind,
                        &format!("JsonPrimitive(\"{}\")", KOTLIN.escape(&option.value)),
                    );
                    format!(
                        "{}FilterChip(selected = {current} == \"{}\", onClick = {{ {click} }}, label = {{ Text(\"{}\") }})",
                        indent_str(indent + 1),
                        KOTLIN.escape(&option.value),
                        KOTLIN.escape(&option.label)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("{pad}FlowRow{modifier} {{\n{inner}\n{pad}}}")
        }
        ViewNode::Tabs {
            bind,
            tabs,
            on_change,
            style,
            ..
        } => {
            let modifier = compose_modifier_param(style.as_ref());
            let current = KOTLIN.model_text(bind, scope_name, scope_var);
            let items = tabs
                .iter()
                .map(|tab| {
                    let click = compose_change(
                        on_change.as_deref(),
                        bind,
                        &format!("JsonPrimitive(\"{}\")", KOTLIN.escape(&tab.value)),
                    );
                    format!(
                        "{}NavigationBarItem(selected = {current} == \"{}\", onClick = {{ {click} }}, icon = {{}}, label = {{ Text(\"{}\") }})",
                        indent_str(indent + 3),
                        KOTLIN.escape(&tab.value),
                        KOTLIN.escape(&tab.label)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let pages = tabs
                .iter()
                .map(|tab| {
                    let content =
                        compose_children(&tab.children, indent + 3, scope_name, scope_var);
                    format!(
                        "{}if ({current} == \"{}\") {{\n{content}\n{}}}",
                        indent_str(indent + 2),
                        KOTLIN.escape(&tab.value),
                        indent_str(indent + 2)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "{pad}Scaffold(bottomBar = {{\n{}NavigationBar {{\n{items}\n{}}}\n{}}}{modifier}) {{ innerPadding ->\n{}Column(modifier = Modifier.padding(innerPadding)) {{\n{pages}\n{}}}\n{pad}}}",
                indent_str(indent + 1),
                indent_str(indent + 2),
                indent_str(indent + 1),
                indent_str(indent + 1),
                indent_str(indent + 2)
            )
        }
    }
}

fn compose_action(on_click: Option<&str>) -> String {
    on_click
        .map(|action| format!("CrepusActions.perform(\"{}\")", KOTLIN.escape(action)))
        .unwrap_or_default()
}

fn compose_change(on_change: Option<&str>, bind: &str, value: &str) -> String {
    let action = on_change
        .map(|action| format!("\"{}\"", KOTLIN.escape(action)))
        .unwrap_or_else(|| KOTLIN.null.to_string());
    format!(
        "CrepusActions.performChange({}, \"{}\", {})",
        action,
        KOTLIN.escape(bind),
        value
    )
}

fn compose_content_color_wrapper(out: String, style: Option<&ViewStyle>, indent: usize) -> String {
    let Some(color) = style
        .and_then(|style| style.foreground_color.as_deref())
        .map(compose_hex_argb)
    else {
        return out;
    };
    let pad = indent_str(indent);
    let inner = out
        .lines()
        .map(|line| format!("{}{}", indent_str(1), line))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{pad}CompositionLocalProvider(LocalContentColor provides Color(0x{color})) {{\n{inner}\n{pad}}}"
    )
}

fn compose_children(
    children: &[ViewNode],
    indent: usize,
    scope_name: Option<&str>,
    scope_var: Option<&str>,
) -> String {
    children
        .iter()
        .map(|child| compose_node(child, indent, scope_name, scope_var))
        .collect::<Vec<_>>()
        .join("\n")
}

fn compose_text_args(style: Option<&ViewStyle>) -> String {
    let mut args = Vec::new();
    if let Some(modifier) = compose_modifier(style) {
        args.push(format!("modifier = {modifier}"));
    }
    if let Some(style) = style {
        if let Some(size) = style.font_size {
            args.push(format!("fontSize = {size:.1}.sp"));
        }
        if let Some(weight) = style.font_weight {
            args.push(format!("fontWeight = {}", compose_font_weight(weight)));
        }
        if let Some(color) = &style.foreground_color {
            args.push(format!("color = Color(0x{})", compose_hex_argb(color)));
        }
        if let Some(align) = compose_text_align(style.text_align.as_deref()) {
            args.push(format!("textAlign = {align}"));
        }
        if style.italic == Some(true) {
            args.push("fontStyle = FontStyle.Italic".to_string());
        }
        if style.underline == Some(true) && style.strikethrough == Some(true) {
            args.push(
                "textDecoration = TextDecoration.combine(listOf(TextDecoration.Underline, TextDecoration.LineThrough))"
                    .to_string(),
            );
        } else if style.underline == Some(true) {
            args.push("textDecoration = TextDecoration.Underline".to_string());
        } else if style.strikethrough == Some(true) {
            args.push("textDecoration = TextDecoration.LineThrough".to_string());
        }
        if let Some(line_height) = style.line_height {
            args.push(format!("lineHeight = {:.1}.sp", line_height * 16.0));
        }
        if let Some(lines) = style.line_clamp {
            args.push(format!("maxLines = {lines}"));
        }
    }
    if args.is_empty() {
        String::new()
    } else {
        format!(", {}", args.join(", "))
    }
}

fn compose_modifier_call_args(style: Option<&ViewStyle>) -> String {
    compose_modifier(style)
        .map(|modifier| format!("(modifier = {modifier})"))
        .unwrap_or_else(|| "()".to_string())
}

fn compose_modifier_param(style: Option<&ViewStyle>) -> String {
    compose_modifier(style)
        .map(|modifier| format!(", modifier = {modifier}"))
        .unwrap_or_default()
}

fn compose_button_args(style: Option<&ViewStyle>) -> String {
    let mut args = Vec::new();
    if let Some(style) = style {
        let mut modifier_style = style.clone();
        modifier_style.background_color = None;
        modifier_style.foreground_color = None;
        modifier_style.padding = None;
        modifier_style.padding_horizontal = None;
        modifier_style.padding_vertical = None;
        modifier_style.padding_top = None;
        modifier_style.padding_bottom = None;
        modifier_style.padding_left = None;
        modifier_style.padding_right = None;
        if let Some(modifier) = compose_modifier(Some(&modifier_style)) {
            args.push(format!("modifier = {modifier}"));
        }
        if style.background_color.is_some() || style.foreground_color.is_some() {
            let container = style
                .background_color
                .as_deref()
                .map(compose_hex_argb)
                .unwrap_or_else(|| "FF6750A4".to_string());
            let content = style
                .foreground_color
                .as_deref()
                .map(compose_hex_argb)
                .unwrap_or_else(|| "FFFFFFFF".to_string());
            args.push(format!(
                "colors = ButtonDefaults.buttonColors(containerColor = Color(0x{container}), contentColor = Color(0x{content}))"
            ));
            args.push(
                "elevation = ButtonDefaults.buttonElevation(defaultElevation = 0.dp, pressedElevation = 0.dp, focusedElevation = 0.dp, hoveredElevation = 0.dp, disabledElevation = 0.dp)"
                    .to_string(),
            );
        }
        if let Some(radius) = style.corner_radius {
            args.push(format!("shape = RoundedCornerShape({radius:.0}.dp)"));
        }
        if let Some(padding) = compose_button_content_padding(style) {
            args.push(format!("contentPadding = {padding}"));
        }
    }
    if args.is_empty() {
        String::new()
    } else {
        format!(", {}", args.join(", "))
    }
}

fn compose_button_content_padding(style: &ViewStyle) -> Option<String> {
    if let Some(value) = style.padding {
        return Some(format!("PaddingValues({value:.0}.dp)"));
    }
    if style.padding_horizontal.is_some() || style.padding_vertical.is_some() {
        return Some(format!(
            "PaddingValues(horizontal = {:.0}.dp, vertical = {:.0}.dp)",
            style.padding_horizontal.unwrap_or(0.0),
            style.padding_vertical.unwrap_or(0.0)
        ));
    }
    if style.padding_top.is_some()
        || style.padding_bottom.is_some()
        || style.padding_left.is_some()
        || style.padding_right.is_some()
    {
        return Some(format!(
            "PaddingValues(start = {:.0}.dp, top = {:.0}.dp, end = {:.0}.dp, bottom = {:.0}.dp)",
            style.padding_left.unwrap_or(0.0),
            style.padding_top.unwrap_or(0.0),
            style.padding_right.unwrap_or(0.0),
            style.padding_bottom.unwrap_or(0.0)
        ));
    }
    None
}

fn compose_modifier(style: Option<&ViewStyle>) -> Option<String> {
    compose_modifier_chain(None, style)
}

fn compose_modifier_chain(base: Option<String>, style: Option<&ViewStyle>) -> Option<String> {
    let mut modifier = base.unwrap_or_else(|| "Modifier".to_string());
    let mut used = modifier != "Modifier";
    if let Some(style) = style {
        for value in compose_spacing_values(style, "margin") {
            modifier.push_str(&format!(".padding({value})"));
            used = true;
        }
        if style.width == Some(-1.0) && style.height == Some(-1.0) {
            modifier.push_str(".fillMaxSize()");
            used = true;
        } else {
            if style.width == Some(-1.0) || style.max_width == Some(-1.0) {
                modifier.push_str(".fillMaxWidth()");
                used = true;
            } else if let Some(width) = style.width.filter(|v| *v > 0.0) {
                modifier.push_str(&format!(".width({width:.0}.dp)"));
                used = true;
            }
            if style.min_width.is_some() || style.max_width.is_some() {
                modifier.push_str(&format!(
                    ".widthIn(min = {}, max = {})",
                    compose_dp_or_unspecified(style.min_width),
                    compose_dp_or_unspecified(style.max_width)
                ));
                used = true;
            }
            if style.height == Some(-1.0) || style.max_height == Some(-1.0) {
                modifier.push_str(".fillMaxHeight()");
                used = true;
            } else if let Some(height) = style.height.filter(|v| *v > 0.0) {
                modifier.push_str(&format!(".height({height:.0}.dp)"));
                used = true;
            }
            if style.min_height.is_some() || style.max_height.is_some() {
                modifier.push_str(&format!(
                    ".heightIn(min = {}, max = {})",
                    compose_dp_or_unspecified(style.min_height),
                    compose_dp_or_unspecified(style.max_height)
                ));
                used = true;
            }
        }
        if let Some(color) = &style.background_color {
            if let Some(radius) = style.corner_radius {
                modifier.push_str(&format!(".clip(RoundedCornerShape({radius:.0}.dp))"));
            }
            modifier.push_str(&format!(
                ".background(Color(0x{}))",
                compose_hex_argb(color)
            ));
            used = true;
        }
        if let Some(width) = style.border_width {
            let color = style
                .border_color
                .as_deref()
                .map(compose_hex_argb)
                .unwrap_or_else(|| "FF888888".to_string());
            let radius = style.corner_radius.unwrap_or(0.0);
            modifier.push_str(&format!(
                ".border({width:.0}.dp, Color(0x{color}), RoundedCornerShape({radius:.0}.dp))"
            ));
            used = true;
        }
        for value in compose_spacing_values(style, "padding") {
            modifier.push_str(&format!(".padding({value})"));
            used = true;
        }
        if let Some(opacity) = style.opacity {
            modifier.push_str(&format!(".alpha({opacity:.3}f)"));
            used = true;
        }
        if style.hidden == Some(true) {
            modifier.push_str(".alpha(0f)");
            used = true;
        }
        if style.translate_x.is_some() || style.translate_y.is_some() {
            modifier.push_str(&format!(
                ".offset(x = {:.0}.dp, y = {:.0}.dp)",
                style.translate_x.unwrap_or(0.0),
                style.translate_y.unwrap_or(0.0)
            ));
            used = true;
        }
        if let Some(rotate) = style.rotate {
            modifier.push_str(&format!(".rotate({rotate:.1}f)"));
            used = true;
        }
        if style.scale_x.is_some() || style.scale_y.is_some() {
            modifier.push_str(&format!(
                ".scale(scaleX = {:.3}f, scaleY = {:.3}f)",
                style.scale_x.unwrap_or(1.0),
                style.scale_y.unwrap_or(1.0)
            ));
            used = true;
        }
    }
    used.then_some(modifier)
}

fn compose_dp_or_unspecified(value: Option<f32>) -> String {
    value
        .filter(|value| *value > 0.0)
        .map(|value| format!("{value:.0}.dp"))
        .unwrap_or_else(|| "androidx.compose.ui.unit.Dp.Unspecified".to_string())
}

fn compose_spacing_values(style: &ViewStyle, kind: &str) -> Vec<String> {
    let (all, horizontal, vertical, top, bottom, left, right) = if kind == "padding" {
        (
            style.padding,
            style.padding_horizontal,
            style.padding_vertical,
            style.padding_top,
            style.padding_bottom,
            style.padding_left,
            style.padding_right,
        )
    } else {
        (
            style.margin,
            style.margin_horizontal,
            style.margin_vertical,
            style.margin_top,
            style.margin_bottom,
            style.margin_left,
            style.margin_right,
        )
    };
    let mut out = Vec::new();
    if let Some(value) = all {
        out.push(format!("{value:.0}.dp"));
    }
    if horizontal.is_some() || vertical.is_some() {
        out.push(format!(
            "horizontal = {:.0}.dp, vertical = {:.0}.dp",
            horizontal.unwrap_or(0.0),
            vertical.unwrap_or(0.0)
        ));
    }
    if top.is_some() || bottom.is_some() || left.is_some() || right.is_some() {
        out.push(format!(
            "start = {:.0}.dp, top = {:.0}.dp, end = {:.0}.dp, bottom = {:.0}.dp",
            left.unwrap_or(0.0),
            top.unwrap_or(0.0),
            right.unwrap_or(0.0),
            bottom.unwrap_or(0.0)
        ));
    }
    out
}

fn compose_text_align(value: Option<&str>) -> Option<&'static str> {
    match value {
        Some("center") => Some("TextAlign.Center"),
        Some("right") | Some("end") | Some("trailing") => Some("TextAlign.End"),
        Some("justify") => Some("TextAlign.Justify"),
        Some("left") | Some("start") | Some("leading") => Some("TextAlign.Start"),
        _ => None,
    }
}

fn compose_font_weight(weight: u16) -> &'static str {
    match weight {
        0..=299 => "FontWeight.Thin",
        300..=399 => "FontWeight.Light",
        400..=499 => "FontWeight.Normal",
        500..=599 => "FontWeight.Medium",
        600..=699 => "FontWeight.SemiBold",
        700..=799 => "FontWeight.Bold",
        _ => "FontWeight.ExtraBold",
    }
}

fn compose_hex_argb(color: &str) -> String {
    let trimmed = color.trim_start_matches('#');
    match trimmed.len() {
        6 => format!("FF{}", trimmed.to_ascii_uppercase()),
        8 => trimmed.to_ascii_uppercase(),
        _ => "FF888888".to_string(),
    }
}
