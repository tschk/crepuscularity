#![cfg(feature = "web-modules")]

use crepuscularity_native::{
    compile_moonshine_vue, compile_moonshine_vue_with_resolver, MoonshineModuleTarget,
    VueTypeModule,
};
use std::path::Path;

const TARGETS: [MoonshineModuleTarget; 2] =
    [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr];

#[test]
fn vue_template_refs_lower_static_and_directive_string_refs_on_elements_and_components() {
    for markup in [
        r#"<div><input ref="inputEl"/><Child ref="childVm"/></div>"#,
        r#"<div><input :ref="'inputEl'"/><Child :ref="'childVm'"><span>slotted</span></Child></div>"#,
    ] {
        let source = format!(
            r#"<script setup lang="ts">import {{ref,shallowRef}} from 'vue';import Child from './Child.vue';const inputEl=ref<HTMLInputElement|null>(null);const childVm=shallowRef<unknown>(null);</script><template>{markup}</template>"#
        );
        let dom =
            compile_moonshine_vue(&source, Path::new("Parent.vue"), MoonshineModuleTarget::Dom)
                .expect("DOM template refs");
        assert!(dom.source.contains("bindRef"), "{}", dom.source);
        assert!(dom.source.contains("componentRef"), "{}", dom.source);
        assert!(dom.source.contains("[\"ref\"]:"), "{}", dom.source);
        assert!(dom.source.contains("cleanup: \"commit\""), "{}", dom.source);
        assert!(dom.source.contains("inputEl.value"), "{}", dom.source);
        assert!(dom.source.contains("childVm.value"), "{}", dom.source);

        let ssr =
            compile_moonshine_vue(&source, Path::new("Parent.vue"), MoonshineModuleTarget::Ssr)
                .expect("SSR template refs");
        assert!(!ssr.source.contains("bindRef"), "{}", ssr.source);
        assert!(!ssr.source.contains("componentRef"), "{}", ssr.source);
        assert!(!ssr.source.contains("[\"ref\"]"), "{}", ssr.source);
        assert!(
            !ssr.source
                .contains("@tschk/moonshine-dom/component-instance"),
            "{}",
            ssr.source
        );
        assert!(
            !ssr.source.contains("@tschk/moonshine-vue/expose"),
            "{}",
            ssr.source
        );
    }
}

#[test]
fn vue_template_refs_reject_non_ref_setup_targets_dynamic_refs_and_unsupported_hosts() {
    for (setup, markup) in [
        ("", r#"<input ref="missing"/>"#),
        (
            "import {ref} from 'vue';const inputEl=ref(null);",
            r#"<input ref=""/>"#,
        ),
        (
            "import {ref} from 'vue';let inputEl=ref(null);",
            r#"<input ref="inputEl"/>"#,
        ),
        (
            "import {computed} from 'vue';const inputEl=computed(()=>null);",
            r#"<input ref="inputEl"/>"#,
        ),
        (
            "import {reactive,toRef} from 'vue';const state=reactive({el:null});const inputEl=toRef(state,'el');",
            r#"<input ref="inputEl"/>"#,
        ),
        (
            "import {reactive} from 'vue';const inputEl=reactive({});",
            r#"<input ref="inputEl"/>"#,
        ),
        (
            "import {ref} from './local-ref';const inputEl=ref(null);",
            r#"<input ref="inputEl"/>"#,
        ),
        (
            "import {ref} from 'vue';const inputEl=ref(null);",
            r#"<input :ref="inputEl"/>"#,
        ),
        (
            "import {ref} from 'vue';const fn=(el:unknown)=>el;const inputEl=ref(null);",
            r#"<input :ref="fn"/>"#,
        ),
        (
            "import {ref} from 'vue';const a=ref(null);const b=ref(null);",
            r#"<input ref="a" :ref="'b'"/>"#,
        ),
        (
            "import {ref} from 'vue';const text=ref('');const inputEl=ref(null);",
            r#"<input v-model="text" ref="inputEl"/>"#,
        ),
        (
            "import {ref} from 'vue';const choice=ref('a');const selectEl=ref(null);",
            r#"<select v-model="choice" ref="selectEl"><option value="a">A</option></select>"#,
        ),
        (
            "import {ref} from 'vue';const choice=ref('a');const optEl=ref(null);",
            r#"<select v-model="choice"><option value="a" ref="optEl">A</option></select>"#,
        ),
        (
            "import {ref} from 'vue';const slotEl=ref(null);",
            r#"<slot ref="slotEl"/>"#,
        ),
        (
            "import {ref} from 'vue';const dynEl=ref(null);",
            r#"<component is="div" ref="dynEl"/>"#,
        ),
        (
            "import {ref} from 'vue';const tplEl=ref(null);",
            r#"<template ref="tplEl"><p>ok</p></template>"#,
        ),
    ] {
        let source = format!(
            r#"<script setup lang="ts">{setup}</script><template>{markup}</template>"#
        );
        for target in TARGETS {
            let mut resolver = |_: &Path, _: &str| {
                Ok(VueTypeModule {
                    path: "Child.vue".into(),
                    source: "<template><p>child</p></template>".into(),
                })
            };
            assert!(
                compile_moonshine_vue_with_resolver(
                    &source,
                    Path::new("InvalidRef.vue"),
                    target,
                    &mut resolver,
                )
                .is_err(),
                "{setup} | {markup} ({target:?})"
            );
        }
    }
}
