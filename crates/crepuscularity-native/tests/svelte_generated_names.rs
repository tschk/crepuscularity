#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn exposure_helpers_avoid_decoded_script_bindings() {
    let source = r#"<script lang="ts">const \u005f_crepusSvelteExpose0=(...args:any[])=>{};export function read(){return 7}</script><p>ok</p>"#;
    let output = compile_moonshine_svelte(
        source,
        Path::new("Exposed.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap();
    let compact: String = output
        .source
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(
        compact.contains("exposeComponentas__crepusSvelteExpose1"),
        "{}",
        output.source
    );
}

#[test]
fn generated_props_avoid_decoded_bindings_in_both_script_scopes() {
    for script in [
        r#"<script lang="ts">const \u005f_crepusSvelteProps0='authored';</script>"#,
        r#"<script module lang="ts">const \u005f_crepusSvelteProps0='authored';</script>"#,
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = format!("{script}<p>ok</p>");
            let output =
                compile_moonshine_svelte(&source, Path::new("Props.svelte"), target).unwrap();
            assert!(
                output.source.contains("__crepusSvelteProps1"),
                "{}",
                output.source
            );
        }
    }
}

#[test]
fn generated_names_reserve_template_and_root_snippet_bindings() {
    for source in [
        r#"<script lang="ts">const value=7;</script><p>{((\u005f_crepusSvelteProps0:number)=>\u005f_crepusSvelteProps0)(value)}</p>"#,
        r#"<script lang="ts"></script>{#snippet item(\u005f_crepusSvelteProps0:number)}<p>{\u005f_crepusSvelteProps0}</p>{/snippet}{@render item(7)}"#,
        r#"<script lang="ts">const identity= <T>(value:T)=>{const \u005f_crepusSvelteProps0=value;return value};const value=identity(7);</script><p>{value}</p>"#,
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output =
                compile_moonshine_svelte(source, Path::new("Template.svelte"), target).unwrap();
            assert!(
                output.source.contains("__crepusSvelteProps1"),
                "{}",
                output.source
            );
        }
    }
}

#[test]
fn typed_snippet_parameters_require_authored_typescript_mode() {
    let source = r#"{#snippet item(\u005f_crepusSvelteProps0:number)}<p>{\u005f_crepusSvelteProps0}</p>{/snippet}{@render item(7)}"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let error = compile_moonshine_svelte(source, Path::new("JavaScript.svelte"), target)
            .expect_err("typed snippet parameters are not JavaScript");
        assert!(
            error.contains("invalid authored JavaScript Svelte template expression"),
            "{error}"
        );
    }
}
