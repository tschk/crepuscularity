#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_vue, compile_moonshine_vue_with_resolver, MoonshineModule,
    MoonshineModuleTarget, VueTypeModule,
};
use std::path::Path;

const TARGETS: [MoonshineModuleTarget; 2] =
    [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr];

fn compact(output: &MoonshineModule) -> String {
    output.source.split_whitespace().collect()
}

fn assert_native(source: &str, expected: &[&str]) {
    for target in TARGETS {
        let output = compile_moonshine_vue(source, Path::new("Setup.vue"), target).unwrap();
        let compact = compact(&output);
        assert!(output.has_default_export, "{}", output.source);
        assert!(!compact.contains("defineProps"), "{}", output.source);
        assert!(
            !compact.contains("from\"vue\"") && !compact.contains("from'vue'"),
            "{}",
            output.source
        );
        assert!(output.imports.iter().all(|module| module != "vue"
            && !module.starts_with("vue/")
            && !module.starts_with("@vue/")));
        for needle in expected {
            assert!(compact.contains(needle), "{needle}: {}", output.source);
        }
    }
}

#[test]
fn empty_and_const_setup_compile_with_an_empty_props_parameter() {
    for (source, needle) in [
        ("<script setup></script><template><p>Static</p></template>", "__crepusVueProps0:{}"),
        (
            "<script setup lang=\"ts\"></script><template><p>Static</p></template>",
            "__crepusVueProps0:{}",
        ),
        (
            "<script setup>const value=1;</script><template><p>{{value}}</p></template>",
            "constvalue=1",
        ),
        (
            "<script setup lang=\"ts\">const value:number=1;</script><template><p>{{value}}</p></template>",
            "__crepusVueProps0:{}",
        ),
    ] {
        assert_native(source, &[needle]);
    }
}

#[test]
fn counters_keep_stable_handlers_in_js_ts_and_aliased_setup() {
    for source in [
        r#"<script setup>import {ref,computed} from 'vue';const count=ref(0);const doubled=computed(()=>count.value*2);function increment(){count.value++}</script><template><button @click="increment">{{count}}/{{doubled}}</button></template>"#,
        r#"<script setup lang="ts">import {ref,computed} from 'vue';const count=ref<number>(0);const doubled=computed(():number=>count.value*2);const increment=():void=>{count.value++};</script><template><button @click="increment">{{count}}/{{doubled}}</button></template>"#,
        r#"<script setup lang="ts">import {ref as r,computed as c} from 'vue';const count=r(0);const doubled=c(()=>count.value*2);function increment(){count.value++}</script><template><button @click="increment">{{count}}/{{doubled}}</button></template>"#,
    ] {
        assert_native(
            source,
            &[
                "count.value++",
                "count.value*2",
                "increment",
                "__crepusVueProps0:{}",
            ],
        );
    }
}

#[test]
fn generated_props_parameter_avoids_authored_names() {
    for binding in [
        "const __crepusVueProps0=1;",
        r"const \u005f_crepusVueProps0=1;",
    ] {
        let source = format!(
            "<script setup lang=\"ts\">{binding}</script><template><p>Static</p></template>"
        );
        assert_native(&source, &["__crepusVueProps1:{}"]);
    }
}

#[test]
fn plain_imported_child_without_props_composes_but_does_not_fall_through() {
    let compile = |parent: &str, child: &'static str, target| {
        compile_moonshine_vue_with_resolver(
            parent,
            Path::new("Parent.vue"),
            target,
            &mut |_: &Path, _: &str| {
                Ok(VueTypeModule {
                    path: "Child.vue".into(),
                    source: child.into(),
                })
            },
        )
    };
    for child in [
        "<template><button>child</button></template>",
        "<script setup></script><template><button>child</button></template>",
    ] {
        for target in TARGETS {
            let parent = "<script setup lang=\"ts\">import Child from './Child.vue';</script><template><section><Child/></section></template>";
            let output = compile(parent, child, target).unwrap();
            assert!(output.imports.iter().any(|module| module == "./Child.vue"));
            assert!(output.source.contains("Child"), "{}", output.source);
            for markup in [
                "<Child id=\"id\"/>",
                "<Child class=\"class\"/>",
                "<Child style=\"color:red\"/>",
                "<Child :id=\"'id'\"/>",
                "<Child @click=\"handler\"/>",
            ] {
                let parent = format!("<script setup lang=\"ts\">import Child from './Child.vue';function handler(){{}}</script><template>{markup}</template>");
                let error = compile(&parent, child, target).unwrap_err();
                assert!(
                    error.contains("fallthrough") || error.contains("reserved"),
                    "{markup}: {error}"
                );
            }
        }
    }
}

#[test]
fn unsupported_setup_forms_keep_their_diagnostics_without_props() {
    for (body, expected) in [
        ("await Promise.resolve();", "async setup unsupported"),
        (
            "const value=await Promise.resolve(1);",
            "async setup unsupported",
        ),
        ("for await (const item of []) {}", "await"),
        ("defineExpose({}, {});", "defineExpose requires at most one"),
        ("const props=defineProps();", "typed identifier declaration"),
        (
            "const props=defineProps({label:String});",
            "typed identifier declaration",
        ),
        (
            "const a=defineProps<{}>();const b=defineProps<{}>();",
            "typed identifier declaration",
        ),
        (
            "const {label}=defineProps<{label:string}>();",
            "destructuring unsupported",
        ),
        (
            "defineProps<{label:string}>();",
            "top-level typed declaration",
        ),
        (
            "function load(){const props=defineProps<{label:string}>();}",
            "top-level typed declaration",
        ),
        (
            "const props=withDefaults(defineProps<{label?:string}>());",
            "withDefaults requires typed defineProps and one finite",
        ),
        (
            "const props=withDefaults(defineProps<{label?:string}>(),{other:'x'});",
            "unknown default key",
        ),
        (
            "const d={};const props=withDefaults(defineProps<{label?:string}>(),{...d});",
            "spread defaults",
        ),
        (
            "let props=withDefaults(defineProps<{label?:string}>(),{label:'x'});",
            "const props declaration",
        ),
        (
            "import {useAttrs} from 'vue';const attrs=useAttrs();",
            "unsupported native Vue helper useAttrs",
        ),
        (
            "import {ref} from 'vue/dist/vue.esm-bundler.js';",
            "host subpaths unsupported",
        ),
        (
            "import {ref} from '@vue/reactivity';",
            "host subpaths unsupported",
        ),
        ("export const value=1;", "setup exports unsupported"),
        ("export default {};", "setup exports unsupported"),
        ("export * from './other';", "setup exports unsupported"),
    ] {
        let source =
            format!("<script setup lang=\"ts\">{body}</script><template><p>x</p></template>");
        for target in TARGETS {
            let error = compile_moonshine_vue(&source, Path::new("Bad.vue"), target).unwrap_err();
            assert!(error.contains(expected), "{body}: {error}");
        }
    }
}
