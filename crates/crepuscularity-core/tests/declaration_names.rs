use crepuscularity_core::{ast::Node, parse_template_with_path};
use std::path::Path;

#[test]
fn declarations_reject_names_the_evaluator_cannot_reference() {
    for name in [
        "a+b",
        "user.name",
        "123",
        "true",
        "null",
        "a b",
        "a-b",
        "$value",
        "界",
    ] {
        for keyword in ["let", "default"] {
            for filename in ["view.crepus", "view.jsx"] {
                let source = format!("$: {keyword} {name} = 42");
                assert!(
                    parse_template_with_path(&source, Some(Path::new(filename))).is_err(),
                    "accepted {filename}: {source}"
                );
            }
        }
        for tag in ["let", "let-default"] {
            let source = format!("<{tag} name='{name}' value={{42}}/>");
            assert!(
                parse_template_with_path(&source, Some(Path::new("view.jsx"))).is_err(),
                "accepted {source}"
            );
        }
    }
    assert!(parse_template_with_path(
        "<let name=' title ' value={42}/>",
        Some(Path::new("view.jsx"))
    )
    .is_err());
}

#[test]
fn valid_names_keep_the_same_declaration_ast() {
    for name in ["title", "_value", "value2"] {
        for (keyword, tag, is_default) in [("let", "let", false), ("default", "let-default", true)]
        {
            let source = format!("$: {keyword} {name} = 42");
            let indent = parse_template_with_path(&source, Some(Path::new("view.crepus"))).unwrap();
            let jsx = parse_template_with_path(&source, Some(Path::new("view.jsx"))).unwrap();
            let tagged = parse_template_with_path(
                &format!("<{tag} name='{name}' value={{42}}/>"),
                Some(Path::new("view.jsx")),
            )
            .unwrap();
            assert_eq!(indent, jsx);
            assert_eq!(jsx, tagged);
            let Node::LetDecl(declaration) = &indent[0] else {
                panic!("expected declaration")
            };
            assert_eq!(declaration.name, name);
            assert_eq!(declaration.is_default, is_default);
        }
    }
}
