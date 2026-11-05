use crepuscularity_core::TemplateContext;
use crepuscularity_lvgl::render_template_to_lvgl_xml;

#[test]
fn named_background_text_and_border_colors_match_the_complete_shared_palette() {
    for family in [
        "amber", "blue", "cyan", "emerald", "fuchsia", "gray", "green", "indigo", "lime", "mauve",
        "mist", "neutral", "olive", "orange", "pink", "purple", "red", "rose", "sky", "slate",
        "stone", "taupe", "teal", "violet", "yellow", "zinc",
    ] {
        for shade in [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950] {
            let name = format!("{family}-{shade}");
            let expected = crepuscularity_colors::lookup_named_color(&name).unwrap();
            let xml = render_template_to_lvgl_xml(
                &format!("div bg-{name} text-{name} border-{name}"),
                &TemplateContext::new(),
            )
            .unwrap();
            for attr in ["bg_color", "text_color", "border_color"] {
                assert!(
                    xml.contains(&format!("{attr}=\"{expected}\"")),
                    "{name}: {xml}"
                );
            }
        }
    }
}

#[test]
fn special_colors_arbitrary_hex_and_unknown_names_keep_existing_behavior() {
    for (name, expected) in [
        ("black", "#000000"),
        ("white", "#ffffff"),
        ("transparent", "#000000"),
        ("[#123abc]", "#123abc"),
    ] {
        let xml = render_template_to_lvgl_xml(
            &format!("div bg-{name} text-{name} border-{name}"),
            &TemplateContext::new(),
        )
        .unwrap();
        for attr in ["bg_color", "text_color", "border_color"] {
            assert!(
                xml.contains(&format!("{attr}=\"{expected}\"")),
                "{name}: {xml}"
            );
        }
    }
    let xml = render_template_to_lvgl_xml(
        "div bg-not-a-color text-not-a-color border-not-a-color",
        &TemplateContext::new(),
    )
    .unwrap();
    for attr in ["bg_color=", "text_color=", "border_color="] {
        assert!(!xml.contains(attr), "{xml}");
    }
}
