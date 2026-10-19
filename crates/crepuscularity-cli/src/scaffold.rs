use std::fs;
use std::path::Path;

use console::style;

use crate::ui;

pub fn validate_project_name(name: &str) -> Result<(), &'static str> {
    if !name
        .chars()
        .next()
        .is_some_and(|c| unicode_ident::is_xid_start(c) || c == '_')
        || !name
            .chars()
            .all(|c| unicode_ident::is_xid_continue(c) || matches!(c, '-' | '_'))
    {
        return Err("project name must start with a Unicode identifier character or underscore and contain only Unicode identifier characters, hyphens, or underscores");
    }
    Ok(())
}

pub(crate) fn cargo_package_name(name: &str) -> String {
    let mut name: String = name
        .chars()
        .map(|c| {
            if unicode_ident::is_xid_continue(c) || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !name
        .chars()
        .next()
        .is_some_and(|c| c == '_' || unicode_ident::is_xid_start(c))
    {
        name.insert(0, '_');
    }
    name
}

/// Create a directory and all of its parents if they do not exist.
pub fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("failed to create '{}': {e}", path.display()))
}

/// Validate a relative copy path under an existing canonical destination root.
// shortcut: preflight assumes stable filesystem links; use handle-relative writes if concurrent mutation must be supported.
pub(crate) fn validate_copy_destination(root: &Path, relative: &Path) -> std::io::Result<()> {
    use std::path::Component;
    if relative.components().any(|part| {
        matches!(
            part,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "copy destination must stay within its root",
        ));
    }
    let mut path = root.to_path_buf();
    for part in relative.components() {
        path.push(part.as_os_str());
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let resolved = path.canonicalize().map_err(|error| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "unresolved copy destination symlink {}: {error}",
                            path.display()
                        ),
                    )
                })?;
                if !resolved.starts_with(root) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!(
                            "copy destination escapes its root through {}",
                            path.display()
                        ),
                    ));
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Claim a new project directory without replacing an existing destination.
pub fn create_project_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        ensure_dir(parent)?;
    }
    fs::create_dir(path).map_err(|e| format!("failed to create project '{}': {e}", path.display()))
}

/// Write `content` to `path`, creating parent directories first.
pub fn write_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            ensure_dir(parent)?;
        }
    }
    fs::write(path, content).map_err(|e| format!("failed to write '{}': {e}", path.display()))
}

/// Apply `replacements` to `template` and write the result to `path`.
///
/// Each replacement is a `(placeholder, value)` pair; the placeholder is
/// searched literally in the template.
pub fn write_template(
    path: &Path,
    template: &str,
    replacements: &[(&str, &str)],
) -> Result<(), String> {
    let mut content = template.to_string();
    for (placeholder, value) in replacements {
        content = content.replace(placeholder, value);
    }
    write_file(path, &content)
}

/// Print a consistent post-scaffold success message.
///
/// `name` is the project name; `path` is the directory that was created.
/// `next_steps` are printed verbatim, indented by two spaces.
pub fn scaffold_success(name: &str, path: &Path, next_steps: &[&str]) {
    let _ = name;
    eprintln!(
        "\n{} created {}",
        ui::ok(),
        style(format!("{}/", path.display())).cyan().bold()
    );
    eprintln!();
    eprintln!("{}", style("Next steps:").dim());
    for step in next_steps {
        eprintln!("  {step}");
    }
}

#[cfg(test)]
mod project_directory_tests {
    use super::*;
    #[test]
    fn project_creation_is_exclusive_and_supports_nested_paths() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("nested/app");
        create_project_dir(&project).unwrap();
        fs::write(project.join("keep"), "original").unwrap();
        assert!(create_project_dir(&project).is_err());
        assert_eq!(
            fs::read_to_string(project.join("keep")).unwrap(),
            "original"
        );
    }
}
