use crepuscularity_core::TemplateContext;
use crepuscularity_embedded::{
    render_template_to_framebuffer, Rgb888Buffer, ScreenSize, DEFAULT_BG,
};
use std::path::Path;
use std::process::Command;

fn snapshot(cwd: &Path, source: &Path, out: &Path, reference: &str) {
    let result = Command::new(env!("CARGO_BIN_EXE_crepus"))
        .current_dir(cwd)
        .arg("embedded")
        .arg("snapshot")
        .arg(source)
        .args(["--width", "16", "--height", "8", "--out"])
        .arg(out)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut buffer = Rgb888Buffer::new(ScreenSize::new(16, 8), DEFAULT_BG);
    render_template_to_framebuffer(
        reference,
        &TemplateContext::new(),
        ScreenSize::new(16, 8),
        &mut buffer,
    )
    .unwrap();
    let mut expected = b"P6\n16 8\n255\n".to_vec();
    expected.extend_from_slice(buffer.as_rgb888_bytes());
    assert_eq!(std::fs::read(out).unwrap(), expected);
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn snapshot_resolves_sibling_include_beside_input_outside_working_directory() {
    let directory = tempfile::tempdir().unwrap();
    let app = directory.path().join("app");
    let cwd = directory.path().join("caller");
    std::fs::create_dir(&app).unwrap();
    std::fs::create_dir(&cwd).unwrap();
    let reference = "div w-full h-full bg-[#ff0000]";
    std::fs::write(app.join("entry.crepus"), "include part.crepus").unwrap();
    std::fs::write(app.join("part.crepus"), reference).unwrap();
    // A misleading cwd sibling must never win over the input file's sibling.
    std::fs::write(cwd.join("part.crepus"), "div w-full h-full bg-[#0000ff]").unwrap();
    snapshot(
        &cwd,
        &app.join("entry.crepus"),
        &directory.path().join("include.ppm"),
        reference,
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn snapshot_uses_vue_filename_to_select_template_frontend() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("entry.vue");
    std::fs::write(&source, "<script>const ignored = 1 < 2;</script><template><div class=\"w-full h-full bg-[#0000ff]\" /></template>").unwrap();
    snapshot(
        directory.path(),
        &source,
        &directory.path().join("vue.ppm"),
        "div w-full h-full bg-[#0000ff]",
    );
}
