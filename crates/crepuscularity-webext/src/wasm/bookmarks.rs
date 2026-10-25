use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use super::core::{self, Result};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkTreeNode {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub index: Option<i64>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub date_added: Option<f64>,
    #[serde(default)]
    pub date_group_modified: Option<f64>,
    #[serde(default)]
    pub children: Option<Vec<BookmarkTreeNode>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkSearchQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

pub fn namespace() -> Result<JsValue> {
    core::namespace("bookmarks")
}

pub async fn search(query: &BookmarkSearchQuery) -> Result<Vec<BookmarkTreeNode>> {
    let bm = namespace()?;
    let value =
        core::call_method(&bm, "bookmarks.search", "search", &[core::to_js(query)?]).await?;
    core::from_js(value)
}

pub async fn get_tree() -> Result<Vec<BookmarkTreeNode>> {
    let bm = namespace()?;
    let value = core::call_method(&bm, "bookmarks.getTree", "getTree", &[]).await?;
    core::from_js(value)
}

pub async fn get_recent(num: usize) -> Result<Vec<BookmarkTreeNode>> {
    let bm = namespace()?;
    let value = core::call_method(
        &bm,
        "bookmarks.getRecent",
        "getRecent",
        &[JsValue::from_f64(num as f64)],
    )
    .await?;
    core::from_js(value)
}

pub fn flatten_tree(nodes: &[BookmarkTreeNode]) -> Vec<BookmarkTreeNode> {
    let mut flat = Vec::new();
    for node in nodes {
        if node.url.is_some() {
            flat.push(node.clone());
        }
        if let Some(ref children) = node.children {
            flat.extend(flatten_tree(children));
        }
    }
    flat
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flatten_tree_empty() {
        let nodes = vec![];
        let flat = flatten_tree(&nodes);
        assert!(flat.is_empty());
    }

    #[test]
    fn test_flatten_tree_only_folders() {
        let nodes = vec![
            BookmarkTreeNode {
                id: "1".to_string(),
                url: None,
                ..Default::default()
            },
            BookmarkTreeNode {
                id: "2".to_string(),
                url: None,
                children: Some(vec![BookmarkTreeNode {
                    id: "3".to_string(),
                    url: None,
                    ..Default::default()
                }]),
                ..Default::default()
            },
        ];
        let flat = flatten_tree(&nodes);
        assert!(flat.is_empty());
    }

    #[test]
    fn test_flatten_tree_with_bookmarks() {
        let nodes = vec![
            BookmarkTreeNode {
                id: "1".to_string(),
                url: None,
                children: Some(vec![
                    BookmarkTreeNode {
                        id: "2".to_string(),
                        url: Some("https://example.com".to_string()),
                        ..Default::default()
                    },
                    BookmarkTreeNode {
                        id: "3".to_string(),
                        url: None,
                        children: Some(vec![BookmarkTreeNode {
                            id: "4".to_string(),
                            url: Some("https://test.com".to_string()),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            },
            BookmarkTreeNode {
                id: "5".to_string(),
                url: Some("https://hello.com".to_string()),
                ..Default::default()
            },
        ];
        let flat = flatten_tree(&nodes);
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].id, "2");
        assert_eq!(flat[1].id, "4");
        assert_eq!(flat[2].id, "5");
    }
}
