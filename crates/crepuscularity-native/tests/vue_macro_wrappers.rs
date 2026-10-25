#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

fn source(statement: &str) -> String {
    format!("<script setup lang=\"ts\">const props=defineProps<{{label:string}}>();{statement}</script><template><p>{{{{props.label}}}}</p></template>")
}

fn wrappers(name: &str) -> Vec<String> {
    vec![
        format!("({name})"),
        format!("(({name}))"),
        format!("({name} as typeof {name})"),
        format!("{name}!"),
        format!("({name} satisfies typeof {name})"),
    ]
}

#[test]
fn unsupported_vue_macros_cannot_hide_inside_transparent_callee_wrappers() {
    for name in ["defineExpose", "defineEmits", "withDefaults"] {
        for callee in wrappers(name) {
            let statement = format!("{callee}({{}});");
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let error = compile_moonshine_vue(
                    &source(&statement),
                    Path::new("WrappedMacro.vue"),
                    target,
                )
                .unwrap_err();
                assert!(
                    error.contains("unsupported") && error.contains(name),
                    "{statement}: {error}"
                );
            }
        }
    }
}

#[test]
fn extra_define_props_calls_are_counted_through_transparent_wrappers() {
    for callee in wrappers("defineProps") {
        let statement = format!("{callee}();");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error =
                compile_moonshine_vue(&source(&statement), Path::new("ExtraProps.vue"), target)
                    .unwrap_err();
            assert!(error.contains("top-level"), "{statement}: {error}");
        }
    }
}
