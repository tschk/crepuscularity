#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_vue_with_resolver, MoonshineModuleTarget, VueTypeModule,
};
use std::path::Path;
const CHILD: &str = r#"<script setup lang="ts">const props=defineProps<{modelValue:string}>();const emit=defineEmits<{'update:modelValue':[value:string]}>();function change(){emit('update:modelValue','next')}</script><template><button @click="change">{{props.modelValue}}</button></template>"#;
#[test]
fn plain_component_models_use_existing_live_props_and_declared_events() {
    let source = r#"<script setup lang="ts">import {ref} from 'vue';import Child from './Child.vue';const value=ref('initial');function observe(value:string){}</script><template><Child v-model="value" @update:modelValue="observe"/></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out = compile_moonshine_vue_with_resolver(
            source,
            Path::new("Parent.vue"),
            target,
            &mut |_, _| {
                Ok(VueTypeModule {
                    path: "Child.vue".into(),
                    source: CHILD.into(),
                })
            },
        )
        .unwrap();
        assert!(out.source.contains("onUpdate:modelValue"), "{}", out.source);
        assert!(out.source.contains("value.value ="), "{}", out.source);
        assert!(out.source.contains("observe"), "{}", out.source);
    }
}
#[test]
fn component_model_contracts_and_writable_targets_are_required() {
    for (template, child) in [
        (r#"<Child v-model="props.value"/>"#, CHILD),
        (
            r#"<Child v-model.trim="value" :modelModifiers="{}"/>"#,
            CHILD,
        ),
        (r#"<Child v-model="value" modelValue="conflict"/>"#, CHILD),
        (
            r#"<Child v-model="value"/>"#,
            r#"<script setup lang="ts">const props=defineProps<{modelValue:string}>();</script><template><p>{{props.modelValue}}</p></template>"#,
        ),
        (
            r#"<Child v-model="value"/>"#,
            r#"<script setup lang="ts">const props=defineProps<{}>();const emit=defineEmits<{'update:modelValue':[string]}>();</script><template><p>x</p></template>"#,
        ),
    ] {
        let source = format!(
            r#"<script setup lang="ts">import {{ref}} from 'vue';import Child from './Child.vue';const props=defineProps<{{value:string}}>();const value=ref('x');</script><template>{template}</template>"#
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_vue_with_resolver(
                    &source,
                    Path::new("Parent.vue"),
                    target,
                    &mut |_, _| Ok(VueTypeModule {
                        path: "Child.vue".into(),
                        source: child.into()
                    })
                )
                .is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn component_model_callbacks_preserve_both_attribute_orders() {
    for model_first in [true, false] {
        let attrs = if model_first {
            r#"v-model="value" @update:modelValue="observe""#
        } else {
            r#"@update:modelValue="observe" v-model="value""#
        };
        let source = format!(
            r#"<script setup lang="ts">import {{ref}} from 'vue';import Child from './Child.vue';const value=ref('x');function observe(value:string){{}}</script><template><Child {attrs}/></template>"#
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = compile_moonshine_vue_with_resolver(
                &source,
                Path::new("Parent.vue"),
                target,
                &mut |_, _| {
                    Ok(VueTypeModule {
                        path: "Child.vue".into(),
                        source: CHILD.into(),
                    })
                },
            )
            .unwrap();
            let start = out.source.find("\"onUpdate:modelValue\"").unwrap();
            let callback = &out.source[start..];
            assert_eq!(
                callback.find("value.value =").unwrap() < callback.find("observe").unwrap(),
                model_first,
                "{}",
                out.source
            );
        }
    }
}

#[test]
fn component_model_once_listeners_keep_dom_once_and_ssr_source_order() {
    let source = r#"<script setup lang="ts">import {ref} from 'vue';import Child from './Child.vue';const value=ref('x');function observe(value:string){}</script><template><Child @update:modelValue.once="observe" v-model="value"/></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out = compile_moonshine_vue_with_resolver(
            source,
            Path::new("Parent.vue"),
            target,
            &mut |_, _| {
                Ok(VueTypeModule {
                    path: "Child.vue".into(),
                    source: CHILD.into(),
                })
            },
        )
        .unwrap();
        assert_eq!(
            out.source.contains("onUpdate:modelValueOnce"),
            matches!(target, MoonshineModuleTarget::Dom),
            "{}",
            out.source
        );
    }
}
