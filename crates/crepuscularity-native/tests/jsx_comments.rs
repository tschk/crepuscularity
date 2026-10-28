use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::{ir::LabelPart, render_template_to_ir_with_path, ViewNode};
use std::path::Path;

#[test]
fn jsx_comments_preserve_button_interpolation_and_text_bindings() {
    let mut ctx = TemplateContext::new();
    ctx.set("count", 7);
    for comment in [
        "",
        "{/* comment with } and <fake /> */}",
        "{ /* one */ /* two */ }",
    ] {
        let source = format!("<button>{comment}{{count}}</button><span>{comment}{{count}}</span><button>{comment}{{count}} suffix</button><button>{{count}}{{count}}</button>");
        let ir =
            render_template_to_ir_with_path(&source, &ctx, Some(Path::new("app.jsx"))).unwrap();
        assert!(
            matches!(&ir.root[0], ViewNode::Button { label, label_parts, .. } if label == "7" && label_parts == &vec![LabelPart::Expr("count".into())]),
            "{ir:?}"
        );
        assert!(
            matches!(&ir.root[1], ViewNode::Text { content, bind: Some(bind), .. } if content == "7" && bind == "count"),
            "{ir:?}"
        );
        assert!(
            matches!(&ir.root[2], ViewNode::Button { label, label_parts, .. } if label == "7 suffix" && label_parts == &vec![LabelPart::Expr("count".into()), LabelPart::Literal(" suffix".into())]),
            "{ir:?}"
        );
        assert!(
            matches!(&ir.root[3], ViewNode::Button { label, label_parts, .. } if label == "77" && label_parts == &vec![LabelPart::Expr("count".into()), LabelPart::Expr("count".into())]),
            "{ir:?}"
        );
    }
}

#[test]
fn jsx_inline_comments_preserve_adjacent_label_parts_and_literal_comment_syntax() {
    let mut ctx = TemplateContext::new();
    ctx.set("count", 7);
    let source = r#"<button>Before{/* hidden */}{count} after</button><button label="{/* literal */}" /><span>{"{/* literal */}"}</span>"#;
    let ir = render_template_to_ir_with_path(source, &ctx, Some(Path::new("app.tsx"))).unwrap();
    assert!(
        matches!(&ir.root[0], ViewNode::Button { label, label_parts, .. } if label == "Before7 after" && label_parts == &vec![LabelPart::Literal("Before".into()), LabelPart::Expr("count".into()), LabelPart::Literal(" after".into())]),
        "{ir:?}"
    );
    assert!(
        matches!(&ir.root[1], ViewNode::Button { label, .. } if label == "{/* literal */}"),
        "{ir:?}"
    );
    assert!(
        matches!(&ir.root[2], ViewNode::Text { content, .. } if content == "{/* literal */}"),
        "{ir:?}"
    );
}
