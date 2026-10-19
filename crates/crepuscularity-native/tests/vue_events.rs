#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_vue, compile_moonshine_vue_with_resolver, MoonshineModuleTarget,
    VueTypeModule,
};
use std::path::Path;

#[test]
fn typed_events_use_raw_props_and_public_listener_types() {
    let source = r#"<script setup lang="ts">
const props=defineProps<{id:string}>();
const emit=defineEmits<{change:[amount:number,label:string];'item-change':[amount:number]}>();
function fire(){emit('change',1,props.id)}
</script><template><button @click="fire">{{props.id}}</button></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_vue(source, Path::new("Child.vue"), target).unwrap();
        assert!(compiled.source.contains("@tschk/moonshine-vue/events"));
        assert!(compiled.source.contains("EventListeners"));
        assert!(compiled.source.contains("createEmitter"));
        assert!(!compiled.source.contains("defineEmits"));
        assert!(compiled.source.contains("change"));
    }
}

#[test]
fn component_events_keep_dom_once_and_fuse_ssr_in_source_order() {
    let source = r#"<script setup lang="ts">import Child from './Child.vue';
const props=defineProps<{}>();function first(){}function second(){}function kebab(){}</script>
<template><Child @change="first" @change.once="second" @item-change="kebab"/></template>"#;
    let child = |_: &Path, _: &str| {
        Ok(VueTypeModule {path: "Child.vue".into(), source: "<script setup lang=\"ts\">const props=defineProps<{}>();const emit=defineEmits<{change:[];'item-change':[]}>();</script><template><p>child</p></template>".into()})
    };
    let dom = compile_moonshine_vue_with_resolver(
        source,
        Path::new("Parent.vue"),
        MoonshineModuleTarget::Dom,
        &mut child.clone(),
    )
    .unwrap()
    .source;
    assert!(dom.contains("onChangeOnce"));
    assert!(dom.contains("onItemChange"));
    let ssr = compile_moonshine_vue_with_resolver(
        source,
        Path::new("Parent.vue"),
        MoonshineModuleTarget::Ssr,
        &mut child.clone(),
    )
    .unwrap()
    .source;
    assert!(!ssr.contains("onChangeOnce"));
    assert!(ssr.contains("[first, second]"));
}

#[test]
fn unsupported_macros_event_types_and_handlers_fail_explicitly() {
    for declaration in [
        "const emit=defineEmits(['change']);",
        "const emit=defineEmits<{(event:'change',n:number):void}>();",
        "const emit=defineEmits<{change:number}>();",
        "const emit=defineEmits<{'foo-bar':[number];fooBar:[string]}>();",
        "const emit=defineEmits<{change:[];changeOnce:[number]}>();",
        "const emit=defineEmits<{change?:[]}>();",
        "const emit=defineEmits<{[key:string]:[]}>();",
        "const emit=defineEmits<{'two--parts':[]}>();",
        "const emit=defineEmits<{'événement':[]}>();",
        "const emit=defineEmits<{'update:bad:name':[]}>();",
        "const emit=defineEmits<{change:[typeof props.id]}>();",
        "const emit=defineEmits<{change:[]}>();const again=defineEmits<{change:[]}>();",
        "function create(){return defineEmits<{change:[]}>()}",
    ] {
        let source=format!("<script setup lang=\"ts\">const props=defineProps<{{id:string}}>();{declaration}</script><template><p>text</p></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_vue(&source, Path::new("Bad.vue"), target).is_err(),
                "{source}"
            );
        }
    }
    for (declaration, attribute) in [
        ("const handler=null;", "@change=\"handler\""),
        ("const handler=[()=>{}];", "@change=\"handler\""),
        ("let handler=()=>{};handler=()=>{};", "@change=\"handler\""),
        ("function handler(){}", "@change.capture=\"handler\""),
        ("function handler(){}", "@two--parts=\"handler\""),
    ] {
        let source=format!("<script setup lang=\"ts\">import Child from './Child.vue';const props=defineProps<{{}}>();{declaration}</script><template><Child {attribute}/></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_vue_with_resolver(&source, Path::new("Bad.vue"), target, &mut |_: &Path, _: &str| Ok(VueTypeModule {path:"Child.vue".into(),source:"<script setup lang=\"ts\">const props=defineProps<{}>();const emit=defineEmits<{change:[]}>();</script><template><p>child</p></template>".into()})).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn emitter_helpers_are_hygienic() {
    let source = r#"<script setup lang="ts">const props=defineProps<{}>();const __crepusVueEmitter0='occupied';const __crepusVueListeners0='occupied';const emit=defineEmits<{change:[]}>();</script><template><p>text</p></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_vue(source, Path::new("Hygiene.vue"), target).unwrap();
        assert!(compiled
            .source
            .contains("createEmitter as __crepusVueEmitter1"));
        assert!(compiled
            .source
            .contains("EventListeners as __crepusVueListeners1"));
    }
}

#[test]
fn undeclared_component_events_and_unknown_reserved_props_do_not_fall_through() {
    for markup in [
        "<Child @click=\"handler\"/>",
        "<Child id=\"literal\"/>",
        "<Child class=\"literal\"/>",
        "<Child style=\"color:red\"/>",
    ] {
        let source=format!("<script setup lang=\"ts\">import Child from './Child.vue';const props=defineProps<{{}}>();function handler(){{}}</script><template>{markup}</template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error=compile_moonshine_vue_with_resolver(&source,Path::new("Parent.vue"),target,&mut |_,_|Ok(VueTypeModule{path:"Child.vue".into(),source:"<script setup lang=\"ts\">const props=defineProps<{}>();</script><template><button>child</button></template>".into()})).unwrap_err();
            assert!(
                error.contains("fallthrough") || error.contains("reserved"),
                "{error}"
            );
        }
    }
}

#[test]
fn id_is_an_ordinary_prop_only_when_the_resolved_child_declares_it() {
    for markup in ["<Child id=\"literal\"/>", "<Child :id=\"props.name\"/>"] {
        let source=format!("<script setup lang=\"ts\">import Child from './Child.vue';const props=defineProps<{{name:string}}>();</script><template>{markup}</template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let mut calls = 0;
            let mut resolver = |_: &Path, _: &str| {
                calls += 1;
                Ok(VueTypeModule{path:"Child.vue".into(),source:"<script setup lang=\"ts\">const props=defineProps<{id:string}>();</script><template><p>{{props.id}}</p></template>".into()})
            };
            assert!(compile_moonshine_vue_with_resolver(
                &source,
                Path::new("Parent.vue"),
                target,
                &mut resolver
            )
            .is_ok());
            assert_eq!(calls, 1);
        }
    }
}

#[test]
fn declared_class_and_style_now_use_vue_normalization() {
    for markup in [
        "<Child class=\"literal\"/>",
        "<Child style=\"color:red\"/>",
        "<Child :class=\"props.name\"/>",
    ] {
        let source=format!("<script setup lang=\"ts\">import Child from './Child.vue';const props=defineProps<{{name:string}}>();</script><template>{markup}</template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(compile_moonshine_vue_with_resolver(&source,Path::new("Parent.vue"),target,&mut |_,_|Ok(VueTypeModule{path:"Child.vue".into(),source:"<script setup lang=\"ts\">const props=defineProps<{class:string;style:unknown}>();</script><template><p>child</p></template>".into()})).is_ok());
        }
    }
}

#[test]
fn child_metadata_matches_cli_case_insensitive_vue_extensions() {
    let source = r#"<script setup lang="ts">import Child from './Child.VUE';const props=defineProps<{}>();function handler(){}</script><template><Child @change="handler"/></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        assert!(compile_moonshine_vue_with_resolver(source,Path::new("Parent.vue"),target,&mut |_,_|Ok(VueTypeModule{path:"Child.VUE".into(),source:"<script setup lang=\"ts\">const props=defineProps<{}>();const emit=defineEmits<{change:[]}>();</script><template><p>child</p></template>".into()})).is_ok());
    }
}

#[test]
fn emit_only_children_need_no_artificial_props_macro_or_fallthrough() {
    let child = r#"<script setup lang="ts">const emit=defineEmits<{change:[number]}>();function fire(){emit('change',1)}</script><template><button @click="fire">emit</button></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_vue(child, Path::new("Child.vue"), target).unwrap();
        assert!(output.source.contains("EventListeners"));
        let parent = r#"<script setup lang="ts">import Child from './Child.vue';const props=defineProps<{}>();function handler(n:number){}</script><template><Child @change="handler"/></template>"#;
        assert!(compile_moonshine_vue_with_resolver(
            parent,
            Path::new("Parent.vue"),
            target,
            &mut |_, _| Ok(VueTypeModule {
                path: "Child.vue".into(),
                source: child.into()
            })
        )
        .is_ok());
        let bad = parent.replace("@change=\"handler\"", "id=\"unrecognized\"");
        assert!(compile_moonshine_vue_with_resolver(
            &bad,
            Path::new("Parent.vue"),
            target,
            &mut |_, _| Ok(VueTypeModule {
                path: "Child.vue".into(),
                source: child.into()
            })
        )
        .is_err());
    }
}
