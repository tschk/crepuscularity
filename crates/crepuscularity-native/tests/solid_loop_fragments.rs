use crepuscularity_native::{emit_solid_component, render_template_to_ir, ViewNode};

#[test]
fn solid_loop_bodies_are_single_jsx_expressions_for_any_child_count() {
    for children in [0, 1, 2] {
        let mut ir = render_template_to_ir("text \"Seed\"", &Default::default()).unwrap();
        let text = ir.root[0].clone();
        ir.root = vec![ViewNode::ForEach {
            bind: "rows".into(),
            item_name: "row".into(),
            item_body: vec![text; children],
            style: None,
        }];
        let output = emit_solid_component(&ir, "Rows");
        assert!(output.contains("=> (<>\n"), "{output}");
        assert!(output.contains("</>)}</For>"), "{output}");
        assert_eq!(output.matches("<span").count(), children);
    }
}
