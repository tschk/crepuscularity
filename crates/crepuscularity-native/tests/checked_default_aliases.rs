use crepuscularity_core::{TemplateContext, TemplateValue};
use crepuscularity_native::{render_template_to_ir, render_template_to_ir_with_path};
use std::path::Path;

fn lower(source: &str, filename: &str, ctx: &TemplateContext) -> serde_json::Value {
    serde_json::to_value(
        render_template_to_ir_with_path(source, ctx, Some(Path::new(filename))).unwrap(),
    )
    .unwrap()
}

#[test]
fn checked_aliases_match_canonical_ir_across_frontends() {
    let mut ctx = TemplateContext::new();
    ctx.set("state", true);
    for alias in ["defaultChecked", "default-checked"] {
        for (filename, canonical) in [
            ("Entry.crepus", "input type=checkbox checked"),
            ("Entry.jsx", "<input type='checkbox' checked/>"),
            ("Entry.svelte", "<input type='checkbox' checked/>"),
            ("Entry.astro", "<input type='checkbox' checked/>"),
            (
                "Entry.vue",
                "<template><input type='checkbox' checked/></template>",
            ),
            ("Entry.ng", "<input type='checkbox' checked/>"),
            ("Entry.crepus", "input type=checkbox checked={state}"),
            ("Entry.jsx", "<input type='checkbox' checked={state}/>"),
            ("Entry.svelte", "<input type='checkbox' checked={state}/>"),
            ("Entry.astro", "<input type='checkbox' checked={state}/>"),
            (
                "Entry.vue",
                "<template><input type='checkbox' :checked='state'/></template>",
            ),
            ("Entry.ng", "<input type='checkbox' [checked]='state'/>"),
        ] {
            let source = canonical.replace("checked", alias);
            let result = lower(&source, filename, &ctx);
            assert_eq!(
                result,
                lower(canonical, filename, &ctx),
                "{filename}: {source}"
            );
            assert_eq!(result["root"][0]["checked"], true, "{filename}: {source}");
        }
    }
}

#[test]
fn checked_aliases_preserve_html_presence_custom_coercion_and_binding() {
    let mut ctx = TemplateContext::new();
    for tag in ["input type=checkbox", "checkbox", "toggle", "switch"] {
        for alias in ["checked", "defaultChecked", "default-checked"] {
            for (state, html, custom) in [
                (TemplateValue::Bool(true), true, true),
                (TemplateValue::Bool(false), false, false),
                (TemplateValue::Null, false, false),
                (TemplateValue::Str("false".into()), true, false),
                (TemplateValue::Str("0".into()), true, false),
                (TemplateValue::Str("".into()), true, true),
                (TemplateValue::Str("yes".into()), true, true),
            ] {
                ctx.set("state", state);
                let ir = render_template_to_ir(&format!("{tag} bind=live {alias}={{state}}"), &ctx)
                    .unwrap();
                let json = serde_json::to_value(ir).unwrap();
                assert_eq!(
                    json["root"][0]["checked"],
                    if tag == "input type=checkbox" {
                        html
                    } else {
                        custom
                    },
                    "{tag} {alias}"
                );
                assert_eq!(json["root"][0]["bind"], "live");
            }
            let bare = render_template_to_ir(&format!("{tag} {alias}"), &ctx).unwrap();
            assert_eq!(
                serde_json::to_value(bare).unwrap()["root"][0]["checked"],
                true
            );
            assert!(render_template_to_ir(&format!("{tag} {alias}={{1 +}}"), &ctx).is_err());
        }
    }
}

#[test]
fn duplicate_checked_names_and_distinct_aliases_are_rejected() {
    for tag in ["input type=checkbox", "checkbox", "toggle", "switch"] {
        for alias in ["checked", "defaultChecked", "default-checked"] {
            let error = render_template_to_ir(
                &format!("{tag} {alias}=true {alias}=false"),
                &TemplateContext::new(),
            )
            .unwrap_err()
            .to_string();
            assert!(
                error.contains(&format!("duplicate {alias} binding")),
                "{error}"
            );
        }
        for attrs in [
            "checked=true defaultChecked=true",
            "checked=false default-checked=false",
            "defaultChecked=true default-checked=true",
        ] {
            let error = render_template_to_ir(&format!("{tag} {attrs}"), &TemplateContext::new())
                .unwrap_err()
                .to_string();
            assert!(error.contains("cannot combine"), "{error}");
        }
    }
}
