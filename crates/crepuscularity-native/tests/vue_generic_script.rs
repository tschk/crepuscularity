#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn generic_script_components_cannot_emit_unbound_type_parameters() {
    for generic in ["generic=\"T extends string\"", "generic=\"\""] {
        let source = format!("<script setup lang=\"ts\" {generic}>const props=defineProps<{{label:string}}>();function identity(value:T):T{{return value}}</script><template><p>{{{{props.label}}}}</p></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let result = compile_moonshine_vue(&source, Path::new("Generic.vue"), target);
            assert!(result.is_err(), "accepted {generic}");
            assert!(result.unwrap_err().contains("generic"));
        }
    }
}
