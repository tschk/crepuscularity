use crepuscularity_core::{
    diagnose_crepus_source_with_path, parse_template, parse_template_with_path,
};
use std::path::Path;

#[test]
fn leading_bom_preserves_template_ast_for_every_frontend() {
    for (filename, source) in [
        ("view.crepus", "div\n  \"hello\""),
        ("view.jsx", "<div>hello</div>"),
        ("view.vue", "<template><div>hello</div></template>"),
        ("view.svelte", "<div>hello</div>"),
        ("view.astro", "<div>hello</div>"),
        ("view.component.html", "<div>hello</div>"),
    ] {
        let path = Some(Path::new(filename));
        let expected = parse_template_with_path(source, path).unwrap();
        let actual = parse_template_with_path(&format!("\u{feff}{source}"), path).unwrap();
        assert_eq!(actual, expected, "{filename}");
    }
    let source = "<div>hello</div>";
    assert_eq!(
        parse_template(&format!("\u{feff}{source}")).unwrap(),
        parse_template(source).unwrap()
    );
}

#[test]
fn leading_bom_offsets_stay_in_original_source_coordinates() {
    for (filename, source) in [
        ("view.jsx", "<div>😀</bad>"),
        ("view.vue", "<template><div>😀</bad></template>"),
        ("view.svelte", "<div>😀</bad>"),
        ("view.astro", "<div>😀</bad>"),
        ("view.ng.html", "<div>😀</bad>"),
    ] {
        let path = Some(Path::new(filename));
        let plain = diagnose_crepus_source_with_path(source, path);
        let bom = diagnose_crepus_source_with_path(&format!("\u{feff}{source}"), path);
        assert_eq!(plain.len(), 1, "{filename}");
        assert_eq!(bom.len(), 1, "{filename}");
        assert_eq!(bom[0].message, plain[0].message, "{filename}");
        assert_eq!(bom[0].start_line, plain[0].start_line);
        assert_eq!(
            bom[0].start_character,
            plain[0].start_character + 1,
            "{filename}"
        );
        assert_eq!(
            bom[0].end_character,
            plain[0].end_character + 1,
            "{filename}"
        );
    }
}

#[test]
fn bom_normalization_preserves_interior_text_and_vue_validation() {
    assert!(parse_template("\u{feff}").unwrap().is_empty());
    let source = "<div>inside\u{feff}text</div>";
    assert_eq!(
        parse_template(&format!("\u{feff}{source}")).unwrap(),
        parse_template(source).unwrap()
    );
    let vue = "\u{feff}\u{feff}<template/>";
    assert!(parse_template_with_path(vue, Some(Path::new("view.vue"))).is_err());
}

#[test]
fn vue_template_errors_point_into_the_original_sfc_after_script_blocks() {
    let source =
        "<script>const icon = '😀';</script>\n<template>\n<div v-else>bad</div>\n</template>";
    for prefix in ["", "\u{feff}"] {
        let diagnostics = diagnose_crepus_source_with_path(
            &format!("{prefix}{source}"),
            Some(Path::new("view.vue")),
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            (diagnostics[0].start_line, diagnostics[0].start_character),
            (2, 0)
        );
        assert_eq!(
            (diagnostics[0].end_line, diagnostics[0].end_character),
            (2, 1)
        );
    }
}
