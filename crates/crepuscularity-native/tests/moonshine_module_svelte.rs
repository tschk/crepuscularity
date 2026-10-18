#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn ordinary_typescript_helpers_share_native_svelte_context_and_lifecycle() {
    let source = "import {createContext as context,getAllContexts as contexts,onMount,untrack as detached} from 'svelte';export const [get,set,has]=context<string>();export default <number>1;";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_module(source, Path::new("context.ts"), target).unwrap();
        assert!(compiled.source.contains("@tschk/moonshine-svelte/svelte"));
        let reactive = if matches!(target, MoonshineModuleTarget::Ssr) {
            "@tschk/moonshine-dom/ssr"
        } else {
            "@tschk/moonshine-dom/reactive"
        };
        assert!(compiled.source.contains(reactive));
        assert!(compiled.source.contains("context<string>()"));
        assert!(compiled.source.contains("getAllContexts as contexts"));
        assert!(compiled.has_default_export);
        assert_eq!(compiled.imports, ["svelte"]);
        assert!(!compiled.source.contains("from \"svelte\""));
    }
}

#[test]
fn ordinary_helper_type_imports_keep_their_erasure_contract() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_module("import type{getContext as Read,untrack as Detach}from'svelte';export type Getter=typeof Read;export type Untrack=typeof Detach;", Path::new("types.ts"), target).unwrap();
        let imports: Vec<_> = compiled
            .source
            .lines()
            .filter(|line| line.starts_with("import"))
            .collect();
        assert_eq!(imports.len(), 2);
        assert!(imports.iter().all(|line| line.starts_with("import type ")));
    }
}

#[test]
fn unsupported_svelte_runtime_forms_cannot_escape_through_plain_modules() {
    for source in [
        "import * as Svelte from 'svelte';export{Svelte};",
        "import Svelte from 'svelte';export{Svelte};",
        "import 'svelte';",
        "import{beforeUpdate}from'svelte';export{beforeUpdate};",
        "import{getContext}from'svelte/internal';export{getContext};",
        "export{beforeUpdate}from'svelte';",
        "export * from 'svelte';",
        "export * as helpers from 'svelte';",
        "export const load=()=>import('svelte');",
        "export type Helpers=typeof import('svelte');",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_module(source, Path::new("helper.ts"), target).is_err(),
                "accepted {source}"
            );
        }
    }
}

#[test]
fn named_helper_reexports_preserve_hosts_aliases_types_and_default_metadata() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let source="export{getContext as default,getAllContexts as contexts,createContext as makeContext,type setContext as Setter,untrack as detached}from'svelte';";
        let compiled = compile_moonshine_module(source, Path::new("barrel.ts"), target).unwrap();
        assert!(compiled.has_default_export);
        assert_eq!(compiled.imports, ["svelte"]);
        assert!(compiled.source.contains("getContext as default"));
        assert!(compiled.source.contains("type setContext as Setter"));
        assert!(compiled.source.contains("@tschk/moonshine-svelte/svelte"));
        assert!(!compiled.source.contains("from \"svelte\""));
        let types = compile_moonshine_module(
            "export type{getContext as Reader,untrack as Detach}from'svelte';",
            Path::new("types.ts"),
            target,
        )
        .unwrap();
        assert!(types
            .source
            .lines()
            .filter(|line| line.starts_with("export"))
            .all(|line| line.starts_with("export type ")));
    }
}

#[test]
fn tick_imports_and_reexports_keep_target_runtime_aliases_and_metadata() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let runtime = match target {
            MoonshineModuleTarget::Dom => "@tschk/moonshine-svelte/svelte",
            MoonshineModuleTarget::Ssr => "@tschk/moonshine-svelte/ssr",
        };
        for source in [
            "import{tick}from'svelte';export{tick};",
            "export{tick}from'svelte';",
            "import{tick as settle}from'svelte';export{settle};",
            "export{tick as settle}from'svelte';",
        ] {
            let compiled = compile_moonshine_module(source, Path::new("tick.ts"), target).unwrap();
            assert!(compiled.source.contains(runtime), "{}", compiled.source);
            assert_eq!(compiled.imports, ["svelte"]);
            assert!(!compiled.has_default_export);
            assert!(!compiled.source.contains("from \"svelte\""));
            if source.contains("settle") {
                assert!(compiled.source.contains("settle"));
            }
            let allocator = oxc_allocator::Allocator::default();
            let parsed =
                oxc_parser::Parser::new(&allocator, &compiled.source, oxc_span::SourceType::ts())
                    .parse();
            assert!(
                parsed.errors.is_empty(),
                "{:?}\n{}",
                parsed.errors,
                compiled.source
            );
        }
    }
}
