use crepuscularity_core::parser::{
    parse_svelte_module, parse_svelte_module_with_boundary,
    parse_svelte_module_with_styles_and_boundary,
};
#[test]
fn ordinary_authored_entries_keep_rejecting_styles() {
    let source = "<p>safe</p><style>:global(.safe){color:red}</style>";
    assert!(parse_svelte_module(source).is_err());
    assert!(parse_svelte_module_with_boundary(source, |_| None).is_err());
}
#[test]
fn explicit_style_host_receives_original_css() {
    let css = "\n:global(.safe) { content:'héllo'; }\n";
    let module = parse_svelte_module_with_styles_and_boundary(
        &format!("<p>safe</p><style lang=\"css\">{css}</style>"),
        |_| None,
    )
    .expect("style-aware host");
    assert_eq!(module.style.as_deref(), Some(css));
    assert!(!module.template_jsx.contains("<style"));
}
#[test]
fn explicit_style_host_rejects_unowned_attributes() {
    for attr in [
        "lang=\"scss\"",
        "src=\"external.css\"",
        "module",
        "global",
        "lang=\"css\" lang=\"css\"",
    ] {
        assert!(
            parse_svelte_module_with_styles_and_boundary(
                &format!("<p>safe</p><style {attr}>:global(.safe){{color:red}}</style>"),
                |_| None
            )
            .is_err(),
            "{attr}"
        );
    }
}
