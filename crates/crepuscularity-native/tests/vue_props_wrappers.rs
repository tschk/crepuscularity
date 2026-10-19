#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;

fn source(prefix: &str, initializer: &str, suffix: &str) -> String {
    format!("<script setup lang=\"ts\">{prefix}type Props={{label:string}};const props={initializer};{suffix}</script><template><p>{{{{props.label}}}}</p></template>")
}

#[test]
fn transparent_define_props_callee_and_result_wrappers_are_fully_lowered() {
    for initializer in [
        "(defineProps)<Props>()",
        "((defineProps))<Props>()",
        "defineProps!<Props>()",
        "(defineProps as typeof defineProps)<Props>()",
        "(defineProps satisfies typeof defineProps)<Props>()",
        "(defineProps<Props>())",
        "(defineProps<Props>() as Props)",
        "(defineProps<Props>() satisfies Props)",
        "defineProps<Props>()!",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(
                &source("", initializer, ""),
                Path::new("WrappedProps.vue"),
                target,
            )
            .unwrap();
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output.source, SourceType::tsx()).parse();
            assert!(
                parsed.errors.is_empty(),
                "{:?}: {}",
                parsed.errors,
                output.source
            );
            struct Calls(usize);
            impl<'a> Visit<'a> for Calls {
                fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
                    if call
                        .callee
                        .get_identifier_reference()
                        .is_some_and(|id| id.name == "defineProps")
                    {
                        self.0 += 1;
                    }
                    walk::walk_call_expression(self, call);
                }
            }
            let mut calls = Calls(0);
            calls.visit_program(&parsed.program);
            assert_eq!(calls.0, 0, "{}", output.source);
            assert!(!output.imports.iter().any(|path| path == "vue"));
            let helper = if matches!(target, MoonshineModuleTarget::Dom) {
                "createProps"
            } else {
                "createServerProps"
            };
            assert!(output.source.contains(helper), "{}", output.source);
            assert!(output.source.contains("[\"label\"]"), "{}", output.source);
        }
    }
}

#[test]
fn wrapped_shadowed_define_props_is_not_a_compiler_macro() {
    for prefix in [
        "function defineProps<T>(){return {} as T}",
        "const defineProps=<T,>()=>({} as T);",
        "import {factory as defineProps} from './factory';",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(
                &source(prefix, "(defineProps)<Props>()", ""),
                Path::new("ShadowedProps.vue"),
                target,
            )
            .unwrap_err();
            assert!(error.contains("unshadowed"), "{error}");
        }
    }
}

#[test]
fn wrapped_props_calls_still_reject_arguments_duplicates_and_nested_declarations() {
    for (initializer, suffix) in [
        ("(defineProps)<Props>({})", ""),
        (
            "(defineProps)<Props>()",
            "const extra=(defineProps)<Props>();",
        ),
        (
            "{} as Props",
            "function nested(){return (defineProps)<Props>();}",
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(compile_moonshine_vue(
                &source("", initializer, suffix),
                Path::new("InvalidProps.vue"),
                target
            )
            .is_err());
        }
    }
}
