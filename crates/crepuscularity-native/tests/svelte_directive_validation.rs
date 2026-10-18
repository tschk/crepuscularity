#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn erased_directive_operands_still_validate_rune_placement() {
    for markup in [
        "<div use:action={$state(1)}/>",
        "<div {@attach $state(1)}/>",
        "<div use:action={$derived(1)}/>",
    ] {
        let source = format!("<script>function action(){{}}</script>{markup}");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error =
                compile_moonshine_svelte(&source, Path::new("Invalid.svelte"), target).unwrap_err();
            assert!(error.contains("misplaced Svelte rune"), "{error}");
        }
    }
}

#[test]
fn directive_validation_preserves_valid_state_and_erases_server_reads() {
    let source = "<script>let count=$state(1);function action(){};function attachment(){const $state=(value)=>value;const local=$state(2);return ()=>local;}</script><div use:action={count} {@attach attachment}/>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Valid.svelte"), target)
            .unwrap()
            .source;
        if matches!(target, MoonshineModuleTarget::Ssr) {
            assert!(!output.contains("installAction"), "{output}");
            assert!(!output.contains("installAttachment"), "{output}");
            assert!(!output.contains("__crepusDirective"), "{output}");
        }
    }
}
