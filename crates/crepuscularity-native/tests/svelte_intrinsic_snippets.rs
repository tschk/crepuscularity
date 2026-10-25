#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;

#[test]
fn intrinsic_snippets_keep_outer_attributes_and_independent_child_scopes() {
    let source = r#"<script lang="ts">const item='outer';</script><section title={typeof item}>{#snippet item(value:number)}<b>{value}</b>{/snippet}{@render item(1)}<aside title={typeof item}>{#snippet item()}nested{/snippet}{@render item()}</aside></section><div>{#snippet item()}sibling{/snippet}{@render item()}</div>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Intrinsic.svelte"), target)
            .unwrap()
            .source;
        let allocator = Allocator::default();
        assert!(
            Parser::new(&allocator, &output, SourceType::tsx())
                .parse()
                .errors
                .is_empty(),
            "{output}"
        );
        assert!(
            output.contains("const item = \"outer\"") || output.contains("const item = 'outer'"),
            "{output}"
        );
        assert!(output.contains("typeof item"), "{output}");
        assert!(output.contains("__crepusInlineSnippet0"), "{output}");
        assert!(output.contains("__crepusInlineSnippet1"), "{output}");
        assert!(output.contains("__crepusInlineSnippet2"), "{output}");
    }
}

#[test]
fn escaped_outer_helper_names_are_not_captured_by_generated_inline_functions() {
    let source = r#"<script lang="ts">const \u005f_crepusInlineSnippet0=()=> 'outer0';const \u005f_crepusInlineSnippet1=()=> 'outer1';</script><section>{#snippet item()}<b>{\u005f_crepusInlineSnippet0()}</b>{/snippet}{@render item()}</section><div>{#snippet item()}<i>{\u005f_crepusInlineSnippet1()}</i>{/snippet}{@render item()}</div>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Escaped.svelte"), target)
            .unwrap()
            .source;
        assert!(
            output.contains("__crepusInlineSnippetHygiene0_"),
            "{output}"
        );
        assert!(
            output.contains("__crepusInlineSnippetHygiene1_"),
            "{output}"
        );
        assert!(output.contains("__crepusInlineSnippet0()"), "{output}");
        assert!(output.contains("__crepusInlineSnippet1()"), "{output}");
        assert!(
            !output.contains("function __crepusInlineSnippet0("),
            "{output}"
        );
        assert!(
            !output.contains("function __crepusInlineSnippet1("),
            "{output}"
        );
    }
}

#[test]
fn duplicate_local_snippets_and_direct_nested_block_declarations_remain_guarded() {
    for source in [
        "<section>{#snippet item()}one{/snippet}{#snippet item()}two{/snippet}</section>",
        "{#if true}{#snippet item()}one{/snippet}{/if}",
        "{#snippet outer()}{#snippet inner()}one{/snippet}{/snippet}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_svelte(source, Path::new("Invalid.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn intrinsic_snippet_directives_use_existing_authored_groups() {
    for (name, source) in [
        (
            "App.svelte",
            include_str!("fixtures/intrinsic-directives/App.svelte"),
        ),
        (
            "Child.svelte",
            include_str!("fixtures/intrinsic-directives/Child.svelte"),
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_svelte(source, Path::new(name), target)
                .unwrap()
                .source;
            assert!(!output.contains("__crepusDirective0"), "{output}");
            let allocator = Allocator::default();
            assert!(
                Parser::new(&allocator, &output, SourceType::tsx())
                    .parse()
                    .errors
                    .is_empty(),
                "{output}"
            );
            if name == "App.svelte" && matches!(target, MoonshineModuleTarget::Dom) {
                assert!(output.contains("installAction"), "{output}");
            }
        }
    }
}

#[test]
fn generated_controls_are_static_but_authored_local_components_remain_dynamic() {
    let controls = r#"<script lang="ts">let rows=$state([1]);let promise=Promise.resolve(1);</script>{#each rows as row}<b>{row}</b>{/each}{#key rows}<i>key</i>{/key}{#await promise}<u>pending</u>{:then value}<em>{value}</em>{/await}"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(controls, Path::new("Controls.svelte"), target)
            .unwrap()
            .source;
        assert!(!output.contains("__crepusSvelteDynamic"), "{output}");
        let local = r#"<script lang="ts">const Child=()=>null;let Current=$state(Child);</script><Current/><Child/>"#;
        let output = compile_moonshine_svelte(local, Path::new("Local.svelte"), target)
            .unwrap()
            .source;
        assert!(output.contains("__crepusSvelteDynamic"), "{output}");
        assert!(output.contains("Current.value"), "{output}");
        assert!(output.contains("typeof Child"), "{output}");
    }
}
