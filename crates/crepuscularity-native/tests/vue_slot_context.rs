#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;
#[test]
fn named_and_aliased_injection_helpers_use_existing_native_context() {
    for imports in ["import{provide,inject}from'vue';provide('token',props.label);const token=inject<string>('token');", "import{provide as offer,inject as read}from'vue';offer('token',props.label);const token=read<string>('token');"] {
        let source=format!("<script setup lang=\"ts\">const props=defineProps<{{label:string}}>();{imports}</script><template><b>{{{{token}}}}</b></template>");
        for target in [MoonshineModuleTarget::Dom,MoonshineModuleTarget::Ssr] {let compiled=compile_moonshine_vue(&source,Path::new("Context.vue"),target).expect("native context helpers");assert!(compiled.source.contains("@tschk/moonshine-vue/context"));assert!(!compiled.imports.iter().any(|value| value=="vue"));}
    }
}
#[test]
fn unrelated_vue_helpers_stay_explicitly_rejected() {
    for helper in ["useSlots", "useAttrs", "onActivated", "getCurrentInstance"] {
        let source=format!("<script setup lang=\"ts\">import{{{helper}}}from'vue';const props=defineProps<{{label:string}}>();</script><template><b>{{{{props.label}}}}</b></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_vue(&source, Path::new("Unsupported.vue"), target)
                    .unwrap_err()
                    .contains("unsupported native Vue helper")
            );
        }
    }
}
