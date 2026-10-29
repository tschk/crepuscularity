#![cfg(feature = "std")]
use crepuscularity_embedded::{Color, Framebuffer, Rgb888Buffer, ScreenSize};

#[test]
fn ppm_contains_exact_header_and_row_major_rgb_channels() {
    let mut fb = Rgb888Buffer::new(ScreenSize::new(2, 2), Color::rgb(0, 0, 0));
    for (x, y, color) in [
        (0, 0, Color::rgb(255, 0, 0)),
        (1, 0, Color::rgb(0, 255, 0)),
        (0, 1, Color::rgb(0, 0, 255)),
        (1, 1, Color::rgb(21, 87, 193)),
    ] {
        fb.set_pixel(x, y, color);
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("image.ppm");
    fb.write_ppm(&path).unwrap();
    let mut expected = b"P6\n2 2\n255\n".to_vec();
    expected.extend([255, 0, 0, 0, 255, 0, 0, 0, 255, 21, 87, 193]);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    let empty = Rgb888Buffer::new(ScreenSize::new(0, 0), Color::rgb(0, 0, 0));
    empty.write_ppm(&path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"P6\n0 0\n255\n");
}
