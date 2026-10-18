use crepuscularity_core::eval::validate_expr;

#[test]
fn malformed_expressions_are_rejected_without_execution() {
    for expression in [
        "",
        " ",
        "value +",
        "(value",
        "value)",
        "value &&",
        "call()",
        "value; alert(1)",
    ] {
        assert!(
            validate_expr(expression).is_err(),
            "accepted {expression:?}"
        );
    }
}

#[test]
fn valid_expressions_do_not_resolve_names_or_perform_arithmetic() {
    for expression in [
        "runtime.ready",
        "missing > 0",
        "1 / 0",
        "9223372036854775807 * 9223372036854775807",
        "false && (1 / 0)",
        "'literal'",
        "(value + 2) > other",
    ] {
        assert!(validate_expr(expression).is_ok(), "rejected {expression:?}");
    }
}
