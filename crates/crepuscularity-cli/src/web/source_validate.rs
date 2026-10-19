//! Validate reachable template sources before native lowering and output writes.
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crepuscularity_core::ast::Node;
use crepuscularity_core::context::TemplateContext;
use crepuscularity_core::include_paths::{include_frontend_path, resolve_include_path};
use crepuscularity_core::parser::{
    parse_component_file, parse_svelte_component, parse_template_with_path, parse_vue_sfc,
};
use crepuscularity_core::virtual_files::lookup_virtual_file_entry;

type PendingSource = (PathBuf, PathBuf, Option<String>, TemplateContext);

/// Validate the entry's template dependency graph, without evaluating DSL or JS.
/// Unreferenced files in the loaded virtual bundle do not participate.
pub(super) fn validate_templates(
    site_dir: &Path,
    files: &HashMap<String, String>,
    entry: &str,
    deny_scripts: bool,
) -> Result<HashSet<PathBuf>, String> {
    let (file, component) = split_component(entry);
    let mut context = TemplateContext::new();
    context.virtual_files = Arc::new(files.clone());
    context.base_dir = Path::new(file)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf);
    let mut pending = vec![(
        PathBuf::from(file),
        PathBuf::from(file),
        component.map(str::to_owned),
        context,
    )];
    let mut visited = HashSet::new();
    let mut sources = HashSet::new();
    // An explicit worklist avoids adding include-chain depth to the Rust stack;
    // native lowering remains responsible for its own include depth/cycle guard.
    while let Some((path, frontend_path, component, context)) = pending.pop() {
        if !visited.insert((path.clone(), frontend_path.clone(), component.clone())) {
            continue;
        }
        let (source, actual_path) =
            if let Some((key, content)) = lookup_virtual_file_entry(&context, &path) {
                (content.to_owned(), site_dir.join(key))
            } else {
                match std::fs::read_to_string(&path) {
                    Ok(content) => (content, path.clone()),
                    Err(error) if path.is_relative() => {
                        let actual = site_dir.join(&path);
                        let content = std::fs::read_to_string(&actual).map_err(|fallback| {
                            format!(
                                "{}: {fallback} (requested {}: {error})",
                                actual.display(),
                                path.display()
                            )
                        })?;
                        (content, actual)
                    }
                    Err(error) => return Err(format!("{}: {error}", path.display())),
                }
            };
        sources.insert(super::module_emit::absolute_resolved(&actual_path)?);
        let extension = frontend_path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let scripted = if deny_scripts {
            match extension.as_str() {
                "svelte" => {
                    let parsed = parse_svelte_component(&source)
                        .map_err(|error| format!("{}: {error}", path.display()))?;
                    if parsed.style.is_some() {
                        return Err(format!(
                            "{}: native Svelte styles are unsupported",
                            path.display()
                        ));
                    }
                    parsed.script.is_some() || parsed.module_script.is_some()
                }
                "vue" => {
                    let parsed = parse_vue_sfc(&source)
                        .map_err(|error| format!("{}: {error}", path.display()))?;
                    if !parsed.styles.is_empty() {
                        return Err(format!(
                            "{}: native Vue styles are unsupported",
                            path.display()
                        ));
                    }
                    !parsed.scripts.is_empty()
                }
                _ => false,
            }
        } else {
            false
        };
        if scripted {
            return Err(format!(
                "{}: script execution/lowering unsupported; use authored .tsx/.jsx module or template-only SFC",
                path.display()
            ));
        }
        let nodes = if let Some(component) = component {
            let parsed = parse_component_file(&source)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            parsed
                .components
                .get(&component)
                .ok_or_else(|| format!("{}: component '{component}' not found", path.display()))?
                .nodes
                .clone()
        } else {
            parse_template_with_path(&source, Some(&frontend_path))
                .map_err(|error| format!("{}: {error}", path.display()))?
        };
        collect_includes(&nodes, &context, &mut pending)?;
    }
    Ok(sources)
}

fn split_component(path: &str) -> (&str, Option<&str>) {
    match path.split_once('#') {
        Some((path, component)) => (path, Some(component)),
        None => (path, None),
    }
}

fn collect_includes(
    nodes: &[Node],
    context: &TemplateContext,
    pending: &mut Vec<PendingSource>,
) -> Result<(), String> {
    for node in nodes {
        match node {
            Node::Include(include) => {
                // Supplied slots retain the caller's include base, independently
                // of whether the same component file was already visited.
                collect_includes(&include.slot, context, pending)?;
                let (file, component) = split_component(&include.path);
                let path = resolve_include_path(context.base_dir.as_deref(), file)
                    .map_err(|error| format!("{}: {error}", include.path))?;
                let mut child = TemplateContext::new();
                child.base_dir = path.parent().map(Path::to_path_buf);
                child.virtual_files = Arc::clone(&context.virtual_files);
                let frontend_path = if component.is_some() {
                    path.clone()
                } else {
                    include_frontend_path(&path, file)
                };
                pending.push((path, frontend_path, component.map(str::to_owned), child));
            }
            Node::Element(element) => collect_includes(&element.children, context, pending)?,
            Node::If(block) => {
                collect_includes(&block.then_children, context, pending)?;
                if let Some(children) = &block.else_children {
                    collect_includes(children, context, pending)?;
                }
            }
            Node::For(block) => collect_includes(&block.body, context, pending)?,
            Node::Match(block) => {
                for arm in &block.arms {
                    collect_includes(&arm.body, context, pending)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn same_frontend_aliases_keep_distinct_physical_sources_in_validation() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("a")).unwrap();
        std::fs::create_dir(dir.path().join("b")).unwrap();
        let plain = dir.path().join("plain.txt");
        let scripted = dir.path().join("scripted.txt");
        std::fs::write(&plain, "<template><p>Plain</p></template>").unwrap();
        std::fs::write(
            &scripted,
            "<script setup>const value=1</script><template><p>Scripted</p></template>",
        )
        .unwrap();
        std::os::unix::fs::symlink(&plain, dir.path().join("a/view.vue")).unwrap();
        std::os::unix::fs::symlink(&scripted, dir.path().join("b/view.vue")).unwrap();
        let entry = dir.path().join("entry.crepus");
        // The LIFO worklist visits the plain alias first. Both authored basenames
        // produce the same frontend path, but the second physical file must run
        // its own script admission check.
        std::fs::write(&entry, "include b/view.vue\ninclude a/view.vue").unwrap();
        let error = validate_templates(dir.path(), &HashMap::new(), entry.to_str().unwrap(), true)
            .unwrap_err();
        assert!(
            error.contains("script execution/lowering unsupported"),
            "{error}"
        );
    }
}
