use std::collections::HashMap;
use std::fs;

use crepuscularity_core::context::TemplateContext;
use serde_json::json;

use crate::{
    apply_mutations, ast_shape_compatible, diff_ir, generate_native_source, plan_hot_reload,
    render_from_files, render_template_to_ir, to_json, HotReloadMessage, NativeCodegenTarget,
    ViewIr, IR_VERSION,
};

#[test]
fn plain_text_stack() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "Ada");
    let tpl = "div flex flex-col gap-4\n  span\n    \"Hello {name}\"";
    let ir = render_template_to_ir(tpl, &ctx).unwrap();
    assert_eq!(ir.version, IR_VERSION);

    // flex-col explicitly sets flexDirection so web consumers see it alongside axis.
    let expected = json!({
        "version": IR_VERSION,
        "root": [{
            "kind": "stack",
            "axis": "column",
            "spacing": 16.0,
            "style": {
                "classes": ["flex", "flex-col", "gap-4"],
                "flexDirection": "column"
            },
            "children": [{
                "kind": "text",
                "content": "Hello Ada"
            }]
        }]
    });
    let v: serde_json::Value = serde_json::to_value(&ir).unwrap();
    assert_eq!(v, expected);
}

#[test]
fn webview_embeds_lower_to_native_ir() {
    let embed = render_template_to_ir(
        "embed https://example.com adapter=\"webview\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let iframe = render_template_to_ir(
        "iframe w-full h-64 src=\"https://example.com/frame\"",
        &TemplateContext::new(),
    )
    .unwrap();

    assert_eq!(
        to_json(&embed).unwrap(),
        format!(
            r#"{{"version":{IR_VERSION},"root":[{{"kind":"webView","src":"https://example.com"}}]}}"#
        )
    );
    assert!(matches!(
        &iframe.root[0],
        crate::ViewNode::WebView { src, style: Some(_) } if src == "https://example.com/frame"
    ));
}

#[test]
fn codegen_swiftui_emits_standalone_view_source() {
    let ir = render_template_to_ir(
        "div flex flex-col gap-4 p-4 bg-blue-500\n  span text-lg font-bold text-white\n    \"Hello Ada\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let source = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "HelloScreen");
    assert!(source.contains("import SwiftUI"));
    assert!(source.contains("#if canImport(UIKit)"));
    assert!(source.contains("#if canImport(AppKit)"));
    assert!(source.contains("public enum CrepusActions"));
    assert!(source.contains("public static var dispatch: (String) -> String"));
    assert!(source.contains("public static func perform(_ action: String)"));
    assert!(source.contains("public static func dismissKeyboard()"));
    assert!(source.contains("UIApplication.shared.sendAction"));
    assert!(source.contains("NSApp.keyWindow?.makeFirstResponder(nil)"));
    assert!(source.contains("DispatchQueue.global(qos: .userInitiated).async"));
    assert!(source.contains("DispatchQueue.main.async"));
    assert!(!source.contains("knownActions"));
    assert!(source.contains("public struct HelloScreen: View"));
    assert!(source.contains("VStack(alignment: .leading, spacing: 16.0)"));
    assert!(source.contains(".onTapGesture { CrepusActions.dismissKeyboard() }"));
    assert!(source.contains("Text(\"Hello Ada\")"));
    assert!(source.contains(".fontWeight(.bold)"));
    assert!(source.contains("Color(red:"));
}

#[test]
fn codegen_compose_emits_composable_source() {
    let ir = render_template_to_ir(
        "div flex flex-row gap-2 p-2\n  span text-lg font-bold\n    \"Hello Ada\"\n  button @click=\"tap\"\n    \"Tap\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "HelloScreen");
    assert!(source.contains("@Composable"));
    assert!(source.contains("fun HelloScreen(modifier: Modifier = Modifier)"));
    assert!(source.contains("Row(modifier = modifier.padding(8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp))"));
    assert!(
        source.contains("Text(\"Hello Ada\", fontSize = 18.0.sp, fontWeight = FontWeight.Bold)")
    );
    assert!(source.contains("object CrepusActions"));
    assert!(source.contains("var dispatch: (String) -> String"));
    assert!(source.contains("fun perform(action: String)"));
    assert!(source.contains("Thread {"));
    assert!(source.contains("android.os.Handler(android.os.Looper.getMainLooper()).post"));
    assert!(!source.contains("knownActions"));
    assert!(source.contains("Button(onClick = { CrepusActions.perform(\"tap\") })"));
}

#[test]
fn codegen_emits_native_webviews() {
    let ir = ViewIr {
        version: IR_VERSION,
        root: vec![crate::ViewNode::WebView {
            src: "https://example.com/embed".to_string(),
            style: None,
        }],
    };

    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "WebViewScreen");
    assert!(swift.contains("import WebKit"));
    assert!(swift.contains("public struct CrepusWebView: UIViewRepresentable"));
    assert!(swift.contains("CrepusWebView(source: \"https://example.com/embed\")"));
    assert!(swift.contains("webView.load(URLRequest(url: url))"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "WebViewScreen");
    assert!(compose.contains("import androidx.compose.ui.viewinterop.AndroidView"));
    assert!(compose.contains("import android.webkit.WebView"));
    assert!(compose.contains("settings.javaScriptEnabled = true"));
    assert!(compose.contains("loadUrl(\"https://example.com/embed\")"));
}

#[test]
fn codegen_wraps_native_rows() {
    let ir = render_template_to_ir(
        "div flex flex-wrap gap-2\n  button\n    \"One\"\n  button\n    \"Two\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "WrappedView");
    assert!(swift.contains("LazyVGrid(columns: [GridItem(.adaptive(minimum: 72), spacing: 8.0)]"));
    assert!(swift.contains("alignment: .leading"));
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "WrappedView");
    assert!(compose.contains("import androidx.compose.foundation.layout.FlowRow"));
    assert!(compose.contains("@OptIn(ExperimentalLayoutApi::class)"));
    assert!(compose.contains("FlowRow("));
    assert!(compose.contains("verticalArrangement = Arrangement.spacedBy(8.dp)"));
}

#[test]
fn codegen_honors_native_justify_center() {
    let ir = render_template_to_ir(
        "div flex flex-col justify-center h-full\n  span\n    \"Centered\"",
        &TemplateContext::new(),
    )
    .unwrap();

    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "CenteredView");
    assert!(!swift.contains("Spacer()"));
    assert!(swift.contains(".frame(maxWidth: nil, maxHeight: .infinity, alignment: .leading)"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "CenteredView");
    assert!(compose.contains("import androidx.compose.ui.Alignment"));
    assert!(compose
        .contains("verticalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterVertically)"));
}

#[test]
fn codegen_emits_native_lists() {
    let ir = render_template_to_ir(
        "ol\n  li\n    \"One\"\n  li\n    \"Two\"",
        &TemplateContext::new(),
    )
    .unwrap();

    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "ListView");
    assert!(swift.contains("List {"));
    assert!(swift.contains("Text(\"1.\")"));
    assert!(!swift.contains("VStack(alignment: .leading, spacing: 8.0)"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "ListView");
    assert!(compose.contains("import androidx.compose.foundation.lazy.LazyColumn"));
    assert!(compose.contains("LazyColumn"));
    assert!(compose.contains("item {"));
    assert!(compose.contains("Text(\"1.\")"));
}

#[test]
fn codegen_preserves_dynamic_state_bindings() {
    let ir = render_template_to_ir(
        "div\n if {count > 1}\n  span\n    \"Many\"\n else\n  span\n    \"One\"\n for hub in {hubs}\n  span\n    \"{hub.name}\"\n span\n  \"{pairing_code}\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "DynamicView");
    assert!(swift.contains("public static let model = CrepusStateStore.shared"));
    assert!(swift.contains("if CrepusActions.model.bool(\"count > 1\")"));
    assert!(swift.contains("CrepusActions.model.items(\"hubs\")"));
    assert!(
        swift.contains("CrepusActions.model.text(\"hub.name\", scopeName: \"hub\", scope: hub)")
    );
    assert!(swift.contains("CrepusActions.model.text(\"pairing_code\")"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "DynamicView");
    assert!(compose.contains("CrepusStateStore.applyResult(raw)"));
    assert!(compose.contains("if (CrepusStateStore.bool(\"count > 1\"))"));
    assert!(compose.contains("CrepusStateStore.items(\"hubs\").forEachIndexed"));
    assert!(
        compose.contains("CrepusStateStore.text(\"hub.name\", scopeName = \"hub\", scope = hub)")
    );
    assert!(compose.contains("CrepusStateStore.text(\"pairing_code\")"));
}

#[test]
fn empty_bound_element_renders_dynamic_text() {
    let ir = render_template_to_ir("div\n  div bind=last_action", &TemplateContext::new()).unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "BoundTextView");
    assert!(swift.contains("Text(CrepusActions.model.text(\"last_action\"))"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "BoundTextView");
    assert!(compose.contains("Text(CrepusStateStore.text(\"last_action\"))"));
}

#[test]
fn password_input_emits_secure_native_fields() {
    let ir = render_template_to_ir(
        "div\n  input type=password bind=setup_secret placeholder=\"Setup secret\"",
        &TemplateContext::new(),
    )
    .unwrap();

    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "SecretView");
    assert!(swift.contains("SecureField(\"Setup secret\", text: Binding"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "SecretView");
    assert!(compose.contains("visualTransformation = PasswordVisualTransformation()"));
}

#[test]
fn codegen_preserves_control_change_bindings() {
    let ir = render_template_to_ir(
        "div\n toggle bind=enabled @change=\"sync_enabled\"\n  \"Enabled\"\n slider bind=volume @change=\"sync_volume\" min=0 max=100\n input bind=name @change=\"sync_name\" placeholder=\"Name\"\n picker bind=mode @change=\"sync_mode\"\n  option value=\"auto\"\n    \"Auto\"\n  option value=\"manual\"\n    \"Manual\"",
        &TemplateContext::new(),
    )
    .unwrap();

    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "ControlsView");
    assert!(swift
        .contains("public static func performChange(_ action: String?, bind: String, value: Any)"));
    assert!(swift.contains(
        "resultSink(CrepusRustActions.dispatchChangeStored(action ?? \"\", bind: bind, value: value))"
    ));
    assert!(swift.contains(
        "CrepusActions.performChange(\"sync_enabled\", bind: \"enabled\", value: newValue)"
    ));
    assert!(swift.contains(
        "CrepusActions.performChange(\"sync_volume\", bind: \"volume\", value: newValue)"
    ));
    assert!(swift
        .contains("CrepusActions.performChange(\"sync_name\", bind: \"name\", value: newValue)"));
    assert!(swift
        .contains("CrepusActions.performChange(\"sync_mode\", bind: \"mode\", value: newValue)"));
    assert!(swift.contains(".pickerStyle(.segmented)"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "ControlsView");
    assert!(
        compose.contains("fun performChange(action: String?, bind: String, value: JsonElement)")
    );
    assert!(compose.contains(
        "resultSink(CrepusRustActions.dispatchChangeJson(action ?: \"\", bind, value.toString()))"
    ));
    assert!(compose
        .contains("CrepusActions.performChange(\"sync_enabled\", \"enabled\", JsonPrimitive(it))"));
    assert!(compose
        .contains("CrepusActions.performChange(\"sync_volume\", \"volume\", JsonPrimitive(it))"));
    assert!(
        compose.contains("CrepusActions.performChange(\"sync_name\", \"name\", JsonPrimitive(it))")
    );
    assert!(compose
        .contains("CrepusActions.performChange(\"sync_mode\", \"mode\", JsonPrimitive(\"auto\"))"));
    assert!(compose.contains("FilterChip(selected = CrepusStateStore.text(\"mode\") == \"auto\""));
}

#[test]
fn compose_button_colors_use_material_colors() {
    let ir = render_template_to_ir(
        "button @click=\"tap\" rounded-lg bg-[#ffffff] text-[#000000] px-6 py-5\n  \"Tap\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "HelloScreen");
    assert!(source.contains("import androidx.compose.material3.ButtonDefaults"));
    assert!(source.contains("import androidx.compose.foundation.layout.PaddingValues"));
    assert!(source.contains("contentPadding = PaddingValues(horizontal = 24.dp, vertical = 20.dp)"));
    assert!(source.contains("colors = ButtonDefaults.buttonColors(containerColor = Color(0xFFFFFFFF), contentColor = Color(0xFF000000))"));
    assert!(source.contains("elevation = ButtonDefaults.buttonElevation(defaultElevation = 0.dp, pressedElevation = 0.dp, focusedElevation = 0.dp, hoveredElevation = 0.dp, disabledElevation = 0.dp)"));
    assert!(source.contains("shape = RoundedCornerShape(8.dp)"));
    assert!(!source.contains("modifier = Modifier.padding(horizontal = 24.dp, vertical = 20.dp)"));
    assert!(!source.contains("Modifier.background(Color(0xFFFFFFFF))"));
}

#[test]
fn compose_named_black_and_white_colors() {
    let ir = render_template_to_ir(
        "div bg-black\n  span text-white\n    \"Named colors\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "HelloScreen");
    assert!(source.contains("background(Color(0xFF000000))"));
    assert!(source.contains("color = Color(0xFFFFFFFF)"));
}

#[test]
fn compose_stack_text_color_inherits_to_children() {
    let ir = render_template_to_ir(
        "div text-white\n  span\n    \"Inherited\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "HelloScreen");
    assert!(source.contains("import androidx.compose.material3.LocalContentColor"));
    assert!(source.contains("import androidx.compose.runtime.CompositionLocalProvider"));
    assert!(
        source.contains("CompositionLocalProvider(LocalContentColor provides Color(0xFFFFFFFF))")
    );
}

#[test]
fn native_registry_classes_emit_real_platform_styles() {
    let ir = render_template_to_ir(
        "div flex flex-col gap-2 bg-zinc-900 rounded-lg overflow-y-auto w-full max-w-[360px]\n  span text-xs text-zinc-100\n    \"Tiny\"\n  span text-sm\n    \"Small\"\n  span text-2xl font-semibold\n    \"Large\"",
        &TemplateContext::new(),
    )
    .unwrap();

    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "RegistryParityView");
    assert!(swift.contains("VStack(alignment: .leading, spacing: 8.0)"));
    assert!(swift.contains(".background(Color(red:"));
    assert!(swift.contains(".clipShape(RoundedRectangle(cornerRadius: 8.0))"));
    assert!(swift.contains(".frame(maxWidth: .infinity, maxHeight: nil, alignment: .topLeading)"));
    assert!(swift.contains(".frame(minWidth: nil, maxWidth: 360.0, minHeight: nil, maxHeight: nil, alignment: .topLeading)"));
    assert!(swift.contains(".font(.system(size: 12.0))"));
    assert!(swift.contains(".font(.system(size: 14.0))"));
    assert!(swift.contains(".font(.system(size: 24.0))"));
    assert!(swift.contains(".fontWeight(.semibold)"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "RegistryParityView");
    assert!(compose.contains(".clip(RoundedCornerShape(8.dp))"));
    assert!(compose.contains(".background(Color(0xFF18181B))"));
    assert!(
        compose.contains(".widthIn(min = androidx.compose.ui.unit.Dp.Unspecified, max = 360.dp)")
    );
    assert!(compose.contains("verticalArrangement = Arrangement.spacedBy(8.dp)"));
    assert!(compose.contains("color = Color(0xFFF4F4F5)"));
    assert!(compose.contains("fontSize = 12.0.sp"));
    assert!(compose.contains("fontSize = 14.0.sp"));
    assert!(compose.contains("fontSize = 24.0.sp"));
    assert!(compose.contains("fontWeight = FontWeight.SemiBold"));
}

#[test]
fn web_style_classes_lower_to_typed_native_codegen() {
    let ir = render_template_to_ir(
        "div flex flex-col w-full h-full px-4 pt-6 mb-2 bg-[#101624] border border-[#334155] rounded-xl opacity-75 shadow-lg translate-x-2 translate-y-1 rotate-6 scale-95 overflow-hidden\n  span text-center text-lg font-semibold italic underline line-through leading-tight line-clamp-2 text-[#f8fafc]\n    \"Literal web-style UI\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let value = serde_json::to_value(&ir).unwrap();
    let root_style = &value["root"][0]["style"];
    assert_eq!(root_style["width"], -1.0);
    assert_eq!(root_style["height"], -1.0);
    assert_eq!(root_style["paddingHorizontal"], 16.0);
    assert_eq!(root_style["paddingTop"], 24.0);
    assert_eq!(root_style["marginBottom"], 8.0);
    assert_eq!(root_style["borderWidth"], 1.0);
    assert_eq!(root_style["opacity"], 0.75);
    assert_eq!(root_style["translateX"], 8.0);
    assert_eq!(root_style["translateY"], 4.0);
    assert_eq!(root_style["rotate"], 6.0);
    assert!((root_style["scaleX"].as_f64().unwrap() - 0.95).abs() < 0.001);
    assert!((root_style["scaleY"].as_f64().unwrap() - 0.95).abs() < 0.001);
    let text_style = &value["root"][0]["children"][0]["style"];
    assert_eq!(text_style["textAlign"], "center");
    assert_eq!(text_style["italic"], true);
    assert_eq!(text_style["underline"], true);
    assert_eq!(text_style["strikethrough"], true);
    assert_eq!(text_style["lineClamp"], 2);

    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "WebParityView");
    assert!(swift.contains(".padding(.horizontal, 16)"));
    assert!(swift.contains(".padding(.top, 24)"));
    assert!(swift.contains(".padding(.bottom, 8)"));
    assert!(swift.contains(".opacity(0.750)"));
    assert!(swift.contains(".border(Color(red:"));
    assert!(swift.contains(".offset(x: 8.0, y: 4.0)"));
    assert!(swift.contains(".rotationEffect(.degrees(6.0))"));
    assert!(swift.contains(".scaleEffect(x: 0.950, y: 0.950)"));
    assert!(swift.contains(".multilineTextAlignment(.center)"));
    assert!(swift.contains(".italic()"));
    assert!(swift.contains(".underline()"));
    assert!(swift.contains(".strikethrough()"));
    assert!(swift.contains(".lineLimit(2)"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "WebParityView");
    assert!(compose.contains(".padding(horizontal = 16.dp, vertical = 0.dp)"));
    assert!(compose.contains(".padding(start = 0.dp, top = 24.dp, end = 0.dp, bottom = 0.dp)"));
    assert!(compose.contains(".padding(start = 0.dp, top = 0.dp, end = 0.dp, bottom = 8.dp)"));
    assert!(compose.contains(".border(1.dp, Color(0xFF334155), RoundedCornerShape(12.dp))"));
    assert!(compose.contains(".alpha(0.750f)"));
    assert!(compose.contains(".offset(x = 8.dp, y = 4.dp)"));
    assert!(compose.contains(".rotate(6.0f)"));
    assert!(compose.contains(".scale(scaleX = 0.950f, scaleY = 0.950f)"));
    assert!(compose.contains("textAlign = TextAlign.Center"));
    assert!(compose.contains("fontStyle = FontStyle.Italic"));
    assert!(compose.contains("TextDecoration.combine"));
    assert!(compose.contains("maxLines = 2"));
}

fn round_trip(ir: &ViewIr) {
    let s = to_json(ir).unwrap();
    let back: ViewIr = serde_json::from_str(&s).unwrap();
    assert_eq!(*ir, back);
}

#[test]
fn serde_round_trip() {
    let ir = render_template_to_ir(
        "div flex flex-row\n if {show}\n  \"yes\"\n else\n  \"no\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["children"][0]["kind"], "if");
    round_trip(&ir);
}

#[test]
fn for_loop() {
    let ir = render_template_to_ir(
        "div\n for item in {items}\n  span\n    \"{item}\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["children"][0]["kind"], "forEach");
    assert_eq!(v["root"][0]["children"][0]["bind"], "items");
    assert_eq!(v["root"][0]["children"][0]["itemName"], "item");
    assert_eq!(v["root"][0]["children"][0]["itemBody"][0]["bind"], "item");
    round_trip(&ir);
}

#[test]
fn include_virtual_file() {
    let mut ctx = TemplateContext::new();
    let mut files = HashMap::new();
    files.insert(
        "child.crepus".into(),
        "span text-green-400\n  \"In child\"".into(),
    );
    ctx.virtual_files = std::sync::Arc::new(files);
    let tpl = "include child.crepus";
    let ir = render_template_to_ir(tpl, &ctx).unwrap();
    let s = serde_json::to_string(&ir).unwrap();
    assert!(s.contains("In child"));
    assert!(s.contains("#05df72") || s.contains("green"));
}

#[test]
fn file_include_rejects_parent_dir() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("templates");
    fs::create_dir(&root).unwrap();
    fs::write(temp.path().join("secret.crepus"), "div\n  \"secret\"").unwrap();

    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(root);
    let err = render_template_to_ir("include ../secret.crepus", &ctx)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("include path outside base dir"),
        "expected base-dir rejection, got: {err}"
    );
}

#[test]
fn file_include_rejects_absolute_path() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("templates");
    fs::create_dir(&root).unwrap();
    let secret = temp.path().join("secret.crepus");
    fs::write(&secret, "div\n  \"secret\"").unwrap();

    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(root);
    let err = render_template_to_ir(&format!("include {}", secret.display()), &ctx)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("include path outside base dir"),
        "expected absolute-path rejection, got: {err}"
    );
}

#[test]
fn match_arm() {
    let mut ctx = TemplateContext::new();
    ctx.set("status", "on");
    let tpl = "div\n match {status}\n \"on\" =>\n  \"OK\"\n _ =>\n  \"?\"";
    let ir = render_template_to_ir(tpl, &ctx).unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert!(v.to_string().contains("OK"), "expected OK in {}", v);
    round_trip(&ir);
}

#[test]
fn button_and_dynamic_color() {
    let mut ctx = TemplateContext::new();
    ctx.set("surface", "18181b");
    let tpl = "button @click=\"go\" bg-{surface}\n  \"Tap\"";
    let ir = render_template_to_ir(tpl, &ctx).unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["kind"], "button");
    assert_eq!(v["root"][0]["label"], "Tap");
    assert_eq!(v["root"][0]["onClick"], "go");
    round_trip(&ir);
}

#[test]
fn dropzone_ir() {
    let tpl = r#"
dropzone @drop="files" accept="image/*,application/pdf" border rounded p-4
  span text-sm
    "Drop files"
"#;
    let ir = render_template_to_ir(tpl, &TemplateContext::new()).unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["kind"], "dropzone");
    assert_eq!(v["root"][0]["label"], "Drop files");
    assert_eq!(v["root"][0]["accept"], "image/*,application/pdf");
    assert_eq!(v["root"][0]["onDrop"], "files");
    round_trip(&ir);
}

#[test]
fn standard_controls_ir() {
    let tpl = r#"
div flex flex-col
  toggle bind=photos checked=true @change="sync.photos"
    "Photos"
  checkbox bind=clipboard checked=false
    "Clipboard"
  slider bind=quota value=25 min=0 max=100 step=5 label="Quota"
  progress value=62 max=100 label="Transfer"
  meter value=512 min=0 max=1024 label="Storage"
  badge tone="success"
    "Online"
  divider
  spacer size=16
"#;
    let ir = render_template_to_ir(tpl, &TemplateContext::new()).unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    let children = &v["root"][0]["children"];
    assert_eq!(children[0]["kind"], "toggle");
    assert_eq!(children[0]["checked"], true);
    assert_eq!(children[0]["onChange"], "sync.photos");
    assert_eq!(children[1]["kind"], "checkbox");
    assert_eq!(children[2]["kind"], "slider");
    assert_eq!(children[2]["step"], 5.0);
    assert_eq!(children[3]["kind"], "progress");
    assert_eq!(children[4]["kind"], "meter");
    assert_eq!(children[5]["kind"], "badge");
    assert_eq!(children[6]["kind"], "divider");
    assert_eq!(children[7]["kind"], "spacer");
    round_trip(&ir);
}

#[test]
fn web_and_react_native_style_tags_lower_to_native_ir() {
    let tpl = r#"
<View class="flex flex-col gap-2">
  <Text class="text-lg font-semibold">Acme</Text>
  <p>Paragraph copy</p>
  <h1>Heading</h1>
  <ul>
    <li><span>One</span></li>
    <li><Text>Two</Text></li>
  </ul>
  <Switch checked={true} label="LAN sync" />
  <Progress value={80} max={100} label="Backup" />
</View>
"#;
    let ir = render_template_to_ir(tpl, &TemplateContext::new()).unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    let root = &v["root"][0];
    assert_eq!(root["kind"], "stack");
    assert_eq!(root["children"][0]["kind"], "text");
    assert_eq!(root["children"][3]["kind"], "list");
    assert_eq!(root["children"][3]["children"][0]["kind"], "listItem");
    assert_eq!(root["children"][4]["kind"], "toggle");
    assert_eq!(root["children"][5]["kind"], "progress");
    round_trip(&ir);
}

#[test]
fn render_from_files_entry() {
    let mut files = HashMap::new();
    files.insert("main.crepus".into(), "div\n  \"ok\"".into());
    let ir = render_from_files(&files, "main.crepus", &TemplateContext::new()).unwrap();
    assert_eq!(ir.root.len(), 1);
}

#[test]
fn render_from_files_component_entry() {
    let mut files = HashMap::new();
    files.insert(
        "components.crepus".into(),
        "--- Card\ndiv\n  \"card\"".into(),
    );
    let ir = render_from_files(&files, "components.crepus#Card", &TemplateContext::new()).unwrap();
    assert_eq!(ir.root.len(), 1);
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["children"][0]["content"], "card");
}

#[test]
fn render_from_files_missing_file() {
    let files = HashMap::new();
    let err = render_from_files(&files, "missing.crepus", &TemplateContext::new())
        .unwrap_err()
        .to_string();
    assert!(err.contains("file not found in virtual fs: missing.crepus"));
}

#[test]
fn render_from_files_missing_component_file() {
    let files = HashMap::new();
    let err = render_from_files(&files, "missing.crepus#Component", &TemplateContext::new())
        .unwrap_err()
        .to_string();
    assert!(err.contains("file not found in virtual fs: missing.crepus"));
}

#[test]
fn ir_patch_round_trip_text_update() {
    let old = render_template_to_ir("div\n  \"Hello\"", &TemplateContext::new()).unwrap();
    let new = render_template_to_ir("div\n  \"Hello world\"", &TemplateContext::new()).unwrap();

    let patch = diff_ir(&old, &new);
    assert!(!patch.is_empty());

    let mut applied = old.clone();
    apply_mutations(&mut applied, &patch).unwrap();
    assert_eq!(applied, new);
}

#[test]
fn ast_gate_rejects_control_flow_condition_changes() {
    let old = crepuscularity_core::parse_template("if {a}\n  \"x\"\nelse\n  \"y\"").unwrap();
    let new = crepuscularity_core::parse_template("if {b}\n  \"x\"\nelse\n  \"y\"").unwrap();
    assert!(!ast_shape_compatible(&old, &new));
}

#[test]
fn plan_hot_reload_returns_patch_for_literal_changes() {
    let msg = plan_hot_reload(
        "div\n  \"Hello\"",
        "div\n  \"Hello world\"",
        &TemplateContext::new(),
    );
    match msg {
        HotReloadMessage::Patch { mutations } => assert!(!mutations.is_empty()),
        other => panic!("expected Patch, got {other:?}"),
    }
}

#[test]
fn plan_hot_reload_falls_back_to_full_reload_for_semantic_changes() {
    let msg = plan_hot_reload(
        "if {a}\n  div\n    \"x\"",
        "if {b}\n  div\n    \"x\"",
        &TemplateContext::new(),
    );
    match msg {
        HotReloadMessage::FullReload { reason, .. } => {
            assert!(reason.contains("semantics"));
        }
        other => panic!("expected FullReload, got {other:?}"),
    }
}

#[test]
fn input_and_picker_ir() {
    let tpl = r#"
div flex flex-col
  input bind=note placeholder="Hi"
  picker bind=mode
    span value="matrix" "Matrix"
    span "Stalwart"
"#;
    let ir = render_template_to_ir(tpl, &TemplateContext::new()).unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["children"][0]["kind"], "input");
    assert_eq!(v["root"][0]["children"][1]["kind"], "picker");
}

#[test]
fn tabs_ir_and_native_codegen() {
    let tpl = r#"
tabs bind=page
  tab value="photos" label="Photos" icon="photo.on.rectangle"
    div "Photos screen"
  tab value="files" label="Files"
    div "Files screen"
"#;
    let ir = render_template_to_ir(tpl, &TemplateContext::new()).unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["kind"], "tabs");
    assert_eq!(v["root"][0]["tabs"][0]["value"], "photos");
    assert_eq!(v["root"][0]["tabs"][0]["icon"], "photo.on.rectangle");

    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "TabsView");
    assert!(swift.contains("TabView(selection: Binding"));
    assert!(swift.contains(".tabItem { Label(\"Photos\", systemImage: \"photo.on.rectangle\") }"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "TabsView");
    assert!(compose.contains("Scaffold(bottomBar = {"));
    assert!(compose
        .contains("NavigationBarItem(selected = CrepusStateStore.text(\"page\") == \"photos\""));
}

#[test]
fn native_codegen_keeps_conditional_sections() {
    let tpl = r#"
div flex flex-col
  if {signed_in}
    div
      "Home"
  if {!signed_in}
    div
      "Setup"
"#;
    let ir = render_template_to_ir(tpl, &TemplateContext::new()).unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "ConditionalView");
    assert!(swift.contains("if CrepusActions.model.bool(\"signed_in\")"));
    assert!(swift.contains("Text(\"Home\")"));
    assert!(swift.contains("if CrepusActions.model.bool(\"!signed_in\")"));
    assert!(swift.contains("Text(\"Setup\")"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "ConditionalView");
    assert!(compose.contains("if (CrepusStateStore.bool(\"signed_in\"))"));
    assert!(compose.contains("Text(\"Home\")"));
    assert!(compose.contains("if (CrepusStateStore.bool(\"!signed_in\"))"));
    assert!(compose.contains("Text(\"Setup\")"));
}

#[test]
fn file_picker_ir_and_codegen_actions() {
    let tpl = r#"
file-picker accept="image/*,video/*" multiple @pick="pickMedia" "Choose media"
"#;
    let ir = render_template_to_ir(tpl, &TemplateContext::new()).unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["kind"], "filePicker");
    assert_eq!(v["root"][0]["accept"][0], "image/*");
    assert_eq!(v["root"][0]["multiple"], true);
    assert_eq!(v["root"][0]["onPick"], "pickMedia");

    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "PickerView");
    assert!(swift.contains("\"pickMedia\""));
    assert!(swift.contains("CrepusActions.perform(\"pickMedia\")"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "PickerView");
    assert!(compose.contains("\"pickMedia\""));
    assert!(compose.contains("CrepusActions.perform(\"pickMedia\")"));
}

#[test]
fn image_object_fit_and_position_classes() {
    let tpl = r#"
img object-cover object-right-top src="https://example.com/cat.png" placeholder="Loading image…"
"#;
    let ir = render_template_to_ir(tpl, &TemplateContext::new()).unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["kind"], "image");
    assert_eq!(v["root"][0]["style"]["objectFit"], "cover");
    assert_eq!(v["root"][0]["style"]["objectPosition"], "right-top");
    assert_eq!(v["root"][0]["placeholder"], "Loading image…");
}

#[test]
fn gradient_background_classes() {
    let tpl = r#"
div bg-gradient-to-r from-blue-500 to-red-500
  "Gradient"
"#;
    let ir = render_template_to_ir(tpl, &TemplateContext::new()).unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["kind"], "stack");
    assert_eq!(v["root"][0]["style"]["backgroundGradientDirection"], "to-r");
    assert_eq!(
        v["root"][0]["style"]["backgroundGradientFrom"],
        crate::colors::lookup_named_color("blue-500").unwrap()
    );
    assert_eq!(
        v["root"][0]["style"]["backgroundGradientTo"],
        crate::colors::lookup_named_color("red-500").unwrap()
    );
}

// ── Svelte frontend → View IR ────────────────────────────────────────────────

fn render_svelte(src: &str, ctx: &TemplateContext) -> ViewIr {
    let nodes = crepuscularity_core::parser::parse_template_with_path(
        src,
        Some(std::path::Path::new("Card.svelte")),
    )
    .expect("svelte template should parse");
    crate::render_nodes_to_ir(&nodes, ctx).expect("svelte AST should lower")
}

#[test]
fn svelte_text_interpolation_lowers_to_text() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "Ada");
    let ir = render_svelte(
        "<div class=\"flex flex-col gap-4\"><span>Hello {name}</span></div>",
        &ctx,
    );
    let expected = json!({
        "version": IR_VERSION,
        "root": [{
            "kind": "stack",
            "axis": "column",
            "spacing": 16.0,
            "style": {
                "classes": ["flex", "flex-col", "gap-4"],
                "flexDirection": "column"
            },
            "children": [{
                "kind": "text",
                "content": "Hello Ada"
            }]
        }]
    });
    assert_eq!(serde_json::to_value(&ir).unwrap(), expected);
}

#[test]
fn svelte_if_block_lowers_to_if_node() {
    let ir = render_svelte(
        "<div>{#if ready}<span>go</span>{:else}<span>wait</span>{/if}</div>",
        &TemplateContext::new(),
    );
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["children"][0]["kind"], "if");
    assert_eq!(v["root"][0]["children"][0]["condition"], "ready");
    assert_eq!(
        v["root"][0]["children"][0]["elseChildren"][0]["content"],
        "wait"
    );
    round_trip(&ir);
}

#[test]
fn svelte_each_block_lowers_to_for_each() {
    let ir = render_svelte(
        "<div>{#each items as item}<span>{item}</span>{/each}</div>",
        &TemplateContext::new(),
    );
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["children"][0]["kind"], "forEach");
    assert_eq!(v["root"][0]["children"][0]["bind"], "items");
    assert_eq!(v["root"][0]["children"][0]["itemName"], "item");
    assert_eq!(v["root"][0]["children"][0]["itemBody"][0]["bind"], "item");
    round_trip(&ir);
}

#[test]
fn svelte_events_and_bindings_reach_the_ir() {
    let ir = render_svelte(
        "<div><button on:click={increment}>+</button><input bind:value={draft} /></div>",
        &TemplateContext::new(),
    );
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["children"][0]["kind"], "button");
    assert_eq!(v["root"][0]["children"][0]["onClick"], "increment");
    assert_eq!(v["root"][0]["children"][1]["kind"], "input");
    assert_eq!(v["root"][0]["children"][1]["bind"], "draft");
    round_trip(&ir);
}

#[test]
fn svelte_anchor_lowers_to_link_with_class_tokens() {
    let ir = render_svelte(
        r#"<a class="text-blue-400 underline" href="https://example.com" target="_blank">Docs</a>"#,
        &TemplateContext::new(),
    );
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["kind"], "link");
    assert_eq!(v["root"][0]["href"], "https://example.com");
    assert_eq!(v["root"][0]["target"], "_blank");
    assert_eq!(
        v["root"][0]["style"]["classes"],
        json!(["text-blue-400", "underline"])
    );
    assert_eq!(v["root"][0]["children"][0]["content"], "Docs");
    round_trip(&ir);
}

#[test]
fn svelte_component_golden_lowers_to_expected_ir() {
    let src = r#"<script>
  let count = $state(0);
  const increment = () => count++;
</script>

<style>
  .card { border-radius: 8px; }
</style>

<!-- a representative card -->
<div class="flex flex-col gap-4 p-4">
  <h1 class="text-2xl font-bold">{title}</h1>
  {#if count}
    <span class="text-sm">Clicked {count} times</span>
  {:else}
    <span class="text-sm">Never clicked</span>
  {/if}
  <button class="px-4 py-2" on:click={increment}>Add</button>
  <ul>
    {#each items as item}
      <li>{item}</li>
    {/each}
  </ul>
  <a class="underline" href="https://example.com">Docs</a>
</div>
"#;
    let mut ctx = TemplateContext::new();
    ctx.set("title", "Counter");
    ctx.set("count", "0");
    let ir = render_svelte(src, &ctx);
    let v = serde_json::to_value(&ir).unwrap();

    assert_eq!(v["version"], IR_VERSION);
    let root = &v["root"][0];
    assert_eq!(root["kind"], "stack");
    assert_eq!(root["axis"], "column");
    assert_eq!(root["spacing"], 16.0);
    assert_eq!(
        root["style"]["classes"],
        json!(["flex", "flex-col", "gap-4", "p-4"])
    );

    let kids = root["children"].as_array().unwrap();
    assert_eq!(kids.len(), 5, "script/style/comment must not become nodes");

    assert_eq!(kids[0]["kind"], "text");
    assert_eq!(kids[0]["content"], "Counter");
    assert_eq!(
        kids[0]["style"]["classes"],
        json!(["text-2xl", "font-bold"])
    );

    assert_eq!(kids[1]["kind"], "if");
    assert_eq!(kids[1]["condition"], "count");
    assert_eq!(kids[1]["thenChildren"][0]["content"], "Clicked 0 times");
    assert_eq!(kids[1]["elseChildren"][0]["content"], "Never clicked");

    assert_eq!(kids[2]["kind"], "button");
    assert_eq!(kids[2]["onClick"], "increment");
    assert_eq!(kids[2]["style"]["classes"], json!(["px-4", "py-2"]));

    assert_eq!(kids[3]["kind"], "list");
    assert_eq!(kids[3]["children"][0]["kind"], "forEach");
    assert_eq!(kids[3]["children"][0]["bind"], "items");
    assert_eq!(kids[3]["children"][0]["itemName"], "item");

    assert_eq!(kids[4]["kind"], "link");
    assert_eq!(kids[4]["href"], "https://example.com");
    assert_eq!(kids[4]["style"]["classes"], json!(["underline"]));

    round_trip(&ir);
}

fn vue_ir(source: &str, ctx: &TemplateContext) -> ViewIr {
    let nodes = crepuscularity_core::parser::parse_template_with_path(
        source,
        Some(std::path::Path::new("Component.vue")),
    )
    .unwrap();
    crate::render_nodes_to_ir(&nodes, ctx).unwrap()
}

#[test]
fn vue_sfc_lowers_to_view_ir_with_class_tokens() {
    let mut ctx = TemplateContext::new();
    ctx.set("title", "Vue Panel");
    ctx.set("count", "2");
    ctx.set("query", "");
    ctx.set("loggedIn", "yes");
    let mut item_a = TemplateContext::new();
    item_a.set("value", "alpha");
    let mut item_b = TemplateContext::new();
    item_b.set("value", "beta");
    ctx.vars.insert(
        "items".into(),
        crepuscularity_core::context::TemplateValue::List(vec![item_a, item_b]),
    );

    let source = r#"<template>
  <div class="flex flex-col gap-4 p-4">
    <h1 class="text-lg font-bold">{{ title }}</h1>
    <p v-if="loggedIn">Signed in</p>
    <p v-else>Signed out</p>
    <button class="px-4 py-2" @click.prevent="increment">Add</button>
    <input type="text" v-model="query" />
    <ul class="list">
      <li v-for="(item, i) in items" class="row">{{ item }}</li>
    </ul>
  </div>
</template>

<script setup>
const increment = () => {}
</script>

<style scoped>
.row { color: red; }
</style>
"#;

    let ir = vue_ir(source, &ctx);
    assert_eq!(ir.version, IR_VERSION);

    let v = serde_json::to_value(&ir).unwrap();
    let root = &v["root"][0];
    assert_eq!(root["kind"], "stack");
    assert_eq!(root["axis"], "column");
    assert_eq!(
        root["style"]["classes"],
        json!(["flex", "flex-col", "gap-4", "p-4"])
    );

    let kids = &root["children"];
    assert_eq!(kids[0]["content"], "Vue Panel");
    assert_eq!(kids[0]["style"]["classes"], json!(["text-lg", "font-bold"]));

    assert_eq!(kids[1]["kind"], "if");
    assert_eq!(kids[1]["condition"], "loggedIn");
    assert_eq!(kids[1]["thenChildren"][0]["content"], "Signed in");
    assert_eq!(kids[1]["elseChildren"][0]["content"], "Signed out");

    assert_eq!(kids[2]["kind"], "button");
    assert_eq!(kids[2]["label"], "Add");
    assert_eq!(kids[2]["onClick"], "increment");
    assert_eq!(kids[2]["style"]["classes"], json!(["px-4", "py-2"]));

    assert_eq!(kids[3]["kind"], "input");
    assert_eq!(kids[3]["bind"], "query");

    assert_eq!(kids[4]["children"][0]["kind"], "forEach");
    assert_eq!(kids[4]["children"][0]["bind"], "items");
    assert_eq!(kids[4]["children"][0]["itemName"], "item");
    assert_eq!(kids[4]["kind"], "list");
    assert_eq!(kids[4]["style"]["classes"], json!(["list"]));
    assert_eq!(
        kids[4]["children"][0]["itemBody"][0]["style"]["classes"],
        json!(["row"])
    );
    assert_eq!(
        kids[4]["children"][0]["itemBody"][0]["children"][0]["bind"],
        "item"
    );

    round_trip(&ir);
}

#[test]
fn vue_conditional_classes_lower_to_active_classes() {
    let mut ctx = TemplateContext::new();
    ctx.set("isOn", "1");
    ctx.set("visible", "");
    let ir = vue_ir(
        r#"<template>
  <div class="base" :class="{ 'font-bold': isOn, dimmed: visible }" v-show="visible"></div>
</template>"#,
        &ctx,
    );
    let v = serde_json::to_value(&ir).unwrap();
    let classes = &v["root"][0]["style"]["classes"];
    assert_eq!(classes, &json!(["base", "font-bold", "hidden"]));
    round_trip(&ir);
}

#[test]
fn element_id_lowers_into_view_style() {
    let ir = render_template_to_ir(
        "div #page flex\n h1 #tsc-heading\n  \"hello\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert_eq!(v["root"][0]["style"]["id"], "page");
    assert_eq!(v["root"][0]["children"][0]["style"]["id"], "tsc-heading");
    round_trip(&ir);
}

#[test]
fn element_without_id_omits_the_field() {
    let ir = render_template_to_ir("div flex", &TemplateContext::new()).unwrap();
    let v = serde_json::to_value(&ir).unwrap();
    assert!(v["root"][0]["style"].get("id").is_none());
    round_trip(&ir);
}
