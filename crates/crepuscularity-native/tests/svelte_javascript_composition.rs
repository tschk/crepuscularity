#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_svelte, MoonshineModuleLanguage, MoonshineModuleTarget,
};
use std::path::Path;

#[test]
fn raw_html_retains_javascript_computed_override_semantics_in_both_targets() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for declaration in [
            "class Counter{x=$state(1);['x'](){return 9}}",
            "class Counter{constructor(){this.x=$state(1)}['x'](){return 9}}",
        ] {
            let source = format!(
                "<script>{declaration}const model=new Counter();export function read(){{return model.x()}}</script><div>{{@html '<b>x</b>'}}</div>"
            );
            let module = compile_moonshine_svelte(&source, Path::new("Raw.svelte"), target)
                .unwrap_or_else(|error| panic!("{error}\n{source}"));
            assert_eq!(module.language, MoonshineModuleLanguage::JavaScript);
            assert!(module.source.contains("return 9"), "{}", module.source);
            let helper = match target {
                MoonshineModuleTarget::Dom => "htmlContent as",
                MoonshineModuleTarget::Ssr => "rawHTML as",
            };
            assert!(module.source.contains(helper), "{}", module.source);
        }
    }
}

#[test]
fn raw_html_does_not_relax_original_javascript_or_typed_class_validation() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for script in [
            "<script>class Counter{#x;#x}</script>",
            "<script>const value:number=1;</script>",
        ] {
            let source = format!("{script}<div>{{@html '<b>x</b>'}}</div>");
            let error = compile_moonshine_svelte(&source, Path::new("Raw.svelte"), target)
                .expect_err("original JavaScript must still be validated");
            assert!(error.contains("invalid authored JavaScript"), "{error}");
        }
        let source = "<script lang='ts'>class Counter{x=$state(1);['x'](){return 9}}</script><div>{@html '<b>x</b>'}</div>";
        let error = compile_moonshine_svelte(source, Path::new("Raw.svelte"), target)
            .expect_err("TypeScript retains its duplicate class-member validation");
        assert!(
            error.contains("invalid Svelte instance export scope"),
            "{error}"
        );
    }
}

#[test]
fn javascript_each_headers_keep_computed_overrides_in_both_targets() {
    let source = "{#each [class {x;['x'](){return 9}}] as Item}<p>item</p>{/each}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let module = compile_moonshine_svelte(source, Path::new("Header.svelte"), target)
            .unwrap_or_else(|error| panic!("{error}\n{source}"));
        assert_eq!(module.language, MoonshineModuleLanguage::JavaScript);
        assert!(module.source.contains("return 9"), "{}", module.source);
    }
}

#[test]
fn each_header_language_validation_rejects_original_private_and_typed_errors() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let private = "{#each [class {#x;#x}] as Item}<p/>{/each}";
        let error = compile_moonshine_svelte(private, Path::new("Header.svelte"), target)
            .expect_err("private duplicate names remain invalid in JavaScript");
        assert!(error.contains("each requires"), "{error}");
        for source in [
            "{#each ([1] as number[]) as item}<p>{item}</p>{/each}",
            "<script lang='ts'></script>{#each [class {x;['x'](){return 9}}] as Item}<p/>{/each}",
        ] {
            let error = compile_moonshine_svelte(source, Path::new("Header.svelte"), target)
                .expect_err("the selected source language must validate the each expression");
            assert!(error.contains("each requires"), "{error}");
        }
    }
}

#[test]
fn typed_each_headers_retain_typescript_assertions() {
    let source = "<script lang='ts'></script>{#each ([1] as number[]) as item}<p>{item}</p>{/each}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let module = compile_moonshine_svelte(source, Path::new("Header.svelte"), target)
            .unwrap_or_else(|error| panic!("{error}\n{source}"));
        assert_eq!(module.language, MoonshineModuleLanguage::TypeScript);
    }
}

#[test]
fn await_headers_and_branch_bindings_validate_the_authored_language() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for template in [
            "{#await (1 as number) then value}<p>{value}</p>{/await}",
            "{#await 1}<p>pending</p>{:then value:number}<p>{value}</p>{/await}",
            "{#await 1 then value:number}<p>{value}</p>{/await}",
        ] {
            compile_moonshine_svelte(template, Path::new("Await.svelte"), target)
                .expect_err("TypeScript syntax requires the authored TypeScript mode");
            let typed = format!("<script lang='ts'></script>{template}");
            let module = compile_moonshine_svelte(&typed, Path::new("Await.svelte"), target)
                .unwrap_or_else(|error| panic!("{error}\n{typed}"));
            assert_eq!(module.language, MoonshineModuleLanguage::TypeScript);
        }
    }
}
