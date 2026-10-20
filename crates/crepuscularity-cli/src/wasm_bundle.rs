//! Shared helpers for `wasm32-unknown-unknown` + wasm-bindgen (web + webext).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::build_options::{BuildOptions, OptimizationLevel};

/// Locate the runtime library artifact, never an unrelated file in the target directory.
pub fn find_wasm_file(dir: &Path, runtime_dir: &Path) -> Option<PathBuf> {
    let source = std::fs::read_to_string(runtime_dir.join("Cargo.toml")).ok()?;
    let manifest: toml::Value = toml::from_str(&source).ok()?;
    let name = manifest
        .get("lib")
        .and_then(|lib| lib.get("name"))
        .and_then(toml::Value::as_str)
        .or_else(|| manifest.get("package")?.get("name")?.as_str())?;
    let artifact = dir.join(format!("{}.wasm", name.replace('-', "_")));
    artifact.is_file().then_some(artifact)
}

/// Ask Cargo where this runtime writes artifacts, including workspace/config overrides.
pub fn wasm_profile_dir(runtime_dir: &Path, options: BuildOptions) -> Result<PathBuf, String> {
    let mut cmd = Command::new(cargo_executable());
    prepend_rustup_bin_to_path(&mut cmd);
    // Match cargo_build_wasm32's environment policy.
    cmd.env_remove("CARGO_TARGET_DIR");
    let output = cmd
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ])
        .current_dir(runtime_dir)
        .output()
        .map_err(|error| format!("cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cargo metadata JSON: {error}"))?;
    let target = metadata
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .ok_or("cargo metadata omitted target_directory")?;
    Ok(Path::new(target)
        .join("wasm32-unknown-unknown")
        .join(options.cargo_profile()))
}

/// Prefer Cargo home's installed toolchain proxies over unrelated PATH tools.
fn rustup_bin_dir() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CARGO_HOME") {
        let bin = std::path::absolute(home).ok()?.join("bin");
        return bin.is_dir().then_some(bin);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let p = Path::new(&home).join(".cargo/bin");
        if p.is_dir() {
            return Some(p);
        }
    }
    #[cfg(windows)]
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let p = Path::new(&profile).join(".cargo/bin");
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

fn prepend_rustup_bin_to_path(cmd: &mut Command) {
    // Nested Cargo changes directory; keep its home anchored to the caller.
    if let Some(home) = std::env::var_os("CARGO_HOME") {
        if let Ok(home) = std::path::absolute(home) {
            cmd.env("CARGO_HOME", home);
        }
    }
    let Some(bin) = rustup_bin_dir() else {
        return;
    };
    let new_path = match std::env::var_os("PATH") {
        Some(rest) => {
            let mut v = std::ffi::OsString::from(&bin);
            v.push(if cfg!(windows) { ";" } else { ":" });
            v.push(rest);
            v
        }
        None => bin.into_os_string(),
    };
    cmd.env("PATH", new_path);
}

fn cargo_executable() -> std::ffi::OsString {
    if let Some(c) = std::env::var_os("CARGO") {
        return c;
    }
    if let Some(bin) = rustup_bin_dir() {
        let p = bin.join(if cfg!(windows) { "cargo.exe" } else { "cargo" });
        if p.is_file() {
            return p.into_os_string();
        }
    }
    "cargo".into()
}

/// Enable incremental compilation and sccache caching for wasm32-unknown-unknown.
///
/// - `CARGO_BUILD_INCREMENTAL=true` avoids full re-link on small changes.
/// - `SCCACHE` is detected on `PATH` and enabled automatically for crate-level
///   reuse across builds (more effective than the `cargo`-level `--quiet` flag).
///
/// Prefer rustup's `cargo` when `PATH` points at a non-rustup toolchain (e.g. Homebrew rustc
/// without wasm std). Uses `CARGO` if set, else Cargo home's executable when present.
pub fn cargo_build_wasm32(runtime_dir: &Path, options: BuildOptions) -> Result<(), String> {
    let cargo_exe = cargo_executable();
    let mut cmd = Command::new(cargo_exe);
    prepend_rustup_bin_to_path(&mut cmd);
    cmd.env_remove("CARGO_TARGET_DIR");
    cmd.env("CARGO_BUILD_INCREMENTAL", "true");

    // Detect sccache on PATH and enable it automatically for crate-level reuse.
    let sccache_available = std::process::Command::new("sccache")
        .arg("--version")
        .output()
        .ok()
        .is_some_and(|o| o.status.success());
    if sccache_available && std::env::var_os("RUSTC_WRAPPER").is_none() {
        cmd.env("RUSTC_WRAPPER", "sccache");
    }

    cmd.args(["build", "--target", "wasm32-unknown-unknown"]);
    options.apply_profile_to_command(&mut cmd);
    let out = cmd
        .arg("--quiet")
        .current_dir(runtime_dir)
        .output()
        .map_err(|e| format!("cargo: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn wasm_bindgen_command(wasm_path: &Path, out_dir: &Path, out_name: &str) -> Command {
    let mut cmd = Command::new("wasm-bindgen");
    cmd.args(["--target", "web", "--out-dir"])
        .arg(out_dir)
        .args(["--out-name", out_name])
        .arg(wasm_path);
    cmd
}

pub fn run_wasm_bindgen(wasm_path: &Path, out_dir: &Path, out_name: &str) -> Result<(), String> {
    let out = wasm_bindgen_command(wasm_path, out_dir, out_name)
        .output()
        .map_err(|e| format!("wasm-bindgen: {e}"))?;
    if out.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    // version mismatch: "rust Wasm file schema version: 0.2.123"
    if let Some(ver) = parse_wasm_bindgen_needed_version(&stderr) {
        return Err(format!(
            "{stderr}\nThis module requires wasm-bindgen-cli {ver}. Select a matching wasm-bindgen executable on PATH or update the runtime dependency and rebuild. The installed tool was not changed."
        ));
    }

    Err(stderr.into_owned())
}

fn parse_wasm_bindgen_needed_version(stderr: &str) -> Option<String> {
    // "rust Wasm file schema version: 0.2.123"
    for line in stderr.lines() {
        if line.contains("rust Wasm file schema version:") {
            return line
                .split("rust Wasm file schema version:")
                .nth(1)?
                .split_whitespace()
                .next()
                .map(|s| s.to_string());
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmOptStatus {
    Optimized,
    NotInstalled,
}

pub fn run_wasm_opt(
    wasm_path: &Path,
    optimization: OptimizationLevel,
) -> Result<WasmOptStatus, String> {
    let Some(level) = optimization.wasm_opt_flag() else {
        return Ok(WasmOptStatus::Optimized);
    };
    let available = Command::new("wasm-opt")
        .arg("--version")
        .output()
        .ok()
        .is_some_and(|o| o.status.success());
    if !available {
        return Ok(WasmOptStatus::NotInstalled);
    }

    let tmp = wasm_path.with_extension("wasm.opt");
    let out = Command::new("wasm-opt")
        .args([level, "--enable-bulk-memory"])
        .arg(wasm_path)
        .arg("-o")
        .arg(&tmp)
        .output()
        .map_err(|e| format!("wasm-opt: {e}"))?;
    if !out.status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    std::fs::rename(&tmp, wasm_path).map_err(|e| format!("replace optimized wasm: {e}"))?;
    Ok(WasmOptStatus::Optimized)
}

#[cfg(test)]
mod artifact_tests {
    use super::*;

    #[test]
    fn cargo_target_directory_respects_nested_workspace_configuration() {
        let root = tempfile::tempdir().unwrap();
        let runtime = root.path().join("app/runtime");
        std::fs::create_dir_all(runtime.join("src")).unwrap();
        std::fs::create_dir_all(root.path().join(".cargo")).unwrap();
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[workspace]\nmembers=['app/runtime']\nresolver='2'\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join(".cargo/config.toml"),
            "[build]\ntarget-dir='artifacts'\n",
        )
        .unwrap();
        std::fs::write(runtime.join("Cargo.toml"), "[package]\nname='fixture-runtime'\nversion='0.1.0'\nedition='2021'\n[lib]\ncrate-type=['cdylib']\n").unwrap();
        std::fs::write(runtime.join("src/lib.rs"), "pub fn fixture() {}\n").unwrap();
        let options = crate::build_options::BuildOptionsArgs::default()
            .into_options()
            .unwrap();
        let expected = root
            .path()
            .canonicalize()
            .unwrap()
            .join("artifacts/wasm32-unknown-unknown/debug");
        assert_eq!(wasm_profile_dir(&runtime, options).unwrap(), expected);
    }

    #[test]
    #[cfg(unix)]
    fn bindgen_arguments_preserve_non_utf8_paths() {
        use std::os::unix::ffi::OsStrExt;
        let wasm = Path::new(std::ffi::OsStr::from_bytes(b"input \xff.wasm"));
        let output = Path::new(std::ffi::OsStr::from_bytes(b"output \xfe"));
        let command = wasm_bindgen_command(wasm, output, "runtime");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args[3].as_bytes(), b"output \xfe");
        assert_eq!(args[6].as_bytes(), b"input \xff.wasm");
    }

    #[test]
    fn selects_runtime_library_and_rejects_stale_artifacts() {
        let root = tempfile::tempdir().unwrap();
        let runtime = root.path().join("runtime");
        let target = root.path().join("target");
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(
            runtime.join("Cargo.toml"),
            "[package]\nname = 'my-runtime'\n",
        )
        .unwrap();
        std::fs::write(target.join("stale.wasm"), []).unwrap();
        std::fs::write(target.join("my_runtime.d.wasm"), []).unwrap();
        assert_eq!(find_wasm_file(&target, &runtime), None);
        let expected = target.join("my_runtime.wasm");
        std::fs::write(&expected, []).unwrap();
        assert_eq!(find_wasm_file(&target, &runtime), Some(expected));

        std::fs::write(
            runtime.join("Cargo.toml"),
            "[package]\nname = 'my-runtime'\n[lib]\nname = 'custom_lib'\n",
        )
        .unwrap();
        assert_eq!(find_wasm_file(&target, &runtime), None);
        let expected = target.join("custom_lib.wasm");
        std::fs::write(&expected, []).unwrap();
        assert_eq!(find_wasm_file(&target, &runtime), Some(expected));
    }
}
