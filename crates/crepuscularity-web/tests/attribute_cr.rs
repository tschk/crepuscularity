use crepuscularity_core::{
    ast::{EmbedNode, Node},
    TemplateContext,
};
use crepuscularity_web::{render_nodes_to_html, render_template_to_html};

fn variants(source: &str) -> Vec<String> {
    let ctx = TemplateContext::new();
    #[allow(unused_mut)]
    let mut out = vec![render_template_to_html(source, &ctx).unwrap()];
    #[cfg(feature = "ssr")]
    for markers in [false, true] {
        out.push(
            crepuscularity_web::render_template_to_html_with_ssr(source, &ctx, markers).unwrap(),
        );
    }
    #[cfg(feature = "hydration")]
    out.push(crepuscularity_web::render_template_to_html_with_hydration(source, &ctx).unwrap());
    out
}

#[test]
fn ids_and_handler_attributes_preserve_cr_and_crlf_across_html_modes() {
    for newline in ["\r", "\r\n"] {
        let encoded = newline.replace('\r', "&#13;");
        let source = format!(
            "<button id=\"before{newline}after\" @click=\"save{newline}now\">Text</button>"
        );
        let mut rendered = variants(&source);
        let mut nodes =
            crepuscularity_core::parse_template("slot-rotate @click=save\n  \"One\"\n  \"Two\"")
                .unwrap();
        let Node::Element(element) = &mut nodes[0] else {
            panic!("expected rotating text element")
        };
        element.id = Some(format!("before{newline}after"));
        element.event_handlers[0].handler = format!("save{newline}now");
        let ctx = TemplateContext::new();
        rendered.push(render_nodes_to_html(&nodes, &ctx).unwrap());
        #[cfg(feature = "ssr")]
        for markers in [false, true] {
            rendered.push(
                crepuscularity_web::render_nodes_ssr(
                    &nodes,
                    &ctx,
                    &std::cell::Cell::new(0),
                    &mut Default::default(),
                    markers,
                )
                .unwrap(),
            );
        }
        for html in rendered {
            assert!(
                html.contains(&format!("id=\"before{encoded}after\"")),
                "{html:?}"
            );
            assert!(
                html.contains(&format!("data-onclick=\"save{encoded}now\"")),
                "{html:?}"
            );
        }
    }
}

#[test]
fn supplied_embed_source_and_scope_preserve_carriage_returns() {
    let nodes = [Node::Embed(EmbedNode {
        src: "module\r\nsource.js".into(),
        adapter: Some("rust".into()),
        props: vec![],
    })];
    let ctx = TemplateContext::new();
    #[allow(unused_mut)]
    let mut out = vec![render_nodes_to_html(&nodes, &ctx).unwrap()];
    #[cfg(feature = "ssr")]
    out.push(
        crepuscularity_web::render_nodes_ssr(
            &nodes,
            &ctx,
            &std::cell::Cell::new(0),
            &mut Default::default(),
            true,
        )
        .unwrap(),
    );
    for html in out {
        assert!(
            html.contains("data-crepus-island-src=\"module&#13;\nsource.js\""),
            "{html:?}"
        );
        assert!(
            html.contains("data-crepus-scope=\"module&#13;\nsource.js\""),
            "{html:?}"
        );
    }
}

#[cfg(feature = "ssr")]
#[test]
fn document_attribute_escaping_is_separate_from_text_escaping() {
    let doc = crepuscularity_web::SsrDocument {
        lang: "en\r\ncustom",
        body_class: Some("before\rafter"),
        title: "Title\rtext",
        ..Default::default()
    };
    let html = crepuscularity_web::wrap_ssr_document("<p>Body</p>", &doc);
    assert!(html.contains("lang=\"en&#13;\ncustom\""), "{html:?}");
    assert!(html.contains("class=\"before&#13;after\""), "{html:?}");
    assert!(
        html.contains("<title>Title\rtext</title>"),
        "text escaping must stay unchanged: {html:?}"
    );
}

#[test]
fn ordinary_html_text_escaping_does_not_encode_carriage_returns() {
    assert_eq!(
        crepuscularity_web::escape_html("text\r\n<&"),
        "text\r\n&lt;&amp;"
    );
}
