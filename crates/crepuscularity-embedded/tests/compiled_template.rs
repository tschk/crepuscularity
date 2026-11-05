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
        golden: "32e4e6d8efe757eda0990a3fb4f464b733d7dae89f74c3c7459b8064878978ed",
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
        golden: "6af116ce7910e8b0c1b1388226542cf73047a49b6c99c03253b3a66c4c5cb19f",
    },
    Case {
        template: STM32_UI,
        source: include_str!("../../../examples/advanced/embedded-stm32/ui.crepus"),
        vars: &[("cpu", Value::Int(73)), ("status", Value::Str("ok"))],
        golden: "c72e2fdb4b91d988dd2d3b511bd7d7fa6e3f892986249744c9533ca75dab103e",
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
        assert_eq!(
            digest(expected.pixels()),
            case.golden,
            "std renderer, case {i}"
        );
        assert_eq!(actual.pixels(), expected.pixels(), "case {i}");
        for y in (0..SCREEN.height).step_by(4) {
            for x in (0..SCREEN.width).step_by(4) {
                assert_eq!(
                    actual_doc.hit_test(x, y),
                    expected_doc.hit_test(x, y),
                    "case {i} at {x},{y}"
                );
            }
        }
    }
}
