use std::sync::Arc;

use crepuscularity_core::ast::{Node, TextPart};
use crepuscularity_core::{CrepusError, TemplateContext};

fn variants(source: &str, ctx: &TemplateContext) -> Vec<Result<String, CrepusError>> {
    #[allow(unused_mut)]
    let mut out = vec![crepuscularity_web::render_template_to_html(source, ctx)];
    #[cfg(feature = "ssr")]
    for markers in [false, true] {
        out.push(crepuscularity_web::render_template_to_html_with_ssr(
            source, ctx, markers,
        ));
    }
    #[cfg(feature = "hydration")]
    out.push(crepuscularity_web::render_template_to_html_with_hydration(
        source, ctx,
    ));
    out
}

fn put(ctx: &mut TemplateContext, path: &str, source: &str) {
    Arc::make_mut(&mut ctx.virtual_files).insert(path.into(), source.into());
}

fn body(html: &str) -> &str {
    html.split("<script").next().unwrap()
}

fn text(html: &str) -> String {
    let mut tag = false;
    body(html)
        .chars()
        .filter(|ch| match ch {
            '<' => {
                tag = true;
                false
            }
            '>' => {
                tag = false;
                false
            }
            _ => !tag,
        })
        .collect()
}

fn manual_slot(ctx: &mut TemplateContext) {
    let mut caller = TemplateContext::new();
    caller.set("name", "public-caller");
    ctx.slot = Some((
        vec![Node::Text(vec![TextPart::Expr("name".into())])],
        Arc::new(caller),
    ));
}

#[test]
fn nested_slot_forwarding_reaches_public_slot_and_unsupplied_child_uses_fallback() {
    let mut ctx = TemplateContext::new();
    manual_slot(&mut ctx);
    put(&mut ctx, "outer.crepus", "include inner.crepus\n  slot");
    put(&mut ctx, "inner.crepus", "slot\ninclude fallback.crepus");
    put(&mut ctx, "fallback.crepus", "slot\n  span \"own-fallback\"");
    for html in variants("include outer.crepus\n  slot\nslot", &ctx) {
        let html = html.unwrap();
        assert_eq!(
            text(&html),
            "public-callerown-fallbackpublic-caller",
            "{html}"
        );
    }
}

#[test]
fn ordinary_and_named_slots_capture_declarations_and_shadowed_loop_callers() {
    let mut ctx = TemplateContext::new();
    let mut outer = TemplateContext::new();
    outer.set("name", "outer-row");
    let mut inner = TemplateContext::new();
    inner.set("name", "inner-row");
    outer.set("children", vec![inner]);
    ctx.set("rows", vec![outer]);
    ctx.set("title", "root-title");
    put(&mut ctx, "card.crepus", "slot");
    put(
        &mut ctx,
        "ui.crepus",
        "+++\n[Card.defaults]\ntitle = 'component-title'\n+++\n--- Card\nslot",
    );
    for target in ["card.crepus", "ui.crepus#Card"] {
        let source = format!("$: let title = \"lexical-title\"\nfor row in rows\n  include {target}\n    span \"{{title}}/{{row.name}};\"\n  for row in row.children\n    include {target}\n      span \"{{title}}/{{row.name}};\"\n  include {target}\n    span \"{{row.name}};\"");
        for html in variants(&source, &ctx) {
            let html = html.unwrap();
            assert_eq!(
                text(&html),
                "lexical-title/outer-row;lexical-title/inner-row;outer-row;",
                "{html}"
            );
        }
    }
}

#[test]
fn textarea_forwarded_slot_uses_caller_and_plain_rendering_with_leading_newline() {
    let mut ctx = TemplateContext::new();
    let mut caller = TemplateContext::new();
    caller.set("name", "caller&value");
    ctx.slot = Some((
        vec![Node::Text(vec![
            TextPart::Literal("\n".into()),
            TextPart::Expr("name".into()),
        ])],
        Arc::new(caller),
    ));
    put(&mut ctx, "area.crepus", "textarea\n  slot");
    put(&mut ctx, "areas.crepus", "--- Area\ntextarea\n  slot");
    for target in ["area.crepus", "areas.crepus#Area"] {
        for html in variants(&format!("include {target}\n  slot"), &ctx) {
            let html = html.unwrap();
            let content = body(&html)
                .split("<textarea")
                .nth(1)
                .unwrap()
                .split_once('>')
                .unwrap()
                .1
                .split("</textarea>")
                .next()
                .unwrap();
            assert_eq!(content, "\n\ncaller&amp;value", "{html}");
            assert!(!content.contains("data-crepus"), "{html}");
        }
    }
}

#[test]
fn slot_relative_includes_use_caller_base_directory_and_virtual_files() {
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some("app".into());
    put(&mut ctx, "app/components/card.crepus", "slot");
    put(&mut ctx, "app/sibling.crepus", "include leaf.crepus");
    put(&mut ctx, "app/leaf.crepus", "span \"caller-relative\"");
    put(
        &mut ctx,
        "app/components/sibling.crepus",
        "span \"wrong-component-relative\"",
    );
    for html in variants(
        "include components/card.crepus\n  include sibling.crepus",
        &ctx,
    ) {
        let html = html.unwrap();
        assert_eq!(text(&html), "caller-relative", "{html}");
    }
}

#[test]
fn slot_errors_do_not_poison_later_renders_or_sibling_slot_frames() {
    let mut ctx = TemplateContext::new();
    manual_slot(&mut ctx);
    put(&mut ctx, "card.crepus", "slot");
    for _ in 0..2 {
        for result in variants("include card.crepus\n  include missing.crepus", &ctx) {
            assert!(result.is_err());
        }
        let source =
            "include card.crepus\n  span \"first;\"\ninclude card.crepus\n  span \"second;\"\nslot";
        for html in variants(source, &ctx) {
            let html = html.unwrap();
            assert_eq!(text(&html), "first;second;public-caller", "{html}");
        }
    }
}
#[test]
fn forwarded_slot_survives_callee_condition_match_and_loop_scopes() {
    let mut ctx = TemplateContext::new();
    ctx.set("caller", "caller-value;");
    let mut row = TemplateContext::new();
    row.set("caller", "loop-value;");
    ctx.set("copies", vec![row.clone(), row]);
    put(&mut ctx, "controls.crepus", "if enabled\n  slot\nmatch status\n  \"chosen\" =>\n    slot\n  _ =>\n    span \"wrong-arm\"\nfor row in copies\n  slot");
    let source = "div\n  include controls.crepus enabled=true status=\"chosen\" copies=copies caller=\"callee-value;\"\n    span \"{caller}\"";
    let expected = "caller-value;".repeat(4);
    for html in variants(source, &ctx) {
        let html = html.unwrap();
        assert_eq!(text(&html), expected, "{html}");
    }
    #[cfg(feature = "ssr")]
    {
        let nodes = crepuscularity_core::parse_template(source).unwrap();
        let mut bind = serde_json::Map::new();
        let html = crepuscularity_web::render_nodes_ssr(
            &nodes,
            &ctx,
            &std::cell::Cell::new(0),
            &mut bind,
            true,
        )
        .unwrap();
        assert_eq!(text(&html), expected, "{html}");
        let ids = html
            .split("data-crepus-id=\"c")
            .skip(1)
            .map(|part| part.split('"').next().unwrap())
            .collect::<Vec<_>>();
        let unique = ids
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), unique.len(), "{html}");
        assert_eq!(unique.len(), bind.len(), "{html}");
        assert!(unique.iter().all(|id| bind.contains_key(*id)));
        assert_eq!(html.matches("data-crepus-root=\"1\"").count(), 1, "{html}");
    }
}
