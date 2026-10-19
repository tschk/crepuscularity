#![cfg(feature = "web-modules")]
use crepuscularity_core::parser::parse_vue_sfc;
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn empty_css_language_uses_default_compilation_without_normalizing_source_metadata() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let mut baseline = None;
        for (attribute, language) in [
            ("", None),
            ("lang", None),
            ("lang=\"\"", Some("")),
            ("lang=\"css\"", Some("css")),
        ] {
            let source=format!("<template><p class=\"mark\">text</p></template><style {attribute}>.mark{{color:red}}</style>");
            assert_eq!(
                parse_vue_sfc(&source).unwrap().styles[0].lang.as_deref(),
                language
            );
            let output = compile_moonshine_vue(&source, Path::new("Entry.vue"), target).unwrap();
            assert_eq!(output.styles, [".mark{color:red}"]);
            if let Some(expected) = &baseline {
                assert_eq!(&output.source, expected);
            } else {
                baseline = Some(output.source);
            }
        }
    }
}

#[test]
fn empty_css_language_keeps_restrictions_and_omits_empty_assets() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_vue(
            "<template><p>text</p></template><style lang=\"\"> \n </style>",
            Path::new("Empty.vue"),
            target,
        )
        .unwrap();
        assert!(output.styles.is_empty());
        for style in [
            "<style lang=\"\" scoped>p{color:red}</style>",
            "<style lang=\"\" module>p{color:red}</style>",
            "<style lang=\"\">p{background:image-set('x.png' 1x)}</style>",
        ] {
            let source = format!("<template><p>text</p></template>{style}");
            assert!(compile_moonshine_vue(&source, Path::new("Unsupported.vue"), target).is_err());
        }
    }
}
