//! Rust-side component catalog for the `crepus` CLI.
//!
//! Metadata is embedded from `catalog/components.json` (synced from
//! `plugins/crepuscularity-components/catalog/`). Full UI implementations for
//! the Moonshine/React target live in [`github.com/tschk/moonshine`](https://github.com/tschk/moonshine)
//! (`@tschk/moonshine-components`).

use std::sync::OnceLock;

use serde::Deserialize;

const CATALOG_JSON: &str = include_str!("../catalog/components.json");

/// Catalog entry exposed to the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentMeta {
    pub id: String,
    pub category: String,
    pub title: String,
    pub platforms: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    #[serde(default)]
    themes: Vec<String>,
    #[serde(default)]
    components: Vec<CatalogComponent>,
}

#[derive(Debug, Deserialize)]
struct CatalogComponent {
    id: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    platforms: Vec<String>,
}

struct ParsedCatalog {
    ids: Vec<&'static str>,
    themes: Vec<&'static str>,
    components: Vec<ComponentMeta>,
}

fn parsed() -> &'static ParsedCatalog {
    static CATALOG: OnceLock<ParsedCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let file: CatalogFile =
            serde_json::from_str(CATALOG_JSON).expect("embedded components.json must parse");

        let ids: Vec<&'static str> = file.components.iter().map(|c| leak_str(&c.id)).collect();

        let themes: Vec<&'static str> = if file.themes.is_empty() {
            // Fallback names matching catalog/themes/*.json
            vec![
                "dither-kit",
                "kumo",
                "night",
                "chalk",
                "aurora",
                "dawn",
                "zinc",
            ]
        } else {
            file.themes.iter().map(|t| leak_str(t)).collect()
        };

        let components = file
            .components
            .into_iter()
            .map(|c| {
                let title = c.title.or(c.name).unwrap_or_else(|| c.id.clone());
                ComponentMeta {
                    id: c.id,
                    category: c.category,
                    title,
                    platforms: c.platforms,
                }
            })
            .collect();

        ParsedCatalog {
            ids,
            themes,
            components,
        }
    })
}

fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}

/// All component ids from the embedded catalog.
pub fn component_ids() -> &'static [&'static str] {
    parsed().ids.as_slice()
}

/// Theme names from the embedded catalog.
pub fn theme_names() -> &'static [&'static str] {
    parsed().themes.as_slice()
}

/// Full component metadata (id, category, title, platforms).
pub fn list_components() -> Vec<ComponentMeta> {
    parsed().components.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_non_empty_and_include_known() {
        let ids = component_ids();
        assert!(!ids.is_empty(), "component_ids must not be empty");
        assert!(ids.contains(&"sparkline"), "missing sparkline");
        assert!(ids.contains(&"button"), "missing button");
    }

    #[test]
    fn themes_non_empty() {
        let themes = theme_names();
        assert!(!themes.is_empty());
        assert!(themes.contains(&"zinc") || themes.contains(&"dawn"));
    }

    #[test]
    fn list_components_matches_ids() {
        let list = list_components();
        assert_eq!(list.len(), component_ids().len());
        let button = list.iter().find(|c| c.id == "button").expect("button");
        assert!(!button.title.is_empty());
        assert!(!button.platforms.is_empty());
    }
}
