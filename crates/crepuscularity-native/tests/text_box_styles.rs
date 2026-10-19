use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::{render_template_to_ir_with_path, to_json};

#[test]
fn text_ir_keeps_common_box_styles() {
    let source = "text p-1.5 bg-blue-500 border-2 border-red-500 rounded-lg opacity-25 \"Styled\"";
    let ir = render_template_to_ir_with_path(source, &TemplateContext::new(), None).unwrap();
    let value: serde_json::Value = serde_json::from_str(&to_json(&ir).unwrap()).unwrap();
    let style = &value["root"][0]["style"];
    assert_eq!(style["padding"], 6.0);
    assert_eq!(style["borderWidth"], 2.0);
    assert_eq!(style["opacity"], 0.25);
    assert!(style["backgroundColor"].as_str().unwrap().starts_with('#'));
    assert!(style["borderColor"].as_str().unwrap().starts_with('#'));
    assert!(style["cornerRadius"].as_f64().unwrap() > 0.0);
}

#[test]
fn arbitrary_font_sizes_and_borders_remain_distinct_from_colors() {
    for tag in ["text", "div"] {
        let source =
            format!("{tag} text-[18.5px] text-red-500 border-[2.5px] border-blue-500 \"Styled\"");
        let ir = render_template_to_ir_with_path(&source, &TemplateContext::new(), None).unwrap();
        let value: serde_json::Value = serde_json::from_str(&to_json(&ir).unwrap()).unwrap();
        let style = &value["root"][0]["style"];
        assert_eq!(style["fontSize"], 18.5);
        assert_eq!(style["borderWidth"], 2.5);
        assert!(style["foregroundColor"].as_str().unwrap().starts_with('#'));
        assert!(style["borderColor"].as_str().unwrap().starts_with('#'));
    }
}

#[test]
fn invalid_arbitrary_sizes_do_not_replace_valid_styles() {
    for invalid in ["NaN", "inf", "-2", "1e39", "6rem"] {
        let source =
            format!("text text-[18px] border-2 text-[{invalid}px] border-[{invalid}px] \"Styled\"");
        let ir = render_template_to_ir_with_path(&source, &TemplateContext::new(), None).unwrap();
        let value: serde_json::Value = serde_json::from_str(&to_json(&ir).unwrap()).unwrap();
        assert_eq!(value["root"][0]["style"]["fontSize"], 18.0);
        assert_eq!(value["root"][0]["style"]["borderWidth"], 2.0);
    }
}

#[test]
fn nonfinite_dimensions_do_not_enter_serialized_ir() {
    for invalid in ["NaN", "inf", "1e38", "[NaNpx]", "[infpx]"] {
        let source = format!("div p-2 p-{invalid} w-4 w-{invalid}");
        let ir = render_template_to_ir_with_path(&source, &TemplateContext::new(), None).unwrap();
        let value: serde_json::Value = serde_json::from_str(&to_json(&ir).unwrap()).unwrap();
        assert_eq!(value["root"][0]["style"]["padding"], 8.0, "{invalid}");
        assert_eq!(value["root"][0]["style"]["width"], 16.0, "{invalid}");
    }
}

#[test]
fn nonfinite_presentation_values_preserve_previous_styles() {
    for invalid in ["NaN", "inf", "-inf", "1e39"] {
        let source = format!("text leading-4 leading-[{invalid}px] tracking-wide tracking-[{invalid}px] rounded-lg rounded-[{invalid}px] scale-50 scale-{invalid} rotate-30 rotate-{invalid} \"Styled\"");
        let ir = render_template_to_ir_with_path(&source, &TemplateContext::new(), None).unwrap();
        let value: serde_json::Value = serde_json::from_str(&to_json(&ir).unwrap()).unwrap();
        let style = &value["root"][0]["style"];
        assert_eq!(style["lineHeight"], 16.0, "{invalid}");
        assert_eq!(style["letterSpacing"], serde_json::json!(0.4), "{invalid}");
        assert_eq!(style["cornerRadius"], 8.0, "{invalid}");
        assert_eq!(style["scaleX"], 0.5, "{invalid}");
        assert_eq!(style["scaleY"], 0.5, "{invalid}");
        assert_eq!(style["rotate"], 30.0, "{invalid}");
    }
}

#[test]
fn invalid_line_clamps_preserve_a_previous_positive_limit() {
    for invalid in ["0", "-1", "-2147483648", "2147483648", "bad"] {
        let source = format!("text line-clamp-2 line-clamp-{invalid} \"Styled\"");
        let ir = render_template_to_ir_with_path(&source, &TemplateContext::new(), None).unwrap();
        let value: serde_json::Value = serde_json::from_str(&to_json(&ir).unwrap()).unwrap();
        assert_eq!(value["root"][0]["style"]["lineClamp"], 2, "{invalid}");
        let source = format!("text line-clamp-{invalid} \"Styled\"");
        let ir = render_template_to_ir_with_path(&source, &TemplateContext::new(), None).unwrap();
        let value: serde_json::Value = serde_json::from_str(&to_json(&ir).unwrap()).unwrap();
        assert!(
            value["root"][0]["style"]["lineClamp"].is_null(),
            "{invalid}"
        );
    }
}
