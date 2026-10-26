//! Scanner for detecting capability usage in .crepus templates.
//!
//! Scans templates to find browser API calls in expression slots and maps
//! them to required capabilities.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use crate::capabilities::{Capability, CapabilitySet};

/// A detected capability usage in source code.
#[derive(Clone, Debug)]
pub struct CapabilityUsage {
    pub capability: Capability,
    pub source_file: String,
    pub line: usize,
    pub api_call: String,
}

/// Map of browser API patterns to required capabilities.
fn api_capability_map() -> &'static HashMap<&'static str, Capability> {
    static MAP: OnceLock<HashMap<&'static str, Capability>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = HashMap::new();

        // Storage API
        map.insert("browser.storage", Capability::Storage);
        map.insert("chrome.storage", Capability::Storage);
        map.insert("storage.local", Capability::Storage);
        map.insert("storage.sync", Capability::Storage);

        // Messaging API
        map.insert("browser.runtime.sendMessage", Capability::Messaging);
        map.insert("chrome.runtime.sendMessage", Capability::Messaging);
        map.insert("browser.runtime.connect", Capability::Messaging);
        map.insert("chrome.runtime.connect", Capability::Messaging);
        map.insert("runtime.onMessage", Capability::Messaging);

        // Tabs API
        map.insert("browser.tabs", Capability::Tabs);
        map.insert("chrome.tabs", Capability::Tabs);

        // Notifications
        map.insert("browser.notifications", Capability::Notifications);
        map.insert("chrome.notifications", Capability::Notifications);

        // Clipboard
        map.insert("navigator.clipboard", Capability::Clipboard);
        map.insert("browser.clipboard", Capability::Clipboard);

        // History
        map.insert("browser.history", Capability::History);
        map.insert("chrome.history", Capability::History);

        // Bookmarks
        map.insert("browser.bookmarks", Capability::Bookmarks);
        map.insert("chrome.bookmarks", Capability::Bookmarks);

        // Downloads
        map.insert("browser.downloads", Capability::Downloads);
        map.insert("chrome.downloads", Capability::Downloads);

        // WebRequest
        map.insert("browser.webRequest", Capability::WebRequest);
        map.insert("chrome.webRequest", Capability::WebRequest);

        // Cookies
        map.insert("browser.cookies", Capability::Cookies);
        map.insert("chrome.cookies", Capability::Cookies);

        // Identity
        map.insert("browser.identity", Capability::Identity);
        map.insert("chrome.identity", Capability::Identity);

        // Scripting
        map.insert("browser.scripting", Capability::Scripting);
        map.insert("chrome.scripting", Capability::Scripting);

        // Alarms
        map.insert("browser.alarms", Capability::Alarms);
        map.insert("chrome.alarms", Capability::Alarms);

        // Context menus
        map.insert("browser.contextMenus", Capability::ContextMenus);
        map.insert("chrome.contextMenus", Capability::ContextMenus);
        map.insert("browser.menus", Capability::ContextMenus);

        // Geolocation
        map.insert("navigator.geolocation", Capability::Geolocation);

        map
    })
}

/// Scan a .crepus file for browser API usage in expression slots and return detected capabilities.
pub fn scan_crepus_for_capabilities(path: &Path) -> Result<Vec<CapabilityUsage>, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    let file_name = path.to_string_lossy().to_string();

    Ok(scan_content_for_capabilities(&content, &file_name))
}

/// Scan content string for API usage.
pub fn scan_content_for_capabilities(content: &str, file_name: &str) -> Vec<CapabilityUsage> {
    let api_map = api_capability_map();
    let mut usages = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        for (api_pattern, capability) in api_map {
            if line.contains(api_pattern) {
                usages.push(CapabilityUsage {
                    capability: capability.clone(),
                    source_file: file_name.to_string(),
                    line: line_num + 1,
                    api_call: api_pattern.to_string(),
                });
            }
        }
    }

    usages
}

/// Scan multiple files and aggregate capabilities.
pub fn scan_directory_for_capabilities(dir: &Path) -> Result<CapabilitySet, std::io::Error> {
    let mut set = CapabilitySet::new();

    let walker = ignore::WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(false)
        .build();

    for result in walker {
        let entry = match result {
            Ok(entry) => entry,
            Err(e) => {
                let msg = e.to_string();
                return Err(e
                    .into_io_error()
                    .unwrap_or_else(|| std::io::Error::other(msg)));
            }
        };
        let path = entry.path();

        if path.is_file() && path.extension().is_some_and(|ext| ext == "crepus") {
            let usages = scan_crepus_for_capabilities(path)?;
            for usage in usages {
                set.add(usage.capability);
            }
        }
    }

    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_storage_api() {
        let content = r#"
div panel
  button on-click={browser.storage.local.get('key')}
    "Load"
"#;

        let usages = scan_content_for_capabilities(content, "test.crepus");
        assert!(!usages.is_empty());
        assert!(usages.iter().any(|u| u.capability == Capability::Storage));
    }

    #[test]
    fn test_scan_multiple_apis() {
        let content = r#"
div
  button on-click={chrome.tabs.query({})}
    "Get tabs"
  button on-click={browser.notifications.create('id', {})}
    "Notify"
"#;

        let usages = scan_content_for_capabilities(content, "test.crepus");
        assert!(usages.iter().any(|u| u.capability == Capability::Tabs));
        assert!(usages
            .iter()
            .any(|u| u.capability == Capability::Notifications));
    }

    #[test]
    fn test_scan_crepus_for_capabilities() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        let content = r#"
div
  button on-click={browser.storage.local.get('key')}
    "Load"
"#;
        file.write_all(content.as_bytes()).unwrap();

        let usages = scan_crepus_for_capabilities(file.path()).unwrap();
        assert!(!usages.is_empty());
        assert!(usages.iter().any(|u| u.capability == Capability::Storage));
    }

    #[test]
    fn test_scan_crepus_for_capabilities_error() {
        let path = Path::new("non_existent_file_12345.crepus");
        let result = scan_crepus_for_capabilities(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_scan_directory_for_capabilities() {
        let dir = tempfile::tempdir().unwrap();

        let root_file = dir.path().join("root.crepus");
        std::fs::write(
            &root_file,
            "div on-click={browser.storage.local.get('key')}",
        )
        .unwrap();

        let non_crepus_file = dir.path().join("ignored.txt");
        std::fs::write(&non_crepus_file, "browser.tabs").unwrap();

        let sub_dir = dir.path().join("components");
        std::fs::create_dir(&sub_dir).unwrap();
        let sub_file = sub_dir.join("comp.crepus");
        std::fs::write(&sub_file, "div on-click={browser.notifications.create()}").unwrap();

        let capabilities = scan_directory_for_capabilities(dir.path()).unwrap();

        assert!(capabilities.has(&Capability::Storage));
        assert!(capabilities.has(&Capability::Notifications));
        assert!(!capabilities.has(&Capability::Tabs));
    }
}
