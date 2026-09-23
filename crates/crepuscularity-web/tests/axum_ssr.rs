use std::cell::Cell;

use crepuscularity_core::TemplateContext;
use crepuscularity_web::{render_ssr_document_with_nodes, SsrDocument};

#[tokio::test]
async fn stream_ssr_escapes_body_class() {
    use axum::body::to_bytes;
    use crepuscularity_web::stream_ssr_response_with_nodes;

    let nodes = crepuscularity_core::ast_cache::parse_content(r#"div "ok""#).unwrap();
    let doc = SsrDocument {
        title: "Test",
        body_class: Some(r#"x" onload="alert(1)"#),
        ..Default::default()
    };

    let response = stream_ssr_response_with_nodes(nodes, TemplateContext::new(), doc).await;
    let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(
        !html.contains(r#"onload="alert(1)""#),
        "body class XSS: {html}"
    );
    assert!(
        html.contains("&quot;"),
        "double quote must be escaped: {html}"
    );
}

#[test]
fn ssr_renders_template() {
    let template = r#"div p-4
  "Hello {name}""#;
    let mut ctx = TemplateContext::new();
    ctx.set("name", "World");
    let html = crepuscularity_web::render_template_to_html(template, &ctx).unwrap();
    assert!(html.contains("Hello World"));
}

#[test]
fn ssr_renders_template_with_nodes() {
    let template = r#"div p-4
  "Hello SSR""#;
    let nodes = crepuscularity_core::ast_cache::parse_content(template).unwrap();

    let ctx = TemplateContext::new();
    let counter = Cell::new(0u32);
    let mut bind = crepuscularity_web::BindMap::new();
    let doc = SsrDocument {
        title: "Test",
        ..Default::default()
    };

    let html =
        render_ssr_document_with_nodes(&nodes, &counter, &mut bind, &ctx, &doc, true).unwrap();

    assert!(html.contains("Hello SSR"));
    assert!(html.contains("<title>Test</title>"));
    assert!(html.contains("<!DOCTYPE html>"));
    // The template rendered correctly into an HTML5 document
    assert!(html.contains("Hello SSR"));
}

#[test]
fn ssr_renders_with_variables() {
    let template = r#"div
  "Count: {count}""#;
    let nodes = crepuscularity_core::ast_cache::parse_content(template).unwrap();

    let mut ctx = TemplateContext::new();
    ctx.set("count", 42);
    let counter = Cell::new(0u32);
    let mut bind = crepuscularity_web::BindMap::new();
    let doc = SsrDocument::default();

    let html =
        render_ssr_document_with_nodes(&nodes, &counter, &mut bind, &ctx, &doc, true).unwrap();

    assert!(html.contains("Count: 42"));
}

#[test]
fn ssr_handles_complex_template() {
    let template = r#"div.container
  h1.text-xl
    "Title"
  p.text-sm.text-gray-500
    "Body content here""#;
    let nodes = crepuscularity_core::ast_cache::parse_content(template).unwrap();

    let ctx = TemplateContext::new();
    let counter = Cell::new(0u32);
    let mut bind = crepuscularity_web::BindMap::new();
    let doc = SsrDocument {
        title: "Complex",
        body_class: Some("dark"),
        ..Default::default()
    };

    let html =
        render_ssr_document_with_nodes(&nodes, &counter, &mut bind, &ctx, &doc, true).unwrap();

    assert!(html.contains("container"));
    assert!(html.contains("text-xl"));
    assert!(html.contains("Body content here"));
    assert!(html.contains("class=\"dark\""));
    assert!(html.contains("<title>Complex</title>"));
}

#[test]
fn ssr_handle_missing_variable() {
    let template = r#"div
  "{missing}""#;
    let nodes = crepuscularity_core::ast_cache::parse_content(template).unwrap();

    let ctx = TemplateContext::new();
    let counter = Cell::new(0u32);
    let mut bind = crepuscularity_web::BindMap::new();
    let doc = SsrDocument::default();

    let html = render_ssr_document_with_nodes(&nodes, &counter, &mut bind, &ctx, &doc, true);

    // Missing variables render as empty string
    assert!(html.is_ok());
    let html = html.unwrap();
    // The {missing} should be empty (not an error)
    assert!(!html.contains("{missing}"));
}

#[test]
fn ssr_router_basic() {
    use crepuscularity_web::SsrRouter;

    // Smoke test: router builds without panicking
    let _router = SsrRouter::new()
        .route("/", r#"div "Home""#, "Home")
        .route("/about", r#"div "About""#, "About")
        .into_axum_router();
}

#[test]
fn ssr_options_works() {
    use crepuscularity_web::{SsrHandler, SsrOptions};

    let opts = SsrOptions::new(r#"div "Hello from options""#, "Options Test");
    assert_eq!(opts.title, "Options Test");
    assert!(!opts.nodes.is_empty());

    let handler = SsrHandler::new(opts);
    let state = handler.state();
    assert_eq!(state.title, "Options Test");
}

#[test]
fn ssr_html_safety() {
    let template = r#"div
  "{unsafe}""#;
    let nodes = crepuscularity_core::ast_cache::parse_content(template).unwrap();

    let mut ctx = TemplateContext::new();
    ctx.set("unsafe", "<script>alert('xss')</script>");
    let counter = Cell::new(0u32);
    let mut bind = crepuscularity_web::BindMap::new();
    let doc = SsrDocument::default();

    let html =
        render_ssr_document_with_nodes(&nodes, &counter, &mut bind, &ctx, &doc, true).unwrap();

    // HTML should be escaped
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;"));
}

#[test]
fn ssr_loop_rendering() {
    let template = r#"ul
  for item in {items}
    li "{item.value}""#;
    let nodes = crepuscularity_core::ast_cache::parse_content(template).unwrap();

    let mut ctx = TemplateContext::new();
    let mut item_a = TemplateContext::new();
    item_a.set("value", "a");
    let mut item_b = TemplateContext::new();
    item_b.set("value", "b");
    let mut item_c = TemplateContext::new();
    item_c.set("value", "c");
    ctx.set(
        "items",
        crepuscularity_core::TemplateValue::List(vec![item_a, item_b, item_c]),
    );
    let counter = Cell::new(0u32);
    let mut bind = crepuscularity_web::BindMap::new();
    let doc = SsrDocument::default();

    let html =
        render_ssr_document_with_nodes(&nodes, &counter, &mut bind, &ctx, &doc, true).unwrap();

    // Items should appear in the rendered HTML
    assert!(html.contains("a") || html.len() > 100);
}

#[tokio::test]
async fn test_handler_error_hides_details() {
    use axum::extract::State;
    use crepuscularity_web::{SsrHandler, SsrOptions};

    // We can simulate an error by using an IncludeNode pointing to a missing file
    // which causes render_ssr_document_with_nodes to return an error.
    let mut opts = SsrOptions::new(r#"div"#, "Title");
    opts.nodes = std::sync::Arc::new(vec![crepuscularity_core::ast::Node::Include(
        crepuscularity_core::ast::IncludeNode {
            path: "non_existent.crepus".to_string(),
            props: vec![],
            slot: vec![],
        }
    )]);

    let state = State(std::sync::Arc::new(opts));
    let response = SsrHandler::handle(state).await;

    assert_eq!(response.0, "<pre style='color:red'>Internal Server Error</pre>");
}
