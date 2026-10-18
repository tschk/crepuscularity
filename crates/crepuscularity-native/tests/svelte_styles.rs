#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;
#[test]
fn scriptless_and_script_styles_use_explicit_global_asset_metadata() {
    for script in ["", "<script>let n=$state(0)</script>"] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output=compile_moonshine_svelte(&format!("{script}<p class=\"rule\">safe</p><style>:global(.rule) {{ color:red; }}</style>"),Path::new("Global.svelte"),target).expect("all-global style compiles");
            assert_eq!(output.styles, [".rule { color:red; }"]);
            assert!(!output.source.contains("svelte-"));
        }
    }
}
#[test]
fn global_block_and_comment_only_css_do_not_need_scoped_classes() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(
            "<p>safe</p><style>:global { .rule {color:red} }</style>",
            Path::new("Global.svelte"),
            target,
        )
        .expect("global block");
        assert_eq!(output.styles, [" .rule {color:red} "]);
        let empty = compile_moonshine_svelte(
            "<p>safe</p><style>/* empty */</style>",
            Path::new("Empty.svelte"),
            target,
        )
        .expect("empty CSS");
        assert!(empty.styles.is_empty());
    }
}
#[test]
fn unsupported_selector_forms_and_preprocessed_styles_fail_closed() {
    for body in [
        "<style>.rule:is(.x){color:red}</style>",
        "<style>.rule :global(.outside) .local{color:red}</style>",
        "<style lang=\"scss\">:global(.rule){color:red}</style>",
        "<style>:global(.rule){background:url(x)}</style>",
        "<style>:global(.rule){color red}</style>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_svelte(
                    &format!("<p>safe</p>{body}"),
                    Path::new("Bad.svelte"),
                    target
                )
                .is_err(),
                "{body}"
            );
        }
    }
}

#[test]
fn local_scope_hash_matches_targets_and_compile_only_markers_are_erased() {
    let source = r#"<script>let classes=$state(['inner']);</script><div class="outer"><span class={classes}>local</span></div><aside>unrelated</aside><style>.outer > .inner{color:red}hr.absent{color:blue}</style>"#;
    let dom = compile_moonshine_svelte(
        source,
        Path::new("Scoped.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .expect("DOM scoped CSS");
    let ssr = compile_moonshine_svelte(
        source,
        Path::new("Scoped.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .expect("SSR scoped CSS");
    assert_eq!(dom.styles, ssr.styles);
    assert!(dom.styles[0].contains(":where(.crepus-v1-"));
    assert!(!dom.styles[0].contains("hr.absent"));
    for output in [dom, ssr] {
        assert!(!output.source.contains("__crepusSvelteScope0"));
        assert!(output
            .source
            .contains("classValue as __crepusSvelteScopedClass"));
        assert!(output.source.contains("@tschk/moonshine-svelte/class"));
    }
}
