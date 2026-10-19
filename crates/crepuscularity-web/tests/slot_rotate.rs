use crepuscularity_core::context::TemplateContext;
use crepuscularity_web::render_template_to_html;

#[test]
fn slot_rotate_error_not_enough_children() {
    let tpl = "slot-rotate\n  \"one\"";
    let ctx = TemplateContext::new();
    let result = render_template_to_html(tpl, &ctx);

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("slot-rotate needs at least two plain-text phrase children"));
}

#[cfg(feature = "ssr")]
#[test]
fn slot_rotate_error_not_enough_children_ssr() {
    let tpl = "slot-rotate\n  \"one\"";
    let ctx = TemplateContext::new();
    let result = crepuscularity_web::render_template_to_html_with_ssr(tpl, &ctx, false);

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("slot-rotate needs at least two plain-text phrase children"));
}

#[test]
fn slot_rotate_invalid_timer_delays_use_the_default() {
    for value in [
        "0",
        "-1",
        "2147483648",
        "18446744073709551615",
        "1.5",
        "Infinity",
        "invalid",
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("delay", value);
        let template = "slot-rotate interval={delay}\n  \"one\"\n  \"two\"";
        let html = render_template_to_html(template, &ctx).unwrap();
        assert!(
            html.contains("data-slot-interval=\"3200\""),
            "{value}: {html}"
        );
        #[cfg(feature = "ssr")]
        {
            let html = crepuscularity_web::render_template_to_html_with_ssr(template, &ctx, false)
                .unwrap();
            assert!(
                html.contains("data-slot-interval=\"3200\""),
                "{value}: {html}"
            );
        }
    }
}

#[test]
fn slot_rotate_preserves_valid_timer_delays() {
    for value in ["1", "400", "2147483647"] {
        let mut ctx = TemplateContext::new();
        ctx.set("delay", value);
        let html =
            render_template_to_html("slot-rotate interval={delay}\n  \"one\"\n  \"two\"", &ctx)
                .unwrap();
        assert!(html.contains(&format!("data-slot-interval=\"{value}\"")));
    }
}
