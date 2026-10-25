#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn exact_pre_boundary_uses_textarea_value_and_inline_key_without_actions() {
    let source = include_str!("fixtures/pre-boundary-ordered/pre-boundary-reference.svelte");
    let output = compile_moonshine_svelte(
        source,
        Path::new("pre-boundary-reference.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert!(output.contains("value: __crepusTextareaMemo("), "{output}");
    assert!(
        output.contains("For as __crepusDirectiveNativeFor"),
        "{output}"
    );
    assert!(
        output.contains("__crepusDirectiveNativeFor0(props, (read) => read)"),
        "{output}"
    );
    // The direct Text expression is consumed into value; it must not become an
    // ordinary text/range child of the textarea node.
    let textarea = output.find("\"textarea\"").unwrap();
    let tail = &output[textarea..];
    assert!(
        tail.find("value: __crepusTextareaMemo(").unwrap() < tail.find("readUpdates").unwrap(),
        "{output}"
    );
}

#[test]
fn textarea_literal_expression_and_default_protocol_remain_distinct() {
    for source in [
        "<textarea>literal body</textarea>",
        "<script>let value=$state('body')</script><textarea>{value}</textarea>",
        "<script>let value=$state('body');let initial=$state('default')</script><textarea defaultValue={initial}>{value}</textarea>",
        "<textarea defaultValue='default'>literal body</textarea>",
    ] {
        let dom = compile_moonshine_svelte(source, Path::new("Textarea.svelte"), MoonshineModuleTarget::Dom).unwrap().source;
        assert!(dom.contains("textarea"), "{dom}");
        assert!(!dom.contains("document.createTextNode"), "{dom}");
        compile_moonshine_svelte(source, Path::new("Textarea.svelte"), MoonshineModuleTarget::Ssr).unwrap();
    }
}
