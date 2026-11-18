//! Shared helpers for `wasm32-unknown-unknown` + wasm-bindgen (web + webext).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::build_options::{BuildOptions, OptimizationLevel};

/// Locate the first `*.wasm` artifact in a release directory (ignores `.d.wasm`).
pub fn find_wasm_file(dir: &Path) -> Option<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return None;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().map(|e| e == "wasm").unwrap_or(false)
            && !p
                .file_name()
                .map(|n| n.to_string_lossy().ends_with(".d.wasm"))
                .unwrap_or(false)
        {
            return Some(p);
        }
    }
    None
}

pub fn wasm_profile_dirs(
    app_path: &Path,
    runtime_dir: &Path,
    options: BuildOptions,
) -> (PathBuf, PathBuf) {
    let profile = options.cargo_profile();
    let workspace_target = app_path.join("target/wasm32-unknown-unknown").join(profile);
    let local_target = runtime_dir
        .join("target/wasm32-unknown-unknown")
        .join(profile);
    (workspace_target, local_target)
}

/// `~/.cargo/bin` when present — must be **before** Homebrew `/opt/homebrew/bin` on `PATH` so
/// nested builds invoke rustup's `rustc` (with wasm std), not a standalone `rustc`.
fn rustup_bin_dir() -> Option<PathBuf> {
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
    if let Some(home) = std::env::var_os("HOME") {
        let p = Path::new(&home).join(".cargo/bin/cargo");
        if p.is_file() {
            return p.into_os_string();
        }
    }
    #[cfg(windows)]
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let p = Path::new(&profile).join(".cargo/bin/cargo.exe");
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
/// without wasm std). Uses `CARGO` if set, else `~/.cargo/bin/cargo` when present.
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

pub fn run_wasm_bindgen(wasm_path: &Path, out_dir: &Path, out_name: &str) -> Result<(), String> {
    let out_dir_str = out_dir.to_string_lossy();
    let wasm = wasm_path.to_str().ok_or("wasm path utf-8")?;
    let mut cmd = Command::new("wasm-bindgen");
    prepend_rustup_bin_to_path(&mut cmd);
    let out = cmd
        .args([
            "--target",
            "web",
            "--out-dir",
            out_dir_str.as_ref(),
            "--out-name",
            out_name,
            wasm,
        ])
        .output()
        .map_err(|e| format!("wasm-bindgen: {e}"))?;
    if out.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    // version mismatch: "rust Wasm file schema version: 0.2.123"
    if let Some(ver) = parse_wasm_bindgen_needed_version(&stderr) {
        let spinner = crate::ui::spinner(&format!("installing wasm-bindgen-cli {ver}"));
        let install = Command::new("cargo")
            .args(["install", "wasm-bindgen-cli", "--version", &ver, "--locked"])
            .output()
            .map_err(|e| format!("cargo install wasm-bindgen-cli {ver}: {e}"))?;
        if !install.status.success() {
            crate::ui::spinner_err(
                &spinner,
                &format!(
                    "could not install wasm-bindgen-cli {ver}: {}",
                    String::from_utf8_lossy(&install.stderr)
                ),
            );
        }
        crate::ui::spinner_ok(&spinner, &format!("wasm-bindgen-cli {ver} installed"));
        // retry
        let mut cmd2 = Command::new("wasm-bindgen");
        prepend_rustup_bin_to_path(&mut cmd2);
        let out2 = cmd2
            .args([
                "--target",
                "web",
                "--out-dir",
                out_dir_str.as_ref(),
                "--out-name",
                out_name,
                wasm,
            ])
            .output()
            .map_err(|e| format!("wasm-bindgen (retry): {e}"))?;
        if out2.status.success() {
            return Ok(());
        }
        return Err(String::from_utf8_lossy(&out2.stderr).into_owned());
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
