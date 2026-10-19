#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_vue_with_resolver, MoonshineModuleTarget, VueTypeModule,
};
use std::path::Path;

const BROKEN: &str = "export interface Props { label: }";
const LEAF: &str = "/site/types/broken.ts";
fn source(specifier: &str) -> String {
    format!("<script setup lang=\"ts\">import type {{Props}} from '{specifier}';const props=defineProps<Props>();</script><template><p>{{{{props.label}}}}</p></template>")
}

#[test]
fn imported_props_parse_failures_name_the_broken_leaf_and_keep_parser_details() {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, BROKEN, oxc_span::SourceType::tsx()).parse();
    assert!(!parsed.errors.is_empty());
    let details = format!("{:?}", parsed.errors);
    for specifier in ["./broken", "./barrel"] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let mut resolver = |_: &Path, import: &str| match import {
                "./barrel" => Ok(VueTypeModule {
                    path: "/site/barrel.ts".into(),
                    source: "export type {Props} from './broken';".into(),
                }),
                "./broken" => Ok(VueTypeModule {
                    path: LEAF.into(),
                    source: BROKEN.into(),
                }),
                _ => Err(format!("unexpected import {import}")),
            };
            let error = compile_moonshine_vue_with_resolver(
                &source(specifier),
                Path::new("/site/App.vue"),
                target,
                &mut resolver,
            )
            .expect_err("malformed imported prop types must fail");
            assert!(error.contains(LEAF), "{specifier}: {error}");
            assert!(error.contains(&details), "lost parser details: {error}");
        }
    }
}

#[test]
fn valid_prop_type_reexports_keep_their_existing_compilation_contract() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let mut resolver = |_: &Path, import: &str| match import {
            "./barrel" => Ok(VueTypeModule {
                path: "/site/barrel.ts".into(),
                source: "export type {Props} from './valid';".into(),
            }),
            "./valid" => Ok(VueTypeModule {
                path: "/site/types/valid.ts".into(),
                source: "export interface Props {label:string}".into(),
            }),
            _ => Err(format!("unexpected import {import}")),
        };
        let output = compile_moonshine_vue_with_resolver(
            &source("./barrel"),
            Path::new("/site/App.vue"),
            target,
            &mut resolver,
        )
        .expect("valid reexports remain supported");
        assert!(output.has_default_export);
        assert!(output.source.contains("label"), "{}", output.source);
    }
}
