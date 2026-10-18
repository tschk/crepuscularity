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
fn native_text_truncation_applies_a_single_line_with_ellipsis() {
    for (classes, expected_lines) in [
        ("truncate", 1),
        ("truncate line-clamp-3", 1),
        ("line-clamp-2", 2),
    ] {
        let ir = render_template_to_ir(
            &format!("text {classes} \"Long text for truncation\""),
            &TemplateContext::new(),
        )
        .unwrap();
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "TruncatedText");
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "TruncatedText");
        assert!(
            swift.contains(&format!(".lineLimit({expected_lines})")),
            "{swift}"
        );
        assert!(
            compose.contains(&format!("maxLines = {expected_lines}")),
            "{compose}"
        );
        assert!(
            compose.contains("overflow = TextOverflow.Ellipsis"),
            "{compose}"
        );
    }
    let ir = render_template_to_ir(
        "text line-clamp-2 text-clip \"Clipped\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "ClippedText");
    assert!(
        compose.contains("overflow = TextOverflow.Clip"),
        "{compose}"
    );
}

#[test]
fn native_text_codegen_omits_nonpositive_line_limits() {
    for limit in [i32::MIN, -1, 0, 1, 3, i32::MAX] {
        let mut ir = render_template_to_ir("text \"Limited\"", &TemplateContext::new()).unwrap();
        *ir.root[0].style_mut() = Some(crate::ViewStyle {
            line_clamp: Some(limit),
            ..Default::default()
        });
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "LineLimitScreen");
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "LineLimitScreen");
        if limit > 0 {
            assert!(swift.contains(&format!(".lineLimit({limit})")), "{swift}");
            assert!(
                compose.contains(&format!("maxLines = {limit}")),
                "{compose}"
            );
        } else {
            assert!(!swift.contains(".lineLimit("), "{swift}");
            assert!(!compose.contains("maxLines ="), "{compose}");
        }
    }
}

#[test]
fn native_text_codegen_preserves_canonical_font_families() {
    for (family, design, compose_family) in [
        ("sans", ".default", "SansSerif"),
        ("serif", ".serif", "Serif"),
        ("mono", ".monospaced", "Monospace"),
    ] {
        let ir = render_template_to_ir(
            &format!("text font-{family} text-[24px] font-bold \"Family\""),
            &TemplateContext::new(),
        )
        .unwrap();
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "FontFamilyScreen");
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "FontFamilyScreen");
        assert!(swift.contains(&format!(".fontDesign({design})")), "{swift}");
        assert!(swift.contains(".font(.system(size: 24.0))"), "{swift}");
        assert!(swift.contains(".fontWeight(.bold)"), "{swift}");
        assert!(
            compose.contains(&format!("fontFamily = FontFamily.{compose_family}")),
            "{compose}"
        );
        assert!(compose.contains("fontSize = 24.0.sp"), "{compose}");
        assert!(
            compose.contains("fontWeight = FontWeight.Bold"),
            "{compose}"
        );
    }
    let mut ir = render_template_to_ir("text \"Custom\"", &TemplateContext::new()).unwrap();
    *ir.root[0].style_mut() = Some(crate::ViewStyle {
        font_family: Some("unsupported-family".into()),
        ..Default::default()
    });
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "CustomFamily");
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "CustomFamily");
    assert!(!swift.contains(".fontDesign("), "{swift}");
    assert!(!compose.contains("fontFamily ="), "{compose}");
}

#[test]
fn native_text_codegen_preserves_letter_spacing() {
    for (element, content) in [("text", "\"Tracking\""), ("badge", "label=Tracking")] {
        for (class, value) in [
            ("tracking-tight", "-0.4"),
            ("tracking-normal", "0.0"),
            ("tracking-widest", "1.6"),
            ("tracking-[0.125px]", "0.125"),
            ("tracking-[0.00001px]", "1e-5"),
        ] {
            let ir = render_template_to_ir(
                &format!("{element} {class} {content}"),
                &TemplateContext::new(),
            )
            .unwrap();
            let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "TrackingScreen");
            let compose =
                generate_native_source(&ir, NativeCodegenTarget::Compose, "TrackingScreen");
            assert!(swift.contains(&format!(".tracking({value})")), "{swift}");
            assert!(
                compose.contains(&format!("letterSpacing = {value}.sp")),
                "{compose}"
            );
        }
    }
}

#[test]
fn native_codegen_preserves_fractional_font_sizes() {
    for (size, emitted) in [("12.25", "12.25"), ("0.00001", "1e-5")] {
        let ir = render_template_to_ir(
            &format!("text text-[{size}px] \"Fine text\""),
            &TemplateContext::new(),
        )
        .unwrap();
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "FineText");
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "FineText");
        assert!(
            swift.contains(&format!(".font(.system(size: {emitted}))")),
            "{swift}"
        );
        assert!(
            compose.contains(&format!("fontSize = {emitted}.sp")),
            "{compose}"
        );
    }
}

#[test]
fn compose_line_height_uses_the_explicit_font_size() {
    for (classes, expected) in [
        ("leading-normal", "24.0"),
        ("text-[24px] leading-normal", "36.0"),
        ("text-[12.25px] leading-tight", "15.3125"),
    ] {
        let ir = render_template_to_ir(
            &format!("text {classes} \"Line height\""),
            &TemplateContext::new(),
        )
        .unwrap();
        let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "LineHeight");
        assert!(
            source.contains(&format!("lineHeight = {expected}.sp")),
            "{source}"
        );
    }
}

#[test]
fn native_codegen_preserves_all_nine_font_weights() {
    for (class, swift_weight, compose_weight) in [
        ("thin", ".ultraLight", "Thin"),
        ("extralight", ".thin", "ExtraLight"),
        ("light", ".light", "Light"),
        ("normal", ".regular", "Normal"),
        ("medium", ".medium", "Medium"),
        ("semibold", ".semibold", "SemiBold"),
        ("bold", ".bold", "Bold"),
        ("extrabold", ".heavy", "ExtraBold"),
        ("black", ".black", "Black"),
    ] {
        let ir = render_template_to_ir(
            &format!("text font-{class} \"Weight\""),
            &TemplateContext::new(),
        )
        .unwrap();
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "WeightScreen");
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "WeightScreen");
        assert!(
            swift.contains(&format!(".fontWeight({swift_weight})")),
            "{class}: {swift}"
        );
        assert!(
            compose.contains(&format!("fontWeight = FontWeight.{compose_weight}")),
            "{class}: {compose}"
        );
    }
}

#[test]
fn short_hex_template_colors_reach_codegen() {
    let ir = render_template_to_ir("div bg-[#f008]", &TemplateContext::new()).unwrap();
    assert!(to_json(&ir).unwrap().contains("#ff000088"));
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "AlphaScreen");
    assert!(swift.contains("Color(red: 1.000, green: 0.000, blue: 0.000, opacity: 0.533)"));
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "AlphaScreen");
    assert!(compose.contains("Color(0x88FF0000)"));
}

#[test]
fn palette_opacity_reaches_native_codegen() {
    let ir = render_template_to_ir("div bg-red-500/50", &TemplateContext::new()).unwrap();
    let json = to_json(&ir).unwrap();
    assert!(json.contains("#fb2c3680"));
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "AlphaScreen");
    assert!(swift.contains("opacity: 0.502)"));
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "AlphaScreen");
    assert!(compose.contains("Color(0x80FB2C36)"));
}

#[test]
fn codegen_preserves_css_hex_alpha_channel_order() {
    let ir = render_template_to_ir("div bg-[#ff000080]", &TemplateContext::new()).unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "AlphaScreen");
    assert!(swift.contains("Color(red: 1.000, green: 0.000, blue: 0.000, opacity: 0.502)"));
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "AlphaScreen");
    assert!(compose.contains("Color(0x80FF0000)"));
}

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
                "content": "Hello Ada",
                "bind": "\"\" + \"Hello \" + (name)"
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
    assert!(source.contains("DispatchQueue(label: \"dev.crepuscularity.native.actions\""));
    assert!(source.contains("actionQueue.async"));
    assert!(source.contains("DispatchQueue.main.sync"));
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
    assert!(source.contains("Executors.newSingleThreadExecutor"));
    assert!(source.contains("executor.execute"));
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
fn codegen_compose_keeps_context_strings_literal() {
    let mut ctx = TemplateContext::new();
    ctx.set("message", "Price $amount ${run { 7 }}");
    ctx.set("url", "https://example.invalid/${run { 7 }}");
    let ir = render_template_to_ir(
        "button \"{message}\"\niframe src={url}\nbutton label={message}",
        &ctx,
    )
    .unwrap();
    assert!(matches!(
        &ir.root[0],
        crate::ViewNode::Button { label, label_parts, .. }
            if label == "Price $amount ${run { 7 }}"
                && matches!(label_parts.as_slice(), [crate::ir::LabelPart::Expr(value)] if value == "message")
    ));
    assert!(matches!(
        &ir.root[1],
        crate::ViewNode::WebView { src, .. } if src == "https://example.invalid/${run { 7 }}"
    ));
    assert!(matches!(
        &ir.root[2],
        crate::ViewNode::Button { label, label_parts, .. }
            if label == "Price $amount ${run { 7 }}" && label_parts.is_empty()
    ));
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "LiteralView");
    assert!(compose.contains(r#"Text(CrepusStateStore.text("message"))"#));
    assert!(compose.contains(r#"Text("Price \$amount \${run { 7 }}")"#));
    let url = r#""https://example.invalid/\${run { 7 }}""#;
    assert_eq!(compose.matches(url).count(), 3);

    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "LiteralView");
    assert!(swift.contains(r#"Text(CrepusActions.model.text("message"))"#));
    assert!(swift.contains(r#"Text("Price $amount ${run { 7 }}")"#));
}

#[test]
fn codegen_compose_keeps_decoded_ir_strings_literal() {
    let ir: ViewIr = serde_json::from_value(json!({
        "version": IR_VERSION,
        "root": [{
            "kind": "tabs",
            "bind": "$selected",
            "onChange": "change:$value",
            "tabs": [{
                "value": "${run { 7 }}",
                "label": "$label",
                "children": []
            }]
        }, {
            "kind": "picker",
            "bind": "$selected",
            "onChange": "change:$value",
            "options": [{"value": "$option", "label": "$label"}]
        }]
    }))
    .unwrap();
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "LiteralView");
    assert_eq!(compose.matches(r#""\${run { 7 }}""#).count(), 3);
    assert!(compose.contains(r#"CrepusStateStore.text("\$selected")"#));
    assert!(compose.contains(
        r#"CrepusActions.performChange("change:\$value", "\$selected", JsonPrimitive("\$option"))"#
    ));
    assert_eq!(compose.matches(r#"Text("\$label")"#).count(), 2);
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
    assert!(swift.contains(
        "CrepusActions.model.text(\"hub.name\", scopeName: \"hub\", scope: `__crepusItem_687562`)"
    ));
    assert!(swift.contains("CrepusActions.model.text(\"pairing_code\")"));

    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "DynamicView");
    assert!(compose.contains("CrepusStateStore.applyResult(raw)"));
    assert!(compose.contains("if (CrepusStateStore.bool(\"count > 1\"))"));
    assert!(compose.contains("CrepusStateStore.items(\"hubs\").forEachIndexed"));
    assert!(compose.contains(
        "CrepusStateStore.text(\"hub.name\", scopeName = \"hub\", scope = `__crepusItem_687562`)"
    ));
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
fn compose_codegen_does_not_emit_malformed_hex_as_source() {
    let mut ir = render_template_to_ir("div bg-red-500", &TemplateContext::new()).unwrap();
    if let crate::ViewNode::Stack {
        style: Some(style), ..
    } = &mut ir.root[0]
    {
        style.background_color = Some("#zzzzzz".into());
    } else {
        panic!("expected styled stack");
    }
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "InvalidColor");
    assert!(!compose.contains("0xFFZZZZZZ"));
    assert!(compose.contains("Color(0xFF888888)"));
    let ir = render_template_to_ir("div bg-[orange]", &TemplateContext::new()).unwrap();
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "NamedColor");
    assert!(compose.contains("Color(0xFFFFA500)"));
}

#[test]
fn native_codegen_resolves_named_css_colors() {
    let ir = render_template_to_ir("div bg-[red]", &TemplateContext::new()).unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "NamedColor");
    assert!(swift.contains("Color(red: 1.000, green: 0.000, blue: 0.000, opacity: 1.000)"));
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "NamedColor");
    assert!(compose.contains("Color(0xFFFF0000)"));
}

#[test]
fn progress_rejects_nonpositive_maximum() {
    for template in ["progress max=0", "progress max=-1"] {
        assert!(
            render_template_to_ir(template, &TemplateContext::new()).is_err(),
            "{template}"
        );
    }
    assert!(render_template_to_ir("progress max=0.0001", &TemplateContext::new()).is_ok());
}

#[test]
fn progress_codegen_preserves_small_values_and_maximum() {
    let ir =
        render_template_to_ir("progress value=0.0001 max=0.0002", &TemplateContext::new()).unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "FineProgress");
    assert!(swift.contains("value: 0.0001, total: 0.0002"));
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "FineProgress");
    assert!(compose.contains("progress = 0.0001f / 0.0002f"));
}

#[test]
fn codegen_preserves_small_slider_values_and_steps() {
    let ir = render_template_to_ir(
        "slider value=0.0001 min=0 max=0.0002 step=0.00001",
        &TemplateContext::new(),
    )
    .unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "FineSlider");
    assert!(swift.contains(".constant(0.0001)"));
    assert!(swift.contains("in: 0.0...0.0002, step: 1e-5"));
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "FineSlider");
    assert!(compose.contains("value = 0.0001f"));
    assert!(compose.contains("valueRange = 0.0f..0.0002f"));
    assert!(compose.contains("steps = 19"));
}

#[test]
fn slider_rejects_reversed_ranges_and_nonpositive_steps() {
    for template in ["slider min=10 max=0", "slider step=0", "slider step=-1"] {
        assert!(
            render_template_to_ir(template, &TemplateContext::new()).is_err(),
            "{template}"
        );
    }
    assert!(
        render_template_to_ir("slider min=-10 max=10 step=0.5", &TemplateContext::new()).is_ok()
    );
}

#[test]
fn numeric_controls_preserve_documented_invalid_value_fallbacks() {
    for (tag, props) in [
        ("slider", &["value", "min", "max", "step"][..]),
        ("progress", &["value", "max"][..]),
        ("meter", &["value", "min", "max"][..]),
    ] {
        let baseline = render_template_to_ir(tag, &TemplateContext::new()).unwrap();
        let baseline = serde_json::to_value(baseline).unwrap();
        for invalid in ["oops", "NaN", "-inf", "inf", "1e100", "", "   "] {
            for prop in props {
                let mut ctx = TemplateContext::new();
                ctx.set("invalid", invalid);
                for attribute in [
                    format!(r#"{prop}="{invalid}""#),
                    format!("{prop}={{invalid}}"),
                ] {
                    let template = format!("{tag} {attribute}");
                    let actual = render_template_to_ir(&template, &ctx).unwrap();
                    assert_eq!(
                        serde_json::to_value(actual).unwrap()["root"][0],
                        baseline["root"][0],
                        "{template} with invalid={invalid:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn picker_values_evaluate_context_and_preserve_explicit_empty_value() {
    let mut ctx = TemplateContext::new();
    ctx.set("mode_value", "dynamic-mode");
    let ir = render_template_to_ir(
        "picker bind=mode\n  option value={mode_value}\n    \"Dynamic\"\n  option value=\"\"\n    \"None\"\n  option\n    \"Auto Mode\"", &ctx).unwrap();
    let value = serde_json::to_value(&ir).unwrap();
    let options = &value["root"][0]["options"];
    assert_eq!(options[0]["value"], "dynamic-mode");
    assert_eq!(options[1]["value"], "");
    assert_eq!(options[2]["value"], "auto_mode");
}

#[test]
fn input_placeholder_evaluates_context_and_preserves_literal_text() {
    let mut ctx = TemplateContext::new();
    ctx.set("hint", "Enter your name");
    let ir = render_template_to_ir("input bind=name placeholder={hint}", &ctx).unwrap();
    assert!(to_json(&ir).unwrap().contains("Enter your name"));
    let ir = render_template_to_ir("input bind=name placeholder=\"  Name  \"", &ctx).unwrap();
    assert!(to_json(&ir).unwrap().contains("  Name  "));
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
        "CrepusRustActions.dispatchQueuedChange(action, bind: bind, valueJson: valueJson)"
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
    assert!(
        compose.contains("CrepusRustActions.dispatchQueuedChange(action ?: \"\", bind, valueJson)")
    );
    assert!(compose
        .contains("CrepusActions.performChange(\"sync_enabled\", \"enabled\", JsonPrimitive(it))"));
    assert!(compose
        .contains("CrepusActions.performChange(\"sync_volume\", \"volume\", JsonPrimitive(it))"));
    assert!(
        compose.contains("CrepusActions.performChange(\"sync_name\", \"name\", JsonPrimitive(it))")
    );
    assert!(compose
        .contains("CrepusActions.performChange(\"sync_mode\", \"mode\", JsonPrimitive(\"auto\"))"));
    assert!(compose.contains("FilterChip(selected = CrepusStateStore.draftText(\"mode\") { CrepusStateStore.text(\"mode\") } == \"auto\""));
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
        .contains("NavigationBarItem(selected = CrepusStateStore.draftText(\"page\") { CrepusStateStore.text(\"page\") } == \"photos\""));
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
                "content": "Hello Ada",
                "bind": "\"\" + \"Hello \" + (name)"
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
    <button class="px-4 py-2" @click="increment">Add</button>
    <input type="text" v-model="query" />
    <ul class="list">
      <li v-for="(item) in items" class="row">{{ item }}</li>
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

#[test]
fn swiftui_codegen_dispatches_long_press_for_all_existing_fields() {
    for tag in ["button", "div", "toggle", "img", "li"] {
        let ir = render_template_to_ir(
            &format!("<{tag} @longpress={{hold}} />"),
            &TemplateContext::new(),
        )
        .unwrap();
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "GestureScreen");
        assert!(swift.contains(".highPriorityGesture(LongPressGesture().onEnded { _ in CrepusActions.perform(\"hold\") })"), "{tag}: {swift}");
    }
    let ir = render_template_to_ir(
        "<button @click={tap} @longpress={hold}>Gesture</button>",
        &TemplateContext::new(),
    )
    .unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "GestureScreen");
    assert!(
        swift.contains("Button(action: { CrepusActions.perform(\"tap\") })"),
        "{swift}"
    );
    assert!(swift.contains("LongPressGesture"), "{swift}");
}

#[test]
fn compose_codegen_preserves_long_press_fields_and_button_click_actions() {
    for tag in ["button", "div", "toggle", "img", "li"] {
        let ir = render_template_to_ir(
            &format!("<{tag} @longpress={{hold}} />"),
            &TemplateContext::new(),
        )
        .unwrap();
        let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "GestureScreen");
        let invocation = if matches!(tag, "button" | "toggle") {
            ".crepusLongPress(\"hold\", true)"
        } else {
            ".crepusLongPress(\"hold\")"
        };
        assert!(source.contains(invocation), "{tag}: {source}");
        assert!(source.contains("allowConsumedDown || !longPress.isConsumed"));
        assert!(source.contains("longPress.consume()"));
        assert!(
            source.contains("onLongClick { CrepusActions.perform(action); true }"),
            "{source}"
        );
        assert!(
            source.contains("awaitFirstDown(requireUnconsumed = !allowConsumedDown)"),
            "{source}"
        );
    }
    let ir = render_template_to_ir(
        "<button @click={tap} @longpress={hold}>Gesture</button>",
        &TemplateContext::new(),
    )
    .unwrap();
    let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "GestureScreen");
    assert!(
        source.contains("Button(onClick = { CrepusActions.perform(\"tap\") }"),
        "{source}"
    );
    assert!(
        source.contains("awaitPointerEvent(PointerEventPass.Initial)"),
        "{source}"
    );
    let image = render_template_to_ir(
        "<img class=\"p-4 text-red-500\" @longpress={hold} />",
        &TemplateContext::new(),
    )
    .unwrap();
    let image_source =
        generate_native_source(&image, NativeCodegenTarget::Compose, "GestureScreen");
    let image_line = image_source
        .lines()
        .find(|line| line.contains("Text(\"\""))
        .unwrap();
    assert_eq!(image_line.matches("modifier =").count(), 1, "{image_line}");
    assert!(
        image_line.contains(".crepusLongPress(\"hold\").padding("),
        "{image_line}"
    );
    assert!(image_line.contains("color = Color("), "{image_line}");

    let plain = render_template_to_ir("<button>Tap</button>", &TemplateContext::new()).unwrap();
    let plain_source = generate_native_source(&plain, NativeCodegenTarget::Compose, "PlainScreen");
    assert!(!plain_source.contains("crepusLongPress"));
}

#[test]
fn codegen_preserves_unbound_control_change_actions() {
    for tag in ["toggle", "checkbox", "slider"] {
        let ir = render_template_to_ir(
            &format!("<{tag} @change={{changed}} />"),
            &TemplateContext::new(),
        )
        .unwrap();
        for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
            let source = generate_native_source(&ir, target, "UnboundControls");
            assert!(
                source.contains("CrepusActions.perform(\"changed\")"),
                "{tag}: {source}"
            );
            assert!(
                !source.contains("CrepusActions.performChange(\"changed\""),
                "{tag}: {source}"
            );
        }
        let plain = render_template_to_ir(&format!("<{tag} />"), &TemplateContext::new()).unwrap();
        let source = generate_native_source(&plain, NativeCodegenTarget::SwiftUi, "PlainControls");
        assert!(source.contains(".constant("), "{tag}: {source}");
    }
}

#[test]
fn codegen_preserves_handler_names_for_custom_dispatch() {
    let ir = render_template_to_ir(
        "<div @longpress={device.info}><button @click={device.info}>Info</button><toggle @change={device.info}/><slider @change={device.info}/><file-picker @pick={device.info}/></div>",
        &TemplateContext::new(),
    ).unwrap();
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        let source = generate_native_source(&ir, target, "PluginControls");
        assert_eq!(source.matches("device.info").count(), 5, "{source}");
        assert!(
            source.contains("CrepusActions.perform(\"device.info\")"),
            "{source}"
        );
    }
}

#[test]
fn sliders_keep_initial_values_inside_their_bounds() {
    for (template, expected) in [
        ("slider min=10 max=20", 10.0),
        ("slider min=10 max=20 value=0", 10.0),
        ("slider min=10 max=20 value=30", 20.0),
        ("slider min=-20 max=-10", -10.0),
        ("slider min=10 max=20 value=15", 15.0),
    ] {
        let ir = render_template_to_ir(template, &TemplateContext::new()).unwrap();
        let value = serde_json::to_value(ir).unwrap();
        assert_eq!(value["root"][0]["value"], expected, "{template}: {value}");
    }
    assert!(render_template_to_ir("slider min=-3e38 max=3e38", &TemplateContext::new()).is_err());
}

#[test]
fn generated_slider_getters_clamp_bound_model_values() {
    let ir = render_template_to_ir(
        "slider bind=volume min=10 max=20 @change=save",
        &TemplateContext::new(),
    )
    .unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "BoundSlider");
    assert!(
        swift.contains("Swift.min(Swift.max(model.draftNumber(path: \"volume\", fallback: CrepusActions.model.number(\"volume\")), 10.0), 20.0)"),
        "{swift}"
    );
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "BoundSlider");
    assert!(
        compose.contains("CrepusStateStore.draftNumber(\"volume\") { CrepusStateStore.number(\"volume\") }.coerceIn(10.0f, 20.0f)"),
        "{compose}"
    );
}

#[test]
fn compose_slider_steps_match_uniform_declared_intervals() {
    for (template, steps) in [
        ("slider min=10 max=20 step=2", 4),
        ("slider min=-1 max=1 step=0.25", 7),
    ] {
        let ir = render_template_to_ir(template, &TemplateContext::new()).unwrap();
        let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "Stepped");
        assert!(source.contains(&format!("steps = {steps}")), "{source}");
    }
    for template in [
        "slider min=0 max=10 step=3",
        "slider min=0 max=10 step=10",
        "slider step=0.00001",
        "slider",
    ] {
        let ir = render_template_to_ir(template, &TemplateContext::new()).unwrap();
        let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "Continuous");
        assert!(!source.contains("steps ="), "{source}");
    }
}

#[test]
fn controlled_drafts_use_resolved_paths_while_labels_read_committed_state() {
    let ir = render_template_to_ir(
        "div\n for row in {rows}\n  input bind=row.title @change=save @input=observe\n  toggle bind=row.enabled\n  slider bind=row.volume\n  span \"{row.title}\"\n  for child in {row.children}\n   checkbox bind=child.selected",
        &TemplateContext::new(),
    ).unwrap();
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        let source = generate_native_source(&ir, target, "DraftControls");
        for (accessor, path) in [
            (
                "draftText",
                "`__crepusItem_726f77__crepusPath` + \".title\"",
            ),
            (
                "draftBool",
                "`__crepusItem_726f77__crepusPath` + \".enabled\"",
            ),
            (
                "draftNumber",
                "`__crepusItem_726f77__crepusPath` + \".volume\"",
            ),
            (
                "draftBool",
                "`__crepusItem_6368696c64__crepusPath` + \".selected\"",
            ),
        ] {
            let label = match target {
                NativeCodegenTarget::SwiftUi => "path: ",
                NativeCodegenTarget::Compose => "",
            };
            assert!(
                source.contains(&format!(".{accessor}({label}{path}")),
                "{source}"
            );
        }
        let committed_label = match target {
            NativeCodegenTarget::SwiftUi => "Text(CrepusActions.model.text(\"row.title\", scopeName: \"row\", scope: `__crepusItem_726f77`))",
            NativeCodegenTarget::Compose => "Text(CrepusStateStore.text(\"row.title\", scopeName = \"row\", scope = `__crepusItem_726f77`))",
        };
        assert!(source.contains(committed_label), "{source}");
        let change = source.find("CrepusActions.performChange(\"save\"").unwrap();
        let input = source.find("CrepusActions.perform(\"observe\")").unwrap();
        assert!(change < input, "{source}");
    }
}

#[test]
fn repeated_control_writes_resolve_indexed_model_paths() {
    let ir = render_template_to_ir(
        "div\n for row in {rows}\n  toggle bind=row.enabled\n  input bind=row.title\n  slider bind=row.volume\n  for child in {row.children}\n   checkbox bind=child.selected\n  toggle bind=rootEnabled",
        &TemplateContext::new(),
    ).unwrap();
    for target in [NativeCodegenTarget::Compose, NativeCodegenTarget::SwiftUi] {
        let source = generate_native_source(&ir, target, "Repeated");
        assert!(
            source.contains("`__crepusItem_726f77__crepusPath` = \"rows\" + \".\""),
            "{source}"
        );
        for suffix in [".enabled", ".title", ".volume", ".children"] {
            assert!(
                source.contains(&format!("`__crepusItem_726f77__crepusPath` + \"{suffix}\"")),
                "{source}"
            );
        }
        assert!(
            source.contains("`__crepusItem_6368696c64__crepusPath` + \".selected\""),
            "{source}"
        );
        assert!(source.contains("\"rootEnabled\""));
        assert!(!source.contains("{{ _,"));
    }
}

#[test]
fn repeated_control_index_names_do_not_collide_with_item_aliases() {
    let ir = render_template_to_ir(
        "div\n for crepusIndex in {rows}\n  toggle bind=crepusIndex.enabled",
        &TemplateContext::new(),
    )
    .unwrap();
    for target in [NativeCodegenTarget::Compose, NativeCodegenTarget::SwiftUi] {
        let source = generate_native_source(&ir, target, "AliasCollision");
        assert!(
            source.contains("`__crepusItem_637265707573496e646578__crepusIndex`, `__crepusItem_637265707573496e646578`"),
            "{source}"
        );
        assert!(source.contains("`__crepusItem_637265707573496e646578__crepusPath` + \".enabled\""));
    }
}

#[test]
fn unbound_inputs_do_not_read_or_write_the_root_model() {
    for template in [
        "input",
        "input @change=sync",
        "input type=password @change=sync",
        "textarea @change=sync",
    ] {
        let ir = render_template_to_ir(template, &TemplateContext::new()).unwrap();
        for target in [NativeCodegenTarget::Compose, NativeCodegenTarget::SwiftUi] {
            let source = generate_native_source(&ir, target, "UnboundInput");
            assert!(!source.contains(".text(\"\""), "{source}");
            assert!(!source.contains("performChange(\"sync\""), "{source}");
            if template.contains("@change") {
                assert!(
                    source.contains("CrepusActions.perform(\"sync\")"),
                    "{source}"
                );
            }
        }
    }
}

#[test]
fn compose_inputs_preserve_single_line_and_multiline_ir_flags() {
    for (template, single_line) in [
        ("input bind=title", true),
        ("input type=password bind=password", true),
        ("textarea bind=notes", false),
        ("input multiline bind=notes", false),
    ] {
        let ir = render_template_to_ir(template, &TemplateContext::new()).unwrap();
        let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "InputLines");
        assert!(
            source.contains(&format!("singleLine = {single_line}")),
            "{source}"
        );
        if template.contains("password") {
            assert!(
                source.contains("PasswordVisualTransformation()"),
                "{source}"
            );
        }
    }
}

#[test]
fn secure_multiline_inputs_are_rejected_instead_of_rendered_unmasked() {
    for template in [
        "textarea secure bind=secret",
        "input multiline type=password bind=secret",
        "input multiline secure bind=secret",
    ] {
        let error = render_template_to_ir(template, &TemplateContext::new()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("secure multiline inputs are not supported"),
            "{error}"
        );
    }
    let mut ir =
        render_template_to_ir("input type=password bind=secret", &TemplateContext::new()).unwrap();
    let crate::ViewNode::Input { multiline, .. } = &mut ir.root[0] else {
        panic!("expected input")
    };
    *multiline = true;
    let source = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "DirectSecureIr");
    assert!(source.contains("SecureField("), "{source}");
    assert!(!source.contains("TextEditor("), "{source}");
}

#[test]
fn native_input_aliases_evaluate_dynamic_password_types() {
    let mut ctx = TemplateContext::new();
    ctx.set("kind", "PASSWORD");
    for tag in ["input", "textfield", "textinput"] {
        let template = format!("{tag} type={{kind}} bind=secret");
        let ir = render_template_to_ir(&template, &ctx).unwrap();
        assert!(
            matches!(ir.root[0], crate::ViewNode::Input { secure: true, .. }),
            "{tag}"
        );
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "SecureAlias");
        assert!(swift.contains("SecureField("), "{swift}");
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "SecureAlias");
        assert!(
            compose.contains("PasswordVisualTransformation()"),
            "{compose}"
        );
    }
    assert!(render_template_to_ir("textarea type={kind} bind=secret", &ctx).is_err());
    ctx.set("kind", "checkbox");
    for tag in ["textfield", "textinput"] {
        let ir = render_template_to_ir(&format!("{tag} type={{kind}} bind=text"), &ctx).unwrap();
        assert!(matches!(
            ir.root[0],
            crate::ViewNode::Input { secure: false, .. }
        ));
    }
}

#[test]
fn repeated_control_aliases_cannot_shadow_bridge_symbols() {
    for name in [
        "CrepusStateStore",
        "CrepusActions",
        "__crepusItem_726f77__crepusPath",
    ] {
        let template = format!("div\n for {name} in {{rows}}\n  toggle bind={name}.enabled");
        let ir = render_template_to_ir(&template, &TemplateContext::new()).unwrap();
        for target in [NativeCodegenTarget::Compose, NativeCodegenTarget::SwiftUi] {
            let source = generate_native_source(&ir, target, "BridgeAlias");
            assert!(!source.contains(&format!(", `{name}` ->")), "{source}");
            assert!(!source.contains(&format!(", `{name}` in")), "{source}");
            assert!(source.contains(&format!("\"{name}.enabled\"")), "{source}");
        }
    }
}

#[test]
fn native_links_preserve_destinations_and_click_behavior() {
    let ir = render_template_to_ir(
        r#"<a href="https://example.com/docs?q=$value" class="p-4 text-blue-400">Docs</a>"#,
        &TemplateContext::new(),
    )
    .unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "NativeLinks");
    assert!(
        swift.contains("SwiftUI.Link(destination: crepusDestination)"),
        "{swift}"
    );
    assert!(swift.contains("Foundation.URL(string: \"https://example.com/docs?q=$value\")"));
    assert!(swift.contains("Text(\"Docs\")"));
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "NativeLinks");
    assert!(compose.contains("LocalUriHandler.current"));
    assert!(compose.contains(".clickable { try { __crepusUriHandler.openUri("));
    assert!(compose.contains("https://example.com/docs?q=\\$value"));
    assert!(compose.contains("catch (_: IllegalArgumentException)"));
    let empty = render_template_to_ir("a\n  \"Label\"", &TemplateContext::new()).unwrap();
    assert!(
        !generate_native_source(&empty, NativeCodegenTarget::Compose, "EmptyLink")
            .contains("LocalUriHandler")
    );
    assert!(
        !generate_native_source(&empty, NativeCodegenTarget::SwiftUi, "EmptyLink")
            .contains("SwiftUI.Link(")
    );
}

#[test]
fn native_link_destinations_preserve_text_and_report_expression_errors() {
    let mut ctx = TemplateContext::new();
    ctx.set("destination", "https://example.com/path'");
    let ir = render_template_to_ir("a href={destination}\n  \"Docs\"", &ctx).unwrap();
    let crate::ViewNode::Link { href, .. } = &ir.root[0] else {
        panic!("expected link")
    };
    assert_eq!(href, "https://example.com/path'");
    assert!(render_template_to_ir("a href={1 +}\n  \"Docs\"", &ctx).is_err());
}

#[test]
fn evaluated_attribute_quotes_are_content_not_extra_delimiters() {
    let mut ctx = TemplateContext::new();
    ctx.set("label", "'Status'");
    ctx.set("source", "https://example.com/embed'");
    let ir = render_template_to_ir(
        "div\n toggle label={label}\n iframe src={source}\n slider step=\"0.5\"",
        &ctx,
    )
    .unwrap();
    let crate::ViewNode::Stack { children, .. } = &ir.root[0] else {
        panic!("expected stack")
    };
    assert!(matches!(&children[0], crate::ViewNode::Toggle { label, .. } if label == "'Status'"));
    assert!(
        matches!(&children[1], crate::ViewNode::WebView { src, .. } if src == "https://example.com/embed'")
    );
    assert!(
        matches!(&children[2], crate::ViewNode::Slider { step: Some(step), .. } if *step == 0.5)
    );
}

#[test]
fn native_control_label_errors_are_not_silently_dropped() {
    for tag in ["toggle", "checkbox", "slider", "progress", "badge"] {
        let source = format!("{tag} label={{1 +}}");
        assert!(
            render_template_to_ir(&source, &TemplateContext::new()).is_err(),
            "{source}"
        );
    }
    assert!(render_template_to_ir("toggle\n  \"Status {1 +}\"", &TemplateContext::new()).is_err());
    let ir = render_template_to_ir("toggle label=\"\"\n  \"Fallback\"", &TemplateContext::new())
        .unwrap();
    assert!(matches!(&ir.root[0], crate::ViewNode::Toggle { label, .. } if label == "Fallback"));
}

#[test]
fn selection_metadata_evaluates_context_and_preserves_empty_values() {
    let mut ctx = TemplateContext::new();
    ctx.set("tab_value", "settings-panel");
    ctx.set("tab_label", "'Settings'");
    ctx.set("tab_icon", "gearshape");
    let ir = render_template_to_ir("tabs bind=page\n tab value={tab_value} label={tab_label} icon={tab_icon}\n  \"Body\"\n tab value=\"\" label=\"None\"", &ctx).unwrap();
    let crate::ViewNode::Tabs { tabs, .. } = &ir.root[0] else {
        panic!("expected tabs")
    };
    assert_eq!(tabs[0].value, "settings-panel");
    assert_eq!(tabs[0].label, "'Settings'");
    assert_eq!(tabs[0].icon.as_deref(), Some("gearshape"));
    assert_eq!(tabs[1].value, "");
    let ir = render_template_to_ir(
        "picker bind=page\n option label={tab_label} value=\"\"\n  \"Fallback\"",
        &ctx,
    )
    .unwrap();
    let crate::ViewNode::Picker { options, .. } = &ir.root[0] else {
        panic!("expected picker")
    };
    assert_eq!(options[0].label, "'Settings'");
    assert_eq!(options[0].value, "");
    for source in [
        "tabs\n tab label={1 +}",
        "tabs\n tab value={1 +}",
        "tabs\n tab icon={1 +}",
        "picker\n option label={1 +}",
    ] {
        assert!(render_template_to_ir(source, &ctx).is_err(), "{source}");
    }
}

#[test]
fn compose_control_roots_preserve_the_supplied_modifier() {
    for template in [
        "input bind=title",
        "slider bind=volume",
        "progress",
        "picker bind=choice",
        "tabs bind=page",
        "toggle bind=enabled",
        "list",
    ] {
        let ir = render_template_to_ir(template, &TemplateContext::new()).unwrap();
        let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "EmbeddedControl");
        assert!(
            source.contains("modifier = modifier"),
            "{template}: {source}"
        );
    }
    let ir = render_template_to_ir("input bind=title p-4", &TemplateContext::new()).unwrap();
    let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "StyledRoot");
    assert!(source.contains("modifier = modifier.padding("), "{source}");
}

#[test]
fn compose_other_root_nodes_preserve_modifiers_and_gestures() {
    for template in [
        "span\n  \"Text\"",
        "button\n  \"Press\"",
        "badge label=\"Badge\"",
        "meter",
        "divider",
        "spacer",
        "image src=\"image.png\"",
        "scroll\n div",
        "li\n  \"Item\"",
        "filepicker",
    ] {
        let ir = render_template_to_ir(template, &TemplateContext::new()).unwrap();
        let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "ModifiedRoot");
        assert!(
            source.contains("modifier = modifier"),
            "{template}: {source}"
        );
    }
    let ir = render_template_to_ir(
        "button @longpress=hold\n  \"Hold\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "GestureRoot");
    assert_eq!(
        source.matches("modifier.crepusLongPress(\"hold\"").count(),
        1,
        "{source}"
    );
}

#[test]
fn compose_grouped_roots_apply_the_supplied_modifier_once() {
    for template in [
        "if {visible}\n span\n  \"Shown\"",
        "for row in {rows}\n span\n  \"{row.name}\"",
        "span\n \"First\"\nspan\n \"Second\"",
    ] {
        let ir = render_template_to_ir(template, &TemplateContext::new()).unwrap();
        let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "GroupedRoot");
        assert_eq!(
            source.matches("Column(modifier = modifier)").count(),
            1,
            "{source}"
        );
    }
    let ir = render_template_to_ir(
        "div flex-row\n for row in {rows}\n  span\n   \"{row.name}\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "NestedFlow");
    assert!(!source.contains("Column(modifier = modifier)"), "{source}");
    assert!(source.contains("Row(modifier = modifier"), "{source}");
}

#[test]
fn native_spacers_preserve_fractional_sizes() {
    for (size, emitted) in [("0.25", "0.25"), ("0.00001", "1e-5")] {
        let ir =
            render_template_to_ir(&format!("spacer size={size}"), &TemplateContext::new()).unwrap();
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "FineSpacer");
        assert!(
            swift.contains(&format!("Spacer(minLength: {emitted})")),
            "{swift}"
        );
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "FineSpacer");
        assert!(
            compose.contains(&format!(".height({emitted}.dp)")),
            "{compose}"
        );
    }
}

#[test]
fn native_spacer_codegen_defaults_invalid_direct_ir_sizes() {
    let base = render_template_to_ir("spacer", &TemplateContext::new()).unwrap();
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        let expected = generate_native_source(&base, target, "SafeSpacer");
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0] {
            let mut ir = base.clone();
            let crate::ViewNode::Spacer { size, .. } = &mut ir.root[0] else {
                panic!("expected spacer");
            };
            *size = Some(invalid);
            assert_eq!(generate_native_source(&ir, target, "SafeSpacer"), expected);
        }
    }
}

#[test]
fn native_spacers_reject_invalid_explicit_sizes() {
    for source in [
        "spacer size=-1",
        "spacer size=\"NaN\"",
        "spacer size=\"inf\"",
        "spacer size=\"1e100\"",
        "spacer size=\"bad\"",
        "spacer size={1 +}",
    ] {
        assert!(
            render_template_to_ir(source, &TemplateContext::new()).is_err(),
            "{source}"
        );
    }
    for source in [
        "spacer",
        "spacer gap-2",
        "spacer size=0",
        "spacer size=0.25",
    ] {
        assert!(
            render_template_to_ir(source, &TemplateContext::new()).is_ok(),
            "{source}"
        );
    }
}

#[test]
fn native_media_attribute_errors_propagate_without_losing_empty_text() {
    for source in [
        "image src={1 +}",
        "img alt={1 +}",
        "image placeholder={1 +}",
        "iframe src={1 +}",
    ] {
        assert!(
            render_template_to_ir(source, &TemplateContext::new()).is_err(),
            "{source}"
        );
    }
    let ir = render_template_to_ir(
        "image src=\"asset.png\" alt=\"\" placeholder=\"\"",
        &TemplateContext::new(),
    )
    .unwrap();
    assert!(
        matches!(&ir.root[0],crate::ViewNode::Image { src,alt:Some(alt),placeholder:Some(placeholder),.. } if src == "asset.png" && alt.is_empty() && placeholder.is_empty())
    );
}

#[test]
fn native_dividers_preserve_axis_and_vertical_extent() {
    for (source, height) in [
        ("divider flex-row", "16.0"),
        ("divider flex-row h-8", "32.0"),
    ] {
        let ir = render_template_to_ir(source, &TemplateContext::new()).unwrap();
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "DividerScreen");
        assert!(swift.contains("HStack(spacing: 0) { Divider() }"));
        assert!(swift.contains(&format!(".frame(width: 1, height: {height})")));
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "DividerScreen");
        assert!(compose.contains(&format!(
            "VerticalDivider(modifier = modifier.height({height}.dp)"
        )));
    }
    let ir = render_template_to_ir("divider", &TemplateContext::new()).unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "DividerScreen");
    assert!(swift.contains("VStack(spacing: 0) { Divider() }"));
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "DividerScreen");
    assert!(compose.contains("HorizontalDivider(modifier = modifier)"));
}

#[test]
fn file_controls_propagate_metadata_expression_errors() {
    for tag in ["dropzone", "file-picker", "filepicker", "media-picker"] {
        for template in [
            format!("{tag} accept={{1 +}}"),
            format!("{tag}\n  \"{{1 +}}\""),
        ] {
            assert!(
                render_template_to_ir(&template, &TemplateContext::new()).is_err(),
                "{template}"
            );
        }
        let mut ctx = TemplateContext::new();
        ctx.set("types", "image/png, image/jpeg");
        let ir =
            render_template_to_ir(&format!("{tag} accept={{types}}\n  \"Upload\""), &ctx).unwrap();
        let json = serde_json::to_value(ir).unwrap();
        assert_eq!(json["root"][0]["label"], "Upload");
        assert!(json["root"][0]["accept"].to_string().contains("image/png"));
    }
}

#[test]
fn link_and_badge_metadata_propagates_expression_errors() {
    for template in ["a target={1 +}", "link rel={1 +}", "badge tone={1 +}"] {
        assert!(
            render_template_to_ir(template, &TemplateContext::new()).is_err(),
            "{template}"
        );
    }
    let mut ctx = TemplateContext::new();
    ctx.set("target", " _blank ");
    ctx.set("rel", " noopener noreferrer ");
    ctx.set("tone", " success ");
    let ir = render_template_to_ir(
        "a target={target} rel={rel}\n badge tone={tone}\n  \"Saved\"",
        &ctx,
    )
    .unwrap();
    let json = serde_json::to_value(ir).unwrap();
    assert_eq!(json["root"][0]["target"], "_blank");
    assert_eq!(json["root"][0]["rel"], "noopener noreferrer");
    assert_eq!(json["root"][0]["children"][0]["tone"], "success");
    let ir = render_template_to_ir("a target=\" \" rel=\"\"", &ctx).unwrap();
    let json = serde_json::to_value(ir).unwrap();
    assert!(json["root"][0].get("target").is_none());
    assert!(json["root"][0].get("rel").is_none());
}

#[test]
fn file_picker_multiple_evaluates_boolean_values() {
    for tag in ["file-picker", "filepicker", "media-picker"] {
        for (suffix, expected) in [
            ("", false),
            (" multiple", true),
            (" multiple={false}", false),
            (" multiple={true}", true),
        ] {
            let ir =
                render_template_to_ir(&format!("{tag}{suffix}"), &TemplateContext::new()).unwrap();
            assert_eq!(
                serde_json::to_value(ir).unwrap()["root"][0]["multiple"],
                expected,
                "{tag}{suffix}"
            );
        }
        assert!(
            render_template_to_ir(&format!("{tag} multiple={{1 +}}"), &TemplateContext::new())
                .is_err()
        );
        let mut ctx = TemplateContext::new();
        ctx.set("allow_many", false);
        let ir = render_template_to_ir(&format!("{tag} multiple={{allow_many}}"), &ctx).unwrap();
        assert_eq!(
            serde_json::to_value(ir).unwrap()["root"][0]["multiple"],
            false
        );
    }
}

#[test]
fn native_checked_controls_preserve_boolean_attributes() {
    for tag in ["checkbox", "toggle", "switch", "input type=checkbox"] {
        for (attribute, expected) in [
            ("", false),
            (" checked", true),
            (" checked=\"\"", true),
            (" checked={false}", false),
            (" checked={true}", true),
        ] {
            let template = format!("{tag}{attribute}");
            let ir = render_template_to_ir(&template, &TemplateContext::new()).unwrap();
            assert_eq!(
                serde_json::to_value(ir).unwrap()["root"][0]["checked"],
                expected,
                "{template}"
            );
        }
    }
}

#[test]
fn numeric_controls_propagate_expression_errors() {
    for (tag, props) in [
        ("slider", &["value", "min", "max", "step"][..]),
        ("progress", &["value", "max"][..]),
        ("meter", &["value", "min", "max"][..]),
    ] {
        for prop in props {
            let template = format!("{tag} {prop}={{1 +}}");
            assert!(
                render_template_to_ir(&template, &TemplateContext::new()).is_err(),
                "{template}"
            );
        }
        let mut ctx = TemplateContext::new();
        ctx.set("amount", 25);
        let ir = render_template_to_ir(&format!("{tag} value={{amount + 5}}"), &ctx).unwrap();
        assert_eq!(serde_json::to_value(ir).unwrap()["root"][0]["value"], 30.0);
    }
}

#[test]
fn checked_controls_propagate_expression_errors() {
    for tag in ["toggle", "switch", "checkbox", "input type=checkbox"] {
        let template = format!("{tag} checked={{1 +}}");
        assert!(
            render_template_to_ir(&template, &TemplateContext::new()).is_err(),
            "{template}"
        );
        let mut ctx = TemplateContext::new();
        ctx.set("enabled", true);
        let ir = render_template_to_ir(&format!("{tag} checked={{enabled}}"), &ctx).unwrap();
        assert_eq!(
            serde_json::to_value(ir).unwrap()["root"][0]["checked"],
            true
        );
    }
}

#[test]
fn button_labels_evaluate_attributes_and_fall_back_to_children() {
    let mut ctx = TemplateContext::new();
    ctx.set("caption", " Save ");
    for (template, expected) in [
        ("button label={caption}\n  \"Fallback\"", "Save"),
        ("button label=\" \"\n  \"Fallback\"", "Fallback"),
        ("button\n  \"Fallback\"", "Fallback"),
    ] {
        let ir = render_template_to_ir(template, &ctx).unwrap();
        assert_eq!(
            serde_json::to_value(&ir).unwrap()["root"][0]["label"],
            expected
        );
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "LabeledButton");
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "LabeledButton");
        assert!(swift.contains(&format!("Text(\"{expected}\")")));
        assert!(compose.contains(&format!("Text(\"{expected}\")")));
    }
    assert!(render_template_to_ir("button label={1 +}", &ctx).is_err());
}

#[test]
fn meters_reject_reversed_and_overflowing_ranges() {
    for (template, diagnostic) in [
        ("meter min=10 max=5", "meter min must not exceed max"),
        (
            "meter min=-3e38 max=3e38",
            "meter range width must be finite",
        ),
    ] {
        let error = render_template_to_ir(template, &TemplateContext::new()).unwrap_err();
        assert!(error.to_string().contains(diagnostic), "{error}");
    }
    for template in [
        "meter",
        "meter min=-10 max=10",
        "meter min=5 max=5",
        "meter value=200 min=0 max=100",
    ] {
        assert!(
            render_template_to_ir(template, &TemplateContext::new()).is_ok(),
            "{template}"
        );
    }
}

#[test]
fn native_meters_render_normalized_progress_controls() {
    for (min, max, value, expected) in [
        (10, 20, 15, "0.5"),
        (10, 20, 0, "0.0"),
        (10, 20, 30, "1.0"),
        (5, 5, 5, "1.0"),
        (5, 5, 4, "0.0"),
    ] {
        let ir = render_template_to_ir(
            &format!("meter min={min} max={max} value={value} label=\"Storage\""),
            &TemplateContext::new(),
        )
        .unwrap();
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "MeterScreen");
        assert!(
            swift.contains(&format!(
                "ProgressView(\"Storage\", value: {expected}, total: 1.0)"
            )),
            "{swift}"
        );
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "MeterScreen");
        assert!(
            compose.contains(&format!(
                "LinearProgressIndicator(progress = {expected}f / 1.0f)"
            )),
            "{compose}"
        );
        assert!(compose.contains("Column(modifier = modifier)"));
        assert!(compose.contains("Text(\"Storage\")"));
    }
}

#[test]
fn native_progress_codegen_sanitizes_direct_ir_values() {
    for (value, max, expected_value, expected_max) in [
        (-10.0, 100.0, "0.0", "100.0"),
        (200.0, 100.0, "100.0", "100.0"),
        (f32::NAN, 100.0, "0.0", "100.0"),
        (10.0, 0.0, "0.0", "1.0"),
        (10.0, f32::INFINITY, "0.0", "1.0"),
    ] {
        let mut ir = render_template_to_ir("progress", &TemplateContext::new()).unwrap();
        ir.root[0] = crate::ViewNode::Progress {
            label: None,
            value,
            max,
            style: None,
        };
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "SafeProgress");
        assert!(
            swift.contains(&format!("value: {expected_value}, total: {expected_max}")),
            "{swift}"
        );
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "SafeProgress");
        assert!(
            compose.contains(&format!("progress = {expected_value}f / {expected_max}f")),
            "{compose}"
        );
    }
}

#[test]
fn file_controls_evaluate_explicit_labels() {
    let mut ctx = TemplateContext::new();
    ctx.set("caption", " Upload ");
    for tag in ["dropzone", "file-picker", "filepicker", "media-picker"] {
        for (attribute, expected) in [("label={caption}", "Upload"), ("label=\" \"", "Fallback")] {
            let template = format!("{tag} {attribute}\n  \"Fallback\"");
            let ir = render_template_to_ir(&template, &ctx).unwrap();
            assert_eq!(
                serde_json::to_value(ir).unwrap()["root"][0]["label"],
                expected,
                "{template}"
            );
        }
        assert!(render_template_to_ir(&format!("{tag} label={{1 +}}"), &ctx).is_err());
    }
}

#[test]
fn native_slider_codegen_sanitizes_direct_ir_numbers() {
    for (value, min, max, step, expected_value, expected_min, expected_max, expected_step) in [
        (30.0, 10.0, 20.0, Some(2.0), 20.0, 10.0, 20.0, Some(2.0)),
        (-5.0, 10.0, 20.0, None, 10.0, 10.0, 20.0, None),
        (f32::NAN, 10.0, 20.0, None, 10.0, 10.0, 20.0, None),
        (f32::INFINITY, 10.0, 20.0, None, 10.0, 10.0, 20.0, None),
        (15.0, 10.0, 20.0, Some(0.0), 15.0, 10.0, 20.0, None),
        (15.0, 10.0, 20.0, Some(-1.0), 15.0, 10.0, 20.0, None),
        (15.0, 10.0, 20.0, Some(f32::NAN), 15.0, 10.0, 20.0, None),
        (
            15.0,
            10.0,
            20.0,
            Some(f32::INFINITY),
            15.0,
            10.0,
            20.0,
            None,
        ),
        (15.0, f32::NAN, 20.0, Some(2.0), 0.0, 0.0, 100.0, None),
        (15.0, 10.0, f32::INFINITY, None, 0.0, 0.0, 100.0, None),
        (15.0, 20.0, 10.0, None, 0.0, 0.0, 100.0, None),
        (0.0, -f32::MAX, f32::MAX, Some(1.0), 0.0, 0.0, 100.0, None),
        (5.0, 2.0, 2.0, Some(1.0), 2.0, 2.0, 2.0, None),
        (3.0, 0.0, 10.0, Some(20.0), 3.0, 0.0, 10.0, None),
    ] {
        let ir = ViewIr {
            version: IR_VERSION,
            root: vec![crate::ViewNode::Slider {
                label: None,
                bind: None,
                value,
                min,
                max,
                step,
                on_change: None,
                style: None,
            }],
        };
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "SafeSlider");
        assert!(
            swift.contains(&format!(".constant({expected_value:?})")),
            "{swift}"
        );
        assert!(
            swift.contains(&format!("in: {expected_min:?}...{expected_max:?}")),
            "{swift}"
        );
        match expected_step {
            Some(step) => assert!(swift.contains(&format!(", step: {step:?}")), "{swift}"),
            None => assert!(!swift.contains(", step:"), "{swift}"),
        }
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "SafeSlider");
        assert!(
            compose.contains(&format!("Slider(value = {expected_value:?}f,")),
            "{compose}"
        );
        assert!(
            compose.contains(&format!(
                "valueRange = {expected_min:?}f..{expected_max:?}f"
            )),
            "{compose}"
        );
        if expected_step.is_none() {
            assert!(!compose.contains(", steps ="), "{compose}");
        }
    }
}

#[test]
fn native_slider_codegen_handles_f32_overflow_from_json() {
    let source = format!(
        r#"{{"version":{IR_VERSION},"root":[{{"kind":"slider","value":1e100,"min":0,"max":10,"step":1e100}}]}}"#
    );
    let ir: ViewIr = serde_json::from_str(&source).unwrap();
    let crate::ViewNode::Slider { value, step, .. } = &ir.root[0] else {
        panic!("expected slider")
    };
    assert!(value.is_infinite());
    assert!(step.is_some_and(|step| step.is_infinite()));
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        let source = generate_native_source(&ir, target, "JsonSlider");
        assert!(!source.contains("inf"), "{source}");
        assert!(!source.contains("NaN"), "{source}");
    }
}

#[test]
fn native_style_codegen_preserves_fractional_padding_and_margins() {
    let mut ir = render_template_to_ir("div", &TemplateContext::new()).unwrap();
    let style = ir.root[0].style_mut();
    *style = Some(crate::ViewStyle {
        padding: Some(0.5),
        padding_horizontal: Some(1.25),
        padding_top: Some(2.75),
        margin: Some(3.5),
        margin_vertical: Some(4.25),
        margin_right: Some(5.75),
        width: Some(12.25),
        height: Some(24.5),
        corner_radius: Some(6.75),
        border_width: Some(0.75),
        ..Default::default()
    });
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "FractionalStyle");
    for expected in [
        ".padding(0.5)",
        ".padding(.horizontal, 1.25)",
        ".padding(.top, 2.75)",
        ".padding(3.5)",
        ".padding(.vertical, 4.25)",
        ".padding(.trailing, 5.75)",
    ] {
        assert!(swift.contains(expected), "missing {expected}: {swift}");
    }
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "FractionalStyle");
    for expected in [
        ".padding(0.5.dp)",
        "horizontal = 1.25.dp",
        "top = 2.75.dp",
        ".padding(3.5.dp)",
        "vertical = 4.25.dp",
        "end = 5.75.dp",
        ".width(12.25.dp)",
        ".height(24.5.dp)",
        "RoundedCornerShape(6.75.dp)",
        ".border(0.75.dp,",
    ] {
        assert!(compose.contains(expected), "missing {expected}: {compose}");
    }
}

#[test]
fn native_style_codegen_omits_nonfinite_numeric_hints() {
    for tag in [
        "text probe",
        "button label=probe",
        "div direction=row\n divider",
    ] {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut ir = render_template_to_ir(tag, &TemplateContext::new()).unwrap();
            if tag == "text probe" {
                ir.root[0] = crate::ViewNode::Text {
                    content: "probe".into(),
                    bind: None,
                    style: None,
                };
            }
            *ir.root[0].style_mut() = Some(crate::ViewStyle {
                padding: Some(bad),
                padding_horizontal: Some(bad),
                padding_vertical: Some(bad),
                padding_top: Some(bad),
                padding_bottom: Some(bad),
                padding_left: Some(bad),
                padding_right: Some(bad),
                margin: Some(bad),
                margin_horizontal: Some(bad),
                margin_vertical: Some(bad),
                margin_top: Some(bad),
                margin_bottom: Some(bad),
                margin_left: Some(bad),
                margin_right: Some(bad),
                width: Some(bad),
                height: Some(bad),
                min_width: Some(bad),
                min_height: Some(bad),
                max_width: Some(bad),
                max_height: Some(bad),
                aspect_ratio: Some(bad),
                font_size: Some(bad),
                line_height: Some(bad),
                letter_spacing: Some(bad),
                corner_radius: Some(bad),
                border_width: Some(bad),
                opacity: Some(bad),
                translate_x: Some(bad),
                translate_y: Some(bad),
                scale_x: Some(bad),
                scale_y: Some(bad),
                rotate: Some(bad),
                shadow_radius: Some(bad),
                shadow_offset_x: Some(bad),
                shadow_offset_y: Some(bad),
                ..Default::default()
            });
            if let crate::ViewNode::Stack {
                children, style, ..
            } = &mut ir.root[0]
            {
                *children[0].style_mut() = style.clone();
            }
            for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
                let source = generate_native_source(&ir, target, "FiniteStyle");
                assert!(!source.contains("NaN"), "{source}");
                assert!(!source.contains("inf"), "{source}");
            }
        }
    }
}

#[test]
fn native_style_codegen_filters_derived_line_height_overflow_and_keeps_sizing_sentinels() {
    let mut ir = render_template_to_ir("text probe", &TemplateContext::new()).unwrap();
    ir.root[0] = crate::ViewNode::Text {
        content: "probe".into(),
        bind: None,
        style: None,
    };
    *ir.root[0].style_mut() = Some(crate::ViewStyle {
        line_height: Some(f32::MAX),
        width: Some(-1.0),
        height: Some(-2.0),
        ..Default::default()
    });
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "FiniteStyle");
    assert!(!compose.contains("lineHeight ="), "{compose}");
    assert!(compose.contains(".fillMaxWidth()"), "{compose}");
    assert!(!compose.contains("height(-2"), "{compose}");
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "FiniteStyle");
    assert!(swift.contains("maxWidth: .infinity"), "{swift}");
    assert!(!swift.contains("height: -2"), "{swift}");
}

#[test]
fn native_style_codegen_keeps_valid_transform_axes() {
    let mut ir = render_template_to_ir("div", &TemplateContext::new()).unwrap();
    *ir.root[0].style_mut() = Some(crate::ViewStyle {
        translate_x: Some(f32::NAN),
        translate_y: Some(2.5),
        scale_x: Some(f32::INFINITY),
        scale_y: Some(0.75),
        ..Default::default()
    });
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "FiniteStyle");
    assert!(swift.contains(".offset(x: 0.0, y: 2.5)"), "{swift}");
    assert!(
        swift.contains(".scaleEffect(x: 1.000, y: 0.750)"),
        "{swift}"
    );
    let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "FiniteStyle");
    assert!(
        compose.contains(".offset(x = 0.dp, y = 2.5.dp)"),
        "{compose}"
    );
    assert!(
        compose.contains(".scale(scaleX = 1.000f, scaleY = 0.750f)"),
        "{compose}"
    );
}

#[test]
fn native_style_codegen_handles_f32_overflow_from_json() {
    let json = format!(
        r#"{{"version":{IR_VERSION},"root":[{{"kind":"text","content":"probe","style":{{"padding":1e100,"translateX":1e100,"scaleY":2.0}}}}]}}"#
    );
    let ir: ViewIr = serde_json::from_str(&json).unwrap();
    let crate::ViewNode::Text {
        style: Some(style), ..
    } = &ir.root[0]
    else {
        panic!("expected styled text");
    };
    assert!(style.padding.unwrap().is_infinite());
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        let source = generate_native_source(&ir, target, "FiniteStyle");
        assert!(!source.contains("inf"), "{source}");
        assert!(!source.contains("NaN"), "{source}");
    }
}

#[test]
fn compose_style_codegen_uses_floating_literals_outside_kotlin_int_range() {
    for value in [2147483648.0_f32, 3e9, f32::MAX, -3e9, -f32::MAX] {
        let mut ir = render_template_to_ir("button label=probe", &TemplateContext::new()).unwrap();
        *ir.root[0].style_mut() = Some(crate::ViewStyle {
            padding: Some(value),
            margin: Some(value),
            width: Some(value),
            height: Some(value),
            min_width: Some(value),
            max_height: Some(value),
            corner_radius: Some(value),
            border_width: Some(value),
            translate_x: Some(value),
            translate_y: Some(value),
            ..Default::default()
        });
        let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "LargeDimensions");
        assert!(!source.contains(&format!("{value}.dp")), "{source}");
        assert!(source.contains(&format!("{value:?}.dp")), "{source}");
    }
}

#[test]
fn native_stack_codegen_defaults_nonfinite_spacing() {
    for template in ["div flex-col", "div flex-row", "div flex-row flex-wrap"] {
        let mut ir = render_template_to_ir(template, &TemplateContext::new()).unwrap();
        for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
            let expected = generate_native_source(&ir, target, "SpacingScreen");
            for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let crate::ViewNode::Stack { spacing, .. } = &mut ir.root[0] else {
                    panic!("expected stack");
                };
                *spacing = Some(invalid);
                assert_eq!(
                    generate_native_source(&ir, target, "SpacingScreen"),
                    expected
                );
            }
            let crate::ViewNode::Stack { spacing, .. } = &mut ir.root[0] else {
                panic!("expected stack");
            };
            *spacing = None;
        }
    }
}

#[test]
fn compose_stack_codegen_preserves_large_spacing() {
    let mut ir = render_template_to_ir("div", &TemplateContext::new()).unwrap();
    let crate::ViewNode::Stack { spacing, .. } = &mut ir.root[0] else {
        panic!("expected stack");
    };
    *spacing = Some(f32::MAX);
    let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "LargeSpacing");
    assert!(
        source.contains(&format!("Arrangement.spacedBy({:?}.dp)", f32::MAX)),
        "{source}"
    );
}

#[test]
fn swiftui_style_codegen_combines_fill_fixed_and_fit_dimensions() {
    for (width, height, expected) in [
        (-1.0, 24.0, ".frame(width: nil, height: 24.0"),
        (12.0, -1.0, ".frame(width: 12.0, height: nil"),
        (-2.0, 24.0, ".fixedSize(horizontal: true, vertical: false)"),
        (12.0, -2.0, ".fixedSize(horizontal: false, vertical: true)"),
        (-2.0, -2.0, ".fixedSize(horizontal: true, vertical: true)"),
    ] {
        let mut ir = render_template_to_ir("div", &TemplateContext::new()).unwrap();
        *ir.root[0].style_mut() = Some(crate::ViewStyle {
            width: Some(width),
            height: Some(height),
            ..Default::default()
        });
        let source = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "SizingDimensions");
        assert!(source.contains(expected), "{source}");
        if width == -1.0 {
            assert!(source.contains("maxWidth: .infinity"), "{source}");
        }
        if height == -1.0 {
            assert!(source.contains("maxHeight: .infinity"), "{source}");
        }
    }
}

#[test]
fn native_vertical_dividers_handle_fill_and_fit_height_sentinels() {
    for (class, fills) in [("h-full", true), ("h-fit", false)] {
        let ir = render_template_to_ir(
            &format!("divider flex-row {class}"),
            &TemplateContext::new(),
        )
        .unwrap();
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "DividerSizing");
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "DividerSizing");
        assert!(!swift.contains("height: -"), "{swift}");
        assert!(!compose.contains(".height(-"), "{compose}");
        if fills {
            assert!(swift.contains(".frame(width: 1, height: nil)"), "{swift}");
            assert!(swift.contains("maxHeight: .infinity"), "{swift}");
            assert!(compose.contains("modifier.fillMaxHeight()"), "{compose}");
            assert!(!compose.contains(".height(16.0.dp)"), "{compose}");
        } else {
            assert!(swift.contains(".frame(width: 1, height: 16.0)"), "{swift}");
            assert!(compose.contains(".height(16.0.dp)"), "{compose}");
        }
    }
}

#[test]
fn native_codegen_omits_negative_padding_and_retains_valid_edges() {
    for template in ["div", "button label=probe"] {
        let mut ir = render_template_to_ir(template, &TemplateContext::new()).unwrap();
        *ir.root[0].style_mut() = Some(crate::ViewStyle {
            padding: Some(-2.0),
            padding_horizontal: Some(-2.0),
            padding_vertical: Some(-2.0),
            padding_top: Some(-2.0),
            padding_bottom: Some(-2.0),
            padding_left: Some(1.25),
            padding_right: Some(-2.0),
            ..Default::default()
        });
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "PaddingBounds");
        assert!(!swift.contains(".padding(-2"), "{swift}");
        assert!(!swift.contains(", -2"), "{swift}");
        assert!(swift.contains(".padding(.leading, 1.25)"), "{swift}");
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "PaddingBounds");
        assert!(!compose.contains("-2.dp"), "{compose}");
        assert!(compose.contains("start = 1.25.dp"), "{compose}");
    }
}

#[test]
fn native_padding_classes_ignore_invalid_negative_overrides() {
    let ir =
        render_template_to_ir("div p-4 p-[-2px] pl-[1.25px]", &TemplateContext::new()).unwrap();
    let crate::ViewNode::Stack {
        style: Some(style), ..
    } = &ir.root[0]
    else {
        panic!("expected styled stack");
    };
    assert_eq!(style.padding, Some(16.0));
    assert_eq!(style.padding_left, Some(1.25));
    assert!(style.classes.iter().any(|class| class == "p-[-2px]"));
}

#[test]
fn native_maximum_fill_dimensions_preserve_explicit_divider_height() {
    for (classes, explicit_height) in [
        ("max-h-full", None),
        ("max-h-screen", None),
        ("h-8 max-h-full", Some("32.0")),
        ("h-fit max-h-full", Some("16.0")),
    ] {
        let ir = render_template_to_ir(
            &format!("divider flex-row {classes}"),
            &TemplateContext::new(),
        )
        .unwrap();
        let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "MaximumFill");
        let compose = generate_native_source(&ir, NativeCodegenTarget::Compose, "MaximumFill");
        assert!(swift.contains("maxHeight: .infinity"), "{swift}");
        assert!(compose.contains(".fillMaxHeight()"), "{compose}");
        if let Some(height) = explicit_height {
            assert!(
                swift.contains(&format!(".frame(width: 1, height: {height})")),
                "{swift}"
            );
            assert!(
                compose.contains(&format!(".height({height}.dp)")),
                "{compose}"
            );
        } else {
            assert!(swift.contains(".frame(width: 1, height: nil)"), "{swift}");
            assert!(!compose.contains(".height(16.0.dp)"), "{compose}");
        }
    }
    let ir = render_template_to_ir("div max-w-full max-h-full", &TemplateContext::new()).unwrap();
    let swift = generate_native_source(&ir, NativeCodegenTarget::SwiftUi, "MaximumFill");
    assert!(swift.contains("maxWidth: .infinity"), "{swift}");
    assert!(swift.contains("maxHeight: .infinity"), "{swift}");
}

#[test]
fn compose_signed_margins_use_layout_instead_of_negative_padding() {
    for template in ["div m-[-4px]", "div mx-[-4px]", "div mt-[-4px] mr-[2px]"] {
        let ir = render_template_to_ir(template, &TemplateContext::new()).unwrap();
        let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "SignedMargin");
        assert!(source.contains(".crepusMargin("), "{source}");
        assert!(!source.contains(".padding(-4.dp)"), "{source}");
        for padding in source.split(".padding(").skip(1) {
            assert!(
                !padding.split(')').next().unwrap().contains("-4.dp"),
                "{source}"
            );
        }
    }
    let ir = render_template_to_ir("div m-4", &TemplateContext::new()).unwrap();
    let source = generate_native_source(&ir, NativeCodegenTarget::Compose, "PositiveMargin");
    assert!(source.contains(".padding(16.dp)"), "{source}");
    assert!(!source.contains("crepusMargin"), "{source}");
}

#[test]
fn native_standard_negative_margin_classes_lower_with_signed_values() {
    let ir =
        render_template_to_ir("div -m-4 -mx-2 -mt-px -mr-[-2px]", &TemplateContext::new()).unwrap();
    let crate::ViewNode::Stack {
        style: Some(style), ..
    } = &ir.root[0]
    else {
        panic!("expected styled stack");
    };
    assert_eq!(style.margin, Some(-16.0));
    assert_eq!(style.margin_horizontal, Some(-8.0));
    assert_eq!(style.margin_top, Some(-1.0));
    assert_eq!(style.margin_right, Some(2.0));
}

#[test]
fn native_input_edit_setters_dispatch_both_actions_after_one_binding_write() {
    for (multiline, secure) in [(false, false), (true, false), (false, true)] {
        for bind in ["title", ""] {
            for on_change in [Some("changed"), None] {
                let ir: ViewIr = serde_json::from_value(json!({
                    "version": IR_VERSION,
                    "root": [{"kind":"input", "placeholder":"Edit", "bind":bind,
                        "multiline":multiline, "secure":secure,
                        "onChange":on_change, "onInput":"edited"}]
                }))
                .unwrap();
                for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
                    let source = generate_native_source(&ir, target, "InputEvents");
                    assert_eq!(
                        source.matches("CrepusActions.performChange(").count(),
                        usize::from(!bind.is_empty()),
                        "{source}"
                    );
                    assert_eq!(
                        source.matches("CrepusActions.perform(\"edited\")").count(),
                        1,
                        "{source}"
                    );
                    assert_eq!(
                        source.matches("\"changed\"").count(),
                        usize::from(on_change.is_some()),
                        "{source}"
                    );
                    if !bind.is_empty() {
                        assert!(
                            source.find("CrepusActions.performChange(").unwrap()
                                < source.find("CrepusActions.perform(\"edited\")").unwrap(),
                            "{source}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn input_action_only_changes_replace_the_ir_node() {
    let old: ViewIr = serde_json::from_value(json!({
        "version": IR_VERSION,
        "root": [{"kind":"input", "placeholder":"Edit", "bind":"title", "onChange":"changed"}]
    }))
    .unwrap();
    let mut new = old.clone();
    let crate::ViewNode::Input { on_input, .. } = &mut new.root[0] else {
        panic!("input")
    };
    *on_input = Some("edited".into());
    let mutations = diff_ir(&old, &new);
    assert!(
        matches!(&mutations[..], [crate::IrMutation::ReplaceNode { path, node }] if path == &vec![0] && node == &new.root[0])
    );
    let mut patched = old;
    apply_mutations(&mut patched, &mutations).unwrap();
    assert_eq!(patched, new);
}

#[test]
fn nested_native_loops_keep_ancestor_bindings_and_shadow_locals() {
    let ir = render_template_to_ir(
        "div\n for row in rows\n  for child in row.children\n   input bind=row.title\n   input bind=child.title\n   span \"{row.title} / {child.title}\"\n   if {row.title == child.title}\n    span \"equal\"\n   for row in child.rows\n    input bind=row.title\n  input bind=row.title",
        &TemplateContext::new(),
    ).unwrap();
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        let source = generate_native_source(&ir, target, "Lexical");
        for path in [
            "`__crepusItem_726f77__crepusPath` + \".title\"",
            "`__crepusItem_6368696c64__crepusPath` + \".title\"",
            "`__crepusItem_726f77__depth2__crepusPath` + \".title\"",
        ] {
            assert!(source.contains(path), "{source}");
        }
        assert!(source.contains("@crepus:lexical"), "{source}");
        let separator = if target == NativeCodegenTarget::SwiftUi {
            ": "
        } else {
            " = "
        };
        assert!(source.contains(&format!("text(\"row.title\", scopeName{separator}\"row\", scope{separator}`__crepusItem_726f77`)")), "{source}");
        assert!(source.contains(&format!("text(\"child.title\", scopeName{separator}\"child\", scope{separator}`__crepusItem_6368696c64`)")), "{source}");
        assert!(!source.contains("path: \"row.title\""), "{source}");
        assert!(
            !source.contains("performChange(\"\", \"row.title\""),
            "{source}"
        );
    }
}

#[test]
fn slot_rotate_rejects_duplicate_intervals() {
    for attributes in [
        "interval=100 interval=200",
        "interval={delay} interval=200",
        "interval=100 interval={delay}",
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("delay", 500);
        let template = format!("slot-rotate {attributes}\n  \"one\"\n  \"two\"");
        let error = render_template_to_ir(&template, &ctx).unwrap_err();
        assert!(error.to_string().contains("duplicate interval"), "{error}");
    }
}

#[test]
fn slot_rotate_retains_interval_values_fallback_and_evaluation_errors() {
    let mut ctx = TemplateContext::new();
    ctx.set("delay", 500);
    for (attribute, expected) in [
        ("", 3200),
        ("interval=100", 100),
        ("interval=0", 0),
        ("interval={delay}", 500),
        ("interval=\"oops\"", 3200),
        ("interval=-1", 3200),
        ("interval=\"18446744073709551616\"", 3200),
    ] {
        let template = format!("slot-rotate {attribute}\n  \"one\"\n  \"two\"");
        let ir = render_template_to_ir(&template, &ctx).unwrap();
        let crate::ViewNode::SlotRotate { interval_ms, .. } = &ir.root[0] else {
            panic!("expected slot-rotate");
        };
        assert_eq!(*interval_ms, expected, "{attribute}");
    }
    assert!(render_template_to_ir("slot-rotate interval={1 +}\n  \"one\"", &ctx).is_err());
}
