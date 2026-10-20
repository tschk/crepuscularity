use crepuscularity_core::ast::Node;
use crepuscularity_core::parser::{parse_component_file, parse_template_with_path};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub(crate) struct WebIslandRef {
    pub(crate) src: String,
    pub(crate) adapter: String,
}

#[derive(Debug, Serialize)]
struct WebIslandManifest {
    version: u8,
    islands: BTreeMap<String, WebIslandManifestEntry>,
}

#[derive(Debug, Serialize)]
struct WebIslandManifestEntry {
    adapter: String,
    module: String,
}

pub(crate) fn collect_web_islands(
    files: &HashMap<String, String>,
    entry: &str,
) -> Result<Vec<WebIslandRef>, String> {
    let mut context = crepuscularity_core::TemplateContext::new();
    context.virtual_files = std::sync::Arc::new(files.clone());
    let mut pending: Vec<_> = files
        .keys()
        .filter(|path| {
            Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("crepus"))
        })
        .map(|path| (PathBuf::from(path), None::<String>, 0usize))
        .collect();
    let (entry, selector) = entry
        .split_once('#')
        .map_or((entry, None), |(path, name)| (path, Some(name.to_string())));
    if files.contains_key(entry) {
        pending.push((PathBuf::from(entry), selector, 0));
    }
    // Match the renderer's include-depth bound; discovery is conservative and
    // does not reject inactive cycles. Bound distinct namespaces as well, since
    // virtual suffix fallback can branch into ever-growing logical paths.
    const MAX_DEPTH: usize = 64;
    const MAX_STATES: usize = 4096;
    let mut seen = HashMap::new();
    let mut accepted_states = 0;
    let mut out = BTreeMap::new();
    while let Some((path, selector, depth)) = pending.pop() {
        // Equivalent ./ spellings must not grow a cyclic include worklist.
        let path: PathBuf = path
            .components()
            .filter(|part| !matches!(part, Component::CurDir))
            .collect();
        let key = (path.clone(), selector.clone());
        if seen.get(&key).is_some_and(|previous| *previous <= depth) {
            continue;
        }
        accepted_states += 1;
        if accepted_states > MAX_STATES {
            return Err(format!(
                "island template discovery exceeds {MAX_STATES} states"
            ));
        }
        seen.insert(key, depth);
        let Some((_, content)) =
            crepuscularity_core::virtual_files::lookup_virtual_file_entry(&context, &path)
        else {
            // Discovery covers inactive branches too; a missing lazy include is not fatal here.
            continue;
        };
        let mut includes = Vec::new();
        if selector.is_some()
            || path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("crepus"))
        {
            let components =
                parse_component_file(content).map_err(|e| format!("{}: {e}", path.display()))?;
            // Scan all definitions as before; lazy rendering owns selector validation.
            if !components.components.is_empty() {
                for component in components.components.values() {
                    collect_nodes(&component.nodes, &mut out, &mut includes)?;
                }
            } else {
                let nodes = parse_template_with_path(content, Some(&path))
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                collect_nodes(&nodes, &mut out, &mut includes)?;
            }
        } else {
            let nodes = parse_template_with_path(content, Some(&path))
                .map_err(|e| format!("{}: {e}", path.display()))?;
            collect_nodes(&nodes, &mut out, &mut includes)?;
        }
        if depth >= MAX_DEPTH {
            continue;
        }
        for include in includes {
            let (file, selector) = include
                .split_once('#')
                .map_or((include.as_str(), None), |(file, name)| {
                    (file, Some(name.to_string()))
                });
            let requested = Path::new(file);
            if requested.has_root()
                || requested
                    .components()
                    .any(|part| matches!(part, Component::ParentDir | Component::Prefix(_)))
            {
                return Err(format!("include path outside base dir: {file}"));
            }
            let resolved = path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join(requested);
            pending.push((resolved, selector, depth + 1));
        }
    }
    Ok(out.into_values().collect())
}

pub(crate) fn build_web_islands(
    site_dir: &Path,
    out_dir: &Path,
    files: &HashMap<String, String>,
    entry: &str,
) -> Result<(), String> {
    let refs = collect_web_islands(files, entry)?;
    let manifest_path = out_dir.join("crepus-islands.json");
    let root = out_dir
        .canonicalize()
        .map_err(|e| format!("resolve island output: {e}"))?;
    crate::scaffold::validate_copy_destination(&root, Path::new("crepus-islands.json"))
        .map_err(|e| format!("island manifest destination: {e}"))?;
    let island_out_dir = out_dir.join("islands");
    if !refs.is_empty() {
        crate::scaffold::validate_copy_destination(&root, Path::new("islands"))
            .map_err(|e| format!("island output destination: {e}"))?;
        for island in &refs {
            if island.adapter != "rust" {
                let relative = Path::new("islands").join(island_output_file_name(&island.src));
                crate::scaffold::validate_copy_destination(&root, &relative)
                    .map_err(|e| format!("island output destination: {e}"))?;
            }
        }
        std::fs::create_dir_all(&island_out_dir).map_err(|e| format!("mkdir islands: {e}"))?;
    }

    let mut manifest = WebIslandManifest {
        version: 1,
        islands: BTreeMap::new(),
    };

    for island in refs {
        if island.adapter == "rust" {
            manifest.islands.insert(
                island.src.clone(),
                WebIslandManifestEntry {
                    adapter: "rust".to_string(),
                    module: "rust".to_string(),
                },
            );
            continue;
        }
        let entry = resolve_island_entry(site_dir, &island.src)?;
        let file_name = island_output_file_name(&island.src);
        let out_file = island_out_dir.join(&file_name);
        if island.adapter == "react" {
            let wrapper = react_island_wrapper(&entry)?;
            run_bun_build(wrapper.path(), &out_file)?;
        } else {
            run_bun_build(&entry, &out_file)?;
        }
        manifest.islands.insert(
            island.src.clone(),
            WebIslandManifestEntry {
                adapter: island.adapter,
                module: format!("./islands/{file_name}"),
            },
        );
    }

    let manifest_json =
        serde_json::to_string_pretty(&manifest).map_err(|e| format!("island manifest: {e}"))?;

    crate::scaffold::validate_copy_destination(&root, Path::new("crepus-islands.json"))
        .map_err(|e| format!("island manifest destination: {e}"))?;
    std::fs::write(&manifest_path, manifest_json)
        .map_err(|e| format!("write {}: {e}", manifest_path.display()))?;
    Ok(())
}

fn collect_nodes(
    nodes: &[Node],
    out: &mut BTreeMap<String, WebIslandRef>,
    includes: &mut Vec<String>,
) -> Result<(), String> {
    for node in nodes {
        match node {
            Node::Element(el) => collect_nodes(&el.children, out, includes)?,
            Node::If(block) => {
                collect_nodes(&block.then_children, out, includes)?;
                if let Some(else_children) = &block.else_children {
                    collect_nodes(else_children, out, includes)?;
                }
            }
            Node::For(block) => collect_nodes(&block.body, out, includes)?,
            Node::Match(block) => {
                for arm in &block.arms {
                    collect_nodes(&arm.body, out, includes)?;
                }
            }
            Node::Include(include) => {
                includes.push(include.path.clone());
                collect_nodes(&include.slot, out, includes)?;
            }
            Node::Embed(embed) => {
                let adapter = embed.adapter.clone().unwrap_or_else(|| "module".into());
                if let Some(previous) = out.get(&embed.src) {
                    if previous.adapter != adapter {
                        return Err(format!(
                            "conflicting island adapters for {}: {} and {}",
                            embed.src, previous.adapter, adapter
                        ));
                    }
                } else {
                    out.insert(
                        embed.src.clone(),
                        WebIslandRef {
                            src: embed.src.clone(),
                            adapter,
                        },
                    );
                }
            }
            Node::Text(_) | Node::LetDecl(_) | Node::RawText(_) | Node::RawHtml(_) => {}
        }
    }
    Ok(())
}

fn react_island_wrapper(entry: &Path) -> Result<tempfile::NamedTempFile, String> {
    let directory = entry
        .parent()
        .ok_or_else(|| "island entry has no parent directory".to_string())?;
    let filename = entry
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "island entry filename is not UTF-8".to_string())?;
    let import =
        serde_json::to_string(&format!("./{filename}")).map_err(|error| error.to_string())?;
    let wrapper = tempfile::Builder::new()
        .prefix(".crepus-react-island-")
        .suffix(".tsx")
        .tempfile_in(directory)
        .map_err(|error| format!("create React island wrapper: {error}"))?;
    let source = format!(
        r#"import * as authored from {import};
import {{ createElement }} from "react";
import {{ createRoot }} from "react-dom/client";

export function mount(element: Element, props: Record<string, unknown>, context: unknown) {{
  const component: any = authored;
  if (typeof component.mount === "function") return component.mount(element, props, context);
  const root = createRoot(element);
  root.render(createElement(component.default, props));
  return () => root.unmount();
}}
"#
    );
    std::fs::write(wrapper.path(), source)
        .map_err(|error| format!("write React island wrapper: {error}"))?;
    Ok(wrapper)
}

fn resolve_island_entry(site_dir: &Path, src: &str) -> Result<PathBuf, String> {
    let path = Path::new(src);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!("island path outside site root: {src}"));
    }
    let full = site_dir.join(path);
    if !full.is_file() {
        return Err(format!("island entry not found: {}", full.display()));
    }
    Ok(full)
}

fn island_output_file_name(src: &str) -> String {
    let path = Path::new(src);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("island")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("{stem}-{:016x}.js", stable_hash(src.as_bytes()))
}

fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn run_bun_build(entry: &Path, out_file: &Path) -> Result<(), String> {
    let output = Command::new("bun")
        .arg("build")
        .arg("--target")
        .arg("browser")
        .arg("--format")
        .arg("esm")
        .arg("--packages")
        .arg("bundle")
        .arg("--env")
        .arg("disable")
        .arg("--define")
        .arg("process.env.NODE_ENV=\"production\"")
        .arg("--outfile")
        .arg(out_file)
        .arg(entry)
        .output()
        .map_err(|e| format!("bun build: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(format!("bun build {} failed:\n{stderr}", entry.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    #[ignore = "requires installed Bun for a real island bundle"]
    fn island_bundle_preserves_contained_output_aliases() {
        let site = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        std::fs::write(site.path().join("a.ts"), "export const value = 123;").unwrap();
        let internal = out.path().join("internal");
        std::fs::create_dir(&internal).unwrap();
        std::os::unix::fs::symlink(&internal, out.path().join("islands")).unwrap();
        let target = out.path().join("generated.js");
        std::fs::write(&target, "old").unwrap();
        let name = island_output_file_name("./a.ts");
        std::os::unix::fs::symlink(&target, internal.join(&name)).unwrap();
        let alias = site.path().join("selected-output");
        std::os::unix::fs::symlink(out.path(), &alias).unwrap();
        let files = HashMap::from([(
            "index.crepus".to_string(),
            "embed ./a.ts adapter=\"module\"".to_string(),
        )]);
        build_web_islands(site.path(), &alias, &files, "index.crepus").unwrap();
        assert!(std::fs::read_to_string(alias.join("islands").join(name))
            .unwrap()
            .contains("123"));
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(alias.join("crepus-islands.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["islands"]["./a.ts"]["adapter"], "module");
    }

    #[test]
    #[cfg(unix)]
    fn island_outputs_are_preflighted_before_any_bundle_write() {
        for kind in ["directory", "file", "manifest"] {
            let site = tempfile::tempdir().unwrap();
            let out = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            for name in ["a.ts", "b.ts"] {
                std::fs::write(site.path().join(name), "export const value = 1;").unwrap();
            }
            let files = HashMap::from([(
                "index.crepus".to_string(),
                "embed ./a.ts adapter=\"module\"\nembed ./b.ts adapter=\"module\"".to_string(),
            )]);
            let a = island_output_file_name("./a.ts");
            let b = island_output_file_name("./b.ts");
            std::fs::write(outside.path().join(&b), "keep outside").unwrap();
            let islands = out.path().join("islands");
            if kind == "directory" {
                std::os::unix::fs::symlink(outside.path(), &islands).unwrap();
            } else {
                std::fs::create_dir(&islands).unwrap();
            }
            std::fs::write(islands.join(&a), "keep earlier bundle").unwrap();
            if kind == "file" {
                std::os::unix::fs::symlink(outside.path().join(&b), islands.join(&b)).unwrap();
            }
            let manifest = out.path().join("crepus-islands.json");
            if kind == "manifest" {
                std::fs::write(outside.path().join("manifest.json"), "keep manifest").unwrap();
                std::os::unix::fs::symlink(outside.path().join("manifest.json"), &manifest)
                    .unwrap();
            } else {
                std::fs::write(&manifest, "keep manifest").unwrap();
            }
            let error =
                build_web_islands(site.path(), out.path(), &files, "index.crepus").unwrap_err();
            assert!(error.contains("destination"), "{kind}: {error}");
            assert_eq!(
                std::fs::read_to_string(islands.join(&a)).unwrap(),
                "keep earlier bundle",
                "{kind}"
            );
            assert_eq!(
                std::fs::read_to_string(outside.path().join(&b)).unwrap(),
                "keep outside",
                "{kind}"
            );
            assert_eq!(
                std::fs::read_to_string(manifest).unwrap(),
                "keep manifest",
                "{kind}"
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn empty_manifest_rejects_escaping_destination_links() {
        let site = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let outside = site.path().join("authored.json");
        std::fs::write(&outside, "keep authored").unwrap();
        std::os::unix::fs::symlink(&outside, out.path().join("crepus-islands.json")).unwrap();
        assert!(
            build_web_islands(site.path(), out.path(), &HashMap::new(), "index.crepus").is_err()
        );
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "keep authored");
    }

    #[test]
    fn empty_site_emits_empty_manifest_and_clears_stale_entries() {
        let site = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let manifest = out.path().join("crepus-islands.json");
        for stale in [false, true] {
            if stale {
                std::fs::write(
                    &manifest,
                    r#"{"version":1,"islands":{"old":{"adapter":"module","module":"./old.js"}}}"#,
                )
                .unwrap();
            }
            build_web_islands(site.path(), out.path(), &HashMap::new(), "index.crepus").unwrap();
            let value: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
            assert_eq!(value, serde_json::json!({"version":1,"islands":{}}));
            assert!(!out.path().join("islands").exists());
        }
    }

    #[test]
    fn island_discovery_ignores_opaque_authored_modules() {
        let files = HashMap::from([
            ("index.crepus".into(), "embed ./islands/Counter.tsx adapter=\"react\"".into()),
            ("islands/Counter.tsx".into(), "import React from 'react';\nexport default function Counter() { return <button>Count</button>; }".into()),
            // This deliberately malformed, unused module is opaque to discovery.
            ("unused.tsx".into(), "else".into()),
        ]);
        let islands = collect_web_islands(&files, "index.crepus").unwrap();
        assert_eq!(islands.len(), 1);
        assert_eq!(islands[0].src, "./islands/Counter.tsx");
    }

    #[test]
    fn island_discovery_follows_frontend_include_roles() {
        for (name, source) in [
            ("child.jsx", "<crepus-island src=\"./islands/wave.ts\" />"),
            ("child.tsx", "<crepus-island src=\"./islands/wave.ts\" />"),
        ] {
            let files = HashMap::from([
                ("index.crepus".into(), format!("include {name}")),
                (name.into(), source.into()),
                ("unused.tsx".into(), "export const opaque = ;".into()),
            ]);
            let islands = collect_web_islands(&files, "index.crepus").unwrap();
            assert_eq!(islands.len(), 1, "{name}");
            assert_eq!(islands[0].src, "./islands/wave.ts");
        }
    }

    #[test]
    fn selected_non_crepus_entry_is_a_template_and_dot_cycles_terminate() {
        let files = HashMap::from([
            (
                "Entry.jsx".into(),
                "<crepus-island src=\"wave.ts\" />".into(),
            ),
            ("unused.tsx".into(), "else".into()),
        ]);
        let islands = collect_web_islands(&files, "Entry.jsx").unwrap();
        assert_eq!(islands.len(), 1);
        assert_eq!(islands[0].src, "wave.ts");

        let files = HashMap::from([(
            "index.crepus".into(),
            "include ./index.crepus\nembed wave.ts".into(),
        )]);
        let islands = collect_web_islands(&files, "./index.crepus").unwrap();
        assert_eq!(islands.len(), 1);
    }

    #[test]
    fn island_discovery_rejects_present_malformed_frontend_dependencies() {
        let files = HashMap::from([
            ("index.crepus".into(), "include child.vue".into()),
            ("child.vue".into(), "<template><p></template>".into()),
        ]);
        assert!(collect_web_islands(&files, "index.crepus").is_err());
        let absent = HashMap::from([(
            "index.crepus".into(),
            "if false\n  include missing.vue".into(),
        )]);
        assert!(collect_web_islands(&absent, "index.crepus")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn lazy_missing_named_components_do_not_add_discovery_errors() {
        let files = HashMap::from([
            (
                "index.crepus".into(),
                "if false\n  include cards.txt#Missing".into(),
            ),
            ("cards.txt".into(), "--- Available\nembed wave.ts".into()),
        ]);
        let islands = collect_web_islands(&files, "index.crepus").unwrap();
        assert_eq!(islands.len(), 1);
        assert_eq!(islands[0].src, "wave.ts");
    }

    #[test]
    fn virtual_suffix_cycles_are_depth_bounded_and_branching_has_a_budget() {
        let files = HashMap::from([(
            "index.crepus".into(),
            "include sub/index.crepus\nembed wave.ts".into(),
        )]);
        assert_eq!(
            collect_web_islands(&files, "index.crepus").unwrap().len(),
            1
        );

        let files = HashMap::from([(
            "index.crepus".into(),
            "include a/index.crepus\ninclude b/index.crepus".into(),
        )]);
        let error = collect_web_islands(&files, "index.crepus").unwrap_err();
        assert!(error.contains("exceeds 4096 states"), "{error}");
    }

    #[test]
    fn custom_tags_in_vue_and_astro_are_not_island_aliases() {
        for (path, source) in [
            (
                "entry.vue",
                "<template><crepus-island src=\"wave.ts\" /></template>",
            ),
            ("entry.astro", "<crepus-island src=\"wave.ts\" />"),
        ] {
            let files = HashMap::from([(path.into(), source.into())]);
            assert!(collect_web_islands(&files, path).unwrap().is_empty());
        }
    }

    #[test]
    fn collect_web_islands_finds_indent_and_component_islands() {
        let mut files = HashMap::new();
        files.insert(
            "index.crepus".to_string(),
            r#"div
  embed ./islands/a.ts adapter="module" title="A""#
                .to_string(),
        );
        files.insert(
            "ui.crepus".to_string(),
            r#"--- Card
div
  embed ./islands/b.tsx adapter="react" count={n}"#
                .to_string(),
        );

        let islands = collect_web_islands(&files, "index.crepus").unwrap();
        assert_eq!(islands.len(), 2);
        assert!(islands.iter().any(|i| i.src == "./islands/a.ts"));
        assert!(islands.iter().any(|i| i.src == "./islands/b.tsx"));
    }

    #[test]
    fn island_modules_are_not_reparsed_as_templates() {
        let files = HashMap::from([
            ("index.crepus".into(), "embed Widget.tsx adapter=\"react\"".into()),
            ("Widget.tsx".into(), "import React from 'react';\nexport default function Widget(){ return <button onClick={() => {}}>Hello</button>; }".into()),
        ]);
        let islands = collect_web_islands(&files, "index.crepus").unwrap();
        assert_eq!(islands.len(), 1);
        assert_eq!(islands[0].src, "Widget.tsx");
    }

    #[test]
    fn included_frontends_use_their_authored_filename() {
        for (path, content) in [
            ("Child.vue", "<template><p v-else>orphan</p></template>"),
            ("Child.astro", "<script>unsupported()</script><p />"),
        ] {
            assert!(crepuscularity_core::parser::parse_template_with_path(
                content,
                Some(Path::new(path))
            )
            .is_err());
            let files = HashMap::from([
                ("index.crepus".into(), format!("include {path}")),
                (path.into(), content.into()),
            ]);
            assert!(
                collect_web_islands(&files, "index.crepus").is_err(),
                "accepted {path}"
            );
        }
    }

    #[test]
    fn resolve_island_entry_rejects_parent_dir() {
        let err = resolve_island_entry(Path::new("."), "../outside.ts").unwrap_err();
        assert!(err.contains("outside site root"));
    }
    #[test]
    fn react_wrapper_preserves_mount_precedence_and_cleans_up_its_temp_file() {
        let directory = tempfile::tempdir().unwrap();
        let entry = directory.path().join("Authored \"quoted\" {name}.tsx");
        std::fs::write(
            &entry,
            "throw new Error('must not execute while wrapping');",
        )
        .unwrap();
        let wrapper = react_island_wrapper(&entry).unwrap();
        assert_eq!(wrapper.path().parent(), entry.parent());
        let path = wrapper.path().to_owned();
        let source = std::fs::read_to_string(&path).unwrap();
        let encoded_import = source
            .lines()
            .next()
            .unwrap()
            .strip_prefix("import * as authored from ")
            .unwrap()
            .strip_suffix(';')
            .unwrap();
        assert_eq!(
            serde_json::from_str::<String>(encoded_import).unwrap(),
            "./Authored \"quoted\" {name}.tsx"
        );
        assert!(source.contains("from \"react-dom/client\""));
        assert!(source.contains("root.render(createElement(component.default, props))"));
        assert!(source.contains("return () => root.unmount()"));
        assert!(
            source
                .find("return component.mount(element, props, context)")
                .unwrap()
                < source.find("const root = createRoot(element)").unwrap()
        );
        drop(wrapper);
        assert!(!path.exists());
        assert!(entry.exists());
    }

    #[test]
    fn conflicting_island_adapters_are_rejected_and_identical_refs_are_deduplicated() {
        let mut files = HashMap::new();
        files.insert(
            "index.crepus".into(),
            "div\n  embed ./Widget.tsx adapter=\"react\"\n  embed ./Widget.tsx adapter=\"react\""
                .into(),
        );
        assert_eq!(
            collect_web_islands(&files, "index.crepus").unwrap().len(),
            1
        );
        files.insert(
            "other.crepus".into(),
            "div\n  embed ./Widget.tsx adapter=\"module\"".into(),
        );
        let error = collect_web_islands(&files, "index.crepus").unwrap_err();
        assert!(error.contains("conflicting island adapters"), "{error}");
        assert!(error.contains("./Widget.tsx"), "{error}");
        assert!(
            error.contains("react") && error.contains("module"),
            "{error}"
        );
    }
}
