use std::fs;
use std::process::Command;

fn crepus() -> Command {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
}

#[test]
fn test_inspect_dump_html_error_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    // Use .svelte extension to trigger a parser that actually returns errors on invalid syntax
    // (the default indentation-based parser might just swallow `<invalid` or `{{ unclosed` as text/garbage).
    let template_file = temp_dir.path().join("invalid.svelte");

    // Svelte parser expects valid block structure. This should fail to parse.
    fs::write(&template_file, "{#if true}").unwrap();

    let output = crepus()
        .args([
            "inspect",
            template_file.to_str().unwrap(),
            "--mode",
            "render",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stderr.contains("Parse error") || stderr.contains("Render error"),
        "stderr was: {}, stdout was: {}",
        stderr,
        stdout
    );
}
