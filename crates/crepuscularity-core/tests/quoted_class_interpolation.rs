use crepuscularity_core::{ast::Node, parse_template, TemplateContext};

fn classes(source: &str) -> Vec<String> {
    let nodes = parse_template(source).unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!("expected element");
    };
    element.classes.clone()
}

#[test]
fn quoted_class_expressions_keep_internal_whitespace_and_separate_static_tokens() {
    for source in ["div class='w-{1 + 2} flex'", "div class=\"w-{1 + 2} flex\""] {
        let tokens = classes(source);
        assert_eq!(tokens, ["w-{1 + 2}", "flex"]);
        let resolved: Vec<_> = tokens
            .iter()
            .map(|token| TemplateContext::new().interpolate(token).unwrap())
            .collect();
        assert_eq!(resolved, ["w-3", "flex"]);
    }
}

#[test]
fn unicode_whitespace_separates_classes_only_outside_interpolations() {
    let tokens = classes("div class='flex\u{2003}w-{1 + 2}\u{00a0}items-center'");
    assert_eq!(tokens, ["flex", "w-{1 + 2}", "items-center"]);
    assert_eq!(
        TemplateContext::new().interpolate(&tokens[1]).unwrap(),
        "w-3"
    );
}

#[test]
fn quoted_braces_and_nested_delimiters_do_not_split_expression_tokens() {
    let tokens = classes("div class=\"token-{'a } b'} flex\"");
    assert_eq!(tokens, ["token-{'a } b'}", "flex"]);
    assert_eq!(
        TemplateContext::new().interpolate(&tokens[0]).unwrap(),
        "token-a } b"
    );
    // Curly nesting is retained as one token even when the evaluator rejects
    // that expression form; class splitting must not invent extra classes.
    let tokens = classes("div class='w-{{1 + 2}} flex'");
    assert_eq!(tokens, ["w-{{1 + 2}}", "flex"]);
    assert!(TemplateContext::new().interpolate(&tokens[0]).is_err());
}

#[test]
fn malformed_interpolations_reach_the_original_interpolation_error() {
    for (source, token) in [
        ("div class='flex w-{1 + 2'", "w-{1 + 2"),
        ("div class='flex w-{ } items-center'", "w-{ }"),
        ("div class='flex w-{1 +} items-center'", "w-{1 +}"),
    ] {
        let tokens = classes(source);
        assert_eq!(tokens[0], "flex");
        assert_eq!(tokens[1], token);
        let expected = TemplateContext::new()
            .interpolate(token)
            .unwrap_err()
            .to_string();
        let actual = TemplateContext::new()
            .interpolate(&tokens[1])
            .unwrap_err()
            .to_string();
        assert_eq!(actual, expected);
    }
}

#[test]
fn ordinary_static_class_lists_keep_their_existing_ast() {
    for source in [
        "div class='flex  items-center\ttext-red-500'",
        "div class=\"flex  items-center\ttext-red-500\"",
    ] {
        assert_eq!(classes(source), ["flex", "items-center", "text-red-500"]);
    }
}

#[test]
fn quoted_when_classes_share_interpolation_aware_splitting() {
    let nodes = parse_template("div when:{enabled}=\"w-{1 + 2} flex\"").unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!("expected element");
    };
    assert_eq!(element.conditional_classes.len(), 2);
    assert_eq!(element.conditional_classes[0].class, "w-{1 + 2}");
    assert_eq!(element.conditional_classes[1].class, "flex");
    let mut ctx = TemplateContext::new();
    ctx.set("enabled", true);
    for class in &element.conditional_classes {
        assert!(ctx.eval_condition(&class.condition).unwrap());
    }
    assert_eq!(
        ctx.interpolate(&element.conditional_classes[0].class)
            .unwrap(),
        "w-3"
    );
    assert_eq!(
        classes("div class='before-{1 + 2}-after flex'"),
        ["before-{1 + 2}-after", "flex"]
    );
}
