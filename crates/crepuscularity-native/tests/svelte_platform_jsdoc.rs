#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn generated_js_public_types_do_not_use_authored_global_namespaces() {
    for script in ["", " module"] {
        let source = format!("<script{script}>/** @typedef {{number}} globalThis.Parameters */\n/** @typedef {{number}} globalThis.ReturnType */\nconst value=1;</script><p>{{value}}</p>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let module =
                compile_moonshine_svelte(&source, Path::new("Types.svelte"), target).unwrap();
            assert!(
                module.source.contains(
                    "@param {import(\"@tschk/moonshine-dom/platform\").GlobalTypes.Parameters<"
                ),
                "{}",
                module.source
            );
            assert!(
                module.source.contains(
                    "@returns {import(\"@tschk/moonshine-dom/platform\").GlobalTypes.ReturnType<"
                ),
                "{}",
                module.source
            );
            assert!(!module.source.contains("@param {globalThis.Parameters"));
            assert!(!module.source.contains("@returns {globalThis.ReturnType"));
            assert!(
                module.source.contains("globalThis.Parameters"),
                "authored typedef must survive"
            );
            assert!(
                !module.source.contains("sourceRealm as"),
                "type-only namespace capture needs no runtime import"
            );
        }
    }
}
