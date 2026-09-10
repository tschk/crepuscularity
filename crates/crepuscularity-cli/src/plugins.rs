//! `crepus plugins` — plugin manifest tests and ABI bindgen (equilibrium-ffi).

use std::path::PathBuf;
use std::process::Command;

use console::style;
use crepuscularity_plugin_bindgen::{
    default_manifest_path, generate_all, BindgenOptions, GeneratedFile,
};

use crate::cli::PluginsCommands;
use crate::ui;

fn repo_root() -> PathBuf {
    std::env::var("CREPUS_REPO_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        })
}

pub fn execute(command: PluginsCommands) {
    match command {
        PluginsCommands::Bindgen {
            manifest,
            abi_header,
            out_dir,
        } => run_bindgen(manifest, abi_header, out_dir),
        PluginsCommands::Test { manifest } => run_test(manifest),
    }
}

fn run_bindgen(manifest: Option<PathBuf>, abi_header: Option<PathBuf>, out_dir: Option<PathBuf>) {
    let root = repo_root();
    let manifest_path = manifest.unwrap_or_else(|| default_manifest_path(&root));
    let opts = BindgenOptions {
        repo_root: root,
        manifest_path,
        abi_header,
        out_dir,
    };

    match generate_all(&opts) {
        Ok(files) => {
            ui::success(&format!(
                "wrote {} binding file(s) (equilibrium-ffi)",
                files.len()
            ));
            for GeneratedFile { language, path } in files {
                println!(
                    "  {} {} {}",
                    style("✓").green(),
                    style(&language).cyan(),
                    path.display()
                );
            }
        }
        Err(e) => ui::error(&format!("bindgen failed: {e}")),
    }
}

fn run_test(manifest: Option<PathBuf>) {
    let root = repo_root();
    let manifest_path = manifest.unwrap_or_else(|| default_manifest_path(&root));
    let raw = match std::fs::read_to_string(&manifest_path) {
        Ok(s) => s,
        Err(e) => ui::error(&format!("read {}: {e}", manifest_path.display())),
    };

    #[derive(serde::Deserialize)]
    struct M {
        package: Vec<Package>,
    }
    #[derive(serde::Deserialize)]
    struct Package {
        language: String,
        test: String,
    }

    let m: M = match toml::from_str(&raw) {
        Ok(m) => m,
        Err(e) => ui::error(&format!("parse manifest: {e}")),
    };

    let mut failed = false;
    for pkg in &m.package {
        print!("  {} {} … ", style("→").cyan(), pkg.language);

        let args = match shlex::split(&pkg.test) {
            Some(a) if !a.is_empty() => a,
            _ => {
                println!("{}", style("invalid command string").red());
                failed = true;
                continue;
            }
        };

        let bin = &args[0];
        let allowed = [
            "cargo", "go", "bun", "deno", "node", "zig", "swift", "dotnet", "python", "python3",
            "ruby", "php", "javac", "java", "kotlinc", "kotlin", "v", "gcc", "g++", "clang",
            "clang++", "rustc", "test",
        ];

        if !allowed.contains(&bin.as_str()) {
            println!(
                "{}",
                style(format!(
                    "security error: command '{bin}' is not in the allowlist"
                ))
                .red()
            );
            failed = true;
            continue;
        }

        let status = Command::new(bin)
            .args(&args[1..])
            .current_dir(&root)
            .status();

        match status {
            Ok(s) if s.success() => println!("{}", style("ok").green()),
            Ok(s) => {
                println!("{}", style(format!("exit {s}")).red());
                failed = true;
            }
            Err(e) => {
                println!("{}", style(format!("{e}")).red());
                failed = true;
            }
        }
    }

    if failed {
        ui::error("one or more plugin tests failed");
    }
    ui::success("all plugin tests passed");
}
