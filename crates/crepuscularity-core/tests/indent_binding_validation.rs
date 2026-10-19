use crepuscularity_core::{ast::Node, parse_template};

#[test]
fn bindings_require_a_property_and_expression() {
    for source in [
        "div bind:",
        "div bind:value",
        "div bind:={value}",
        "div bind:value=",
        "div bind:value={}",
        "div bind:value={   }",
    ] {
        assert!(parse_template(source).is_err(), "{source}");
    }
}

#[test]
fn binding_removes_only_the_outer_expression_braces() {
    for (source, expected) in [
        ("div bind:value={{nested: value}}", "{nested: value}"),
        ("div bind:value={value}", "value"),
        ("div bind:value=value", "value"),
    ] {
        let nodes = parse_template(source).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("expected element");
        };
        assert_eq!(element.bindings[0].value, expected);
    }
}

#[test]
fn events_require_names_handlers_and_nonempty_modifier_entries() {
    for source in [
        "button @click",
        "button @=save",
        "button @click=",
        "button @click={}",
        "button @click={   }",
        "button @click|=save",
        "button @click||once=save",
    ] {
        assert!(parse_template(source).is_err(), "{source}");
    }
    let nodes = parse_template("button @keydown|ctrl+s=save @click={|_, _, _| {}}").unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!("expected element");
    };
    assert_eq!(element.event_handlers[0].modifiers, ["ctrl+s"]);
    assert_eq!(element.event_handlers[1].handler, "{|_, _, _| {}}");
}

#[test]
fn conditional_class_directives_validate_and_preserve_quoted_braces() {
    for source in [
        "div class:",
        "div class:={enabled}",
        "div class:hidden={}",
        "div when:",
        "div when:{}=hidden",
        "div when:enabled",
    ] {
        assert!(parse_template(source).is_err(), "{source}");
    }
    let (condition, classes) = crepuscularity_core::parser::parse_when_attribute_suffix(
        r#"{label == "}"}="hidden block""#,
    )
    .unwrap();
    assert_eq!(condition, r#"label == "}""#);
    assert_eq!(classes, r#""hidden block""#);
    let nodes = parse_template(r#"div when:{label == "}"}="hidden block""#).unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!("expected element");
    };
    assert_eq!(element.conditional_classes.len(), 2);
    assert_eq!(element.conditional_classes[0].condition, condition);
}

#[test]
fn animations_validate_settings_and_keep_quoted_and_braced_forms_equivalent() {
    for source in [
        "div animate:opacity",
        "div animate:={300ms}",
        "div animate:opacity={300ms linear forever}",
        "div animate:opacity={300ms linear repeat extra}",
    ] {
        assert!(parse_template(source).is_err(), "{source}");
    }
    for source in [
        r#"div animate:opacity="500ms ease-in repeat""#,
        "div animate:opacity={500ms ease-in repeat}",
    ] {
        let nodes = parse_template(source).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("expected element");
        };
        let animation = &element.animations[0];
        assert_eq!(animation.duration_expr, "500ms");
        assert_eq!(animation.easing, "ease-in");
        assert!(animation.repeat);
    }
}

#[test]
fn duplicate_id_forms_are_rejected_in_both_orders() {
    let forms = ["#first", "id=\"first\"", "bind:id={name}"];
    for first in forms {
        for second in forms {
            let source = format!("div {first} {second}");
            assert!(parse_template(&source).is_err(), "{source}");
        }
        assert!(parse_template(&format!("div {first}")).is_ok());
    }
}

#[test]
fn generic_attribute_bindings_reject_empty_expressions_but_allow_empty_literals() {
    for source in ["div title={}", "div data-value={   }", "input disabled={}"] {
        assert!(parse_template(source).is_err(), "{source}");
    }
    for source in [r#"div title="""#, "div title=", "div title={name}"] {
        assert!(parse_template(source).is_ok(), "{source}");
    }
}

#[test]
fn standalone_text_expressions_must_be_nonempty() {
    for source in ["{}", "{   }", "{=}", "{=   }"] {
        assert!(parse_template(source).is_err(), "{source}");
    }
    for source in ["{name}", "{=content}"] {
        assert!(parse_template(source).is_ok(), "{source}");
    }
}

#[test]
fn braced_control_expressions_reject_trailing_text_and_missing_braces() {
    for source in [
        "if {enabled} ignored",
        "match {status} ignored",
        "for item in {items} ignored",
        "$: let value = {name} ignored",
        "if {enabled",
        "$: default value = {name",
    ] {
        assert!(parse_template(source).is_err(), "{source}");
    }
}

#[test]
fn else_if_expressions_use_the_same_boundary_validation() {
    for condition in ["{enabled} ignored", "{enabled"] {
        let source = format!("if false\n  div\nelse if {condition}\n  span");
        assert!(parse_template(&source).is_err(), "{source}");
    }
    assert!(parse_template("if false\n  div\nelse if {enabled}\n  span").is_ok());
}

#[test]
fn braced_control_expressions_must_be_nonempty() {
    for source in [
        "if {}",
        "if {   }",
        "match {}",
        "for item in {}",
        "$: let value = {}",
        "$: default value = {}",
        "if false\n  div\nelse if {}\n  span",
    ] {
        assert!(parse_template(source).is_err(), "{source}");
    }
}

#[test]
fn declarations_require_names_and_values() {
    for source in [
        "$: let name",
        "$: let = value",
        "$: let name =",
        "$: default name",
        "$: default = value",
        "$: default name =",
    ] {
        assert!(parse_template(source).is_err(), "{source}");
    }
    for source in ["$: let name = value", "$: default name = value"] {
        assert!(parse_template(source).is_ok(), "{source}");
    }
}

#[test]
fn match_arms_reject_empty_expressions_but_allow_empty_string_patterns() {
    for pattern in ["{}", "{   }"] {
        let source = format!("match {{status}}\n  {pattern} =>\n    div");
        assert!(parse_template(&source).is_err(), "{source}");
    }
    assert!(parse_template("match {status}\n  \"\" =>\n    div").is_ok());
}

#[test]
fn loops_require_variables_and_iterators() {
    for source in ["for item", "for item in", "for  in items", "for item in {}"] {
        assert!(parse_template(source).is_err(), "{source}");
    }
    assert!(parse_template("for item in {items}\n  div").is_ok());
}

#[test]
fn orphan_control_flow_cannot_silently_truncate_a_template() {
    for source in [
        "div\nelse\n  span",
        "div\nelse if {enabled}\n  span",
        "div\n_ =>\n  span",
        "div\n  else\n    span",
        "div\n  _ =>\n    span",
        "match {status}\n  invalid arm\n    span",
    ] {
        assert!(parse_template(source).is_err(), "{source}");
    }
}

#[test]
fn reserved_control_headers_require_arguments() {
    for source in [
        "if",
        "for",
        "match",
        "$: let",
        "$: default",
        "if false\n  div\nelse if\n  span",
    ] {
        assert!(parse_template(source).is_err(), "{source}");
    }
    assert!(parse_template("div\n  span").is_ok());
}

#[test]
fn attribute_child_lines_preserve_directives_and_reject_duplicate_ids() {
    for source in [
        "div #parent\n  id=child",
        "div id={name}\n  bind:id={other}",
        "div\n  id=first\n  id=second",
    ] {
        assert!(parse_template(source).is_err(), "{source}");
    }
    let nodes = parse_template(
        "div\n  class:hidden={enabled}\n  @click=save\n  animate:opacity={300ms linear repeat}",
    )
    .unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!("expected element");
    };
    assert_eq!(element.conditional_classes.len(), 1);
    assert_eq!(element.event_handlers[0].handler, "save");
    assert!(element.animations[0].repeat);
    assert!(element.children.is_empty());
}

#[test]
fn multi_attribute_child_lines_preserve_all_attributes() {
    let nodes = parse_template(
        "div\n  id=entry class=\"border\" title=\"Title\" @click=save class:hidden={enabled}",
    )
    .unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!("expected element");
    };
    assert_eq!(element.id.as_deref(), Some("entry"));
    assert_eq!(element.classes, ["border"]);
    assert_eq!(element.bindings[0].prop, "title");
    assert_eq!(element.event_handlers[0].handler, "save");
    assert_eq!(element.conditional_classes.len(), 1);
    assert!(element.children.is_empty());
    assert!(parse_template("div\n  id=first bind:id={second}").is_err());
}

#[test]
fn quoted_text_interpolation_must_be_nonempty() {
    for source in [r#""Hello {}""#, r#"div "Hello {   }""#] {
        assert!(parse_template(source).is_err(), "{source}");
    }
    assert!(parse_template(r#""""#).is_ok());
    assert!(parse_template(r#""Literal {'{}'}""#).is_ok());
}
