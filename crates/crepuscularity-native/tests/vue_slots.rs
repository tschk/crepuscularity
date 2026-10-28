#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;
#[test]
fn implicit_slot_transport_and_static_outlets_compile_both_targets() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let parent = r#"<script setup lang="ts">import Child from './Child.vue'; const props=defineProps<{label:string}>();</script><template><Child :label="props.label"><b>{{props.label}}</b></Child></template>"#;
        let child = r#"<script setup lang="ts">const props=defineProps<{label:string}>();</script><template><article><slot><i>{{props.label}}</i></slot><slot/></article></template>"#;
        let parent =
            compile_moonshine_vue(parent, Path::new("Parent.vue"), target).expect("implicit slot");
        let child =
            compile_moonshine_vue(child, Path::new("Child.vue"), target).expect("static outlet");
        assert!(parent.source.contains("withDefaultSlot"));
        assert!(child.source.contains("SlotOutlet"));
    }
}
#[test]
fn slot_names_scopes_and_component_listeners_remain_diagnostics() {
    for markup in [
        "<Child><template>inert</template></Child>",
        "<slot name=\"other\"/>",
        "<slot :name=\"props.label\"/>",
        "<slot :value=\"props.label\"/>",
        "<Child #default=\"item\">{{item}}</Child>",
        "<Child @click=\"handler\">body</Child>",
    ] {
        let source = format!(
            "<script setup lang=\"ts\">import Child from './Child.vue';const props=defineProps<{{label:string}}>();function handler(){{}}</script><template>{markup}</template>"
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_vue(&source, Path::new("Parent.vue"), target).is_err(),
                "{markup}"
            );
        }
    }
}

#[test]
fn slot_wrapper_and_outlet_names_avoid_authored_prefixes() {
    let source = r#"<script setup lang="ts">import Child from './Child.vue';const props=defineProps<{label:string}>();const __crepusVueOutlet0='zero';const __crepusVueOutlet00='zero-zero';const __crepusVueOutlet1='one';const __crepusVueOutlet20='twenty';</script><template><article><Child :label="props.label"><b>{{__crepusVueOutlet0}}/{{__crepusVueOutlet00}}/{{__crepusVueOutlet1}}/{{__crepusVueOutlet20}}</b></Child><slot/></article></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_vue(source, Path::new("Collision.vue"), target)
            .expect("hygienic wrapper and outlet");
        assert!(compiled.source.contains("__crepusVueOutlet30"));
        assert!(compiled.source.contains("__crepusVueOutlet31"));
        assert_eq!(
            compiled
                .source
                .matches("const __crepusVueOutlet0 =")
                .count(),
            1
        );
    }
}
