#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_module, MoonshineModule, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use std::path::Path;

const PLATFORM: &str = "@tschk/moonshine-dom/platform";

fn compile(source: &str, javascript: bool, target: MoonshineModuleTarget) -> MoonshineModule {
    compile_moonshine_module(
        source,
        Path::new(if javascript {
            "model.svelte.js"
        } else {
            "model.svelte.ts"
        }),
        target,
    )
    .expect("companion module compiles")
}
fn valid(source: &str, javascript: bool) {
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        source,
        if javascript {
            SourceType::mjs()
        } else {
            SourceType::ts()
        },
    )
    .parse();
    assert!(parsed.errors.is_empty(), "{source}\n{:?}", parsed.errors);
    if javascript {
        let checked = SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        assert!(checked.errors.is_empty(), "{source}\n{:?}", checked.errors);
    }
}
fn platform_imports(module: &MoonshineModule) -> usize {
    module
        .imports
        .iter()
        .filter(|import| import.as_str() == PLATFORM)
        .count()
}

#[test]
fn global_this_binders_import_platform_realm_into_source_and_metadata() {
    for source in [
        "const globalThis={marker:1};export function factory(input){let [first]=$derived(input);return ()=>[first,globalThis.marker,globalThis[input]]}",
        "export function factory(globalThis){let [first]=$derived(globalThis.read());return ()=>first}",
    ] {
        for javascript in [false, true] {
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                let module = compile(source, javascript, target);
                valid(&module.source, javascript);
                assert_eq!(platform_imports(&module), 1, "{:?}", module.imports);
                assert_eq!(module.source.matches(PLATFORM).count(), 1, "{}", module.source);
                assert!(
                    module.source.contains("sourceRealm as __crepusSourceRealm0"),
                    "{}",
                    module.source
                );
                assert_eq!(
                    module.source.matches("__crepusSourceRealm0()").count(),
                    3,
                    "{}",
                    module.source
                );
                assert!(!module.source.contains("globalThis.Array"), "{}", module.source);
                assert!(!module.source.contains("globalThis.Symbol"), "{}", module.source);
                if source.contains("globalThis[input]") {
                    assert!(module.source.contains("globalThis.marker"), "{}", module.source);
                    assert!(module.source.contains("globalThis[input]"), "{}", module.source);
                }
            }
        }
    }
}

#[test]
fn companions_without_global_this_binders_keep_default_realm_and_imports() {
    let source = "export function factory(input){let [first]=$derived(input);return ()=>[first,globalThis.marker]}";
    for javascript in [false, true] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let module = compile(source, javascript, target);
            valid(&module.source, javascript);
            assert_eq!(platform_imports(&module), 0, "{:?}", module.imports);
            assert!(!module.source.contains(PLATFORM), "{}", module.source);
            assert!(!module.source.contains("sourceRealm"), "{}", module.source);
            assert!(
                module.source.contains("globalThis.Array.isArray"),
                "{}",
                module.source
            );
        }
    }
}

#[test]
fn global_this_binders_without_generated_realm_use_add_no_imports() {
    let source = "const globalThis={};export function factory(){let value=$state(1);return ()=>[value,globalThis]}";
    for javascript in [false, true] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let module = compile(source, javascript, target);
            valid(&module.source, javascript);
            assert_eq!(platform_imports(&module), 0, "{:?}", module.imports);
            assert!(!module.source.contains(PLATFORM), "{}", module.source);
        }
    }
}

#[test]
fn platform_realm_alias_avoids_plain_and_escaped_authored_names() {
    let source = "const __crepusSourceRealm0=1;const \\u005f_crepusSourceRealm1=2;const globalThis={};export function factory(input){let [first]=$derived(input);return ()=>[first,__crepusSourceRealm0,__crepusSourceRealm1,globalThis]}";
    for javascript in [false, true] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let module = compile(source, javascript, target);
            valid(&module.source, javascript);
            assert_eq!(platform_imports(&module), 1, "{:?}", module.imports);
            assert!(
                module
                    .source
                    .contains("sourceRealm as __crepusSourceRealm2"),
                "{}",
                module.source
            );
            assert_eq!(
                module.source.matches("__crepusSourceRealm2()").count(),
                3,
                "{}",
                module.source
            );
        }
    }
}
