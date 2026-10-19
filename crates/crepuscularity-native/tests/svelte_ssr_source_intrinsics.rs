#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_module, compile_moonshine_svelte, MoonshineModuleTarget,
};
use std::path::Path;

#[test]
fn source_intrinsics_delay_children_and_preserve_literal_origin() {
    for script in ["", " lang=\"ts\""] {
        let source = format!(
            r#"<script{script}>let dynamic='false';function read(){{return dynamic}}</script><div title={{read()}}><input disabled={{'false'}} hidden={{dynamic}} value={{undefined}}/><p>child</p></div>"#
        );
        let output = compile_moonshine_svelte(
            &source,
            Path::new("Origin.svelte"),
            MoonshineModuleTarget::Ssr,
        )
        .unwrap()
        .source;
        assert!(
            output.contains("hSource as __crepusSourceIntrinsic"),
            "{output}"
        );
        assert!(
            output.contains("peekComponentContext as __crepusSourceConsumer"),
            "{output}"
        );
        assert!(
            output.contains("withComponentConsumer as __crepusSourceWithConsumer"),
            "{output}"
        );
        assert!(output.contains("disabled: \"false\""), "{output}");
        assert!(output.contains("hidden: () => dynamic"), "{output}");
        assert!(output.contains("__crepusSourceChild(() =>"), "{output}");
        assert!(!output.contains("getRuneConsumer"), "{output}");
    }
}

#[test]
fn source_component_props_and_snippet_calls_keep_existing_component_factory() {
    let source = r#"<script lang="ts">import Child from './Child.svelte';import{getContext}from'svelte';</script>{#snippet item()}<p title={getContext('name')}>text</p>{/snippet}<section title={getContext('name')}><Child title={getContext('name')}>{@render item()}</Child></section>"#;
    let output = compile_moonshine_svelte(
        source,
        Path::new("Context.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap()
    .source;
    assert!(
        output.contains("hSource as __crepusSourceIntrinsic"),
        "{output}"
    );
    assert!(
        !output.contains("__crepusSourceIntrinsic(Child"),
        "{output}"
    );
    assert!(!output.contains("runWithOwner(consumer"), "{output}");
}

#[test]
fn generic_jsx_and_raw_svg_html_guards_remain_unchanged() {
    let generic = compile_moonshine_module(
        "export default function App(){return <div title={'x'}><p>child</p></div>}",
        Path::new("App.tsx"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap()
    .source;
    assert!(!generic.contains("hSource"), "{generic}");
    assert!(!generic.contains("SourceChild"), "{generic}");
    let source_svg = compile_moonshine_svelte(
        "<svg><g><title>name</title><a href='/'>link</a></g><foreignObject><p>html</p></foreignObject></svg>",
        Path::new("Svg.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap().source;
    assert!(source_svg.contains("hSource as"), "{source_svg}");
    for tag in ["circle", "path"] {
        let output = compile_moonshine_svelte(
            &format!("<svg><{tag}/></svg>"),
            Path::new("Svg.svelte"),
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert!(output.source.contains("hSource as"), "{}", output.source);
        assert!(
            output.source.contains(&format!("\"{tag}\"")),
            "{}",
            output.source
        );
    }
    assert!(compile_moonshine_svelte(
        "<svg>{@html markup}</svg>",
        Path::new("Unknown.svelte"),
        MoonshineModuleTarget::Ssr
    )
    .is_err());
    assert!(compile_moonshine_module(
        "export default function App(){return <svg><g/></svg>}",
        Path::new("Generic.tsx"),
        MoonshineModuleTarget::Ssr
    )
    .is_err());
}
