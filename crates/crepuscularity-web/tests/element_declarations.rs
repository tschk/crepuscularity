use crepuscularity_core::{TemplateContext, TemplateValue};
use crepuscularity_web::render_template_to_html;

fn outputs(source: &str, ctx: &TemplateContext) -> Vec<String> {
    #[allow(unused_mut)]
    let mut out = vec![render_template_to_html(source, ctx).unwrap()];
    #[cfg(feature = "ssr")]
    for markers in [false, true] {
        out.push(
            crepuscularity_web::render_template_to_html_with_ssr(source, ctx, markers).unwrap(),
        );
    }
    #[cfg(feature = "hydration")]
    out.push(crepuscularity_web::render_template_to_html_with_hydration(source, ctx).unwrap());
    out
}

#[test]
fn child_declarations_observe_order_defaults_and_lexical_boundaries() {
    let mut ctx = TemplateContext::new();
    ctx.set("label", "CALLER");
    let source = "div\n  span \"before:{label}\"\n  $: default label = 'DEFAULT'\n  span \"default:{label}\"\n  $: let label = 'LOCAL'\n  span \"local:{label}\"\n  section\n    $: let label = 'INNER'\n    span \"inner:{label}\"\n  span \"after:{label}\"\nspan \"outside:{label}\"";
    for html in outputs(source, &ctx) {
        for expected in [
            "before:CALLER",
            "default:CALLER",
            "local:LOCAL",
            "inner:INNER",
            "after:LOCAL",
            "outside:CALLER",
        ] {
            assert!(html.contains(expected), "missing {expected}: {html}");
        }
    }
    assert_eq!(ctx.get_str("label"), "CALLER");
    for html in outputs(
        "div\n  $: default label = 'DEFAULT'\n  span \"default:{label}\"",
        &TemplateContext::new(),
    ) {
        assert!(html.contains("default:DEFAULT"), "{html}");
    }
}

#[test]
fn declaration_overlay_reaches_control_flow_includes_and_caller_slots() {
    let mut ctx = TemplateContext::new();
    std::sync::Arc::make_mut(&mut ctx.virtual_files).insert(
        "child.crepus".into(),
        "section\n  span \"include:{label}\"\n  slot".into(),
    );
    let mut first = TemplateContext::new();
    first.set("value", 1);
    let mut second = TemplateContext::new();
    second.set("value", 2);
    ctx.set("items", TemplateValue::List(vec![first, second]));
    let source = "div\n  $: let label = 'LOCAL'\n  if label == 'LOCAL'\n    span \"condition:{label}\"\n  for item in items\n    span \"loop:{label}-{item}\"\n  include child.crepus label={label}\n    span \"slot:{label}\"";
    for html in outputs(source, &ctx) {
        for expected in [
            "condition:LOCAL",
            "loop:LOCAL-1",
            "loop:LOCAL-2",
            "include:LOCAL",
            "slot:LOCAL",
        ] {
            assert!(html.contains(expected), "missing {expected}: {html}");
        }
    }
}

#[test]
fn invalid_child_declaration_expression_is_not_silently_discarded() {
    let source = "div\n  $: let label = 1 +\n  span \"after\"";
    assert!(render_template_to_html(source, &TemplateContext::new()).is_err());
    #[cfg(feature = "ssr")]
    for markers in [false, true] {
        assert!(crepuscularity_web::render_template_to_html_with_ssr(
            source,
            &TemplateContext::new(),
            markers
        )
        .is_err());
    }
    #[cfg(feature = "hydration")]
    assert!(crepuscularity_web::render_template_to_html_with_hydration(
        source,
        &TemplateContext::new()
    )
    .is_err());
}
