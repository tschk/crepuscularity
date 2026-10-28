#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn original_direct_member_and_prop_selectors_use_inline_dom_reconciliation() {
    for (name, source) in [
        (
            "dynamic.svelte",
            include_str!("fixtures/ordered-dynamic/dynamic.svelte"),
        ),
        (
            "dynamic-child.svelte",
            include_str!("fixtures/ordered-dynamic/dynamic-child.svelte"),
        ),
    ] {
        let dom = compile_moonshine_svelte(source, Path::new(name), MoonshineModuleTarget::Dom)
            .unwrap()
            .source;
        assert!(
            dom.contains("(component) => props.render(component), undefined, (read) => read)"),
            "{dom}"
        );
        let ssr = compile_moonshine_svelte(source, Path::new(name), MoonshineModuleTarget::Ssr)
            .unwrap()
            .source;
        assert!(!ssr.contains("props.render(component), undefined"), "{ssr}");
    }
}

#[test]
fn original_retired_children_keep_owned_destroy_callbacks() {
    for (name, source) in [
        (
            "dynamic-first.svelte",
            include_str!("fixtures/ordered-dynamic/dynamic-first.svelte"),
        ),
        (
            "dynamic-second.svelte",
            include_str!("fixtures/ordered-dynamic/dynamic-second.svelte"),
        ),
    ] {
        let dom = compile_moonshine_svelte(source, Path::new(name), MoonshineModuleTarget::Dom)
            .unwrap()
            .source;
        assert!(dom.contains("destroyed.push"), "{dom}");
        assert!(dom.contains("onDestroy"), "{dom}");
        assert!(!dom.contains("__crepusSvelteMapComponent"), "{dom}");
    }
}
