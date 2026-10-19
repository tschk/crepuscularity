#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn terminal_else_with_nested_if_rejects_further_outer_branches() {
    for trailing in [
        "<p v-else>TRAILING</p>",
        "<p v-else-if=\"props.other\">TRAILING</p>",
    ] {
        let source = format!(
            "<script setup lang=\"ts\">const props = defineProps<{{outer: boolean; inner: boolean; other: boolean}}>();</script>\
             <template><p v-if=\"props.outer\">FIRST</p>\
             <template v-else><p v-if=\"props.inner\">NESTED</p></template>{trailing}</template>"
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(&source, Path::new("Invalid.vue"), target)
                .expect_err("a nested if cannot reopen the terminal outer else");
            assert!(error.contains("v-else"), "{error}");
        }
    }
}

#[test]
fn genuine_outer_chain_reaches_existing_native_control_boundary() {
    let source = r#"<script setup lang="ts">
const props = defineProps<{outer: boolean; middle: boolean; inner: boolean}>();
</script>
<template>
  <p v-if="props.outer">FIRST</p>
  <p v-else-if="props.middle">MIDDLE</p>
  <template v-else><p v-if="props.inner">NESTED</p><p v-else>INNER ELSE</p></template>
</template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let error = compile_moonshine_vue(source, Path::new("Valid.vue"), target)
            .expect_err("native authored Vue still rejects conditional template controls");
        assert_eq!(error, "unsupported native Vue template control/content");
    }
}

#[test]
fn plain_typed_props_component_still_compiles() {
    let source = r#"<script setup lang="ts">
const props = defineProps<{label: string}>();
</script><template><p>{{ props.label }}</p></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        compile_moonshine_vue(source, Path::new("Plain.vue"), target)
            .expect("plain typed-props authored Vue remains supported");
    }
}
