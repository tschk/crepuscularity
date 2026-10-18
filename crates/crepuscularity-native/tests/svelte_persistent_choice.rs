#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn ordered_choice_initializes_selection_inside_structural_range() {
    let source = r#"<script lang="ts">let shown=$state(true);</script><svg>{#if shown}<a><title>yes</title></a>{:else}<a><title>no</title></a>{/if}</svg>"#;
    let output = compile_moonshine_svelte(
        source,
        Path::new("Choice.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert!(
        output.contains("let view: ReturnType<typeof setup> | undefined"),
        "{output}"
    );
    assert!(output.contains("scheduler.branch(setup)"), "{output}");
    assert!(
        output.contains("createComponentScope as __crepusChoiceScope0"),
        "{output}"
    );
    assert!(output.contains("return view?.();"), "{output}");
    assert!(output.contains("__crepusChoicePrepare0(node);"), "{output}");
    assert!(output.contains("return () => node;"), "{output}");
    assert!(!output.contains("__crepusChoiceBranch"), "{output}");
    // SVG admission is DOM-specific; retain the ordinary SSR conditional contract.
    let ssr_source = r#"<script lang="ts">let shown=$state(true);</script>{#if shown}<p>yes</p>{:else}<p>no</p>{/if}"#;
    let ssr = compile_moonshine_svelte(
        ssr_source,
        Path::new("Choice.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap()
    .source;
    assert!(!ssr.contains("__crepusChoiceRange"), "{ssr}");
}
