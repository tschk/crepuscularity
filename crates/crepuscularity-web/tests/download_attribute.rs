use std::sync::Arc;

use crepuscularity_core::{TemplateContext, TemplateValue};

fn variants(source: &str, ctx: &TemplateContext) -> Vec<String> {
    #[allow(unused_mut)]
    let mut output = vec![crepuscularity_web::render_template_to_html(source, ctx).unwrap()];
    #[cfg(feature = "ssr")]
    for markers in [false, true] {
        output.push(
            crepuscularity_web::render_template_to_html_with_ssr(source, ctx, markers).unwrap(),
        );
    }
    #[cfg(feature = "hydration")]
    output.push(crepuscularity_web::render_template_to_html_with_hydration(source, ctx).unwrap());
    output
}

#[test]
fn typed_download_booleans_differ_from_literal_filenames() {
    for (value, expected) in [
        (TemplateValue::Bool(false), None),
        (TemplateValue::Null, None),
        (TemplateValue::Bool(true), Some("")),
        (TemplateValue::Str("false".into()), Some("false")),
        (TemplateValue::Str("true".into()), Some("true")),
        (TemplateValue::Str("".into()), Some("")),
        (
            TemplateValue::Str("report &\".txt".into()),
            Some("report &amp;&quot;.txt"),
        ),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("value", value);
        Arc::make_mut(&mut ctx.virtual_files).insert(
            "Entry.jsx".into(),
            "<a href=\"/file\" download={value}>File</a>".into(),
        );
        for source in [
            "a href='/file' download={value} \"File\"",
            "include Entry.jsx value=value",
        ] {
            for html in variants(source, &ctx) {
                let opening = html.split("<a ").nth(1).unwrap().split('>').next().unwrap();
                if let Some(expected) = expected {
                    assert!(
                        opening.contains(&format!(" download=\"{expected}\"")),
                        "{source}: {html}"
                    );
                } else {
                    assert!(!opening.contains(" download"), "{source}: {html}");
                }
            }
        }
    }
}

#[test]
fn angular_attribute_download_retains_false_as_literal_presence() {
    let mut ctx = TemplateContext::new();
    Arc::make_mut(&mut ctx.virtual_files).insert(
        "Entry.ng".into(),
        "<a href=\"/file\" [attr.download]=\"false\">File</a>".into(),
    );
    for html in variants("include Entry.ng", &ctx) {
        let opening = html.split("<a ").nth(1).unwrap().split('>').next().unwrap();
        assert!(opening.contains(" download=\"false\""), "{html}");
    }
}
