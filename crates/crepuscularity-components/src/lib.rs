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

impl CatalogComponent {
    fn display_name(&self) -> &str {
        self.title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
            .or_else(|| self.name.as_deref().filter(|name| !name.trim().is_empty()))
            .unwrap_or(&self.id)
    }
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
                let title = c.display_name().to_owned();
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
    fn catalog_metadata_is_nonempty_and_unambiguous() {
        let mut ids = std::collections::HashSet::new();
        for component in list_components() {
            assert!(!component.id.trim().is_empty());
            assert!(
                ids.insert(component.id.clone()),
                "duplicate id: {}",
                component.id
            );
            assert!(!component.title.trim().is_empty(), "{}", component.id);
            assert!(!component.category.trim().is_empty(), "{}", component.id);
            assert!(!component.platforms.is_empty(), "{}", component.id);
            let mut platforms = std::collections::HashSet::new();
            for platform in &component.platforms {
                assert!(!platform.trim().is_empty(), "{}", component.id);
                assert!(
                    platforms.insert(platform),
                    "duplicate platform: {}",
                    component.id
                );
            }
        }
        let mut themes = std::collections::HashSet::new();
        for theme in theme_names() {
            assert!(!theme.trim().is_empty());
            assert!(themes.insert(theme), "duplicate theme: {theme}");
        }
    }

    #[test]
    fn blank_titles_fall_back_to_name_or_id() {
        for title in ["", "   "] {
            let with_name: CatalogComponent = serde_json::from_value(
                serde_json::json!({"id": "button", "title": title, "name": "Button"}),
            )
            .unwrap();
            assert_eq!(with_name.display_name(), "Button");
            let without_name: CatalogComponent = serde_json::from_value(
                serde_json::json!({"id": "button", "title": title, "name": " "}),
            )
            .unwrap();
            assert_eq!(without_name.display_name(), "button");
        }
    }

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
