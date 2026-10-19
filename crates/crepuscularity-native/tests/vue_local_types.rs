#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;

fn source(declarations: &str, props: &str) -> String {
    format!("<script setup lang=\"ts\">{declarations}const props=defineProps<{props}>();</script><template><p>{{{{props.label}}}}</p></template>")
}

#[test]
fn local_types_are_hoisted_and_props_shapes_remain_finite() {
    for declarations in [
        "type Props={label:string};",
        "interface Base{label:string}interface Props extends Base{count?:number}",
        "type Left={label:string};type Right={count?:number};type Props=Left & Right;",
        "import type {External} from './types';type First=External|Second;type Second={next?:First};type Props={label:string};",
        "const T=1;const U=2;type Identity<T>=T;type Unwrap<X>=X extends infer U?U:never;type Props={label:string};",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(&source(declarations, "Props"), Path::new("Types.vue"), target).unwrap();
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output.source, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}: {}", parsed.errors, output.source);
            let mut module_types = 0;
            for statement in &parsed.program.body {
                if matches!(statement, Statement::TSTypeAliasDeclaration(_) | Statement::TSInterfaceDeclaration(_)) {
                    module_types += 1;
                }
            }
            assert!(module_types > 0, "{}", output.source);
            struct NestedTypes(usize);
            impl<'a> Visit<'a> for NestedTypes {
                fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
                    struct Types(usize);
                    impl<'a> Visit<'a> for Types {
                        fn visit_ts_type_alias_declaration(&mut self, _: &TSTypeAliasDeclaration<'a>) { self.0 += 1; }
                        fn visit_ts_interface_declaration(&mut self, _: &TSInterfaceDeclaration<'a>) { self.0 += 1; }
                    }
                    let mut found = Types(0);
                    walk::walk_function_body(&mut found, body);
                    self.0 += found.0;
                }
            }
            let mut nested = NestedTypes(0);
            nested.visit_program(&parsed.program);
            assert_eq!(nested.0, 0, "{}", output.source);
            assert!(output.source.contains("Props"));
        }
    }
}

#[test]
fn unsupported_props_shapes_still_fail_after_local_type_admission() {
    for (declarations, props, diagnostic) in [
        ("type Props=Props;", "Props", "cyclic props"),
        (
            "type Props<T>={label:T};",
            "Props<string>",
            "finite object type",
        ),
        (
            "interface Props{label:string}interface Props{extra:number}",
            "Props",
            "merged props declarations",
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(
                &source(declarations, props),
                Path::new("InvalidTypes.vue"),
                target,
            )
            .unwrap_err();
            assert!(error.contains(diagnostic), "{error}");
        }
    }
}

#[test]
fn even_unused_hoisted_types_cannot_capture_setup_value_bindings() {
    for declarations in [
        "const local=1;type Unused=typeof local;",
        "const local=1;interface Unused{value:typeof local}",
        "const key=Symbol();interface Unused{[key]:string}",
        "class Item{};type Unused=Item;",
        "enum Kind{A};interface Unused{value:Kind}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(
                &source(declarations, "{label:string}"),
                Path::new("Capture.vue"),
                target,
            )
            .unwrap_err();
            assert!(error.contains("setup"), "{error}");
        }
    }
}

#[test]
fn local_alias_chains_keep_the_named_resolution_depth_limit() {
    let mut declarations = String::new();
    for index in 0..65 {
        declarations.push_str(&if index == 64 {
            format!("type P{index}={{label:string}};")
        } else {
            format!("type P{index}=P{};", index + 1)
        });
    }
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let error =
            compile_moonshine_vue(&source(&declarations, "P0"), Path::new("Deep.vue"), target)
                .unwrap_err();
        assert!(
            error.contains("props type resolution exceeds 64"),
            "{error}"
        );
    }
}

#[test]
fn hoisted_types_reject_merged_symbols_with_setup_runtime_declarations_in_either_order() {
    for (type_declaration, value_declaration, dependency) in [
        (
            "type Shared={x:number};",
            "const Shared=1;",
            "type Uses=typeof Shared;",
        ),
        (
            "interface Shared{x:number}",
            "const Shared=1;",
            "type Uses=typeof Shared;",
        ),
        (
            "type Shared={x:number};",
            "function Shared(){return 1}",
            "type Uses=typeof Shared;",
        ),
        (
            "interface Shared{x:number}",
            "function Shared(){return 1}",
            "type Uses=typeof Shared;",
        ),
        (
            "interface Shared{x?:number}",
            "class Shared{}",
            "type Uses=typeof Shared;",
        ),
        (
            "interface Shared{x?:number}",
            "class Shared{}",
            "type Uses=Shared;",
        ),
        (
            "type Shared={x:number};",
            "enum Shared{A}",
            "type Uses=typeof Shared;",
        ),
    ] {
        for type_first in [true, false] {
            let declarations = if type_first {
                format!("{type_declaration}{value_declaration}{dependency}")
            } else {
                format!("{value_declaration}{type_declaration}{dependency}")
            };
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let error = compile_moonshine_vue(
                    &source(&declarations, "{label:string}"),
                    Path::new("MergedCapture.vue"),
                    target,
                )
                .expect_err("a merged type symbol must not hide a setup runtime dependency");
                assert!(error.contains("setup value"), "{declarations}: {error}");
            }
        }
    }
}
