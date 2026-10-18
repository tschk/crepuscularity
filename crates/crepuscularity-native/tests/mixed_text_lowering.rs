use crepuscularity_core::{context::value_to_str, eval::eval_expr, TemplateContext};
use crepuscularity_native::{ir::LabelPart, render_template_to_ir_with_path, ViewNode};
use std::path::Path;

#[test]
fn all_frontends_keep_mixed_text_live_and_button_parts_intact() {
    let mut context = TemplateContext::new();
    for (filename, source) in [
        ("app.crepus", "text \"Hello {count}\"\nbutton \"{count}\""),
        (
            "app.jsx",
            "<span>Hello {count}</span><button>{count}</button>",
        ),
        (
            "app.svelte",
            "<span>Hello {count}</span><button>{count}</button>",
        ),
        (
            "app.vue",
            "<template><span>Hello {{count}}</span><button>{{count}}</button></template>",
        ),
        (
            "app.astro",
            "<span>Hello {count}</span><button>{count}</button>",
        ),
        (
            "app.ng.html",
            "<span>Hello {{count}}</span><button>{{count}}</button>",
        ),
    ] {
        context.set("count", 7);
        let ir =
            render_template_to_ir_with_path(source, &context, Some(Path::new(filename))).unwrap();
        let ViewNode::Text {
            content,
            bind: Some(binding),
            ..
        } = &ir.root[0]
        else {
            panic!("{filename}: {ir:?}")
        };
        assert_eq!(content, "Hello 7", "{filename}");
        assert_eq!(
            value_to_str(&eval_expr(binding, &context).unwrap()),
            "Hello 7",
            "{filename}"
        );
        context.set("count", 9);
        assert_eq!(
            value_to_str(&eval_expr(binding, &context).unwrap()),
            "Hello 9",
            "{filename}"
        );
        assert!(
            matches!(&ir.root[1], ViewNode::Button { label_parts, .. } if label_parts == &vec![LabelPart::Expr("count".into())]),
            "{filename}: {ir:?}"
        );
    }
}
