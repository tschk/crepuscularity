#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use std::path::Path;

fn compile(source: &str, target: MoonshineModuleTarget) -> crepuscularity_native::MoonshineModule {
    compile_moonshine_module(source, Path::new("Lists.ts"), target).unwrap()
}

fn reactive(target: MoonshineModuleTarget) -> &'static str {
    match target {
        MoonshineModuleTarget::Dom => "@tschk/moonshine-dom/reactive",
        MoonshineModuleTarget::Ssr => "@tschk/moonshine-dom/ssr",
    }
}

#[test]
fn mixed_named_imports_route_lists_and_reactivity_without_duplicate_metadata() {
    for source in [
        "import {mapArray as map, indexArray as index, createSignal, type Accessor} from 'solid-js'; export {map,index,createSignal};",
        "import {'mapArray' as map, 'indexArray' as index, createSignal, type Accessor} from 'solid-js'; export {map,index,createSignal};",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile(source,target);
            assert_eq!(output.imports,["solid-js"]);
            assert!(!output.has_default_export);
            assert!(output.has_module_declarations);
            assert!(!output.source.contains("solid-js"),"{}",output.source);
            assert!(output.source.contains("@tschk/moonshine-dom/control"),"{}",output.source);
            assert!(output.source.contains(reactive(target)),"{}",output.source);
            assert!(output.source.contains("as map") && output.source.contains("as index"),"{}",output.source);
            assert!(output.source.contains("createSignal") && output.source.contains("Accessor"),"{}",output.source);
            assert!(!output.source.contains("import { h as"),"non-JSX helper gained host import: {}",output.source);
        }
    }
}

#[test]
fn reexports_keep_aliases_default_string_names_and_type_only_declarations() {
    for (source,default,names) in [
        ("export {mapArray as map,indexArray as index,createSignal as state,type Accessor} from 'solid-js';",false,vec!["as map","as index","as state","Accessor"]),
        ("export {mapArray as default,createSignal as state} from 'solid-js';",true,vec!["as default","as state"]),
        ("export {mapArray as 'mapped-list',indexArray as 'indexed-list',createSignal as state} from 'solid-js';",false,vec!["mapped-list","indexed-list","as state"]),
        ("export type {mapArray as MapFunction,indexArray as IndexFunction,Accessor} from 'solid-js';",false,vec!["MapFunction","IndexFunction","Accessor"]),
        ("import type {mapArray as MapFunction,indexArray as IndexFunction,Accessor} from 'solid-js'; export type MapType=typeof MapFunction;",false,vec!["MapFunction","IndexFunction","Accessor"]),
    ] {
        for target in [MoonshineModuleTarget::Dom,MoonshineModuleTarget::Ssr] {
            let output=compile(source,target);
            assert_eq!(output.imports,["solid-js"]);
            assert_eq!(output.has_default_export,default,"{}",output.source);
            assert!(output.source.contains("@tschk/moonshine-dom/control"),"{}",output.source);
            assert!(output.source.contains(reactive(target)),"{}",output.source);
            assert!(!output.source.contains("solid-js"),"{}",output.source);
            for name in &names { assert!(output.source.contains(name),"missing {name}: {}",output.source); }
            if source.contains("type {") { assert!(output.source.contains("type"),"{}",output.source); }
        }
    }
}

#[test]
fn import_type_queries_route_each_qualified_symbol_to_its_actual_module() {
    let source="export type MapType=typeof import('solid-js').mapArray; export type IndexType=typeof import('solid-js').indexArray; export type Read=import('solid-js').Accessor<number>;";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(source, target);
        assert_eq!(output.imports, ["solid-js"; 3]);
        assert_eq!(
            output
                .source
                .matches("@tschk/moonshine-dom/control")
                .count(),
            2,
            "{}",
            output.source
        );
        assert_eq!(
            output.source.matches(reactive(target)).count(),
            1,
            "{}",
            output.source
        );
        assert!(
            output.source.contains(".mapArray") && output.source.contains(".indexArray"),
            "{}",
            output.source
        );
        assert!(
            output.source.contains("Accessor<number>"),
            "{}",
            output.source
        );
    }
}

#[test]
fn unsupported_solid_import_surfaces_stay_hard_errors() {
    for source in [
        "export {For} from 'solid-js';",
        "export {Show} from 'solid-js';",
        "export {mapArray,unknown} from 'solid-js';",
        "import * as Solid from 'solid-js'; export {Solid};",
        "import Solid from 'solid-js'; export {Solid};",
        "export * from 'solid-js';",
        "export const load=()=>import('solid-js');",
        "export {mapArray} from 'solid-js/web';",
        "export {mapArray} from 'solid-js/store';",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_module(source, Path::new("Lists.ts"), target).is_err(),
                "accepted {source}"
            );
        }
    }
}

#[test]
fn mixed_declaration_attributes_are_preserved_on_each_split_or_rejected() {
    for source in [
        "import {mapArray,createSignal} from 'solid-js' with {type:'json'}; export {mapArray,createSignal};",
        "export {mapArray,createSignal} from 'solid-js' with {type:'json'};",
    ] {
        for target in [MoonshineModuleTarget::Dom,MoonshineModuleTarget::Ssr] {
            if let Ok(output)=compile_moonshine_module(source,Path::new("Lists.ts"),target) {
                assert_eq!(output.source.matches("with {").count(),2,"declaration attributes lost while splitting: {}",output.source);
                assert_eq!(output.source.matches("json").count(),2,"{}",output.source);
            }
        }
    }
}
