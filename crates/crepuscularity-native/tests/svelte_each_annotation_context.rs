#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn annotation_context_fixtures_emit_both_target_helper_contracts() {
    for (name, source) in [
        (
            "OriginalTypeof.svelte",
            include_str!("fixtures/each-annotation-context/OriginalTypeof.svelte"),
        ),
        (
            "Mixed.svelte",
            include_str!("fixtures/each-annotation-context/Mixed.svelte"),
        ),
        (
            "InvalidBroad.svelte",
            include_str!("fixtures/each-annotation-context/InvalidBroad.svelte"),
        ),
        (
            "InvalidLiteral.svelte",
            include_str!("fixtures/each-annotation-context/InvalidLiteral.svelte"),
        ),
        (
            "InvalidMember.svelte",
            include_str!("fixtures/each-annotation-context/InvalidMember.svelte"),
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            // Invalid* sources are syntax-valid: their emitted graphs must fail the
            // real strict checkers, not a compiler-specific handwritten type checker.
            let output = compile_moonshine_svelte(source, Path::new(name), target)
                .unwrap()
                .source;
            assert!(output.contains("__crepusEachAnnotatedChildren"), "{output}");
            assert!(!output.contains("satisfies"), "{output}");
            if name == "Mixed.svelte" {
                assert!(output.contains("__crepusEachChildren"), "{output}");
                assert!(
                    output.contains("__crepusEachAnnotatedSnapshotChildren")
                        == matches!(target, MoonshineModuleTarget::Ssr),
                    "{output}"
                );
            }
        }
    }
}
