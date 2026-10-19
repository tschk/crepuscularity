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
    let git_ref = serde_json::to_string(git_ref).expect("Git ref string serialization cannot fail");
    format!(
        "{pad}{PUB_NAME}:\n{inner}git:\n{inner2}url: {REPO_URL}\n{inner2}ref: {git_ref}\n{inner2}path: {PKG_PATH}\n"
    )
}

fn add(dir: Option<PathBuf>, version: Option<&str>, git_ref: &str) -> Result<(), CrepusCliError> {
    let root = dir.unwrap_or_else(|| PathBuf::from("."));
    let pubspec = root.join("pubspec.yaml");
    let original =
        fs::read_to_string(&pubspec).map_err(|err| CrepusCliError::io(err, pubspec.clone()))?;

    if has_renderer_dependency(&original).map_err(CrepusCliError::context)? {
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

/// Inspect the supported block mapping without treating comments, values, or
/// nested package options as dependency keys. Reject forms we cannot safely edit.
fn has_renderer_dependency(source: &str) -> Result<bool, String> {
    let unsupported = || {
        "unsupported pubspec dependencies mapping; use a top-level `dependencies:` block with two-space package entries".to_string()
    };
    let mut in_dependencies = false;
    let mut found_section = false;
    let mut found_package = false;
    let mut saw_entry = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            in_dependencies = false;
            if let Some((key, value)) = line.split_once(':') {
                if key.trim_matches(['\'', '"']) == "dependencies" {
                    if found_section || key != "dependencies" || !value.trim().is_empty() {
                        return Err(unsupported());
                    }
                    found_section = true;
                    in_dependencies = true;
                }
            }
            continue;
        }
        if !in_dependencies {
            continue;
        }
        let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
        if line.contains('\t') || indentation < 2 || (!saw_entry && indentation != 2) {
            return Err(unsupported());
        }
        if indentation > 2 {
            continue;
        }
        saw_entry = true;
        let (key, _) = trimmed.split_once(':').ok_or_else(unsupported)?;
        let key = key.trim();
        if key == "<<"
            || key.starts_with(['?', '-', '&', '*'])
            || (key.starts_with('"') && key.contains('\\'))
        {
            return Err(unsupported());
        }
        if matches!(
            key,
            "crepuscularity_flutter" | "'crepuscularity_flutter'" | "\"crepuscularity_flutter\""
        ) {
            found_package = true;
        }
    }
    Ok(found_package)
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
        assert!(block.contains("ref: \"abc123\""));
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
        let updated = insert_under_dependencies(source, &block).unwrap();
        let deps_idx = updated.find("dependencies:").unwrap();
        let pkg_idx = updated.find("crepuscularity_flutter:").unwrap();
        let flutter_idx = updated.find("  flutter:").unwrap();
        assert!(deps_idx < pkg_idx && pkg_idx < flutter_idx);
    }

    #[test]
    fn insert_returns_none_without_dependencies_section() {
        assert!(insert_under_dependencies("name: demo\n", "x").is_none());
    }
    #[test]
    fn dependency_detection_ignores_comments_values_nested_and_dev_entries() {
        for source in [
            "# crepuscularity_flutter: later\ndependencies:\n  flutter:\n    sdk: flutter\n",
            "description: 'crepuscularity_flutter: later'\ndependencies:\n  flutter:\n    crepuscularity_flutter: nested\n",
            "dev_dependencies:\n  crepuscularity_flutter: any\ndependencies:\n  flutter:\n    sdk: flutter\n",
            "dependencies:\n  crepuscularity_flutter_extra: any\n",
        ] {
            assert!(!has_renderer_dependency(source).unwrap(), "{source}");
            let updated = insert_under_dependencies(source, &dependency_block(None, "main", 2)).unwrap();
            assert!(has_renderer_dependency(&updated).unwrap());
        }
    }

    #[test]
    fn existing_dependency_keys_include_quoted_spellings() {
        for key in [
            PUB_NAME,
            "'crepuscularity_flutter'",
            "\"crepuscularity_flutter\"",
        ] {
            let source = format!("dependencies:\n  {key}:\n    path: ../local\n");
            assert!(has_renderer_dependency(&source).unwrap());
        }
    }

    #[test]
    fn unsafe_dependency_forms_are_rejected_before_editing() {
        for source in [
            "dependencies: {flutter: any}\n",
            r#"dependencies:
  "\u0063repuscularity_flutter": any
"#,
            "'dependencies':\n  flutter: any\n",
            "dependencies:\n    flutter: any\n",
            "dependencies:\n  <<: *shared\n",
            "dependencies:\n  flutter: any\ndependencies:\n  other: any\n",
        ] {
            assert!(has_renderer_dependency(source).is_err(), "{source}");
        }
    }
    #[test]
    fn add_edits_comment_only_match_once_and_preserves_existing_or_unsafe_files() {
        let dir = tempfile::tempdir().unwrap();
        let pubspec = dir.path().join("pubspec.yaml");
        let original = "name: demo\n# TODO crepuscularity_flutter: later\ndependencies:\n  flutter:\n    sdk: flutter\n";
        fs::write(&pubspec, original).unwrap();
        add(Some(dir.path().into()), None, "main").unwrap();
        let updated = fs::read(&pubspec).unwrap();
        assert_ne!(updated, original.as_bytes());
        assert!(String::from_utf8_lossy(&updated).contains("\n  crepuscularity_flutter:\n"));
        add(Some(dir.path().into()), None, "main").unwrap();
        assert_eq!(fs::read(&pubspec).unwrap(), updated);

        for key in [
            PUB_NAME,
            "'crepuscularity_flutter'",
            "\"crepuscularity_flutter\"",
        ] {
            let original =
                format!("name: demo\r\ndependencies:\r\n  {key}: {{path: ../local}}\r\n");
            fs::write(&pubspec, &original).unwrap();
            add(Some(dir.path().into()), None, "main").unwrap();
            assert_eq!(fs::read(&pubspec).unwrap(), original.as_bytes());
        }
        for unsupported in [
            "dependencies: {flutter: any}\n",
            r#"dependencies:
  "\u0063repuscularity_flutter": any
"#,
        ] {
            fs::write(&pubspec, unsupported).unwrap();
            assert!(add(Some(dir.path().into()), None, "main").is_err());
            assert_eq!(fs::read(&pubspec).unwrap(), unsupported.as_bytes());
        }
    }
    #[test]
    fn dependency_refs_are_quoted_and_preserve_exact_values() {
        for git_ref in [
            "main",
            "#release",
            "true",
            "null",
            "release\"candidate",
            "branch\\name",
            "雪",
            "line\nnext",
        ] {
            let block = dependency_block(None, git_ref, 0);
            let value = block
                .lines()
                .find_map(|line| line.trim().strip_prefix("ref: "))
                .unwrap();
            let decoded: String = serde_json::from_str(value)
                .expect("Git ref must be a quoted YAML-compatible string");
            assert_eq!(decoded, git_ref);
            assert_eq!(block.lines().count(), 5);
        }
    }

    #[test]
    fn add_preserves_special_git_refs_and_is_idempotent() {
        for git_ref in [
            "#release",
            "true",
            "null",
            "release\"candidate",
            "branch\\name",
            "雪",
            "line\nnext",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let pubspec = dir.path().join("pubspec.yaml");
            fs::write(
                &pubspec,
                "name: demo\ndependencies:\n  flutter:\n    sdk: flutter\n",
            )
            .unwrap();
            add(Some(dir.path().into()), None, git_ref).unwrap();
            let updated = fs::read_to_string(&pubspec).unwrap();
            let value = updated
                .lines()
                .find_map(|line| line.trim().strip_prefix("ref: "))
                .unwrap();
            let decoded: String =
                serde_json::from_str(value).expect("Inserted Git ref must retain its string value");
            assert_eq!(decoded, git_ref);
            assert!(updated.contains("  flutter:\n    sdk: flutter\n"));
            add(Some(dir.path().into()), None, "different-ref").unwrap();
            assert_eq!(fs::read_to_string(pubspec).unwrap(), updated);
        }
    }
}
