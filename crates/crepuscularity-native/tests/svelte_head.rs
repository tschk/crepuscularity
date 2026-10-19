#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

const TARGETS: [MoonshineModuleTarget; 2] =
    [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr];

#[test]
fn authored_head_compiles_to_independent_target_runtime() {
    let source = r#"<script lang="ts">let name=$state('one');</script>
<svelte:head><title>  {name}  </title><meta name="description" content={name}/></svelte:head><p>{name}</p>"#;
    for target in TARGETS {
        let module = compile_moonshine_svelte(source, Path::new("App.svelte"), target).unwrap();
        assert!(
            module.source.contains("@tschk/moonshine-dom/head"),
            "{}",
            module.source
        );
        assert!(!module.source.contains("__CrepusHead"), "{}", module.source);
        assert!(
            !module.source.contains("__CrepusTitle"),
            "{}",
            module.source
        );
        assert!(
            !module.source.contains("svelte/internal"),
            "{}",
            module.source
        );
        assert!(module.has_default_export);
    }
}

#[test]
fn authored_head_rejects_misplacement_and_invalid_title_content() {
    for source in [
        "<div><svelte:head/></div>",
        "{#if true}<svelte:head/>{/if}",
        "<svelte:head/><svelte:head/>",
        "<svelte:head lang='en'/>",
        "<svelte:head><title lang='en'>x</title></svelte:head>",
        "<svelte:head><title><b>x</b></title></svelte:head>",
        "<svelte:head><title><!--x--></title></svelte:head>",
        "<svelte:head><title>{@html 'x'}</title></svelte:head>",
    ] {
        for target in TARGETS {
            assert!(
                compile_moonshine_svelte(source, Path::new("Invalid.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn head_preserves_empty_titles_svg_titles_and_conditional_title_fragments() {
    for source in [
        "<svelte:head><title/></svelte:head>",
        "<svelte:head><title> </title></svelte:head>",
        "<svelte:head><title>one</title><title>two</title></svelte:head>",
        "<svelte:head>{#if true}<title>one</title>{/if}<title>two</title></svelte:head>",
        "<svelte:head><svg><title>SVG</title></svg></svelte:head>",
        "<title>body</title>",
    ] {
        for target in TARGETS {
            compile_moonshine_svelte(source, Path::new("Valid.svelte"), target).unwrap();
        }
    }
}
