use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn compile_crepus(path: impl AsRef<Path>) -> Result<(), String> {
    let root = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").map_err(|e| format!("CARGO_MANIFEST_DIR: {e}"))?,
    );
    let path = root.join(path.as_ref());
    compile_path(&path)
}

fn compile_path(path: &Path) -> Result<(), String> {
    compile_path_inner(path, &mut HashSet::new())
}

fn compile_path_inner(path: &Path, ancestors: &mut HashSet<PathBuf>) -> Result<(), String> {
    println!("cargo:rerun-if-changed={}", path.display());

    let metadata =
        fs::metadata(path).map_err(|e| format!("could not stat {}: {e}", path.display()))?;

    if metadata.is_dir() {
        let canonical = fs::canonicalize(path)
            .map_err(|e| format!("could not resolve directory {}: {e}", path.display()))?;
        // Preserve sibling aliases and their rebuild paths; only active ancestors cycle.
        if !ancestors.insert(canonical.clone()) {
            return Ok(());
        }
        let result = (|| {
            let mut entries = fs::read_dir(path)
                .map_err(|e| format!("could not read directory {}: {e}", path.display()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| {
                    format!("could not read directory entry in {}: {e}", path.display())
                })?;
            entries.sort_by_key(|entry| entry.path());
            for entry in entries {
                compile_path_inner(&entry.path(), ancestors)?;
            }
            Ok(())
        })();
        ancestors.remove(&canonical);
        return result;
    }

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        extension.as_str(),
        "crepus" | "csx" | "jsx" | "tsx" | "vue" | "svelte" | "astro" | "ng"
    ) || crate::parser::is_angular_mode(Some(path))
    {
        let source = fs::read_to_string(path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        if !crate::parser::is_markup_mode(&source, Some(path)) {
            let file = crate::parse_component_file(&source)
                .map_err(|e| format!("could not parse {}: {e}", path.display()))?;
            if !file.components.is_empty() {
                return Ok(());
            }
        }
        crate::parse_template_with_path(&source, Some(path))
            .map_err(|e| format!("could not parse {}: {e}", path.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_single_template_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("view.crepus");
        fs::write(&file, "div\n  \"hello\"").unwrap();
        compile_path(&file).unwrap();
    }

    #[test]
    fn accepts_nested_template_directory() {
        let dir = tempfile::tempdir().unwrap();
        let views = dir.path().join("views");
        fs::create_dir_all(views.join("nested")).unwrap();
        fs::write(views.join("app.crepus"), "div #app\n  \"hello\"").unwrap();
        fs::write(views.join("nested").join("card.crepus"), "div\n  \"card\"").unwrap();
        compile_path(&views).unwrap();
    }

    #[test]
    fn rejects_invalid_template_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("bad.crepus");
        fs::write(&file, "<div").unwrap();
        let err = compile_path(&file).unwrap_err();
        assert!(err.contains("bad.crepus"));
    }

    #[test]
    fn validates_all_markup_frontends() {
        let dir = tempfile::tempdir().unwrap();
        for (extension, valid, invalid) in [
            (
                "vue",
                "<template><div /></template>",
                "<template><div></template>",
            ),
            ("svelte", "<div />", "<div"),
            ("astro", "<div />", "<div"),
            ("ng", "<div></div>", "<div"),
            ("ng.html", "<div></div>", "<div"),
            ("component.html", "<div></div>", "<div"),
            (
                "VUE",
                "<template><div /></template>",
                "<template><div></template>",
            ),
            ("SVELTE", "<div />", "<div"),
            ("ASTRO", "<div />", "<div"),
            ("JSX", "<div />", "<div"),
            ("CSX", "<div />", "<div"),
            ("TSX", "<div />", "<div"),
            ("NG", "<div></div>", "<div"),
            ("NG.HTML", "<div></div>", "<div"),
            ("COMPONENT.HTML", "<div></div>", "<div"),
        ] {
            let file = dir.path().join(format!("view.{extension}"));
            fs::write(&file, valid).unwrap();
            compile_path(&file).unwrap();
            fs::write(&file, invalid).unwrap();
            assert!(compile_path(&file).is_err(), "skipped {extension}");
        }
    }

    #[test]
    fn markup_literal_component_markers_keep_the_selected_frontend() {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["vue", "svelte", "astro", "jsx", "ng.html", "crepus"] {
            let body = "<pre>before\n--- Card\nafter</pre>";
            let source = if extension == "vue" {
                format!("<template>{body}</template>")
            } else {
                body.to_owned()
            };
            let path = dir.path().join(format!("literal.{extension}"));
            crate::parse_template_with_path(&source, Some(&path)).unwrap();
            fs::write(&path, source).unwrap();
            compile_path(&path).unwrap_or_else(|error| panic!("{extension}: {error}"));
        }
    }

    #[test]
    fn component_marker_cannot_bypass_markup_validation() {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["vue", "svelte", "astro", "jsx", "ng.html", "crepus"] {
            let source = if extension == "vue" {
                "<template><div>\n--- Card\nspan"
            } else {
                "<div>\n--- Card\nspan"
            };
            let path = dir.path().join(format!("invalid.{extension}"));
            assert!(crate::parse_template_with_path(source, Some(&path)).is_err());
            fs::write(&path, source).unwrap();
            let error =
                compile_path(&path).expect_err("component marker bypassed markup validation");
            assert!(error.contains(&format!("invalid.{extension}")), "{error}");
        }
    }

    #[test]
    fn preserves_component_file_errors() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("components.crepus");
        for (source, message) in [
            (
                "+++\n[Card]\ndescription = 42\n+++\n--- Card\nspan",
                "description must be",
            ),
            (
                "--- Card\nspan\n--- Card\nbutton",
                "duplicate component name",
            ),
            (
                "+++\n[Card]\n--- Card\nspan",
                "unclosed component frontmatter",
            ),
        ] {
            fs::write(&file, source).unwrap();
            let error = compile_path(&file).unwrap_err();
            assert!(error.contains(message), "{error}");
            assert!(error.contains("components.crepus"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn directory_symlink_cycles_do_not_repeat_ancestor_traversal() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("view.crepus"), "span \"VALID\"").unwrap();
        symlink(dir.path(), dir.path().join("nested/ancestor")).unwrap();
        compile_path(dir.path()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn directory_cycle_does_not_hide_invalid_siblings() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        // Visit the cycle first: a broken traversal reports ELOOP before the real error.
        symlink(dir.path(), dir.path().join("a-cycle")).unwrap();
        fs::write(dir.path().join("z-invalid.crepus"), "<div").unwrap();
        let error = compile_path(dir.path()).unwrap_err();
        assert!(error.contains("z-invalid.crepus"), "{error}");
        assert!(error.contains("could not parse"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn directory_traversal_preserves_file_alias_frontend_identity() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        // Vue extracts this script; treating the alias as JSX would reject its '<'.
        let source = "<script>const less = 1 < 2;</script><template><p>Vue</p></template>";
        fs::write(dir.path().join("source.txt"), source).unwrap();
        let alias = dir.path().join("view.vue");
        symlink("source.txt", &alias).unwrap();
        crate::parse_template_with_path(source, Some(&alias)).unwrap();
        compile_path(dir.path()).unwrap();
    }

    #[test]
    fn compile_crepus_success() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("view.crepus");
        fs::write(&file, "div\n  \"hello\"").unwrap();
        // PathBuf::join with an absolute path replaces the base path,
        // so this will ignore CARGO_MANIFEST_DIR and resolve correctly.
        compile_crepus(&file).unwrap();
    }

    #[test]
    fn compile_crepus_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("nonexistent.crepus");
        let err = compile_crepus(&file).unwrap_err();
        assert!(err.contains("could not stat"));
    }
}
