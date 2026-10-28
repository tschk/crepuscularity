#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn select_models_compile_typed_options_through_opt_in_helpers() {
    let source = r#"<script setup lang="ts">import {ref} from 'vue';const selected=ref(new Set([1]));const value=ref(1);</script><template><select multiple v-model.number="selected"><optgroup label="choices"><option :value="value">one</option><option value="2">two</option></optgroup></select></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out = compile_moonshine_vue(source, Path::new("App.vue"), target).unwrap();
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(out.source.contains("createSelectModel"), "{}", out.source);
            assert!(out.source.contains("createModelOption"), "{}", out.source);
            assert!(out.source.contains("selected.value ="), "{}", out.source);
        } else {
            assert!(
                out.source.contains("createServerModelOption"),
                "{}",
                out.source
            );
            assert!(!out.source.contains("createSelectModel"), "{}", out.source);
        }
        assert!(!out.source.contains("createTextModel"), "{}", out.source);
    }
}

#[test]
fn select_models_reject_readonly_targets_and_unverified_directives() {
    for template in [
        r#"<select v-model="props.value"><option>x</option></select>"#,
        r#"<select v-model.trim="value"><option>x</option></select>"#,
        r#"<select v-model="value" value="conflict"><option>x</option></select>"#,
        r#"<select><option>x</option></select>"#,
        r#"<select v-model="value"><div>invalid option</div></select>"#,
        r#"<select v-model="value"><select v-model="value"></select></select>"#,
    ] {
        let source = format!(
            r#"<script setup>import {{ref}} from 'vue';const value=ref('x');const props=defineProps<{{value:string}}>();</script><template>{template}</template>"#
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_vue(&source, Path::new("App.vue"), target).is_err(),
                "{source}"
            );
        }
    }
}
