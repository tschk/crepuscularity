use crepuscularity_core::TemplateContext;

#[test]
fn interpolation_requires_an_expression_inside_braces() {
    let context = TemplateContext::new();
    for source in ["{}", "before {   } after", "{\n\t}", "{1} {}"] {
        assert!(context.interpolate(source).is_err(), "accepted {source:?}");
    }
}

#[test]
fn explicit_empty_values_and_missing_variables_keep_their_output() {
    let context = TemplateContext::new();
    for source in ["{''}", "{null}", "{missing}", ""] {
        assert_eq!(context.interpolate(source).unwrap(), "");
    }
    assert_eq!(
        context.interpolate("before {''} after").unwrap(),
        "before  after"
    );
    assert_eq!(context.interpolate("{1 + 2}").unwrap(), "3");
}
