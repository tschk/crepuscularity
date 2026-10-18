#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_vue_with_resolver, MoonshineModuleTarget, VueTypeModule,
};
use std::path::Path;
const CHILD: &str = r#"<script setup lang="ts">const props=defineProps<{title:string;count:number;firstName:string}>();const emit=defineEmits<{'update:title':[string];'update:count':[number];'update:firstName':[string]}>();</script><template><p>{{props.title}}</p></template>"#;
#[test]
fn named_multiple_models_preserve_canonical_keys_and_distinct_setters() {
    let source = r#"<script setup lang="ts">import {ref} from 'vue';import Child from './Child.vue';const title=ref('a');const count=ref(1);const name=ref('n');function before(value:string){}function after(value:number){}</script><template><Child @update:title="before" v-model:title="title" v-model:count="count" @update:count="after" v-model:first-name="name"/></template>"#;
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
        for key in [
            "onUpdate:title",
            "onUpdate:count",
            "onUpdate:firstName",
            "title.value =",
            "count.value =",
            "name.value =",
        ] {
            assert!(out.source.contains(key), "{}", out.source);
        }
        assert!(!out.source.contains("bind:"), "{}", out.source);
    }
}
#[test]
fn named_model_guards_keep_intrinsics_readonly_and_colliding_props_out() {
    for template in [
        r#"<input type="text" v-model:title="title"/>"#,
        r#"<input type="checkbox" v-model:title="title"/>"#,
        r#"<select v-model:title="title"><option>x</option></select>"#,
        r#"<Child v-model:title="props.title"/>"#,
        r#"<Child v-model:title.trim="title" :titleModifiers="{}"/>"#,
        r#"<Child v-model:first-name="name" v-model:firstName="name"/>"#,
        r#"<Child v-model:title="title" :title="title"/>"#,
    ] {
        let source = format!(
            r#"<script setup lang="ts">import {{ref}} from 'vue';import Child from './Child.vue';const props=defineProps<{{title:string}}>();const title=ref('a');const name=ref('n');</script><template>{template}</template>"#
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_vue_with_resolver(
                    &source,
                    Path::new("Parent.vue"),
                    target,
                    &mut |_, _| Ok(VueTypeModule {
                        path: "Child.vue".into(),
                        source: CHILD.into()
                    })
                )
                .is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn class_and_style_models_reuse_component_normalization() {
    let child = r#"<script setup lang="ts">const props=defineProps<{class:unknown;style:unknown;id:unknown}>();const emit=defineEmits<{'update:class':[unknown];'update:style':[unknown];'update:id':[unknown]}>();</script><template><p>{{props.class}}</p></template>"#;
    let source = r#"<script setup lang="ts">import {ref} from 'vue';import Child from './Child.vue';const classes=ref<unknown>({active:true});const styles=ref<unknown>({color:'red'});const id=ref('id');</script><template><Child v-model:class="classes" v-model:style="styles" v-model:id="id"/></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out = compile_moonshine_vue_with_resolver(
            source,
            Path::new("Parent.vue"),
            target,
            &mut |_, _| {
                Ok(VueTypeModule {
                    path: "Child.vue".into(),
                    source: child.into(),
                })
            },
        )
        .unwrap();
        for helper in if matches!(target, MoonshineModuleTarget::Dom) {
            ["normalizeClass as", "normalizeStyleDOM as"]
        } else {
            ["normalizeClassSSR as", "normalizeStyleSSR as"]
        } {
            assert!(out.source.contains(helper), "{}", out.source);
        }
    }
}

#[test]
fn model_props_and_once_listener_keys_cannot_alias_other_handlers() {
    for (child, template, expected) in [
        (
            r#"<script setup lang="ts">const props=defineProps<{onTitle:string}>();const emit=defineEmits<{'update:onTitle':[string];title:[string]}>();</script><template><p>x</p></template>"#,
            r#"<Child v-model:on-title="title" @title="observe"/>"#,
            "collides",
        ),
        (
            r#"<script setup lang="ts">const props=defineProps<{titleOnce:string}>();const emit=defineEmits<{'update:title':[string];'update:titleOnce':[string]}>();</script><template><p>x</p></template>"#,
            r#"<Child v-model:title-once="title" @update:title.once="observe"/>"#,
            "listener keys collide",
        ),
    ] {
        let source = format!(
            r#"<script setup lang="ts">import {{ref}} from 'vue';import Child from './Child.vue';const title=ref('x');function observe(value:string){{}}</script><template>{template}</template>"#
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue_with_resolver(
                &source,
                Path::new("Parent.vue"),
                target,
                &mut |_, _| {
                    Ok(VueTypeModule {
                        path: "Child.vue".into(),
                        source: child.into(),
                    })
                },
            )
            .expect_err("must reject");
            assert!(error.contains(expected), "{error}");
        }
    }
}
