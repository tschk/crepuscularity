#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_svelte, MoonshineModuleLanguage, MoonshineModuleTarget,
};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use std::path::Path;
fn compile(source: &str, target: MoonshineModuleTarget) -> String {
    let module = compile_moonshine_svelte(source, Path::new("Patterns.svelte"), target).unwrap();
    let allocator = Allocator::default();
    let source_type = if module.language == MoonshineModuleLanguage::JavaScript {
        SourceType::mjs()
    } else {
        SourceType::tsx()
    };
    let parsed = Parser::new(&allocator, &module.source, source_type).parse();
    assert!(
        parsed.errors.is_empty(),
        "{:?}\n{}",
        parsed.errors,
        module.source
    );
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(
        semantic.errors.is_empty(),
        "{:?}\n{}",
        semantic.errors,
        module.source
    );
    module.source
}
#[test]
fn seven_legal_pattern_forms_compile_as_complete_typed_modules() {
    for (collection, pattern, body) in [
        ("[{}]", "{value=3}:{value?:number}", "{value}/{value}"),
        (
            "[{value:1,extra:2}]",
            "{value,...rest}:{value:number;extra:number}",
            "{value}/{rest.extra}",
        ),
        (
            "[[1,2,3]]",
            "[head,...tail]:number[]",
            "{head}/{tail.length}",
        ),
        (
            "[[1,2,3]]",
            "[head,,...tail]:number[]",
            "{head}/{tail.length}",
        ),
        ("[[]]", "[head=3,...tail]:number[]", "{head}/{tail.length}"),
        ("[[1,'a']]", "[head,tail]:[number,string]", "{head}/{tail}"),
        (
            "[[1,2]]",
            "[head,...tail]:readonly number[]",
            "{head}/{tail.length}",
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            compile(&format!("<script lang='ts'>let rows={collection};</script>{{#each rows as {pattern}}}<p>{body}</p>{{/each}}"),target);
        }
    }
}
#[test]
fn nested_defaults_keep_outer_aliases_and_shared_gather_scopes() {
    let source="<script lang='ts'>const rows=[{items:[undefined,2,3],value:undefined}];</script>{#each [7] as outer:number}{#each rows as {items:[head=outer,,...tail],value:alias=head,...rest}}<p>{head}/{alias}/{tail.join(',')}/{rest}</p>{/each}{/each}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(source, target);
        assert!(!output.contains("unsupported"));
    }
}
#[test]
fn javascript_patterns_and_iterator_source_remain_original_language() {
    let source="<script>let rows=$state.raw([[undefined,2,3]]);function fallback(){return 9}</script>{#each rows as [first=fallback(),,...rest],index(index)}<p>{first}/{first}/{rest.join(',')}</p>{/each}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        compile(source, target);
    }
}
#[test]
fn native_ssr_patterns_and_dom_lazy_projections_are_distinct() {
    let source="<script lang='ts'>let rows:Iterable<number|undefined>[]=[];function fallback(){return 9}</script>{#each rows as [unused=fallback(),used=fallback()],index(index)}<p>{used}/{used}</p>{/each}";
    let dom = compile(source, MoonshineModuleTarget::Dom);
    assert!(dom.contains("Symbol.iterator"));
    assert!(dom.contains("createDerived"));
    assert!(dom.contains("=== void 0"), "{dom}");
    let server = compile(source, MoonshineModuleTarget::Ssr);
    assert!(!server.contains("__crepusEachGather"));
    assert!(!server.contains("__crepusEachPatternMemo"));
    assert!(server.contains("unused = fallback()") || server.contains("unused=fallback()"));
}
#[test]
fn rest_uses_safe_own_data_properties_without_an_optional_runtime_api() {
    let source="<script lang='ts'>let rows:{id:number;extra:string}[]=[];</script>{#each rows as {id,...rest}}<p>{rest.extra}/{rest.extra}</p>{/each}";
    let dom = compile(source, MoonshineModuleTarget::Dom);
    assert!(dom.contains("defineProperty"));
    assert!(dom.contains("getOwnPropertySymbols"));
    assert!(!dom.contains("createDerived"));
}
#[test]
fn pattern_alias_writes_stay_readonly_but_member_binding_is_legal() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        compile("<script lang='ts'>let rows:{nested?:{value:number}}[]=[];</script>{#each rows as {nested={value:1},...rest}}<input type='number' bind:value={nested.value}/><p>{rest}</p>{/each}",target);
        for body in ["<p>{head=2}</p>", "<input bind:value={head}/>"] {
            let source=format!("<script lang='ts'>const rows:number[][]=[];</script>{{#each rows as [head=1,...tail]}}{body}{{/each}}");
            let error =
                compile_moonshine_svelte(&source, Path::new("Invalid.svelte"), target).unwrap_err();
            assert!(error.contains("each bindings cannot"), "{error}");
        }
    }
}
#[test]
fn computed_keys_remain_explicitly_unsupported_not_a_boundary_failure() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let source="<script lang='ts'>const key='value';const rows=[];</script>{#each rows as {[key]:value,...rest}}<p>{value}</p>{/each}";
        let error =
            compile_moonshine_svelte(source, Path::new("Invalid.svelte"), target).unwrap_err();
        assert!(error.contains("computed/numeric pattern keys"), "{error}");
        assert!(!error.contains("unclosed"), "{error}");
    }
}

#[test]
fn untyped_props_each_helpers_guard_any_and_erase_overloads_in_javascript() {
    // This verifies actual source emission and syntax. The strict consumer
    // matrix separately checks that typed/unknown collections cannot choose any.
    for language in [" lang=\"ts\"", ""] {
        let source = format!(
            "<script{language}>let {{cases}}=$props();</script>\
             {{#each cases as row (row.id)}}<p>{{row.id}}</p>{{/each}}\
             {{#each cases as {{id,name}} (id)}}<p>{{id}}:{{name}}</p>{{/each}}"
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile(&source, target);
            let allocator = Allocator::default();
            let typed = !language.is_empty();
            let parsed = Parser::new(
                &allocator,
                &output,
                if typed {
                    SourceType::tsx()
                } else {
                    SourceType::mjs()
                },
            )
            .parse();
            assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
            let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
            let helpers = if matches!(target, MoonshineModuleTarget::Dom) {
                ["__crepusEachChildren", "__crepusEachKey"]
            } else {
                ["__crepusEachChildren", "__crepusEachSnapshotChildren"]
            };
            for helper in helpers {
                let mut implementations = 0;
                let mut overloads = 0;
                let mut guarded_overloads = 0;
                for node in semantic.nodes().iter() {
                    let oxc_ast::AstKind::Function(function) = node.kind() else {
                        continue;
                    };
                    if !function.id.as_ref().is_some_and(|id| id.name == helper) {
                        continue;
                    }
                    if function.body.is_some() {
                        implementations += 1;
                    } else {
                        overloads += 1;
                        let span = function.span;
                        let signature: String = output[span.start as usize..span.end as usize]
                            .chars()
                            .filter(|ch| !ch.is_whitespace())
                            .collect();
                        guarded_overloads += usize::from(
                            signature.contains("0extends1&globalThis.ReturnType<R>?unknown:never"),
                        );
                    }
                }
                assert_eq!(implementations, 1, "{helper}: {output}");
                assert_eq!(overloads, if typed { 2 } else { 0 }, "{helper}: {output}");
                assert_eq!(guarded_overloads, usize::from(typed), "{helper}: {output}");
            }
        }
    }
}

#[test]
fn annotated_each_helpers_keep_one_contextual_implementation_in_both_hosts() {
    // Annotated row types constrain the reader and infer T from the callback;
    // an any-only overload here would widen tuple and typeof literal context.
    // The strict consumer matrix checks that inference with real checkers.
    for source in [
        "<script lang=\"ts\">const sample=1;type Box<T>={value:T};</script>{#each [1] as item:typeof sample}<p>{item}</p>{/each}",
        "<script lang=\"ts\">const sample=1;type Box<T>={value:T};</script>{#each [[1,\"a\"]] as [head,tail]:[number,string]}<p>{head}:{tail}</p>{/each}",
        "<script lang=\"ts\">const sample=1;</script>{#each [1] as item:typeof sample,index (item)}<p>{item}</p>{/each}{#each [{value:1,extra:2}] as {value}:{value:number}}<p>{value.toFixed()}</p>{/each}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile(source, target);
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}\n{output}", parsed.errors);
            let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
            let mut annotated = Vec::new();
            for node in semantic.nodes().iter() {
                let oxc_ast::AstKind::Function(function) = node.kind() else {
                    continue;
                };
                let Some(id) = function.id.as_ref() else {
                    continue;
                };
                if !id.name.starts_with("__crepusEachAnnotated") {
                    continue;
                }
                annotated.push(id.name.to_string());
                assert!(function.body.is_some(), "{}: {output}", id.name);
            }
            let count = annotated.len();
            annotated.sort();
            annotated.dedup();
            assert!(count > 0 && count == annotated.len(), "{annotated:?}: {output}");
            assert!(!output.contains("0 extends 1 &"), "{output}");
        }
    }
}
