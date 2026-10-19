//! `crepus flutter` — dependency helper for the **`crepuscularity-flutter`**
//! Flutter runtime renderer package (`plugins/crepuscularity-flutter`).
//!
//! Consuming apps / CI call this instead of hardcoding a dependency spec, so the
//! renderer can be pinned to a git ref today and swapped to a published pub.dev
//! version later without the consumer changing shape.
//!
//! - `crepus flutter dep` prints the pubspec dependency block to stdout.
//! - `crepus flutter add` inserts that block into a Flutter app's `pubspec.yaml`.

use std::fs;
use std::path::PathBuf;

use crate::cli::FlutterCommands;
use crate::error::CrepusCliError;

// The pub package name uses an underscore (pub.dev forbids hyphens); the repo
// path stays hyphenated to match the sibling crates.
const PUB_NAME: &str = "crepuscularity_flutter";
const REPO_URL: &str = "https://github.com/tschk/crepuscularity";
const PKG_PATH: &str = "plugins/crepuscularity-flutter";

pub fn execute(cmd: FlutterCommands) -> Result<(), CrepusCliError> {
    match cmd {
        FlutterCommands::Dep { version, git_ref } => {
            print!("{}", dependency_block(version.as_deref(), &git_ref, 0));
            Ok(())
        }
        FlutterCommands::Add {
            dir,
            version,
            git_ref,
        } => add(dir, version.as_deref(), &git_ref),
    }
}

/// The pubspec dependency block, indented by `base` spaces (0 for stdout, 2 for
/// insertion under `dependencies:`).
fn dependency_block(version: Option<&str>, git_ref: &str, base: usize) -> String {
    let pad = " ".repeat(base);
    if let Some(version) = version {
        return format!("{pad}{PUB_NAME}: ^{version}\n");
    }
    let inner = " ".repeat(base + 2);
    let inner2 = " ".repeat(base + 4);
    format!(
        "{pad}{PUB_NAME}:\n{inner}git:\n{inner2}url: {REPO_URL}\n{inner2}ref: {git_ref}\n{inner2}path: {PKG_PATH}\n"
    )
}

fn add(dir: Option<PathBuf>, version: Option<&str>, git_ref: &str) -> Result<(), CrepusCliError> {
    let root = dir.unwrap_or_else(|| PathBuf::from("."));
    let pubspec = root.join("pubspec.yaml");
    let original =
        fs::read_to_string(&pubspec).map_err(|err| CrepusCliError::io(err, pubspec.clone()))?;

    if original.contains(&format!("{PUB_NAME}:")) {
        println!(
            "{PUB_NAME} is already a dependency in {} — leaving it unchanged.",
            pubspec.display()
        );
        return Ok(());
    }

    let block = dependency_block(version, git_ref, 2);
    let updated = insert_under_dependencies(&original, &block).ok_or_else(|| {
        CrepusCliError::context(format!(
            "no top-level `dependencies:` section found in {}",
            pubspec.display()
        ))
    })?;
    fs::write(&pubspec, updated).map_err(|err| CrepusCliError::io(err, pubspec.clone()))?;
    println!("Added {PUB_NAME} to {}.", pubspec.display());
    Ok(())
}

/// Insert `block` immediately after the top-level `dependencies:` line.
fn insert_under_dependencies(source: &str, block: &str) -> Option<String> {
    let mut out = String::with_capacity(source.len() + block.len());
    let mut inserted = false;
    for line in source.lines() {
        out.push_str(line);
        out.push('\n');
        if !inserted && line.trim_end() == "dependencies:" {
            out.push_str(block);
            inserted = true;
        }
    }
    inserted.then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_block_pins_ref_and_path() {
        let block = dependency_block(None, "abc123", 0);
        assert!(block.contains("crepuscularity_flutter:"));
        assert!(block.contains("url: https://github.com/tschk/crepuscularity"));
        assert!(block.contains("ref: abc123"));
        assert!(block.contains("path: plugins/crepuscularity-flutter"));
    }

    #[test]
    fn published_block_uses_caret_version() {
        let block = dependency_block(Some("0.1.0"), "main", 0);
        assert_eq!(block, "crepuscularity_flutter: ^0.1.0\n");
    }

    #[test]
    fn insert_places_block_under_dependencies() {
        let source = "name: demo\ndependencies:\n  flutter:\n    sdk: flutter\n";
        let block = dependency_block(None, "main", 2);
        let updated = insert_under_dependencies(source, &block).expect("failed to insert block");
        let deps_idx = updated.find("dependencies:").expect("missing dependencies");
        let pkg_idx = updated
            .find("crepuscularity_flutter:")
            .expect("missing package");
        let flutter_idx = updated.find("  flutter:").expect("missing flutter sdk");
        assert!(deps_idx < pkg_idx && pkg_idx < flutter_idx);
    }

    #[test]
    fn insert_returns_none_without_dependencies_section() {
        assert!(insert_under_dependencies("name: demo\n", "x").is_none());
    }
}
