//! Watch registration shared by the terminal and desktop dev loops.
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use notify::{RecursiveMode, Watcher};

pub(crate) fn watch_project(watcher: &mut impl Watcher, cwd: &Path) {
    let src = cwd.join("src");
    if src.exists() {
        register(watcher, &src, RecursiveMode::Recursive);
    }
    register(
        watcher,
        &cwd.join("Cargo.toml"),
        RecursiveMode::NonRecursive,
    );
    // Parse first: an incomplete manifest edit must not remove established watches.
    let Some(paths) = declared_files(cwd) else {
        return;
    };
    for path in paths {
        if path.is_file() {
            register(watcher, &path, RecursiveMode::NonRecursive);
        }
    }
}

fn register(watcher: &mut impl Watcher, path: &Path, mode: RecursiveMode) {
    if let Err(error) = watcher.watch(path, mode) {
        eprintln!("[crepus] could not watch {}: {error}", path.display());
    }
}

fn declared_files(cwd: &Path) -> Option<HashSet<PathBuf>> {
    let source = std::fs::read_to_string(cwd.join("Cargo.toml")).ok()?;
    let manifest: toml::Value = toml::from_str(&source).ok()?;
    let mut paths = HashSet::new();
    if let Some(bins) = manifest.get("bin").and_then(toml::Value::as_array) {
        for bin in bins {
            if let Some(path) = bin.get("path").and_then(toml::Value::as_str) {
                paths.insert(cwd.join(path));
            }
        }
    }
    if let Some(path) = manifest
        .get("lib")
        .and_then(|lib| lib.get("path"))
        .and_then(toml::Value::as_str)
    {
        paths.insert(cwd.join(path));
    }
    match manifest
        .get("package")
        .and_then(|package| package.get("build"))
    {
        Some(toml::Value::Boolean(false)) => {}
        Some(toml::Value::String(path)) => {
            paths.insert(cwd.join(path));
        }
        _ => {
            paths.insert(cwd.join("build.rs"));
        }
    }
    Some(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_paths_respect_disabled_build_and_refresh_manifest_edits() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("Cargo.toml");
        std::fs::write(&manifest, "[package]\nname='host'\nbuild=false\n[[bin]]\npath='app/main.rs'\n[lib]\npath='lib/root.rs'\n").unwrap();
        let paths = declared_files(root.path()).unwrap();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&root.path().join("app/main.rs")));
        assert!(paths.contains(&root.path().join("lib/root.rs")));
        assert!(!paths.contains(&root.path().join("build.rs")));
        std::fs::write(
            &manifest,
            "[package]\nname='host'\nbuild='tools/build.rs'\n",
        )
        .unwrap();
        assert_eq!(
            declared_files(root.path()).unwrap(),
            HashSet::from([root.path().join("tools/build.rs")])
        );
        std::fs::write(&manifest, "[package\n").unwrap();
        assert!(declared_files(root.path()).is_none());
    }
}
