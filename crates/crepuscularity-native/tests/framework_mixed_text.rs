use crepuscularity_core::ast::TextPart;
use crepuscularity_core::text_binding::encode_text_binding;
use crepuscularity_native::{
    emit_imba_component, emit_react_component, emit_solid_component, emit_svelte_component,
    emit_vue_component, ViewIr, ViewNode, IR_VERSION,
};

fn outputs(ir: &ViewIr) -> Vec<(&'static str, String, &'static str)> {
    let imba = emit_imba_component(ir).unwrap();
    vec![
        ("react", emit_react_component(ir, "Mixed"), "scope."),
        ("solid", emit_solid_component(ir, "Mixed"), "scope()."),
        ("svelte", emit_svelte_component(ir), "scope."),
        ("vue", emit_vue_component(ir), "scope."),
        ("imba", imba.bindings, "scope."),
    ]
}

fn normalized_markup(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&#123;", "{")
        .replace("&#125;", "}")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[test]
fn mixed_text_converts_each_dynamic_value_using_core_display_semantics() {
    let parts = vec![
        TextPart::Literal("prefix {literal} & <tag> \"quoted\" \\path: ".into()),
        TextPart::Expr("flag".into()),
        TextPart::Literal(" / ".into()),
        TextPart::Expr("items".into()),
        TextPart::Literal(" / ".into()),
        TextPart::Expr("object".into()),
        TextPart::Literal(" / ".into()),
        TextPart::Expr("null".into()),
        TextPart::Literal(" / ".into()),
        TextPart::Expr("missing".into()),
    ];
    let ir = ViewIr {
        version: IR_VERSION,
        root: vec![ViewNode::Text {
            content: "snapshot".into(),
            bind: encode_text_binding(&parts),
            style: None,
        }],
    };
    let TextPart::Literal(prefix) = &parts[0] else {
        unreachable!()
    };
    let prefix = serde_json::Value::String(prefix.clone())
        .to_string()
        .replace('{', "\\u007b")
        .replace('}', "\\u007d");
    for (name, output, scope) in outputs(&ir) {
        let output = normalized_markup(&output);
        assert!(output.contains(&prefix), "{name}: {output}");
        for binding in ["flag", "items", "object", "missing"] {
            assert!(
                output.contains(&format!("__crepusText({scope}{binding})")),
                "{name}: {output}"
            );
        }
        assert!(output.contains("__crepusText(null)"), "{name}: {output}");
        assert_eq!(
            output.matches("__crepusText(").count(),
            6,
            "five parts plus runtime helper: {name}: {output}"
        );
        assert!(
            output.contains("`[${value.length} items]`"),
            "{name}: {output}"
        );
        assert!(!output.contains("__crepusText(\"\" +"), "{name}: {output}");
    }
}

#[test]
fn mixed_text_inside_loops_resolves_each_part_with_current_local_alias() {
    let parts = vec![
        TextPart::Literal("row: ".into()),
        TextPart::Expr("row.value".into()),
        TextPart::Literal(" / ".into()),
        TextPart::Expr("suffix".into()),
    ];
    let ir = ViewIr {
        version: IR_VERSION,
        root: vec![ViewNode::ForEach {
            bind: "rows".into(),
            item_name: "row".into(),
            style: None,
            item_body: vec![ViewNode::Text {
                content: "snapshot".into(),
                bind: encode_text_binding(&parts),
                style: None,
            }],
        }],
    };
    for (name, output, scope) in outputs(&ir) {
        let output = normalized_markup(&output);
        assert!(
            output.contains("__crepusText(_crepusItem0?.value)"),
            "{name}: {output}"
        );
        assert!(
            output.contains(&format!("__crepusText({scope}suffix)")),
            "{name}: {output}"
        );
        assert!(
            !output.contains("__crepusText(scope.row?.value)"),
            "{name}: {output}"
        );
    }
}

#[test]
fn ordinary_noncanonical_expression_bindings_keep_their_existing_semantics() {
    let ir = ViewIr {
        version: IR_VERSION,
        root: vec![ViewNode::Text {
            content: "snapshot".into(),
            bind: Some("count + 1".into()),
            style: None,
        }],
    };
    for (name, output, scope) in outputs(&ir) {
        assert!(
            output.contains(&format!("__crepusText({scope}count + 1)")),
            "{name}: {output}"
        );
    }
}

#[test]
fn native_mixed_text_uses_existing_per_part_display_and_snapshot_fallback() {
    use crepuscularity_native::{generate_native_source, NativeCodegenTarget};
    let parts = vec![
        TextPart::Literal("Prefix \"quoted\" \n".into()),
        TextPart::Expr("first".into()),
        TextPart::Literal(" / ".into()),
        TextPart::Expr("second".into()),
    ];
    let text = ViewNode::Text {
        content: "initial snapshot".into(),
        bind: encode_text_binding(&parts),
        style: None,
    };
    let ir = ViewIr {
        version: IR_VERSION,
        root: vec![text],
    };
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        let output = generate_native_source(&ir, target, "Mixed");
        assert!(output.contains(".text(\"first\")"), "{output}");
        assert!(output.contains(".text(\"second\")"), "{output}");
        assert!(output.contains("Prefix \\\"quoted\\\" \\n"), "{output}");
        assert!(!output.contains("initial snapshot"), "{output}");
        let arithmetic = ViewIr {
            version: IR_VERSION,
            root: vec![ViewNode::Text {
                content: "Total: 3".into(),
                bind: encode_text_binding(&[
                    TextPart::Literal("Total: ".into()),
                    TextPart::Expr("1 + 2".into()),
                ]),
                style: None,
            }],
        };
        let output = generate_native_source(&arithmetic, target, "Mixed");
        assert!(output.contains("Text(\"Total: 3\""), "{output}");
        assert!(!output.contains(".text(\"1 + 2\""), "{output}");
        let plain = ViewIr {
            version: IR_VERSION,
            root: vec![ViewNode::Text {
                content: "snapshot".into(),
                bind: Some("first".into()),
                style: None,
            }],
        };
        assert!(generate_native_source(&plain, target, "Plain").contains(".text(\"first\")"));
    }
}

#[test]
fn native_mixed_text_keeps_existing_loop_scope_arguments() {
    use crepuscularity_native::{generate_native_source, NativeCodegenTarget};
    let ir = ViewIr {
        version: IR_VERSION,
        root: vec![ViewNode::ForEach {
            bind: "rows".into(),
            item_name: "row".into(),
            style: None,
            item_body: vec![ViewNode::Text {
                content: "row: initial".into(),
                bind: encode_text_binding(&[
                    TextPart::Literal("row: ".into()),
                    TextPart::Expr("row.value".into()),
                ]),
                style: None,
            }],
        }],
    };
    for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
        let output = generate_native_source(&ir, target, "Mixed");
        assert!(output.contains(".text(\"row.value\","), "{output}");
        assert!(!output.contains("row: initial"), "{output}");
    }
}

#[test]
fn native_text_and_button_constants_do_not_freeze_adjacent_live_paths() {
    use crepuscularity_native::{generate_native_source, LabelPart, NativeCodegenTarget};
    for (expression, expected_literal) in [
        ("7", r#""7""#),
        ("-2.5", r#""-2.5""#),
        ("'hello'", r#""hello""#),
        ("true", r#""true""#),
        (r#"'C:\q'"#, r#""C:\\q""#),
        ("None", r#""""#),
        ("Null", r#""""#),
        ("none", r#""""#),
        ("null", r#""""#),
        (
            r#""line\nquote\" and \\path""#,
            r#""line\nquote\" and \\path""#,
        ),
    ] {
        let parts = vec![
            TextPart::Literal("Name: ".into()),
            TextPart::Expr("name".into()),
            TextPart::Literal(" / ".into()),
            TextPart::Expr(expression.into()),
            TextPart::Literal(" literal {data} ".into()),
        ];
        let label_parts = parts
            .iter()
            .map(|part| match part {
                TextPart::Literal(value) => LabelPart::Literal(value.clone()),
                TextPart::Expr(value) => LabelPart::Expr(value.clone()),
            })
            .collect();
        let ir = ViewIr {
            version: IR_VERSION,
            root: vec![
                ViewNode::Text {
                    content: "frozen snapshot".into(),
                    bind: encode_text_binding(&parts),
                    style: None,
                },
                ViewNode::Button {
                    label: "frozen snapshot".into(),
                    label_parts,
                    on_click: None,
                    on_long_press: None,
                    style: None,
                },
            ],
        };
        for target in [NativeCodegenTarget::SwiftUi, NativeCodegenTarget::Compose] {
            let output = generate_native_source(&ir, target, "Constants");
            assert_eq!(
                output.matches(".text(\"name\")").count(),
                2,
                "{expression}: {output}"
            );
            assert!(
                output.contains(&format!(" + {expected_literal} + \" literal {{data}} \"")),
                "{expression}: {output}"
            );
            assert!(
                !output.contains("frozen snapshot"),
                "{expression}: {output}"
            );
            let escaped_expression = serde_json::to_string(expression).unwrap();
            assert!(
                !output.contains(&format!(".text({escaped_expression}")),
                "constant should not reach host evaluator: {expression}: {output}"
            );
        }
    }
}
