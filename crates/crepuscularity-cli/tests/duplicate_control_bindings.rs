use std::{fs, process::Command};

#[test]
fn duplicate_control_bindings_fail_before_publishing_components() {
    for source in [
        "checkbox checked=true checked=false",
        "input bind=user.first bind=user.second",
        "picker\n  option value=one value=two label=Choice",
    ] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("index.crepus"), source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .args(["web", "build", "--emit", "imba", "--site"])
            .arg(site.path())
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {source}");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.contains("duplicate") && diagnostic.contains("binding"),
            "{diagnostic}"
        );
        assert!(
            !site.path().join("dist").exists(),
            "failed build published output"
        );
    }
}
