use crepuscularity_core::TemplateContext;
use crepuscularity_web::render_template_to_html;

#[test]
fn for_loop_merges_list_item_fields() {
    let tpl = r#"
section
  for item in {items}
    p
      {label}
"#;
    let mut row = TemplateContext::new();
    row.set("label", "hello");
    let mut ctx = TemplateContext::new();
    ctx.set("items", vec![row]);
    let out = render_template_to_html(tpl, &ctx).unwrap();
    assert!(out.contains("hello"), "got: {out}");
}

#[test]
fn for_loop_sees_parent_keys_overwritten_by_item() {
    let tpl = r#"
div
  for item in {items}
    span
      {title}
"#;
    let mut row = TemplateContext::new();
    row.set("title", "row-title");
    let mut ctx = TemplateContext::new();
    ctx.set("title", "outer-title");
    ctx.set("items", vec![row]);
    let out = render_template_to_html(tpl, &ctx).unwrap();
    assert!(out.contains("row-title"), "got: {out}");
    assert!(!out.contains("outer-title"));
}

fn assert_plain_and_ssr(
    nodes: &[crepuscularity_core::ast::Node],
    ctx: &TemplateContext,
    expected: &str,
) {
    assert_eq!(
        crepuscularity_web::render_nodes_to_html(nodes, ctx).unwrap(),
        expected
    );
    #[cfg(feature = "ssr")]
    {
        let mut bind = serde_json::Map::new();
        let marked = crepuscularity_web::render_nodes_ssr(
            nodes,
            ctx,
            &std::cell::Cell::new(0),
            &mut bind,
            true,
        )
        .unwrap();
        // These fixtures have no scripts or escaped '<'; compare text through SSR spans.
        fn text(html: &str) -> String {
            let mut tag = false;
            html.chars()
                .filter(|ch| match ch {
                    '<' => {
                        tag = true;
                        false
                    }
                    '>' => {
                        tag = false;
                        false
                    }
                    _ => !tag,
                })
                .collect()
        }
        assert_eq!(text(&marked), text(expected), "{marked}");
        assert!(
            !bind.is_empty(),
            "loop hydration bindings were lost: {marked}"
        );
    }
}

#[test]
fn loop_restores_missing_fields_and_duplicate_alias_writes_over_a_large_parent_list() {
    let mut first = TemplateContext::new();
    first
        .set("title", "first")
        .set("temporary", "first-only")
        .set("items", "field");
    let mut second = TemplateContext::new();
    second.set("id", 2);
    let mut ctx = TemplateContext::new();
    ctx.set("title", "parent");
    ctx.set("items", vec![TemplateContext::new(); 2048]);
    ctx.set("rows", vec![first, second]);
    let nodes = crepuscularity_core::parse_template(
        "for items in rows\n  span \"{title}|{temporary}|{items.title}|{items.items}\"\np \"{title}|{temporary}|{items.length}\"",
    ).unwrap();
    assert_plain_and_ssr(
        &nodes,
        &ctx,
        "<span>first|first-only|first|field</span><span>parent|||</span><p>parent||2048</p>",
    );
    assert_eq!(ctx.get_list_ref("items").len(), 2048);
    assert_eq!(ctx.get_str("title"), "parent");
    assert!(ctx.get("temporary").is_none());
}

#[test]
fn nested_loop_aliases_and_declarations_do_not_escape_their_scopes() {
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
    let nodes = crepuscularity_core::parse_template(
        "for row in rows\n  p \"{title}|{row.title}\"\n  for row in row.children\n    $: let title = \"local\"\n    span \"{title}|{row.title}\"\n  p \"{title}|{row.title}\"\np \"{title}|{row.title}\"",
    ).unwrap();
    assert_plain_and_ssr(&nodes, &ctx,
        "<p>outer1|outer1</p><span>local|inner</span><span>local|</span><p>outer1|outer1</p><p>outer2|outer2</p><p>outer2|outer2</p><p>root|</p>");
    assert_eq!(ctx.get_str("title"), "root");
    assert_eq!(ctx.get_str("row"), "parent-row");
    assert!(ctx.get("children").is_none());
}

#[test]
fn empty_loop_alias_still_merges_and_restores_row_fields() {
    use crepuscularity_core::ast::{ForBlock, Node};
    let mut first = TemplateContext::new();
    first.set("title", "first").set("temporary", "only-first");
    let mut ctx = TemplateContext::new();
    ctx.set("title", "parent")
        .set("rows", vec![first, TemplateContext::new()]);
    let mut nodes = vec![Node::For(ForBlock {
        pattern: String::new(),
        iterator: "rows".into(),
        body: crepuscularity_core::parse_template("span \"{title}|{temporary}\"").unwrap(),
    })];
    nodes.extend(crepuscularity_core::parse_template("p \"{title}|{temporary}\"").unwrap());
    assert_plain_and_ssr(
        &nodes,
        &ctx,
        "<span>first|only-first</span><span>parent|</span><p>parent|</p>",
    );
    assert!(!ctx.vars.contains_key(""));
}

#[test]
fn loop_body_errors_still_propagate_without_mutating_the_parent() {
    let mut row = TemplateContext::new();
    row.set("title", "row");
    let mut ctx = TemplateContext::new();
    ctx.set("title", "parent").set("rows", vec![row]);
    let dir = tempfile::tempdir().unwrap();
    ctx.base_dir = Some(dir.path().to_path_buf());
    let nodes =
        crepuscularity_core::parse_template("for row in rows\n  include missing.crepus").unwrap();
    assert!(crepuscularity_web::render_nodes_to_html(&nodes, &ctx).is_err());
    #[cfg(feature = "ssr")]
    assert!(crepuscularity_web::render_nodes_ssr(
        &nodes,
        &ctx,
        &std::cell::Cell::new(0),
        &mut serde_json::Map::new(),
        true,
    )
    .is_err());
    assert_eq!(ctx.get_str("title"), "parent");
    assert!(ctx.get("row").is_none());
}
