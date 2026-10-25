#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_vue_with_resolver, MoonshineModuleTarget, VueTypeModule,
};
use std::path::Path;
#[test]
fn declared_component_class_style_distinguishes_literal_and_expression() {
    let source = r#"<script setup lang="ts">import Child from './Child.vue';const props=defineProps<{}>();const classes=['active'];const styles={color:'blue'};</script><template><Child class="{classes}" style="color:red"/><Child :class="classes" :style="styles"/><Child :style="'color:red'"/></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let mut resolver = |_: &Path, _: &str| {
            Ok(VueTypeModule{path:"Child.vue".into(),source:r#"<script setup lang="ts">const props=defineProps<{class?:unknown;style?:unknown}>();</script><template><p>child</p></template>"#.into()})
        };
        let compiled = compile_moonshine_vue_with_resolver(
            source,
            Path::new("Parent.vue"),
            target,
            &mut resolver,
        )
        .expect("declared normalization");
        assert!(compiled.source.contains("@tschk/moonshine-vue/normalize"));
        assert!(compiled.source.contains("normalizeStaticStyle"));
        assert!(compiled.source.contains("normalizeClass"));
        assert!(compiled.source.contains("normalizeStyle"));
    }
}
#[test]
fn undeclared_props_and_intrinsic_style_remain_guarded() {
    for markup in [
        r#"<Child class="literal"/>"#,
        r#"<Child :style="styles"/>"#,
        r#"<p style="color:red"/>"#,
        r#"<Child ref="x"/>"#,
        r#"<Child key="x"/>"#,
    ] {
        let source = format!(
            r#"<script setup lang="ts">import Child from './Child.vue';const props=defineProps<{{}}>();const styles={{color:'red'}};</script><template>{markup}</template>"#
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let mut resolver = |_: &Path, _: &str| {
                Ok(VueTypeModule{path:"Child.vue".into(),source:r#"<script setup lang="ts">const props=defineProps<{}>();</script><template><p>child</p></template>"#.into()})
            };
            assert!(
                compile_moonshine_vue_with_resolver(
                    &source,
                    Path::new("Parent.vue"),
                    target,
                    &mut resolver
                )
                .is_err(),
                "{markup}"
            );
        }
    }
}
