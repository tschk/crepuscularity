#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn ordinary_input_caches_raw_values_and_textarea_retains_its_special_helper() {
    let source = r#"<script lang="ts">let value=$state<string|undefined>();</script><input value={value}/><textarea value={value}></textarea><select value={value}><option>a</option></select><p style={{color:value}}>body</p>"#;
    let output = compile_moonshine_svelte(
        source,
        Path::new("Values.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert_eq!(
        output
            .matches("let __crepusDirectiveBlock0_valuePrevious")
            .count(),
        1,
        "{output}"
    );
    assert!(
        output.contains("!== (__crepusDirectiveBlock0_valueNext"),
        "{output}"
    );
    assert!(output.contains("__crepusTextareaUpdate("), "{output}");
    assert!(output.contains("\"style\", (() =>"), "{output}");
    let apply = output
        .find("\"value\", __crepusDirectiveBlock0_valueNext")
        .unwrap();
    assert!(
        apply < output[apply..].find("valuePrevious").unwrap() + apply,
        "{output}"
    );
    let ssr = compile_moonshine_svelte(
        source,
        Path::new("Values.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap()
    .source;
    assert!(!ssr.contains("valuePrevious"), "{ssr}");
}

#[test]
fn bound_values_keep_the_existing_binding_protocol() {
    let source = r#"<script lang="ts">let value=$state('a');</script><input bind:value/><textarea bind:value></textarea>"#;
    let output = compile_moonshine_svelte(
        source,
        Path::new("Bound.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert!(!output.contains("valuePrevious"), "{output}");
}
