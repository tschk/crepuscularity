use crepuscularity_core::{eval::eval_expr, parse_template, TemplateContext};
use crepuscularity_tui::{render_template_to_buffer, BufferRenderer};
use ratatui::{buffer::Buffer, layout::Rect};

#[test]
fn consumed_property_errors_leave_the_buffer_unchanged() {
    for property in [
        "id",
        "title",
        "scroll-bottom",
        "scroll-offset",
        "scroll-y",
        "scroll",
    ] {
        for body in ["\"CONTENT\"", "span \"CONTENT\""] {
            let source = format!(
                "span \"BEFORE\"\ndiv border overflow-y-scroll {property}={{1 +}}\n  {body}"
            );
            let area = Rect::new(2, 3, 30, 5);
            let mut buffer = Buffer::empty(area);
            for cell in &mut buffer.content {
                cell.set_symbol("X");
            }
            let previous = buffer.clone();
            let ctx = TemplateContext::new();
            let error = render_template_to_buffer(&source, &ctx, &mut buffer, area).unwrap_err();
            assert_eq!(
                error.to_string(),
                eval_expr("1 +", &ctx).unwrap_err().to_string(),
                "{source}"
            );
            assert_eq!(buffer, previous, "{source}");
        }
    }
}

#[test]
fn unused_properties_keep_lazy_evaluation() {
    for source in [
        "div hidden id={1 +} title={1 +} scroll-offset={1 +}\n  span \"BAD\"\nspan \"VISIBLE\"",
        "if false\n  div id={1 +} title={1 +}\n    span \"BAD\"\nspan \"VISIBLE\"",
        "div scroll-offset={1 +} scroll-bottom={1 +}\n  span \"VISIBLE\"",
        "div flex-row overflow-y-scroll scroll-offset={1 +}\n  \"VISIBLE\"",
        "div overflow-y-scroll scroll-offset={1 +}\nspan \"VISIBLE\"",
    ] {
        let area = Rect::new(0, 0, 30, 5);
        let mut buffer = Buffer::empty(area);
        render_template_to_buffer(source, &TemplateContext::new(), &mut buffer, area).unwrap();
        let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(text.contains("VISIBLE"), "{source}");
    }
}

#[test]
fn renderer_recovers_and_preserves_scroll_value_fallbacks() {
    let area = Rect::new(0, 0, 20, 2);
    let mut buffer = Buffer::empty(area);
    let mut renderer = BufferRenderer::new();
    let mut ctx = TemplateContext::new();
    ctx.set("content", "zero\none\ntwo");
    for property in ["title", "id"] {
        let invalid = parse_template(&format!("div {property}={{1 +}}\n  \"BAD\"")).unwrap();
        assert!(renderer
            .render_nodes(&invalid, &ctx, &mut buffer, area)
            .is_err());
    }
    let valid = parse_template("div h-full overflow-y-scroll scroll-offset={offset} scroll-bottom={bottom}\n  \"{content}\"").unwrap();
    ctx.set("scroll_offset", "1");
    for (offset, bottom, first) in [
        ("nonsense", "false", "z"),
        ("-1", "false", "z"),
        ("1", "false", "o"),
        ("0", "1", "o"),
    ] {
        ctx.set("offset", offset);
        ctx.set("bottom", bottom);
        buffer.reset();
        renderer
            .render_nodes(&valid, &ctx, &mut buffer, area)
            .unwrap();
        assert_eq!(buffer[(0, 0)].symbol(), first);
    }
    ctx.vars.remove("offset");
    ctx.set("bottom", false);
    buffer.reset();
    renderer
        .render_nodes(&valid, &ctx, &mut buffer, area)
        .unwrap();
    assert_eq!(buffer[(0, 0)].symbol(), "z");
    let fallback = parse_template("div h-full overflow-y-scroll\n  \"{content}\"").unwrap();
    buffer.reset();
    renderer
        .render_nodes(&fallback, &ctx, &mut buffer, area)
        .unwrap();
    assert_eq!(buffer[(0, 0)].symbol(), "o");
}

#[test]
fn valid_titles_update_and_missing_titles_stay_empty() {
    let nodes = parse_template("div border title={heading}\n  \"BODY\"").unwrap();
    let mut renderer = BufferRenderer::new();
    let mut ctx = TemplateContext::new();
    let area = Rect::new(0, 0, 20, 4);
    let mut buffer = Buffer::empty(area);
    for heading in [Some("FIRST"), Some("SECOND"), None] {
        if let Some(heading) = heading {
            ctx.set("heading", heading);
        } else {
            ctx.vars.remove("heading");
        }
        buffer.reset();
        renderer
            .render_nodes(&nodes, &ctx, &mut buffer, area)
            .unwrap();
        let top: String = (0..20).map(|x| buffer[(x, 0)].symbol()).collect();
        assert_eq!(top.contains("FIRST"), heading == Some("FIRST"));
        assert_eq!(top.contains("SECOND"), heading == Some("SECOND"));
    }
}
