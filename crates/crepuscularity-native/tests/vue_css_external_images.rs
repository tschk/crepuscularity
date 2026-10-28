#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn css_string_image_urls_fail_before_styles_are_emitted() {
    for image in [
        "image-set(\"./missing.png\" 1x)",
        "-webkit-image-set(\"./missing.png\" 1x)",
        "IMAGE-SET(\"./missing.png\" 1x, \"./large.png\" 2x)",
    ] {
        let source =
            format!("<template><p>text</p></template><style>p{{background-image:{image}}}</style>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let result = compile_moonshine_vue(&source, Path::new("ExternalImage.vue"), target);
            assert!(result.is_err(), "accepted {image}");
            assert!(result.unwrap_err().contains("URLs"));
        }
    }
}
