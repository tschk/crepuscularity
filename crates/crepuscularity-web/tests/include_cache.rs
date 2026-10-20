use std::sync::Arc;

use crepuscularity_core::{CrepusError, TemplateContext};
use crepuscularity_web::render_template_to_html;

fn variants(template: &str, ctx: &TemplateContext) -> Vec<Result<String, CrepusError>> {
    #[allow(unused_mut)]
    let mut results = vec![render_template_to_html(template, ctx)];
    #[cfg(feature = "ssr")]
    for markers in [false, true] {
        results.push(crepuscularity_web::render_template_to_html_with_ssr(
            template, ctx, markers,
        ));
    }
    results
}

fn put(ctx: &mut TemplateContext, path: &str, source: &str) {
    Arc::make_mut(&mut ctx.virtual_files).insert(path.to_string(), source.to_string());
}

#[test]
fn repeated_includes_keep_props_and_caller_slots_independent() {
    let mut ctx = TemplateContext::new();
    let rows = [
        ("first-title", "first-slot"),
        ("second-title", "second-slot"),
    ]
    .into_iter()
    .map(|(title, slot)| {
        let mut row = TemplateContext::new();
        row.set("title", title);
        row.set("slot_text", slot);
        row
    })
    .collect::<Vec<_>>();
    ctx.set("rows", rows);
    put(&mut ctx, "card.crepus", "article\n  h2 \"{title}\"\n  slot");
    let template =
        "for row in rows\n  include card.crepus title=row.title\n    span \"{row.slot_text}\"";
    // Repeat complete renders as well as includes within each render.
    for _ in 0..2 {
        for html in variants(template, &ctx) {
            let html = html.unwrap();
            for text in ["first-title", "first-slot", "second-title", "second-slot"] {
                assert_eq!(html.matches(text).count(), 1, "{html}");
            }
            assert!(
                html.find("first-title") < html.find("second-title"),
                "{html}"
            );
        }
    }
}

#[test]
fn changed_source_at_same_virtual_path_refreshes_cached_include() {
    let mut ctx = TemplateContext::new();
    for text in ["original", "replacement", "original"] {
        put(&mut ctx, "editable.crepus", &format!("span \"{text}\""));
        for html in variants("include editable.crepus", &ctx) {
            let html = html.unwrap();
            assert!(html.contains(text), "{html}");
            let obsolete = if text == "original" {
                "replacement"
            } else {
                "original"
            };
            assert!(!html.contains(obsolete), "{html}");
        }
    }
}

#[test]
fn cached_include_paths_keep_frontend_dispatch() {
    let mut ctx = TemplateContext::new();
    for (path, source, text) in [
        (
            "child.vue",
            "<template><strong>Vue child</strong></template>",
            "Vue child",
        ),
        (
            "child.svelte",
            "<strong>Svelte child</strong>",
            "Svelte child",
        ),
        ("child.jsx", "<strong>JSX child</strong>", "JSX child"),
    ] {
        put(&mut ctx, path, source);
        for _ in 0..2 {
            for html in variants(&format!("include {path}"), &ctx) {
                let html = html.unwrap();
                assert!(html.contains(text) && html.contains("<strong"), "{html}");
            }
        }
    }
}

#[test]
fn invalid_include_keeps_diagnostics_and_recovers_after_edit() {
    let mut ctx = TemplateContext::new();
    for _ in 0..2 {
        put(
            &mut ctx,
            "invalid.vue",
            "<template><div v-unknown=\"x\"></div></template>",
        );
        for html in variants("include invalid.vue", &ctx) {
            let error = html.unwrap_err().to_string();
            assert!(
                error.contains("include parse error:")
                    && error.contains("unsupported Vue directive"),
                "{error}"
            );
        }
        put(
            &mut ctx,
            "invalid.vue",
            "<template><span>Recovered child</span></template>",
        );
        for html in variants("include invalid.vue", &ctx) {
            assert!(html.unwrap().contains("Recovered child"));
        }
    }
}
