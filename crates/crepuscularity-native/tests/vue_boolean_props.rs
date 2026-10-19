#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

fn source(declarations: &str, props: &str) -> String {
    format!("<script setup lang=\"ts\">{declarations}const props=defineProps<{props}>();</script><template><p>{{{{props.flag}}}}</p></template>")
}

#[test]
fn boolean_metadata_preserves_direct_union_order_and_literal_types() {
    for (ty, expected) in [
        ("boolean", true),
        ("true", true),
        ("false", true),
        ("boolean|string", true),
        ("string|boolean", false),
        ("true|'yes'", true),
        ("'yes'|false", false),
        ("null|string|boolean|undefined", false),
        ("(boolean|string)", true),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(
                &source("", &format!("{{flag?:{ty}}}")),
                Path::new("Cast.vue"),
                target,
            )
            .unwrap();
            assert!(
                output.source.contains(&format!("[[\"flag\", {expected}]]")),
                "{ty}: {}",
                output.source
            );
        }
    }
}

#[test]
fn local_finite_interface_inheritance_retains_boolean_metadata() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_vue(
            &source(
                "interface Base{flag?:boolean|string}interface Props extends Base{label?:string}",
                "Props",
            ),
            Path::new("Cast.vue"),
            target,
        )
        .unwrap();
        assert!(
            output.source.contains("[[\"flag\", true]]"),
            "{}",
            output.source
        );
        assert!(
            output.source.contains("[\"label\", \"flag\"]"),
            "{}",
            output.source
        );
    }
}

#[test]
fn ambiguous_boolean_shapes_remain_explicitly_unsupported() {
    for (props, expected) in [
        ("{flag?:boolean & {}}", "intersected Boolean prop"),
        ("{flag?:boolean}&{flag?:string}", "duplicate Boolean prop"),
        ("{flag?:string}&{flag?:boolean}", "duplicate Boolean prop"),
        ("{flag?:boolean}&{flag?:boolean}", "duplicate Boolean prop"),
        ("{flag?:BooleanAlias}", "indirect prop value types"),
        ("{id?:boolean}", "Boolean id/class/style props"),
        ("{class?:boolean|string}", "Boolean id/class/style props"),
        ("{style?:string|boolean}", "Boolean id/class/style props"),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(
                &source("type BooleanAlias=boolean;", props),
                Path::new("Cast.vue"),
                target,
            )
            .unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }
}

#[test]
fn non_boolean_props_keep_two_argument_helper_call() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_vue(
            &source("", "{flag?:string}"),
            Path::new("Plain.vue"),
            target,
        )
        .unwrap();
        assert!(output.source.contains(", [\"flag\"])"), "{}", output.source);
    }
}

#[test]
fn imported_reexport_and_interface_base_preserve_boolean_order() {
    use crepuscularity_native::{compile_moonshine_vue_with_resolver, VueTypeModule};
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let mut resolver = |_: &Path, specifier: &str| {
            Ok(VueTypeModule {
            path: specifier.into(),
            source: match specifier {
                "./public" => "export type {Props as PublicProps} from './types';",
                "./types" => "interface Base{flag?:string|boolean}export interface Props extends Base{enabled?:boolean}",
                _ => return Err("unknown import".into()),
            }.into(),
        })
        };
        let output = compile_moonshine_vue_with_resolver(
            &source("import type {PublicProps} from './public';", "PublicProps"),
            Path::new("Imported.vue"),
            target,
            &mut resolver,
        )
        .unwrap();
        assert!(
            output
                .source
                .contains("[[\"enabled\", true], [\"flag\", false]]"),
            "{}",
            output.source
        );
    }
}

#[test]
fn spread_defaults_do_not_silently_bypass_the_macro_guard() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let source = "<script setup lang=\"ts\">const props=withDefaults(defineProps<{flag?:boolean}>(),{...{flag:true}});</script><template><p>{{props.flag}}</p></template>";
        let error = compile_moonshine_vue(source, Path::new("Defaults.vue"), target).unwrap_err();
        assert!(error.contains("spread defaults"), "{error}");
    }
}

#[test]
fn declared_kebab_component_inputs_are_canonicalized_but_collisions_reject() {
    use crepuscularity_native::{compile_moonshine_vue_with_resolver, VueTypeModule};
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for attr in [
            "flag-name",
            "flag-name=\"\"",
            "flag-name=\"flag-name\"",
            ":flag-name=\"false\"",
        ] {
            let mut resolver = |_: &Path, _: &str| {
                Ok(VueTypeModule {
                    path: "Child.vue".into(),
                    source: source("", "{flagName?:boolean}"),
                })
            };
            let parent=format!("<script setup lang=\"ts\">import Child from './Child.vue';const p=defineProps<{{}}>();</script><template><Child {attr}/></template>");
            let output = compile_moonshine_vue_with_resolver(
                &parent,
                Path::new("Parent.vue"),
                target,
                &mut resolver,
            )
            .unwrap();
            assert!(output.source.contains("flagName"), "{}", output.source);
            assert!(
                !output.source.contains("\"flag-name\":"),
                "{}",
                output.source
            );
        }
        for attrs in [
            "flagName=\"canonical\" flag-name=\"alias\"",
            "flag-name=\"alias\" flagName=\"canonical\"",
            ":unknown-name=\"false\"",
        ] {
            let mut resolver = |_: &Path, _: &str| {
                Ok(VueTypeModule {
                    path: "Child.vue".into(),
                    source: source("", "{flagName?:boolean}"),
                })
            };
            let parent=format!("<script setup lang=\"ts\">import Child from './Child.vue';const p=defineProps<{{}}>();</script><template><Child {attrs}/></template>");
            let error = compile_moonshine_vue_with_resolver(
                &parent,
                Path::new("Parent.vue"),
                target,
                &mut resolver,
            )
            .unwrap_err();
            assert!(error.contains("unsupported"), "{error}");
        }
    }
}

#[test]
fn boolean_metadata_does_not_silently_drop_literal_hyphen_declared_keys() {
    for props in [
        "{flag?:boolean;'foo-bar'?:string}",
        "{'flag-name'?:boolean}",
        "{flag?:boolean}&{'foo-bar'?:string}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(&source("", props), Path::new("Invalid.vue"), target)
                .unwrap_err();
            assert!(error.contains("hyphenated"), "{error}");
        }
    }
}

#[test]
fn literal_boolean_inputs_precast_without_widening_the_public_props_type() {
    use crepuscularity_native::{compile_moonshine_vue_with_resolver, VueTypeModule};
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let mut resolver = |_: &Path, _: &str| {
            Ok(VueTypeModule {
                path: "Child.vue".into(),
                source: source(
                    "",
                    "{disabled?:boolean;first?:boolean|string;last?:string|boolean}",
                ),
            })
        };
        let parent = "<script setup lang=\"ts\">import Child from './Child.vue';const props=defineProps<{}>();</script><template><section><Child disabled=\"\" first=\"\" last=\"\"/><Child disabled=\"disabled\" first=\"first\" last=\"last\"/></section></template>";
        let output = compile_moonshine_vue_with_resolver(
            parent,
            Path::new("Parent.vue"),
            target,
            &mut resolver,
        )
        .unwrap();
        let compact = output.source.split_whitespace().collect::<String>();
        assert!(
            compact.contains("disabled:{enumerable:true,get:()=>true}"),
            "{}",
            output.source
        );
        assert!(
            compact.contains("first:{enumerable:true,get:()=>true}"),
            "{}",
            output.source
        );
        assert!(
            compact.contains("last:{enumerable:true,get:()=>\"\"}"),
            "{}",
            output.source
        );
        assert!(
            compact.contains("last:{enumerable:true,get:()=>\"last\"}"),
            "{}",
            output.source
        );
    }
}

#[test]
fn boolean_name_literals_follow_ascii_word_boundaries_and_reject_unicode() {
    use crepuscularity_native::{compile_moonshine_vue_with_resolver, VueTypeModule};
    for (key, literal) in [
        ("foo$Bar", "foo$bar"),
        ("foo_Bar", "foo_-bar"),
        ("foo1Bar", "foo1-bar"),
        ("FlagName", "flag-name"),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let mut resolver = |_: &Path, _: &str| {
                Ok(VueTypeModule {
                    path: "Child.vue".into(),
                    source: source("", &format!("{{'{key}'?:boolean}}")),
                })
            };
            let parent=format!("<script setup lang=\"ts\">import Child from './Child.vue';const props=defineProps<{{}}>();</script><template><Child {key}=\"{literal}\"/></template>");
            let output = compile_moonshine_vue_with_resolver(
                &parent,
                Path::new("Parent.vue"),
                target,
                &mut resolver,
            )
            .unwrap();
            assert!(
                output
                    .source
                    .split_whitespace()
                    .collect::<String>()
                    .contains("get:()=>true"),
                "{}",
                output.source
            );
        }
    }
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let mut resolver = |_: &Path, _: &str| {
            Ok(VueTypeModule {
                path: "Child.vue".into(),
                source: source("", "{ÅName?:boolean}"),
            })
        };
        let parent="<script setup lang=\"ts\">import Child from './Child.vue';const props=defineProps<{}>();</script><template><Child ÅName=\"åname\"/></template>";
        let error = compile_moonshine_vue_with_resolver(
            parent,
            Path::new("Parent.vue"),
            target,
            &mut resolver,
        )
        .unwrap_err();
        assert!(
            error.contains("non-ASCII Boolean prop name literals"),
            "{error}"
        );
    }
}
