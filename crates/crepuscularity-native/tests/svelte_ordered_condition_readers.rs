#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn ordered_condition_readers_cache_values_without_a_separate_structural_job() {
    let source = r#"<script lang="ts">let value=$state(1);function read(){return value}</script>{#if read()}<p>plain</p>{/if}{#if read() && value}<p>and</p>{/if}{#if read() || value}<p>or</p>{/if}{#if read() ?? value}<p>nullish</p>{/if}"#;
    let dom = compile_moonshine_svelte(
        source,
        Path::new("Conditions.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert!(
        dom.contains("createMemo as __crepusConditionMemo0"),
        "{dom}"
    );
    assert!(dom.contains("createRenderMemo as __crepusMemo0"), "{dom}");
    assert_eq!(
        dom.matches("__crepusChoose0(() => __crepusConditionMemo0(")
            .count(),
        4,
        "{dom}"
    );
    assert!(dom.contains("() => __crepusMemo0("), "{dom}");
    let ssr = compile_moonshine_svelte(
        source,
        Path::new("Conditions.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap()
    .source;
    assert!(!ssr.contains("__crepusConditionMemo"), "{ssr}");
}
