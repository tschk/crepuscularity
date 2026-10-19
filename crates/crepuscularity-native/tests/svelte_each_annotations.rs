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
    let module = compile_moonshine_svelte(source, Path::new("App.svelte"), target).unwrap();
    assert_eq!(module.language, MoonshineModuleLanguage::TypeScript);
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &module.source, SourceType::tsx()).parse();
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
fn typed_scalar_and_existing_object_contexts_compile_as_complete_graph_inputs() {
    let prefix="<script lang='ts'>type Row={id:number,nested:{value:number}};let rows:Row[]=[{id:1,nested:{value:2}}];let sample=1;</script>";
    for body in [
        "{#each [1] as item:number,index (item)}<p>{item.toFixed()}:{index}</p>{/each}",
        "{#each rows as row:Row,index (row.id)}<p>{row.nested.value}:{index}</p>{/each}",
        "{#each rows as row:Row}<input type='number' bind:value={row.nested.value}/>{/each}",
        "{#each rows as row}<input type='number' bind:value={row.nested.value}/>{/each}",
        "{#each rows as {nested}:Row}<input type='number' bind:value={nested.value}/>{/each}",
        "{#each rows as {nested:{value:renamed}}:Row,index (renamed)}<p>{renamed}:{index}</p>{/each}",
        "{#each [1] as item:typeof sample}<p>{item}</p>{/each}",
        "{#each rows as {id}:{id?:number}}<p>{id}</p>{/each}",
    ] {
        for target in [MoonshineModuleTarget::Dom,MoonshineModuleTarget::Ssr] {
            compile(&format!("{prefix}{body}"),target);
        }
    }
}

#[test]
fn typed_context_bound_names_still_constrain_const_payloads() {
    let prefix = "<script lang='ts'>type Row={value:number};const rows:Row[]=[];</script>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        compile(&format!("{prefix}{{#each rows as {{value}}:Row}}{{@const copy=value}}<p>{{copy}}</p>{{/each}}"),target);
        let error=compile_moonshine_svelte(&format!("{prefix}{{#each rows as {{value}}:Row}}{{@const value=1}}<p>{{value}}</p>{{/each}}"),Path::new("App.svelte"),target).unwrap_err();
        assert!(
            error.contains("const") && error.contains("value"),
            "{error}"
        );
    }
}

#[test]
fn unsupported_patterns_get_native_diagnostics_after_correct_boundary_recognition() {
    for context in [
        "{[field]:value}:Row",
        "{0:value}:Row",
        "{nested:{[field]:value}}:Row",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source=format!("<script lang='ts'>type Row={{value:number}};const rows=[];</script>{{#each rows as {context}}}<p>item</p>{{/each}}");
            let error =
                compile_moonshine_svelte(&source, Path::new("App.svelte"), target).unwrap_err();
            assert!(error.contains("computed/numeric pattern keys"), "{error}");
            assert!(!error.contains("unclosed"), "{error}");
        }
    }
}

#[test]
fn typed_contexts_in_original_javascript_fail_closed() {
    for context in [
        "item:number",
        "{value}:{value:number}",
        "item:typeof sample",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source=format!("<script>const rows=[];let sample=1;</script>{{#each rows as {context}}}<p>item</p>{{/each}}");
            assert!(
                compile_moonshine_svelte(&source, Path::new("App.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
}
