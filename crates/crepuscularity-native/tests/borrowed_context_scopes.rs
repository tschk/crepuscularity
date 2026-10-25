use std::collections::HashMap;

use crepuscularity_core::{context::value_to_str, eval::eval_expr, TemplateContext};
use crepuscularity_native::{render_from_files, render_template_to_ir, LabelPart, ViewNode};

fn text(node: &ViewNode) -> (&str, Option<&str>) {
    match node {
        ViewNode::Text { content, bind, .. } => (content, bind.as_deref()),
        _ => panic!("expected text, got {node:?}"),
    }
}

#[test]
fn skipped_default_preserves_live_text_and_label_provenance() {
    let mut ctx = TemplateContext::new();
    ctx.set("count", 1);
    let ir = render_template_to_ir(
        r#"<let-default name="count" value={2} /><span>Count {count}</span><button>Choose {count}</button>"#,
        &ctx,
    )
    .unwrap();
    let (content, binding) = text(&ir.root[0]);
    assert_eq!(content, "Count 1");
    ctx.set("count", 3);
    assert_eq!(
        value_to_str(&eval_expr(binding.unwrap(), &ctx).unwrap()),
        "Count 3"
    );
    assert!(
        matches!(&ir.root[1], ViewNode::Button { label, label_parts, .. }
        if label == "Choose 1" && label_parts == &vec![LabelPart::Literal("Choose ".into()), LabelPart::Expr("count".into())])
    );
}

#[test]
fn nested_declarations_keep_ordered_snapshots_and_do_not_leak() {
    let mut ctx = TemplateContext::new();
    ctx.set("title", "Root");
    let source = "div\n  text \"Before {title}\"\n  $: let title = \"Local\"\n  text \"After {title}\"\n  div\n    $: let title = \"Nested\"\n    text \"Inner {title}\"\n  text \"Restored {title}\"\ntext \"Outside {title}\"";
    let ir = render_template_to_ir(source, &ctx).unwrap();
    let ViewNode::Stack { children, .. } = &ir.root[0] else {
        panic!("expected stack")
    };
    assert_eq!(text(&children[0]).0, "Before Root");
    assert!(text(&children[0]).1.is_some());
    assert_eq!(text(&children[1]), ("After Local", None));
    let ViewNode::Stack {
        children: nested, ..
    } = &children[2]
    else {
        panic!("expected nested stack")
    };
    assert_eq!(text(&nested[0]), ("Inner Nested", None));
    assert_eq!(text(&children[3]), ("Restored Local", None));
    assert_eq!(text(&ir.root[1]).0, "Outside Root");
    assert!(text(&ir.root[1]).1.is_some());
    assert_eq!(ctx.get_str("title"), "Root");
}

#[test]
fn include_and_slot_snapshots_keep_distinct_scopes_and_restore_live_siblings() {
    let mut ctx = TemplateContext::new();
    ctx.set("title", "Root");
    ctx.set("private", "Secret");
    let files = HashMap::from([
        (
            "main.crepus".into(),
            "include card.crepus title=\"Prop\"\n  text \"Slot {title}\"\ntext \"Outside {title}\""
                .into(),
        ),
        (
            "card.crepus".into(),
            "text \"Body {title}/{private}\"\nslot".into(),
        ),
    ]);
    let ir = render_from_files(&files, "main.crepus", &ctx).unwrap();
    assert_eq!(text(&ir.root[0]), ("Body Prop/", None));
    let ViewNode::Stack { children, .. } = &ir.root[1] else {
        panic!("expected slot stack")
    };
    assert_eq!(text(&children[0]), ("Slot Root", None));
    assert_eq!(text(&ir.root[2]).0, "Outside Root");
    assert!(text(&ir.root[2]).1.is_some());
    assert_eq!(ctx.get_str("title"), "Root");
    assert_eq!(ctx.get_str("private"), "Secret");
}
