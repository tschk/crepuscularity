use crepuscularity_embedded::{
    font::draw_text, Color, FontMetrics, Framebuffer, Rect, Rgb565Buffer, ScreenSize,
};

fn metrics(width: u8, height: u8) -> FontMetrics {
    FontMetrics {
        cell_width: width,
        cell_height: height,
        baseline: height.saturating_sub(1),
    }
}

fn framebuffer() -> Rgb565Buffer {
    Rgb565Buffer::new(ScreenSize::new(32, 32), Color::rgb(0, 0, 0))
}

#[test]
fn smaller_cells_clip_ascii_to_the_character_rectangle() {
    let mut fb = framebuffer();
    draw_text(
        &mut fb,
        Rect::new(2, 3, 20, 20),
        "K",
        Color::rgb(255, 255, 255),
        &metrics(4, 4),
    );
    assert!(fb.pixels().iter().any(|pixel| *pixel != 0));
    for (i, pixel) in fb.pixels().iter().enumerate() {
        if *pixel != 0 {
            let (x, y) = (i % 32, i / 32);
            assert!(
                (2..6).contains(&x) && (3..7).contains(&y),
                "pixel outside cell: ({x}, {y})"
            );
        }
    }
}

#[test]
fn measured_bounds_match_roomy_bounds_for_custom_metrics() {
    for width in [1, 4, 8, 12] {
        for height in [1, 4, 8, 12] {
            let metrics = metrics(width, height);
            for text in ["K", "KL", "K\nL", "界K"] {
                let mut measured = framebuffer();
                let mut roomy = framebuffer();
                let (w, h) = metrics.measure_text(text);
                let white = Color::rgb(255, 255, 255);
                draw_text(&mut measured, Rect::new(3, 2, w, h), text, white, &metrics);
                draw_text(&mut roomy, Rect::new(3, 2, 29, 30), text, white, &metrics);
                assert_eq!(
                    measured.pixels(),
                    roomy.pixels(),
                    "{width}x{height}: {text:?}"
                );
            }
        }
    }
}

#[test]
fn zero_sized_cells_do_not_paint_ascii_or_replacement_glyphs() {
    for (width, height) in [(0, 8), (8, 0), (0, 0)] {
        for text in ["K", "界", "KL\n界"] {
            let mut fb = framebuffer();
            draw_text(
                &mut fb,
                Rect::new(0, 0, 32, 32),
                text,
                Color::rgb(255, 255, 255),
                &metrics(width, height),
            );
            assert!(
                fb.pixels().iter().all(|pixel| *pixel == 0),
                "{width}x{height}: {text:?}"
            );
        }
    }
}

#[test]
fn larger_cells_keep_the_original_unscaled_bitmap() {
    let mut roomy = framebuffer();
    let mut builtin = framebuffer();
    let white = Color::rgb(255, 255, 255);
    draw_text(
        &mut roomy,
        Rect::new(2, 3, 24, 24),
        "K",
        white,
        &metrics(12, 12),
    );
    draw_text(
        &mut builtin,
        Rect::new(2, 3, 24, 24),
        "K",
        white,
        &FontMetrics::default(),
    );
    assert_eq!(roomy.pixels(), builtin.pixels());
}

#[test]
fn adjacent_and_multiline_characters_match_individually_clipped_tiles() {
    let mut actual = framebuffer();
    let mut expected = framebuffer();
    let white = Color::rgb(255, 255, 255);
    let metrics = metrics(4, 4);
    draw_text(
        &mut actual,
        Rect::new(2, 3, 20, 20),
        "KL\nMN",
        white,
        &metrics,
    );
    for (row, line) in ["KL", "MN"].iter().enumerate() {
        for (column, ch) in line.chars().enumerate() {
            let mut tile = Rgb565Buffer::new(ScreenSize::new(4, 4), Color::rgb(0, 0, 0));
            draw_text(
                &mut tile,
                Rect::new(0, 0, 4, 4),
                &ch.to_string(),
                white,
                &metrics,
            );
            for y in 0..4 {
                for x in 0..4 {
                    if tile.pixels()[y * 4 + x] != 0 {
                        expected.set_pixel(
                            (2 + column * 4 + x) as u16,
                            (3 + row * 4 + y) as u16,
                            white,
                        );
                    }
                }
            }
        }
    }
    assert_eq!(actual.pixels(), expected.pixels());
}
