use crepuscularity_core::TemplateContext;
use crepuscularity_web::render_template_to_html;
use std::sync::Arc;

fn variants(source: &str) -> Vec<String> {
    let mut ctx = TemplateContext::new();
    ctx.set("classes", "literal-{name} flex");
    ctx.set("name", "CHANGED");
    ctx.set("label", "a\"<&雪");
    Arc::make_mut(&mut ctx.virtual_files).insert("Entry.ng".into(), source.into());
    let template = "include Entry.ng classes=classes label=label name=name";
    let result = vec![render_template_to_html(template, &ctx).unwrap()];
    #[cfg(feature = "ssr")]
    let result = {
        let mut result = result;
        result.push(
            crepuscularity_web::render_template_to_html_with_ssr(template, &ctx, true).unwrap(),
        );
        result
    };
    #[cfg(feature = "hydration")]
    let result = {
        let mut result = result;
        result.push(
            crepuscularity_web::render_template_to_html_with_hydration(template, &ctx).unwrap(),
        );
        result
    };
    result
}

#[test]
fn attribute_presence_null_and_literal_strings_match_in_all_renderers() {
    for html in variants(
        r#"<input [attr.checked]="false" [disabled]="false" [attr.title]="null" [attr.name]="label"/><div [attr.hidden]="false" [attr.class]="classes" [attr.id]="null">visible</div>"#,
    ) {
        assert!(html.contains("checked=\"false\""), "{html}");
        assert!(html.contains("hidden=\"false\""), "{html}");
        assert!(html.contains("class=\"literal-{name} flex\""), "{html}");
        assert!(html.contains("name=\"a&quot;&lt;&amp;雪\""), "{html}");
        assert!(!html.contains(" disabled="), "{html}");
        assert!(!html.contains(" title="), "{html}");
        assert!(
            !html
                .split("<div")
                .nth(1)
                .unwrap()
                .split('>')
                .next()
                .unwrap()
                .contains(" id="),
            "{html}"
        );
    }
}

#[test]
fn attribute_values_do_not_replace_textarea_content_and_urls_stay_filtered() {
    for html in variants(
        r#"<textarea [attr.value]="'attribute'">body</textarea><a [attr.href]="'javascript:bad'">safe</a><input [attr.bind]="false"/>"#,
    ) {
        assert!(
            html.contains("value=\"attribute\">body</textarea>"),
            "{html}"
        );
        assert!(!html.contains("href="), "{html}");
        assert!(html.contains("bind=\"false\""), "{html}");
    }
}

#[test]
fn attribute_interval_is_serialized_without_configuring_rotation() {
    for html in variants(
        r#"<slot-rotate [attr.interval]="99" [attr.class]="classes">one<!-- split -->two</slot-rotate>"#,
    ) {
        assert!(html.contains("interval=\"99\""), "{html}");
        assert!(html.contains("data-slot-interval=\"3200\""), "{html}");
        assert!(
            html.contains("class=\"crepus-slot literal-{name} flex\""),
            "{html}"
        );
    }
}

use crepuscularity_core::parse_template_with_path;
use std::path::Path;

fn direct_variants(source: &str) -> Vec<String> {
    let mut ctx = TemplateContext::new();
    ctx.set("classes", "literal-{name} flex");
    ctx.set("name", "CHANGED");
    let nodes = parse_template_with_path(source, Some(Path::new("Entry.ng"))).unwrap();
    #[allow(unused_mut)]
    let mut output = vec![crepuscularity_web::render_nodes_to_html(&nodes, &ctx).unwrap()];
    #[cfg(feature = "ssr")]
    output.push(
        crepuscularity_web::render_nodes_ssr(
            &nodes,
            &ctx,
            &std::cell::Cell::new(0),
            &mut Default::default(),
            true,
        )
        .unwrap(),
    );
    output
}

#[test]
fn attributes_preserve_nullable_string_presence_and_literal_class_values() {
    for html in direct_variants(
        r#"<input [attr.checked]="false" [disabled]="false" [attr.title]="null"/><div [attr.hidden]="false" [attr.class]="classes" [attr.id]="null">visible</div>"#,
    ) {
        assert!(html.contains("checked=\"false\""), "{html}");
        assert!(html.contains("hidden=\"false\""), "{html}");
        assert!(html.contains("class=\"literal-{name} flex\""), "{html}");
        assert!(!html.contains(" disabled="), "{html}");
        assert!(!html.contains(" title="), "{html}");
        assert!(!html.contains(" id="), "{html}");
    }
}

#[test]
fn attribute_interval_does_not_configure_rotation_or_duplicate_required_class() {
    for html in direct_variants(
        r#"<slot-rotate [attr.interval]="99" [attr.class]="classes">one<!-- split -->two</slot-rotate>"#,
    ) {
        assert!(html.contains("interval=\"99\""), "{html}");
        assert!(html.contains("data-slot-interval=\"3200\""), "{html}");
        assert!(
            html.contains("class=\"crepus-slot literal-{name} flex\""),
            "{html}"
        );
        assert_eq!(html.matches(" class=").count(), 1, "{html}");
    }
}

#[test]
fn mixed_case_attribute_classes_match_html_semantics_without_rewriting_svg_names() {
    for html in direct_variants(
        r#"<slot-rotate [attr.ClAsS]="classes">one<!-- split -->two</slot-rotate><svg [attr.viewBox]="'0 0 10 10'"></svg>"#,
    ) {
        assert!(
            html.contains("class=\"crepus-slot literal-{name} flex\""),
            "{html}"
        );
        assert!(!html.contains(" ClAsS="), "{html}");
        assert!(html.contains("viewBox=\"0 0 10 10\""), "{html}");
    }
}

#[test]
fn mixed_case_url_attributes_keep_the_same_safety_policy() {
    for html in direct_variants(
        r#"<a [attr.HREF]="'javascript:alert(1)'">unsafe</a><a [attr.HREF]="'data:text/html,bad'">unsafe data</a><button [attr.FORMAction]="'data:text/html,bad'">unsafe</button><img [attr.SRC]="'data:image/png;base64,AA=='"/><a [attr.HREF]="'https://example.test/path'">safe</a>"#,
    ) {
        assert!(!html.contains("javascript:"), "{html}");
        assert!(!html.contains("data:text/html"), "{html}");
        assert!(!html.contains(" FORMAction="), "{html}");
        assert!(
            html.contains("SRC=\"data:image/png;base64,AA==\""),
            "{html}"
        );
        assert!(
            html.contains("HREF=\"https://example.test/path\""),
            "{html}"
        );
    }
}
