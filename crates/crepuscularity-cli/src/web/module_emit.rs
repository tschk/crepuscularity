//! Compile only the reachable authored-module graph; never evaluate source.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use crepuscularity_native::{
    compile_moonshine_astro, compile_moonshine_module_with_resolver, compile_moonshine_svelte,
    compile_moonshine_vue_with_resolver, rewrite_moonshine_imports,
    try_compile_moonshine_module_with_resolver, MoonshineModule, MoonshineModuleLanguage,
    MoonshineModuleTarget, SolidComponentModule, VueTypeModule,
};

pub(super) struct ModuleOutput {
    pub files: BTreeMap<PathBuf, String>,
}

/// `None` leaves plain JSX templates on their existing View IR path.
pub(super) fn prepare(
    site: &Path,
    output: &Path,
    entry: &str,
    target: MoonshineModuleTarget,
) -> Result<Option<ModuleOutput>, String> {
    let (entry_file, selector) = entry
        .split_once('#')
        .map_or((entry, None), |(file, selector)| (file, Some(selector)));
    let entry_path = Path::new(entry_file);
    let extension = entry_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let component_entry = is_component(entry_path);
    if !component_entry
        && !extension.eq_ignore_ascii_case("tsx")
        && !extension.eq_ignore_ascii_case("jsx")
    {
        return Ok(None);
    }
    let root = site
        .canonicalize()
        .map_err(|error| format!("site {}: {error}", site.display()))?;
    let entry_key = normalize(entry_path)?;
    let entry_source = read_source(&root, &entry_key)?;
    if extension.eq_ignore_ascii_case("vue") {
        let sfc = crepuscularity_core::parser::parse_vue_sfc(&entry_source)
            .map_err(|error| error.to_string())?;
        if sfc.scripts.is_empty() && sfc.styles.is_empty() {
            return Ok(None);
        }
    }
    if extension.eq_ignore_ascii_case("astro")
        && crepuscularity_core::parser::split_astro_source(&entry_source)
            .map_err(|error| error.to_string())?
            .frontmatter
            .is_none()
    {
        return Ok(None);
    }
    let compiled = if component_entry {
        compile_source(&root, &entry_source, &entry_key, target)?
    } else {
        let Some(compiled) = try_compile_moonshine_module_with_resolver(
            &entry_source,
            &entry_key,
            target,
            &mut |importer, specifier| resolve_solid_module(&root, importer, specifier),
        )?
        else {
            return Ok(None);
        };
        compiled
    };
    if selector.is_some() {
        return Err("authored module entries do not support #Component selectors".into());
    }
    let output_root = absolute_resolved(output)?;
    if root == output_root || root.starts_with(&output_root) {
        return Err("module output directory must not contain or equal the source site".into());
    }
    let (target_name, facade_name) = match target {
        MoonshineModuleTarget::Dom => ("moonshine-dom", "CrepusEmit.moonshine-dom.ts"),
        MoonshineModuleTarget::Ssr => ("moonshine-ssr", "CrepusEmit.moonshine-ssr.ts"),
    };
    let module_root = PathBuf::from("modules").join(target_name);
    let has_default = compiled.has_default_export;
    let mut files = BTreeMap::new();
    let mut visited = HashSet::new();
    let mut canonical_keys = HashMap::new();
    let mut graph = BTreeMap::new();
    let mut pending = vec![(entry_key.clone(), Some(compiled))];
    while let Some((key, compiled)) = pending.pop() {
        if !visited.insert(key.clone()) {
            continue;
        }
        let canonical = root
            .join(&key)
            .canonicalize()
            .map_err(|error| error.to_string())?;
        if let Some(previous) = canonical_keys.insert(canonical, key.clone()) {
            if previous != key {
                return Err(format!(
                    "module {} aliases already included {}; symlink module aliases are unsupported",
                    key.display(),
                    previous.display()
                ));
            }
        }
        let compiled = match compiled {
            Some(compiled) => compiled,
            None => compile_source(&root, &read_source(&root, &key)?, &key, target)
                .map_err(|error| format!("module {}: {error}", key.display()))?,
        };
        let mut dependencies = Vec::new();
        for import in &compiled.imports {
            if import.starts_with('.') {
                let dependency = resolve_import(&root, &key, import)?;
                dependencies.push((import.clone(), dependency.clone()));
                pending.push((dependency, None));
            } else if import.starts_with('/') || import.starts_with("file:") {
                return Err(format!(
                    "module {}: absolute import {import:?} is unsupported",
                    key.display()
                ));
            }
        }
        graph.insert(key, (compiled, dependencies));
    }
    let javascript = graph
        .iter()
        .filter(|(_, (module, _))| module.language == MoonshineModuleLanguage::JavaScript)
        .map(|(path, _)| path.clone())
        .collect::<HashSet<_>>();
    let output_keys = allocate_output_keys_with_languages(graph.keys(), &javascript);
    let mut assets = BTreeMap::new();
    let mut reserved = output_keys.values().cloned().collect::<HashSet<_>>();
    for (key, (compiled, dependencies)) in graph {
        let emitted_key = &output_keys[&key];
        let replacements = dependencies
            .into_iter()
            .filter_map(|(import, dependency)| {
                let emitted_dependency = &output_keys[&dependency];
                if is_component(&dependency)
                    || emitted_dependency != &dependency
                    || emitted_key.parent() != key.parent()
                {
                    Some((import, relative_specifier(emitted_key, emitted_dependency)))
                } else {
                    None
                }
            })
            .collect::<HashMap<_, _>>();
        let mut source = if replacements.is_empty() {
            compiled.source
        } else {
            rewrite_moonshine_imports(&compiled.source, emitted_key, &replacements)
                .map_err(|error| format!("rewrite module {}: {error}", key.display()))?
        };
        for (index, style) in compiled.styles.into_iter().enumerate() {
            let mut suffix = 0;
            let asset_key = loop {
                let candidate = PathBuf::from(format!(
                    "{}.style-{index}-{suffix}.css",
                    emitted_key.to_string_lossy()
                ));
                let declaration = PathBuf::from(format!("{}.d.ts", candidate.to_string_lossy()));
                if !reserved.iter().any(|path| {
                    path == &candidate
                        || path.starts_with(&candidate)
                        || path == &declaration
                        || path.starts_with(&declaration)
                }) {
                    reserved.insert(candidate.clone());
                    reserved.insert(declaration);
                    break candidate;
                }
                suffix += 1;
            };
            let specifier = relative_specifier(emitted_key, &asset_key);
            let quoted = serde_json::to_string(&specifier).map_err(|error| error.to_string())?;
            source.push_str(&format!("\nimport {quoted};\n"));
            files.insert(
                module_root.join(format!("{}.d.ts", asset_key.to_string_lossy())),
                "export {};\n".to_owned(),
            );
            assets.insert(module_root.join(asset_key), style);
        }
        files.insert(module_root.join(emitted_key), source);
    }
    let facade_key = module_root.join(&output_keys[&entry_key]);
    let specifier = format!("./{}", facade_key.to_string_lossy().replace('\\', "/"));
    let quoted = serde_json::to_string(&specifier).map_err(|error| error.to_string())?;
    let mut facade = format!(
        "// Generated authored-module entry; source is not executed during compilation.\nexport * from {quoted};\n"
    );
    if has_default {
        facade.push_str(&format!("export {{ default }} from {quoted};\n"));
    }
    files.insert(PathBuf::from(facade_name), facade);
    let mut manifest = serde_json::json!({
        "version": 1,
        "target": target_name,
        "entry": facade_name,
        "sourceEntry": entry_key.to_string_lossy().replace('\\', "/"),
        "sourceModule": facade_key.to_string_lossy().replace('\\', "/"),
        "files": files.keys().map(|path| path.to_string_lossy().replace('\\', "/")).collect::<Vec<_>>(),
    });
    if !assets.is_empty() {
        manifest["assets"] = serde_json::json!(assets
            .keys()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect::<Vec<_>>());
    }
    files.extend(assets);
    files.insert(
        PathBuf::from("crepus-module-manifest.json"),
        serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?,
    );
    let mut destinations = HashMap::new();
    for relative in files.keys() {
        let javascript = relative
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("js") || extension.eq_ignore_ascii_case("jsx")
            });
        if relative.starts_with(&module_root) && javascript {
            for extension in ["ts", "tsx", "d.ts"] {
                let substituted = relative.with_extension(extension);
                if !files.contains_key(&substituted) && output_root.join(&substituted).is_file() {
                    return Err(format!(
                        "existing module {} shadows JavaScript module {}; use an empty output directory",
                        substituted.display(),
                        relative.display()
                    ));
                }
            }
        }
        let destination = absolute_resolved(&output_root.join(relative))?;
        if destination.is_dir() {
            return Err(format!(
                "module output {} is an existing directory",
                relative.display()
            ));
        }
        if destination.exists() && !destination.is_file() {
            return Err(format!(
                "module output {} is not a regular file",
                relative.display()
            ));
        }
        if !destination.starts_with(&output_root) {
            return Err(format!(
                "module output {} escapes output directory",
                relative.display()
            ));
        }
        if let Some(previous) = destinations.insert(destination.clone(), relative) {
            return Err(format!(
                "module outputs {} and {} resolve to the same destination",
                previous.display(),
                relative.display()
            ));
        }
        if let Some(source) = canonical_keys.get(&destination) {
            return Err(format!(
                "module output would overwrite source {}",
                source.display()
            ));
        }
    }
    Ok(Some(ModuleOutput { files }))
}

fn is_component(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("svelte")
                || extension.eq_ignore_ascii_case("vue")
                || extension.eq_ignore_ascii_case("astro")
        })
}

fn compile_source(
    root: &Path,
    source: &str,
    path: &Path,
    target: MoonshineModuleTarget,
) -> Result<MoonshineModule, String> {
    if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svelte"))
    {
        compile_moonshine_svelte(source, path, target)
    } else if path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("astro"))
    {
        compile_moonshine_astro(source, path, target)
    } else if is_component(path) {
        compile_moonshine_vue_with_resolver(source, path, target, &mut |importer, specifier| {
            let path = resolve_import(root, importer, specifier)?;
            let source = read_source(root, &path)?;
            Ok(VueTypeModule { path, source })
        })
    } else {
        compile_moonshine_module_with_resolver(source, path, target, &mut |importer, specifier| {
            resolve_solid_module(root, importer, specifier)
        })
    }
}

fn resolve_solid_module(
    root: &Path,
    importer: &Path,
    specifier: &str,
) -> Result<SolidComponentModule, String> {
    let path = resolve_import(root, importer, specifier)?;
    let source = read_source(root, &path)?;
    Ok(SolidComponentModule { path, source })
}

fn output_key(source: &Path, javascript: &HashSet<PathBuf>) -> PathBuf {
    if is_component(source) {
        let mut path = source.as_os_str().to_os_string();
        path.push(if javascript.contains(source) {
            ".js"
        } else {
            ".ts"
        });
        PathBuf::from(path)
    } else {
        source.to_path_buf()
    }
}

/// Allocate all leaves together so discovery order cannot change output names.
#[cfg(test)]
fn allocate_output_keys<'a>(
    sources: impl IntoIterator<Item = &'a PathBuf>,
) -> BTreeMap<PathBuf, PathBuf> {
    allocate_output_keys_with_languages(sources, &HashSet::new())
}

fn allocate_output_keys_with_languages<'a>(
    sources: impl IntoIterator<Item = &'a PathBuf>,
    javascript: &HashSet<PathBuf>,
) -> BTreeMap<PathBuf, PathBuf> {
    let mut sources = sources.into_iter().cloned().collect::<Vec<_>>();
    sources.sort();
    let mut directories = HashSet::new();
    for source in &sources {
        let preferred = output_key(source, javascript);
        for parent in preferred.ancestors().skip(1) {
            if !parent.as_os_str().is_empty() {
                directories.insert(parent.to_path_buf());
            }
        }
    }
    let mut reserved = sources
        .iter()
        .filter(|source| !is_component(source))
        .flat_map(|source| {
            // TypeScript resolves .js/.jsx imports through .ts, .tsx and .d.ts first.
            // Generated components must not capture imports of ordinary modules.
            let substitutes_typescript = source
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("js") || extension.eq_ignore_ascii_case("jsx")
                });
            std::iter::once(source.clone()).chain(
                ["ts", "tsx", "d.ts"]
                    .into_iter()
                    .filter(move |_| substitutes_typescript)
                    .map(|extension| source.with_extension(extension)),
            )
        })
        .collect::<HashSet<_>>();
    let mut output = BTreeMap::new();
    for source in sources {
        if !is_component(&source) {
            output.insert(source.clone(), source);
            continue;
        }
        let preferred = output_key(&source, javascript);
        let emitted = if !reserved.contains(&preferred)
            && !directories.contains(&preferred)
            && !(javascript.contains(&source)
                && ["ts", "tsx", "d.ts"].iter().any(|extension| {
                    let substitute = preferred.with_extension(extension);
                    reserved.contains(&substitute) || directories.contains(&substitute)
                })) {
            preferred
        } else {
            let name = source.file_name().expect("normalized module leaf");
            (0..)
                .map(|index| {
                    let mut candidate = name.to_os_string();
                    candidate.push(format!(
                        ".__crepus{index}.{}",
                        if javascript.contains(&source) {
                            "js"
                        } else {
                            "ts"
                        }
                    ));
                    source.with_file_name(candidate)
                })
                .find(|candidate| {
                    !reserved.contains(candidate)
                        && !directories.contains(candidate)
                        && !(javascript.contains(&source)
                            && ["ts", "tsx", "d.ts"].iter().any(|extension| {
                                let substitute = candidate.with_extension(extension);
                                reserved.contains(&substitute) || directories.contains(&substitute)
                            }))
                })
                .unwrap()
        };
        reserved.insert(emitted.clone());
        output.insert(source, emitted);
    }
    output
}

fn relative_specifier(importer: &Path, dependency: &Path) -> String {
    let parent = importer.parent().unwrap_or(Path::new(""));
    let from = parent.components().collect::<Vec<_>>();
    let to = dependency.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..from.len() {
        relative.push("..");
    }
    for component in &to[common..] {
        relative.push(component.as_os_str());
    }
    let value = relative.to_string_lossy().replace('\\', "/");
    if value.starts_with("../") {
        value
    } else {
        format!("./{value}")
    }
}

fn normalize(path: &Path) -> Result<PathBuf, String> {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => result.push(value),
            Component::ParentDir if result.pop() => {}
            _ => {
                return Err(format!(
                    "module path {} escapes the source site",
                    path.display()
                ));
            }
        }
    }
    Ok(result)
}

fn read_source(root: &Path, key: &Path) -> Result<String, String> {
    let path = root.join(key);
    let mut component_path = root.to_path_buf();
    for component in key.components() {
        component_path.push(component);
        if std::fs::symlink_metadata(&component_path)
            .map_err(|error| format!("module {}: {error}", key.display()))?
            .file_type()
            .is_symlink()
        {
            return Err(format!(
                "module {} uses a source symlink; symlink module paths are unsupported",
                key.display()
            ));
        }
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("module {}: {error}", key.display()))?;
    if !canonical.starts_with(root) {
        return Err(format!(
            "module {} resolves outside the source site",
            key.display()
        ));
    }
    if !canonical.is_file() {
        return Err(format!(
            "module {} source is not a regular file",
            key.display()
        ));
    }
    std::fs::read_to_string(path).map_err(|error| format!("read module {}: {error}", key.display()))
}

fn resolve_import(root: &Path, importer: &Path, import: &str) -> Result<PathBuf, String> {
    let base = normalize(&importer.parent().unwrap_or(Path::new("")).join(import))?;
    let extensions = ["ts", "tsx", "js", "jsx", "svelte", "vue", "astro"];
    let mut candidates = vec![];
    if let Some(extension) = base.extension().and_then(|value| value.to_str()) {
        if !extensions
            .iter()
            .any(|supported| extension.eq_ignore_ascii_case(supported))
        {
            return Err(format!(
                "module {}: unsupported relative import {import:?}",
                importer.display()
            ));
        }
        if root.join(&base).is_file() {
            candidates.push(base);
        }
    } else {
        for extension in extensions {
            for candidate in [
                base.with_extension(extension),
                base.join(format!("index.{extension}")),
            ] {
                if root.join(&candidate).is_file() {
                    candidates.push(candidate);
                }
            }
            // Component files are a fallback, not an ambiguity with ordinary modules.
            if extension == "jsx" && !candidates.is_empty() {
                break;
            }
        }
    }
    match candidates.as_slice() {
        [candidate] => {
            read_source(root, candidate)?;
            Ok(candidate.clone())
        }
        [] => Err(format!(
            "module {}: cannot resolve relative import {import:?}",
            importer.display()
        )),
        _ => Err(format!(
            "module {}: ambiguous relative import {import:?}",
            importer.display()
        )),
    }
}

/// Resolve existing ancestors as well as missing output-directory suffixes.
pub(crate) fn absolute_resolved(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let mut ancestor = absolute.as_path();
    let mut suffix = vec![];
    while !ancestor.exists() {
        if std::fs::symlink_metadata(ancestor)
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(format!(
                "output path {} contains an unresolved symlink {}",
                path.display(),
                ancestor.display()
            ));
        }
        suffix.push(
            ancestor
                .file_name()
                .ok_or_else(|| format!("invalid output path {}", path.display()))?
                .to_os_string(),
        );
        ancestor = ancestor
            .parent()
            .ok_or_else(|| format!("invalid output path {}", path.display()))?;
    }
    if !suffix.is_empty() && !ancestor.is_dir() {
        return Err(format!(
            "output ancestor {} is not a directory",
            ancestor.display()
        ));
    }
    let mut resolved = ancestor.canonicalize().map_err(|error| error.to_string())?;
    for part in suffix.into_iter().rev() {
        resolved.push(part);
    }
    Ok(resolved)
}

#[cfg(test)]
mod output_mapping_tests {
    use super::*;

    fn allocate(paths: &[&str]) -> BTreeMap<PathBuf, PathBuf> {
        let paths = paths.iter().map(PathBuf::from).collect::<Vec<_>>();
        allocate_output_keys(&paths)
    }

    #[test]
    fn javascript_components_avoid_tsx_and_declaration_substitution_at_every_candidate() {
        let sources = [
            "Card.svelte",
            "Card.svelte.tsx",
            "Card.svelte.__crepus0.d.ts",
            "Card.svelte.__crepus1.tsx",
        ]
        .map(PathBuf::from);
        let javascript = HashSet::from([PathBuf::from("Card.svelte")]);
        let output = allocate_output_keys_with_languages(sources.iter(), &javascript);
        assert_eq!(
            output[&PathBuf::from("Card.svelte")],
            PathBuf::from("Card.svelte.__crepus2.js")
        );
    }

    #[test]
    fn javascript_component_keys_avoid_ts_substitution_and_same_name_companions() {
        let sources = [
            "Card.svelte",
            "Card.svelte.js",
            "Card.svelte.ts",
            "Card.svelte.__crepus0.ts",
        ]
        .map(PathBuf::from);
        let javascript = HashSet::from([PathBuf::from("Card.svelte")]);
        let output = allocate_output_keys_with_languages(sources.iter(), &javascript);
        assert_eq!(
            output[Path::new("Card.svelte")],
            Path::new("Card.svelte.__crepus1.js")
        );
        assert_eq!(
            output[Path::new("Card.svelte.js")],
            Path::new("Card.svelte.js")
        );
        assert_eq!(
            output[Path::new("Card.svelte.ts")],
            Path::new("Card.svelte.ts")
        );
    }
    #[test]
    fn ordinary_names_and_noncolliding_components_are_unchanged() {
        let mapping = allocate(&["ui/Card.svelte", "state.svelte.ts", "App.tsx"]);
        assert_eq!(
            mapping[Path::new("ui/Card.svelte")],
            Path::new("ui/Card.svelte.ts")
        );
        assert_eq!(
            mapping[Path::new("state.svelte.ts")],
            Path::new("state.svelte.ts")
        );
        assert_eq!(mapping[Path::new("App.tsx")], Path::new("App.tsx"));
    }

    #[test]
    fn component_companion_collision_is_deterministic() {
        let forward = allocate(&["Card.svelte", "Card.svelte.ts"]);
        let reverse = allocate(&["Card.svelte.ts", "Card.svelte"]);
        assert_eq!(forward, reverse);
        assert_eq!(
            forward[Path::new("Card.svelte")],
            Path::new("Card.svelte.__crepus0.ts")
        );
        assert_eq!(
            forward[Path::new("Card.svelte.ts")],
            Path::new("Card.svelte.ts")
        );
    }

    #[test]
    fn remap_candidates_skip_reserved_files_and_required_directories() {
        let mapping = allocate(&[
            "ui/Card.svelte",
            "ui/Card.svelte.ts",
            "ui/Card.svelte.__crepus0.ts",
            "ui/Card.svelte.__crepus1.ts/child.ts",
        ]);
        assert_eq!(
            mapping[Path::new("ui/Card.svelte")],
            Path::new("ui/Card.svelte.__crepus2.ts")
        );
    }

    #[test]
    fn javascript_companions_reserve_typescript_substitution_paths() {
        for extension in ["js", "jsx", "JS", "JSX"] {
            let companion = format!("Card.svelte.{extension}");
            let forward = allocate(&["Card.svelte", &companion]);
            let reverse = allocate(&[&companion, "Card.svelte"]);
            assert_eq!(forward, reverse);
            assert_eq!(
                forward[Path::new("Card.svelte")],
                Path::new("Card.svelte.__crepus0.ts")
            );
            assert_eq!(forward[Path::new(&companion)], Path::new(&companion));
        }
    }

    #[test]
    fn javascript_remap_candidates_and_directories_are_reserved() {
        let mapping = allocate(&[
            "ui/Card.svelte",
            "ui/Card.svelte.js",
            "ui/Card.svelte.__crepus0.jsx",
            "ui/Card.svelte.__crepus1.js",
            "ui/Card.svelte.__crepus2.ts/child.ts",
        ]);
        assert_eq!(
            mapping[Path::new("ui/Card.svelte")],
            Path::new("ui/Card.svelte.__crepus3.ts")
        );
        for ordinary in [
            "ui/Card.svelte.js",
            "ui/Card.svelte.__crepus0.jsx",
            "ui/Card.svelte.__crepus1.js",
            "ui/Card.svelte.__crepus2.ts/child.ts",
        ] {
            assert_eq!(mapping[Path::new(ordinary)], Path::new(ordinary));
        }
    }

    #[test]
    fn unrelated_javascript_modules_do_not_remap_components() {
        let mapping = allocate(&["ui/Card.svelte", "ui/other.js", "ui/view.jsx"]);
        assert_eq!(
            mapping[Path::new("ui/Card.svelte")],
            Path::new("ui/Card.svelte.ts")
        );
        assert_eq!(mapping[Path::new("ui/other.js")], Path::new("ui/other.js"));
        assert_eq!(mapping[Path::new("ui/view.jsx")], Path::new("ui/view.jsx"));
    }

    #[test]
    fn component_preferred_output_cannot_replace_another_modules_directory() {
        let mapping = allocate(&["A.svelte", "A.svelte.ts/B.svelte"]);
        assert_eq!(
            mapping[Path::new("A.svelte")],
            Path::new("A.svelte.__crepus0.ts")
        );
        assert_eq!(
            mapping[Path::new("A.svelte.ts/B.svelte")],
            Path::new("A.svelte.ts/B.svelte.ts")
        );
    }
    #[test]
    fn extensionless_scripts_precede_component_fallback_without_hiding_script_ambiguity() {
        let root = tempfile::tempdir().unwrap();
        let canonical = root.path().canonicalize().unwrap();
        let root = canonical.as_path();
        std::fs::write(root.join("Card.svelte"), "<p/>").unwrap();
        std::fs::write(root.join("Card.ts"), "export const value=1").unwrap();
        assert_eq!(
            resolve_import(root, Path::new("Card.svelte"), "./Card").unwrap(),
            Path::new("Card.ts")
        );
        assert_eq!(
            resolve_import(root, Path::new("Card.svelte"), "./Card.svelte").unwrap(),
            Path::new("Card.svelte")
        );
        std::fs::write(root.join("Card.js"), "export const value=2").unwrap();
        assert!(resolve_import(root, Path::new("App.svelte"), "./Card")
            .unwrap_err()
            .contains("ambiguous"));
        std::fs::remove_file(root.join("Card.js")).unwrap();
        std::fs::create_dir(root.join("Card")).unwrap();
        std::fs::write(root.join("Card/index.ts"), "export const value=3").unwrap();
        assert!(resolve_import(root, Path::new("App.svelte"), "./Card")
            .unwrap_err()
            .contains("ambiguous"));
        std::fs::remove_file(root.join("Card.ts")).unwrap();
        std::fs::remove_file(root.join("Card/index.ts")).unwrap();
        assert_eq!(
            resolve_import(root, Path::new("App.svelte"), "./Card").unwrap(),
            Path::new("Card.svelte")
        );
    }
}

#[cfg(test)]
mod vue_graph_tests {
    use super::*;
    #[test]
    fn vue_components_do_not_overwrite_same_basename_helpers() {
        let sources = [PathBuf::from("Card.vue"), PathBuf::from("Card.vue.ts")];
        let mapping = allocate_output_keys(sources.iter());
        assert_ne!(mapping[Path::new("Card.vue")], PathBuf::from("Card.vue.ts"));
        assert_eq!(
            mapping[Path::new("Card.vue.ts")],
            PathBuf::from("Card.vue.ts")
        );
    }
}

#[cfg(all(test, unix))]
mod output_resolution_tests {
    use super::absolute_resolved;
    use std::os::unix::fs::symlink;

    #[test]
    fn missing_suffixes_do_not_hide_dangling_symlink_ancestors() {
        let root = tempfile::tempdir().unwrap();
        let link = root.path().join("output");
        symlink(root.path().join("missing"), &link).unwrap();
        let error = absolute_resolved(&link.join("nested/component.ts")).unwrap_err();
        assert!(error.contains("unresolved symlink"), "{error}");
        assert!(!root.path().join("missing").exists());
    }

    #[test]
    fn existing_symlink_directories_keep_missing_output_suffixes() {
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("actual");
        std::fs::create_dir(&destination).unwrap();
        let link = root.path().join("output");
        symlink(&destination, &link).unwrap();
        assert_eq!(
            absolute_resolved(&link.join("nested/component.ts")).unwrap(),
            destination
                .canonicalize()
                .unwrap()
                .join("nested/component.ts")
        );
        assert!(!destination.join("nested").exists());
    }
}
