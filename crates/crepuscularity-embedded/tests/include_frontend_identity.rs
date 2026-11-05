#![cfg(all(unix, feature = "std"))]
use crepuscularity_core::{parse_template, TemplateContext};
use crepuscularity_embedded::{render_nodes_to_document, EmbeddedNode, ScreenSize};
use std::os::unix::fs::symlink;

fn fixture() -> (tempfile::TempDir, TemplateContext) {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.data");
    std::fs::write(
        &source,
        "<template><span v-if=\"show\" id=\"payload\">VISIBLE</span></template>",
    )
    .unwrap();
    symlink(&source, dir.path().join("view.vue")).unwrap();
    symlink(&source, dir.path().join("view.jsx")).unwrap();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(dir.path().into());
    (dir, ctx)
}
fn count(nodes: &[EmbeddedNode]) -> usize {
    nodes
        .iter()
        .map(|n| usize::from(n.id.as_deref() == Some("payload")) + count(&n.children))
        .sum()
}
#[test]
fn symlinked_vue_include_keeps_authored_frontend_and_prop_condition() {
    let (_dir, ctx) = fixture();
    let nodes =
        parse_template("include view.vue show={false}\ninclude view.vue show={true}").unwrap();
    let doc = render_nodes_to_document(&nodes, &ctx, ScreenSize::new(64, 32)).unwrap();
    assert_eq!(count(&doc.root), 1);
}
#[test]
fn same_physical_file_vue_and_jsx_aliases_do_not_share_wrong_ast() {
    let (_dir, ctx) = fixture();
    for source in [
        "include view.vue show={false}\ninclude view.jsx show={false}",
        "include view.jsx show={false}\ninclude view.vue show={false}",
    ] {
        let nodes = parse_template(source).unwrap();
        let doc = render_nodes_to_document(&nodes, &ctx, ScreenSize::new(64, 32)).unwrap();
        assert_eq!(count(&doc.root), 1, "{source}");
    }
}
