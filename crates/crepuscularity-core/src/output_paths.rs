//! Filesystem output identities for preflight collision checks.
//!
//! Resolves existing symlinks and missing suffixes without creating files. This
//! compares UTF-8 path components under a conservative portable naming policy:
//! canonical Unicode normalization and default full case folding (Unicode 16.0
//! folding tables). Names distinct on a case-sensitive filesystem can therefore
//! collide here. This does not model every filesystem equivalence, detect hard
//! links, or protect against filesystem changes before a later write.

use caseless::Caseless;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use unicode_normalization::UnicodeNormalization;

/// Reject output files resolving to the same location or an ancestor of another output.
///
/// Labels identify the conflicting outputs in the error. Existing symlinks are
/// resolved before processing `..`, including links to not-yet-created outputs.
/// Symlink loops and inaccessible ancestors return errors. Distinct files in the
/// same output directory are allowed when their portable names are distinct.
/// Non-UTF-8 resolved path components return an error before any output writes.
pub fn validate_output_paths<'a>(
    paths: impl IntoIterator<Item = (&'a str, &'a Path)>,
) -> Result<(), String> {
    let mut seen: HashMap<Vec<String>, (&str, &Path)> = HashMap::new();
    for (label, path) in paths {
        let identity = resolve_identity(path)
            .and_then(|resolved| portable_identity(&resolved))
            .map_err(|error| format!("output {label:?} ({}): {error}", path.display()))?;
        if let Some((_, (previous_label, previous_path))) = seen
            .iter()
            .find(|(previous, _)| identity.starts_with(previous) || previous.starts_with(&identity))
        {
            return Err(format!(
                "output collision: {previous_label:?} ({}) and {label:?} ({}) overlap as file destinations",
                previous_path.display(), path.display()
            ));
        }
        seen.insert(identity, (label, path));
    }
    Ok(())
}

// Keep boundaries as components: folding a name never creates a path separator.
// NFD before and after full folding implements canonical caseless comparison.
fn portable_identity(path: &Path) -> Result<Vec<String>, String> {
    path.components()
        .map(|component| {
            let name = component
                .as_os_str()
                .to_str()
                .ok_or_else(|| "portable output names must be valid UTF-8".to_owned())?;
            Ok(name.nfd().default_case_fold().nfd().collect())
        })
        .collect()
}

fn resolve_identity(path: &Path) -> Result<PathBuf, String> {
    resolve_with_links(path, &mut 0)
}

fn resolve_with_links(path: &Path, links: &mut usize) -> Result<PathBuf, String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    resolve_from(path, links, &cwd)
}

fn resolve_from(path: &Path, links: &mut usize, base: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("empty output path".into());
    }
    // Windows absolute() normalizes `..`; a verbatim PathBuf can do so on push too.
    // Resolve only the starting prefix, then walk the original components ourselves.
    let mut components = path.components().peekable();
    let mut resolved = match components.peek().copied() {
        Some(Component::Prefix(prefix)) => {
            components.next();
            let mut root = PathBuf::from(prefix.as_os_str());
            if matches!(components.peek(), Some(Component::RootDir)) {
                root.push(components.next().unwrap().as_os_str());
                root
            } else {
                // Drive-relative C:foo uses that drive's native current directory.
                std::path::absolute(root).map_err(|error| error.to_string())?
            }
        }
        Some(Component::RootDir) => {
            let root = components.next().unwrap();
            let mut start = PathBuf::new();
            if let Some(Component::Prefix(prefix)) = base.components().next() {
                start.push(prefix.as_os_str());
            }
            start.push(root.as_os_str());
            start
        }
        _ => base.to_path_buf(),
    };
    // The seed has already been resolved independently of the input tail. Normalize
    // its native spelling (notably Windows verbatim prefixes) even if the next
    // component is missing, or there are no remaining components at all.
    match resolved.canonicalize() {
        Ok(canonical) => resolved = canonical,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    for component in components {
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
                // Do not join a relative target containing `..` onto a verbatim
                // parent: that would normalize before ancestor validation as well.
                resolved = resolve_from(&target, links, resolved.parent().unwrap_or(base))?;
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
    fn portable_names_collide_without_creating_missing_outputs() {
        let directory = tempfile::tempdir().unwrap();
        for (first, second) in [
            ("missing/Output.json", "missing/output.json"),
            ("missing/Éclair.json", "missing/E\u{301}clair.json"),
            ("missing/Straße.json", "missing/STRASSE.json"),
            ("missing/\u{1c90}.json", "missing/\u{10d0}.json"),
        ] {
            let first = directory.path().join(first);
            let second = directory.path().join(second);
            for paths in [[&first, &second], [&second, &first]] {
                let error = validate_output_paths([
                    ("first", paths[0].as_path()),
                    ("second", paths[1].as_path()),
                ])
                .unwrap_err();
                assert!(
                    error.contains("first") && error.contains("second"),
                    "{error}"
                );
            }
        }
        validate_output_paths([
            ("one", directory.path().join("missing/one.json").as_path()),
            ("two", directory.path().join("missing/two.json").as_path()),
        ])
        .unwrap();
        assert!(!directory.path().join("missing").exists());
    }

    #[test]
    fn portable_file_ancestors_collide_in_both_orders() {
        let directory = tempfile::tempdir().unwrap();
        for (parent, child) in [
            ("missing/Output", "missing/output/child.json"),
            ("missing/Éclair", "missing/E\u{301}clair/child.json"),
            ("missing/Straße", "missing/STRASSE/child.json"),
        ] {
            let parent = directory.path().join(parent);
            let child = directory.path().join(child);
            for paths in [[&parent, &child], [&child, &parent]] {
                assert!(validate_output_paths([
                    ("parent", paths[0].as_path()),
                    ("child", paths[1].as_path()),
                ])
                .is_err());
            }
        }
        validate_output_paths([
            ("one", directory.path().join("missing/Output").as_path()),
            (
                "two",
                directory
                    .path()
                    .join("missing/output-other/child.json")
                    .as_path(),
            ),
        ])
        .unwrap();
        assert!(!directory.path().join("missing").exists());
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_portable_output_names_fail_without_writes() {
        use std::os::unix::ffi::OsStringExt;
        let directory = tempfile::tempdir().unwrap();
        let output = directory
            .path()
            .join(std::ffi::OsString::from_vec(vec![0xff]));
        let error = validate_output_paths([("invalid", output.as_path())]).unwrap_err();
        assert!(error.contains("UTF-8"), "{error}");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[cfg(windows)]
    #[test]
    fn windows_path_shapes_preserve_file_before_parent_validation() {
        let cwd = std::env::current_dir().unwrap();
        let directory = tempfile::tempdir_in(&cwd).unwrap();
        let relative = directory.path().strip_prefix(&cwd).unwrap();
        let file = directory.path().join("sentinel");
        std::fs::write(&file, "unchanged").unwrap();
        let rooted_file: PathBuf = file.components().skip(1).collect();
        let Some(Component::Prefix(prefix)) = cwd.components().next() else {
            panic!("Windows current directory must have a prefix");
        };
        let mut inputs = vec![
            PathBuf::from(format!("{}\\sentinel\\..\\other", relative.display())),
            PathBuf::from(format!("{}\\..\\other", file.display())),
            PathBuf::from(format!("{}\\..\\other", rooted_file.display())),
        ];
        if let std::path::Prefix::Disk(drive) | std::path::Prefix::VerbatimDisk(drive) =
            prefix.kind()
        {
            inputs.push(PathBuf::from(format!(
                "{}:{}\\sentinel\\..\\other",
                char::from(drive),
                relative.display()
            )));
        }
        for input in inputs {
            let error = resolve_identity(&input).unwrap_err();
            assert!(
                error.contains("not a directory"),
                "{}: {error}",
                input.display()
            );
        }
        let expected = directory.path().canonicalize().unwrap().join("other");
        let relative_other = relative.join("other");
        assert_eq!(resolve_identity(&relative_other).unwrap(), expected);
        assert_eq!(std::fs::read_to_string(file).unwrap(), "unchanged");
    }

    #[cfg(windows)]
    #[test]
    fn windows_missing_cwd_child_and_bare_cwd_share_canonical_identity() {
        let cwd = std::env::current_dir().unwrap();
        let directory = tempfile::tempdir_in(&cwd).unwrap();
        let missing_name = format!(
            "{}-missing",
            directory.path().file_name().unwrap().to_string_lossy()
        );
        let relative = PathBuf::from(missing_name);
        let absolute = cwd.join(&relative);
        assert!(!absolute.exists());
        assert_eq!(
            resolve_identity(&relative).unwrap(),
            resolve_identity(&absolute).unwrap()
        );
        assert!(validate_output_paths([
            ("relative", relative.as_path()),
            ("absolute", absolute.as_path())
        ])
        .is_err());
        assert_eq!(
            resolve_identity(Path::new(".")).unwrap(),
            cwd.canonicalize().unwrap()
        );
        assert_eq!(resolve_identity(&cwd).unwrap(), cwd.canonicalize().unwrap());
        assert!(!absolute.exists());
    }

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
