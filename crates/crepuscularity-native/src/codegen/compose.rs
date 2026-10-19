use crate::ir::{StackAxis, ViewIr, ViewNode, ViewStyle};

use super::shared::{
    bool_literal, finite_opt, indent_str, nonnegative_opt, text_line_limit,
    vertical_divider_height, ScopeBinding, KOTLIN,
};

pub(super) fn generate_compose(ir: &ViewIr, view_name: &str) -> String {
    let mut body = compose_nodes(&ir.root, 1, &[]);
    let link_imports = if body.contains("__crepusUriHandler.openUri(") {
        body = format!("    val __crepusUriHandler = LocalUriHandler.current\n{body}");
        "import androidx.compose.foundation.clickable\nimport androidx.compose.ui.platform.LocalUriHandler\n"
    } else {
        ""
    };
    let webview_imports =
        "import android.webkit.WebView\nimport android.webkit.WebViewClient\nimport androidx.compose.ui.viewinterop.AndroidView\n";
    let source = format!(
        "import androidx.compose.foundation.background\nimport androidx.compose.foundation.border\nimport androidx.compose.foundation.horizontalScroll\nimport androidx.compose.foundation.layout.Arrangement\nimport androidx.compose.foundation.layout.Column\nimport androidx.compose.foundation.layout.ExperimentalLayoutApi\nimport androidx.compose.foundation.layout.FlowRow\nimport androidx.compose.foundation.layout.PaddingValues\nimport androidx.compose.foundation.layout.Row\nimport androidx.compose.foundation.layout.Spacer\nimport androidx.compose.foundation.layout.fillMaxHeight\nimport androidx.compose.foundation.layout.fillMaxSize\nimport androidx.compose.foundation.layout.fillMaxWidth\nimport androidx.compose.foundation.layout.height\nimport androidx.compose.foundation.layout.heightIn\nimport androidx.compose.foundation.layout.offset\nimport androidx.compose.foundation.layout.padding\nimport androidx.compose.foundation.layout.width\nimport androidx.compose.foundation.layout.widthIn\nimport androidx.compose.foundation.lazy.LazyColumn\nimport androidx.compose.foundation.rememberScrollState\nimport androidx.compose.foundation.shape.RoundedCornerShape\nimport androidx.compose.foundation.verticalScroll\nimport androidx.compose.material3.Button\nimport androidx.compose.material3.ButtonDefaults\nimport androidx.compose.material3.HorizontalDivider\nimport androidx.compose.material3.VerticalDivider\nimport androidx.compose.material3.FilterChip\nimport androidx.compose.material3.LinearProgressIndicator\nimport androidx.compose.material3.LocalContentColor\nimport androidx.compose.material3.NavigationBar\nimport androidx.compose.material3.NavigationBarItem\nimport androidx.compose.material3.Scaffold\nimport androidx.compose.material3.Slider\nimport androidx.compose.material3.Switch\nimport androidx.compose.material3.Text\nimport androidx.compose.material3.TextField\nimport androidx.compose.runtime.Composable\nimport androidx.compose.runtime.CompositionLocalProvider\nimport androidx.compose.ui.Alignment\nimport androidx.compose.ui.Modifier\nimport androidx.compose.ui.draw.alpha\nimport androidx.compose.ui.draw.clip\nimport androidx.compose.ui.draw.rotate\nimport androidx.compose.ui.draw.scale\nimport androidx.compose.ui.graphics.Color\nimport androidx.compose.ui.text.font.FontFamily\nimport androidx.compose.ui.text.font.FontStyle\nimport androidx.compose.ui.text.font.FontWeight\nimport androidx.compose.ui.text.input.PasswordVisualTransformation\nimport androidx.compose.ui.text.style.TextAlign\nimport androidx.compose.ui.text.style.TextDecoration\nimport androidx.compose.ui.text.style.TextOverflow\nimport androidx.compose.ui.unit.dp\nimport androidx.compose.ui.unit.sp\nimport kotlinx.serialization.json.JsonElement\nimport kotlinx.serialization.json.JsonObject\nimport kotlinx.serialization.json.JsonPrimitive\n\nobject CrepusActions {{\n    private val executor: java.util.concurrent.ExecutorService =\n        java.util.concurrent.Executors.newSingleThreadExecutor {{ task ->\n            Thread(task, \"crepus-actions\").apply {{ isDaemon = true }}\n        }}\n    var dispatch: (String) -> String = {{ \"{{}}\" }}\n    var resultSink: (String) -> Unit = {{}}\n\n    fun applyResult(raw: String) {{\n        CrepusStateStore.applyResult(raw)\n    }}\n\n    fun perform(action: String) {{\n        val dispatch = dispatch\n        val resultSink = resultSink\n        executor.execute {{\n            val result = try {{\n                dispatch(action)\n            }} catch (error: Exception) {{\n                CrepusRustActions.queuedChangeError(error)\n            }}\n            val delivered = java.util.concurrent.CountDownLatch(1)\n            check(android.os.Handler(android.os.Looper.getMainLooper()).post {{\n                try {{\n                    resultSink(result)\n                }} finally {{\n                    delivered.countDown()\n                }}\n            }}) {{ \"main looper unavailable\" }}\n            awaitDelivery(delivered)\n        }}\n    }}\n\n    private fun awaitDelivery(delivered: java.util.concurrent.CountDownLatch) {{\n        var interrupted = false\n        while (true) {{\n            try {{\n                delivered.await()\n                break\n            }} catch (_: InterruptedException) {{\n                interrupted = true\n            }}\n        }}\n        if (interrupted) Thread.currentThread().interrupt()\n    }}\n\n    fun performChange(action: String?, bind: String, value: JsonElement) {{\n        val resultSink = resultSink\n        val snapshot = runCatching {{\n            val valueJson = value.toString()\n            valueJson to CrepusStateStore.snapshotDraft(valueJson)\n        }}.getOrElse {{ error ->\n            resultSink(CrepusRustActions.queuedChangeError(error))\n            return\n        }}\n        val valueJson = snapshot.first\n        val sequence = CrepusStateStore.beginDraft(bind, snapshot.second)\n        executor.execute {{\n            val result = CrepusRustActions.dispatchQueuedChange(action ?: \"\", bind, valueJson)\n            val delivered = java.util.concurrent.CountDownLatch(1)\n            check(android.os.Handler(android.os.Looper.getMainLooper()).post {{\n                try {{\n                    resultSink(result)\n                }} finally {{\n                    try {{\n                        CrepusStateStore.clearDraft(bind, sequence)\n                    }} finally {{\n                        delivered.countDown()\n                    }}\n                }}\n            }}) {{ \"main looper unavailable\" }}\n            awaitDelivery(delivered)\n        }}\n    }}\n}}\n\n@OptIn(ExperimentalLayoutApi::class)\n@Composable\nfun {view_name}(modifier: Modifier = Modifier) {{\n{body}\n}}\n"
    );
    let source = if body.contains(".crepusMargin(") {
        let imports = "import androidx.compose.ui.layout.LayoutModifier\nimport androidx.compose.ui.layout.Measurable\nimport androidx.compose.ui.layout.MeasureResult\nimport androidx.compose.ui.layout.MeasureScope\nimport androidx.compose.ui.unit.Constraints\nimport androidx.compose.ui.unit.constrainWidth\nimport androidx.compose.ui.unit.constrainHeight\nimport androidx.compose.ui.unit.Dp\nimport androidx.compose.ui.unit.LayoutDirection\nimport kotlin.math.roundToInt\n";
        let helper = include_str!("compose_margin.kt");
        format!("{imports}{source}\n{helper}")
    } else {
        source
    };
    if body.contains(".crepusLongPress(") {
        let imports = "import androidx.compose.foundation.gestures.awaitEachGesture\nimport androidx.compose.foundation.gestures.awaitFirstDown\nimport androidx.compose.foundation.gestures.awaitLongPressOrCancellation\nimport androidx.compose.ui.input.pointer.PointerEventPass\nimport androidx.compose.ui.input.pointer.pointerInput\nimport androidx.compose.ui.semantics.onLongClick\nimport androidx.compose.ui.semantics.semantics\n";
        let helper = r#"
private fun Modifier.crepusLongPress(action: String, allowConsumedDown: Boolean = false): Modifier = semantics {
    onLongClick { CrepusActions.perform(action); true }
}.pointerInput(action) {
    awaitEachGesture {
        // Native controls consume their own down event before this modifier's Main pass.
        val down = awaitFirstDown(requireUnconsumed = !allowConsumedDown)
        val longPress = awaitLongPressOrCancellation(down.id)
        if (longPress != null && (allowConsumedDown || !longPress.isConsumed)) {
            longPress.consume()
            CrepusActions.perform(action)
            do {
                val event = awaitPointerEvent(PointerEventPass.Initial)
                event.changes.forEach { it.consume() }
            } while (event.changes.any { it.pressed })
        }
    }
}
"#;
        format!("{webview_imports}{link_imports}{imports}{source}{helper}")
    } else {
        format!("{webview_imports}{link_imports}{source}")
    }
}

fn compose_nodes(nodes: &[ViewNode], indent: usize, scopes: &[ScopeBinding]) -> String {
    if nodes.len() == 1 {
        compose_node_with_base(&nodes[0], indent, Some("modifier".to_string()), scopes)
    } else {
        let pad = indent_str(indent);
        let inner = compose_children(nodes, indent + 1, scopes);
        format!("{pad}Column(modifier = modifier) {{\n{inner}\n{pad}}}")
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
        format!("Arrangement.spacedBy({}, {alignment})", compose_dp(spacing))
    } else {
        format!("Arrangement.spacedBy({})", compose_dp(spacing))
    }
}

fn compose_node(node: &ViewNode, indent: usize, scopes: &[ScopeBinding]) -> String {
    compose_node_with_base(node, indent, None, scopes)
}

fn compose_node_with_base(
    node: &ViewNode,
    indent: usize,
    base_modifier: Option<String>,
    scopes: &[ScopeBinding],
) -> String {
    let pad = indent_str(indent);
    if let Some(base) = &base_modifier {
        if let ViewNode::If { style, .. } | ViewNode::ForEach { style, .. } = node {
            let modifier = compose_modifier_chain(Some(base.clone()), style.as_ref())
                .unwrap_or_else(|| base.clone());
            let inner = compose_node(node, indent + 1, scopes);
            let out = format!("{pad}Column(modifier = {modifier}) {{\n{inner}\n{pad}}}");
            return compose_content_color_wrapper(out, style.as_ref(), indent);
        }
    }
    let on_long_press = match node {
        ViewNode::Stack { on_long_press, .. }
        | ViewNode::Scroll { on_long_press, .. }
        | ViewNode::Button { on_long_press, .. }
        | ViewNode::Toggle { on_long_press, .. }
        | ViewNode::Image { on_long_press, .. }
        | ViewNode::ListItem { on_long_press, .. } => on_long_press.as_deref(),
        _ => None,
    };
    let gesture_modifier = on_long_press.map(|action| {
        let control = if matches!(node, ViewNode::Button { .. } | ViewNode::Toggle { .. }) {
            ", true"
        } else {
            ""
        };
        format!(
            "{}.crepusLongPress(\"{}\"{control})",
            base_modifier.as_deref().unwrap_or("Modifier"),
            KOTLIN.escape(action)
        )
    });
    let base_modifier = gesture_modifier.clone().or(base_modifier);
    match node {
        ViewNode::Text {
            content,
            bind,
            style,
            ..
        } => {
            let args = compose_text_args_with_base(style.as_ref(), base_modifier.clone());
            let text = KOTLIN.text_content(content, bind.as_deref(), scopes);
            format!("{pad}Text({text}{args})")
        }
        ViewNode::If {
            condition,
            then_children,
            else_children,
            ..
        } => {
            let condition = KOTLIN.model_bool(condition, scopes);
            let then_inner = compose_children(then_children, indent + 1, scopes);
            if let Some(else_children) = else_children {
                let else_inner = compose_children(else_children, indent + 1, scopes);
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
            let binding = KOTLIN.loop_binding(item_name, scopes);
            let item_var = &binding.variable;
            let index_var =
                KOTLIN.identifier(&format!("{}__crepusIndex", item_var.trim_matches('`')));
            let items = KOTLIN.model_items(bind, scopes);
            let path_var = KOTLIN.binding_path_var(item_var);
            let path = KOTLIN.binding_path(bind, scopes);
            let mut child_scopes = scopes.to_vec();
            child_scopes.push(binding.clone());
            let inner = compose_children(item_body, indent + 1, &child_scopes);
            format!("{pad}{items}.forEachIndexed {{ {index_var}, {item_var} ->\n{}val {path_var} = {path} + \".\" + {index_var}.toString()\n{inner}\n{pad}}}", indent_str(indent + 1))
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
            let spacing = finite_opt(*spacing).unwrap_or(8.0);
            let arrangement = match axis {
                StackAxis::Row => "horizontalArrangement",
                StackAxis::Column => "verticalArrangement",
            };
            args.push(format!(
                "{arrangement} = {}",
                compose_arrangement(*axis, spacing, justify_content.as_deref())
            ));
            if wrap {
                args.push(format!(
                    "verticalArrangement = Arrangement.spacedBy({})",
                    compose_dp(spacing)
                ));
            }
            let args = args.join(", ");
            let inner = compose_children(children, indent + 1, scopes);
            let out = format!("{pad}{view}({args}) {{\n{inner}\n{pad}}}");
            compose_content_color_wrapper(out, style.as_ref(), indent)
        }
        ViewNode::Button {
            label,
            label_parts,
            on_click,
            style,
            ..
        } => {
            let modifier = compose_button_args(style.as_ref(), base_modifier);
            let action = compose_action(on_click.as_deref());
            format!(
                "{pad}Button(onClick = {{ {action} }}{modifier}) {{\n{}Text({})\n{pad}}}",
                indent_str(indent + 1),
                KOTLIN.label_text(label, label_parts, scopes)
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
            let modifier = if let Some(gesture) = gesture_modifier {
                compose_modifier_chain(Some(gesture), style.as_ref())
                    .map(|value| format!("(modifier = {value})"))
                    .unwrap_or_default()
            } else {
                compose_modifier_call_args(base_modifier.clone(), style.as_ref())
            };
            let checked_value = bind
                .as_deref()
                .map(|bind| compose_draft("Bool", bind, scopes))
                .unwrap_or_else(|| bool_literal(*checked).to_string());
            let on_change = bind
                .as_deref()
                .map(|bind| compose_change(on_change.as_deref(), bind, "JsonPrimitive(it)", scopes))
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
            step,
            on_change,
            style,
        } => {
            let (value, min, max, step) = super::shared::slider_values(*value, *min, *max, *step);
            let modifier = compose_modifier_call_args(base_modifier.clone(), style.as_ref());
            let steps = step
                .and_then(|step| {
                    let intervals = (max as f64 - min as f64) / step as f64;
                    let rounded = intervals.round();
                    // shortcut: limit native tick allocation; use custom snapping for larger grids.
                    ((2.0..=1000.0).contains(&rounded)
                        && (intervals - rounded).abs()
                            <= intervals.abs() * f32::EPSILON as f64 * 4.0)
                        .then(|| format!(", steps = {}", rounded as u32 - 1))
                })
                .unwrap_or_default();
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
                .map(|bind| {
                    format!(
                        "{}.coerceIn({min:?}f, {max:?}f)",
                        compose_draft("Number", bind, scopes)
                    )
                })
                .unwrap_or_else(|| format!("{value:?}f"));
            let on_value_change = bind
                .as_deref()
                .map(|bind| compose_change(on_change.as_deref(), bind, "JsonPrimitive(it)", scopes))
                .unwrap_or_else(|| compose_action(on_change.as_deref()));
            format!(
                "{pad}Column{modifier} {{\n{label}{}Slider(value = {slider_value}, onValueChange = {{ {on_value_change} }}, valueRange = {min:?}f..{max:?}f{steps})\n{pad}}}",
                indent_str(indent + 1)
            )
        }
        ViewNode::Progress {
            label,
            value,
            max,
            style,
        } => {
            let (value, max) = super::shared::progress_values(*value, *max);
            let modifier = compose_modifier_call_args(base_modifier.clone(), style.as_ref());
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
                "{pad}Column{modifier} {{\n{label}{}LinearProgressIndicator(progress = {value:?}f / {max:?}f)\n{pad}}}",
                indent_str(indent + 1)
            )
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
            compose_node_with_base(&progress, indent, base_modifier, scopes)
        }
        ViewNode::Badge { label, style, .. } => {
            let args = compose_text_args_with_base(style.as_ref(), base_modifier.clone());
            format!("{pad}Text(\"{}\"{args})", KOTLIN.escape(label))
        }
        ViewNode::Divider { axis, style } => {
            let (name, base) = match axis {
                StackAxis::Row => {
                    let height = vertical_divider_height(
                        style.as_ref().and_then(|style| style.height),
                        style.as_ref().and_then(|style| style.max_height),
                    );
                    let base = base_modifier.unwrap_or_else(|| "Modifier".to_string());
                    (
                        "VerticalDivider",
                        Some(match height {
                            Some(height) => format!("{base}.height({height:?}.dp)"),
                            None => base,
                        }),
                    )
                }
                StackAxis::Column => ("HorizontalDivider", base_modifier),
            };
            let modifier = compose_modifier_call_args(base, style.as_ref());
            format!("{pad}{name}{modifier}")
        }
        ViewNode::Spacer { size, style } => {
            let base = compose_modifier_chain(base_modifier, style.as_ref())
                .unwrap_or_else(|| "Modifier".to_string());
            format!(
                "{pad}Spacer(modifier = {base}.height({:?}.dp))",
                nonnegative_opt(*size).unwrap_or(8.0)
            )
        }
        ViewNode::Dropzone {
            label,
            style,
            children,
            ..
        } => {
            let modifier = compose_modifier_call_args(base_modifier.clone(), style.as_ref());
            let inner = if children.is_empty() {
                format!(
                    "{}Text(\"{}\")",
                    indent_str(indent + 1),
                    KOTLIN.escape(label)
                )
            } else {
                compose_children(children, indent + 1, scopes)
            };
            format!("{pad}Column{modifier} {{\n{inner}\n{pad}}}")
        }
        ViewNode::FilePicker {
            label,
            on_pick,
            style,
            ..
        } => {
            let modifier = compose_button_args(style.as_ref(), base_modifier);
            let action = compose_action(on_pick.as_deref());
            format!(
                "{pad}Button(onClick = {{ {action} }}{modifier}) {{\n{}Text(\"{}\")\n{pad}}}",
                indent_str(indent + 1),
                KOTLIN.escape(label)
            )
        }
        ViewNode::Image {
            src,
            src_bind,
            alt,
            style,
            ..
        } => {
            let args = compose_text_args_with_base(style.as_ref(), base_modifier);
            let value = if alt.is_none() {
                src_bind
                    .as_ref()
                    .map(|bind| KOTLIN.model_text(bind, scopes))
            } else {
                None
            }
            .unwrap_or_else(|| format!("\"{}\"", KOTLIN.escape(alt.as_deref().unwrap_or(src))));
            format!("{pad}Text({value}{args})")
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
            ..
        } => {
            let scroll = match axis {
                StackAxis::Row => "horizontalScroll",
                StackAxis::Column => "verticalScroll",
            };
            let base = format!(
                "{}.{scroll}(rememberScrollState())",
                base_modifier.as_deref().unwrap_or("Modifier")
            );
            let modifier = compose_modifier_chain(Some(base), style.as_ref());
            let view = match axis {
                StackAxis::Row => "Row",
                StackAxis::Column => "Column",
            };
            let inner = compose_children(children, indent + 1, scopes);
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
            let modifier = compose_modifier_call_args(base_modifier.clone(), style.as_ref());
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
                            compose_node(child, indent + 3, scopes),
                            indent_str(indent + 2),
                            indent_str(indent + 1)
                        )
                    } else {
                        format!(
                            "{}item {{\n{}\n{}}}",
                            indent_str(indent + 1),
                            compose_node(child, indent + 2, scopes),
                            indent_str(indent + 1)
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("{pad}LazyColumn{modifier} {{\n{rows}\n{pad}}}")
        }
        ViewNode::ListItem {
            style, children, ..
        } => {
            let inner = compose_children(children, indent + 1, scopes);
            let modifier = compose_modifier_call_args(base_modifier, style.as_ref());
            format!("{pad}Column{modifier} {{\n{inner}\n{pad}}}")
        }
        ViewNode::Link {
            href,
            href_bind,
            style,
            children,
            ..
        } => {
            let inner = compose_children(children, indent + 1, scopes);
            let base = base_modifier.as_deref().unwrap_or("Modifier");
            let clickable = if href_bind.is_none() && href.is_empty() {
                base.to_string()
            } else {
                let error =
                    KOTLIN.escape(r#"{"ok":false,"action":"openUri","error":"cannot open URI"}"#);
                if let Some(bind) = href_bind {
                    let value = KOTLIN.model_text(bind, scopes);
                    format!("{base}.clickable(enabled = {value}.isNotEmpty()) {{ try {{ __crepusUriHandler.openUri({value}) }} catch (_: IllegalArgumentException) {{ CrepusActions.resultSink(\"{error}\") }} }}")
                } else {
                    format!("{base}.clickable {{ try {{ __crepusUriHandler.openUri(\"{}\") }} catch (_: IllegalArgumentException) {{ CrepusActions.resultSink(\"{error}\") }} }}", KOTLIN.escape(href))
                }
            };
            let modifier = compose_modifier_chain(Some(clickable), style.as_ref())
                .unwrap_or_else(|| base.to_string());
            let out = format!("{pad}Column(modifier = {modifier}) {{\n{inner}\n{pad}}}");
            compose_content_color_wrapper(out, style.as_ref(), indent)
        }
        ViewNode::SlotRotate { phrases, style, .. } => {
            let args = compose_text_args_with_base(style.as_ref(), base_modifier.clone());
            format!(
                "{pad}Text(\"{}\"{args})",
                KOTLIN.escape(phrases.first().map(String::as_str).unwrap_or(""))
            )
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
                compose_change(on_change.as_deref(), bind, "JsonPrimitive(it)", scopes);
            if let Some(action) = on_input.as_deref() {
                if !on_value_change.is_empty() {
                    on_value_change.push_str("; ");
                }
                on_value_change.push_str(&compose_action(Some(action)));
            }
            let modifier = compose_modifier_param(base_modifier.clone(), style.as_ref());
            let secure_arg = if *secure {
                ", visualTransformation = PasswordVisualTransformation()"
            } else {
                ""
            };
            format!(
                "{pad}TextField(value = {}, onValueChange = {{ {on_value_change} }}, placeholder = {{ Text(\"{}\") }}, singleLine = {}{secure_arg}{modifier})",
                compose_draft("Text", bind, scopes),
                KOTLIN.escape(placeholder),
                bool_literal(!multiline),
            )
        }
        ViewNode::Picker {
            bind,
            options,
            on_change,
            style,
            ..
        } => {
            let modifier = compose_modifier_call_args(base_modifier.clone(), style.as_ref());
            let current = compose_draft("Text", bind, scopes);
            let inner = options
                .iter()
                .map(|option| {
                    let click = compose_change(
                        on_change.as_deref(),
                        bind,
                        &format!("JsonPrimitive(\"{}\")", KOTLIN.escape(&option.value)),
                     scopes);
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
            let modifier = compose_modifier_param(base_modifier.clone(), style.as_ref());
            let current = compose_draft("Text", bind, scopes);
            let items = tabs
                .iter()
                .map(|tab| {
                    let click = compose_change(
                        on_change.as_deref(),
                        bind,
                        &format!("JsonPrimitive(\"{}\")", KOTLIN.escape(&tab.value)),
                     scopes);
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
                    let content = compose_children(&tab.children, indent + 3, scopes);
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

fn compose_change(
    on_change: Option<&str>,
    bind: &str,
    value: &str,
    scopes: &[ScopeBinding],
) -> String {
    let bind = bind.trim();
    if bind.is_empty() {
        return compose_action(on_change);
    }
    let action = on_change
        .map(|action| format!("\"{}\"", KOTLIN.escape(action)))
        .unwrap_or_else(|| KOTLIN.null.to_string());
    format!(
        "CrepusActions.performChange({}, {}, {})",
        action,
        KOTLIN.binding_path(bind, scopes),
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

fn compose_children(children: &[ViewNode], indent: usize, scopes: &[ScopeBinding]) -> String {
    children
        .iter()
        .map(|child| compose_node(child, indent, scopes))
        .collect::<Vec<_>>()
        .join("\n")
}

fn compose_text_args_with_base(style: Option<&ViewStyle>, base: Option<String>) -> String {
    let mut args = Vec::new();
    if let Some(modifier) = compose_modifier_chain(base, style) {
        args.push(format!("modifier = {modifier}"));
    }
    if let Some(style) = style {
        if let Some(size) = finite_opt(style.font_size) {
            args.push(format!("fontSize = {size:?}.sp"));
        }
        if let Some(weight) = style.font_weight {
            args.push(format!("fontWeight = {}", compose_font_weight(weight)));
        }
        let family = match style.font_family.as_deref() {
            Some("sans") => Some("SansSerif"),
            Some("serif") => Some("Serif"),
            Some("mono") => Some("Monospace"),
            _ => None,
        };
        if let Some(family) = family {
            args.push(format!("fontFamily = FontFamily.{family}"));
        }
        if let Some(spacing) = finite_opt(style.letter_spacing) {
            args.push(format!("letterSpacing = {spacing:?}.sp"));
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
        if let Some(line_height) = finite_opt(style.line_height).and_then(|line_height| {
            finite_opt(Some(
                line_height * finite_opt(style.font_size).unwrap_or(16.0),
            ))
        }) {
            args.push(format!("lineHeight = {line_height:?}.sp"));
        }
        if let Some(lines) = text_line_limit(style) {
            args.push(format!("maxLines = {lines}"));
            let overflow = if style.text_overflow.as_deref() == Some("clip") {
                "Clip"
            } else {
                "Ellipsis"
            };
            args.push(format!("overflow = TextOverflow.{overflow}"));
        }
    }
    if args.is_empty() {
        String::new()
    } else {
        format!(", {}", args.join(", "))
    }
}

fn compose_modifier_call_args(base: Option<String>, style: Option<&ViewStyle>) -> String {
    compose_modifier_chain(base, style)
        .map(|modifier| format!("(modifier = {modifier})"))
        .unwrap_or_else(|| "()".to_string())
}

fn compose_modifier_param(base: Option<String>, style: Option<&ViewStyle>) -> String {
    compose_modifier_chain(base, style)
        .map(|modifier| format!(", modifier = {modifier}"))
        .unwrap_or_default()
}

fn compose_button_args(style: Option<&ViewStyle>, mut base: Option<String>) -> String {
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
        if let Some(modifier) = compose_modifier_chain(base.take(), Some(&modifier_style)) {
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
        if let Some(radius) = finite_opt(style.corner_radius) {
            args.push(format!(
                "shape = RoundedCornerShape({})",
                compose_dp(radius)
            ));
        }
        if let Some(padding) = compose_button_content_padding(style) {
            args.push(format!("contentPadding = {padding}"));
        }
    }
    if let Some(modifier) = base {
        args.push(format!("modifier = {modifier}"));
    }
    if args.is_empty() {
        String::new()
    } else {
        format!(", {}", args.join(", "))
    }
}

fn compose_button_content_padding(style: &ViewStyle) -> Option<String> {
    if let Some(value) = nonnegative_opt(style.padding) {
        return Some(format!("PaddingValues({})", compose_dp(value)));
    }
    let horizontal = nonnegative_opt(style.padding_horizontal);
    let vertical = nonnegative_opt(style.padding_vertical);
    if horizontal.is_some() || vertical.is_some() {
        return Some(format!(
            "PaddingValues(horizontal = {}, vertical = {})",
            compose_dp(horizontal.unwrap_or(0.0)),
            compose_dp(vertical.unwrap_or(0.0))
        ));
    }
    let top = nonnegative_opt(style.padding_top);
    let bottom = nonnegative_opt(style.padding_bottom);
    let left = nonnegative_opt(style.padding_left);
    let right = nonnegative_opt(style.padding_right);
    if top.is_some() || bottom.is_some() || left.is_some() || right.is_some() {
        return Some(format!(
            "PaddingValues(start = {}, top = {}, end = {}, bottom = {})",
            compose_dp(left.unwrap_or(0.0)),
            compose_dp(top.unwrap_or(0.0)),
            compose_dp(right.unwrap_or(0.0)),
            compose_dp(bottom.unwrap_or(0.0))
        ));
    }
    None
}

fn compose_modifier_chain(base: Option<String>, style: Option<&ViewStyle>) -> Option<String> {
    let mut modifier = base.unwrap_or_else(|| "Modifier".to_string());
    let mut used = modifier != "Modifier";
    if let Some(style) = style {
        for (value, negative) in compose_spacing_values(style, "margin") {
            let operation = if negative { "crepusMargin" } else { "padding" };
            modifier.push_str(&format!(".{operation}({value})"));
            used = true;
        }
        let width = finite_opt(style.width);
        let height = finite_opt(style.height);
        let min_width = finite_opt(style.min_width);
        let max_width = finite_opt(style.max_width);
        let min_height = finite_opt(style.min_height);
        let max_height = finite_opt(style.max_height);
        if width == Some(-1.0) && height == Some(-1.0) {
            modifier.push_str(".fillMaxSize()");
            used = true;
        } else {
            if width == Some(-1.0) || max_width == Some(-1.0) {
                modifier.push_str(".fillMaxWidth()");
                used = true;
            } else if let Some(width) = width.filter(|v| *v > 0.0) {
                modifier.push_str(&format!(".width({})", compose_dp(width)));
                used = true;
            }
            if min_width.is_some() || max_width.is_some() {
                modifier.push_str(&format!(
                    ".widthIn(min = {}, max = {})",
                    compose_dp_or_unspecified(min_width),
                    compose_dp_or_unspecified(max_width)
                ));
                used = true;
            }
            if height == Some(-1.0) || max_height == Some(-1.0) {
                modifier.push_str(".fillMaxHeight()");
                used = true;
            } else if let Some(height) = height.filter(|v| *v > 0.0) {
                modifier.push_str(&format!(".height({})", compose_dp(height)));
                used = true;
            }
            if min_height.is_some() || max_height.is_some() {
                modifier.push_str(&format!(
                    ".heightIn(min = {}, max = {})",
                    compose_dp_or_unspecified(min_height),
                    compose_dp_or_unspecified(max_height)
                ));
                used = true;
            }
        }
        if let Some(color) = &style.background_color {
            if let Some(radius) = finite_opt(style.corner_radius) {
                modifier.push_str(&format!(
                    ".clip(RoundedCornerShape({}))",
                    compose_dp(radius)
                ));
            }
            modifier.push_str(&format!(
                ".background(Color(0x{}))",
                compose_hex_argb(color)
            ));
            used = true;
        }
        if let Some(width) = finite_opt(style.border_width) {
            let color = style
                .border_color
                .as_deref()
                .map(compose_hex_argb)
                .unwrap_or_else(|| "FF888888".to_string());
            let radius = finite_opt(style.corner_radius).unwrap_or(0.0);
            modifier.push_str(&format!(
                ".border({}, Color(0x{color}), RoundedCornerShape({}))",
                compose_dp(width),
                compose_dp(radius)
            ));
            used = true;
        }
        for (value, _) in compose_spacing_values(style, "padding") {
            modifier.push_str(&format!(".padding({value})"));
            used = true;
        }
        if let Some(opacity) = finite_opt(style.opacity) {
            modifier.push_str(&format!(".alpha({opacity:.3}f)"));
            used = true;
        }
        if style.hidden == Some(true) {
            modifier.push_str(".alpha(0f)");
            used = true;
        }
        let translate_x = finite_opt(style.translate_x);
        let translate_y = finite_opt(style.translate_y);
        if translate_x.is_some() || translate_y.is_some() {
            modifier.push_str(&format!(
                ".offset(x = {}, y = {})",
                compose_dp(translate_x.unwrap_or(0.0)),
                compose_dp(translate_y.unwrap_or(0.0))
            ));
            used = true;
        }
        if let Some(rotate) = finite_opt(style.rotate) {
            modifier.push_str(&format!(".rotate({rotate:.1}f)"));
            used = true;
        }
        let scale_x = finite_opt(style.scale_x);
        let scale_y = finite_opt(style.scale_y);
        if scale_x.is_some() || scale_y.is_some() {
            modifier.push_str(&format!(
                ".scale(scaleX = {:.3}f, scaleY = {:.3}f)",
                scale_x.unwrap_or(1.0),
                scale_y.unwrap_or(1.0)
            ));
            used = true;
        }
    }
    used.then_some(modifier)
}

// Kotlin has no Long.dp; large integral values need a floating-point literal.
fn compose_dp(value: f32) -> String {
    let number_value = f64::from(value);
    let number = if number_value >= f64::from(i32::MIN) && number_value <= f64::from(i32::MAX) {
        value.to_string()
    } else {
        format!("{value:?}")
    };
    format!("{number}.dp")
}

fn compose_dp_or_unspecified(value: Option<f32>) -> String {
    finite_opt(value)
        .filter(|value| *value > 0.0)
        .map(compose_dp)
        .unwrap_or_else(|| "androidx.compose.ui.unit.Dp.Unspecified".to_string())
}

fn compose_spacing_values(style: &ViewStyle, kind: &str) -> Vec<(String, bool)> {
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
    let filter: fn(Option<f32>) -> Option<f32> = if kind == "padding" {
        nonnegative_opt
    } else {
        finite_opt
    };
    let all = filter(all);
    let horizontal = filter(horizontal);
    let vertical = filter(vertical);
    let top = filter(top);
    let bottom = filter(bottom);
    let left = filter(left);
    let right = filter(right);
    let mut out = Vec::new();
    if let Some(value) = all {
        out.push((compose_dp(value), value < 0.0));
    }
    if horizontal.is_some() || vertical.is_some() {
        out.push((
            format!(
                "horizontal = {}, vertical = {}",
                compose_dp(horizontal.unwrap_or(0.0)),
                compose_dp(vertical.unwrap_or(0.0))
            ),
            horizontal.is_some_and(|v| v < 0.0) || vertical.is_some_and(|v| v < 0.0),
        ));
    }
    if top.is_some() || bottom.is_some() || left.is_some() || right.is_some() {
        out.push((
            format!(
                "start = {}, top = {}, end = {}, bottom = {}",
                compose_dp(left.unwrap_or(0.0)),
                compose_dp(top.unwrap_or(0.0)),
                compose_dp(right.unwrap_or(0.0)),
                compose_dp(bottom.unwrap_or(0.0))
            ),
            [left, top, right, bottom]
                .into_iter()
                .flatten()
                .any(|v| v < 0.0),
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
        0..=199 => "FontWeight.Thin",
        200..=299 => "FontWeight.ExtraLight",
        300..=399 => "FontWeight.Light",
        400..=499 => "FontWeight.Normal",
        500..=599 => "FontWeight.Medium",
        600..=699 => "FontWeight.SemiBold",
        700..=799 => "FontWeight.Bold",
        800..=899 => "FontWeight.ExtraBold",
        _ => "FontWeight.Black",
    }
}

fn compose_hex_argb(color: &str) -> String {
    let trimmed = color.trim_start_matches('#');
    match trimmed.len() {
        6 if trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) => {
            format!("FF{}", trimmed.to_ascii_uppercase())
        }
        8 if trimmed.bytes().all(|byte| byte.is_ascii_hexdigit()) => {
            format!("{}{}", &trimmed[6..], &trimmed[..6]).to_ascii_uppercase()
        }
        _ => crate::colors::resolve_rgba(color)
            .map(|[r, g, b, a]| {
                format!(
                    "{:02X}{:02X}{:02X}{:02X}",
                    (a * 255.0).round() as u8,
                    (r * 255.0).round() as u8,
                    (g * 255.0).round() as u8,
                    (b * 255.0).round() as u8
                )
            })
            .unwrap_or_else(|| "FF888888".to_string()),
    }
}

fn compose_draft(kind: &str, bind: &str, scopes: &[ScopeBinding]) -> String {
    let bind = bind.trim();
    let fallback = match kind {
        "Bool" => KOTLIN.model_bool(bind, scopes),
        "Number" => KOTLIN.model_number(bind, scopes),
        _ => KOTLIN.model_text(bind, scopes),
    };
    if bind.is_empty() {
        return fallback;
    }
    format!(
        "CrepusStateStore.draft{kind}({}) {{ {fallback} }}",
        KOTLIN.binding_path(bind, scopes)
    )
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
            let output = compose_node(&node, 0, &scopes);
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
        let output = compose_node(&node, 0, &[]);
        assert!(output.contains(".text(\"url\""), "{output}");
        assert!(
            output.contains("isNotEmpty") || output.contains("!crepusHref.isEmpty"),
            "{output}"
        );
    }
}
