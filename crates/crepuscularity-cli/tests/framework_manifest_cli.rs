//! Framework emit must use the same manifest path and override rules as HTML builds.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn write(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

struct Fixture {
    _temp: tempfile::TempDir,
    project: PathBuf,
    caller: PathBuf,
    manifest: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("manifest project");
        let caller = temp.path().join("unrelated caller");
        write(
            caller.join("index.crepus"),
            "text \"Wrong caller source\"\n",
        );
        write(
            project.join("site a/index.crepus"),
            "text \"Wrong first target\"\n",
        );
        write(
            project.join("site b/pages/start page.crepus"),
            "text \"Chosen target\"\n",
        );
        let manifest = project.join("crepus.toml");
        write(
            &manifest,
            r#"[[targets]]
type = "web"
id = "other"
site = "site a"
[[targets]]
type = "web"
id = "chosen"
site = "site b"
entry = "pages/start page.crepus"
out = "results/chosen"
"#,
        );
        Self {
            _temp: temp,
            project,
            caller,
            manifest,
        }
    }

    fn command(&self, emit: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_crepus"));
        command
            .current_dir(&self.caller)
            .args(["web", "build", "--emit", emit]);
        command
    }
}

fn success(output: Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_text(output: &Path, expected: &str) {
    let ir: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(output.join("crepus-view-ir.json")).unwrap())
            .unwrap();
    assert_eq!(ir["root"][0]["content"], expected);
    assert!(!output.join("runtime").exists());
}

#[test]
fn framework_manifest_selection_and_explicit_overrides_share_html_path_rules() {
    let fixture = Fixture::new();
    success(
        fixture
            .command("react")
            .arg("--manifest")
            .arg(&fixture.manifest)
            .args(["--target", "chosen"])
            .output()
            .unwrap(),
    );
    assert_text(&fixture.project.join("results/chosen"), "Chosen target");

    let override_site = fixture.project.join("override site");
    write(
        override_site.join("pages/start page.crepus"),
        "text \"Override site\"\n",
    );
    write(
        override_site.join("override.crepus"),
        "text \"Override entry\"\n",
    );
    success(
        fixture
            .command("react")
            .arg("--manifest")
            .arg(&fixture.manifest)
            .args(["--target", "chosen", "--site"])
            .arg(&override_site)
            .output()
            .unwrap(),
    );
    assert_text(&fixture.project.join("results/chosen"), "Override site");

    let explicit_output = fixture.project.join("explicit output");
    success(
        fixture
            .command("react")
            .arg("--manifest")
            .arg(&fixture.manifest)
            .args(["--target", "chosen", "--site"])
            .arg(&override_site)
            .args(["--entry", "override.crepus", "--output"])
            .arg(&explicit_output)
            .output()
            .unwrap(),
    );
    assert_text(&explicit_output, "Override entry");
}

#[test]
fn framework_emit_rejects_missing_invalid_ambiguous_and_unknown_target_selection() {
    let fixture = Fixture::new();
    let invalid = fixture.project.join("invalid.toml");
    write(&invalid, "not valid TOML [[[");
    for (manifest, target, diagnostic) in [
        (
            fixture.project.join("missing.toml"),
            None,
            "cannot load web targets",
        ),
        (invalid, None, "cannot load web targets"),
        (fixture.manifest.clone(), None, "pass --target ID"),
        (
            fixture.manifest.clone(),
            Some("absent"),
            "no web target with id",
        ),
    ] {
        let output_dir = fixture.project.join("must not be written");
        let mut command = fixture.command("react");
        command
            .arg("--manifest")
            .arg(manifest)
            .arg("--output")
            .arg(&output_dir);
        if let Some(target) = target {
            command.args(["--target", target]);
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output_dir.exists());
    }
}

#[test]
fn explicit_site_uses_only_its_local_manifest_and_validates_local_selectors() {
    let fixture = Fixture::new();
    let site = fixture.project.join("standalone site");
    write(site.join("index.crepus"), "text \"Standalone\"\n");
    // The caller's parent manifest has two targets; --site must not discover it.
    success(
        fixture
            .command("react")
            .current_dir(&fixture.project)
            .arg("--site")
            .arg(&site)
            .output()
            .unwrap(),
    );
    assert_text(&site.join("dist"), "Standalone");
    write(
        site.join("crepus.toml"),
        "[[targets]]\ntype='web'\nid='local'\nsite='.'\nentry='local.crepus'\n",
    );
    write(site.join("local.crepus"), "text \"Local target\"\n");
    success(
        fixture
            .command("react")
            .arg("--site")
            .arg(&site)
            .args(["--target", "local"])
            .output()
            .unwrap(),
    );
    assert_text(&site.join("dist"), "Local target");
    let output = fixture
        .command("react")
        .arg("--site")
        .arg(&site)
        .args(["--target", "missing"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no web target with id"));
}

#[test]
fn manifest_selected_authored_module_entry_keeps_its_filename_and_graph_route() {
    let fixture = Fixture::new();
    write(
        fixture.project.join("site b/src/Main App.tsx"),
        "export default function App() { return <span>Authored module</span>; }\n",
    );
    write(&fixture.manifest, "[[targets]]\ntype='web'\nid='module'\nsite='site b'\nentry='src/Main App.tsx'\nout='results/module'\n");
    success(
        fixture
            .command("moonshine-dom")
            .arg("--manifest")
            .arg(&fixture.manifest)
            .args(["--target", "module"])
            .output()
            .unwrap(),
    );
    let output = fixture.project.join("results/module");
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(output.join("crepus-module-manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["sourceEntry"], "src/Main App.tsx");
    assert!(output.join(manifest["entry"].as_str().unwrap()).is_file());
    assert!(!output.join("crepus-view-ir.json").exists());
}

#[test]
fn explicit_site_reports_bad_local_manifest_before_touching_any_output() {
    let failures: &[&str] = if cfg!(unix) {
        &["syntax", "directory", "invalid utf8", "dangling symlink"]
    } else {
        &["syntax", "directory", "invalid utf8"]
    };
    for &failure in failures {
        for output_state in ["absent", "existing", "file"] {
            let fixture = Fixture::new();
            let site = fixture.project.join("local site");
            write(site.join("index.crepus"), "text \"Must not fallback\"\n");
            let manifest = site.join("crepus.toml");
            let diagnostic = match failure {
                #[cfg(unix)]
                "dangling symlink" => {
                    std::os::unix::fs::symlink(site.join("missing-target.toml"), &manifest)
                        .unwrap();
                    "read"
                }
                "syntax" => {
                    write(
                        &manifest,
                        "[[targets]]\ntype='web'\nentry='chosen.crepus'\n[targets.seo\n",
                    );
                    "parse"
                }
                "directory" => {
                    std::fs::create_dir(&manifest).unwrap();
                    "read"
                }
                _ => {
                    std::fs::write(&manifest, [0xff]).unwrap();
                    "read"
                }
            };
            let output_dir = site.join("dist");
            match output_state {
                "existing" => {
                    write(output_dir.join("CrepusEmit.react.tsx"), "keep component");
                    write(output_dir.join("crepus-view-ir.json"), "keep IR");
                }
                "file" => write(&output_dir, "keep output file"),
                _ => {}
            }
            let output = fixture
                .command("react")
                .arg("--site")
                .arg(&site)
                .output()
                .unwrap();
            assert!(
                !output.status.success(),
                "{failure}/{output_state} unexpectedly succeeded"
            );
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(diagnostic) && stderr.contains("crepus.toml"),
                "{stderr}"
            );
            match output_state {
                "existing" => {
                    assert_eq!(
                        std::fs::read_to_string(output_dir.join("CrepusEmit.react.tsx")).unwrap(),
                        "keep component"
                    );
                    assert_eq!(
                        std::fs::read_to_string(output_dir.join("crepus-view-ir.json")).unwrap(),
                        "keep IR"
                    );
                }
                "file" => assert_eq!(
                    std::fs::read_to_string(output_dir).unwrap(),
                    "keep output file"
                ),
                _ => assert!(!output_dir.exists()),
            }
        }
    }
}

#[test]
fn explicit_site_with_valid_non_web_manifest_retains_default_entry_and_output() {
    let fixture = Fixture::new();
    let site = fixture.project.join("non web site");
    write(site.join("index.crepus"), "text \"Default entry\"\n");
    write(site.join("crepus.toml"), "[ios]\nscheme='Host'\n");
    success(
        fixture
            .command("react")
            .arg("--site")
            .arg(&site)
            .output()
            .unwrap(),
    );
    assert_text(&site.join("dist"), "Default entry");
}
