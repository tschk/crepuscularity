use crepuscularity_core::{TemplateContext, TemplateValue};

#[test]
fn loop_bindings_preserve_scalar_types_and_object_scopes() {
    for (value, expression, expected) in [
        (Some(TemplateValue::Int(2)), "item + 1", "3"),
        (Some(TemplateValue::Bool(false)), "item == false", "true"),
        (
            Some(TemplateValue::Str(String::new())),
            "item == ''",
            "true",
        ),
        (Some(TemplateValue::Null), "item == null", "true"),
        (None, "item.name", "雪"),
    ] {
        let mut item = TemplateContext::new();
        item.set("name", "雪");
        if let Some(value) = value {
            item.set("value", value);
        }
        let mut ctx = TemplateContext::new();
        ctx.set("item", "parent value must not leak");
        ctx.set("items", TemplateValue::List(vec![item]));
        let source = format!("for item in items\n  span \"{{{expression}}}\"");
        let xml = crepuscularity_lvgl::render_template_to_lvgl_xml(&source, &ctx).unwrap();
        assert!(
            xml.contains(&format!("text=\"{expected}\"")),
            "{expression}: {xml}"
        );
    }
}

#[test]
fn loop_iterator_expressions_resolve_nested_lists() {
    let mut item = TemplateContext::new();
    item.set("value", 7_i64);
    let mut data = TemplateContext::new();
    data.set("items", TemplateValue::List(vec![item]));
    let mut ctx = TemplateContext::new();
    ctx.set("data", TemplateValue::Scope(data));
    let source = "for item in data.items\n  span \"Value {item + 1}\"";
    let xml = crepuscularity_lvgl::render_template_to_lvgl_xml(source, &ctx).unwrap();
    assert!(xml.contains("text=\"Value 8\""), "{xml}");
    assert!(
        crepuscularity_lvgl::render_template_to_lvgl_xml("for item in {1 +}\n  span", &ctx)
            .is_err()
    );
}

fn assert_label_xml(xml: &str, escaped_text: &[&str]) {
    let labels: String = escaped_text
        .iter()
        .map(|text| format!("    <lv_label text=\"{text}\"/>\n"))
        .collect();
    assert_eq!(
        xml,
        format!("<component name=\"CrepusView\">\n  <view>\n{labels}  </view>\n</component>\n")
    );
}

#[test]
fn loop_scope_restores_missing_fields_and_duplicate_alias_writes() {
    let mut first = TemplateContext::new();
    first
        .set("title", "first<&")
        .set("temporary", "first-only")
        .set("items", "field\"");
    let mut second = TemplateContext::new();
    second.set("id", 2);
    let mut ctx = TemplateContext::new();
    ctx.set("title", "parent")
        .set("items", vec![TemplateContext::new(); 1024])
        .set("rows", vec![first, second])
        .set("crepus_target", "wrong")
        .set("is_lvgl", false)
        .set("is_web", true);
    let source = "for items in rows\n  span \"{title}|{temporary}|{items.title}|{items.items}\"\np \"{title}|{temporary}|{items.length}|{is_lvgl}|{is_web}|{crepus_target}\"";
    let xml = crepuscularity_lvgl::render_template_to_lvgl_xml(source, &ctx).unwrap();
    assert_label_xml(
        &xml,
        &[
            "first&lt;&amp;|first-only|first&lt;&amp;|field&quot;",
            "parent|||",
            "parent||1024|true|false|lvgl",
        ],
    );
    assert_eq!(ctx.get_list_ref("items").len(), 1024);
    assert_eq!(ctx.get_str("title"), "parent");
    assert_eq!(ctx.get_str("crepus_target"), "wrong");
    assert!(!ctx.get_bool("is_lvgl") && ctx.get_bool("is_web"));
    assert!(ctx.get("temporary").is_none());
}

#[test]
fn nested_loop_aliases_and_declarations_stay_local() {
    let mut child = TemplateContext::new();
    child.set("title", "inner");
    let mut first = TemplateContext::new();
    first
        .set("title", "outer1")
        .set("children", vec![child, TemplateContext::new()]);
    let mut second = TemplateContext::new();
    second.set("title", "outer2");
    let mut ctx = TemplateContext::new();
    ctx.set("title", "root")
        .set("row", "parent-row")
        .set("rows", vec![first, second]);
    let source = "for row in rows\n  p \"{title}|{row.title}\"\n  for row in row.children\n    $: let title = \"local\"\n    span \"{title}|{row.title}\"\n  p \"{title}|{row.title}\"\np \"{title}|{row.title}\"";
    let xml = crepuscularity_lvgl::render_template_to_lvgl_xml(source, &ctx).unwrap();
    assert_label_xml(
        &xml,
        &[
            "outer1|outer1",
            "local|inner",
            "local|",
            "outer1|outer1",
            "outer2|outer2",
            "outer2|outer2",
            "root|",
        ],
    );
    assert_eq!(ctx.get_str("title"), "root");
    assert_eq!(ctx.get_str("row"), "parent-row");
    assert!(ctx.get("children").is_none());
}

#[test]
fn default_declarations_and_empty_loop_alias_preserve_parent_scope() {
    use crepuscularity_core::ast::{ForBlock, Node};
    let mut first = TemplateContext::new();
    first.set("title", "first").set("temporary", "first-only");
    let mut ctx = TemplateContext::new();
    ctx.set("title", "parent")
        .set("rows", vec![first, TemplateContext::new()]);
    let mut nodes = vec![Node::For(ForBlock {
        pattern: String::new(),
        iterator: "rows".into(),
        body: crepuscularity_core::parse_template(
            "$: default title = \"fallback\"\nspan \"{title}|{temporary}\"",
        )
        .unwrap(),
    })];
    nodes.extend(crepuscularity_core::parse_template("p \"{title}|{temporary}\"").unwrap());
    let xml =
        crepuscularity_lvgl::render_nodes_to_lvgl_xml(&nodes, &ctx, &Default::default()).unwrap();
    assert_label_xml(&xml, &["first|first-only", "parent|", "parent|"]);
    assert!(!ctx.vars.contains_key(""));
}

#[test]
fn loop_body_errors_propagate_without_changing_the_parent() {
    let mut row = TemplateContext::new();
    row.set("title", "row");
    let mut ctx = TemplateContext::new();
    ctx.set("title", "parent").set("rows", vec![row]);
    let error = crepuscularity_lvgl::render_template_to_lvgl_xml(
        "for row in rows\n  $: let title = \"local\"\n  embed widget.js adapter=\"react\"",
        &ctx,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("embed island 'widget.js'"),
        "{error}"
    );
    assert_eq!(ctx.get_str("title"), "parent");
    assert!(ctx.get("row").is_none());
}
