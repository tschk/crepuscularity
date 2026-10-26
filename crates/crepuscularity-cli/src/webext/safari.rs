use std::collections::BTreeSet;

use serde_json::Value;

/// Correct only generated application targets, after validating every target's references.
pub(super) fn normalize_host_identifiers(
    project: &mut Value,
    bundle_id: &str,
) -> Result<bool, String> {
    let invalid =
        || "generated Safari project has an invalid target/configuration graph".to_string();
    let objects = project
        .get("objects")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    let mut hosts = BTreeSet::new();
    let mut others = BTreeSet::new();
    let mut found_host = false;
    for target in objects
        .values()
        .filter(|value| value.get("isa").and_then(Value::as_str) == Some("PBXNativeTarget"))
    {
        let product_type = target
            .get("productType")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        let is_host = product_type == "com.apple.product-type.application";
        found_host |= is_host;
        let list_id = target
            .get("buildConfigurationList")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        let list = objects.get(list_id).ok_or_else(invalid)?;
        if list.get("isa").and_then(Value::as_str) != Some("XCConfigurationList") {
            return Err(invalid());
        }
        let configs = list
            .get("buildConfigurations")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty())
            .ok_or_else(invalid)?;
        for config in configs {
            let id = config.as_str().ok_or_else(invalid)?;
            let config = objects.get(id).ok_or_else(invalid)?;
            if config.get("isa").and_then(Value::as_str) != Some("XCBuildConfiguration")
                || config
                    .get("buildSettings")
                    .and_then(Value::as_object)
                    .is_none()
            {
                return Err(invalid());
            }
            if is_host {
                hosts.insert(id.to_string());
            } else {
                others.insert(id.to_string());
            }
        }
    }
    if !found_host {
        return Err("generated Safari project has no application target".to_string());
    }
    if !hosts.is_disjoint(&others) {
        return Err(
            "generated Safari application shares a configuration with another target".to_string(),
        );
    }
    // All references are checked before changing any setting, including later configurations.
    let objects = project
        .get_mut("objects")
        .and_then(Value::as_object_mut)
        .ok_or_else(invalid)?;
    let mut changed = false;
    for id in hosts {
        let settings = objects
            .get_mut(&id)
            .and_then(|config| config.get_mut("buildSettings"))
            .and_then(Value::as_object_mut)
            .ok_or_else(invalid)?;
        if settings
            .get("PRODUCT_BUNDLE_IDENTIFIER")
            .and_then(Value::as_str)
            != Some(bundle_id)
        {
            settings.insert(
                "PRODUCT_BUNDLE_IDENTIFIER".to_string(),
                Value::String(bundle_id.to_string()),
            );
            changed = true;
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn project() -> Value {
        json!({"objects": {
            "app": {"isa":"PBXNativeTarget", "productType":"com.apple.product-type.application", "buildConfigurationList":"app-list"},
            "extension": {"isa":"PBXNativeTarget", "productType":"com.apple.product-type.app-extension", "buildConfigurationList":"ext-list"},
            "app-list": {"isa":"XCConfigurationList", "buildConfigurations":["debug", "release"]},
            "ext-list": {"isa":"XCConfigurationList", "buildConfigurations":["ext"]},
            "debug": {"isa":"XCBuildConfiguration", "buildSettings":{"PRODUCT_BUNDLE_IDENTIFIER":"wrong", "OTHER_SETTING":"keep"}},
            "release": {"isa":"XCBuildConfiguration", "buildSettings":{"PRODUCT_BUNDLE_IDENTIFIER":"wrong"}},
            "ext": {"isa":"XCBuildConfiguration", "buildSettings":{"PRODUCT_BUNDLE_IDENTIFIER":"dev.example.Extension"}}
        }})
    }

    #[test]
    fn updates_host_configurations_and_preserves_extension_and_noop() {
        let mut value = project();
        let extension = value["objects"]["ext"].clone();
        assert!(normalize_host_identifiers(&mut value, "dev.example").unwrap());
        for id in ["debug", "release"] {
            assert_eq!(
                value["objects"][id]["buildSettings"]["PRODUCT_BUNDLE_IDENTIFIER"],
                "dev.example"
            );
        }
        assert_eq!(
            value["objects"]["debug"]["buildSettings"]["OTHER_SETTING"],
            "keep"
        );
        assert_eq!(value["objects"]["ext"], extension);
        let before = value.clone();
        assert!(!normalize_host_identifiers(&mut value, "dev.example").unwrap());
        assert_eq!(value, before);
    }

    #[test]
    fn malformed_later_configuration_does_not_partially_update() {
        let mut value = project();
        value["objects"]["release"]["buildSettings"] = Value::Null;
        let before = value.clone();
        assert!(normalize_host_identifiers(&mut value, "dev.example").is_err());
        assert_eq!(value, before);
    }

    #[test]
    fn shared_application_extension_configuration_is_rejected() {
        let mut value = project();
        value["objects"]["ext-list"]["buildConfigurations"] = json!(["debug"]);
        let before = value.clone();
        assert!(normalize_host_identifiers(&mut value, "dev.example").is_err());
        assert_eq!(value, before);
    }

    #[test]
    fn missing_application_is_rejected() {
        let mut value = project();
        value["objects"].as_object_mut().unwrap().remove("app");
        assert!(normalize_host_identifiers(&mut value, "dev.example").is_err());
    }
}
