#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;
const SOURCE: &str = include_str!("fixtures/vue/setup-counter-reference.vue");
#[test]
fn authored_fixture_uses_native_lexical_graph() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_vue(SOURCE, Path::new("Counter.vue"), target).unwrap();
        assert!(output.source.contains("@tschk/moonshine-vue/reactive"));
        assert!(output.source.contains("@tschk/moonshine-vue/text"));
        assert!(output.source.contains("count.value++"));
        assert!(!output.source.contains("defineProps"));
        assert!(!output.source.contains("from \"vue\""));
    }
}
#[test]
fn unsupported_constructs_fail_before_emission() {
    for (source, expected) in [
        (
            SOURCE.replace(
                "<template>",
                "<style scoped>button{color:red}</style><template>",
            ),
            "styles",
        ),
        (
            SOURCE
                .replace("<button", "<textarea")
                .replace("</button>", "</textarea>"),
            "text models require",
        ),
        (
            SOURCE.replace("@click=\"increment\"", "@click=\"increment()\""),
            "stable setup function",
        ),
        (
            SOURCE.replace("@click=\"increment\"", "@click.stop=\"increment\""),
            "event modifiers",
        ),
        (
            SOURCE.replace("const count =", "defineEmits();const count ="),
            "macro",
        ),
        (
            SOURCE.replace(
                "const count =",
                "function defineProps(){return {}};const count =",
            ),
            "unshadowed",
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error =
                compile_moonshine_vue(&source, Path::new("Counter.vue"), target).unwrap_err();
            assert!(error.contains(expected), "{expected}: {error}");
        }
    }
}

#[test]
fn rejects_dynamic_handlers_and_unsupported_public_types() {
    for source in [
        SOURCE
            .replace("function increment() {", "function ignored() {")
            .replace("const api =", "const increment = null;const api ="),
        SOURCE
            .replace("function increment() {", "function ignored() {")
            .replace("const api =", "const increment = [ignored];const api ="),
        SOURCE
            .replace("function increment() {", "function ignored() {")
            .replace("const api =", "const increment = ref(ignored);const api ="),
        SOURCE
            .replace("function increment() {", "function ignored() {")
            .replace("const api =", "let increment = ignored;const api ="),
        SOURCE.replace("const api =", "increment = () => {};const api ="),
        SOURCE
            .replace(
                "const props =",
                "type Props<T> = {id:string;initial:number;label:string};const props =",
            )
            .replace(
                "defineProps<{ id: string; initial: number; label: string }>()",
                "defineProps<Props<string>>()",
            ),
        SOURCE.replace(
            "defineProps<{ id: string; initial: number; label: string }>()",
            "defineProps<typeof api>()",
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(compile_moonshine_vue(&source, Path::new("Counter.vue"), target).is_err());
        }
    }
}
#[test]
fn rejects_entity_and_condensed_boundary_text_but_preserves_inline_spaces() {
    for template in [
        "<p>&lt;&#13;</p>",
        "<p title=\"&quot;\">x</p>",
        "<p> {{ count }} </p>",
        "<p>a  b</p>",
    ] {
        let source = SOURCE.replace("<template>", &format!("<template>{template}"));
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error =
                compile_moonshine_vue(&source, Path::new("Counter.vue"), target).unwrap_err();
            assert!(
                error.contains("entities") || error.contains("whitespace"),
                "{error}"
            );
        }
    }
    let source = SOURCE.replace(
        "<template>",
        "<template><p>a <span>b</span> c</p><p>{{ true }}:{{ ({x:1}) }}</p>",
    );
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        assert!(compile_moonshine_vue(&source, Path::new("Counter.vue"), target).is_ok());
    }
}

#[test]
fn accepts_stable_const_functions_and_preserves_imported_public_type() {
    for source in [
        SOURCE.replace("function increment() {", "const increment = () => {"),
        SOURCE.replace("function increment() {", "const increment = function() {"),
        SOURCE
            .replace(
                "import { ref, computed }",
                "import type { Props } from './props';import { ref, computed }",
            )
            .replace(
                "defineProps<{ id: string; initial: number; label: string }>()",
                "defineProps<Props>()",
            ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = crepuscularity_native::compile_moonshine_vue_with_resolver(
                &source,
                Path::new("Counter.vue"),
                target,
                &mut |_, _| {
                    Ok(crepuscularity_native::VueTypeModule {
                        path: "props.ts".into(),
                        source: "export interface Props {id:string;initial:number;label:string}"
                            .into(),
                    })
                },
            )
            .unwrap();
            assert!(!output.source.contains("Props0: any"));
            assert!(!output.source.contains("Props0: any ="));
        }
    }
}

#[test]
fn lowers_props_mutations_through_owned_props_view() {
    for mutation in [
        "props.initial=2;",
        "props.nested.count=2;",
        "props.initial++;",
        "delete props.initial;",
        "({value:props.initial}={value:2});",
        "[props.initial]=[2];",
    ] {
        let source = SOURCE.replace("const api =", &format!("{mutation}const api ="));
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(&source, Path::new("Counter.vue"), target).unwrap();
            assert!(output.source.contains("@tschk/moonshine-vue/props"));
            assert!(output.source.contains("const props = __crepusVuePropsView"));
        }
    }
    let source = SOURCE.replace(
        "const api =",
        "function unrelated(props:{initial:number}){props.initial=2;}const api =",
    );
    assert!(compile_moonshine_vue(
        &source,
        Path::new("Counter.vue"),
        MoonshineModuleTarget::Dom
    )
    .is_ok());
}

#[test]
fn permits_props_aliases_with_native_props_view() {
    for alias in [
        "const alias=props;alias.initial=2;",
        "let alias;alias=props;alias.initial=2;",
    ] {
        let source = SOURCE.replace("const api =", &format!("{alias}const api ="));
        let output = compile_moonshine_vue(
            &source,
            Path::new("Counter.vue"),
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert!(output.source.contains("@tschk/moonshine-vue/props"));
        assert!(output.source.contains("alias.initial = 2"));
    }
}

#[test]
fn scriptless_vue_components_compile_without_injected_setup() {
    for source in [
        "<template><p>Static child</p></template>",
        "<!-- <script setup>comment only</script> --><template><p title='<script>literal</script>'>Static child</p></template>",
        "<template></template>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(source, Path::new("Child.vue"), target).unwrap();
            assert!(output.has_default_export);
            assert!(output.has_module_declarations);
            assert!(!output.source.contains("defineProps"));
            assert!(!output.source.contains("script setup"));
            let host = match target {
                MoonshineModuleTarget::Dom => "@tschk/moonshine-dom/dom",
                MoonshineModuleTarget::Ssr => "@tschk/moonshine-dom/ssr",
            };
            assert!(output.source.contains(host), "{}", output.source);
        }
    }
}

#[test]
fn scriptless_admission_keeps_style_validation() {
    let source = "<style scoped>p{color:red}</style><template><p>Static</p></template>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let error = compile_moonshine_vue(source, Path::new("Child.vue"), target).unwrap_err();
        assert!(error.contains("styles"), "{error}");
    }
}

#[test]
fn scriptless_components_do_not_enable_vue_component_tags() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let error = compile_moonshine_vue(
            "<template><Child/></template>",
            Path::new("Parent.vue"),
            target,
        )
        .unwrap_err();
        assert!(error.contains("component/form/directive"), "{error}");
    }
}

#[test]
fn dynamic_vue_host_imports_cannot_bypass_the_native_bridge() {
    for module in [
        "vue",
        "vue/dist/vue.runtime.esm-bundler.js",
        "@vue/reactivity",
    ] {
        let source = format!("<script setup lang=\"ts\">const props=defineProps<{{label:string}}>();const load=()=>import({module:?});</script><template><p>{{{{props.label}}}}</p></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error =
                compile_moonshine_vue(&source, Path::new("Dynamic.vue"), target).unwrap_err();
            assert!(error.contains("dynamic Vue host imports"), "{error}");
        }
    }
}

#[test]
fn ordinary_dynamic_imports_remain_in_the_module_graph() {
    let source = "<script setup lang=\"ts\">const props=defineProps<{label:string}>();const load=()=>import('./helper');</script><template><p>{{props.label}}</p></template>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_vue(source, Path::new("Dynamic.vue"), target).unwrap();
        assert!(output.imports.iter().any(|source| source == "./helper"));
        assert!(
            output.source.contains("import(\"./helper\")"),
            "{}",
            output.source
        );
    }
}

#[test]
fn template_expressions_cannot_dynamically_import_vue_hosts() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let error = compile_moonshine_vue(
            "<template><p>{{ import('vue') }}</p></template>",
            Path::new("Dynamic.vue"),
            target,
        )
        .unwrap_err();
        assert!(error.contains("dynamic Vue host imports"), "{error}");
    }
}

#[test]
fn ordinary_modules_cannot_dynamically_import_vue_hosts() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let error = crepuscularity_native::compile_moonshine_module(
            "export const load=()=>import('@vue/reactivity');",
            Path::new("helper.ts"),
            target,
        )
        .unwrap_err();
        assert!(error.contains("dynamic Vue host imports"), "{error}");
    }
}

#[test]
fn explicit_value_imports_admit_default_and_named_component_tags() {
    for (import, tag) in [
        ("import Child from './Child.vue';", "Child"),
        ("import {Child as Alias} from './child.tsx';", "Alias"),
    ] {
        let source = format!("<script setup lang=\"ts\">{import}const props=defineProps<{{label?:string}}>();</script><template><{tag} label=\"Static\"/><{tag} :label=\"props.label\"/></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(&source, Path::new("Parent.vue"), target).unwrap();
            assert!(output.has_default_export);
            assert!(output.source.contains(tag), "{}", output.source);
            assert!(output.source.contains("Static"), "{}", output.source);
            assert!(output.source.contains(".label"), "{}", output.source);
            assert!(output.source.contains("Object.create"), "{}", output.source);
        }
    }
}

#[test]
fn imported_components_reject_unimplemented_vue_wrapper_and_slot_semantics() {
    for markup in [
        "<Child><template>Slot</template></Child>",
        "<Child @click=\"handler\"/>",
        "<Child id=\"id\"/>",
        "<Child class=\"class\"/>",
        "<Child :id=\"props.label\"/>",
        "<Child :class=\"props.label\"/>",
        "<Child :style=\"props.label\"/>",
        "<Child :children=\"props.label\"/>",
        "<Child v-model=\"props.label\"/>",
        "<Child ref=\"child\"/>",
        "<Child :key=\"props.label\"/>",
        "<Child :is=\"props.label\"/>",
        "<Child :innerHTML=\"props.label\"/>",
        "<Child :textContent=\"props.label\"/>",
        "<Child :foo-bar=\"props.label\"/>",
    ] {
        let source = format!("<script setup lang=\"ts\">import Child from './Child.vue';const props=defineProps<{{label:string}}>();function handler(){{}}</script><template>{markup}</template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error =
                compile_moonshine_vue(&source, Path::new("Parent.vue"), target).unwrap_err();
            assert!(
                error.contains("unsupported native Vue component"),
                "{markup}: {error}"
            );
        }
    }
}

#[test]
fn type_only_namespace_and_unimported_components_are_not_runtime_bindings() {
    for (import, tag) in [
        ("import type Child from './Child.vue';", "Child"),
        ("import {type Child} from './child.tsx';", "Child"),
        ("import * as UI from './child.tsx';", "UI"),
        ("import * as UI from './child.tsx';", "UI.Child"),
        ("import * as ui from './child.tsx';", "ui.Child"),
        ("import child from './Child.vue';", "child"),
        ("", "Child"),
    ] {
        let source = format!("<script setup lang=\"ts\">{import}const props=defineProps<{{label:string}}>();</script><template><{tag}/></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error =
                compile_moonshine_vue(&source, Path::new("Parent.vue"), target).unwrap_err();
            assert!(
                error.contains("unsupported native Vue component"),
                "{import}: {error}"
            );
        }
    }
}

#[test]
fn imported_runtime_values_are_unwrapped_in_arithmetic_and_attribute_expressions() {
    for import in [
        "import {count as value} from './state';",
        "import value from './state';",
    ] {
        let source = format!("<script setup lang=\"ts\">{import}const props=defineProps<{{label:string}}>();</script><template><p :data-value=\"value + 1\">{{{{value + 1}}}}</p></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(&source, Path::new("Imported.vue"), target).unwrap();
            // Find the actual imported helper alias rather than assume its fresh suffix.
            let compact: String = output.source.split_whitespace().collect();
            let helper = compact
                .split("unrefas")
                .nth(1)
                .unwrap()
                .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '$')
                .next()
                .unwrap();
            let read = format!("{helper}(value)+1");
            assert!(compact.matches(&read).count() >= 2, "{}", output.source);
            assert!(output.imports.iter().any(|path| path == "./state"));
        }
    }
}

#[test]
fn imported_value_unwrapping_respects_expression_parameter_shadowing_and_type_only_imports() {
    for import in [
        "import {count as value} from './state';",
        "import type {Value as value} from './state';",
        "import {type Value as value} from './state';",
    ] {
        let source = format!("<script setup lang=\"ts\">{import}const props=defineProps<{{label:string}}>();</script><template><p>{{{{(value => value + 1)(2)}}}}</p></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(&source, Path::new("Shadow.vue"), target).unwrap();
            let compact: String = output.source.split_whitespace().collect();
            assert!(compact.contains("(value)=>value+1"), "{}", output.source);
            if import.contains("type") {
                let source = format!("<script setup lang=\"ts\">{import}const props=defineProps<{{label:string}}>();</script><template><p>{{{{value}}}}</p></template>");
                let output =
                    compile_moonshine_vue(&source, Path::new("TypeOnly.vue"), target).unwrap();
                let compact: String = output.source.split_whitespace().collect();
                let helper = compact
                    .split("unrefas")
                    .nth(1)
                    .unwrap()
                    .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '$')
                    .next()
                    .unwrap();
                assert!(
                    !compact.contains(&format!("{helper}(value)")),
                    "{}",
                    output.source
                );
            }
        }
    }
}

#[test]
fn imported_and_setup_refs_expand_object_shorthand_without_rewriting_local_parameters() {
    let source = r#"<script setup lang="ts">import {value} from './state';const local=1;const props=defineProps<{label:string}>();</script><template><p>{{ ({value, local}) }}</p><p>{{ (value => ({value}))(2) }}</p></template>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_vue(source, Path::new("Shorthand.vue"), target).unwrap();
        let compact: String = output.source.split_whitespace().collect();
        assert!(
            compact.contains("value:__crepusVueUnref"),
            "{}",
            output.source
        );
        assert!(
            compact.contains("local:__crepusVueUnref"),
            "{}",
            output.source
        );
        assert!(compact.contains("(value)=>({value})"), "{}", output.source);
    }
}

#[test]
fn unwrapped_proto_shorthand_remains_an_own_property() {
    for declaration in [
        "import {__proto__} from './state';",
        "const __proto__={marker:1};",
    ] {
        let source = format!("<script setup lang=\"ts\">{declaration}const props=defineProps<{{label:string}}>();</script><template><p>{{{{Object.hasOwn({{__proto__}}, '__proto__')}}}}</p></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(&source, Path::new("Proto.vue"), target).unwrap();
            let compact: String = output.source.split_whitespace().collect();
            assert!(compact.contains("[\"__proto__\"]:"), "{}", output.source);
        }
    }
}

#[test]
fn async_event_functions_do_not_make_component_setup_async() {
    for handler in [
        "async function increment(){await Promise.resolve();count.value++;}",
        "const increment=async()=>{await Promise.resolve();count.value++;};",
    ] {
        let source = format!("<script setup lang=\"ts\">import {{ref}} from 'vue';const props=defineProps<{{label:string}}>();const count=ref(0);{handler}</script><template><button @click=\"increment\">{{{{count}}}}</button></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output =
                compile_moonshine_vue(&source, Path::new("AsyncHandler.vue"), target).unwrap();
            assert!(
                output.source.contains("await Promise.resolve()"),
                "{}",
                output.source
            );
            assert!(output.source.contains("count.value++"), "{}", output.source);
            assert!(output.source.contains("increment"), "{}", output.source);
        }
    }
}

#[test]
fn actual_top_level_await_and_nested_unsupported_macros_still_fail() {
    for statement in [
        "await Promise.resolve();",
        "if(true){await Promise.resolve();}",
    ] {
        let source = format!("<script setup lang=\"ts\">const props=defineProps<{{label:string}}>();{statement}</script><template><p>{{{{props.label}}}}</p></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error =
                compile_moonshine_vue(&source, Path::new("AsyncSetup.vue"), target).unwrap_err();
            assert!(error.contains("async setup unsupported"), "{error}");
        }
    }
    for statement in [
        "async function increment(){await Promise.resolve();defineExpose({});}",
        "const increment=async()=>{await Promise.resolve();defineEmits(['click']);};",
    ] {
        let source = format!("<script setup lang=\"ts\">const props=defineProps<{{label:string}}>();{statement}</script><template><button @click=\"increment\">{{{{props.label}}}}</button></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error =
                compile_moonshine_vue(&source, Path::new("NestedMacro.vue"), target).unwrap_err();
            assert!(
                error.contains("unsupported") && !error.contains("async setup"),
                "{error}"
            );
        }
    }
}

#[test]
fn imported_value_handler_identifiers_are_live_unwrapped_event_targets() {
    for import in [
        "import click from './handlers';",
        "import {click} from './handlers';",
        "import {onClick as click} from './handlers';",
    ] {
        let source = format!("<script setup lang=\"ts\">{import}const props=defineProps<{{label:string}}>();</script><template><button @click=\"click\">{{{{props.label}}}}</button></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output =
                compile_moonshine_vue(&source, Path::new("ImportedHandler.vue"), target).unwrap();
            assert!(output.imports.iter().any(|path| path == "./handlers"));
            let compact: String = output.source.split_whitespace().collect();
            let helper = compact
                .split("unrefas")
                .nth(1)
                .unwrap()
                .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '$')
                .next()
                .unwrap();
            assert!(
                compact.contains(&format!("{helper}(click)")),
                "{}",
                output.source
            );
            assert!(
                compact.contains("return__crepusVueEventHandler"),
                "{}",
                output.source
            );
        }
    }
}

#[test]
fn type_only_and_namespace_handler_imports_are_not_stable_runtime_functions() {
    for import in [
        "import type click from './handlers';",
        "import {type click} from './handlers';",
        "import * as click from './handlers';",
    ] {
        let source = format!("<script setup lang=\"ts\">{import}const props=defineProps<{{label:string}}>();</script><template><button @click=\"click\">{{{{props.label}}}}</button></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(&source, Path::new("InvalidHandler.vue"), target)
                .unwrap_err();
            assert!(error.contains("stable setup function"), "{error}");
        }
    }
}
