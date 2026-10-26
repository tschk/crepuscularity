use serde::{Deserialize, Serialize};

use crepuscularity_core::ast::{Element, MatchArm, Node, TextPart};
use crepuscularity_core::context::TemplateContext;
use crepuscularity_core::parse_template_with_path;

use crate::ir::ViewIr;
use crate::mutations::{diff_ir, IrMutation};
use crate::render::render_template_to_ir_with_path;

/// Envelope suitable for transport (SSE, WebSocket, IPC).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HotReloadEnvelope {
    pub sequence: u64,
    pub message: HotReloadMessage,
}

/// Structured hot-reload messages for native shells and tooling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HotReloadMessage {
    Noop,
    Patch {
        mutations: Vec<IrMutation>,
    },
    FullReload {
        ir: ViewIr,
        reason: String,
    },
    Error {
        message: String,
    },
    /// Sent once when a WebSocket client connects (`aurorality dev`).
    DevHello {
        session_id: String,
        watch_dir: String,
        #[serde(default)]
        swiftgen_view: Option<String>,
        #[serde(default)]
        swiftgen_out: Option<String>,
        #[serde(default)]
        swiftgen_view_name: Option<String>,
        #[serde(default)]
        swiftgen_context_type: Option<String>,
        /// When false, `plan_hot_reload` is skipped and only swiftgen events fire.
        #[serde(default = "default_ir_enabled")]
        ir_enabled: bool,
    },
    /// Result of running `swiftgen` after a `.crepus` save (hybrid hot reload).
    SwiftgenStatus {
        ok: bool,
        #[serde(default)]
        errors: Vec<String>,
        view_name: String,
        output_path: String,
        ts_ms: u64,
    },
}

fn default_ir_enabled() -> bool {
    true
}

/// Decide between patch vs full reload using a conservative AST compatibility gate.
///
/// Behavior:
/// - If only compatible template literals/style-level output changed, emits [`HotReloadMessage::Patch`].
/// - If control-flow or other semantic shape changed, emits [`HotReloadMessage::FullReload`].
/// - If parsing the new template fails, emits [`HotReloadMessage::Error`].
pub fn plan_hot_reload(
    old_template: &str,
    new_template: &str,
    ctx: &TemplateContext,
) -> HotReloadMessage {
    plan_hot_reload_with_path(old_template, new_template, ctx, None)
}

/// Plan a reload using the selected file's parser frontend.
pub fn plan_hot_reload_with_path(
    old_template: &str,
    new_template: &str,
    ctx: &TemplateContext,
    path: Option<&std::path::Path>,
) -> HotReloadMessage {
    let old_nodes = match parse_template_with_path(old_template, path) {
        Ok(nodes) => nodes,
        Err(_) => {
            return match render_template_to_ir_with_path(new_template, ctx, path) {
                Ok(ir) => HotReloadMessage::FullReload {
                    ir,
                    reason: "previous template parse failed".to_string(),
                },
                Err(e) => HotReloadMessage::Error {
                    message: e.to_string(),
                },
            };
        }
    };

    let new_nodes = match parse_template_with_path(new_template, path) {
        Ok(nodes) => nodes,
        Err(e) => {
            return HotReloadMessage::Error {
                message: e.to_string(),
            }
        }
    };

    let new_ir = match render_template_to_ir_with_path(new_template, ctx, path) {
        Ok(ir) => ir,
        Err(e) => {
            return HotReloadMessage::Error {
                message: e.to_string(),
            }
        }
    };

    if !ast_shape_compatible(&old_nodes, &new_nodes) {
        return HotReloadMessage::FullReload {
            ir: new_ir,
            reason: "template semantics changed".to_string(),
        };
    }

    let old_ir = match render_template_to_ir_with_path(old_template, ctx, path) {
        Ok(ir) => ir,
        Err(_) => {
            return HotReloadMessage::FullReload {
                ir: new_ir,
                reason: "previous template render failed".to_string(),
            };
        }
    };

    let mutations = diff_ir(&old_ir, &new_ir);
    if mutations.is_empty() {
        HotReloadMessage::Noop
    } else {
        HotReloadMessage::Patch { mutations }
    }
}

/// Conservative shape-compatibility check:
/// textual/style updates are allowed; control-flow or expression shape changes force full reload.
pub fn ast_shape_compatible(old: &[Node], new: &[Node]) -> bool {
    old.len() == new.len() && old.iter().zip(new).all(|(a, b)| node_compatible(a, b))
}

fn node_compatible(old: &Node, new: &Node) -> bool {
    match (old, new) {
        (Node::Element(a), Node::Element(b)) => element_compatible(a, b),
        (Node::Text(a), Node::Text(b)) => text_parts_compatible(a, b),
        (Node::If(a), Node::If(b)) => {
            a.condition == b.condition
                && ast_shape_compatible(&a.then_children, &b.then_children)
                && match (&a.else_children, &b.else_children) {
                    (None, None) => true,
                    (Some(x), Some(y)) => ast_shape_compatible(x, y),
                    _ => false,
                }
        }
        (Node::For(a), Node::For(b)) => {
            a.pattern == b.pattern
                && a.iterator == b.iterator
                && ast_shape_compatible(&a.body, &b.body)
        }
        (Node::Match(a), Node::Match(b)) => {
            a.expr == b.expr
                && a.arms.len() == b.arms.len()
                && a.arms
                    .iter()
                    .zip(&b.arms)
                    .all(|(x, y)| arm_compatible(x, y))
        }
        (Node::LetDecl(a), Node::LetDecl(b)) => {
            a.name == b.name && a.expr == b.expr && a.is_default == b.is_default
        }
        (Node::Include(a), Node::Include(b)) => {
            a.path == b.path
                && a.props.len() == b.props.len()
                && a.props
                    .iter()
                    .zip(&b.props)
                    .all(|((ak, av), (bk, bv))| ak == bk && av == bv)
                && ast_shape_compatible(&a.slot, &b.slot)
        }
        (Node::Embed(a), Node::Embed(b)) => {
            a.src == b.src && a.adapter == b.adapter && a.props == b.props
        }
        (Node::RawText(a), Node::RawText(b)) => a == b,
        (Node::RawHtml(a), Node::RawHtml(b)) => a == b,
        _ => false,
    }
}

fn arm_compatible(old: &MatchArm, new: &MatchArm) -> bool {
    old.pattern == new.pattern && ast_shape_compatible(&old.body, &new.body)
}

fn text_parts_compatible(old: &[TextPart], new: &[TextPart]) -> bool {
    old.len() == new.len()
        && old.iter().zip(new).all(|(a, b)| match (a, b) {
            (TextPart::Literal(_), TextPart::Literal(_)) => true,
            (TextPart::Expr(x), TextPart::Expr(y)) => x == y,
            _ => false,
        })
}

fn element_compatible(old: &Element, new: &Element) -> bool {
    if old.tag != new.tag
        || old.id != new.id
        || old.event_handlers.len() != new.event_handlers.len()
        || old.bindings.len() != new.bindings.len()
        || old.animations.len() != new.animations.len()
        || old.conditional_classes.len() != new.conditional_classes.len()
    {
        return false;
    }

    if old
        .event_handlers
        .iter()
        .zip(&new.event_handlers)
        .any(|(a, b)| {
            a.event != b.event
                || a.modifiers != b.modifiers
                || a.handler != b.handler
                || a.model_before_listener != b.model_before_listener
        })
    {
        return false;
    }

    if old
        .bindings
        .iter()
        .zip(&new.bindings)
        .any(|(a, b)| a.prop != b.prop || a.value != b.value)
    {
        return false;
    }

    if old.animations.iter().zip(&new.animations).any(|(a, b)| {
        a.property != b.property
            || a.duration_expr != b.duration_expr
            || a.easing != b.easing
            || a.repeat != b.repeat
    }) {
        return false;
    }

    if old
        .conditional_classes
        .iter()
        .zip(&new.conditional_classes)
        .any(|(a, b)| a.condition != b.condition)
    {
        return false;
    }

    ast_shape_compatible(&old.children, &new.children)
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn svelte_reload_uses_the_selected_frontend() {
        let path = Some(std::path::Path::new("View.svelte"));
        let mut context = TemplateContext::new();
        context.set("show", true);
        let old = "{#if show}<p>Before</p>{/if}";
        let new = "{#if show}<p>After</p>{/if}";
        let HotReloadMessage::Patch { mutations } =
            plan_hot_reload_with_path(old, new, &context, path)
        else {
            panic!("literal change should produce a patch")
        };
        let mut applied = render_template_to_ir_with_path(old, &context, path).unwrap();
        crate::apply_mutations(&mut applied, &mutations).unwrap();
        assert_eq!(
            applied,
            render_template_to_ir_with_path(new, &context, path).unwrap()
        );
        assert!(matches!(
            plan_hot_reload_with_path(old, "{#if show}", &context, path),
            HotReloadMessage::Error { .. }
        ));
    }
}

#[cfg(test)]
mod model_order_tests {
    use super::*;
    #[test]
    fn changing_native_model_listener_order_requires_replacement() {
        let old = crepuscularity_core::parser::parse_vue_native_file(
            r#"<template><Child v-model="value" @update:modelValue="observe"/></template>"#,
        )
        .unwrap();
        let new = crepuscularity_core::parser::parse_vue_native_file(
            r#"<template><Child @update:modelValue="observe" v-model="value"/></template>"#,
        )
        .unwrap();
        assert!(!ast_shape_compatible(&old, &new));
    }
}

#[cfg(test)]
mod ast_shape_compatible_tests {
    use super::*;
    use crepuscularity_core::ast::{
        Element, EmbedNode, ForBlock, IfBlock, IncludeNode, LetDecl, MatchArm, MatchBlock, TextPart,
    };

    fn empty_element(tag: &str) -> Element {
        Element {
            tag: tag.to_string(),
            id: None,
            classes: vec![],
            conditional_classes: vec![],
            event_handlers: vec![],
            bindings: vec![],
            animations: vec![],
            children: vec![],
        }
    }

    #[test]
    fn test_different_lengths() {
        let old = vec![Node::RawText("a".to_string())];
        let new = vec![
            Node::RawText("a".to_string()),
            Node::RawText("b".to_string()),
        ];
        assert!(!ast_shape_compatible(&old, &new));
    }

    #[test]
    fn test_elements() {
        let old = vec![Node::Element(empty_element("div"))];
        let new = vec![Node::Element(empty_element("div"))];
        assert!(ast_shape_compatible(&old, &new));

        let new_diff = vec![Node::Element(empty_element("span"))];
        assert!(!ast_shape_compatible(&old, &new_diff));
    }

    #[test]
    fn test_text_parts() {
        let old = vec![Node::Text(vec![TextPart::Literal("hello".to_string())])];
        let new = vec![Node::Text(vec![TextPart::Literal("world".to_string())])];
        assert!(ast_shape_compatible(&old, &new));

        let old_expr = vec![Node::Text(vec![TextPart::Expr("a".to_string())])];
        let new_expr = vec![Node::Text(vec![TextPart::Expr("b".to_string())])];
        assert!(!ast_shape_compatible(&old_expr, &new_expr));
    }

    #[test]
    fn test_if_block() {
        let old = vec![Node::If(IfBlock {
            condition: "a".to_string(),
            then_children: vec![],
            else_children: None,
        })];
        let new = vec![Node::If(IfBlock {
            condition: "a".to_string(),
            then_children: vec![],
            else_children: None,
        })];
        assert!(ast_shape_compatible(&old, &new));

        let new_diff_cond = vec![Node::If(IfBlock {
            condition: "b".to_string(),
            then_children: vec![],
            else_children: None,
        })];
        assert!(!ast_shape_compatible(&old, &new_diff_cond));

        let new_diff_else = vec![Node::If(IfBlock {
            condition: "a".to_string(),
            then_children: vec![],
            else_children: Some(vec![]),
        })];
        assert!(!ast_shape_compatible(&old, &new_diff_else));
    }

    #[test]
    fn test_for_block() {
        let old = vec![Node::For(ForBlock {
            pattern: "item".to_string(),
            iterator: "items".to_string(),
            body: vec![],
        })];
        let new = vec![Node::For(ForBlock {
            pattern: "item".to_string(),
            iterator: "items".to_string(),
            body: vec![],
        })];
        assert!(ast_shape_compatible(&old, &new));

        let new_diff = vec![Node::For(ForBlock {
            pattern: "x".to_string(),
            iterator: "items".to_string(),
            body: vec![],
        })];
        assert!(!ast_shape_compatible(&old, &new_diff));
    }

    #[test]
    fn test_match_block() {
        let old = vec![Node::Match(MatchBlock {
            expr: "val".to_string(),
            arms: vec![MatchArm {
                pattern: "1".to_string(),
                body: vec![],
            }],
        })];
        let new = vec![Node::Match(MatchBlock {
            expr: "val".to_string(),
            arms: vec![MatchArm {
                pattern: "1".to_string(),
                body: vec![],
            }],
        })];
        assert!(ast_shape_compatible(&old, &new));

        let new_diff_expr = vec![Node::Match(MatchBlock {
            expr: "other".to_string(),
            arms: vec![MatchArm {
                pattern: "1".to_string(),
                body: vec![],
            }],
        })];
        assert!(!ast_shape_compatible(&old, &new_diff_expr));

        let new_diff_arm = vec![Node::Match(MatchBlock {
            expr: "val".to_string(),
            arms: vec![MatchArm {
                pattern: "2".to_string(),
                body: vec![],
            }],
        })];
        assert!(!ast_shape_compatible(&old, &new_diff_arm));
    }

    #[test]
    fn test_let_decl() {
        let old = vec![Node::LetDecl(LetDecl {
            name: "x".to_string(),
            expr: "1".to_string(),
            is_default: false,
        })];
        let new = vec![Node::LetDecl(LetDecl {
            name: "x".to_string(),
            expr: "1".to_string(),
            is_default: false,
        })];
        assert!(ast_shape_compatible(&old, &new));

        let new_diff = vec![Node::LetDecl(LetDecl {
            name: "y".to_string(),
            expr: "1".to_string(),
            is_default: false,
        })];
        assert!(!ast_shape_compatible(&old, &new_diff));
    }

    #[test]
    fn test_include() {
        let old = vec![Node::Include(IncludeNode {
            path: "a.crepus".to_string(),
            props: vec![("p".to_string(), "1".to_string())],
            slot: vec![],
        })];
        let new = vec![Node::Include(IncludeNode {
            path: "a.crepus".to_string(),
            props: vec![("p".to_string(), "1".to_string())],
            slot: vec![],
        })];
        assert!(ast_shape_compatible(&old, &new));

        let new_diff = vec![Node::Include(IncludeNode {
            path: "b.crepus".to_string(),
            props: vec![("p".to_string(), "1".to_string())],
            slot: vec![],
        })];
        assert!(!ast_shape_compatible(&old, &new_diff));
    }

    #[test]
    fn test_embed() {
        let old = vec![Node::Embed(EmbedNode {
            src: "src".to_string(),
            adapter: Some("adapter".to_string()),
            props: vec![],
        })];
        let new = vec![Node::Embed(EmbedNode {
            src: "src".to_string(),
            adapter: Some("adapter".to_string()),
            props: vec![],
        })];
        assert!(ast_shape_compatible(&old, &new));

        let new_diff = vec![Node::Embed(EmbedNode {
            src: "other".to_string(),
            adapter: Some("adapter".to_string()),
            props: vec![],
        })];
        assert!(!ast_shape_compatible(&old, &new_diff));
    }

    #[test]
    fn test_raw() {
        let old = vec![Node::RawText("t".to_string())];
        let new = vec![Node::RawText("t".to_string())];
        assert!(ast_shape_compatible(&old, &new));

        let new_diff = vec![Node::RawText("x".to_string())];
        assert!(!ast_shape_compatible(&old, &new_diff));

        let old_html = vec![Node::RawHtml("h".to_string())];
        let new_html = vec![Node::RawHtml("h".to_string())];
        assert!(ast_shape_compatible(&old_html, &new_html));

        let new_diff_html = vec![Node::RawHtml("x".to_string())];
        assert!(!ast_shape_compatible(&old_html, &new_diff_html));
    }
}
