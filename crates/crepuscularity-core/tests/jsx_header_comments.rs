use crepuscularity_core::{parse_template, parse_template_with_path};
use std::path::Path;

#[test]
fn leading_hash_comments_do_not_become_jsx_text_nodes() {
    for (source, expected) in [
        (
            "# comment with unmatched { and '😀\n<div>OK</div>",
            "<div>OK</div>",
        ),
        (
            "# comment\n$: let title = 'OK'\n# another comment\n<div>{title}</div>",
            "$: let title = 'OK'\n<div>{title}</div>",
        ),
        ("  # comment\r\n\r\n<div/>", "<div/>"),
    ] {
        assert_eq!(
            parse_template(source).unwrap(),
            parse_template(expected).unwrap()
        );
        assert_eq!(
            parse_template_with_path(source, Some(Path::new("view.jsx"))).unwrap(),
            parse_template(expected).unwrap()
        );
    }
}

#[test]
fn hash_text_inside_elements_and_after_root_content_is_preserved() {
    for source in ["<div>#keep</div>", "<div/>#keep"] {
        let nodes = parse_template(source).unwrap();
        let text = match nodes.last().unwrap() {
            crepuscularity_core::ast::Node::Element(element) => &element.children[0],
            node => node,
        };
        assert_eq!(
            text,
            &crepuscularity_core::ast::Node::Text(vec![
                crepuscularity_core::ast::TextPart::Literal("#keep".into())
            ])
        );
    }
}

#[test]
fn comment_tags_do_not_trigger_depth_limits_and_error_offsets_stay_original() {
    let source = format!("{}<div/>", "# <div>\n".repeat(300));
    assert_eq!(
        parse_template(&source).unwrap(),
        parse_template("<div/>").unwrap()
    );
    let diagnostics = crepuscularity_core::diagnose_crepus_source("# 😀 comment\n<div></wrong>");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].start_line, 1);
}
