use crepuscularity_core::parse_template;

#[test]
fn frontend_depth_errors_survive_default_test_threads() {
    let markup = format!("{}leaf{}", "<div>".repeat(300), "</div>".repeat(300));
    for filename in [
        "Deep.svelte",
        "Deep.vue",
        "Deep.astro",
        "Deep.ng.html",
        "Deep.jsx",
    ] {
        let source = if filename.ends_with(".vue") {
            format!("<template>{markup}</template>")
        } else {
            markup.clone()
        };
        let error = crepuscularity_core::parse_template_with_path(
            &source,
            Some(std::path::Path::new(filename)),
        )
        .unwrap_err();
        assert!(error.to_string().contains("depth"), "{filename}: {error}");
        let valid = format!("{}leaf{}", "<div>".repeat(250), "</div>".repeat(250));
        let valid = if filename.ends_with(".vue") {
            format!("<template>{valid}</template>")
        } else {
            valid
        };
        let nodes = crepuscularity_core::parse_template_with_path(
            &valid,
            Some(std::path::Path::new(filename)),
        )
        .unwrap();
        assert!(!nodes.is_empty(), "{filename}");
    }
}

#[test]
fn excessive_indentation_nesting_returns_an_error_instead_of_empty_nodes() {
    let mut source = String::new();
    for depth in 0..260 {
        source.push_str(&"  ".repeat(depth));
        source.push_str("div\n");
    }
    let error = parse_template(&source).unwrap_err();
    assert!(
        error.to_string().contains("maximum nesting depth"),
        "{error}"
    );
    let boundary = source.lines().take(256).collect::<Vec<_>>().join("\n");
    assert_eq!(parse_template(&boundary).unwrap().len(), 1);
    assert!(!parse_template("div\n  span\n    \"Recovered\"")
        .unwrap()
        .is_empty());
}
