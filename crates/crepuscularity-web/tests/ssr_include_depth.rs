#![cfg(feature = "ssr")]

use std::sync::Arc;

use crepuscularity_core::TemplateContext;
use crepuscularity_web::render_template_to_html_with_ssr;

fn ctx_with(files: &[(&str, &str)]) -> TemplateContext {
    let mut ctx = TemplateContext::new();
    let map = Arc::make_mut(&mut ctx.virtual_files);
    for (k, v) in files {
        map.insert((*k).to_string(), (*v).to_string());
    }
    ctx
}

#[test]
fn ssr_self_referential_include_is_bounded() {
    let ctx = ctx_with(&[("a.crepus", "div\n  include a.crepus")]);
    let err = render_template_to_html_with_ssr("include a.crepus", &ctx, true)
        .expect_err("self-include must be rejected, not recursed forever");
    assert!(
        err.to_string().contains("maximum include depth"),
        "expected depth limit, got: {err}"
    );
}

#[test]
fn ssr_mutually_recursive_includes_are_bounded() {
    let ctx = ctx_with(&[
        ("a.crepus", "div\n  include b.crepus"),
        ("b.crepus", "div\n  include a.crepus"),
    ]);
    let err = render_template_to_html_with_ssr("include a.crepus", &ctx, true)
        .expect_err("include cycle must be rejected");
    assert!(
        err.to_string().contains("maximum include depth"),
        "expected depth limit, got: {err}"
    );
}

#[test]
fn ssr_shallow_includes_still_render() {
    let ctx = ctx_with(&[("leaf.crepus", "span\n  \"leaf\"")]);
    let html = render_template_to_html_with_ssr("include leaf.crepus", &ctx, true)
        .expect("shallow include renders");
    assert!(html.contains("leaf"), "got: {html}");
}

#[test]
fn ssr_sibling_includes_do_not_accumulate_depth() {
    let ctx = ctx_with(&[("leaf.crepus", "span\n  \"leaf\"")]);
    let mut tpl = String::new();
    for _ in 0..200 {
        tpl.push_str("include leaf.crepus\n");
    }
    let html = render_template_to_html_with_ssr(&tpl, &ctx, true).expect("siblings render");
    assert_eq!(html.matches("leaf</span>").count(), 200, "got: {html}");
}
