#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn original_public_class_retention_fixture_separates_text_from_input_dependencies() {
    let source = include_str!("fixtures/ordered-updaters/public-class-original.svelte");
    let output = compile_moonshine_svelte(
        source,
        Path::new("Public.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert_eq!(
        output
            .matches("__crepusDirectiveBlock0_effect(() =>")
            .count(),
        3,
        "{output}"
    );
    assert_eq!(
        output
            .matches("withConsumer(__crepusDirectiveBlock0_consumer")
            .count(),
        3,
        "{output}"
    );
}

#[test]
fn throwing_or_disposing_getters_keep_registration_order_and_independent_jobs() {
    // Runtime callbacks may throw, dispose the mounted root, or mutate peers.
    // Each job must remain individually owned by the existing scheduler; do not
    // add wrappers that suppress those existing error/cleanup paths.
    let source = include_str!("fixtures/ordered-updaters/getter-controls.svelte");
    let output = compile_moonshine_svelte(
        source,
        Path::new("Getters.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert_eq!(
        output
            .matches("__crepusDirectiveBlock0_effect(() =>")
            .count(),
        2,
        "{output}"
    );
    let first_registration = output
        .find("parts: [__crepusSvelteTextReader0(() => __crepusPropUnbound(first.value)())]")
        .unwrap_or_else(|| panic!("{output}"));
    let second_read = output
        .find(" = (() => __crepusPropUnbound(second.value)())();")
        .unwrap_or_else(|| panic!("{output}"));
    let declaration = output[..second_read].rfind("const ").unwrap() + "const ".len();
    let next_value = output[declaration..second_read].trim();
    let apply = output
        .find(&format!("\"value\", {next_value})"))
        .unwrap_or_else(|| panic!("{output}"));
    assert!(
        first_registration < second_read && second_read < apply,
        "{output}"
    );
    // Check only this compiler-owned updater, not authored functions elsewhere.
    let updater_start = output[..second_read].rfind("= () => {").unwrap();
    let updater_end = apply + output[apply..].find("};").unwrap() + 2;
    assert!(
        !output[updater_start..updater_end].contains("catch"),
        "{output}"
    );
    let ssr = compile_moonshine_svelte(
        source,
        Path::new("Getters.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap()
    .source;
    assert!(!ssr.contains("readUpdates"), "{ssr}");
}
