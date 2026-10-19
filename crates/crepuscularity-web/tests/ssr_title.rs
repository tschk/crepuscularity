use crepuscularity_core::TemplateContext;
use std::sync::Arc;

fn variants(source: &str, ctx: &TemplateContext) -> Vec<String> {
    #[allow(unused_mut)]
    let mut html = vec![crepuscularity_web::render_template_to_html(source, ctx).unwrap()];
    #[cfg(feature = "ssr")]
    for markers in [false, true] {
        html.push(
            crepuscularity_web::render_template_to_html_with_ssr(source, ctx, markers).unwrap(),
        );
    }
    #[cfg(feature = "hydration")]
    html.push(crepuscularity_web::render_template_to_html_with_hydration(source, ctx).unwrap());
    html
}

fn title_content(html: &str) -> &str {
    let (_, after) = html.split_once("<title").unwrap();
    after
        .split_once('>')
        .unwrap()
        .1
        .split_once("</title>")
        .unwrap()
        .0
}

#[test]
fn title_text_has_no_region_markup_and_preserves_rcdata_whitespace() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "\n雪🦀<>&\nend");
    for html in variants("<title>{name}</title>", &ctx) {
        assert_eq!(title_content(&html), "\n雪🦀&lt;&gt;&amp;\nend", "{html}");
    }
}

#[test]
fn title_include_control_and_forwarded_slots_use_caller_context() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "caller 雪<&>");
    Arc::make_mut(&mut ctx.virtual_files).insert(
        "heading.crepus".into(),
        "if true\n  slot\nelse\n  \"inactive\"".into(),
    );
    let source = "title\n  include heading.crepus\n    \"{name}\"\np \"{name}\"";
    for html in variants(source, &ctx) {
        assert_eq!(title_content(&html), "caller 雪&lt;&amp;&gt;", "{html}");
        assert!(html.contains("<p"), "{html}");
        #[cfg(feature = "ssr")]
        if html.contains("__crepus_hydration__") && html.contains("data-crepus-kind") {
            // Suppressing title child markers must not suppress later sibling markers.
            assert!(
                html.split_once("</title>")
                    .unwrap()
                    .1
                    .contains("data-crepus-kind=\"text\""),
                "{html}"
            );
        }
    }
}
