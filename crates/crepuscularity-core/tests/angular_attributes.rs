use crepuscularity_core::{
    ast::{BindingOrigin, Node},
    eval::eval_binding,
    parse_template_with_path, TemplateContext, TemplateValue,
};
use std::path::Path;

#[test]
fn attribute_bindings_preserve_origin_and_nullable_strings() {
    let nodes = parse_template_with_path(
        r#"<input [attr.checked]="false" [attr.title]="null" [value]="false"/>"#,
        Some(Path::new("Entry.ng")),
    )
    .unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!()
    };
    let ctx = TemplateContext::new();
    assert_eq!(element.bindings[0].origin, BindingOrigin::Attribute);
    assert!(
        matches!(eval_binding(&element.bindings[0], &ctx).unwrap(), Some(TemplateValue::Str(value)) if value == "false")
    );
    assert!(eval_binding(&element.bindings[1], &ctx).unwrap().is_none());
    assert!(matches!(
        eval_binding(&element.bindings[2], &ctx).unwrap(),
        Some(TemplateValue::Bool(false))
    ));
}

#[test]
fn conflicting_attribute_writers_fail_explicitly() {
    for source in [
        r#"<input [attr.checked]="false" [checked]="true"/>"#,
        r#"<div class="static" [attr.class]="name"/>"#,
        r#"<div [attr.title]="name" title="static"/>"#,
        r#"<div id="static" [attr.ID]="name"/>"#,
    ] {
        let error = parse_template_with_path(source, Some(Path::new("Entry.ng"))).unwrap_err();
        assert!(error.to_string().contains("cannot be combined"), "{error}");
    }
}
