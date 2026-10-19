use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

fn indent(source: &str) -> String {
    source
        .lines()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn flattened_labels_and_options_reject_unrepresentable_control_flow() {
    for flow in [
        "if {true}\n  \"Visible\"",
        "for item in items\n  \"Row\"",
        "match {status}\n  \"one\" =>\n    \"Matched\"\n  _ =>\n    \"Other\"",
    ] {
        for owner in ["button", "picker\n  option value=one"] {
            let source = if owner == "button" {
                format!("button\n{}", indent(flow))
            } else {
                format!("{owner}\n{}", indent(&indent(flow)))
            };
            let nested = if owner == "button" {
                format!("button\n  span\n{}", indent(&indent(flow)))
            } else {
                format!("{owner}\n    span\n{}", indent(&indent(&indent(flow))))
            };
            for source in [source, nested] {
                let error = render_template_to_ir_with_path(&source, &TemplateContext::new(), None)
                    .expect_err(&format!("metadata control flow was dropped: {source}"));
                assert!(
                    error.to_string().contains(
                        "View IR does not support control flow in metadata-only children"
                    ),
                    "{source}: {error}"
                );
            }
        }
    }
}

#[test]
fn ordinary_layout_control_flow_and_real_tab_panel_children_remain_supported() {
    let ir = render_template_to_ir_with_path(
        "div\n  if {true}\n    span \"Visible\"\n  for item in items\n    span \"Row\"",
        &TemplateContext::new(),
        None,
    )
    .unwrap();
    let value = serde_json::to_value(ir).unwrap();
    assert_eq!(value["root"][0]["children"][0]["kind"], "if");
    assert_eq!(value["root"][0]["children"][1]["kind"], "forEach");

    for tag in ["tab", "page"] {
        let source = format!("<tabs><{tag} value='one'><if condition={{true}}><span>Visible</span></if></{tag}></tabs>");
        let ir = render_template_to_ir_with_path(
            &source,
            &TemplateContext::new(),
            Some(Path::new("Panels.jsx")),
        )
        .unwrap();
        let value = serde_json::to_value(ir).unwrap();
        assert_eq!(
            value["root"][0]["tabs"][0]["children"][0]["kind"], "if",
            "{source}: {value}"
        );
    }
}
