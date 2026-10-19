#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn checked_models_use_independent_factories_with_typed_values() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for kind in ["checkbox", "radio"] {
            let source = format!(
                r#"<script setup lang="ts">import {{ref}} from 'vue'; const selected=ref([1]);const choice=ref(1);</script><template><input type="{kind}" v-model="selected" :value="choice" /></template>"#
            );
            let out = compile_moonshine_vue(&source, Path::new("App.vue"), target).unwrap();
            let helper = if matches!(target, MoonshineModuleTarget::Dom) {
                "createCheckedModel"
            } else {
                "createServerCheckedModel"
            };
            assert!(out.source.contains(helper), "{}", out.source);
            assert!(out.source.contains(kind), "{}", out.source);
            assert!(!out.source.contains("from 'vue'"), "{}", out.source);
        }
    }
}

#[test]
fn checked_models_keep_writability_and_static_type_guards() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for template in [
            r#"<input type="checkbox" v-model="props.selected" />"#,
            r#"<input :type="kind" v-model="selected" />"#,
            r#"<input type="checkbox" v-model.number="selected" />"#,
            r#"<input type="file" v-model="selected" />"#,
            r#"<input type="text" v-model="selected" value="conflict" />"#,
        ] {
            let source = format!(
                r#"<script setup lang="ts">import {{ref}} from 'vue';const props=defineProps<{{selected:boolean}}>();const selected=ref(false);const kind='checkbox';</script><template>{template}</template>"#
            );
            assert!(
                compile_moonshine_vue(&source, Path::new("App.vue"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn checked_ssr_context_distinguishes_single_roots_from_nested_and_fragment_inputs() {
    for (template, nested) in [
        (r#"<input type="checkbox" v-model="selected" />"#, false),
        (
            r#"<div><input type="checkbox" v-model="selected" /></div>"#,
            true,
        ),
        (
            r#"<input type="checkbox" v-model="selected" /><span>x</span>"#,
            true,
        ),
    ] {
        let source = format!(
            r#"<script setup>import {{ref}} from 'vue';const selected=ref(false);</script><template>{template}</template>"#
        );
        let out = compile_moonshine_vue(&source, Path::new("App.vue"), MoonshineModuleTarget::Ssr)
            .unwrap();
        assert!(
            out.source.contains(&format!(", {nested});")),
            "{}",
            out.source
        );
        assert!(
            !out.source.contains("createServerTextModel"),
            "{}",
            out.source
        );
    }
}

#[test]
fn mixed_models_use_distinct_hygienic_helpers_and_writable_setters() {
    let source = r#"<script setup>import {ref} from 'vue';const __crepusVueCheckedModel0=1;const selected=ref(false);const text=ref('');</script><template><div><input type="checkbox" v-model="selected"/><input type="text" v-model="text"/></div></template>"#;
    let out =
        compile_moonshine_vue(source, Path::new("App.vue"), MoonshineModuleTarget::Dom).unwrap();
    assert!(
        out.source
            .contains("createCheckedModel as __crepusVueCheckedModel1"),
        "{}",
        out.source
    );
    assert!(out.source.contains("createTextModel"), "{}", out.source);
    assert!(out.source.contains("selected.value ="), "{}", out.source);
}
