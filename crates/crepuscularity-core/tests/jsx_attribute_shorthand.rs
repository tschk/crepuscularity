use std::path::Path;

use crepuscularity_core::{ast::Node, parse_template_with_path};

#[test]
fn jsx_brace_key_shorthand_is_an_explicit_error() {
    for filename in ["view.jsx", "view.tsx", "view.csx"] {
        for source in [
            "<div {value}/>",
            "<div { value }/>",
            "<input {value}/>",
            "<include src=\"child.crepus\" {value}/>",
            "<island src=\"widget.ts\" {value}/>",
            "<crepus-island src=\"widget.ts\" {value}/>",
        ] {
            let error = parse_template_with_path(source, Some(Path::new(filename)))
                .expect_err("JSX does not support Svelte attribute shorthand")
                .to_string();
            assert!(error.contains("shorthand"), "{filename}: {source}: {error}");
        }
    }
}

#[test]
fn named_jsx_attributes_and_existing_spread_boundary_remain_unchanged() {
    let nodes = parse_template_with_path("<div title={value}/>", Some(Path::new("view.jsx")))
        .expect("named JSX expression");
    let Node::Element(element) = &nodes[0] else {
        panic!("expected element");
    };
    assert_eq!(element.bindings[0].prop, "title");
    assert_eq!(element.bindings[0].value, "value");
    let error = parse_template_with_path("<div {...props}/>", Some(Path::new("view.jsx")))
        .expect_err("shared JSX spread remains unsupported")
        .to_string();
    assert!(
        error.contains("spread attributes are not supported"),
        "{error}"
    );
}

#[test]
fn svelte_and_astro_identifier_attribute_shorthand_remains_supported() {
    for (filename, source) in [
        ("view.svelte", "<input {value}/>"),
        ("view.svelte", "<input { value }/>"),
        ("view.astro", "<input {value}/>"),
        ("view.astro", "<input { value }/>"),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename)))
            .expect("Svelte/Astro attribute shorthand");
        let Node::Element(element) = &nodes[0] else {
            panic!("expected element");
        };
        assert_eq!(element.bindings[0].prop, "value");
        assert_eq!(element.bindings[0].value, "value");
    }
}

#[test]
fn alternate_control_branch_parsers_reject_brace_key_shorthand() {
    for filename in ["view.jsx", "view.tsx", "view.csx"] {
        for source in [
            "<if test={false}><else-if test={true} {value}><p/></else-if></if>",
            "<match on={1}><case pattern=\"_\" {value}><p/></case></match>",
            "<match on={1}><case pattern=\"_\" {value}/></match>",
            "<if test={false}><else {value}/></if>",
        ] {
            assert!(
                parse_template_with_path(source, Some(Path::new(filename))).is_err(),
                "accepted {filename}: {source}"
            );
        }
    }
}

#[test]
fn named_control_branches_keep_their_ast_and_spreads_remain_unsupported() {
    let nodes = parse_template_with_path(
        "<if test={false}><else-if test={true}><p/></else-if></if>",
        Some(Path::new("view.jsx")),
    )
    .expect("named else-if condition");
    let Node::If(first) = &nodes[0] else {
        panic!("expected if");
    };
    let Some([Node::If(second)]) = first.else_children.as_deref() else {
        panic!("expected else-if");
    };
    assert_eq!(second.condition, "true");
    assert!(matches!(&second.then_children[..], [Node::Element(element)] if element.tag == "p"));

    let nodes = parse_template_with_path(
        "<match on={1}><case pattern=\"_\"><p/></case></match>",
        Some(Path::new("view.jsx")),
    )
    .expect("named case pattern");
    let Node::Match(block) = &nodes[0] else {
        panic!("expected match");
    };
    assert_eq!(block.expr, "1");
    assert_eq!(block.arms[0].pattern, "_");
    assert!(matches!(&block.arms[0].body[..], [Node::Element(element)] if element.tag == "p"));

    for source in [
        "<if test={false}><else-if test={true} {...props}><p/></else-if></if>",
        "<match on={1}><case pattern=\"_\" {...props}/></match>",
        "<if test={false}><else {...props}/></if>",
    ] {
        let error = parse_template_with_path(source, Some(Path::new("view.jsx")))
            .expect_err("control branch spreads remain unsupported")
            .to_string();
        assert!(
            error.contains("spread attributes are not supported"),
            "{error}"
        );
    }
}
