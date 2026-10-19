#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;

fn source(imports: &str, body: &str) -> String {
    format!("<script setup lang=\"ts\">{imports}const props=defineProps<{{label:string}}>();{body}</script><template><p>{{{{props.label}}}}</p></template>")
}

#[test]
fn named_and_aliased_lifecycle_hooks_route_to_native_host_modules() {
    for (imports, body) in [
        ("import {onMounted,onBeforeUnmount,onUnmounted} from 'vue';", "onMounted(()=>{});onBeforeUnmount(()=>{});onUnmounted(()=>{});"),
        ("import {onMounted as mounted,onBeforeUnmount as before,onUnmounted as after,ref as cell,computed as derive} from 'vue';", "const count=cell(0);const doubled=derive(()=>count.value*2);mounted(()=>{});before(()=>{});after(()=>{});"),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(&source(imports, body), Path::new("Lifecycle.vue"), target).unwrap();
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output.source, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}: {}", parsed.errors, output.source);
            let mut hooks = Vec::new();
            let mut reactive = Vec::new();
            for statement in &parsed.program.body {
                let Statement::ImportDeclaration(import) = statement else { continue; };
                assert_ne!(import.source.value, "vue");
                for specifier in import.specifiers.iter().flatten() {
                    let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else { continue; };
                    let pair = (specifier.imported.name().to_string(), specifier.local.name.to_string());
                    if import.source.value == "@tschk/moonshine-vue/lifecycle" { hooks.push(pair); }
                    else if import.source.value == "@tschk/moonshine-vue/reactive" { reactive.push(pair); }
                }
            }
            let expected: Vec<(String, String)> = if imports.contains(" as mounted") {
                vec![("onMounted".into(),"mounted".into()),("onBeforeUnmount".into(),"before".into()),("onUnmounted".into(),"after".into())]
            } else {
                vec![("onMounted".into(),"onMounted".into()),("onBeforeUnmount".into(),"onBeforeUnmount".into()),("onUnmounted".into(),"onUnmounted".into())]
            };
            assert_eq!(hooks, expected);
            if imports.contains(" as mounted") {
                assert!(reactive.contains(&("ref".into(),"cell".into())));
                assert!(reactive.contains(&("computed".into(),"derive".into())));
            }
        }
    }
}

#[test]
fn unsupported_lifecycle_import_forms_remain_explicit_errors() {
    for imports in [
        "import {onUpdated} from 'vue';",
        "import {onServerPrefetch} from 'vue';",
        "import * as hooks from 'vue';",
        "import hooks from 'vue';",
        "import {onMounted} from 'vue/lifecycle';",
        "import {onMounted} from '@vue/runtime-core';",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(
                &source(imports, ""),
                Path::new("UnsupportedLifecycle.vue"),
                target,
            )
            .unwrap_err();
            assert!(
                error.contains("unsupported") || error.contains("named imports"),
                "{imports}: {error}"
            );
        }
    }
}

#[test]
fn native_lifecycle_registration_requires_exactly_one_non_spread_callback() {
    for (import, name) in [
        ("import {onMounted} from 'vue';", "onMounted"),
        ("import {onUnmounted as cleanup} from 'vue';", "cleanup"),
    ] {
        for arguments in ["", "()=>{}, null", "...callbacks"] {
            let body = format!("const callbacks=[()=>{{}}];{name}({arguments});");
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let error = compile_moonshine_vue(
                    &source(import, &body),
                    Path::new("LifecycleArguments.vue"),
                    target,
                )
                .unwrap_err();
                assert!(
                    error.contains("one callback"),
                    "{name}({arguments}): {error}"
                );
            }
        }
    }
}

#[test]
fn nested_parameters_shadowing_lifecycle_names_are_ordinary_calls() {
    let imports = "import {onMounted, onUnmounted as cleanup} from 'vue';";
    let body = "function ordinary(onMounted,cleanup){onMounted();cleanup(()=>{},null);onMounted(...[]);}ordinary(()=>{},()=>{});onMounted(()=>{});cleanup(()=>{});";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_vue(
            &source(imports, body),
            Path::new("LifecycleShadow.vue"),
            target,
        )
        .expect("local parameter calls must not be validated as imported lifecycle registration");
        let compact: String = output.source.split_whitespace().collect();
        assert!(compact.contains("onMounted()"), "{}", output.source);
        assert!(
            compact.contains("cleanup(()=>{},null)"),
            "{}",
            output.source
        );
        assert!(compact.contains("onMounted(...[])"), "{}", output.source);
    }
}

#[test]
fn transparent_callee_wrappers_preserve_lifecycle_argument_validation() {
    for callee in [
        "(onMounted)",
        "((onMounted))",
        "(onMounted as typeof onMounted)",
        "(onMounted satisfies typeof onMounted)",
        "onMounted!",
        "(((mounted as typeof mounted)!) satisfies typeof mounted)",
    ] {
        for arguments in ["", "()=>{}, null", "...callbacks"] {
            let body = format!("const callbacks=[()=>{{}}];{callee}({arguments});");
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let error = compile_moonshine_vue(
                    &source("import {onMounted,onMounted as mounted} from 'vue';", &body),
                    Path::new("WrappedLifecycle.vue"),
                    target,
                )
                .unwrap_err();
                assert!(
                    error.contains("one callback"),
                    "{callee}({arguments}): {error}"
                );
            }
        }
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let body = format!("{callee}(()=>{{}});");
            compile_moonshine_vue(
                &source("import {onMounted,onMounted as mounted} from 'vue';", &body),
                Path::new("ValidWrappedLifecycle.vue"),
                target,
            )
            .expect("one callback remains valid through transparent wrappers");
        }
    }
}

#[test]
fn wrapped_shadowed_lifecycle_parameters_keep_ordinary_call_semantics() {
    let body = "function ordinary(onMounted){((onMounted))();(onMounted as typeof onMounted)(()=>{},null);onMounted!(...[]);}ordinary(()=>{});onMounted(()=>{});";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        compile_moonshine_vue(
            &source("import {onMounted} from 'vue';", body),
            Path::new("WrappedShadow.vue"),
            target,
        )
        .expect("transparent wrappers must retain the resolved local parameter symbol");
    }
}
