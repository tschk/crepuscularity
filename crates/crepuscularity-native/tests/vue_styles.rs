#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_module, compile_moonshine_vue, MoonshineModuleTarget,
};
use std::path::Path;
#[test]
fn plain_vue_css_is_explicit_metadata_for_both_targets_and_other_modules_stay_empty() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let source = "<template><p class=\"mark\">text</p></template><style>\n.mark { color: red; }\n</style><style lang=\"css\">b{color:blue}</style>";
        let compiled = compile_moonshine_vue(source, Path::new("App.vue"), target).unwrap();
        assert_eq!(
            compiled.styles,
            vec!["\n.mark { color: red; }\n", "b{color:blue}"]
        );
        assert!(!compiled.source.contains(".css"));
        let module = compile_moonshine_module(
            "export default function App(){return <p/>}",
            Path::new("App.tsx"),
            target,
        )
        .unwrap();
        assert!(module.styles.is_empty());
    }
}
#[test]
fn unsupported_style_modes_and_external_features_fail_explicitly() {
    for style in [
        "<style scoped>p{color:red}</style>",
        "<style scoped=\"false\">p{color:red}</style>",
        "<style module=\"false\">p{color:red}</style>",
        "<style lang=\"scss\">p{color:red}</style>",
        "<style global>p{color:red}</style>",
        "<style src=\"./external.css\"></style>",
        "<style>p{color:v-bind(tone)}</style>",
        "<style>@import './a.css';</style>",
        "<style>p{background:url(a.png)}</style>",
        "<style>.escaped\\31 {color:red}</style>",
    ] {
        let source = format!("<template><p>text</p></template>{style}");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_vue(&source, Path::new("App.vue"), target).is_err(),
                "{style}"
            );
        }
    }
}

#[test]
fn duplicate_style_attributes_reject_both_orders_but_boolean_lang_remains_plain_css() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for attrs in [
            "lang=\"css\" lang=\"scss\"",
            "lang=\"scss\" lang=\"css\"",
            "scoped=\"false\" scoped",
            "module=\"false\" module",
        ] {
            let source =
                format!("<template><p>text</p></template><style {attrs}>p{{color:red}}</style>");
            let error = compile_moonshine_vue(&source, Path::new("App.vue"), target).unwrap_err();
            assert!(
                error.contains("duplicate Vue SFC style attribute"),
                "{error}"
            );
        }
        let plain = compile_moonshine_vue(
            "<template><p>text</p></template><style lang>p{color:red}</style>",
            Path::new("App.vue"),
            target,
        )
        .unwrap();
        assert_eq!(plain.styles, ["p{color:red}"]);
    }
}

#[test]
fn empty_plain_styles_omit_assets_after_mode_validation() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for body in ["", "  \n  "] {
            let source = format!("<template><p>text</p></template><style>{body}</style>");
            assert!(compile_moonshine_vue(&source, Path::new("App.vue"), target)
                .unwrap()
                .styles
                .is_empty());
            for mode in ["scoped", "module", "lang=\"scss\""] {
                let source =
                    format!("<template><p>text</p></template><style {mode}>{body}</style>");
                assert!(compile_moonshine_vue(&source, Path::new("App.vue"), target).is_err());
            }
        }
    }
}

#[test]
fn unknown_style_attributes_are_rejected_only_by_native_backend() {
    for attribute in ["global", "data-note=\"x>y\""] {
        let source = format!("<template><p /></template><style {attribute}>p{{color:red}}</style>");
        assert!(crepuscularity_core::parser::parse_vue_sfc(&source).is_ok());
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_vue(&source, Path::new("Unknown.vue"), target)
                    .unwrap_err()
                    .contains("unsupported Vue style attribute")
            );
        }
    }
}
