use std::fs;
use std::path::Path;

use console::style;

use crate::ui;

/// Create a directory and all of its parents if they do not exist.
pub fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| format!("failed to create '{}': {e}", path.display()))
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
