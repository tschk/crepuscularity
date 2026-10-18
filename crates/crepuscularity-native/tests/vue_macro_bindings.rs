#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

fn source(prefix: &str, body: &str) -> String {
    format!("<script setup lang=\"ts\">{prefix}const props=(defineProps)<{{label:string}}>();{body}</script><template><p>{{{{props.label}}}}</p></template>")
}

#[test]
fn unbound_macro_values_cannot_escape_through_aliases_collections_or_type_queries() {
    for name in ["defineExpose", "defineEmits", "withDefaults", "defineProps"] {
        for body in [
            format!("const alias={name};"),
            format!("const list=[{name}];"),
            format!("const object={{{name}}};"),
            format!("const kind=typeof {name};"),
        ] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let error = compile_moonshine_vue(
                    &source("", &body),
                    Path::new("EscapedMacro.vue"),
                    target,
                )
                .unwrap_err();
                assert!(
                    error.contains("macro") && error.contains(name),
                    "{body}: {error}"
                );
            }
        }
    }
}

#[test]
fn resolved_macro_named_values_are_ordinary_lexical_bindings() {
    for (prefix, body) in [
        ("function defineExpose(){return 1}", "const value=defineExpose();"),
        ("import {ordinary as defineEmits} from './helpers';", "const value=defineEmits();"),
        ("", "function ordinary(defineProps,withDefaults){return [defineProps(),withDefaults()]};ordinary(()=>1,()=>2);"),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(&source(prefix, body), Path::new("ResolvedMacro.vue"), target).unwrap();
            let compact: String = output.source.split_whitespace().collect();
            if body.contains("defineExpose") { assert!(compact.contains("defineExpose()"), "{}", output.source); }
            if body.contains("defineEmits") { assert!(compact.contains("defineEmits()"), "{}", output.source); }
            if body.contains("withDefaults") {
                assert!(compact.contains("defineProps()"), "{}", output.source);
                assert!(compact.contains("withDefaults()"), "{}", output.source);
            }
        }
    }
}

#[test]
fn admitted_wrapped_props_remains_valid_but_a_shadowed_props_declaration_is_not_admitted() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        compile_moonshine_vue(&source("", ""), Path::new("Valid.vue"), target).unwrap();
        let error = compile_moonshine_vue(
            &source("function defineProps<T>(){return {} as T}", ""),
            Path::new("Shadow.vue"),
            target,
        )
        .unwrap_err();
        assert!(error.contains("unshadowed"), "{error}");
    }
}

#[test]
fn generated_helpers_avoid_escaped_setup_names() {
    for (binding, expected) in [
        (
            r"const \u005f_crepusVuePropsView0=(...args:any[])=>({label:'shadow'});",
            "as__crepusVuePropsView1",
        ),
        (
            r"const \u005f_crepusVueUnref0=(value:any)=>'shadow';",
            "as__crepusVueUnref1",
        ),
        (
            r"const \u005f_crepusVueText0=(value:any)=>'shadow';",
            "as__crepusVueText1",
        ),
        (
            r"const \u005f_crepusVueProps0='authored';",
            "__crepusVueProps1:",
        ),
        (r"void \u005f_crepusVueUnref0;", "as__crepusVueUnref1"),
        (
            r"const \u005f_crepusVueText00='stem';",
            "as__crepusVueText1",
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output =
                compile_moonshine_vue(&source(binding, ""), Path::new("Escaped.vue"), target)
                    .unwrap();
            let compact: String = output
                .source
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            assert!(compact.contains(expected), "{}", output.source);
        }
    }
}

#[test]
fn generated_helpers_avoid_escaped_template_bindings() {
    for template in [
        r"<p>{{ ((\u005f_crepusVueUnref0:any)=>props.label)(()=>({label:'shadow'})) }}</p>",
        r#"<p :title="((\u005f_crepusVueUnref0:any)=>props.label)(()=>({label:'shadow'}))">ok</p>"#,
    ] {
        let input = format!("<script setup lang=\"ts\">const props=defineProps<{{label:string}}>();</script><template>{template}</template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output =
                compile_moonshine_vue(&input, Path::new("TemplateBinding.vue"), target).unwrap();
            let compact: String = output
                .source
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            assert!(compact.contains("as__crepusVueUnref1"), "{}", output.source);
        }
    }
}
