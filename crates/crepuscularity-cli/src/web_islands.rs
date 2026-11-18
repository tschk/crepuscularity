use crepuscularity_core::ast::Node;
use crepuscularity_core::parser::{parse_component_file, parse_template};
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
) -> Result<Vec<WebIslandRef>, String> {
    let mut out = BTreeMap::new();
    for content in files.values() {
        let components = parse_component_file(content).map_err(|e| e.to_string())?;
        if components.components.is_empty() {
            let nodes = parse_template(content).map_err(|e| e.to_string())?;
            collect_nodes(&nodes, &mut out);
        } else {
            for component in components.components.values() {
                collect_nodes(&component.nodes, &mut out);
            }
        }
    }
    Ok(out.into_values().collect())
}

pub(crate) fn build_web_islands(
    site_dir: &Path,
    out_dir: &Path,
    files: &HashMap<String, String>,
) -> Result<(), String> {
    let refs = collect_web_islands(files)?;
    let manifest_path = out_dir.join("crepus-islands.json");
    if refs.is_empty() {
        if manifest_path.is_file() {
            let _ = std::fs::remove_file(&manifest_path);
        }
        return Ok(());
    }

    let island_out_dir = out_dir.join("islands");
    std::fs::create_dir_all(&island_out_dir).map_err(|e| format!("mkdir islands: {e}"))?;

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
        run_bun_build(&entry, &out_file)?;
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
    std::fs::write(&manifest_path, manifest_json)
        .map_err(|e| format!("write {}: {e}", manifest_path.display()))?;
    Ok(())
}

fn collect_nodes(nodes: &[Node], out: &mut BTreeMap<String, WebIslandRef>) {
    for node in nodes {
        match node {
            Node::Element(el) => collect_nodes(&el.children, out),
            Node::If(block) => {
                collect_nodes(&block.then_children, out);
                if let Some(else_children) = &block.else_children {
                    collect_nodes(else_children, out);
                }
            }
            Node::For(block) => collect_nodes(&block.body, out),
            Node::Match(block) => {
                for arm in &block.arms {
                    collect_nodes(&arm.body, out);
                }
            }
            Node::Include(include) => collect_nodes(&include.slot, out),
            Node::Embed(embed) => {
                if !out.contains_key(&embed.src) {
                    let adapter = embed
                        .adapter
                        .clone()
                        .unwrap_or_else(|| "module".to_string());
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

        let islands = collect_web_islands(&files).unwrap();
        assert_eq!(islands.len(), 2);
        assert!(islands.iter().any(|i| i.src == "./islands/a.ts"));
        assert!(islands.iter().any(|i| i.src == "./islands/b.tsx"));
    }

    #[test]
    fn resolve_island_entry_rejects_parent_dir() {
        let err = resolve_island_entry(Path::new("."), "../outside.ts").unwrap_err();
        assert!(err.contains("outside site root"));
    }
}
