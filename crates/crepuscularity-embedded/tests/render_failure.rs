#![cfg(feature = "std")]
use crepuscularity_embedded::{Color, Rgb565View, ScreenSize, Template};

#[test]
fn failed_template_render_preserves_borrowed_framebuffer() {
    let screen = ScreenSize::new(4, 4);
    let source = "div\n  include \"definitely-missing-fixture.crepus\"";
    let mut pixels = [0x1234u16; 16];
    let mut fb = Rgb565View::new(screen, &mut pixels).unwrap();
    let mut template = Template::from_source(source, screen);
    assert!(template.draw(&mut fb).is_err(), "{source}");
    assert!(template.document().is_none());
    assert_eq!(fb.pixels_mut(), &[0x1234; 16]);
    template.set_source("div bg-[#ffffff]");
    template.draw(&mut fb).unwrap();
    assert_eq!(
        fb.pixels_mut(),
        &[Color::rgb(255, 255, 255).to_rgb565().0; 16]
    );
}
