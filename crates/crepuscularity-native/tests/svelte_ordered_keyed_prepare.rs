#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn authored_ordered_keyed_dispatch_prepares_before_returning_to_siblings() {
    let source = r#"<script lang="ts">let rows=$state([{id:1}]);</script><svg><g>{#each rows as row (row.id)}<a><title>{row.id}</title></a>{/each}</g><foreignObject><div><a>html</a></div></foreignObject></svg>"#;
    let dom = compile_moonshine_svelte(
        source,
        Path::new("Namespace.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert!(
        dom.contains("prepareInsertionRange as __crepusEachPrepare"),
        "{dom}"
    );
    assert!(
        dom.contains("__crepusEachPrepare(__crepusEachPrepared)"),
        "{dom}"
    );
    assert!(dom.contains("return __crepusEachPrepared"), "{dom}");
    assert!(dom.contains("__crepusOnce"), "{dom}");
    let ssr_source = r#"<script lang="ts">let rows=$state([{id:1}]);</script>{#each rows as row (row.id)}<p>{row.id}</p>{/each}"#;
    let ssr = compile_moonshine_svelte(
        ssr_source,
        Path::new("SSR.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap()
    .source;
    assert!(!ssr.contains("prepareInsertionRange"), "{ssr}");
}

#[test]
fn unkeyed_authored_dispatch_does_not_import_keyed_preparation() {
    let source = "<script>let rows=[1];</script>{#each rows as row}<p>{row}</p>{/each}";
    let dom = compile_moonshine_svelte(
        source,
        Path::new("Unkeyed.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert!(
        !dom.contains("prepareInsertionRange as __crepusEachPrepare"),
        "{dom}"
    );
}
