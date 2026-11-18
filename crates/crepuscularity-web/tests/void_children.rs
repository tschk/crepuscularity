use crepuscularity_core::{ast::Node, parse_template, TemplateContext};
use crepuscularity_web::{render_nodes_to_html, render_template_to_html};

fn rejects(source: &str) {
    let ctx = TemplateContext::new();
    let check = |error: crepuscularity_core::CrepusError| {
        assert!(
            error.to_string().contains("void element"),
            "{source}: {error}"
        );
    };
    check(render_template_to_html(source, &ctx).unwrap_err());
    #[cfg(feature = "ssr")]
    for markers in [false, true] {
        check(
            crepuscularity_web::render_template_to_html_with_ssr(source, &ctx, markers)
                .unwrap_err(),
        );
    }
    #[cfg(feature = "hydration")]
    check(crepuscularity_web::render_template_to_html_with_hydration(source, &ctx).unwrap_err());
}

#[test]
fn void_children_are_rejected_from_indent_and_jsx_in_all_render_modes() {
    for tag in ["img", "input", "br", "IMG", "Br"] {
        rejects(&format!("{tag}\n  span \"LOST\""));
        rejects(&format!("<{tag}><span>LOST</span></{tag}>"));
    }
    for child in [
        "$: let value = 1 +",
        "{1 +}",
        "if false\n    span \"inactive\"",
    ] {
        rejects(&format!("img\n  {child}"));
    }
}

#[test]
fn direct_ast_children_reject_before_evaluating_expressions() {
    let mut nodes = parse_template("img").unwrap();
    let Node::Element(element) = &mut nodes[0] else {
        panic!("expected element")
    };
    element.children.push(Node::RawText("1 +".into()));
    let error = render_nodes_to_html(&nodes, &TemplateContext::new()).unwrap_err();
    assert!(error.to_string().contains("void element"), "{error}");
    #[cfg(feature = "ssr")]
    {
        let error = crepuscularity_web::render_nodes_ssr(
            &nodes,
            &TemplateContext::new(),
            &std::cell::Cell::new(0),
            &mut Default::default(),
            true,
        )
        .unwrap_err();
        assert!(error.to_string().contains("void element"), "{error}");
    }
}

#[test]
fn empty_voids_nonvoids_and_inactive_branches_remain_supported() {
    for source in [
        "img",
        "<img/>",
        "<img></img>",
        "<IMG></IMG>",
        "<div>kept</div>",
        "if false\n  img\n    span \"inactive\"\nelse\n  span \"safe\"",
    ] {
        let ctx = TemplateContext::new();
        assert!(render_template_to_html(source, &ctx).is_ok(), "{source}");
        #[cfg(feature = "ssr")]
        for markers in [false, true] {
            assert!(
                crepuscularity_web::render_template_to_html_with_ssr(source, &ctx, markers).is_ok(),
                "{source}"
            );
        }
        #[cfg(feature = "hydration")]
        assert!(
            crepuscularity_web::render_template_to_html_with_hydration(source, &ctx).is_ok(),
            "{source}"
        );
    }
    assert_eq!(
        render_template_to_html("<IMG></IMG>", &TemplateContext::new()).unwrap(),
        "<IMG />"
    );
}
