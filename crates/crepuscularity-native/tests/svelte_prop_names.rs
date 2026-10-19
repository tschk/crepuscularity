#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

fn compact(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn prop_cells_avoid_unicode_escaped_script_bindings() {
    let source = r#"<script lang="ts">const \u005f_crepusSvelteProp=()=>({value:'shadow'});let {label='correct'}=$props();</script><p>{label}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("PropNames.svelte"), target)
            .expect("an escaped ordinary binding must not collide with the prop helper");
        let generated = compact(&output.source);
        assert!(
            generated.contains("function__crepusSvelteProp_<"),
            "{}",
            output.source
        );
        assert!(
            generated.contains("label=__crepusSvelteProp_"),
            "{}",
            output.source
        );
    }
}

#[test]
fn rest_props_avoid_unicode_escaped_script_bindings() {
    let source = r#"<script lang="ts">const \u005f_crepusSvelteRest=()=>({title:'shadow'});let {...rest}=$props();</script><p>{rest.title}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("RestNames.svelte"), target)
            .expect("an escaped ordinary binding must not collide with the rest helper");
        let generated = compact(&output.source);
        assert!(
            generated.contains("function__crepusSvelteRest_("),
            "{}",
            output.source
        );
        assert!(
            generated.contains("rest=__crepusSvelteRest_("),
            "{}",
            output.source
        );
    }
}

#[test]
fn prop_call_helpers_avoid_unicode_escaped_template_parameters() {
    let source = r#"<script lang="ts">let {callback}=$props();</script><p>{((\u005f_crepusPropUnbound:any)=>callback())(()=> 'shadow')}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("CallNames.svelte"), target)
            .expect("template callback parameters must reserve generated call helper names");
        let generated = compact(&output.source);
        assert!(
            generated.contains("function__crepusPropUnbound_<"),
            "{}",
            output.source
        );
        assert!(
            generated.contains("__crepusPropUnbound_(callback.value)()"),
            "{}",
            output.source
        );
    }
}
