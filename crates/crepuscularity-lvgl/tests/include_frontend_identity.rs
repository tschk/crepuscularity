#![cfg(unix)]
use crepuscularity_core::TemplateContext;
use crepuscularity_lvgl::render_template_to_lvgl_xml;
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

#[test]
fn symlinked_vue_include_keeps_its_authored_frontend_and_prop_condition() {
    let (_dir, ctx) = fixture();
    let xml = render_template_to_lvgl_xml(
        "include view.vue show={false}\ninclude view.vue show={true}",
        &ctx,
    )
    .unwrap();
    assert_eq!(xml.matches("id=\"payload\"").count(), 1, "{xml}");
}

#[test]
fn one_physical_file_has_separate_ordinary_asts_for_vue_and_jsx_aliases() {
    let (_dir, ctx) = fixture();
    for source in [
        "include view.vue show={false}\ninclude view.jsx show={false}",
        "include view.jsx show={false}\ninclude view.vue show={false}",
    ] {
        let xml = render_template_to_lvgl_xml(source, &ctx).unwrap();
        assert_eq!(xml.matches("id=\"payload\"").count(), 1, "{source}: {xml}");
    }
}
