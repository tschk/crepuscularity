#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_vue, compile_moonshine_vue_with_resolver, MoonshineModuleTarget,
    VueTypeModule,
};
use std::path::Path;
fn source(ty: &str, imports: &str) -> String {
    format!("<script setup lang=\"ts\">{imports}\nconst props=defineProps<{ty}>();</script><template><p>{{{{props.label}}}}</p></template>")
}
#[test]
fn inline_optional_names_and_backend_helpers() {
    for (target, helper) in [
        (MoonshineModuleTarget::Dom, "createProps"),
        (MoonshineModuleTarget::Ssr, "createServerProps"),
    ] {
        let out = compile_moonshine_vue(
            &source("{label:string; optional?:number; '__proto__'?:string}", ""),
            Path::new("Counter.vue"),
            target,
        )
        .expect("finite inline props");
        assert!(out.source.contains(helper));
        assert!(
            out.source
                .split_whitespace()
                .collect::<String>()
                .contains("[\"label\",\"optional\",\"__proto__\"]"),
            "{}",
            out.source
        );
    }
}
#[test]
fn imported_interface_extends_alias_intersection_and_reexport() {
    let mut calls = vec![];
    let mut resolver = |from: &Path, specifier: &str| {
        calls.push((from.to_owned(), specifier.to_owned()));
        let source=match specifier {"./public"=>"export type {Props as PublicProps} from './types';", "./types"=>"interface Base {label:string} type Extra={count?:number}; export interface Props extends Base, Extra {initial:number}",_=>return Err("unknown import".into())};
        Ok(VueTypeModule {
            path: specifier.into(),
            source: source.into(),
        })
    };
    let out = compile_moonshine_vue_with_resolver(
        &source(
            "PublicProps & {other?:string}",
            "import type {PublicProps} from './public';",
        ),
        Path::new("Counter.vue"),
        MoonshineModuleTarget::Dom,
        &mut resolver,
    )
    .expect("resolved props");
    assert!(
        out.source
            .split_whitespace()
            .collect::<String>()
            .contains("[\"initial\",\"label\",\"count\",\"other\"]"),
        "{}",
        out.source
    );
    assert_eq!(calls.len(), 2);
    assert!(out
        .source
        .split_whitespace()
        .collect::<String>()
        .contains("PublicProps&{other?:string;}"));
}
#[test]
fn imported_types_require_explicit_source_context() {
    let err = compile_moonshine_vue(
        &source("Props", "import type {Props} from './types';"),
        Path::new("Counter.vue"),
        MoonshineModuleTarget::Dom,
    )
    .expect_err("no implicit fs");
    assert!(err.contains("explicit source resolver"));
}
#[test]
fn finite_shape_failures_and_ambiguous_boolean_casting_fail_closed() {
    for ty in [
        "{label:boolean & {}}",
        "{label?:boolean}&{label?:string}",
        "{label?:boolean}&{label?:boolean}",
        "{[key:string]:string}",
        "Record<string,string>",
        "{label:Flag}",
    ] {
        let result = compile_moonshine_vue(
            &source(ty, ""),
            Path::new("Counter.vue"),
            MoonshineModuleTarget::Dom,
        );
        assert!(result.is_err(), "{ty} accepted");
    }
    let mut resolver = |_: &Path, _: &str| {
        Ok(VueTypeModule {
            path: "types.ts".into(),
            source: "export type Props = Props".into(),
        })
    };
    let err = compile_moonshine_vue_with_resolver(
        &source("Props", "import type {Props} from './types';"),
        Path::new("Counter.vue"),
        MoonshineModuleTarget::Dom,
        &mut resolver,
    )
    .expect_err("cycle");
    assert!(err.contains("cyclic props"));
}

#[test]
fn merged_interfaces_fail_instead_of_silently_dropping_keys() {
    let mut resolver = |_: &Path, _: &str| {
        Ok(VueTypeModule {
            path: "types.ts".into(),
            source: "export interface Props {label:string} export interface Props {extra:number}"
                .into(),
        })
    };
    let error = compile_moonshine_vue_with_resolver(
        &source("Props", "import type {Props} from './types';"),
        Path::new("Counter.vue"),
        MoonshineModuleTarget::Dom,
        &mut resolver,
    )
    .expect_err("finite merged names require explicit support");
    assert!(error.contains("merged props declarations"));
}

#[test]
fn casting_edges_and_static_computed_names_fail_closed_in_both_backends() {
    for ty in [
        "{label:boolean & {}}",
        "{label?:string}&{label?:true}",
        "{label?:false}&{label?:boolean}",
        "{label?:keyof {x:number}}",
        "{['label']?:string}",
        "{label:(boolean | string) & {}}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(&source(ty, ""), Path::new("Counter.vue"), target)
                .expect_err("casting/metadata is unsupported");
            assert!(error.contains("unsupported"), "{ty}: {error}");
        }
    }
}
#[test]
fn exported_alias_cycles_fail_with_resolver_context() {
    let mut resolver = |_: &Path, specifier: &str| {
        Ok(VueTypeModule {
            path: specifier.into(),
            source: match specifier {
                "./a" => "export type {Other as Props} from './b';",
                "./b" => "export type {Props as Other} from './a';",
                _ => return Err("unknown module".into()),
            }
            .into(),
        })
    };
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let error = compile_moonshine_vue_with_resolver(
            &source("Props", "import type {Props} from './a';"),
            Path::new("Counter.vue"),
            target,
            &mut resolver,
        )
        .expect_err("alias cycle");
        assert!(error.contains("cyclic props"), "{error}");
    }
}

#[test]
fn acyclic_type_resolution_accepts_64_named_frames_and_rejects_65() {
    for frames in [64usize, 65] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let mut calls = 0;
            let mut resolver = |_: &Path, specifier: &str| {
                calls += 1;
                let level: usize = specifier.strip_prefix("./level").unwrap().parse().unwrap();
                // The setup's imported Props is frame one. Each reexport adds
                // exactly one frame, without a second local alias lookup.
                let module = if level == frames - 2 {
                    "export interface Props {label:string}".to_string()
                } else {
                    format!("export type {{Props}} from './level{}';", level + 1)
                };
                Ok(VueTypeModule {
                    path: format!("level{level}.ts").into(),
                    source: module,
                })
            };
            let result = compile_moonshine_vue_with_resolver(
                &source("Props", "import type {Props} from './level0';"),
                Path::new("Counter.vue"),
                target,
                &mut resolver,
            );
            if frames == 64 {
                let output = result.expect("64 named frames are admitted");
                assert!(output.source.contains("label"));
            } else {
                let error = result.expect_err("65 named frames exceed the limit");
                assert!(
                    error.contains("props type resolution exceeds 64"),
                    "{error}"
                );
            }
            assert_eq!(calls, frames - 1);
        }
    }
}

#[test]
fn diamond_inheritance_can_revisit_a_shared_base_after_each_branch_returns() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let mut shared_calls = 0;
        let mut resolver = |_: &Path, specifier: &str| {
            let module = match specifier {
                "./left" => "import type {Base} from './shared';export interface Left extends Base {left:string}",
                "./right" => "import type {Base} from './shared';export interface Right extends Base {right:string}",
                "./shared" => {
                    shared_calls += 1;
                    "export interface Base {label:string}"
                }
                _ => return Err(format!("unexpected type import {specifier}")),
            };
            Ok(VueTypeModule {
                path: format!("{}.ts", specifier.trim_start_matches("./")).into(),
                source: module.into(),
            })
        };
        let output = compile_moonshine_vue_with_resolver(
            &source(
                "Left & Right",
                "import type {Left} from './left';import type {Right} from './right';",
            ),
            Path::new("Counter.vue"),
            target,
            &mut resolver,
        )
        .expect("shared base is not an active-ancestor cycle");
        assert_eq!(shared_calls, 2);
        assert!(
            output
                .source
                .split_whitespace()
                .collect::<String>()
                .contains("[\"left\",\"label\",\"right\"]"),
            "{}",
            output.source
        );
    }
}

#[test]
fn interface_own_props_precede_bases_like_official_component_options() {
    for (declarations, expected) in [
        ("interface Base{base?:string}interface Props extends Base{own?:number}", "[\"own\",\"base\"]"),
        ("interface A{a?:string}interface B{b?:string}interface Props extends A,B{own?:number}", "[\"own\",\"a\",\"b\"]"),
        ("interface Shared{shared?:string}interface A extends Shared{a?:string}interface B extends Shared{b?:string}interface Props extends A,B{own?:string}", "[\"own\",\"a\",\"shared\",\"b\"]"),
    ] {
        let source = format!("<script setup lang=\"ts\">{declarations};const props=defineProps<Props>()</script><template><pre>{{{{Object.keys(props).join(',')}}}}</pre></template>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(&source, Path::new("Order.vue"), target).unwrap();
            assert!(output.source.split_whitespace().collect::<String>().contains(expected), "{}", output.source);
        }
    }
}
