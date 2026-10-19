use crepuscularity_core::{TemplateContext, TemplateValue};
use crepuscularity_lvgl::render_template_to_lvgl_xml;

#[test]
fn supported_range_widgets_map_static_and_evaluated_bounds() {
    let mut ctx = TemplateContext::new();
    ctx.set("lower", -20_i64);
    ctx.set("upper", 200_i64);
    ctx.set("current", 15_i64);
    for tag in ["slider", "progress", "meter"] {
        for (bounds, min, max) in [
            ("min=-20 max=200", "-20", "200"),
            ("min={lower} max={upper}", "-20", "200"),
            ("min=200 max=-20", "200", "-20"),
        ] {
            let source = format!("{tag} {bounds} value={{current}}");
            let xml = render_template_to_lvgl_xml(&source, &ctx).unwrap();
            assert!(xml.contains(&format!(" min_value=\"{min}\"")), "{xml}");
            assert!(xml.contains(&format!(" max_value=\"{max}\"")), "{xml}");
            assert!(xml.contains(" value=\"15\""), "{xml}");
            assert!(!xml.contains(" min=") && !xml.contains(" max="), "{xml}");
        }
    }
}

#[test]
fn aliases_keep_last_attribute_precedence_and_other_widgets_unchanged() {
    for tag in ["slider", "progress", "meter"] {
        for (bounds, min, max) in [
            ("min=1 min_value=2 max=10 max_value=20", "2", "20"),
            ("min_value=2 min=1 max_value=20 max=10", "1", "10"),
        ] {
            let xml = render_template_to_lvgl_xml(
                &format!("{tag} {bounds} value=3"),
                &TemplateContext::new(),
            )
            .unwrap();
            assert_eq!(xml.matches(" min_value=").count(), 1, "{xml}");
            assert_eq!(xml.matches(" max_value=").count(), 1, "{xml}");
            assert!(xml.contains(&format!(" min_value=\"{min}\"")), "{xml}");
            assert!(xml.contains(&format!(" max_value=\"{max}\"")), "{xml}");
            assert!(xml.contains(" value=\"3\""), "{xml}");
        }
    }
    let xml =
        render_template_to_lvgl_xml("div min=1 max=2 step=3", &TemplateContext::new()).unwrap();
    assert!(
        xml.contains(" min=\"1\"") && xml.contains(" max=\"2\"") && xml.contains(" step=\"3\""),
        "{xml}"
    );
}

#[test]
fn unit_steps_are_omitted_and_unsupported_steps_error_explicitly() {
    let mut ctx = TemplateContext::new();
    for tag in ["slider", "progress", "meter"] {
        for step in ["1", "\"1.0\"", "\"1.000\"", "{1}", "{1.0}", "{unit}"] {
            ctx.set("unit", 1_i64);
            let xml = render_template_to_lvgl_xml(
                &format!("{tag} min=0 max=10 step={step} value=3"),
                &ctx,
            )
            .unwrap();
            assert!(!xml.contains(" step="), "{xml}");
            assert!(xml.contains(" value=\"3\""), "{xml}");
        }
        for value in [
            TemplateValue::Int(2),
            TemplateValue::Int(0),
            TemplateValue::Int(-1),
            TemplateValue::Float(0.5),
            TemplateValue::Bool(true),
            TemplateValue::Null,
            TemplateValue::Str("any".into()),
            TemplateValue::Str("1.0000000000000001".into()),
        ] {
            ctx.set("step", value);
            let error =
                render_template_to_lvgl_xml(&format!("{tag} step={{step}}"), &ctx).unwrap_err();
            assert!(
                error.to_string().contains("step") && error.to_string().contains(tag),
                "{error}"
            );
        }
        assert!(render_template_to_lvgl_xml(&format!("{tag} step=2"), &ctx).is_err());
    }
}

#[test]
fn range_aliases_validate_integer_snapshots_without_changing_raw_target_attributes() {
    let mut ctx = TemplateContext::new();
    for tag in ["slider", "progress", "meter"] {
        for value in [
            TemplateValue::Int(i64::from(i32::MIN)),
            TemplateValue::Int(i64::from(i32::MAX)),
            TemplateValue::Float(-20.0),
            TemplateValue::Str(" 2147483647 ".into()),
            TemplateValue::Str("-2147483648.000".into()),
        ] {
            ctx.set("bound", value);
            assert!(render_template_to_lvgl_xml(
                &format!("{tag} min={{bound}} max={{bound}}"),
                &ctx
            )
            .is_ok());
        }
        let quoted =
            render_template_to_lvgl_xml(&format!("{tag} min=\"-20\" max=\"200.0\""), &ctx).unwrap();
        assert!(quoted.contains(" min_value=\"-20\""), "{quoted}");
        assert!(quoted.contains(" max_value=\"200\""), "{quoted}");
        for value in [
            TemplateValue::Float(1.5),
            TemplateValue::Float(f64::NAN),
            TemplateValue::Float(f64::INFINITY),
            TemplateValue::Float(f64::NEG_INFINITY),
            TemplateValue::Int(i64::from(i32::MAX) + 1),
            TemplateValue::Int(i64::from(i32::MIN) - 1),
            TemplateValue::Float(f64::from(i32::MAX) + 1.0),
            TemplateValue::Str("2147483648".into()),
            TemplateValue::Str("-2147483649".into()),
            TemplateValue::Str("1.0000000000000001".into()),
            TemplateValue::Bool(true),
            TemplateValue::Null,
            TemplateValue::List(vec![]),
            TemplateValue::Scope(TemplateContext::new()),
        ] {
            ctx.set("bound", value);
            for attribute in ["min", "max"] {
                let error =
                    render_template_to_lvgl_xml(&format!("{tag} {attribute}={{bound}}"), &ctx)
                        .unwrap_err();
                assert!(
                    error.to_string().contains(tag) && error.to_string().contains(attribute),
                    "{error}"
                );
            }
        }
        let raw = render_template_to_lvgl_xml(
            &format!("{tag} min_value=\"custom\" max_value=\"custom\""),
            &ctx,
        )
        .unwrap();
        assert!(
            raw.contains(" min_value=\"custom\"") && raw.contains(" max_value=\"custom\""),
            "{raw}"
        );
    }
}
