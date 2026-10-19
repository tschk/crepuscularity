use crepuscularity_core::parser::{
    parse_vue_sfc, parse_vue_sfc_with_generics, parse_vue_sfc_with_metadata, VueScriptBlock,
    VueSfc, VueStyleBlock,
};

#[test]
fn original_public_vue_struct_literals_remain_source_compatible() {
    let script = VueScriptBlock {
        setup: true,
        lang: Some("ts".to_string()),
        content: "const value = 1;".to_string(),
    };
    let sfc = VueSfc {
        template: Some("<p>Original API</p>".to_string()),
        scripts: vec![script],
        styles: vec![VueStyleBlock {
            scoped: false,
            module: false,
            lang: Some("css".to_string()),
            content: "p { color: red }".to_string(),
        }],
    };
    assert_eq!(sfc.scripts[0].content, "const value = 1;");
    assert!(sfc.scripts[0].setup);
    assert_eq!(sfc.scripts[0].lang.as_deref(), Some("ts"));
    assert_eq!(sfc.template.as_deref(), Some("<p>Original API</p>"));
    assert_eq!(sfc.styles[0].content, "p { color: red }");
    assert_eq!(sfc.styles[0].lang.as_deref(), Some("css"));
}

#[test]
fn additive_generic_metadata_preserves_script_order_without_changing_sfc() {
    let source = r#"<script>export const ordinary = 1;</script><script setup lang="ts" generic="T extends string">const value = 2;</script><template><p>API</p></template>"#;
    let (sfc, generics) = parse_vue_sfc_with_generics(source).unwrap();
    assert_eq!(generics, vec![None, Some("T extends string".to_string())]);
    assert_eq!(sfc.scripts.len(), 2);
    assert!(!sfc.scripts[0].setup);
    assert!(sfc.scripts[1].setup);
    assert_eq!(sfc.scripts[0].content, "export const ordinary = 1;");
    assert_eq!(sfc.scripts[1].content, "const value = 2;");
    assert_eq!(sfc.scripts[1].lang.as_deref(), Some("ts"));

    let original = parse_vue_sfc(source).unwrap();
    assert_eq!(original.template, sfc.template);
    assert_eq!(original.scripts.len(), sfc.scripts.len());
    for (old, additive) in original.scripts.iter().zip(&sfc.scripts) {
        assert_eq!(old.setup, additive.setup);
        assert_eq!(old.lang, additive.lang);
        assert_eq!(old.content, additive.content);
    }
}

#[test]
fn empty_generic_attribute_is_distinct_from_absent_metadata() {
    let (sfc, generics) = parse_vue_sfc_with_generics(
        r#"<script setup generic=""></script><script lang="js">const value = 1;</script><template/>"#,
    )
    .unwrap();
    assert_eq!(generics, vec![Some(String::new()), None]);
    assert_eq!(generics.len(), sfc.scripts.len());
    assert!(sfc.scripts[0].content.is_empty());
    assert_eq!(sfc.scripts[1].lang.as_deref(), Some("js"));
}

#[test]
fn additive_sfc_metadata_keeps_script_and_style_attributes_aligned() {
    let source = r#"<script>export const ordinary=1;</script><script setup lang="ts" generic="T extends string">const value=2;</script><template><p>Metadata</p></template><style global data-note="first">p { color:red }</style><style scoped lang="css" data-note="second">p { color:blue }</style>"#;
    let (sfc, metadata) = parse_vue_sfc_with_metadata(source).unwrap();
    assert_eq!(
        metadata.script_generics,
        vec![None, Some("T extends string".to_string())]
    );
    assert_eq!(metadata.script_generics.len(), sfc.scripts.len());
    assert_eq!(
        metadata.style_attributes,
        vec![
            vec!["global".to_string(), "data-note".to_string()],
            vec![
                "scoped".to_string(),
                "lang".to_string(),
                "data-note".to_string()
            ]
        ]
    );
    assert_eq!(metadata.style_attributes.len(), sfc.styles.len());
    assert_eq!(sfc.styles[0].content, "p { color:red }");
    assert!(!sfc.styles[0].scoped);
    assert_eq!(sfc.styles[1].content, "p { color:blue }");
    assert!(sfc.styles[1].scoped);
    assert_eq!(sfc.styles[1].lang.as_deref(), Some("css"));

    let original = parse_vue_sfc(source).unwrap();
    let (with_generics, generics) = parse_vue_sfc_with_generics(source).unwrap();
    assert_eq!(generics, metadata.script_generics);
    for projection in [original, with_generics] {
        assert_eq!(projection.template, sfc.template);
        assert_eq!(projection.scripts.len(), sfc.scripts.len());
        assert_eq!(projection.styles.len(), sfc.styles.len());
        for (actual, expected) in projection.scripts.iter().zip(&sfc.scripts) {
            assert_eq!(actual.setup, expected.setup);
            assert_eq!(actual.lang, expected.lang);
            assert_eq!(actual.content, expected.content);
        }
        for (actual, expected) in projection.styles.iter().zip(&sfc.styles) {
            assert_eq!(actual.scoped, expected.scoped);
            assert_eq!(actual.module, expected.module);
            assert_eq!(actual.lang, expected.lang);
            assert_eq!(actual.content, expected.content);
        }
    }
}
