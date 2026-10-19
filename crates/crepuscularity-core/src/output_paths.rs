//! Filesystem output identities for preflight collision checks.
//!
//! Resolves existing symlinks and missing suffixes without creating files. This
//! detects path aliases, not hard links, and does not protect against filesystem
//! changes between validation and a later write.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// Reject output files resolving to the same location or an ancestor of another output.
///
/// Labels identify the conflicting outputs in the error. Existing symlinks are
/// resolved before processing `..`, including links to not-yet-created outputs.
/// Symlink loops and inaccessible ancestors return errors. Distinct files in the
/// same output directory are allowed.
pub fn validate_output_paths<'a>(
    paths: impl IntoIterator<Item = (&'a str, &'a Path)>,
) -> Result<(), String> {
    let mut seen: HashMap<PathBuf, (&str, &Path)> = HashMap::new();
    for (label, path) in paths {
        let identity = resolve_identity(path)
            .map_err(|error| format!("output {label:?} ({}): {error}", path.display()))?;
        if let Some((_, (previous_label, previous_path))) = seen.iter().find(|(previous, _)| {
            identity.starts_with(previous.as_path()) || previous.starts_with(&identity)
        }) {
            return Err(format!(
                "output collision: {previous_label:?} ({}) and {label:?} ({}) overlap as file destinations",
                previous_path.display(), path.display()
            ));
        }
        seen.insert(identity, (label, path));
    }
    Ok(())
}

fn resolve_identity(path: &Path) -> Result<PathBuf, String> {
    resolve_with_links(path, &mut 0)
}

fn resolve_with_links(path: &Path, links: &mut usize) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(path).map_err(|error| error.to_string())?;
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) => {
                resolved.push(component.as_os_str());
                continue;
            }
            Component::RootDir => resolved.push(component.as_os_str()),
            Component::CurDir | Component::ParentDir | Component::Normal(_) => {
                // A file cannot be traversed even when the next component is `..`.
                match std::fs::metadata(&resolved) {
                    Ok(metadata) if !metadata.is_dir() => {
                        return Err(format!(
                            "ancestor {} is not a directory",
                            resolved.display()
                        ));
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.to_string()),
                }
                match component {
                    Component::ParentDir => {
                        resolved.pop();
                    }
                    Component::Normal(name) => resolved.push(name),
                    _ => {}
                }
            }
        }
        match std::fs::symlink_metadata(&resolved) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                *links += 1;
                if *links > 40 {
                    return Err(format!("too many symlinks resolving {}", path.display()));
                }
                let target = std::fs::read_link(&resolved)
                    .map_err(|error| format!("cannot read link {}: {error}", resolved.display()))?;
                let target = if target.is_absolute() {
                    target
                } else {
                    resolved.parent().unwrap_or(Path::new("")).join(target)
                };
                resolved = resolve_with_links(&target, links)?;
            }
            Ok(_) => {
                resolved = resolved
                    .canonicalize()
                    .map_err(|error| format!("cannot resolve {}: {error}", resolved.display()))?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_file_ancestors_collide_in_both_orders() {
        let directory = tempfile::tempdir().unwrap();
        let parent = directory.path().join("missing/result");
        let child = parent.join("view.json");
        for paths in [
            [parent.as_path(), child.as_path()],
            [child.as_path(), parent.as_path()],
        ] {
            let error = validate_output_paths([("one", paths[0]), ("two", paths[1])]).unwrap_err();
            assert!(error.contains("one") && error.contains("two"), "{error}");
        }
        let sibling = directory.path().join("missing/result-other/view.json");
        validate_output_paths([("one", parent.as_path()), ("two", sibling.as_path())]).unwrap();
        assert!(!directory.path().join("missing").exists());
    }

    #[cfg(unix)]
    #[test]
    fn planned_file_ancestors_collide_through_directory_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        let actual = directory.path().join("actual");
        std::fs::create_dir(&actual).unwrap();
        let alias = directory.path().join("alias");
        std::os::unix::fs::symlink(&actual, &alias).unwrap();
        let parent = actual.join("result");
        let child = alias.join("result/view.json");
        for paths in [
            [parent.as_path(), child.as_path()],
            [child.as_path(), parent.as_path()],
        ] {
            assert!(validate_output_paths([("one", paths[0]), ("two", paths[1])]).is_err());
        }
        assert!(!parent.exists());
    }

    #[test]
    fn aliases_collide_without_writing_and_distinct_files_can_share_a_directory() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("sentinel");
        std::fs::write(&file, "unchanged").unwrap();
        let error = validate_output_paths([("first", file.as_path()), ("second", file.as_path())])
            .unwrap_err();
        assert!(error.contains("first") && error.contains("second") && error.contains("sentinel"));
        let alias = directory.path().join("missing/../sentinel");
        assert!(
            validate_output_paths([("first", file.as_path()), ("alias", alias.as_path())]).is_err()
        );
        let other = directory.path().join("missing/other");
        validate_output_paths([("first", file.as_path()), ("other", other.as_path())]).unwrap();
        assert!(!directory.path().join("missing").exists());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "unchanged");
        assert!(resolve_identity(&file.join("../other")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn existing_symlinks_resolve_before_parent_components_and_allow_missing_targets() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let nested = directory.path().join("actual/nested");
        std::fs::create_dir_all(&nested).unwrap();
        symlink(&nested, directory.path().join("alias")).unwrap();
        let via_link = directory.path().join("alias/../result.json");
        let actual = nested
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .join("result.json");
        assert_eq!(resolve_identity(&via_link).unwrap(), actual);
        assert!(validate_output_paths([
            ("link", via_link.as_path()),
            ("actual", actual.as_path())
        ])
        .is_err());
        let lexical = directory.path().join("result.json");
        validate_output_paths([("link", via_link.as_path()), ("lexical", lexical.as_path())])
            .unwrap();
        symlink(
            directory.path().join("absent"),
            directory.path().join("dangling"),
        )
        .unwrap();
        assert_eq!(
            resolve_identity(&directory.path().join("dangling/file")).unwrap(),
            directory.path().canonicalize().unwrap().join("absent/file")
        );
    }

    #[cfg(unix)]
    #[test]
    fn missing_file_link_aliases_collide_and_loops_fail_without_creating_targets() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let alias = directory.path().join("alias.json");
        let target = directory.path().join("target.json");
        symlink("target.json", &alias).unwrap();
        assert!(
            validate_output_paths([("alias", alias.as_path()), ("target", target.as_path())])
                .is_err()
        );
        let other = directory.path().join("other.json");
        validate_output_paths([("alias", alias.as_path()), ("other", other.as_path())]).unwrap();
        assert!(!target.exists());
        symlink("loop-b", directory.path().join("loop-a")).unwrap();
        symlink("loop-a", directory.path().join("loop-b")).unwrap();
        assert!(resolve_identity(&directory.path().join("loop-a"))
            .unwrap_err()
            .contains("too many symlinks"));
    }
}
