#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_vue, compile_moonshine_vue_with_resolver, MoonshineModuleTarget,
    VueTypeModule,
};
use std::path::Path;

const CHILD: &str = r#"<script setup lang="ts">const props=defineProps<{modelValue:string;title:string;fooBar:string}>();const emit=defineEmits<{'update:modelValue':[string];'update:title':[string];'update:foo-bar':[string]}>();</script><template><p>{{props.title}}</p></template>"#;
const RICH_CHILD: &str = r#"<script setup lang="ts">const props=defineProps<{modelValue:string;model:string;modelModifiers:string;title:string;on:string}>();const emit=defineEmits<{'update:modelValue':[string];'update:model':[string];'update:modelModifiers':[string];'update:title':[string];'update:on':[string];modifiers:[]}>();</script><template><p>{{props.title}}</p></template>"#;

fn parent(template: &str) -> String {
    format!(
        r#"<script setup lang="ts">import {{ref}} from 'vue';import Child from './Child.vue';const text=ref('a');const x=ref<unknown>({{trim:true}});function h(){{}}</script><template>{template}</template>"#
    )
}

fn compile(
    child: &'static str,
    template: &str,
    target: MoonshineModuleTarget,
) -> Result<String, String> {
    compile_moonshine_vue_with_resolver(
        &parent(template),
        Path::new("Parent.vue"),
        target,
        &mut |_, _| {
            Ok(VueTypeModule {
                path: "Child.vue".into(),
                source: child.into(),
            })
        },
    )
    .map(|module| module.source)
}

fn flat(source: &str) -> String {
    source
        .chars()
        .filter(|ch| !ch.is_whitespace() && !matches!(ch, '"' | '\''))
        .collect::<String>()
        .replace(",}", "}")
}

const TARGETS: [MoonshineModuleTarget; 2] =
    [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr];

#[test]
fn default_model_trim_number_use_model_modifiers_in_source_order() {
    for target in TARGETS {
        for (modifiers, object) in [
            ("trim", "{trim:true}"),
            ("number", "{number:true}"),
            ("trim.number", "{trim:true,number:true}"),
            ("number.trim", "{number:true,trim:true}"),
        ] {
            let out = compile(
                CHILD,
                &format!(r#"<Child v-model.{modifiers}="text"/>"#),
                target,
            )
            .unwrap();
            let out = flat(&out);
            assert!(out.contains(object), "{modifiers}: {out}");
            assert!(out.contains("modelModifiers"), "{modifiers}: {out}");
            assert!(!out.contains("modelValueModifiers"), "{modifiers}: {out}");
            assert!(out.contains("onUpdate:modelValue"), "{modifiers}: {out}");
        }
    }
}

#[test]
fn named_kebab_model_uses_canonical_modifier_key() {
    for target in TARGETS {
        let out = compile(CHILD, r#"<Child v-model:foo-bar.trim="text"/>"#, target).unwrap();
        let out = flat(&out);
        assert!(out.contains("fooBarModifiers"), "{out}");
        assert!(out.contains("{trim:true}"), "{out}");
        assert!(!out.contains("foo-barModifiers"), "{out}");
        assert!(!out.contains("modelModifiers"), "{out}");
        assert!(out.contains("onUpdate:fooBar"), "{out}");
        let out = flat(&compile(CHILD, r#"<Child v-model:title.number="text"/>"#, target).unwrap());
        assert!(out.contains("titleModifiers"), "{out}");
        assert!(out.contains("{number:true}"), "{out}");
    }
}

#[test]
fn custom_flags_keep_first_occurrence_order_and_dedupe_repeats() {
    for target in TARGETS {
        let out = flat(&compile(CHILD, r#"<Child v-model.b.a.b.trim.a="text"/>"#, target).unwrap());
        assert!(out.contains("{b:true,a:true,trim:true}"), "{out}");
        let out = flat(&compile(CHILD, r#"<Child v-model.capitalize="text"/>"#, target).unwrap());
        assert!(out.contains("{capitalize:true}"), "{out}");
        let out = flat(&compile(CHILD, r#"<Child v-model.trim.trim="text"/>"#, target).unwrap());
        assert!(out.contains("{trim:true}"), "{out}");
        assert!(!out.contains("trim:true,trim"), "{out}");
    }
}

#[test]
fn explicit_model_value_argument_is_distinct_from_default_model() {
    for target in TARGETS {
        for template in [
            r#"<Child v-model:modelValue.trim="text"/>"#,
            r#"<Child v-model:model-value.trim="text"/>"#,
        ] {
            let out = flat(&compile(CHILD, template, target).unwrap());
            assert!(out.contains("modelValueModifiers"), "{template}: {out}");
            assert!(out.contains("{trim:true}"), "{template}: {out}");
            assert!(!out.contains("modelModifiers"), "{template}: {out}");
            assert!(out.contains("onUpdate:modelValue"), "{template}: {out}");
        }
    }
}

#[test]
fn bare_models_emit_no_modifier_object() {
    for target in TARGETS {
        for template in [
            r#"<Child v-model="text"/>"#,
            r#"<Child v-model:title="text"/>"#,
            r#"<Child v-model:foo-bar="text"/>"#,
        ] {
            let out = compile(CHILD, template, target).unwrap();
            assert!(!out.contains("Modifiers"), "{template}: {out}");
        }
    }
}

#[test]
fn modifier_collisions_across_models_props_and_listeners_are_rejected() {
    for template in [
        r#"<Child v-model.trim="text" :modelModifiers="x"/>"#,
        r#"<Child v-model:title.trim="text" :titleModifiers="x"/>"#,
        r#"<Child v-model.trim="text" v-model:modelValue.trim="text"/>"#,
        r#"<Child v-model:model.trim="text" v-model.trim="text"/>"#,
        r#"<Child v-model:modelModifiers="text" v-model.trim="text"/>"#,
        r#"<Child v-model.trim="text" v-model:modelModifiers="text"/>"#,
        r#"<Child v-model:on.trim="text" @modifiers="h"/>"#,
    ] {
        for target in TARGETS {
            assert!(compile(RICH_CHILD, template, target).is_err(), "{template}");
        }
    }
    for target in TARGETS {
        let out = compile(
            RICH_CHILD,
            r#"<Child v-model:model.trim="text" v-model:title.number="text" v-model="text"/>"#,
            target,
        );
        assert!(out.is_ok(), "{out:?}");
        assert!(compile(
            RICH_CHILD,
            r#"<Child v-model="text" :modelModifiers="x"/>"#,
            target
        )
        .is_ok());
    }
}

#[test]
fn malformed_component_modifiers_and_intrinsic_rules_stay_rejected() {
    for template in [
        r#"<Child v-model..trim="text"/>"#,
        r#"<Child v-model.trim.="text"/>"#,
        r#"<Child v-model.__proto__="text"/>"#,
        r#"<input type="text" v-model.capitalize="text"/>"#,
        r#"<input type="text" v-model.trim.trim="text"/>"#,
        r#"<textarea v-model.number.number="text"></textarea>"#,
        r#"<input type="checkbox" v-model.trim="text"/>"#,
        r#"<select v-model.trim="text"><option>a</option></select>"#,
    ] {
        for target in TARGETS {
            assert!(compile(CHILD, template, target).is_err(), "{template}");
        }
    }
}

#[test]
fn public_props_type_adds_deterministic_modifier_objects_for_update_events() {
    let source = r#"<script setup lang="ts">const props=defineProps<{modelValue:string;fooBar:string}>();const emit=defineEmits<{'update:modelValue':[string];'update:foo-bar':[string];change:[]}>();</script><template><p>{{props.fooBar}}</p></template>"#;
    for target in TARGETS {
        let out = flat(
            &compile_moonshine_vue(source, Path::new("Child.vue"), target)
                .unwrap()
                .source,
        );
        for key in ["modelModifiers", "modelValueModifiers", "fooBarModifiers"] {
            assert!(
                out.contains(&format!("{key}?:Record<string,boolean|undefined>")),
                "{key}: {out}"
            );
        }
        assert!(!out.contains("changeModifiers"), "{out}");
        assert!(!out.contains("foo-barModifiers"), "{out}");
        assert!(out.contains("modelValue:string"), "{out}");
        let first = out.find("fooBarModifiers").unwrap();
        let second = out.find("modelModifiers").unwrap();
        let third = out.find("modelValueModifiers").unwrap();
        assert!(first < second && second < third, "{out}");
    }
    let plain = r#"<script setup lang="ts">const props=defineProps<{label:string}>();</script><template><p>{{props.label}}</p></template>"#;
    for target in TARGETS {
        let out = compile_moonshine_vue(plain, Path::new("Plain.vue"), target)
            .unwrap()
            .source;
        assert!(!out.contains("Modifiers"), "{out}");
    }
}
