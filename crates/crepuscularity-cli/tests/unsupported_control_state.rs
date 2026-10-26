use std::{fs, process::Command};

#[test]
fn unsupported_control_state_fails_before_replacing_generated_ir() {
    for target in [
        "react",
        "svelte",
        "vue",
        "solid",
        "imba",
        "moonshine-dom",
        "moonshine-ssr",
    ] {
        for source in [
            "input disabled=true",
            "picker\n  option readonly=true value=one label=One",
        ] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join("index.crepus"), source).unwrap();
            let dist = site.path().join("dist");
            fs::create_dir(&dist).unwrap();
            let ir = dist.join("crepus-view-ir.json");
            fs::write(&ir, "existing artifact").unwrap();
            let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
                .args(["web", "build", "--emit", target, "--site"])
                .arg(site.path())
                .output()
                .unwrap();
            assert!(!output.status.success(), "{target}: {source}");
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(
                diagnostic.contains("unsupported View IR control state"),
                "{target}: {diagnostic}"
            );
            assert_eq!(
                fs::read_to_string(&ir).unwrap(),
                "existing artifact",
                "{target}: {source}"
            );
            assert_eq!(
                fs::read_dir(&dist).unwrap().count(),
                1,
                "{target}: {source}"
            );
        }
    }
}
