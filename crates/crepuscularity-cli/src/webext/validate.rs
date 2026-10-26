//! Validate only manifest fields that name packaged local assets.
use std::path::{Path, PathBuf};

use serde_json::Value;

pub(super) fn validate_manifest_assets(stage: &Path, json: &str) -> Result<(), String> {
    let manifest: Value =
        serde_json::from_str(json).map_err(|error| format!("manifest JSON: {error}"))?;
    let root = stage
        .canonicalize()
        .map_err(|error| format!("manifest asset root: {error}"))?;
    let mut assets = Vec::new();
    for pointer in ["/icons", "/action/default_icon", "/chrome_url_overrides"] {
        if let Some(value) = manifest.pointer(pointer) {
            if let Some(map) = value.as_object() {
                assets.extend(
                    map.values()
                        .filter_map(Value::as_str)
                        .map(|path| (path, false)),
                );
            } else if let Some(path) = value.as_str() {
                assets.push((path, false));
            }
        }
    }
    for pointer in [
        "/action/default_popup",
        "/options_page",
        "/options_ui/page",
        "/background/service_worker",
        "/background/page",
    ] {
        if let Some(path) = manifest.pointer(pointer).and_then(Value::as_str) {
            // An empty popup explicitly disables it.
            if pointer != "/action/default_popup" || !path.is_empty() {
                assets.push((path, false));
            }
        }
    }
    if let Some(paths) = manifest
        .pointer("/background/scripts")
        .and_then(Value::as_array)
    {
        assets.extend(
            paths
                .iter()
                .filter_map(Value::as_str)
                .map(|path| (path, false)),
        );
    }
    if let Some(scripts) = manifest.get("content_scripts").and_then(Value::as_array) {
        for script in scripts {
            for field in ["js", "css"] {
                if let Some(paths) = script.get(field).and_then(Value::as_array) {
                    assets.extend(
                        paths
                            .iter()
                            .filter_map(Value::as_str)
                            .map(|path| (path, false)),
                    );
                }
            }
        }
    }
    if let Some(groups) = manifest
        .get("web_accessible_resources")
        .and_then(Value::as_array)
    {
        for group in groups {
            if let Some(paths) = group.get("resources").and_then(Value::as_array) {
                assets.extend(
                    paths
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|path| (path, true)),
                );
            }
        }
    }
    for (asset, patterns) in assets {
        validate_asset(&root, asset, patterns)
            .map_err(|error| format!("manifest asset {asset:?}: {error}"))?;
    }
    Ok(())
}

fn decode_path(asset: &str) -> Result<String, String> {
    let path = asset.split(['?', '#']).next().unwrap_or("");
    let mut decoded = Vec::with_capacity(path.len());
    let mut bytes = path.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes.next().and_then(|byte| (byte as char).to_digit(16));
            let low = bytes.next().and_then(|byte| (byte as char).to_digit(16));
            let (Some(high), Some(low)) = (high, low) else {
                return Err("invalid percent encoding".into());
            };
            decoded.push((high * 16 + low) as u8);
        } else {
            decoded.push(byte);
        }
    }
    let path = String::from_utf8(decoded)
        .map_err(|_| "path is not UTF-8")?
        .replace('\\', "/");
    if path.contains('\0')
        || path.starts_with("//")
        || path
            .split('/')
            .next()
            .is_some_and(|part| part.contains(':'))
    {
        return Err("expected a local extension path".into());
    }
    Ok(path)
}

fn validate_asset(root: &Path, asset: &str, patterns: bool) -> Result<(), String> {
    let decoded = decode_path(asset)?;
    let mut relative = PathBuf::new();
    for part in decoded.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => return Err("path traversal is not allowed".into()),
            part => {
                relative.push(part);
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err("empty asset path".into());
    }
    let wildcard = decoded.contains('*');
    if wildcard && !patterns {
        return Err("wildcard is only allowed for web-accessible resources".into());
    }
    if wildcard {
        // Patterns need not match a file (default vendor/* may be empty). Check
        // their literal ancestor so symlinks cannot redirect it outside stage.
        let prefix = decoded
            .trim_start_matches('/')
            .split('/')
            .take_while(|part| !part.contains('*'));
        let mut ancestor = root.join(prefix.collect::<PathBuf>());
        loop {
            match std::fs::symlink_metadata(&ancestor) {
                Ok(_) => break,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if !ancestor.pop() {
                        return Err("invalid pattern ancestor".into());
                    }
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        let resolved = ancestor.canonicalize().map_err(|error| error.to_string())?;
        if !resolved.starts_with(root) {
            return Err("path escapes staged output".into());
        }
        return Ok(());
    }
    let resolved = root
        .join(relative)
        .canonicalize()
        .map_err(|error| format!("missing or unreadable file: {error}"))?;
    if !resolved.starts_with(root) {
        return Err("path escapes staged output".into());
    }
    if !resolved.is_file() {
        return Err("expected a regular file".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn checks_all_local_fields_and_preserves_url_suffixes_and_encoded_utf8() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("assets")).unwrap();
        std::fs::write(dir.path().join("assets/雪 page.html"), "page").unwrap();
        let path = "/assets/%E9%9B%AA%20page.html?mode=popup#top";
        let manifest = json!({"icons":{"16":path},"action":{"default_popup":path,"default_icon":{"32":path}},"options_page":path,"options_ui":{"page":path},"chrome_url_overrides":{"newtab":path},"background":{"service_worker":path,"scripts":[path]},"content_scripts":[{"js":[path],"css":[path],"matches":["https://example.com/*"]}],"web_accessible_resources":[{"resources":[path,"vendor/*"],"matches":["<all_urls>"]}],"homepage_url":"https://example.com/does-not-exist","host_permissions":["https://example.com/*"]});
        validate_manifest_assets(dir.path(), &manifest.to_string()).unwrap();
        validate_manifest_assets(
            dir.path(),
            &json!({"action":{"default_popup":""}}).to_string(),
        )
        .unwrap();
    }

    #[test]
    fn missing_local_fields_are_errors_including_nonempty_resource_literals() {
        let dir = tempfile::tempdir().unwrap();
        for manifest in [
            json!({"action":{"default_popup":"missing.html"}}),
            json!({"icons":{"16":"missing.png"}}),
            json!({"background":{"scripts":["missing.js"]}}),
            json!({"content_scripts":[{"css":["missing.css"]}]}),
            json!({"web_accessible_resources":[{"resources":["missing.txt"]}]}),
        ] {
            let error = validate_manifest_assets(dir.path(), &manifest.to_string()).unwrap_err();
            assert!(
                error.contains("manifest asset") && error.contains("missing"),
                "{error}"
            );
        }
    }

    #[test]
    fn traversal_malformed_encoding_and_nonlocal_paths_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        for path in [
            "../outside",
            "/%2e%2e/outside",
            "folder\\..\\outside",
            "https://example.com/a",
            "//host/a",
            "C:\\outside",
            "%ff",
            "%0",
            "%00",
            "../*/x",
        ] {
            assert!(
                validate_manifest_assets(
                    dir.path(),
                    &json!({"web_accessible_resources":[{"resources":[path]}]}).to_string()
                )
                .is_err(),
                "{path}"
            );
        }
        assert!(validate_manifest_assets(
            dir.path(),
            &json!({"background":{"service_worker":"vendor/*"}}).to_string()
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn literal_and_pattern_ancestors_cannot_escape_through_symlinks() {
        let stage = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("file.js"), "outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), stage.path().join("linked")).unwrap();
        for path in ["linked/file.js", "linked/*", "linked/missing/*"] {
            let error = validate_manifest_assets(
                stage.path(),
                &json!({"web_accessible_resources":[{"resources":[path]}]}).to_string(),
            )
            .unwrap_err();
            assert!(error.contains("escapes staged output"), "{error}");
        }
    }
}
