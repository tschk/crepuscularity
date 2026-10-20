#![cfg(feature = "std")]

use crepuscularity_core::TemplateContext;
use crepuscularity_embedded::{render_file_to_framebuffer, Rgb888Buffer, ScreenSize, DEFAULT_BG};

#[test]
fn include_depth_accepts_64_and_rejects_65() {
    let dir = tempfile::tempdir().unwrap();
    for index in 0..64 {
        std::fs::write(
            dir.path().join(format!("{index}.crepus")),
            format!("include {}.crepus", index + 1),
        )
        .unwrap();
    }
    std::fs::write(dir.path().join("64.crepus"), "span \"Boundary\"").unwrap();
    let screen = ScreenSize::new(64, 32);
    let mut fb = Rgb888Buffer::new(screen, DEFAULT_BG);
    let entry = dir.path().join("0.crepus");
    let document =
        render_file_to_framebuffer(&entry, &TemplateContext::new(), screen, &mut fb).unwrap();
    assert!(format!("{document:?}").contains("Boundary"));
    std::fs::write(dir.path().join("64.crepus"), "include 65.crepus").unwrap();
    std::fs::write(dir.path().join("65.crepus"), "span").unwrap();
    let error =
        render_file_to_framebuffer(&entry, &TemplateContext::new(), screen, &mut fb).unwrap_err();
    assert!(error.to_string().contains("maximum include depth (64)"));
}

#[test]
fn named_component_cycles_are_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let entry = dir.path().join("entry.crepus");
    std::fs::write(&entry, "include ui.crepus#Card").unwrap();
    std::fs::write(
        dir.path().join("ui.crepus"),
        "--- Card\ninclude ui.crepus#Card",
    )
    .unwrap();
    let screen = ScreenSize::new(64, 32);
    let mut fb = Rgb888Buffer::new(screen, DEFAULT_BG);
    let error =
        render_file_to_framebuffer(&entry, &TemplateContext::new(), screen, &mut fb).unwrap_err();
    assert!(error.to_string().contains("maximum include depth"));
}

#[test]
fn cyclic_includes_error_and_depth_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cycle.crepus");
    std::fs::write(&path, "include cycle.crepus").unwrap();
    let screen = ScreenSize::new(64, 32);
    let mut fb = Rgb888Buffer::new(screen, DEFAULT_BG);
    let error =
        render_file_to_framebuffer(&path, &TemplateContext::new(), screen, &mut fb).unwrap_err();
    assert!(error.to_string().contains("maximum include depth"));
    std::fs::write(&path, "span \"Recovered\"").unwrap();
    let parent = dir.path().join("entry.crepus");
    std::fs::write(&parent, "include cycle.crepus").unwrap();
    let document =
        render_file_to_framebuffer(&parent, &TemplateContext::new(), screen, &mut fb).unwrap();
    assert!(format!("{document:?}").contains("Recovered"));
}
