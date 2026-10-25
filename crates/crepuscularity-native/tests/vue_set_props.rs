#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;
#[test]
fn global_set_props_compile_without_boolean_casting() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let out=compile_moonshine_vue("<script setup lang=\"ts\">const props=defineProps<{values:Set<string>}>();</script><template><p>{{props.values.size}}</p></template>",Path::new("Child.vue"),target).unwrap();
        assert!(out.source.contains("Set<string>"), "{}", out.source);
        assert!(!out.source.contains("BooleanProps"), "{}", out.source);
    }
}
#[test]
fn local_set_bindings_do_not_bypass_boolean_metadata_validation() {
    for binding in [
        "type Set<T>=boolean;",
        "interface Set<T>{value:T}",
        "import type {Set} from './types';",
        "function outer(Set:unknown){}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source=format!("<script setup lang=\"ts\">{binding}const props=defineProps<{{values:Set<string>}}>();</script><template><p>x</p></template>");
            assert!(
                compile_moonshine_vue(&source, Path::new("Child.vue"), target).is_err(),
                "{source}"
            );
        }
    }
}
