//! Safe resolution of `include` paths relative to a template `base_dir`.

use std::path::{Path, PathBuf};

use crate::error::CrepusError;

/// Resolve an `include` path, rejecting escapes outside `base_dir`.
///
/// On native targets, existing paths are canonicalized and must stay under the
/// canonical base directory when one is set. Missing paths return a lexical join
/// under the canonical base (virtual bundles and not-yet-written files).
/// On `wasm32`, only syntactic checks run.
pub fn resolve_include_path(base_dir: Option<&Path>, path: &str) -> Result<PathBuf, CrepusError> {
    let requested = Path::new(path);
    if requested.has_root()
        || requested.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(CrepusError::include_path(format!(
            "include path outside base dir: {path}"
        )));
    }

    let candidate = if let Some(base) = base_dir {
        base.join(requested)
    } else {
        requested.to_path_buf()
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(base) = base_dir {
            match std::fs::canonicalize(base) {
                Ok(base) => {
                    let joined = base.join(requested);
                    if !joined.starts_with(&base) {
                        return Err(CrepusError::include_path(format!(
                            "include path outside base dir: {path}"
                        )));
                    }
                    if joined.exists() {
                        let resolved = std::fs::canonicalize(&joined).map_err(|e| {
                            CrepusError::include_path(format!(
                                "failed to resolve include path {path:?}: {e}"
                            ))
                        })?;
                        if !resolved.starts_with(&base) {
                            return Err(CrepusError::include_path(format!(
                                "include path outside base dir: {path}"
                            )));
                        }
                        Ok(resolved)
                    } else {
                        Ok(joined)
                    }
                }
                Err(_) => {
                    // Synthetic bases (virtual bundles, tests) have no on-disk directory.
                    let joined = base.join(requested);
                    if !joined.starts_with(base) {
                        return Err(CrepusError::include_path(format!(
                            "include path outside base dir: {path}"
                        )));
                    }
                    Ok(joined)
                }
            }
        } else if candidate.exists() {
            std::fs::canonicalize(&candidate).map_err(|e| {
                CrepusError::include_path(format!("failed to resolve include path {path:?}: {e}"))
            })
        } else {
            Ok(candidate)
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        Ok(candidate)
    }
}

/// Parser/cache identity for an include, retaining its authored filename.
///
/// Canonical resolution can change a symlink's extension. Use this path only for
/// frontend selection; reads, admission checks and child bases use `resolved`.
pub fn include_frontend_path(resolved: &Path, authored: &str) -> PathBuf {
    match Path::new(authored).file_name() {
        Some(name) => resolved.with_file_name(name),
        None => resolved.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_include_path;

    #[test]
    fn frontend_identity_preserves_authored_basename_and_resolved_parent() {
        let resolved = std::path::Path::new("canonical/target.txt");
        for name in [
            "view.vue",
            "view.jsx",
            "view.component.html",
            "view.ng.html",
        ] {
            assert_eq!(
                super::include_frontend_path(resolved, &format!("alias/{name}")),
                std::path::Path::new("canonical").join(name)
            );
        }
    }

    #[test]
    fn include_path_rejects_parent_dir() {
        let err = resolve_include_path(None, "../secret.crepus").unwrap_err();
        assert!(err.to_string().contains("include path outside base dir"));
    }

    #[test]
    fn include_path_rejects_absolute_path() {
        let path = std::env::temp_dir().join("secret.crepus");
        let err = resolve_include_path(None, path.to_str().unwrap()).unwrap_err();
        assert!(err.to_string().contains("include path outside base dir"));
    }

    #[test]
    fn include_path_resolves_with_base_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        std::fs::write(base.join("foo.crepus"), "div").expect("write fixture");
        let res = resolve_include_path(Some(base), "foo.crepus").unwrap();
        let expected = std::fs::canonicalize(base.join("foo.crepus")).expect("canonicalize");
        assert_eq!(res, expected);
    }

    #[test]
    fn include_path_resolves_without_base_dir() {
        let res = resolve_include_path(None, "foo.crepus").unwrap();
        assert_eq!(res, std::path::PathBuf::from("foo.crepus"));
    }

    #[test]
    fn include_path_resolves_nested() {
        let res = resolve_include_path(None, "foo/bar.crepus").unwrap();
        assert_eq!(res, std::path::PathBuf::from("foo/bar.crepus"));
    }

    #[test]
    fn include_path_rejects_nested_parent_dir() {
        let err = resolve_include_path(None, "foo/../bar.crepus").unwrap_err();
        assert!(err.to_string().contains("include path outside base dir"));
    }

    #[cfg(unix)]
    #[test]
    fn include_path_blocks_symlink_escape_when_file_exists() {
        use std::os::unix::fs::symlink;

        let site = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join("secret.crepus"), "div").expect("write secret");
        symlink(outside.path(), site.path().join("escape")).expect("symlink");

        let err = resolve_include_path(Some(site.path()), "escape/secret.crepus").unwrap_err();
        assert!(err.to_string().contains("include path outside base dir"));
    }

    #[test]
    fn include_path_resolves_missing_file_with_base_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path();
        let res = resolve_include_path(Some(base), "missing.crepus").unwrap();
        let expected = std::fs::canonicalize(base)
            .expect("canonicalize")
            .join("missing.crepus");
        assert_eq!(res, expected);
    }

    #[test]
    fn include_path_resolves_with_synthetic_base_dir() {
        let dir = std::env::temp_dir().join("crepus-synthetic-dir-doesnotexist-12345");
        let res = resolve_include_path(Some(&dir), "virtual.crepus").unwrap();
        assert_eq!(res, dir.join("virtual.crepus"));
    }

    #[test]
    fn include_path_resolves_without_base_dir_but_file_exists() {
        let res = resolve_include_path(None, "src/lib.rs").unwrap();
        let expected = std::fs::canonicalize("src/lib.rs").expect("canonicalize");
        assert_eq!(res, expected);
    }
}
