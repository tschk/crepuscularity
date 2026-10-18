use std::process::Command;

#[test]
fn render_rejects_unsupported_json_context_before_emitting_html() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("index.crepus");
    let context = directory.path().join("context.json");
    std::fs::write(&source, "span\n  \"CONTENT\"\n").unwrap();
    for value in [
        serde_json::json!([]),
        serde_json::json!({"object": {"nested": "discarded"}}),
        serde_json::json!({"items": [["discarded"]]}),
        serde_json::json!({"items": [{"nested": {"value": "discarded"}}]}),
        serde_json::json!({"items": [{"nested": ["discarded"]}]}),
    ] {
        std::fs::write(&context, value.to_string()).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .arg("render")
            .arg(&source)
            .arg("--ctx")
            .arg(&context)
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {value}");
        assert!(output.stdout.is_empty(), "partial HTML for {value}");
        assert!(!output.stderr.is_empty(), "missing diagnostic for {value}");
    }
}

#[test]
fn render_keeps_supported_json_scalars_and_loop_contexts() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("index.crepus");
    let context = directory.path().join("context.json");
    std::fs::write(
        &source,
        "div\n  \"{title}\"\n  for item in items\n    span\n      \"{item.label}\"\n",
    )
    .unwrap();
    std::fs::write(
        &context,
        serde_json::json!({"title": "A & B", "items": [{"label": "First"}, {"label": "Second"}],
            "scalars": [null, true, 42, 1.5, "text"], "nothing": null})
        .to_string(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
        .arg("render")
        .arg(&source)
        .arg("--ctx")
        .arg(&context)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let html = String::from_utf8(output.stdout).unwrap();
    for expected in ["A &amp; B", "First", "Second"] {
        assert!(html.contains(expected), "{html}");
    }
    let toml = directory.path().join("context.toml");
    std::fs::write(&toml, "title = \"TOML retained\"\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
        .arg("render")
        .arg(&source)
        .arg("--ctx")
        .arg(&toml)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("TOML retained"));
}

#[test]
fn render_rejects_invalid_json_instead_of_treating_it_as_toml() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("index.crepus");
    let context = directory.path().join("context.json");
    std::fs::write(&source, "span\n  \"CONTENT\"\n").unwrap();
    for invalid in ["{", r#"{"value":NaN}"#, r#"{"value":Infinity}"#] {
        std::fs::write(&context, invalid).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .arg("render")
            .arg(&source)
            .arg("--ctx")
            .arg(&context)
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {invalid}");
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("context JSON"));
    }
}
