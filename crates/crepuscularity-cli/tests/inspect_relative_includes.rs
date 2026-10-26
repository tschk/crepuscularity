use std::path::Path;
use std::process::Command;

fn inspect(source: &Path, cwd: &Path, mode: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
        .arg("inspect")
        .arg(source)
        .args(["--mode", mode])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn inspect_render_resolves_sibling_includes_from_the_source_directory() {
    let source = tempfile::tempdir().unwrap();
    let caller = tempfile::tempdir().unwrap();
    std::fs::write(
        source.path().join("child.crepus"),
        "span \"SOURCE SIBLING\"",
    )
    .unwrap();
    std::fs::write(caller.path().join("child.crepus"), "span \"WRONG CWD\"").unwrap();
    for (filename, content) in [
        ("index.crepus", "include child.crepus"),
        ("index.jsx", "<include src=\"child.crepus\" />"),
    ] {
        let path = source.path().join(filename);
        std::fs::write(&path, content).unwrap();
        let html = inspect(&path, caller.path(), "render");
        assert!(html.contains("SOURCE SIBLING"), "{filename}: {html}");
        assert!(!html.contains("WRONG CWD"), "{filename}: {html}");
    }
}

#[test]
fn inspect_ir_resolves_named_component_includes_from_the_source_directory() {
    let source = tempfile::tempdir().unwrap();
    let caller = tempfile::tempdir().unwrap();
    std::fs::write(
        source.path().join("child.crepus"),
        "text \"SOURCE SIBLING\"",
    )
    .unwrap();
    std::fs::write(caller.path().join("child.crepus"), "text \"WRONG CWD\"").unwrap();
    let path = source.path().join("Card.crepus");
    std::fs::write(&path, "--- Card\ninclude child.crepus").unwrap();
    let json = inspect(&path, caller.path(), "ir");
    let _: serde_json::Value =
        serde_json::from_str(&json).expect("inspect emits valid View IR JSON");
    assert!(json.contains("SOURCE SIBLING"), "{json}");
    assert!(!json.contains("WRONG CWD"), "{json}");
}

#[test]
fn inspect_ir_dispatches_flat_frontends_and_resolves_source_relative_includes() {
    let source = tempfile::tempdir().unwrap();
    let caller = tempfile::tempdir().unwrap();
    std::fs::write(
        source.path().join("child.crepus"),
        "span \"SOURCE SIBLING\"",
    )
    .unwrap();
    std::fs::write(caller.path().join("child.crepus"), "span \"WRONG CWD\"").unwrap();
    for (filename, content, expects_include) in [
        ("flat.crepus", "span \"FLAT\"\ninclude child.crepus", true),
        (
            "flat.jsx",
            "<div><span>FLAT</span><include src=\"child.crepus\" /></div>",
            true,
        ),
        ("flat.vue", "<template><span>FLAT</span></template>", false),
        ("flat.svelte", "<span>FLAT</span>", false),
        ("flat.astro", "<span>FLAT</span>", false),
        ("flat.ng.html", "<span>FLAT</span>", false),
    ] {
        let path = source.path().join(filename);
        std::fs::write(&path, content).unwrap();
        let json = inspect(&path, caller.path(), "ir");
        let ir: serde_json::Value = serde_json::from_str(&json).expect("valid View IR JSON");
        assert!(ir["root"].is_array(), "{filename}: {json}");
        assert!(json.contains("FLAT"), "{filename}: {json}");
        assert_eq!(
            json.contains("SOURCE SIBLING"),
            expects_include,
            "{filename}: {json}"
        );
        assert!(!json.contains("WRONG CWD"), "{filename}: {json}");
    }
}

#[test]
fn inspect_ir_keeps_missing_named_component_as_an_error() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("Missing.crepus");
    std::fs::write(
        &path,
        "--- Other\nspan \"must not select another component\"",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
        .arg("inspect")
        .arg(&path)
        .args(["--mode", "ir"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Missing"), "{stderr}");
    assert!(stderr.to_lowercase().contains("component"), "{stderr}");
}

#[test]
fn inspect_ir_markup_text_does_not_become_dsl_component_sections() {
    let root = tempfile::tempdir().unwrap();
    for (filename, source) in [
        (
            "literal.vue",
            "<template><pre>FLAT\n--- Card\nkept</pre></template>",
        ),
        ("literal.jsx", "<pre>FLAT\n--- Card\nkept</pre>"),
    ] {
        let path = root.path().join(filename);
        std::fs::write(&path, source).unwrap();
        let json = inspect(&path, root.path(), "ir");
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(json.contains("FLAT"), "{filename}: {json}");
        assert!(json.contains("--- Card"), "{filename}: {json}");
        assert!(json.contains("kept"), "{filename}: {json}");
    }
}
