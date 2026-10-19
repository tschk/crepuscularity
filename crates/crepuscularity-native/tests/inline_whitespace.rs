use crepuscularity_core::TemplateContext;
use crepuscularity_native::{render_template_to_ir_with_path, ViewNode};
use std::path::Path;

fn text(nodes: &[ViewNode]) -> String {
    nodes
        .iter()
        .map(|node| match node {
            ViewNode::Text { content, .. } => content.clone(),
            ViewNode::Stack { children, .. } | ViewNode::Scroll { children, .. } => text(children),
            _ => panic!("unexpected {node:?}"),
        })
        .collect()
}

#[test]
fn markup_frontends_keep_inline_literal_and_expression_boundaries() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "Ada");
    for (filename, source) in [
        (
            "page.jsx",
            "  <p>Hello  <strong>{name}</strong> \t<em>world</em>!</p>  ",
        ),
        (
            "page.svelte",
            "  <p>Hello  <strong>{name}</strong> \t<em>world</em>!</p>  ",
        ),
        (
            "page.vue",
            "<template>  <p>Hello  <strong>{{name}}</strong> \t<em>world</em>!</p>  </template>",
        ),
        (
            "page.astro",
            "  <p>Hello  <strong>{name}</strong> \t<em>world</em>!</p>  ",
        ),
        (
            "page.ng.html",
            "  <p>Hello  <strong>{{name}}</strong> \t<em>world</em>!</p>  ",
        ),
    ] {
        let ir = render_template_to_ir_with_path(source, &ctx, Some(Path::new(filename))).unwrap();
        assert_eq!(ir.root.len(), 1, "{filename}: {ir:?}");
        assert_eq!(text(&ir.root), "Hello  Ada \tworld!", "{filename}: {ir:?}");
    }
}

#[test]
fn formatting_indentation_does_not_become_extra_layout_nodes() {
    for (filename, source) in [
        ("page.jsx", "\n<div>\n  <p>\n    Hello <strong>world</strong>!\n  </p>\n</div>\n"),
        ("page.svelte", "\n<div>\n  <p>\n    Hello <strong>world</strong>!\n  </p>\n</div>\n"),
        ("page.vue", "<template>\n<div>\n  <p>\n    Hello <strong>world</strong>!\n  </p>\n</div>\n</template>"),
        ("page.astro", "\n<div>\n  <p>\n    Hello <strong>world</strong>!\n  </p>\n</div>\n"),
        ("page.ng", "\n<div>\n  <p>\n    Hello <strong>world</strong>!\n  </p>\n</div>\n"),
    ] {
        let ir = render_template_to_ir_with_path(source, &Default::default(), Some(Path::new(filename))).unwrap();
        let ViewNode::Stack { children, .. } = &ir.root[0] else {panic!("{ir:?}")};
        assert_eq!(ir.root.len(), 1, "{filename}");
        assert_eq!(children.len(), 1, "{filename}: {ir:?}");
        assert_eq!(text(&ir.root), "Hello world!", "{filename}: {ir:?}");
    }
}

#[test]
fn explicit_space_only_elements_and_markup_comment_boundaries_are_kept() {
    for (filename, source) in [
        (
            "page.jsx",
            "<p> </p><p>Hello {/* comment */} <strong>world</strong></p>",
        ),
        (
            "page.svelte",
            "<p> </p><p>Hello <!-- comment --> <strong>world</strong></p>",
        ),
        (
            "page.vue",
            "<template><p> </p><p>Hello <!-- comment --> <strong>world</strong></p></template>",
        ),
        (
            "page.astro",
            "<p> </p><p>Hello <!-- comment --> <strong>world</strong></p>",
        ),
        (
            "page.ng",
            "<p> </p><p>Hello <!-- comment --> <strong>world</strong></p>",
        ),
    ] {
        let ir =
            render_template_to_ir_with_path(source, &Default::default(), Some(Path::new(filename)))
                .unwrap();
        assert_eq!(text(&ir.root), " Hello  world", "{filename}: {ir:?}");
    }
}
