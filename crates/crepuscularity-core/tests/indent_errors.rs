use crepuscularity_core::parse_template;

#[test]
fn directive_literal_props_preserve_paths_quotes_and_unicode() {
    for directive in ["include", "embed"] {
        for (literal, expected) in [
            (r#""C:\new\tools""#, r"C:\new\tools"),
            (r#"'A "quote" 雪'"#, "A \"quote\" 雪"),
        ] {
            let nodes =
                parse_template(&format!("{directive} child.crepus title={literal}")).unwrap();
            let props = match &nodes[0] {
                crepuscularity_core::ast::Node::Include(node) => &node.props,
                crepuscularity_core::ast::Node::Embed(node) => &node.props,
                _ => panic!("expected directive"),
            };
            let value = crepuscularity_core::eval::eval_expr(
                &props[0].1,
                &crepuscularity_core::TemplateContext::new(),
            )
            .unwrap();
            assert!(
                matches!(value, crepuscularity_core::TemplateValue::Str(value) if value == expected)
            );
        }
    }
}

#[test]
fn quoted_directive_paths_preserve_spaces_and_drop_quotes() {
    for directive in ["include", "embed"] {
        let nodes =
            parse_template(&format!("{directive} \"ui/card view.crepus\" title=1")).unwrap();
        match &nodes[0] {
            crepuscularity_core::ast::Node::Include(node) => {
                assert_eq!(node.path, "ui/card view.crepus")
            }
            crepuscularity_core::ast::Node::Embed(node) => {
                assert_eq!(node.src, "ui/card view.crepus")
            }
            _ => panic!("expected directive"),
        }
        assert!(parse_template(&format!("{directive} \"unclosed")).is_err());
        assert!(parse_template(&format!("{directive} \"\"")).is_err());
    }
}

#[test]
fn embed_adapter_literal_decodes_the_encoded_expression() {
    let nodes = parse_template(r#"embed child adapter="custom\adapter""#).unwrap();
    let crepuscularity_core::ast::Node::Embed(embed) = &nodes[0] else {
        panic!("expected embed")
    };
    assert_eq!(embed.adapter.as_deref(), Some(r"custom\adapter"));
}

#[test]
fn braced_directive_props_require_an_expression() {
    for directive in ["include", "embed"] {
        for value in ["{}", "{   }", "{\t}"] {
            let error =
                parse_template(&format!("{directive} child.crepus title={value}")).unwrap_err();
            assert!(error
                .to_string()
                .contains("empty directive prop expression"));
        }
        assert!(parse_template(&format!("{directive} child.crepus title=\"\"")).is_ok());
    }
}

#[test]
fn duplicate_directive_props_are_errors() {
    for source in [
        "include child.crepus title=1 title=2",
        "embed child.crepus title=1 title=2",
        "embed child.crepus adapter=\"react\" adapter=\"svelte\"",
    ] {
        let error = parse_template(source).unwrap_err();
        assert!(error.to_string().contains("duplicate directive prop"));
    }
}

#[test]
fn directive_prop_names_follow_attribute_name_rules() {
    for directive in ["include", "embed"] {
        for name in ["bad!", "bad\"", "bad<", "bad/", "bad:"] {
            let error = parse_template(&format!("{directive} child.crepus {name}=1")).unwrap_err();
            assert!(error.to_string().contains("invalid directive prop name"));
        }
        assert!(parse_template(&format!(
            "{directive} child.crepus data-id=1 _value=2 title=3"
        ))
        .is_ok());
    }
}

#[test]
fn directives_accept_tab_separators_before_source_and_props() {
    for directive in ["include", "embed"] {
        let nodes = parse_template(&format!("{directive}\tchild.crepus\ttitle=\"Good\"")).unwrap();
        match &nodes[0] {
            crepuscularity_core::ast::Node::Include(node) => {
                assert_eq!(node.path, "child.crepus");
                assert_eq!(node.props.len(), 1);
            }
            crepuscularity_core::ast::Node::Embed(node) => {
                assert_eq!(node.src, "child.crepus");
                assert_eq!(node.props.len(), 1);
            }
            _ => panic!("expected directive"),
        }
    }
}

#[test]
fn directive_props_require_whitespace_separators() {
    for directive in ["include", "embed"] {
        for props in ["title=\"Good\"other=1", "title={name}other=1"] {
            let error = parse_template(&format!("{directive} child.crepus {props}")).unwrap_err();
            assert!(error
                .to_string()
                .contains("directive props require whitespace"));
        }
        for props in ["title=\"Good\" other=1", "title=name\tother=1"] {
            let nodes = parse_template(&format!("{directive} child.crepus {props}")).unwrap();
            let props = match &nodes[0] {
                crepuscularity_core::ast::Node::Include(node) => &node.props,
                crepuscularity_core::ast::Node::Embed(node) => &node.props,
                _ => panic!("expected directive"),
            };
            assert_eq!(props.len(), 2);
        }
    }
}

#[test]
fn directives_require_a_source_path() {
    for source in ["include", "include   ", "embed", "embed   "] {
        let error = parse_template(source).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("directive requires a source path"),
            "{error}"
        );
    }
    assert!(parse_template("include-button").is_ok());
    assert!(parse_template("embedded-panel").is_ok());
}

#[test]
fn directive_props_do_not_silently_drop_trailing_tokens() {
    for directive in ["include", "embed"] {
        for props in [
            "title=\"Good\" ignored",
            "ignored",
            "=value",
            "bad key=value",
        ] {
            let error = parse_template(&format!("{directive} child.crepus {props}")).unwrap_err();
            assert!(error.to_string().contains("directive prop"), "{error}");
        }
    }
}

#[test]
fn malformed_directive_prop_values_are_parse_errors() {
    for directive in ["include", "embed"] {
        for value in ["\"unclosed", "'unclosed", "{name", ""] {
            let source = format!("{directive} child.crepus title={value}");
            let error = parse_template(&source).unwrap_err();
            assert!(
                error.to_string().contains("directive prop value"),
                "{error}"
            );
        }
    }
}

#[test]
fn include_and_embed_props_ignore_quoted_braces() {
    for directive in ["include", "embed"] {
        let nodes = parse_template(&format!(
            "{directive} child.crepus title={{\"}}\"}} other={{\"{{\"}}"
        ))
        .unwrap();
        let props = match &nodes[0] {
            crepuscularity_core::ast::Node::Include(node) => &node.props,
            crepuscularity_core::ast::Node::Embed(node) => &node.props,
            _ => panic!("expected directive"),
        };
        assert_eq!(props.len(), 2);
        let ctx = crepuscularity_core::TemplateContext::new();
        for (prop, expected) in props.iter().zip(["}", "{"]) {
            let value = crepuscularity_core::eval::eval_expr(&prop.1, &ctx).unwrap();
            assert!(
                matches!(value, crepuscularity_core::TemplateValue::Str(value) if value == expected)
            );
        }
    }
}

#[test]
fn unclosed_text_interpolation_is_a_parse_error() {
    for source in ["\"Hello {name\"", "span \"Hello {name\""] {
        let error = parse_template(source).unwrap_err();
        assert!(
            error.to_string().contains("unclosed text interpolation"),
            "{error}"
        );
    }
}

#[test]
fn unterminated_quoted_text_is_an_error() {
    for source in [
        "\"Unclosed",
        "div\n  \"Unclosed\n  continuation",
        "\"雪",
        "\"Escaped \\\"",
    ] {
        let error = parse_template(source).unwrap_err();
        assert!(
            error.to_string().contains("unterminated quoted text"),
            "{error}"
        );
    }
    assert!(parse_template("div\n  \"First line\n  second line\"").is_ok());
    assert!(parse_template("\"Escaped \\\" quote\"").is_ok());
}

#[test]
fn orphan_control_flow_and_indented_declaration_children_are_errors() {
    for source in [
        "span\n  \"Before\"\nelse\n  span\n    \"Lost\"",
        "else if {ready}\n  span",
        "span\n_ =>\n  span",
        "$: let count = 1\n  span\n    \"Lost\"",
        "match {status}\n  \"ready\" =>\n    span\n   _ =>\n     div",
    ] {
        let error = parse_template(source).unwrap_err();
        assert!(
            error.to_string().contains("unexpected"),
            "{source}: {error}"
        );
    }
    assert!(parse_template("if {ready}\n  span\nelse\n  div").is_ok());
    let nodes =
        parse_template("match {status}\n  \"ready\" =>\n    span\n  _ =>\n    div").unwrap();
    let crepuscularity_core::ast::Node::Match(block) = &nodes[0] else {
        panic!("expected match block")
    };
    assert_eq!(block.arms.len(), 2);
    assert_eq!(block.arms[0].body.len(), 1);
    assert_eq!(block.arms[1].body.len(), 1);
}
