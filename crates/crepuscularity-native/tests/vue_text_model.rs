#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

const TARGETS: [MoonshineModuleTarget; 2] =
    [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr];
const MODEL_MODULE: &str = "@tschk/moonshine-vue/model";

fn component(setup: &str, template: &str) -> String {
    format!("<script setup>{setup}</script><template>{template}</template>")
}

fn ref_component(template: &str) -> String {
    component("import {ref} from 'vue';const text=ref('');", template)
}

fn assert_compiles(source: &str, extra_dom: &[&str]) {
    for target in TARGETS {
        let output = compile_moonshine_vue(source, Path::new("Model.vue"), target)
            .unwrap_or_else(|error| panic!("{target:?}: {error}\n{source}"));
        let compact: String = output.source.split_whitespace().collect();
        assert!(output.has_default_export, "{}", output.source);
        assert!(
            !compact.contains("from\"vue\"") && !compact.contains("from'vue'"),
            "{}",
            output.source
        );
        match target {
            MoonshineModuleTarget::Dom => {
                assert!(output.source.contains(MODEL_MODULE), "{}", output.source);
                assert!(compact.contains("createTextModel"), "{}", output.source);
                for needle in extra_dom {
                    assert!(compact.contains(needle), "{needle}: {}", output.source);
                }
            }
            MoonshineModuleTarget::Ssr => {
                assert!(
                    output.source.contains("@tschk/moonshine-vue/model/ssr"),
                    "{}",
                    output.source
                );
                assert!(
                    !output.source.contains("bindTextModel"),
                    "{}",
                    output.source
                );
                assert!(
                    compact.contains("createServerTextModel"),
                    "{}",
                    output.source
                );
            }
        }
    }
}

fn assert_rejected(source: &str) {
    for target in TARGETS {
        let result = compile_moonshine_vue(source, Path::new("Model.vue"), target);
        assert!(
            result.is_err(),
            "{target:?} accepted:\n{source}\n{}",
            result.unwrap().source
        );
    }
}

#[test]
fn plain_models_compile_for_input_and_textarea() {
    for template in [
        r#"<input v-model="text">"#,
        r#"<input type="text" v-model="text" />"#,
        r#"<textarea v-model="text"></textarea>"#,
        r#"<textarea v-model="text" />"#,
    ] {
        assert_compiles(&ref_component(template), &[]);
    }
}

#[test]
fn supported_static_input_types_compile() {
    for kind in [
        "text", "search", "email", "url", "tel", "password", "number",
    ] {
        let template = format!(r#"<input type="{kind}" v-model="text">"#);
        assert_compiles(&ref_component(&template), &[]);
    }
}

#[test]
fn modifiers_compile_alone_and_combined() {
    for (template, needles) in [
        (r#"<input v-model.lazy="text">"#, &["lazy"][..]),
        (r#"<input v-model.trim="text">"#, &["trim"][..]),
        (
            r#"<input type="number" v-model.number="text">"#,
            &["number"][..],
        ),
        (
            r#"<textarea v-model.lazy.trim="text"></textarea>"#,
            &["lazy", "trim"][..],
        ),
        (
            r#"<input v-model.lazy.trim.number="text">"#,
            &["lazy", "trim", "number"][..],
        ),
    ] {
        assert_compiles(&ref_component(template), needles);
    }
}

#[test]
fn imported_ref_and_shallow_ref_targets_compile() {
    for setup in [
        "import {ref} from 'vue';const text=ref('');",
        "import {shallowRef} from 'vue';const text=shallowRef('');",
        "import {ref as r} from 'vue';const text=r('');",
        "import {shallowRef as s} from 'vue';const text=s('');",
        "import {ref} from 'vue';const text=ref('');function reset(){text.value=''}",
    ] {
        assert_compiles(&component(setup, r#"<input v-model="text">"#), &[]);
    }
    let ts = r#"<script setup lang="ts">import {ref} from 'vue';const text=ref<string>('');</script><template><textarea v-model.trim="text"></textarea></template>"#;
    assert_compiles(ts, &["trim"]);
}

#[test]
fn reactive_object_members_compile() {
    for (setup, target) in [
        (
            "import {reactive} from 'vue';const state=reactive({text:''});",
            "state.text",
        ),
        (
            "import {reactive as r} from 'vue';const state=r({text:''});",
            "state.text",
        ),
    ] {
        let template = format!(r#"<input v-model.trim="{target}">"#);
        assert_compiles(&component(setup, &template), &["trim"]);
    }
}

#[test]
fn multiple_models_share_one_model_import() {
    let source = component(
        "import {ref} from 'vue';const a=ref('');const b=ref('');",
        r#"<input v-model="a"><textarea v-model.lazy="b"></textarea>"#,
    );
    let output =
        compile_moonshine_vue(&source, Path::new("Model.vue"), MoonshineModuleTarget::Dom).unwrap();
    assert_eq!(
        output.source.matches(MODEL_MODULE).count(),
        1,
        "{}",
        output.source
    );
}

#[test]
fn ssr_never_emits_model_helper_for_any_modifier() {
    for template in [
        r#"<input v-model="text">"#,
        r#"<input v-model.lazy.trim="text">"#,
        r#"<textarea v-model.number="text"></textarea>"#,
    ] {
        let source = ref_component(template);
        let output =
            compile_moonshine_vue(&source, Path::new("Model.vue"), MoonshineModuleTarget::Ssr)
                .unwrap();
        assert!(
            !output.source.contains("bindTextModel"),
            "{}",
            output.source
        );
        assert!(
            output.source.contains("@tschk/moonshine-vue/model/ssr"),
            "{}",
            output.source
        );
        assert!(
            !output.imports.iter().any(|module| module == MODEL_MODULE),
            "{:?}",
            output.imports
        );
    }
}

#[test]
fn unsupported_elements_and_input_types_reject() {
    for template in [
        r#"<input type="file" v-model="text">"#,
        r#"<input type="hidden" v-model="text">"#,
        r#"<input type="bogus" v-model="text">"#,
        r#"<input :type="text" v-model="text">"#,
        r#"<input v-bind:type="text" v-model="text">"#,
        r#"<input v-bind="text" v-model="text">"#,
    ] {
        assert_rejected(&ref_component(template));
    }
}

#[test]
fn children_and_conflicting_value_bindings_reject() {
    for template in [
        r#"<textarea v-model="text">seed</textarea>"#,
        r#"<textarea v-model="text">{{ text }}</textarea>"#,
        r#"<textarea v-model="text"><b></b></textarea>"#,
        r#"<input v-model="text" value="seed">"#,
        r#"<input v-model="text" :value="text">"#,
        r#"<input v-model="text" v-bind:value="text">"#,
        r#"<input v-model="text" defaultValue="seed">"#,
        r#"<input v-model="text" :defaultValue="text">"#,
        r#"<textarea v-model="text" value="seed"></textarea>"#,
        r#"<input v-model="text" v-model.lazy="text">"#,
    ] {
        assert_rejected(&ref_component(template));
    }
}

#[test]
fn ref_key_and_content_directives_reject() {
    for template in [
        r#"<input v-model="text" ref="el">"#,
        r#"<input v-model="text" key="k">"#,
        r#"<input v-model="text" :key="text">"#,
        r#"<input v-model="text" v-text="text">"#,
        r#"<textarea v-model="text" v-html="text"></textarea>"#,
    ] {
        assert_rejected(&ref_component(template));
    }
}

#[test]
fn readonly_and_non_assignable_targets_reject() {
    for (setup, target) in [
        (
            "import {ref,computed} from 'vue';const a=ref('');const text=computed(()=>a.value);",
            "text",
        ),
        (
            "import {ref,computed} from 'vue';const state=ref('');const text=computed(()=>state.value);const box={text};",
            "box.text",
        ),
        ("const text='plain';", "text"),
        (
            "const state={text:''};",
            "state.text",
        ),
        (
            "import {ref} from 'vue';const text=ref('');const get=()=>text;",
            "get()",
        ),
        (
            "import {ref} from 'vue';const text=ref('');",
            "text + ''",
        ),
        (
            "import {reactive} from 'vue';const state=reactive({text:''});",
            "state?.text",
        ),
        (
            "import {reactive} from 'vue';const state=reactive({text:''});",
            "state.text?.value",
        ),
        (
            "import {reactive} from 'vue';const state=reactive({text:''});",
            "state",
        ),
        (
            "import {reactive} from 'vue';const state=reactive({text:''});",
            "state.read()",
        ),
        (
            "import {ref} from 'vue';const text=ref('');",
            "missing",
        ),
    ] {
        let template = format!(r#"<input v-model="{target}">"#);
        assert_rejected(&component(setup, &template));
    }
}

#[test]
fn props_targets_reject() {
    for source in [
        r#"<script setup>const props=defineProps(['name']);</script><template><input v-model="props.name"></template>"#,
        r#"<script setup lang="ts">const props=defineProps<{name:string}>();</script><template><textarea v-model="props.name"></textarea></template>"#,
        r#"<script setup lang="ts">const {name}=defineProps<{name:string}>();</script><template><input v-model="name"></template>"#,
    ] {
        assert_rejected(source);
    }
}

#[test]
fn shadowed_vue_imports_do_not_make_models_assignable() {
    for setup in [
        "function ref(v){return {value:v}};const text=ref('');",
        "import {ref} from './local';const text=ref('');",
        "import {computed as ref} from 'vue';const text=ref(()=>'');",
    ] {
        assert_rejected(&component(setup, r#"<input v-model="text">"#));
    }
}
