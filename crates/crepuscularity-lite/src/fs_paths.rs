//! Relative path resolution under a sandbox root (no `..`, no absolute paths).

use std::path::{Component, Path, PathBuf};

use crate::bridge::BridgeError;

pub fn resolve_under_sandbox(root: &Path, relative: &str) -> Result<PathBuf, BridgeError> {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let rel_path = Path::new(relative);
    if rel_path.is_absolute() {
        return Err(BridgeError::new(
            "path_not_relative",
            "path must be relative to the sandbox root",
        ));
    }
    let mut out = canonical_root.clone();
    for c in rel_path.components() {
        match c {
            Component::Normal(s) => {
                out.push(s);
                if let Ok(meta) = std::fs::symlink_metadata(&out) {
                    if meta.file_type().is_symlink() {
                        return Err(BridgeError::new(
                            "path_escape",
                            "symlinks are not allowed in sandbox paths",
                        ));
                    }
                    if let Ok(canonical) = out.canonicalize() {
                        if !canonical.starts_with(&canonical_root) {
                            return Err(BridgeError::new(
                                "path_escape",
                                "resolved path left sandbox root",
                            ));
                        }
                    }
                }
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(BridgeError::new(
                    "path_escape",
                    "only normal path segments are allowed (no ..)",
                ));
            }
        }
    }
    if !out.starts_with(&canonical_root) {
        return Err(BridgeError::new(
            "path_escape",
            "resolved path left sandbox root",
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_dir() {
        let root = Path::new("/tmp/sandbox-test-root");
        let err = resolve_under_sandbox(root, "../etc/passwd").err().unwrap();
        assert_eq!(err.code, "path_escape");
    }

    #[test]
    fn rejects_absolute_path() {
        let root = Path::new("/tmp/sandbox");
        #[cfg(not(windows))]
        let abs_path = "/etc/passwd";
        #[cfg(windows)]
        let abs_path = "C:\\etc\\passwd";
        let err = resolve_under_sandbox(root, abs_path).err().unwrap();
        assert_eq!(err.code, "path_not_relative");
    }

    #[test]
    fn handles_current_dir() {
        let root = Path::new("/tmp/sandbox");
        let p = resolve_under_sandbox(root, "./a/./b/c.txt").unwrap();
        assert_eq!(p, PathBuf::from("/tmp/sandbox/a/b/c.txt"));
    }

    #[test]
    fn accepts_nested() {
        let root = Path::new("/tmp/sandbox");
        let p = resolve_under_sandbox(root, "a/b/c.txt").unwrap();
        assert_eq!(p, PathBuf::from("/tmp/sandbox/a/b/c.txt"));
    }

    #[test]
    fn rejects_preexisting_symlink() {
        let root = std::env::temp_dir().join(format!(
            "crepus-lite-sandbox-symlink-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let outside = std::env::temp_dir().join(format!(
            "crepus-lite-sandbox-outside-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(&outside).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&outside, root.join("link")).unwrap();
        assert!(resolve_under_sandbox(&root, "link/file.txt").is_err());
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
