#![cfg(feature = "std")]

use crepuscularity_core::TemplateContext;
use crepuscularity_embedded::{
    render_file_to_framebuffer, Rgb888Buffer, ScreenSize, Template, DEFAULT_BG,
};

#[test]
fn named_cached_templates_apply_defaults_without_overriding_props() {
    let screen = ScreenSize::new(128, 64);
    let mut fb = Rgb888Buffer::new(screen, DEFAULT_BG);
    let mut template = Template::from_source(
        "+++\n[Card.defaults]\ntitle = 'Default title'\n+++\n--- Card\nspan \"{title}\"",
        screen,
    );
    template.set_component("Card");
    assert!(format!("{:?}", template.draw(&mut fb).unwrap()).contains("Default title"));
    template.set("title", "Provided title");
    let document = format!("{:?}", template.draw(&mut fb).unwrap());
    assert!(document.contains("Provided title"));
    assert!(!document.contains("Default title"));
}

#[test]
fn file_template_and_include_select_vue_frontend() {
    let dir = tempfile::tempdir().unwrap();
    let child = dir.path().join("child.vue");
    std::fs::write(
        &child,
        "<script>const ignored = 1 < 2;</script><template><div>Vue output</div></template>",
    )
    .unwrap();
    let screen = ScreenSize::new(128, 64);
    let mut fb = Rgb888Buffer::new(screen, DEFAULT_BG);
    let document =
        render_file_to_framebuffer(&child, &TemplateContext::new(), screen, &mut fb).unwrap();
    assert!(format!("{document:?}").contains("Vue output"));
    let mut template = Template::from_path(&child, screen).unwrap();
    assert!(format!("{:?}", template.draw(&mut fb).unwrap()).contains("Vue output"));
    let parent = dir.path().join("entry.crepus");
    std::fs::write(&parent, "include child.vue").unwrap();
    let document =
        render_file_to_framebuffer(&parent, &TemplateContext::new(), screen, &mut fb).unwrap();
    assert!(format!("{document:?}").contains("Vue output"));
}
