/// Cargo build runner + JSON diagnostic parser.
///
/// Runs `cargo build --message-format=json-diagnostic-rendered-ansi`,
/// streams the JSON output, and returns a `BuildOutcome`.
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde::Deserialize;

use crate::build_options::BuildOptions;
use crate::hud::{BuildError, DevStatus, HudState};

// ── Cargo JSON schema ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CargoLine {
    reason: String,
    message: Option<CompilerMsg>,
    success: Option<bool>,
    target: Option<CargoTarget>,
    executable: Option<PathBuf>,
}

impl CargoLine {
    fn binary_executable(self) -> Option<(String, PathBuf)> {
        if self.reason != "compiler-artifact" {
            return None;
        }
        let target = self.target?;
        if !target.kind.iter().any(|kind| kind == "bin") {
            return None;
        }
        Some((target.name, self.executable?))
    }
}

#[derive(Deserialize, Default)]
struct CargoTarget {
    #[serde(default)]
    name: String,
    #[serde(default)]
    kind: Vec<String>,
}

#[derive(Deserialize)]
struct CompilerMsg {
    level: String,
    message: String,
    rendered: Option<String>,
    spans: Vec<MsgSpan>,
}

#[derive(Deserialize, Default, Clone)]
struct MsgSpan {
    file_name: String,
    line_start: u32,
}

// ── Public API ─────────────────────────────────────────────────────────────

pub struct BuildOutcome {
    pub success: bool,
    pub errors: Vec<BuildError>,
    executables: Vec<(String, PathBuf)>,
}

#[derive(Clone)]
pub enum BinSelection {
    Required(String),
    Package(String),
}

impl BinSelection {
    pub fn name(&self) -> &str {
        match self {
            Self::Required(name) | Self::Package(name) => name,
        }
    }
}

impl std::fmt::Display for BinSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl BuildOutcome {
    pub fn executable_for(&self, selection: &BinSelection) -> Result<(String, PathBuf), String> {
        if self
            .executables
            .iter()
            .any(|(name, _)| name == selection.name())
        {
            return self
                .executable(selection.name())
                .map(|path| (selection.name().to_owned(), path));
        }
        if let BinSelection::Package(_) = selection {
            return match self.executables.as_slice() {
                [(name, path)] => Ok((name.clone(), path.clone())),
                [] => Err("Cargo reported no executable; select a binary with --bin".into()),
                _ => {
                    Err("Cargo reported multiple executable binaries; select one with --bin".into())
                }
            };
        }
        self.executable(selection.name())
            .map(|path| (selection.name().to_owned(), path))
    }

    pub fn executable(&self, bin_name: &str) -> Result<PathBuf, String> {
        let mut matches = self.executables.iter().filter(|(name, _)| name == bin_name);
        let Some((_, path)) = matches.next() else {
            return Err(format!(
                "Cargo did not report an executable for binary '{bin_name}'"
            ));
        };
        if matches.next().is_some() {
            return Err(format!(
                "Cargo reported multiple executables for binary '{bin_name}'"
            ));
        }
        Ok(path.clone())
    }
}

/// Run `cargo build` in `cwd`.
///
/// Diagnostics are streamed to stderr immediately (ANSI rendered).
/// Errors are also collected in the returned `BuildOutcome` for the HUD.
/// `hud` is optional — pass `None` when running outside the dev loop.
pub fn cargo_build(
    cwd: &Path,
    options: BuildOptions,
    hud: Option<Arc<Mutex<HudState>>>,
) -> BuildOutcome {
    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--message-format=json-diagnostic-rendered-ansi")
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()) // Cargo setup failures are not JSON diagnostics
        .env("CARGO_BUILD_INCREMENTAL", "true");

    options.apply_profile_to_command(&mut cmd);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[crepus] Failed to spawn cargo: {e}");
            return BuildOutcome {
                success: false,
                errors: vec![fallback_error(format!("failed to spawn cargo: {e}"), &hud)],
                executables: vec![],
            };
        }
    };

    let stderr = child.stderr.take().unwrap();
    let stderr_reader = std::thread::spawn(move || {
        let mut stderr = stderr;
        let mut captured = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let count = match stderr.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => count,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            };
            let _ = std::io::stderr().write_all(&buffer[..count]);
            captured.extend_from_slice(&buffer[..count]);
        }
        Ok::<_, std::io::Error>(String::from_utf8_lossy(&captured).into_owned())
    });
    let stdout = child.stdout.take().unwrap();
    let mut errors: Vec<BuildError> = Vec::new();
    let mut success = false;
    let mut executables = Vec::new();

    for line in BufReader::new(stdout).lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let msg: CargoLine = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(_) => continue,
        };

        match msg.reason.as_str() {
            "compiler-message" => {
                let Some(cm) = msg.message else { continue };

                // Always print rendered output to terminal
                if let Some(ref rendered) = cm.rendered {
                    eprint!("{rendered}");
                }

                if cm.level == "error" {
                    let span = cm.spans.first().cloned().unwrap_or_default();
                    let err = BuildError {
                        level: cm.level,
                        message: cm.message,
                        file: span.file_name,
                        line: span.line_start,
                        rendered: cm.rendered,
                    };
                    errors.push(err.clone());

                    // Live-update HUD so spinner shows errors as they arrive
                    if let Some(ref hud) = hud {
                        if let Ok(mut state) = hud.lock() {
                            let count = errors.len();
                            state.status = DevStatus::Failed {
                                errors: errors.clone(),
                                count,
                            };
                        }
                    }
                }
            }
            "compiler-artifact" => {
                if let Some(artifact) = msg.binary_executable() {
                    executables.push(artifact);
                }
            }
            "build-finished" => {
                success = msg.success.unwrap_or(false);
            }
            _ => {}
        }
    }

    let status = child.wait();
    let stderr = match stderr_reader.join() {
        Ok(Ok(stderr)) => stderr,
        Ok(Err(error)) => format!("read Cargo stderr: {error}"),
        Err(_) => "Cargo stderr reader panicked".to_string(),
    };
    success &= status.as_ref().is_ok_and(|status| status.success());
    if !success && errors.is_empty() {
        let message = if !stderr.trim().is_empty() {
            stderr.trim_end().to_string()
        } else {
            match status {
                Ok(status) => format!("Cargo build failed ({status})"),
                Err(error) => format!("wait for Cargo: {error}"),
            }
        };
        errors.push(fallback_error(message, &hud));
    }
    BuildOutcome {
        success,
        errors,
        executables,
    }
}

fn fallback_error(message: String, hud: &Option<Arc<Mutex<HudState>>>) -> BuildError {
    let error = BuildError {
        level: "error".into(),
        message,
        ..Default::default()
    };
    if let Some(hud) = hud {
        if let Ok(mut state) = hud.lock() {
            state.status = DevStatus::Failed {
                errors: vec![error.clone()],
                count: 1,
            };
        }
    }
    error
}

/// Kill a child process (best-effort).
pub fn kill_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Find the binary name to run after a build.
/// An explicit override wins, then `package.default-run`, the first named `[[bin]]`,
/// and finally `[package] name`.
pub fn find_bin_selection(cwd: &Path, override_name: Option<&str>) -> Option<BinSelection> {
    if let Some(name) = override_name {
        return Some(BinSelection::Required(name.to_string()));
    }
    let content = std::fs::read_to_string(cwd.join("Cargo.toml")).ok()?;
    let manifest: toml::Value = toml::from_str(&content).ok()?;
    if let Some(name) = manifest
        .get("package")
        .and_then(|package| package.get("default-run"))
        .and_then(toml::Value::as_str)
        .filter(|name| !name.is_empty())
    {
        return Some(BinSelection::Required(name.to_owned()));
    }
    let name = |value: &toml::Value| {
        value
            .get("name")
            .and_then(toml::Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
    };
    manifest
        .get("bin")
        .and_then(toml::Value::as_array)
        .and_then(|bins| bins.iter().find_map(name))
        .map(BinSelection::Required)
        .or_else(|| {
            manifest
                .get("package")
                .and_then(name)
                .map(BinSelection::Package)
        })
}
#[cfg(test)]
pub fn find_bin_name(cwd: &Path, override_name: Option<&str>) -> Option<String> {
    find_bin_selection(cwd, override_name).map(|selection| selection.name().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_artifacts_filter_non_bins_and_keep_fresh_custom_paths() {
        for fresh in [false, true] {
            let raw = format!(
                r#"{{"reason":"compiler-artifact","target":{{"name":"app","kind":["bin"]}},"executable":"/custom/debug/app.exe","fresh":{fresh}}}"#
            );
            let artifact: CargoLine = serde_json::from_str(&raw).unwrap();
            assert_eq!(
                artifact.binary_executable(),
                Some(("app".into(), PathBuf::from("/custom/debug/app.exe")))
            );
        }
        for raw in [
            r#"{"reason":"compiler-artifact","target":{"name":"build-script-build","kind":["custom-build"]},"executable":"/wrong/build-script-build"}"#,
            r#"{"reason":"compiler-artifact","target":{"name":"lib","kind":["lib"]},"executable":null}"#,
            r#"{"reason":"compiler-artifact","target":{},"executable":"/wrong/unknown"}"#,
            r#"{"reason":"compiler-artifact","target":{"name":"app","kind":["bin"]},"executable":null}"#,
            r#"{"reason":"build-finished","success":true}"#,
        ] {
            let artifact: CargoLine = serde_json::from_str(raw).unwrap();
            assert!(artifact.binary_executable().is_none(), "{raw}");
        }
    }

    #[test]
    fn executable_selection_requires_one_matching_binary() {
        let mut outcome = BuildOutcome {
            success: true,
            errors: vec![],
            executables: vec![
                ("other".into(), PathBuf::from("/custom/debug/other")),
                ("app".into(), PathBuf::from("/custom/debug/app")),
            ],
        };
        assert_eq!(
            outcome.executable("app").unwrap(),
            PathBuf::from("/custom/debug/app")
        );
        assert!(outcome
            .executable("missing")
            .unwrap_err()
            .contains("did not report"));
        outcome
            .executables
            .push(("app".into(), PathBuf::from("/another/debug/app")));
        assert!(outcome.executable("app").unwrap_err().contains("multiple"));
    }

    #[test]
    fn parse_build_finished_msg() {
        let json = r#"{"reason":"build-finished","success":true}"#;
        let msg: CargoLine = serde_json::from_str(json).unwrap();
        assert_eq!(msg.reason, "build-finished");
        assert_eq!(msg.success, Some(true));
    }

    #[test]
    fn parse_compiler_error_msg() {
        let json = r#"{
            "reason": "compiler-message",
            "package_id": "foo 0.1.0",
            "target": {},
            "message": {
                "level": "error",
                "message": "expected `;`",
                "rendered": "error: expected `;`\n --> src/main.rs:1:1\n",
                "spans": [{"file_name": "src/main.rs", "line_start": 1, "line_end": 1, "byte_start": 0, "byte_end": 1, "column_start": 1, "column_end": 1, "text": [], "is_primary": true}]
            }
        }"#;
        let msg: CargoLine = serde_json::from_str(json).unwrap();
        assert_eq!(msg.reason, "compiler-message");
        let cm = msg.message.unwrap();
        assert_eq!(cm.level, "error");
        assert_eq!(cm.message, "expected `;`");
        assert!(cm.rendered.unwrap().contains("expected `;`"));
        assert_eq!(cm.spans[0].file_name, "src/main.rs");
        assert_eq!(cm.spans[0].line_start, 1);
    }

    #[test]
    fn parse_compiler_warning_msg() {
        let json = r#"{
            "reason": "compiler-message",
            "package_id": "bar 0.2.0",
            "target": {},
            "message": {
                "level": "warning",
                "message": "unused variable",
                "rendered": "warning: unused variable `x`\n --> src/lib.rs:5:9\n",
                "spans": [{"file_name": "src/lib.rs", "line_start": 5, "line_end": 5, "byte_start": 0, "byte_end": 1, "column_start": 9, "column_end": 10, "text": [], "is_primary": true}]
            }
        }"#;
        let msg: CargoLine = serde_json::from_str(json).unwrap();
        let cm = msg.message.unwrap();
        assert_eq!(cm.level, "warning");
        assert_eq!(cm.message, "unused variable");
    }

    #[test]
    fn parse_invalid_json_does_not_panic() {
        let result: Result<CargoLine, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn find_bin_name_prefers_bin_over_package() {
        let dir = tempfile::tempdir().unwrap();
        let toml_path = dir.path().join("Cargo.toml");
        std::fs::write(
            &toml_path,
            r#"[package]
name = "my-lib"

[[bin]]
name = "my-app"
"#,
        )
        .unwrap();
        assert_eq!(find_bin_name(dir.path(), None), Some("my-app".into()));
    }

    #[test]
    fn find_bin_name_falls_back_to_package() {
        let dir = tempfile::tempdir().unwrap();
        let toml_path = dir.path().join("Cargo.toml");
        std::fs::write(
            &toml_path,
            r#"[package]
name = "my-lib"
"#,
        )
        .unwrap();
        assert_eq!(find_bin_name(dir.path(), None), Some("my-lib".into()));
    }

    #[test]
    fn find_bin_name_override_wins() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            find_bin_name(dir.path(), Some("custom-bin")),
            Some("custom-bin".into())
        );
    }

    #[test]
    fn find_bin_name_no_toml_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(find_bin_name(dir.path(), None), None);
    }

    #[test]
    fn executable_selection_preserves_required_names_and_exact_ambiguity() {
        let mut outcome = BuildOutcome {
            success: true,
            errors: Vec::new(),
            executables: vec![("worker".into(), PathBuf::from("/fixture/worker"))],
        };
        for manifest in [
            "[package]\nname='host'\ndefault-run='missing'\n",
            "[package]\nname='host'\n[[bin]]\nname='missing'\n",
        ] {
            let root = tempfile::tempdir().unwrap();
            std::fs::write(root.path().join("Cargo.toml"), manifest).unwrap();
            let selection = find_bin_selection(root.path(), None).unwrap();
            assert!(outcome.executable_for(&selection).is_err());
        }
        assert_eq!(
            outcome
                .executable_for(&BinSelection::Package("host".into()))
                .unwrap()
                .0,
            "worker"
        );
        outcome
            .executables
            .push(("worker".into(), PathBuf::from("/another/worker")));
        assert!(outcome
            .executable_for(&BinSelection::Package("worker".into()))
            .unwrap_err()
            .contains("multiple"));
        assert!(outcome
            .executable_for(&BinSelection::Required("worker".into()))
            .unwrap_err()
            .contains("multiple"));
        assert!(outcome
            .executable_for(&BinSelection::Package("host".into()))
            .unwrap_err()
            .contains("--bin"));
        outcome.executables.clear();
        assert!(outcome
            .executable_for(&BinSelection::Package("host".into()))
            .unwrap_err()
            .contains("--bin"));
    }
}
