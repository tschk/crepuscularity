use crepuscularity_core::{context::TemplateContext, parse_template_with_path};
use crepuscularity_web::render_nodes_to_html;
use std::path::Path;

#[test]
fn jsx_declaration_defaults_preserve_explicit_values() {
    let nodes = parse_template_with_path(
        "<let-default name=\"count\" value={3} /><span>{count}</span>",
        Some(Path::new("view.jsx")),
    )
    .unwrap();
    for (explicit, expected) in [(None, "3"), (Some(7_i64), "7")] {
        let mut ctx = TemplateContext::new();
        if let Some(value) = explicit {
            ctx.set("count", value);
        }
        let html = render_nodes_to_html(&nodes, &ctx).unwrap();
        assert!(html.contains(&format!(">{expected}</span>")), "{html}");
    }
}

#[test]
fn literal_attribute_backslashes_survive_html_rendering() {
    for (filename, source) in [
        (
            "Path.vue",
            r#"<template><input placeholder="C:\new\tools" /></template>"#,
        ),
        ("Path.ng.html", r#"<input placeholder="C:\new\tools" />"#),
        ("Path.jsx", r#"<input placeholder="C:\new\tools" />"#),
        ("Path.svelte", r#"<input placeholder="C:\new\tools" />"#),
        ("Path.astro", r#"<input placeholder="C:\new\tools" />"#),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        let html = render_nodes_to_html(&nodes, &TemplateContext::new()).unwrap();
        assert!(
            html.contains(r#"placeholder="C:\new\tools""#),
            "{filename}: {html}"
        );
    }
}

#[test]
fn moustache_delimiters_and_unicode_render_as_escaped_html_text() {
    for (filename, source) in [
        (
            "Text.vue",
            "<template><p>雪 {{ '}}' }} and {{ '<' }} after</p></template>",
        ),
        ("Text.ng.html", "<p>雪 {{ '}}' }} and {{ '<' }} after</p>"),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        let html = render_nodes_to_html(&nodes, &TemplateContext::new()).unwrap();
        assert!(html.contains("雪 }} and &lt; after"), "{filename}: {html}");
    }
}

#[test]
fn astro_comparisons_inside_text_keep_spaces_in_html() {
    let mut ctx = TemplateContext::new();
    ctx.set("count", 1);
    ctx.set("limit", 2);
    let nodes = parse_template_with_path(
        "<p>Before {count < limit} after</p>",
        Some(Path::new("Compare.astro")),
    )
    .unwrap();
    let html = render_nodes_to_html(&nodes, &ctx).unwrap();
    assert!(html.contains("Before true after"), "{html}");
}

#[test]
fn quoted_delimiters_render_as_text_and_html_characters_are_escaped() {
    for (filename, source) in [
        ("Text.crepus", "span\n  \"Brace {'}'} after {'{'}\""),
        ("Inline.crepus", "span \"Brace {'}'} after {'{'}\""),
        ("Text.jsx", "<p>Brace {'}'} after {'{'} and {'<'}</p>"),
        ("Text.svelte", "<p>Brace {'}'} after {'{'} and {'<'}</p>"),
        ("Text.astro", "<p>Brace {'}'} after {'{'} and {'<'}</p>"),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        let html = render_nodes_to_html(&nodes, &TemplateContext::new()).unwrap();
        assert!(html.contains("Brace } after {"), "{filename}: {html}");
        if !filename.ends_with(".crepus") {
            assert!(html.contains("and &lt;"), "{filename}: {html}");
        }
    }
}

#[test]
fn jsx_loop_aliases_render_every_item_in_order() {
    let mut ctx = TemplateContext::new();
    let items = ["First", "雪", "Last"]
        .into_iter()
        .map(|value| {
            let mut item = TemplateContext::new();
            item.set("value", value);
            item
        })
        .collect::<Vec<_>>();
    ctx.set("items", crepuscularity_core::TemplateValue::List(items));
    for variable in ["let", "var"] {
        let source = format!("<for {variable}=\"item\" in={{items}}><span>{{item}}</span></for>");
        let nodes = parse_template_with_path(&source, Some(Path::new("Loop.jsx"))).unwrap();
        let html = render_nodes_to_html(&nodes, &ctx).unwrap();
        assert_eq!(html, "<span>First</span><span>雪</span><span>Last</span>");
    }
}

#[test]
fn jsx_case_aliases_select_matching_and_wildcard_branches() {
    for subject in ["on", "value"] {
        for pattern in ["pattern", "match", "when"] {
            let source = format!("<match {subject}={{choice}}><case {pattern}=\"1\"><span>Selected</span></case><case><span>Fallback</span></case></match>");
            let nodes = parse_template_with_path(&source, Some(Path::new("Match.jsx"))).unwrap();
            for (choice, expected) in [(1, "Selected"), (2, "Fallback")] {
                let mut ctx = TemplateContext::new();
                ctx.set("choice", choice);
                let html = render_nodes_to_html(&nodes, &ctx).unwrap();
                assert_eq!(html, format!("<span>{expected}</span>"));
            }
        }
    }
}

#[test]
fn jsx_loop_binding_preserves_scalar_types_and_object_scopes() {
    use crepuscularity_core::TemplateValue;
    for (value, expression, expected) in [
        (Some(TemplateValue::Int(2)), "item + 1", "3"),
        (Some(TemplateValue::Bool(false)), "item == false", "true"),
        (
            Some(TemplateValue::Str(String::new())),
            "item == ''",
            "true",
        ),
        (Some(TemplateValue::Null), "item == null", "true"),
        (None, "item.name", "雪"),
    ] {
        let mut item = TemplateContext::new();
        item.set("name", "雪");
        if let Some(value) = value {
            item.set("value", value);
        }
        let mut ctx = TemplateContext::new();
        ctx.set("items", TemplateValue::List(vec![item]));
        let source = format!("<for let=\"item\" in={{items}}><span>{{{expression}}}</span></for>");
        let nodes = parse_template_with_path(&source, Some(Path::new("TypedLoop.jsx"))).unwrap();
        let html = render_nodes_to_html(&nodes, &ctx).unwrap();
        assert_eq!(html, format!("<span>{expected}</span>"), "{expression}");
    }
}

#[test]
fn vue_content_directives_replace_children_with_safe_html_output() {
    let source = r#"<template><div v-text="content">Discarded text</div><section v-html="content">Discarded HTML</section></template>"#;
    let nodes = parse_template_with_path(source, Some(Path::new("Content.vue"))).unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("content", "<b>雪</b><script>unsafe()</script>");
    let html = render_nodes_to_html(&nodes, &ctx).unwrap();
    assert!(
        html.contains("<div>&lt;b&gt;雪&lt;/b&gt;&lt;script&gt;unsafe()&lt;/script&gt;</div>"),
        "{html}"
    );
    assert!(html.contains("<section><b>雪</b></section>"), "{html}");
    assert!(!html.contains("Discarded"), "{html}");
    assert!(!html.contains("<script>"), "{html}");
}

#[test]
fn vue_conditional_chain_renders_exactly_the_selected_branch() {
    let source = r#"<template><div v-if="score > 10">High</div><div v-else-if="score > 0">Positive</div><div v-else>Fallback</div></template>"#;
    let nodes = parse_template_with_path(source, Some(Path::new("Branches.vue"))).unwrap();
    for (score, expected) in [
        (11, "High"),
        (1, "Positive"),
        (0, "Fallback"),
        (-1, "Fallback"),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("score", score);
        let html = render_nodes_to_html(&nodes, &ctx).unwrap();
        assert_eq!(html, format!("<div>{expected}</div>"), "score={score}");
    }
}

#[test]
fn angular_content_bindings_replace_children_and_escape_or_sanitize_html() {
    let mut ctx = TemplateContext::new();
    ctx.set("content", "<b>雪</b><script>unsafe()</script>");
    for (property, expected) in [
        ("innerHTML", "<div><b>雪</b></div>"),
        (
            "innerText",
            "<div>&lt;b&gt;雪&lt;/b&gt;&lt;script&gt;unsafe()&lt;/script&gt;</div>",
        ),
        (
            "textContent",
            "<div>&lt;b&gt;雪&lt;/b&gt;&lt;script&gt;unsafe()&lt;/script&gt;</div>",
        ),
    ] {
        let source = format!("<div [{property}]=\"content\">Discarded</div>");
        let nodes = parse_template_with_path(&source, Some(Path::new("Content.ng.html"))).unwrap();
        let html = render_nodes_to_html(&nodes, &ctx).unwrap();
        assert_eq!(html, expected, "{property}");
    }
}

#[test]
fn angular_loops_preserve_numeric_bindings_and_filter_conditional_children() {
    let mut ctx = TemplateContext::new();
    let items = [0, 2, -1, 3]
        .into_iter()
        .map(|value| {
            let mut item = TemplateContext::new();
            item.set("value", value);
            item
        })
        .collect::<Vec<_>>();
    ctx.set("items", crepuscularity_core::TemplateValue::List(items));
    for source in [
        r#"<section *ngFor="let item of items"><span *ngIf="item > 0">{{ item + 1 }}</span></section>"#,
        "@for (item of items; track item) { <section>@if (item > 0) { <span>{{ item + 1 }}</span> }</section> }",
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new("Loops.ng.html"))).unwrap();
        let html = render_nodes_to_html(&nodes, &ctx).unwrap();
        assert_eq!(html, "<section></section><section><span>3</span></section><section></section><section><span>4</span></section>");
    }
}

#[test]
fn svelte_loops_preserve_numeric_values_and_conditional_classes() {
    let mut ctx = TemplateContext::new();
    let items = [0, 2]
        .into_iter()
        .map(|value| {
            let mut item = TemplateContext::new();
            item.set("value", value);
            item
        })
        .collect::<Vec<_>>();
    ctx.set("items", crepuscularity_core::TemplateValue::List(items));
    let nodes = parse_template_with_path(
        "{#each items as item}<span class:active={item > 0}>{item + 1}</span>{/each}",
        Some(Path::new("Loop.svelte")),
    )
    .unwrap();
    let html = render_nodes_to_html(&nodes, &ctx).unwrap();
    assert_eq!(html, "<span>1</span><span class=\"active\">3</span>");
}

#[test]
fn jsx_conditional_class_forms_render_only_for_true_conditions() {
    let nodes = parse_template_with_path(
        "<div class=\"base\" when:active=\"one two\" class:selected={active}>Content</div>",
        Some(Path::new("Classes.jsx")),
    )
    .unwrap();
    for (active, expected_classes) in [(false, "base"), (true, "base one two selected")] {
        let mut ctx = TemplateContext::new();
        ctx.set("active", active);
        let html = render_nodes_to_html(&nodes, &ctx).unwrap();
        assert_eq!(
            html,
            format!("<div class=\"{expected_classes}\">Content</div>")
        );
    }
}

#[test]
fn quoted_and_braced_animations_emit_identical_html_annotations() {
    for value in ["\"300ms ease-in-out\"", "{300ms ease-in-out}"] {
        let source = format!("<div animate:opacity={value}>Content</div>");
        let nodes = parse_template_with_path(&source, Some(Path::new("Animation.jsx"))).unwrap();
        let html = render_nodes_to_html(&nodes, &TemplateContext::new()).unwrap();
        assert_eq!(
            html,
            "<div data-animate-opacity=\"300ms ease-in-out\">Content</div>"
        );
    }
}

#[test]
fn astro_content_directives_escape_or_sanitize_element_and_fragment_output() {
    let mut ctx = TemplateContext::new();
    ctx.set("content", "<b>雪</b><script>unsafe()</script>");
    for tag in ["div", "Fragment"] {
        for (directive, content) in [
            ("set:html", "<b>雪</b>"),
            (
                "set:text",
                "&lt;b&gt;雪&lt;/b&gt;&lt;script&gt;unsafe()&lt;/script&gt;",
            ),
        ] {
            let source = format!("<{tag} {directive}={{content}}>Discarded</{tag}>");
            let nodes =
                parse_template_with_path(&source, Some(Path::new("Content.astro"))).unwrap();
            let html = render_nodes_to_html(&nodes, &ctx).unwrap();
            let expected = if tag == "Fragment" {
                content.to_string()
            } else {
                format!("<div>{content}</div>")
            };
            assert_eq!(html, expected, "{tag}/{directive}");
        }
    }
}

#[test]
fn astro_class_lists_render_static_and_conditional_entries() {
    let nodes = parse_template_with_path(
        r#"<div class="base" class:list={["one", { active: enabled, "two three": selected }]} />"#,
        Some(Path::new("Classes.astro")),
    )
    .unwrap();
    for (enabled, selected, expected) in [
        (false, false, "base one"),
        (true, false, "base one active"),
        (false, true, "base one two three"),
        (true, true, "base one active two three"),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("enabled", enabled);
        ctx.set("selected", selected);
        let html = render_nodes_to_html(&nodes, &ctx).unwrap();
        assert_eq!(html, format!("<div class=\"{expected}\"></div>"));
    }
}

#[test]
fn class_list_string_escapes_render_consistently_across_frontends() {
    for (filename, source) in [
        (
            "Classes.vue",
            r#"<template><div :class={['one\ttwo', 'snow\',雪']} /></template>"#,
        ),
        (
            "Classes.ng.html",
            r#"<div [ngClass]={['one\ttwo', 'snow\',雪']} />"#,
        ),
        (
            "Classes.astro",
            r#"<div class:list={['one\ttwo', 'snow\',雪']} />"#,
        ),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        let html = render_nodes_to_html(&nodes, &TemplateContext::new()).unwrap();
        assert_eq!(
            html, "<div class=\"one two snow&#39;,雪\"></div>",
            "{filename}"
        );
    }
}

#[test]
fn class_object_keys_and_quoted_conditions_render_across_frontends() {
    for (filename, source) in [
        (
            "Keys.vue",
            r#"<template><div :class={{ 'one\ttwo': active, 'hover:bg-red': state == 'a:b' }} /></template>"#,
        ),
        (
            "Keys.ng.html",
            r#"<div [ngClass]={{ 'one\ttwo': active, 'hover:bg-red': state == 'a:b' }} />"#,
        ),
        (
            "Keys.astro",
            r#"<div class:list={{ 'one\ttwo': active, 'hover:bg-red': state == 'a:b' }} />"#,
        ),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        for (active, state, expected) in [
            (true, "a:b", "<div class=\"one two hover:bg-red\"></div>"),
            (false, "other", "<div></div>"),
        ] {
            let mut ctx = TemplateContext::new();
            ctx.set("active", active);
            ctx.set("state", state);
            let html = render_nodes_to_html(&nodes, &ctx).unwrap();
            assert_eq!(html, expected, "{filename}");
        }
    }
}

#[test]
fn nested_class_arrays_flatten_across_frontends() {
    for (filename, source) in [
        (
            "Nested.vue",
            r#"<template><div :class={['one', ['two', { active: enabled }]]} /></template>"#,
        ),
        (
            "Nested.ng.html",
            r#"<div [ngClass]={['one', ['two', { active: enabled }]]} />"#,
        ),
        (
            "Nested.astro",
            r#"<div class:list={['one', ['two', { active: enabled }]]} />"#,
        ),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        for (enabled, expected) in [(false, "one two"), (true, "one two active")] {
            let mut ctx = TemplateContext::new();
            ctx.set("enabled", enabled);
            let html = render_nodes_to_html(&nodes, &ctx).unwrap();
            assert_eq!(
                html,
                format!("<div class=\"{expected}\"></div>"),
                "{filename}"
            );
        }
    }
}

#[test]
fn false_and_null_class_literals_are_omitted_across_frontends() {
    for (filename, source) in [
        (
            "Empty.vue",
            r#"<template><div :class={[false, null, ['false', 'null']]} /></template>"#,
        ),
        (
            "Empty.ng.html",
            r#"<div [ngClass]={[false, null, ['false', 'null']]} />"#,
        ),
        (
            "Empty.astro",
            r#"<div class:list={[false, null, ['false', 'null']]} />"#,
        ),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        let html = render_nodes_to_html(&nodes, &TemplateContext::new()).unwrap();
        assert_eq!(html, "<div class=\"false null\"></div>", "{filename}");
    }
}

#[test]
fn dynamic_false_and_null_class_values_are_omitted_without_changing_literals() {
    let nodes = parse_template_with_path(
        r#"<div class:list={['base', dynamic]} />"#,
        Some(Path::new("Dynamic.astro")),
    )
    .unwrap();
    for (value, expected) in [
        (crepuscularity_core::TemplateValue::Bool(false), "base"),
        (crepuscularity_core::TemplateValue::Null, "base"),
        (
            crepuscularity_core::TemplateValue::Str("false".into()),
            "base false",
        ),
        (
            crepuscularity_core::TemplateValue::Str("null".into()),
            "base null",
        ),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("dynamic", value);
        assert_eq!(
            render_nodes_to_html(&nodes, &ctx).unwrap(),
            format!("<div class=\"{expected}\"></div>")
        );
    }
}

#[test]
fn empty_dynamic_classes_do_not_emit_attributes_or_separators() {
    for (source, expected) in [
        (r#"<div class:list={dynamic} />"#, "<div></div>"),
        (
            r#"<div class:list={[dynamic, 'base', dynamic]} />"#,
            "<div class=\"base\"></div>",
        ),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new("Empty.astro"))).unwrap();
        for value in ["", "   "] {
            let mut ctx = TemplateContext::new();
            ctx.set("dynamic", value);
            assert_eq!(render_nodes_to_html(&nodes, &ctx).unwrap(), expected);
        }
    }
}

#[test]
fn dynamic_multi_class_values_filter_target_prefixes_per_token() {
    let nodes = parse_template_with_path(
        r#"<div class:list={dynamic} />"#,
        Some(Path::new("Targets.astro")),
    )
    .unwrap();
    for (value, expected) in [
        (
            "web:p-4 tui:hidden hover:text-red-500",
            "<div class=\"p-4 hover:text-red-500\"></div>",
        ),
        ("tui:hidden web:p-4", "<div class=\"p-4\"></div>"),
        ("web: tui:hidden", "<div></div>"),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("dynamic", value);
        assert_eq!(render_nodes_to_html(&nodes, &ctx).unwrap(), expected);
    }
}

#[test]
fn false_boolean_attributes_are_omitted_but_aria_values_are_preserved() {
    let source = r#"<input disabled={enabled} checked={enabled} aria-disabled={enabled} />"#;
    let nodes = parse_template_with_path(source, Some(Path::new("Boolean.jsx"))).unwrap();
    for (enabled, expected) in [
        (false, "<input aria-disabled=\"false\" />"),
        (
            true,
            "<input disabled=\"\" checked=\"\" aria-disabled=\"true\" />",
        ),
    ] {
        let mut ctx = TemplateContext::new();
        ctx.set("enabled", enabled);
        assert_eq!(render_nodes_to_html(&nodes, &ctx).unwrap(), expected);
    }
    let nodes = parse_template_with_path(
        r#"<input disabled="false" />"#,
        Some(Path::new("Boolean.jsx")),
    )
    .unwrap();
    assert_eq!(
        render_nodes_to_html(&nodes, &TemplateContext::new()).unwrap(),
        "<input disabled=\"false\" />"
    );
}

#[test]
fn jsx_dynamic_class_attributes_share_filtering_and_empty_value_semantics() {
    for attribute in ["class", "className"] {
        let source = format!("<div {attribute}={{theme}}>Content</div>");
        let nodes = parse_template_with_path(&source, Some(Path::new("Theme.jsx"))).unwrap();
        for (theme, expected) in [
            (
                crepuscularity_core::TemplateValue::Bool(false),
                "<div>Content</div>",
            ),
            (
                crepuscularity_core::TemplateValue::Null,
                "<div>Content</div>",
            ),
            (
                crepuscularity_core::TemplateValue::Str(
                    "tui:hidden web:p-4 hover:text-red-500".into(),
                ),
                "<div class=\"p-4 hover:text-red-500\">Content</div>",
            ),
        ] {
            let mut ctx = TemplateContext::new();
            ctx.set("theme", theme);
            assert_eq!(
                render_nodes_to_html(&nodes, &ctx).unwrap(),
                expected,
                "{attribute}"
            );
        }
    }
}

#[test]
fn jsx_dynamic_ids_render_and_literal_backslashes_are_preserved() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "entry&anchor");
    for (source, expected) in [
        (
            r#"<div id={name} />"#,
            "<div id=\"entry&amp;anchor\"></div>",
        ),
        (r#"<div id="path\name" />"#, r#"<div id="path\name"></div>"#),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new("Ids.jsx"))).unwrap();
        assert_eq!(render_nodes_to_html(&nodes, &ctx).unwrap(), expected);
    }
}

#[test]
fn shared_markup_frontends_render_dynamic_ids() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "entry&anchor");
    for filename in ["Ids.jsx", "Ids.svelte", "Ids.astro"] {
        let nodes =
            parse_template_with_path("<div id={name}>Content</div>", Some(Path::new(filename)))
                .unwrap();
        assert_eq!(
            render_nodes_to_html(&nodes, &ctx).unwrap(),
            "<div id=\"entry&amp;anchor\">Content</div>",
            "{filename}"
        );
    }
}

#[test]
fn quoted_id_expressions_with_operators_are_evaluated() {
    for (source, expected) in [
        (
            r#"<div id={"entry" + "-anchor"} />"#,
            "<div id=\"entry-anchor\"></div>",
        ),
        (r#"<div id={"entry"} />"#, "<div id=\"entry\"></div>"),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new("Ids.jsx"))).unwrap();
        assert_eq!(
            render_nodes_to_html(&nodes, &TemplateContext::new()).unwrap(),
            expected
        );
    }
}

#[test]
fn angular_interpolated_ids_render_evaluated_and_escaped_values() {
    let nodes =
        parse_template_with_path(r#"<div id="{{ name }}" />"#, Some(Path::new("Ids.ng.html")))
            .unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("name", "entry&anchor");
    assert_eq!(
        render_nodes_to_html(&nodes, &ctx).unwrap(),
        "<div id=\"entry&amp;anchor\"></div>"
    );
}

#[test]
fn angular_interpolated_classes_evaluate_and_filter_targets() {
    let nodes = parse_template_with_path(
        r#"<div class="{{ theme }}" />"#,
        Some(Path::new("Classes.ng.html")),
    )
    .unwrap();
    let mut ctx = TemplateContext::new();
    ctx.set("theme", "web:p-4 tui:hidden active");
    assert_eq!(
        render_nodes_to_html(&nodes, &ctx).unwrap(),
        "<div class=\"p-4 active\"></div>"
    );
}

#[test]
fn vue_and_angular_id_forms_render_the_same_escaped_value() {
    let mut ctx = TemplateContext::new();
    ctx.set("name", "entry&anchor");
    for (filename, source) in [
        ("Ids.vue", "<template><div id={name} /></template>"),
        ("Ids.vue", r#"<template><div :id="name" /></template>"#),
        (
            "Ids.vue",
            r#"<template><div v-bind:id="name" /></template>"#,
        ),
        ("Ids.ng.html", "<div id={name} />"),
        ("Ids.ng.html", r#"<div [id]="name" />"#),
        ("Ids.ng.html", r#"<div [attr.id]="name" />"#),
        ("Ids.ng.html", r#"<div id="{{ name }}" />"#),
    ] {
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        assert_eq!(
            render_nodes_to_html(&nodes, &ctx).unwrap(),
            "<div id=\"entry&amp;anchor\"></div>",
            "{filename}: {source}"
        );
    }
}

#[test]
fn astro_comment_delimiters_do_not_consume_surrounding_text() {
    let source = "<div>Before{/* } { \" ' 雪 */}Middle{// } \" ' 雪\n}After</div>";
    let nodes = parse_template_with_path(source, Some(Path::new("Comments.astro"))).unwrap();
    let html = render_nodes_to_html(&nodes, &TemplateContext::new()).unwrap();
    assert_eq!(html, "<div>BeforeMiddleAfter</div>");
}
