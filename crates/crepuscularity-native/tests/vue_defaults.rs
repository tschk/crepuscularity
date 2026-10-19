#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;
fn source(setup: &str) -> String {
    format!(
        "<script setup lang=\"ts\">{setup}</script><template><p>{{{{props.label}}}}</p></template>"
    )
}
#[test]
fn finite_defaults_keep_caller_type_and_resolve_setup_through_typed_helper() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let result=compile_moonshine_vue(&source("type Props={label?:string;flag?:boolean;handler?:()=>number};const props=withDefaults(defineProps<Props>(),{label:'text',flag:true,handler:()=>1});"),Path::new("Defaults.vue"),target).unwrap();
        assert!(result.source.contains("DefaultValues"));
        assert!(result.source.contains("satisfies"));
        assert!(result.source.contains("typeof __crepusVueDefaults"));
        assert!(result.source.contains("withVueScheduler"));
        assert!(!result.source.contains("withDefaults("));
        assert!(!result.source.contains("defineProps("));
        assert!(result
            .source
            .contains(if matches!(target, MoonshineModuleTarget::Dom) {
                "createDefaultProps"
            } else {
                "createServerDefaultProps"
            }));
    }
}
#[test]
fn unsupported_defaults_fail_with_specific_diagnostics() {
    for (setup,expected) in [
  ("const props=withDefaults(defineProps<{label?:string}>(),{...other});","spread defaults"),
  ("const props=withDefaults(defineProps<{label?:string}>(),{get label(){return 'x'}});","static data defaults"),
  ("const props=withDefaults(defineProps<{label?:string}>(),{['label']:'x'});","static data defaults"),
  ("const props=withDefaults(defineProps<{label?:string}>(),{label:'x',label:'y'});","duplicate default keys"),
  ("const props=withDefaults(defineProps<{label?:string}>(),{missing:'x'});","unknown default key"),
  ("const props=withDefaults(defineProps<{'__proto__'?:string}>(),{'__proto__':'x'});","prototype default key"),
  ("let local='x';const props=withDefaults(defineProps<{label?:string}>(),{label:()=>local});","cannot capture setup"),
  ("let props=withDefaults(defineProps<{label?:string}>(),{label:'x'});","const props declaration"),
 ] {for target in [MoonshineModuleTarget::Dom,MoonshineModuleTarget::Ssr] {let error=compile_moonshine_vue(&source(setup),Path::new("Invalid.vue"),target).unwrap_err();assert!(error.contains(expected),"{error}");}}
}

#[test]
fn defaults_factory_cannot_hide_unlowered_nested_macros() {
    for (setup, expected) in [
        ("const props=withDefaults(defineProps<{label?:string}>(),{label:()=>defineProps<{nested:number}>()});", "defineProps macro must be top-level"),
        ("const props=withDefaults(defineProps<{label?:string}>(),{label:()=>withDefaults(defineProps<{nested:number}>(),{nested:1})});", "defineProps macro must be top-level"),
        ("const props=withDefaults(defineProps<{label?:string}>(),{label:()=>withDefaults(1,{})});", "unsupported native Vue macro withDefaults"),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(&source(setup), Path::new("Nested.vue"), target).unwrap_err();
            assert!(error.contains(expected), "{error}");
        }
    }
}

#[test]
fn native_sync_watch_named_alias_uses_existing_reactive_helper() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let result=compile_moonshine_vue(&source("import {ref,watch as observe} from 'vue';const local=ref(0);observe(local,()=>{}, {flush:'sync'});const props=defineProps<{label:string}>();"),Path::new("Watch.vue"),target).unwrap();
        assert!(result.source.contains("watch as observe"));
        assert!(result.source.contains("@tschk/moonshine-vue/reactive"));
        let error = compile_moonshine_vue(
            &source(
                "import {watchPostEffect} from 'vue';const props=defineProps<{label:string}>();",
            ),
            Path::new("Unknown.vue"),
            target,
        )
        .unwrap_err();
        assert!(error.contains("unsupported native Vue helper watchPostEffect"));
    }
}

#[test]
fn finite_default_function_metadata_matches_vue_runtime_constructor_shapes() {
    use oxc_allocator::Allocator;
    use oxc_ast::ast::ArrayExpression;
    use oxc_ast_visit::{walk, Visit};
    use oxc_parser::Parser;
    use oxc_span::{GetSpan, SourceType};
    struct Entry<'s> {
        source: &'s str,
        factory: Vec<bool>,
    }
    impl<'a> Visit<'a> for Entry<'_> {
        fn visit_array_expression(&mut self, array: &ArrayExpression<'a>) {
            if array.elements.len() == 3 {
                let key = array.elements[0].span();
                if serde_json::from_str::<String>(
                    &self.source[key.start as usize..key.end as usize],
                )
                .ok()
                .as_deref()
                    == Some("handler")
                {
                    let value = array.elements[2].span();
                    let value = &self.source[value.start as usize..value.end as usize];
                    assert!(
                        value == "false" || value.contains("typeof"),
                        "unexpected handler factory metadata: {value}"
                    );
                    self.factory.push(value != "false");
                }
            }
            walk::walk_array_expression(self, array);
        }
    }
    for (ty, factory) in [
        ("()=>number", false),
        ("{():number}", false),
        ("{new():object}", false),
        ("{():number;new():object}", false),
        ("(()=>number)&(()=>number)", false),
        ("(()=>number)|(undefined)", false),
        ("never|(()=>number)", false),
        ("unknown|(()=>number)", false),
        ("any&(()=>number)", false),
        ("void|(()=>number)", false),
        ("(new()=>object)|(()=>number)", false),
        ("new()=>object", true),
        ("{():number;label:string}", true),
        ("(()=>number)&{}", true),
        ("(()=>number)|null", true),
        ("(()=>number)|string", true),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let setup = format!("const props=withDefaults(defineProps<{{label?:string;handler?:{ty}}}>(),{{handler:(()=>1) as any}});");
            let output =
                compile_moonshine_vue(&source(&setup), Path::new("FunctionDefault.vue"), target)
                    .unwrap();
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output.source, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let mut entry = Entry {
                source: &output.source,
                factory: Vec::new(),
            };
            entry.visit_program(&parsed.program);
            assert_eq!(entry.factory, [factory], "{ty}: {}", output.source);
        }
    }
}

#[test]
fn indirect_function_defaults_remain_explicitly_unsupported() {
    for setup in [
        "type Handler=()=>number;const props=withDefaults(defineProps<{label?:string;handler?:Handler}>(),{handler:()=>1});",
        "import type{Handler}from'./types';const props=withDefaults(defineProps<{label?:string;handler?:Handler}>(),{handler:()=>1});",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error=compile_moonshine_vue(&source(setup),Path::new("AliasDefault.vue"),target).unwrap_err();
            assert!(error.contains("indirect prop value types"), "{error}");
        }
    }
}

#[test]
fn authored_defaults_type_wrappers_survive_the_module_hoist() {
    use oxc_allocator::Allocator;
    use oxc_ast::ast::VariableDeclarator;
    use oxc_ast_visit::{walk, Visit};
    use oxc_parser::Parser;
    use oxc_span::{GetSpan, SourceType};
    struct Hoists<'s> {
        source: &'s str,
        values: Vec<String>,
    }
    impl<'a> Visit<'a> for Hoists<'_> {
        fn visit_variable_declarator(&mut self, declaration: &VariableDeclarator<'a>) {
            if matches!(&declaration.id, oxc_ast::ast::BindingPattern::BindingIdentifier(id) if id.name.starts_with("__crepusVueDefaults"))
            {
                if let Some(value) = &declaration.init {
                    let span = value.span();
                    self.values.push(
                        self.source[span.start as usize..span.end as usize]
                            .chars()
                            .filter(|c| !c.is_whitespace() && *c != ';')
                            .collect(),
                    );
                }
            }
            walk::walk_variable_declarator(self, declaration);
        }
    }
    for (expression, required) in [
        ("({label:1} as any)", "asany"),
        ("({label:'ok'} as const)", "asconst"),
        (
            "({label:'ok'} satisfies {label:string})",
            "satisfies{label:string}",
        ),
        // Preserve this failing authored constraint for the downstream checker.
        (
            "({label:'ok'} satisfies {label:number})",
            "satisfies{label:number}",
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let setup =
                format!("const props=withDefaults(defineProps<{{label?:string}}>(),{expression});");
            let output =
                compile_moonshine_vue(&source(&setup), Path::new("WrappedDefaults.vue"), target)
                    .unwrap();
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output.source, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let mut hoists = Hoists {
                source: &output.source,
                values: Vec::new(),
            };
            hoists.visit_program(&parsed.program);
            assert_eq!(hoists.values.len(), 1, "{}", output.source);
            assert!(
                hoists.values[0].contains(required),
                "{expression}: {}",
                output.source
            );
        }
    }
}

#[test]
fn default_wrapper_type_queries_cannot_capture_setup_values() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let error=compile_moonshine_vue(&source("const local={label:'ok'};const props=withDefaults(defineProps<{label?:string}>(),({label:'ok'} as typeof local));"),Path::new("WrappedCapture.vue"),target).unwrap_err();
        assert!(error.contains("cannot capture setup values"), "{error}");
    }
}

#[test]
fn asserted_any_defaults_project_only_declared_prop_keys_with_a_hygienic_binder() {
    let setup="const __crepusVueDefaultKey0=1;const props=withDefaults(defineProps<{label?:string;count?:number}>(),({label:1} as any));";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output =
            compile_moonshine_vue(&source(setup), Path::new("AnyDefaults.vue"), target).unwrap();
        let compact = output
            .source
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        assert!(compact.contains("asany"), "{}", output.source);
        assert!(compact.contains("inkeyof("), "{}", output.source);
        assert!(
            compact.contains("&keyoftypeof__crepusVueDefaults"),
            "{}",
            output.source
        );
        assert!(
            !compact.contains("[__crepusVueDefaultKey0in"),
            "{}",
            output.source
        );
    }
}

#[test]
fn defaults_projection_binder_avoids_decoded_type_alias_names() {
    let setup = r"type __crepusVueDefault\u004bey0={label?:string};const props=withDefaults(defineProps<__crepusVueDefault\u004bey0>(),({label:1}as any));";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output =
            compile_moonshine_vue(&source(setup), Path::new("EscapedDefaults.vue"), target)
                .unwrap();
        let compact = output
            .source
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();
        assert!(
            !compact.contains("[__crepusVueDefaultKey0in"),
            "{}",
            output.source
        );
        assert!(
            compact.contains("[__crepusVueDefaultKey1in"),
            "{}",
            output.source
        );
        assert!(
            compact.contains("keyof(__crepusVueDefault"),
            "{}",
            output.source
        );
    }
}

#[test]
fn defaults_helpers_avoid_decoded_setup_and_type_bindings() {
    for (binding, expected) in [
        (
            r"const \u005f_crepusVueDefaults0={trap:true};",
            "const __crepusVueDefaults1 =",
        ),
        (
            r"type \u005f_crepusVueDefaultValues0=number;",
            "DefaultValues as __crepusVueDefaultValues1",
        ),
        (
            r"const \u005f_crepusVueDefaultsScheduler0=0;",
            "withVueScheduler as __crepusVueDefaultsScheduler1",
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let setup=format!("{binding}const props=withDefaults(defineProps<{{label?:string}}>(),{{label:'ok'}});");
            let output =
                compile_moonshine_vue(&source(&setup), Path::new("EscapedHelpers.vue"), target)
                    .unwrap();
            assert!(
                output.source.contains(expected),
                "{binding}: {}",
                output.source
            );
        }
    }
}
