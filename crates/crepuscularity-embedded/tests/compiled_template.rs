//! Compile-time templates render without the parser.
//!
//! Run both ways; the `no_std` build must paint the same pixels the `std` renderer does:
//! `cargo test -p crepuscularity-embedded --test compiled_template`
//! `cargo test -p crepuscularity-embedded --no-default-features --test compiled_template`

use crepuscularity_embedded::compiled::{CompiledTemplate, Value};
use crepuscularity_embedded::{Color, Rgb565Buffer, ScreenSize};
use sha2::{Digest, Sha256};

static FIXTURE: &CompiledTemplate =
    crepuscularity_macros::embedded_document!("tests/fixtures/compiled.crepus");
static STM32_UI: &CompiledTemplate =
    crepuscularity_macros::embedded_document!("../../examples/advanced/embedded-stm32/ui.crepus");

const SCREEN: ScreenSize = ScreenSize::new(240, 160);

struct Case {
    template: &'static CompiledTemplate,
    /// Reference source for the `std` renderer.
    #[cfg_attr(not(feature = "std"), allow(dead_code))]
    source: &'static str,
    vars: &'static [(&'static str, Value<'static>)],
    /// SHA-256 of the RGB565 pixels painted by the `std` renderer.
    golden: &'static str,
}

const CASES: &[Case] = &[
    Case {
        template: FIXTURE,
        source: include_str!("fixtures/compiled.crepus"),
        vars: &[
            ("cpu", Value::Int(42)),
            ("label", Value::Str("load")),
            ("hot", Value::Bool(false)),
            ("alarm", Value::Bool(false)),
            // Target variables win over caller values in both renderers.
            ("is_embedded", Value::Bool(false)),
        ],
        golden: "d439479305f027ed8142093dd117ba51ed65e2993e20bd64bef4c948787bce81",
    },
    Case {
        template: FIXTURE,
        source: include_str!("fixtures/compiled.crepus"),
        vars: &[
            ("cpu", Value::Int(97)),
            ("label", Value::Str("")),
            ("hot", Value::Int(1)),
            ("alarm", Value::Str("yes")),
        ],
        golden: "6d07c2391396eb46cfee1a9e76d12c4e7348b7c4f4dd5d0cc7844d7f9d1d6aa2",
    },
    Case {
        template: STM32_UI,
        source: include_str!("../../../examples/advanced/embedded-stm32/ui.crepus"),
        vars: &[("cpu", Value::Int(73)), ("status", Value::Str("ok"))],
        golden: "684af25a96ada83e8667cae858b0feb9b909035390b7edf8eb522ec602867d0e",
    },
];

fn digest(pixels: &[u16]) -> String {
    let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
    Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn render_compiled(case: &Case) -> (Rgb565Buffer, crepuscularity_embedded::EmbeddedDocument) {
    let mut fb = Rgb565Buffer::new(SCREEN, Color::rgb(0, 0, 0));
    let doc = case.template.render(case.vars, SCREEN, &mut fb);
    (fb, doc)
}

#[test]
fn compiled_templates_match_golden_pixels_and_hit_targets() {
    for (i, case) in CASES.iter().enumerate() {
        let (fb, _) = render_compiled(case);
        assert!(
            fb.pixels().iter().any(|p| *p != fb.pixels()[0]),
            "case {i} painted nothing"
        );
        assert_eq!(digest(fb.pixels()), case.golden, "case {i}");
    }
}

#[test]
fn compiled_fixture_layout_colors_and_half_open_hits_follow_authored_geometry() {
    use crepuscularity_embedded::Rect;
    for (i, case) in CASES.iter().enumerate() {
        let (fb, doc) = render_compiled(case);
        // p-2 = 8px, the row is 16px high, and items-center centers 8px glyph cells.
        assert_eq!(
            doc.node_by_id("cpu-value").unwrap().bounds,
            Rect::new(40, 12, 192, 8)
        );
        let color = if i == 1 {
            Color::rgb(251, 44, 54)
        } else {
            Color::rgb(5, 223, 114)
        }
        .to_rgb565()
        .0;
        assert!(fb.pixels().contains(&color));
        for (index, pixel) in fb.pixels().iter().enumerate() {
            if *pixel == color {
                assert!((40..232).contains(&(index % 240)));
                assert!((12..20).contains(&(index / 240)));
            }
        }
        if i < 2 {
            // 8px top padding + 16px row + 8px gap + 24px status + 8px gap.
            let reset = doc.node_by_id("reset").unwrap();
            assert_eq!(reset.bounds, Rect::new(8, 64, 56, 16));
            assert_eq!(reset.on_click.as_deref(), Some("reset_alarm"));
            assert_eq!(doc.hit_test(8, 64), Some("reset"));
            assert_eq!(doc.hit_test(63, 79), Some("reset"));
            assert_eq!(doc.hit_test(64, 79), None);
            assert_eq!(doc.hit_test(8, 80), None);
            assert_eq!(
                doc.node_by_id(if i == 0 { "ok" } else { "alarm" })
                    .unwrap()
                    .bounds,
                Rect::new(8, 32, 224, 24)
            );
            assert_eq!(doc.hit_test(7, 32), None);
            assert_eq!(
                doc.hit_test(12, 36),
                Some(if i == 0 { "ok" } else { "alarm" })
            );
        } else {
            assert_eq!(
                doc.node_by_id("status-label").unwrap().bounds,
                Rect::new(64, 36, 168, 8)
            );
            assert_eq!(doc.hit_test(64, 36), Some("status-label"));
            assert_eq!(doc.hit_test(64, 44), None);
        }
    }
}

#[test]
fn compiled_conditions_and_button_labels_ignore_nonvisible_children() {
    static CONTROLS: &CompiledTemplate =
        crepuscularity_macros::embedded_document!("tests/fixtures/compiled-controls.crepus");
    for (show, label) in [(false, "Fallback"), (true, "Shown")] {
        let mut fb = Rgb565Buffer::new(SCREEN, Color::rgb(0, 0, 0));
        let doc = CONTROLS.render(&[("show", Value::Bool(show))], SCREEN, &mut fb);
        let action = doc.node_by_id("action").unwrap();
        assert_eq!(action.text.as_deref(), Some(label));
        assert_eq!(action.bounds.y, 0);
        assert_eq!(doc.hit_test(0, 0), Some("action"));
        assert!(fb.pixels().contains(&0x07e0));
        #[cfg(feature = "std")]
        {
            let mut ctx = crepuscularity_embedded::TemplateContext::new();
            ctx.set("show", show);
            let mut expected = Rgb565Buffer::new(SCREEN, Color::rgb(0, 0, 0));
            let expected_doc = crepuscularity_embedded::render_template_to_framebuffer(
                include_str!("fixtures/compiled-controls.crepus"),
                &ctx,
                SCREEN,
                &mut expected,
            )
            .unwrap();
            assert_eq!(fb.pixels(), expected.pixels());
            for y in 0..SCREEN.height {
                for x in 0..SCREEN.width {
                    assert_eq!(doc.hit_test(x, y), expected_doc.hit_test(x, y));
                }
            }
        }
    }
}

#[test]
fn compiled_documents_keep_text_ids_and_click_handlers() {
    let doc = FIXTURE.document(CASES[0].vars, SCREEN);
    let cpu = doc.node_by_id("cpu-value").unwrap();
    assert_eq!(cpu.text.as_deref(), Some("42% load"));
    assert!(doc.node_by_id("alarm").is_none());
    let ok = doc.node_by_id("ok").unwrap();
    assert_eq!(ok.children[0].text.as_deref(), Some("ok embedded 240x160"));
    let reset = doc.node_by_id("reset").unwrap();
    assert_eq!(reset.text.as_deref(), Some("Reset"));
    assert_eq!(reset.on_click.as_deref(), Some("reset_alarm"));

    let doc = FIXTURE.document(CASES[1].vars, SCREEN);
    assert_eq!(
        doc.node_by_id("cpu-value").unwrap().text.as_deref(),
        Some("97% ")
    );
    assert!(doc.node_by_id("ok").is_none());
    assert!(doc.node_by_id("alarm").is_some());
}

/// The `std` renderer is the reference: same template source, same values, same pixels.
#[cfg(feature = "std")]
#[test]
fn std_renderer_agrees_with_compiled_output() {
    use crepuscularity_embedded::{render_template_to_framebuffer, TemplateContext, TemplateValue};
    for (i, case) in CASES.iter().enumerate() {
        let mut ctx = TemplateContext::new();
        for (name, value) in case.vars {
            let value = match *value {
                Value::Str(s) => TemplateValue::Str(s.into()),
                Value::Int(n) => TemplateValue::Int(n),
                Value::Bool(b) => TemplateValue::Bool(b),
            };
            ctx.vars.insert((*name).into(), value);
        }
        let mut expected = Rgb565Buffer::new(SCREEN, Color::rgb(0, 0, 0));
        let expected_doc =
            render_template_to_framebuffer(case.source, &ctx, SCREEN, &mut expected).unwrap();
        let (actual, actual_doc) = render_compiled(case);
        assert_eq!(actual.pixels(), expected.pixels(), "case {i}");
        for y in 0..SCREEN.height {
            for x in 0..SCREEN.width {
                assert_eq!(
                    actual_doc.hit_test(x, y),
                    expected_doc.hit_test(x, y),
                    "case {i} at {x},{y}"
                );
            }
        }
    }
}
