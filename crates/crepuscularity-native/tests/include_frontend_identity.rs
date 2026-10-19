#![cfg(unix)]

use crepuscularity_core::TemplateContext;
use crepuscularity_native::{render_template_to_ir, ViewNode};
use std::os::unix::fs::symlink;

#[test]
fn vue_include_alias_keeps_authored_frontend_after_canonical_resolution() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("payload.crepus"),
        "<template><span v-if=\"visible\">Shown</span></template>",
    )
    .unwrap();
    symlink("payload.crepus", root.path().join("first.vue")).unwrap();
    symlink("payload.crepus", root.path().join("second.vue")).unwrap();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(root.path().to_path_buf());
    let ir = render_template_to_ir(
        "include first.vue visible=true\ninclude second.vue visible=false",
        &ctx,
    )
    .unwrap();
    assert_eq!(ir.root.len(), 2);
    for node in ir.root {
        let ViewNode::If {
            condition,
            then_children,
            ..
        } = node
        else {
            panic!("Vue include alias lost its conditional frontend: {node:?}");
        };
        assert_eq!(condition, "visible");
        assert!(serde_json::to_string(&then_children)
            .unwrap()
            .contains("Shown"));
    }
}
