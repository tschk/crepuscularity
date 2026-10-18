use crepuscularity_core::{eval::eval_expr, TemplateContext, TemplateValue};

fn item(value: i64) -> TemplateContext {
    let mut context = TemplateContext::new();
    context.set("value", value);
    context
}
fn result(expression: &str, context: &TemplateContext) -> bool {
    let TemplateValue::Bool(value) = eval_expr(expression, context).unwrap() else {
        panic!("expected boolean")
    };
    value
}

#[test]
fn collections_compare_contents_instead_of_display_summaries() {
    let mut context = TemplateContext::new();
    context.set("a", TemplateValue::Scope(item(1)));
    context.set("b", TemplateValue::Scope(item(2)));
    context.set("same", TemplateValue::Scope(item(1)));
    context.set("xs", TemplateValue::List(vec![item(1), item(2)]));
    context.set("ys", TemplateValue::List(vec![item(2), item(1)]));
    context.set("same_list", TemplateValue::List(vec![item(1), item(2)]));
    for expression in [
        "a != b",
        "a == same",
        "xs != ys",
        "xs == same_list",
        "a != null",
        "xs != '[2 items]'",
        "a != ''",
        "xs != a",
    ] {
        assert!(result(expression, &context), "{expression}");
    }
}

#[test]
fn collection_ordering_errors_and_short_circuits_skip_it() {
    let mut context = TemplateContext::new();
    context.set("a", TemplateValue::Scope(item(1)));
    for expression in ["a < a", "a >= null", "0 > a"] {
        assert!(eval_expr(expression, &context).is_err(), "{expression}");
    }
    assert!(result("true || a < a", &context));
    assert!(!result("false && a < a", &context));
}

#[test]
fn nested_fields_keys_and_lengths_participate_in_equality() {
    let mut left = item(1);
    left.set("children", TemplateValue::List(vec![item(2)]));
    let mut right = TemplateContext::new();
    right.set("children", TemplateValue::List(vec![item(2)]));
    right.set("value", 1_i64);
    right.base_dir = Some("/different/render/resources".into());
    let mut different = right.clone();
    different.set("children", TemplateValue::List(vec![item(3)]));
    let mut different_key = right.clone();
    different_key.vars.remove("value");
    different_key.set("other", 1_i64);
    let mut context = TemplateContext::new();
    for (name, value) in [
        ("left", left),
        ("right", right),
        ("different", different),
        ("different_key", different_key),
    ] {
        context.set(name, TemplateValue::Scope(value));
    }
    context.set("empty", TemplateValue::List(vec![]));
    context.set("one", TemplateValue::List(vec![item(1)]));
    for expression in [
        "left == right",
        "left != different",
        "left != different_key",
        "empty != one",
        "empty == empty",
    ] {
        assert!(result(expression, &context), "{expression}");
    }
}
