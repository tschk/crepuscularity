#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

const TARGETS: [MoonshineModuleTarget; 2] =
    [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr];

#[test]
fn vue_expose_lowers_in_dom_erases_runtime_in_ssr_and_brands_component_view_in_both_targets() {
    for source in [
        "<template><p>scriptless</p></template>",
        "<script setup lang=\"ts\">const props=defineProps<{label:string}>();</script><template><p>{{props.label}}</p></template>",
        "<script setup lang=\"ts\">defineExpose();</script><template><p>empty</p></template>",
        "<script setup lang=\"ts\">import {ref} from 'vue';const count=ref(1);function increment(){count.value++}defineExpose({count,increment});</script><template><button @click=\"increment\">{{count}}</button></template>",
    ] {
        let dom = compile_moonshine_vue(source, Path::new("Expose.vue"), MoonshineModuleTarget::Dom)
            .expect("DOM defineExpose");
        assert!(
            dom.source
                .contains("@tschk/moonshine-dom/component-instance"),
            "{}",
            dom.source
        );
        assert!(dom.source.contains("exposeComponent"), "{}", dom.source);
        assert!(
            dom.source.contains("@tschk/moonshine-vue/expose"),
            "{}",
            dom.source
        );
        assert!(dom.source.contains("exposeRefs"), "{}", dom.source);
        assert!(
            dom.source.contains("@tschk/moonshine-vue/app"),
            "{}",
            dom.source
        );
        assert!(dom.source.contains("ComponentView"), "{}", dom.source);
        assert!(!dom.source.contains("defineExpose"), "{}", dom.source);

        let ssr = compile_moonshine_vue(source, Path::new("Expose.vue"), MoonshineModuleTarget::Ssr)
            .expect("SSR defineExpose");
        assert!(
            !ssr.source
                .contains("@tschk/moonshine-dom/component-instance"),
            "{}",
            ssr.source
        );
        assert!(!ssr.source.contains("exposeComponent"), "{}", ssr.source);
        assert!(!ssr.source.contains("componentRef"), "{}", ssr.source);
        assert!(
            !ssr.source.contains("@tschk/moonshine-vue/expose"),
            "{}",
            ssr.source
        );
        assert!(!ssr.source.contains("exposeRefs"), "{}", ssr.source);
        assert!(
            ssr.source.contains("@tschk/moonshine-vue/app"),
            "{}",
            ssr.source
        );
        assert!(ssr.source.contains("ComponentView"), "{}", ssr.source);
        assert!(!ssr.source.contains("defineExpose"), "{}", ssr.source);
    }
}

#[test]
fn vue_expose_rejects_duplicate_spread_typed_optional_nested_and_value_positions() {
    for body in [
        "defineExpose({});defineExpose({});",
        "const args=[{}] as const;defineExpose(...args);",
        "defineExpose<{a:number}>({a:1});",
        "defineExpose?.({});",
        "defineExpose({}, {});",
        "const exposed=defineExpose({});",
        "void defineExpose({});",
        "function inner(){defineExpose({});}",
        "const inner=()=>{defineExpose({});};",
    ] {
        let source = format!(
            "<script setup lang=\"ts\">const props=defineProps<{{label?:string}}>();{body}</script><template><p>ok</p></template>"
        );
        for target in TARGETS {
            let error = compile_moonshine_vue(&source, Path::new("InvalidExpose.vue"), target)
                .expect_err(body);
            assert!(
                error.contains("defineExpose"),
                "{body} ({target:?}): {error}"
            );
        }
    }
}
