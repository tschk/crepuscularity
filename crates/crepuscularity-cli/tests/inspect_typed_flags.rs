use std::process::{Command, Output};

fn inspect(source: &std::path::Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .arg("inspect")
        .arg(source)
        .args(["--mode", "render"])
        .args(flags)
        .output()
        .unwrap()
}

#[test]
fn invalid_typed_flags_are_errors_instead_of_default_values() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("index.crepus");
    std::fs::write(&source, "p \"must not render\"\n").unwrap();
    for (flag, value) in [
        ("--bool", "enabled=maybe"),
        ("--bool", "enabled=TRUE"),
        ("--int", "count=nope"),
        ("--int", "count=1.5"),
        ("--int", "count=9223372036854775808"),
    ] {
        let output = inspect(&source, &[flag, value]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "accepted {flag} {value}: {stderr}"
        );
        assert!(output.stdout.is_empty(), "rendered invalid {flag} {value}");
        assert!(stderr.contains(flag), "{flag} {value}: {stderr}");
        assert!(stderr.contains(value), "{flag} {value}: {stderr}");
    }
}

#[test]
fn false_zero_and_negative_integers_remain_typed_values() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("index.crepus");
    std::fs::write(
        &source,
        "if enabled\n  p \"wrong true branch\"\nelse\n  p \"false branch\"\np \"{enabled}|{count}|{negative}\"\n",
    )
    .unwrap();
    let output = inspect(
        &source,
        &[
            "--bool",
            "enabled=false",
            "--int",
            "count=0",
            "--int",
            "negative=-7",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let html = String::from_utf8(output.stdout).unwrap();
    assert!(html.contains("false branch"), "{html}");
    assert!(!html.contains("wrong true branch"), "{html}");
    assert!(html.contains("false|0|-7"), "{html}");
}
