use crepuscularity_core::parse_template_with_path;
use std::path::Path;

#[test]
fn malformed_jsx_declarations_are_errors() {
    for declaration in [
        "$: let",
        "$: default",
        "$: let title",
        "$: let = 1",
        "$: default title =",
        "$: effect run()",
        "$: let title = {}",
        "$: let title = {1",
        "$: let title = {1} ignored",
    ] {
        for source in [
            format!("{declaration}\n<div/>"),
            format!("<div>\n{declaration}\n</div>"),
        ] {
            assert!(
                parse_template_with_path(&source, Some(Path::new("view.jsx"))).is_err(),
                "accepted {source:?}"
            );
        }
    }
}

#[test]
fn valid_declarations_keep_the_same_ast_across_frontends() {
    for declaration in [
        "$: let title = {'OK'}",
        "$: default title = 'OK'",
        "$: let count = 0",
    ] {
        let jsx = parse_template_with_path(
            &format!("{declaration}\n<div/>"),
            Some(Path::new("view.tsx")),
        )
        .unwrap();
        let indent = parse_template_with_path(
            &format!("{declaration}\ndiv"),
            Some(Path::new("view.crepus")),
        )
        .unwrap();
        assert_eq!(jsx, indent);
    }
    assert!(
        parse_template_with_path("$: let title = {}\ndiv", Some(Path::new("view.crepus"))).is_err()
    );
}
