use std::path::Path;

use super::*;
use crate::ast::{Node, TextPart};
use crate::parser::parse_template_with_path;

fn parse(src: &str) -> Vec<Node> {
    parse_template_with_path(src, Some(Path::new("Comp.svelte"))).expect("svelte should parse")
}

fn parse_err(src: &str) -> String {
    parse_template_with_path(src, Some(Path::new("Comp.svelte")))
        .expect_err("expected a parse error")
        .to_string()
}

fn el(node: &Node) -> &crate::ast::Element {
    match node {
        Node::Element(e) => e,
        other => panic!("expected element, got {other:?}"),
    }
}

fn text_of(node: &Node) -> String {
    match node {
        Node::Text(parts) => parts
            .iter()
            .map(|p| match p {
                TextPart::Literal(s) => s.clone(),
                TextPart::Expr(e) => format!("{{{e}}}"),
            })
            .collect(),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn dispatch_selects_svelte_frontend_for_svelte_paths() {
    // `{#if}` is meaningless to the other frontends; parsing it proves dispatch.
    let nodes = parse("{#if ok}<p>yes</p>{/if}");
    assert!(matches!(nodes[0], Node::If(_)));
}

#[test]
fn elements_nest_and_self_close() {
    let nodes = parse("<div><span>hi</span><br /><img src=\"a.png\"></div>");
    let root = el(&nodes[0]);
    assert_eq!(root.tag, "div");
    assert_eq!(root.children.len(), 3);
    assert_eq!(el(&root.children[0]).tag, "span");
    assert_eq!(el(&root.children[1]).tag, "br");
    assert_eq!(el(&root.children[2]).tag, "img");
}

#[test]
fn html_comments_are_skipped() {
    let nodes = parse("<div><!-- a note --><span>hi</span></div>");
    let root = el(&nodes[0]);
    assert_eq!(root.children.len(), 1);
    assert_eq!(el(&root.children[0]).tag, "span");
}

#[test]
fn text_interpolation_produces_expr_parts() {
    let nodes = parse("<p>Hello {name}, you have {count} items</p>");
    let root = el(&nodes[0]);
    assert_eq!(
        text_of(&root.children[0]),
        "Hello {name}, you have {count} items"
    );
    let Node::Text(parts) = &root.children[0] else {
        panic!("expected text");
    };
    assert!(matches!(&parts[1], TextPart::Expr(e) if e == "name"));
}

#[test]
fn static_class_splits_into_tokens() {
    let nodes = parse(r#"<div class="flex gap-4 text-white"></div>"#);
    assert_eq!(
        el(&nodes[0]).classes,
        vec![
            "flex".to_string(),
            "gap-4".to_string(),
            "text-white".to_string()
        ]
    );
}

#[test]
fn dynamic_and_shorthand_attributes_become_bindings() {
    let nodes = parse(r#"<input placeholder="type" value={draft} {title} />"#);
    let e = el(&nodes[0]);
    let binding = |p: &str| {
        e.bindings
            .iter()
            .find(|b| b.prop == p)
            .map(|b| b.value.clone())
    };
    assert_eq!(binding("placeholder").as_deref(), Some("\"type\""));
    assert_eq!(binding("value").as_deref(), Some("draft"));
    assert_eq!(binding("title").as_deref(), Some("title"));
}

#[test]
fn on_directive_events_lower_to_event_handlers() {
    let nodes = parse("<button on:click={increment}>+</button>");
    let e = el(&nodes[0]);
    assert_eq!(e.event_handlers.len(), 1);
    assert_eq!(e.event_handlers[0].event, "click");
    assert_eq!(e.event_handlers[0].handler, "increment");
    assert!(e.event_handlers[0].modifiers.is_empty());
}

#[test]
fn on_directive_modifiers_are_preserved() {
    let nodes = parse("<form on:submit|preventDefault={save}></form>");
    let e = el(&nodes[0]);
    assert_eq!(e.event_handlers[0].event, "submit");
    assert_eq!(
        e.event_handlers[0].modifiers,
        vec!["preventDefault".to_string()]
    );
}

#[test]
fn svelte5_attribute_events_lower_to_event_handlers() {
    let nodes = parse("<button onclick={increment}>+</button>");
    let e = el(&nodes[0]);
    assert_eq!(e.event_handlers.len(), 1);
    assert_eq!(e.event_handlers[0].event, "click");
    assert_eq!(e.event_handlers[0].handler, "increment");
    assert!(e.bindings.is_empty());
}

#[test]
fn non_event_on_prefixed_attribute_stays_a_binding() {
    let nodes = parse(r#"<div once="yes"></div>"#);
    let e = el(&nodes[0]);
    assert!(e.event_handlers.is_empty());
    assert_eq!(e.bindings[0].prop, "once");
}

#[test]
fn bind_directive_lowers_to_bindings() {
    let nodes = parse("<input bind:value={draft} />");
    let e = el(&nodes[0]);
    assert_eq!(e.bindings.len(), 1);
    // Two-way form bindings map onto the shared `bind=` binding.
    assert_eq!(e.bindings[0].prop, "bind");
    assert_eq!(e.bindings[0].value, "draft");
}

#[test]
fn non_form_bind_directive_keeps_its_property_name() {
    let nodes = parse("<div bind:clientWidth={w}></div>");
    let e = el(&nodes[0]);
    assert_eq!(e.bindings[0].prop, "clientWidth");
    assert_eq!(e.bindings[0].value, "w");
}

#[test]
fn bind_shorthand_binds_the_same_name() {
    let nodes = parse("<input bind:value />");
    let e = el(&nodes[0]);
    assert_eq!(e.bindings[0].prop, "bind");
    assert_eq!(e.bindings[0].value, "value");
}

#[test]
fn class_directive_becomes_conditional_class() {
    let nodes = parse(r#"<div class="btn" class:active={selected} class:wide></div>"#);
    let e = el(&nodes[0]);
    assert_eq!(e.classes, vec!["btn".to_string()]);
    assert_eq!(e.conditional_classes.len(), 2);
    assert_eq!(e.conditional_classes[0].class, "active");
    assert_eq!(e.conditional_classes[0].condition, "selected");
    assert_eq!(e.conditional_classes[1].class, "wide");
    assert_eq!(e.conditional_classes[1].condition, "wide");
}

#[test]
fn if_block_lowers_to_if_node() {
    let nodes = parse("{#if ready}<p>go</p>{/if}");
    let Node::If(block) = &nodes[0] else {
        panic!("expected if");
    };
    assert_eq!(block.condition, "ready");
    assert_eq!(el(&block.then_children[0]).tag, "p");
    assert!(block.else_children.is_none());
}

#[test]
fn if_else_block_lowers_to_else_children() {
    let nodes = parse("{#if ready}<p>go</p>{:else}<p>wait</p>{/if}");
    let Node::If(block) = &nodes[0] else {
        panic!("expected if");
    };
    let els = block.else_children.as_ref().expect("else branch");
    assert_eq!(text_of(&el(&els[0]).children[0]), "wait");
}

#[test]
fn else_if_chain_nests_if_blocks() {
    let nodes = parse("{#if a}<p>a</p>{:else if b}<p>b</p>{:else}<p>c</p>{/if}");
    let Node::If(outer) = &nodes[0] else {
        panic!("expected if");
    };
    assert_eq!(outer.condition, "a");
    let outer_else = outer.else_children.as_ref().expect("else");
    let Node::If(inner) = &outer_else[0] else {
        panic!("expected nested if");
    };
    assert_eq!(inner.condition, "b");
    let inner_else = inner.else_children.as_ref().expect("inner else");
    assert_eq!(text_of(&el(&inner_else[0]).children[0]), "c");
}

#[test]
fn each_block_lowers_to_for_node() {
    let nodes = parse("{#each items as item}<li>{item.name}</li>{/each}");
    let Node::For(block) = &nodes[0] else {
        panic!("expected for");
    };
    assert_eq!(block.iterator, "items");
    assert_eq!(block.pattern, "item");
    assert_eq!(el(&block.body[0]).tag, "li");
}

#[test]
fn keyed_each_drops_the_key_expression() {
    let nodes = parse("{#each items as item (item.id)}<li>x</li>{/each}");
    let Node::For(block) = &nodes[0] else {
        panic!("expected for");
    };
    assert_eq!(block.pattern, "item");
    assert_eq!(block.iterator, "items");
}

#[test]
fn nested_blocks_inside_elements() {
    let nodes = parse("<ul>{#each rows as row}{#if row.on}<li>{row.t}</li>{/if}{/each}</ul>");
    let root = el(&nodes[0]);
    let Node::For(f) = &root.children[0] else {
        panic!("expected for");
    };
    assert!(matches!(f.body[0], Node::If(_)));
}

#[test]
fn at_html_becomes_raw_html() {
    let nodes = parse("<div>{@html body}</div>");
    let root = el(&nodes[0]);
    assert!(matches!(&root.children[0], Node::RawHtml(e) if e == "body"));
}

#[test]
fn script_and_style_are_extracted_and_not_rendered() {
    let src = "<script>\n  let count = $state(0);\n</script>\n\n<style>\n  p { color: red; }\n</style>\n\n<p>{count}</p>\n";
    let comp = parse_svelte_component(src).expect("component should parse");
    assert_eq!(comp.nodes.len(), 1);
    assert_eq!(el(&comp.nodes[0]).tag, "p");
    assert!(comp.script.as_deref().unwrap().contains("$state(0)"));
    assert!(comp.style.as_deref().unwrap().contains("color: red"));
}

#[test]
fn module_script_is_captured_separately() {
    let src = "<script context=\"module\">export const x = 1;</script><script>let y = 2;</script><p>hi</p>";
    let comp = parse_svelte_component(src).unwrap();
    assert!(comp
        .module_script
        .as_deref()
        .unwrap()
        .contains("export const x"));
    assert!(comp.script.as_deref().unwrap().contains("let y"));
}

#[test]
fn script_body_never_leaks_into_markup() {
    let src = "<script>if (a < b) { let s = \"<div>\"; }</script><p>ok</p>";
    let comp = parse_svelte_component(src).unwrap();
    assert_eq!(comp.nodes.len(), 1);
    assert_eq!(el(&comp.nodes[0]).tag, "p");
}

#[test]
fn parse_svelte_component_propagates_errors() {
    let result = parse_svelte_component("<div><span>hi</div>");
    assert!(result.unwrap_err().to_string().contains("</span>"));
}

// ── Explicitly unsupported constructs ────────────────────────────────────────

#[test]
fn each_index_binding_is_reported_unsupported() {
    let err = parse_err("{#each items as item, i}<li>{i}</li>{/each}");
    assert!(err.contains("index binding"), "got: {err}");
}

#[test]
fn each_else_is_reported_unsupported() {
    let err = parse_err("{#each items as item}<li>x</li>{:else}<p>none</p>{/each}");
    assert!(
        err.contains("{:else}") || err.contains("empty-list"),
        "got: {err}"
    );
}

#[test]
fn await_and_key_blocks_are_reported_unsupported() {
    assert!(parse_err("{#await p}<p>x</p>{/await}").contains("#await"));
    assert!(parse_err("{#key v}<p>x</p>{/key}").contains("#key"));
    assert!(parse_err("{#snippet row()}<p>x</p>{/snippet}").contains("#snippet"));
}

#[test]
fn render_const_and_debug_tags_are_reported_unsupported() {
    assert!(parse_err("{@render row()}").contains("@render"));
    assert!(parse_err("{@const x = 1}").contains("@const"));
}

#[test]
fn transitions_actions_and_style_directives_are_reported_unsupported() {
    for (src, needle) in [
        ("<div transition:fade></div>", "transition:"),
        ("<div in:fly={{y: 10}}></div>", "in:"),
        ("<div out:fade></div>", "out:"),
        ("<div use:tooltip></div>", "use:"),
        ("<div animate:flip></div>", "animate:"),
        ("<div style:color={c}></div>", "style:"),
        ("<div let:item></div>", "let:"),
    ] {
        let err = parse_err(src);
        assert!(err.contains(needle), "{src} → {err}");
    }
}

#[test]
fn spread_attributes_are_reported_unsupported() {
    assert!(parse_err("<div {...props}></div>").contains("spread"));
}

#[test]
fn slots_components_and_special_elements_are_reported_unsupported() {
    assert!(parse_err("<slot></slot>").contains("<slot>"));
    assert!(parse_err("<Card title=\"x\" />").contains("component tag"));
    assert!(parse_err("<svelte:window />").contains("svelte:window"));
}

#[test]
fn unclosed_element_is_an_error() {
    assert!(parse_err("<div><span>hi</div>").contains("</span>"));
}
