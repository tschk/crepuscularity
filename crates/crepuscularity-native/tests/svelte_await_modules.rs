#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn authored_await_actions_are_consumed_inside_branch_template_groups() {
    let source = include_str!("fixtures/await-mixed/Source.svelte");
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Mixed.svelte"), target).unwrap();
        assert!(
            !compiled.source.contains("__crepusDirective0"),
            "{}",
            compiled.source
        );
    }
}

#[test]
fn authored_await_preserves_typed_receivers_and_decoded_helper_names() {
    let source = include_str!("fixtures/await-receivers.svelte");
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Receivers.svelte"), target)
            .expect("typed await calls and nested defaults must compile");
        assert!(compiled
            .imports
            .iter()
            .any(|path| path == "@tschk/moonshine-svelte/await"));
        assert!(
            compiled.source.contains("__crepusAwaitUnbound = 7"),
            "{}",
            compiled.source
        );
        assert!(
            compiled.source.contains("function __crepusAwaitUnbound_"),
            "{}",
            compiled.source
        );
    }
}
