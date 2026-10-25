#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn known_svg_raw_markup_rejects_before_control_relocation() {
    let original = include_str!("fixtures/raw-html-namespace/original-svg.svelte");
    for source in [
        original.to_owned(),
        "<script>let value='<circle/>'</script>{#snippet body()}<svg>{@html value}</svg>{/snippet}{@render body()}".into(),
        "<script>let value='<circle/>';let rows=[1]</script><svg>{#each rows as row}<g>{@html value}</g>{/each}</svg>".into(),
        "<script>let value='<circle/>';let promise=Promise.resolve(1)</script><svg>{#await promise}<g>{@html value}</g>{:then value}<g>{@html value}</g>{:catch error}<g>{@html error}</g>{/await}</svg>".into(),
        "<script>let value='<circle/>';let show=true</script><svg>{#if show}{@html value}{/if}</svg>".into(),
        "<script>let value='<circle/>'</script><svg><foreignObject><div><svg>{@html value}</svg></div></foreignObject></svg>".into(),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_svelte(&source, Path::new("SVG.svelte"), target).unwrap_err();
            assert!(error.contains("SVG namespace"), "{error}\n{source}");
        }
    }
}

#[test]
fn html_and_foreign_object_children_keep_html_boundary() {
    for source in [
        "<script>let value='<b>html</b>'</script><div>{@html value}</div>",
        "<script>let value='<b>html</b>'</script><svg><foreignObject><div>{@html value}</div></foreignObject></svg>",
        "<script>let value='<b>html</b>';let rows=[1]</script><svg><foreignObject><div>{#each rows as row}{@html value}{/each}</div></foreignObject></svg>",
    ] {
        compile_moonshine_svelte(source, Path::new("HTML.svelte"), MoonshineModuleTarget::Dom).unwrap();
    }
    compile_moonshine_svelte(
        "<script>let value='<b>html</b>'</script><div>{@html value}</div>",
        Path::new("HTML.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap();
}
