use crepuscularity_core::TemplateContext;
use crepuscularity_web::render_template_to_html;

fn render_variants(
    template: &str,
    ctx: &TemplateContext,
) -> Vec<Result<String, crepuscularity_web::CrepusError>> {
    let rendered = std::iter::once(render_template_to_html(template, ctx));
    #[cfg(feature = "ssr")]
    let rendered = rendered.chain([false, true].map(|markers| {
        crepuscularity_web::render_template_to_html_with_ssr(template, ctx, markers)
    }));
    rendered.collect()
}

#[test]
fn renders_id_shorthand_and_same_line_text() {
    let tpl = r#"
div #hero "Hi"
  span
    "there"
"#;
    let html = render_template_to_html(tpl, &TemplateContext::new()).unwrap();
    assert!(
        html.contains(r#"<div id="hero">Hi<span>there</span></div>"#),
        "{html}"
    );
}

#[test]
fn preserves_same_line_text_before_indented_children() {
    let tpl = r#"
div #hero "Hi"
  span
    "there"
"#;
    let html = render_template_to_html(tpl, &TemplateContext::new()).unwrap();
    let hi_ix = html.find("Hi").unwrap();
    let span_ix = html.find("<span>").unwrap();
    assert!(hi_ix < span_ix, "{html}");
}

#[test]
fn quoted_event_handlers_render_without_quotes() {
    let tpl = r#"
button #status @click="on_refresh_status"
  "Click"
"#;
    for source in [
        tpl,
        r#"<button @click="on_refresh_status">Click</button>"#,
        r#"<button onClick="on_refresh_status">Click</button>"#,
    ] {
        let html = render_template_to_html(source, &TemplateContext::new()).unwrap();
        assert!(
            html.contains(r#"data-onclick="on_refresh_status""#),
            "{html}"
        );
    }
}

#[test]
fn event_modifiers_survive_html_and_ssr_output() {
    for source in [
        r#"<button @click|prevent|stop|self|once|capture={save} @keydown|ctrl+s|Esc={save}>Save</button>"#,
        "button @click|prevent|stop|self|once|capture=save @keydown|ctrl+s|Esc=save",
    ] {
        for rendered in render_variants(source, &TemplateContext::new()) {
            let html = rendered.unwrap();
            assert!(html.contains(r#"data-crepus-modifiers-click="[&quot;prevent&quot;,&quot;stop&quot;,&quot;self&quot;,&quot;once&quot;,&quot;capture&quot;]""#), "{html}");
            assert!(
                html.contains(
                    r#"data-crepus-modifiers-keydown="[&quot;ctrl+s&quot;,&quot;Esc&quot;]""#
                ),
                "{html}"
            );
        }
        #[cfg(feature = "hydration")]
        {
            let hydrated = crepuscularity_web::render_template_to_html_with_hydration(
                source,
                &TemplateContext::new(),
            )
            .unwrap();
            assert!(
                hydrated.contains("data-crepus-modifiers-keydown="),
                "{hydrated}"
            );
        }
    }
    let source =
        "<template><button @click.prevent.stop.self.once.capture=\"save\">Save</button></template>";
    let files = std::collections::HashMap::from([("Actions.vue".to_string(), source.to_string())]);
    let html =
        crepuscularity_web::render_from_files(&files, "Actions.vue", &TemplateContext::new())
            .unwrap();
    assert!(html.contains("data-crepus-modifiers-click="), "{html}");
}

#[test]
fn react_capture_and_double_click_aliases_emit_native_event_bindings() {
    let html = render_template_to_html(
        r#"<button onClickCapture={save} onDoubleClick={edit}>Press</button>"#,
        &TemplateContext::new(),
    )
    .unwrap();
    assert!(html.contains(r#"data-onclick="save""#), "{html}");
    assert!(
        html.contains(r#"data-crepus-modifiers-click="[&quot;capture&quot;]""#),
        "{html}"
    );
    assert!(html.contains(r#"data-ondblclick="edit""#), "{html}");
    assert!(!html.contains("data-onclickcapture"), "{html}");
}

#[test]
fn unsupported_web_modifiers_fail_before_emitting_nonfunctional_handlers() {
    for source in [
        "button @click|unknown=save",
        "button @keydown|ctl+s=save",
        "button @keydown|ctrl++s=save",
        "button @keyup|ctrl+=save",
    ] {
        let error = render_template_to_html(source, &TemplateContext::new()).unwrap_err();
        assert!(
            error.to_string().contains("Unsupported web event modifier"),
            "{error}"
        );
        #[cfg(feature = "ssr")]
        assert!(crepuscularity_web::render_template_to_html_with_ssr(
            source,
            &TemplateContext::new(),
            true
        )
        .is_err());
        #[cfg(feature = "hydration")]
        assert!(crepuscularity_web::render_template_to_html_with_hydration(
            source,
            &TemplateContext::new()
        )
        .is_err());
    }
}

#[test]
fn renders_embed_island_placeholder_with_props() {
    let mut ctx = TemplateContext::new();
    ctx.set("n", 7_i32);
    let tpl = r#"embed ./islands/wave.ts adapter="module" title="Wave" count={n}"#;
    let html = render_template_to_html(tpl, &ctx).unwrap();
    assert!(html.contains(r#"data-crepus-island="""#), "{html}");
    assert!(
        html.contains(r#"data-crepus-island-src="./islands/wave.ts""#),
        "{html}"
    );
    assert!(
        html.contains(r#"data-crepus-island-adapter="module""#),
        "{html}"
    );
    assert!(
        html.contains("&quot;title&quot;:&quot;Wave&quot;"),
        "{html}"
    );
    assert!(html.contains("&quot;count&quot;:7"), "{html}");
}

#[test]
fn indentation_id_expressions_evaluate_while_quoted_braces_stay_literal() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "entry");
    let html = render_template_to_html("div id={name}", &ctx).unwrap();
    assert_eq!(html, "<div id=\"entry\"></div>");
    let html = render_template_to_html(r#"div id="{name}""#, &ctx).unwrap();
    assert_eq!(html, "<div id=\"{name}\"></div>");
    assert!(render_template_to_html("div id={}", &ctx).is_err());
}

#[test]
fn indentation_class_expressions_preserve_internal_whitespace() {
    let mut ctx = TemplateContext::new();
    ctx.set("classes", "border p-2");
    let html = render_template_to_html("div class={ classes }", &ctx).unwrap();
    assert_eq!(html, "<div class=\"border p-2\"></div>");
    assert!(render_template_to_html("div class={}", &ctx).is_err());
}

#[test]
fn standalone_indentation_expressions_accept_string_literals() {
    let ctx = TemplateContext::new();
    let html = render_template_to_html(r#"{ "<b>Text</b>" }"#, &ctx).unwrap();
    assert_eq!(html, "&lt;b&gt;Text&lt;/b&gt;");
    let html = render_template_to_html(r#"{= "<b>Text</b>" }"#, &ctx).unwrap();
    assert_eq!(html, "<b>Text</b>");
}

#[test]
fn indentation_control_conditions_preserve_quoted_closing_braces() {
    let mut ctx = TemplateContext::new();
    ctx.set("label", "}");
    let source = "if {label == \"}\"}\n  div\n    \"Matched\"\nelse\n  div\n    \"Missed\"";
    let html = render_template_to_html(source, &ctx).unwrap();
    assert_eq!(html, "<div>Matched</div>");
}

#[test]
fn attribute_child_directives_reach_html_output() {
    let source = "div\n  class:hidden={enabled}\n  @click=save\n  animate:opacity={300ms linear repeat}\n  \"Content\"";
    let mut ctx = TemplateContext::new();
    ctx.set("enabled", true);
    let html = render_template_to_html(source, &ctx).unwrap();
    assert!(html.contains("class=\"hidden\""), "{html}");
    assert!(html.contains("data-onclick=\"save\""), "{html}");
    assert!(
        html.contains("data-animate-opacity=\"300ms linear repeat\""),
        "{html}"
    );
    assert!(html.ends_with(">Content</div>"), "{html}");
    ctx.set("enabled", false);
    let html = render_template_to_html(source, &ctx).unwrap();
    assert!(!html.contains("class=\"hidden\""), "{html}");
}

#[test]
fn multiple_attributes_on_a_child_line_render_on_the_parent() {
    let source = "div\n  id={name} class=\"border\" title=\"Title\" @click=save\n  \"Content\"";
    let mut ctx = TemplateContext::new();
    ctx.set("name", "entry");
    let html = render_template_to_html(source, &ctx).unwrap();
    assert!(
        html.starts_with("<div ") && html.ends_with(">Content</div>"),
        "{html}"
    );
    for attribute in [
        r#"id="entry""#,
        r#"class="border""#,
        r#"title="Title""#,
        r#"data-onclick="save""#,
    ] {
        assert!(html.contains(attribute), "{html}");
    }
}

#[test]
fn svelte_native_event_modifiers_render_with_shared_semantics() {
    let source = "<form on:submit|preventDefault|stopPropagation|self|once|capture={save}></form>";
    let files =
        std::collections::HashMap::from([("Actions.svelte".to_string(), source.to_string())]);
    let html =
        crepuscularity_web::render_from_files(&files, "Actions.svelte", &TemplateContext::new())
            .unwrap();
    assert!(html.contains("data-onsubmit=\"save\""), "{html}");
    assert!(html.contains("[&quot;prevent&quot;,&quot;stop&quot;,&quot;self&quot;,&quot;once&quot;,&quot;capture&quot;]"), "{html}");
}

#[test]
fn multiple_handlers_emit_one_legacy_attribute_and_preserve_all_listeners() {
    for template in [
        "<button @click={first} @click|capture={second} />",
        "<button onClick={first} onClickCapture={second} />",
    ] {
        for rendered in render_variants(template, &TemplateContext::new()) {
            let html = rendered.unwrap();
            assert_eq!(html.matches("data-onclick=").count(), 1, "{html}");
            assert!(html.contains("data-crepus-handlers-click="), "{html}");
            assert!(html.contains("first") && html.contains("second"), "{html}");
        }
    }
    let files = std::collections::HashMap::from([(
        "Multi.svelte".to_string(),
        "<button on:click={first} on:click={second} />".to_string(),
    )]);
    let html =
        crepuscularity_web::render_from_files(&files, "Multi.svelte", &TemplateContext::new())
            .unwrap();
    assert!(html.contains("data-crepus-handlers-click="), "{html}");
    assert!(render_template_to_html(
        "<button @click={first} @click|unsupported={second} />",
        &TemplateContext::new()
    )
    .is_err());
}

#[test]
fn svelte_immediate_stop_modifier_renders() {
    let files = std::collections::HashMap::from([(
        "Immediate.svelte".to_string(),
        "<button on:click|stopImmediatePropagation={save} />".to_string(),
    )]);
    let html =
        crepuscularity_web::render_from_files(&files, "Immediate.svelte", &TemplateContext::new())
            .unwrap();
    assert!(html.contains("[&quot;immediate&quot;]"), "{html}");
}

#[test]
fn svelte_trusted_event_modifier_renders() {
    let files = std::collections::HashMap::from([(
        "Trusted.svelte".to_string(),
        "<button on:click|trusted={save} />".to_string(),
    )]);
    let html =
        crepuscularity_web::render_from_files(&files, "Trusted.svelte", &TemplateContext::new())
            .unwrap();
    assert!(html.contains("[&quot;trusted&quot;]"), "{html}");
}

#[test]
fn bound_textarea_values_render_as_escaped_child_text() {
    let mut ctx = TemplateContext::new();
    ctx.set("body", "<draft>& notes");
    let template = "<textarea value={body}>old</textarea>";
    for rendered in render_variants(template, &ctx) {
        let html = rendered.unwrap();
        assert!(!html.contains(" value="), "{html}");
        assert!(
            html.contains(">&lt;draft&gt;&amp; notes</textarea>"),
            "{html}"
        );
    }
    #[cfg(feature = "hydration")]
    {
        let html =
            crepuscularity_web::render_template_to_html_with_hydration(template, &ctx).unwrap();
        assert!(
            html.contains(">&lt;draft&gt;&amp; notes</textarea>"),
            "{html}"
        );
    }
}

#[test]
fn textarea_values_preserve_leading_newlines_and_empty_values() {
    let mut ctx = TemplateContext::new();
    ctx.set("body", "\nfirst");
    let html = render_template_to_html("<textarea value={body} />", &ctx).unwrap();
    assert_eq!(html, "<textarea>\n\nfirst</textarea>");
    ctx.set("body", "");
    assert_eq!(
        render_template_to_html("<textarea value={body}>old</textarea>", &ctx).unwrap(),
        "<textarea></textarea>"
    );
    assert_eq!(
        render_template_to_html("<textarea>old</textarea>", &ctx).unwrap(),
        "<textarea>old</textarea>"
    );
}

#[test]
fn textarea_child_interpolation_preserves_text_without_ssr_wrappers() {
    let mut ctx = TemplateContext::new();
    ctx.set("body", "\n<draft>& notes");
    let template = "<textarea>{body}</textarea>";
    for rendered in render_variants(template, &ctx) {
        let html = rendered.unwrap();
        assert!(
            html.contains(">\n\n&lt;draft&gt;&amp; notes</textarea>"),
            "{html}"
        );
        assert!(!html.contains("<span"), "{html}");
    }
    #[cfg(feature = "hydration")]
    {
        let html =
            crepuscularity_web::render_template_to_html_with_hydration(template, &ctx).unwrap();
        assert!(
            html.contains(">\n\n&lt;draft&gt;&amp; notes</textarea>"),
            "{html}"
        );
        assert!(!html.contains("<span"), "{html}");
    }
}

#[test]
fn textarea_child_includes_keep_the_recursion_limit() {
    let mut ctx = TemplateContext::new();
    std::sync::Arc::make_mut(&mut ctx.virtual_files).insert(
        "cycle.crepus".to_string(),
        "textarea\n  include cycle.crepus".to_string(),
    );
    for rendered in render_variants("include cycle.crepus", &ctx) {
        let err = rendered.unwrap_err();
        assert!(err.to_string().contains("maximum include depth"), "{err}");
    }
}

#[test]
fn textarea_values_and_children_preserve_carriage_returns() {
    for (body, expected) in [
        ("\r\nfirst\rsecond", "&#13;\nfirst&#13;second"),
        ("\n\rfirst", "\n\n&#13;first"),
        ("x\r\ny", "x&#13;\ny"),
        ("\r", "&#13;"),
        ("&#13;\r<&", "&amp;#13;&#13;&lt;&amp;"),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("body", body);
        for template in ["<textarea value={body}/>", "<textarea>{body}</textarea>"] {
            let rendered = render_variants(template, &ctx);
            #[cfg(feature = "hydration")]
            let rendered = rendered
                .into_iter()
                .chain(std::iter::once(
                    crepuscularity_web::render_template_to_html_with_hydration(template, &ctx),
                ))
                .collect::<Vec<_>>();
            for result in rendered {
                let html = result.unwrap();
                assert!(
                    html.contains(&format!(">{expected}</textarea>")),
                    "{template} body={body:?}: {html}"
                );
                assert!(!html.contains("<span"), "{html}");
            }
        }
    }
}

#[test]
fn form_value_attributes_preserve_carriage_returns() {
    for (value, expected) in [
        ("row\rkey", "row&#13;key"),
        ("row\r\nkey", "row&#13;\nkey"),
        ("&#13;\r<&\"", "&amp;#13;&#13;&lt;&amp;&quot;"),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("key", value);
        for template in [
            "<input type=\"hidden\" name=\"hidden\" value={key}/>",
            "<select name=\"choice\"><option value={key} selected={true}>One</option></select>",
        ] {
            let rendered = render_variants(template, &ctx);
            #[cfg(feature = "hydration")]
            let rendered = rendered
                .into_iter()
                .chain(std::iter::once(
                    crepuscularity_web::render_template_to_html_with_hydration(template, &ctx),
                ))
                .collect::<Vec<_>>();
            for result in rendered {
                let html = result.unwrap();
                assert!(
                    html.contains(&format!("value=\"{expected}\"")),
                    "{template} value={value:?}: {html}"
                );
                assert!(!html.contains('\r'), "raw CR in attribute HTML: {html}");
                if template.starts_with("<select") {
                    assert!(html.contains("selected=\"true\""), "{html}");
                }
            }
        }
    }
}

#[test]
fn jsx_id_and_title_string_expressions_share_escape_semantics() {
    for body in [r"a\rb", r"a\u0041b", r"a\qb", r"a\nb", r"a\tb", r"a\\b"] {
        let expected =
            crepuscularity_core::eval::eval_expr(&format!("\"{body}\""), &TemplateContext::new())
                .unwrap();
        let expected = crepuscularity_core::context::value_to_str(&expected);
        for quote in ['\'', '"'] {
            let source =
                format!("<div id={{{quote}{body}{quote}}} title={{{quote}{body}{quote}}}/>");
            for result in render_variants(&source, &TemplateContext::new()) {
                let html = result.unwrap();
                assert!(
                    html.contains(&format!("id=\"{expected}\"")),
                    "{source}: {html:?}"
                );
                assert!(
                    html.contains(&format!("title=\"{expected}\"")),
                    "{source}: {html:?}"
                );
            }
        }
    }
    let source = r#"<div id="a\rb"/>"#;
    for result in render_variants(source, &TemplateContext::new()) {
        assert!(result.unwrap().contains(r#"id="a\rb""#));
    }
}
