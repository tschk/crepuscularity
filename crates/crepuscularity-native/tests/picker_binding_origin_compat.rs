use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

#[test]
fn portable_picker_presence_and_typed_literals_keep_their_contracts() {
    for (filename, source, expected) in [
        ("Entry.ng", "<file-picker [attr.MULTIPLE]=\"false\"/>", true),
        ("Entry.ng", "<file-picker [attr.MULTIPLE]=\"null\"/>", false),
        ("Entry.ng", "<file-picker [multiple]=\"false\"/>", true),
        ("Entry.ng", "<file-picker [multiple]=\"null\"/>", false),
        ("Entry.jsx", "<file-picker multiple='false'/>", true),
        ("Entry.jsx", "<input type='file' multiple={false}/>", false),
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        let ir = serde_json::to_value(ir).unwrap();
        assert_eq!(
            ir["root"][0]["multiple"], expected,
            "{filename}: {source}: {ir}"
        );
    }
}

#[test]
fn typed_ast_literal_false_uses_boolean_coercion() {
    use crepuscularity_core::ast::{BindingOrigin, Node};
    let mut nodes = crepuscularity_core::parse_template_with_path(
        "<file-picker multiple='false'/>",
        Some(Path::new("Entry.jsx")),
    )
    .unwrap();
    let Node::Element(element) = &mut nodes[0] else {
        panic!("expected picker");
    };
    // Literal provenance is supplied by authored compilers or public AST callers.
    element.bindings[0].origin = BindingOrigin::Literal;
    let ir = crepuscularity_native::render_nodes_to_ir(&nodes, &TemplateContext::new()).unwrap();
    assert_eq!(
        serde_json::to_value(ir).unwrap()["root"][0]["multiple"],
        false
    );
}
