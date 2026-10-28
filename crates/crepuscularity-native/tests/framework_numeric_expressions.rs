use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir,
};

#[test]
fn framework_emitters_preserve_scientific_number_literals() {
    let mut context = TemplateContext::new();
    context.set("value", 7);
    for literal in ["1e3", "1E-3", "1e+3", ".5e2", "1.e3", "-2.5E-2"] {
        let expression = format!("{literal} + value");
        let ir = render_template_to_ir(&format!("text \"{{{expression}}}\""), &context).unwrap();
        for (name, output, scope) in [
            (
                "moonshine",
                emit_moonshine_component(&ir, "Numbers"),
                "scope.",
            ),
            ("solid", emit_solid_component(&ir, "Numbers"), "scope()."),
            ("svelte", emit_svelte_component(&ir), "scope."),
            ("vue", emit_vue_component(&ir), "scope."),
        ] {
            let expected = format!("{literal} + {scope}value");
            assert!(
                output.contains(&expected),
                "{name}: expected {expected:?} in {output}"
            );
        }
    }
}

#[test]
fn framework_emitters_preserve_dsl_unknown_string_escapes() {
    let ir = render_template_to_ir(r#"text "{'C:\q'}""#, &TemplateContext::new()).unwrap();
    for output in [
        emit_moonshine_component(&ir, "Literal"),
        emit_solid_component(&ir, "Literal"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        assert!(output.contains(r#""C:\\q""#), "{output}");
    }
}

#[test]
fn framework_emitters_treat_dsl_null_aliases_as_literals() {
    let mut context = TemplateContext::new();
    for alias in ["none", "None", "Null"] {
        context.set(alias, "must not read this field");
        let ir = render_template_to_ir(&format!("text \"{{{alias}}}\""), &context).unwrap();
        let canonical = render_template_to_ir("text \"{null}\"", &context).unwrap();
        assert_eq!(
            emit_moonshine_component(&ir, "Nulls"),
            emit_moonshine_component(&canonical, "Nulls")
        );
        assert_eq!(
            emit_solid_component(&ir, "Nulls"),
            emit_solid_component(&canonical, "Nulls")
        );
        assert_eq!(
            emit_svelte_component(&ir),
            emit_svelte_component(&canonical)
        );
        assert_eq!(emit_vue_component(&ir), emit_vue_component(&canonical));
    }
}

fn all_numeric_outputs(ir: &crepuscularity_native::ViewIr) -> Vec<(&'static str, String)> {
    use crepuscularity_native::{
        emit_imba_component, emit_moonshine_dom_component, emit_moonshine_ssr_component,
        emit_react_component,
    };
    vec![
        ("react", emit_react_component(ir, "Numbers")),
        ("solid", emit_solid_component(ir, "Numbers")),
        ("svelte", emit_svelte_component(ir)),
        ("vue", emit_vue_component(ir)),
        ("imba", emit_imba_component(ir).unwrap().bindings),
        ("dom", emit_moonshine_dom_component(ir, "Numbers").unwrap()),
        ("ssr", emit_moonshine_ssr_component(ir, "Numbers").unwrap()),
    ]
}

#[test]
fn adjacent_minus_operators_preserve_dsl_arithmetic_in_every_framework() {
    let mut context = TemplateContext::new();
    context.set("count", 3);
    for (expression, snapshot, tokens) in [
        ("--count", "3", "- -HOSTcount"),
        ("1--2", "3", "1- -2"),
        ("count--2", "5", "HOSTcount- -2"),
        ("---count", "-3", "- - -HOSTcount"),
        ("2---count", "-1", "2- - -HOSTcount"),
        ("1e-3--2", "2.001", "1e-3- -2"),
    ] {
        let ir = render_template_to_ir(&format!("text \"{{{expression}}}\""), &context).unwrap();
        assert!(
            matches!(&ir.root[0], crepuscularity_native::ViewNode::Text { content, .. } if content == snapshot),
            "{expression}: {:?}",
            ir.root
        );
        for (framework, output) in all_numeric_outputs(&ir) {
            let prefix = if matches!(framework, "solid" | "dom" | "ssr") {
                "scope()."
            } else {
                "scope."
            };
            let expected = tokens.replace("HOST", prefix);
            assert!(
                output.contains(&expected),
                "{framework}: missing {expected:?} in {output}"
            );
        }
    }
}

#[test]
fn minus_guards_preserve_conditions_loop_aliases_and_quoted_text() {
    use crepuscularity_core::{TemplateContext, TemplateValue};
    let mut context = TemplateContext::new();
    context.set("count", 3);
    let mut item = TemplateContext::new();
    item.set("count", 3);
    context.set("items", TemplateValue::List(vec![item]));
    let ir = render_template_to_ir(
        "if --count > 0\n  text \"{'--'}\"\nfor item in items\n  text \"{--item.count}\"",
        &context,
    )
    .unwrap();
    for (framework, output) in all_numeric_outputs(&ir) {
        let prefix = if matches!(framework, "solid" | "dom" | "ssr") {
            "scope()."
        } else {
            "scope."
        };
        let decoded = output.replace("&gt;", ">").replace("&quot;", "\"");
        assert!(
            decoded.contains(&format!("- -{prefix}count > 0")),
            "{framework}: {output}"
        );
        assert!(
            decoded.contains("- -_crepusItem0?.count"),
            "{framework}: {output}"
        );
        assert!(
            decoded.contains("'--'"),
            "{framework}: quoted minus changed: {output}"
        );
    }
}
