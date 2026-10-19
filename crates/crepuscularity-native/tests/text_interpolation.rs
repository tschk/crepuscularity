use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

#[test]
fn jsx_declaration_defaults_preserve_explicit_values() {
    let source = "<let-default name=\"count\" value={3} /><span>{count}</span>";
    for (explicit, expected) in [(None, "3"), (Some(7_i64), "7")] {
        let mut ctx = TemplateContext::new();
        if let Some(value) = explicit {
            ctx.set("count", value);
        }
        let ir =
            render_template_to_ir_with_path(source, &ctx, Some(Path::new("view.jsx"))).unwrap();
        let json = crepuscularity_native::to_json(&ir).unwrap();
        assert!(
            json.contains(&format!("\"content\":\"{expected}\"")),
            "{json}"
        );
    }
}

#[test]
fn large_integral_float_component_defaults_remain_float_expressions() {
    let source = "+++\n[Value.defaults]\namount = 1e20\n+++\n--- Value\nspan\n  \"{amount}\"";
    let component_file = crepuscularity_core::parser::parse_component_file(source).unwrap();
    let expression = &component_file.components["Value"].meta.defaults["amount"];
    let value = crepuscularity_core::eval::eval_expr(expression, &TemplateContext::new()).unwrap();
    assert!(
        matches!(value, crepuscularity_core::context::TemplateValue::Float(value) if value == 1e20)
    );
    let ir = crepuscularity_native::render_component_file_to_ir(
        source,
        "Value",
        &TemplateContext::new(),
    )
    .unwrap();
    let json = serde_json::to_string(&ir).unwrap();
    assert!(json.contains("100000000000000000000"), "{json}");
}

#[test]
fn component_default_backslashes_remain_literal() {
    let source = "+++\n[Path.defaults]\nplaceholder = 'C:\\new\\tools'\n+++\n--- Path\ninput placeholder={placeholder}";
    let ir =
        crepuscularity_native::render_component_file_to_ir(source, "Path", &TemplateContext::new())
            .unwrap();
    let json = serde_json::to_value(&ir).unwrap();
    assert_eq!(
        json["root"][0]["placeholder"].as_str(),
        Some(r"C:\new\tools")
    );
}

#[test]
fn literal_attribute_backslashes_are_not_expression_escape_sequences() {
    for (filename, source) in [
        (
            "Path.vue",
            r#"<template><input placeholder="C:\new\tools" /></template>"#,
        ),
        ("Path.ng.html", r#"<input placeholder="C:\new\tools" />"#),
        ("Path.jsx", r#"<input placeholder="C:\new\tools" />"#),
        ("Path.svelte", r#"<input placeholder="C:\new\tools" />"#),
        ("Path.astro", r#"<input placeholder="C:\new\tools" />"#),
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        let json = serde_json::to_value(&ir).unwrap();
        assert_eq!(
            json["root"][0]["placeholder"].as_str(),
            Some(r"C:\new\tools"),
            "{filename}: {json}"
        );
    }
}

#[test]
fn moustache_interpolation_handles_quoted_delimiters_and_unicode() {
    for (filename, source) in [
        (
            "Text.vue",
            "<template><p>雪 {{ '}}' }} and {{ '<' }} after</p></template>",
        ),
        ("Text.ng.html", "<p>雪 {{ '}}' }} and {{ '<' }} after</p>"),
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        let json = serde_json::to_string(&ir).unwrap();
        assert!(json.contains("雪 }} and < after"), "{filename}: {json}");
    }
    for (filename, source) in [
        ("Bad.vue", "<template><p>{{ name</p></template>"),
        ("Bad.ng.html", "<p>{{ name</p>"),
    ] {
        let error = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap_err();
        assert!(error.to_string().contains("unclosed moustache"), "{error}");
    }
}

#[test]
fn astro_scalar_boolean_expressions_are_text_in_both_positions() {
    let mut ctx = TemplateContext::new();
    ctx.set("ready", false);
    ctx.set("enabled", true);
    for (source, expected) in [
        ("<p>{ready && enabled}</p>", "false"),
        (
            "<p>Before {ready && enabled} after</p>",
            "Before false after",
        ),
    ] {
        let ir =
            render_template_to_ir_with_path(source, &ctx, Some(Path::new("Bool.astro"))).unwrap();
        let json = serde_json::to_string(&ir).unwrap();
        assert!(
            json.contains(&format!("\"content\":\"{expected}\"")),
            "{json}"
        );
    }
}

#[test]
fn astro_comparisons_inside_text_preserve_adjacent_spaces() {
    let mut ctx = TemplateContext::new();
    ctx.set("count", 1);
    ctx.set("limit", 2);
    let ir = render_template_to_ir_with_path(
        "<p>Before {count < limit} after</p>",
        &ctx,
        Some(Path::new("Compare.astro")),
    )
    .unwrap();
    let json = serde_json::to_string(&ir).unwrap();
    assert!(json.contains("Before true after"), "{json}");
}

#[test]
fn quoted_delimiters_in_text_expressions_survive_all_shared_scanner_paths() {
    for (filename, source) in [
        ("Text.crepus", "span\n  \"Brace {'}'} after {'{'}\""),
        ("Inline.crepus", "span \"Brace {'}'} after {'{'}\""),
        ("Text.jsx", "<p>Brace {'}'} after {'{'} and {'<'}</p>"),
        ("Text.svelte", "<p>Brace {'}'} after {'{'} and {'<'}</p>"),
        ("Text.astro", "<p>Brace {'}'} after {'{'} and {'<'}</p>"),
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        let json = serde_json::to_string(&ir).unwrap();
        assert!(json.contains("Brace } after {"), "{filename}: {json}");
        if !filename.ends_with(".crepus") {
            assert!(json.contains("and <"), "{filename}: {json}");
        }
    }
}

#[test]
fn dynamic_ids_survive_native_view_ir_generation() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "entry&anchor");
    for (filename, source) in [
        ("Ids.jsx", "<div id={name} />"),
        ("Ids.svelte", "<div id={name} />"),
        ("Ids.astro", "<div id={name} />"),
        ("Ids.vue", r#"<template><div :id="name" /></template>"#),
        ("Ids.ng.html", r#"<div [id]="name" />"#),
    ] {
        let ir = render_template_to_ir_with_path(source, &ctx, Some(Path::new(filename))).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(&crepuscularity_native::to_json(&ir).unwrap()).unwrap();
        assert!(
            json.to_string().contains("\"id\":\"entry&anchor\""),
            "{filename}: {json}"
        );
    }
}

#[test]
fn invalid_dynamic_id_expressions_fail_native_rendering() {
    let ctx = TemplateContext::new();
    for filename in ["Invalid.jsx", "Invalid.svelte", "Invalid.astro"] {
        let error =
            render_template_to_ir_with_path("<div id={1 +} />", &ctx, Some(Path::new(filename)))
                .unwrap_err();
        assert!(
            error.to_string().contains("expression"),
            "{filename}: {error}"
        );
    }
    let recovered = render_template_to_ir_with_path(
        r#"<div id="after" />"#,
        &ctx,
        Some(Path::new("Valid.jsx")),
    )
    .unwrap();
    let json = crepuscularity_native::to_json(&recovered).unwrap();
    assert!(json.contains("\"id\":\"after\""), "{json}");
}
