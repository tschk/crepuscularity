use std::path::Path;

use crate::ast::*;
use crate::parser::{parse_template_with_path, parse_vue_sfc};

fn parse(src: &str) -> Vec<Node> {
    parse_template_with_path(src, Some(Path::new("Comp.vue"))).expect("vue template should parse")
}

fn parse_body(body: &str) -> Vec<Node> {
    parse(&format!("<template>\n{body}\n</template>\n"))
}

fn element(node: &Node) -> &Element {
    match node {
        Node::Element(el) => el,
        other => panic!("expected element, got {other:?}"),
    }
}

fn text_of(node: &Node) -> String {
    match node {
        Node::Text(parts) => {
            let mut out = String::new();
            for p in parts {
                match p {
                    TextPart::Literal(s) => out.push_str(s),
                    TextPart::Expr(e) => {
                        out.push('{');
                        out.push_str(e);
                        out.push('}');
                    }
                }
            }
            out
        }
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn sfc_rejects_content_outside_supported_blocks() {
    for (source, bad) in [
        ("<template/><p>lost</p>", "<p>"),
        ("<p>lost</p><template/>", "<p>"),
        ("<template/>lost", "lost"),
        ("<script/>lost<template/>", "lost"),
        ("<template/><docs>lost</docs>", "<docs>"),
        ("<template/></template>", "</template>"),
        ("\u{feff}<template/>é", "é"),
    ] {
        let error = super::sfc::split_sfc(source).expect_err(source);
        assert!(error.message.contains("top-level"), "{source}: {error:?}");
        assert_eq!(error.byte_offset, source.find(bad), "{source}");
        assert!(parse_vue_sfc(source).is_err(), "{source}");
        assert!(
            parse_template_with_path(source, Some(Path::new("Comp.vue"))).is_err(),
            "{source}"
        );
    }
}

#[test]
fn sfc_keeps_supported_blocks_comments_and_unicode_whitespace() {
    let source = "\u{feff} \u{2003}<!-- before -->\n<script setup>const html = '<p>raw</p>'</script>\n<template><p>ok</p></template>\n<!-- after --><style>p { color: red; }</style>\u{2003}";
    let sfc = parse_vue_sfc(source).unwrap();
    assert_eq!(sfc.template.as_deref(), Some("<p>ok</p>"));
    assert_eq!(sfc.scripts.len(), 1);
    assert_eq!(sfc.styles.len(), 1);
    assert!(sfc.scripts[0].content.contains("<p>raw</p>"));
    assert_eq!(text_of(&element(&parse(source)[0]).children[0]), "ok");
}

#[test]
fn sfc_splits_template_script_and_style() {
    let sfc = parse_vue_sfc(
        r#"<template>
  <div class="p-4">hi</div>
</template>

<script setup lang="ts">
const count = 1
</script>

<style scoped>
.p-4 { padding: 1rem; }
</style>
"#,
    )
    .unwrap();

    assert!(sfc
        .template
        .as_deref()
        .unwrap()
        .contains("<div class=\"p-4\">"));
    assert_eq!(sfc.scripts.len(), 1);
    assert!(sfc.scripts[0].setup);
    assert_eq!(sfc.scripts[0].lang.as_deref(), Some("ts"));
    assert!(sfc.scripts[0].content.contains("const count = 1"));
    assert_eq!(sfc.styles.len(), 1);
    assert!(sfc.styles[0].scoped);
    assert!(sfc.styles[0].content.contains("padding: 1rem"));
}

#[test]
fn script_content_is_never_parsed_as_markup() {
    let nodes = parse(
        r#"<template><span>ok</span></template>
<script>
const html = "<div class='nope'></div>"
</script>"#,
    );
    assert_eq!(nodes.len(), 1);
    assert_eq!(element(&nodes[0]).tag, "span");
}

#[test]
fn nested_template_element_does_not_end_the_sfc_block() {
    let nodes = parse(
        r#"<template>
  <ul>
    <template v-for="item in items">
      <li>{{ item }}</li>
    </template>
  </ul>
</template>"#,
    );
    let ul = element(&nodes[0]);
    assert_eq!(ul.tag, "ul");
    let Node::For(block) = &ul.children[0] else {
        panic!("expected for block, got {:?}", ul.children[0]);
    };
    assert_eq!(block.pattern, "item");
    assert_eq!(block.iterator, "items");
    assert_eq!(element(&block.body[0]).tag, "li");
}

#[test]
fn mustache_interpolation_becomes_text_parts() {
    let nodes = parse_body(r#"<p>Hello {{ name }}!</p>"#);
    let p = element(&nodes[0]);
    let Node::Text(parts) = &p.children[0] else {
        panic!("expected text");
    };
    assert!(matches!(&parts[0], TextPart::Literal(s) if s == "Hello "));
    assert!(matches!(&parts[1], TextPart::Expr(e) if e == "name"));
    assert!(matches!(&parts[2], TextPart::Literal(s) if s == "!"));
}

#[test]
fn v_if_else_if_else_chain_lowers_to_nested_if_blocks() {
    let nodes = parse_body(
        r#"<div>
  <p v-if="a">A</p>
  <p v-else-if="b">B</p>
  <p v-else>C</p>
</div>"#,
    );
    let root = element(&nodes[0]);
    assert_eq!(root.children.len(), 1);
    let Node::If(outer) = &root.children[0] else {
        panic!("expected if");
    };
    assert_eq!(outer.condition, "a");
    assert_eq!(text_of(&element(&outer.then_children[0]).children[0]), "A");
    let Some([Node::If(inner)]) = outer.else_children.as_deref() else {
        panic!("expected else-if chain");
    };
    assert_eq!(inner.condition, "b");
    let Some(else_nodes) = inner.else_children.as_deref() else {
        panic!("expected else");
    };
    assert_eq!(text_of(&element(&else_nodes[0]).children[0]), "C");
}

#[test]
fn v_for_rejects_index_form_instead_of_dropping_it() {
    let source = r#"<template><li v-for="(item, i) in items">{{ i }}</li></template>"#;
    let error = parse_template_with_path(source, Some(Path::new("Index.vue"))).unwrap_err();
    assert!(
        error.to_string().contains("index and key bindings"),
        "{error}"
    );
}

#[test]
fn v_show_becomes_a_conditional_hidden_class() {
    let nodes = parse_body(r#"<div v-show="visible"></div>"#);
    let el = element(&nodes[0]);
    assert_eq!(el.conditional_classes.len(), 1);
    assert_eq!(el.conditional_classes[0].class, "hidden");
    assert_eq!(el.conditional_classes[0].condition, "!(visible)");
}

#[test]
fn v_html_and_v_text_replace_children() {
    let nodes = parse_body(r#"<div v-html="body"></div><span v-text="label"></span>"#);
    let html = element(&nodes[0]);
    assert!(matches!(&html.children[0], Node::RawHtml(e) if e == "body"));
    let text = element(&nodes[1]);
    assert_eq!(text_of(&text.children[0]), "{label}");
}

#[test]
fn bindings_shorthand_and_longhand_are_equivalent() {
    let nodes = parse_body(r#"<img :src="url" v-bind:alt="caption" width="64" />"#);
    let el = element(&nodes[0]);
    assert_eq!(el.bindings[0].prop, "src");
    assert_eq!(el.bindings[0].value, "url");
    assert_eq!(el.bindings[1].prop, "alt");
    assert_eq!(el.bindings[1].value, "caption");
    assert_eq!(el.bindings[2].prop, "width");
    assert_eq!(el.bindings[2].value, "\"64\"");
}

#[test]
fn dynamic_class_object_and_array_syntax() {
    let nodes = parse_body(
        r#"<div class="base" :class="{ active: isOn, 'text-red-500 font-bold': danger }"></div>
<div :class="['a', dynamic]"></div>
<div :class="cls"></div>"#,
    );
    let obj = element(&nodes[0]);
    assert_eq!(obj.classes, vec!["base".to_string()]);
    assert_eq!(obj.conditional_classes.len(), 3);
    assert_eq!(obj.conditional_classes[0].class, "active");
    assert_eq!(obj.conditional_classes[0].condition, "isOn");
    assert_eq!(obj.conditional_classes[1].class, "text-red-500");
    assert_eq!(obj.conditional_classes[1].condition, "danger");
    assert_eq!(obj.conditional_classes[2].class, "font-bold");

    let arr = element(&nodes[1]);
    assert_eq!(arr.classes, vec!["a".to_string(), "{dynamic}".to_string()]);

    let plain = element(&nodes[2]);
    assert_eq!(plain.classes, vec!["{cls}".to_string()]);
}

#[test]
fn events_shorthand_longhand_and_modifiers() {
    let nodes = parse_body(
        r#"<button @click="increment">+</button>
<form v-on:submit.prevent.stop="save"></form>"#,
    );
    let button = element(&nodes[0]);
    assert_eq!(button.event_handlers[0].event, "click");
    assert_eq!(button.event_handlers[0].handler, "increment");
    assert!(button.event_handlers[0].modifiers.is_empty());

    let form = element(&nodes[1]);
    assert_eq!(form.event_handlers[0].event, "submit");
    assert_eq!(form.event_handlers[0].handler, "save");
    assert_eq!(form.event_handlers[0].modifiers, vec!["prevent", "stop"]);
}

#[test]
fn event_directives_require_nonempty_handlers() {
    for attribute in [
        "@click",
        "v-on:click",
        "@click=\"\"",
        "v-on:click=\"   \"",
        "@click={}",
        "@click.stop",
        "v-on:submit.prevent.stop=\"\"",
    ] {
        for body in [
            format!("<button {attribute} />"),
            format!("<button {attribute}></button>"),
        ] {
            let source = format!("<template>{body}</template>");
            let error =
                parse_template_with_path(&source, Some(Path::new("Events.vue"))).unwrap_err();
            assert!(
                error.to_string().contains("nonempty handler"),
                "{attribute}: {error}"
            );
        }
    }
}

#[test]
fn v_model_becomes_a_bind_binding() {
    let nodes = parse_body(r#"<input type="text" v-model="query" />"#);
    let el = element(&nodes[0]);
    assert!(el
        .bindings
        .iter()
        .any(|b| b.prop == "bind" && b.value == "query"));
}

#[test]
fn void_elements_and_comments_are_handled() {
    let nodes = parse_body(
        r#"<div>
  <!-- a comment with <div> inside -->
  <br>
  <input type="text">
</div>"#,
    );
    let el = element(&nodes[0]);
    assert_eq!(el.children.len(), 2);
    assert_eq!(element(&el.children[0]).tag, "br");
    assert_eq!(element(&el.children[1]).tag, "input");
}

#[test]
fn id_attribute_populates_element_id() {
    let nodes = parse_body(r#"<section id="main" class="flex gap-2"></section>"#);
    let el = element(&nodes[0]);
    assert_eq!(el.id.as_deref(), Some("main"));
    assert_eq!(el.classes, vec!["flex".to_string(), "gap-2".to_string()]);
}

#[test]
fn unsupported_constructs_report_errors() {
    for (src, needle) in [
        (r#"<div v-slot:header="p"></div>"#, "v-slot"),
        (r#"<div v-once></div>"#, "v-once"),
        (r#"<div v-bind="props"></div>"#, "v-bind"),
        (r#"<div :style="s"></div>"#, ":style"),
        (r#"<input v-model.number="n" />"#, "v-model"),
        (r#"<button @keyup.enter="go"></button>"#, "enter"),
        (r#"<template #header></template>"#, "slot shorthand"),
        (r#"<p v-else>x</p>"#, "v-else"),
    ] {
        let err = parse_template_with_path(
            &format!("<template>{src}</template>"),
            Some(Path::new("Comp.vue")),
        )
        .expect_err("expected unsupported construct to error");
        let message = err.to_string();
        assert!(
            message.contains(needle),
            "error for {src:?} should mention {needle:?}, got: {message}"
        );
    }
}

#[test]
fn missing_template_block_yields_no_nodes() {
    let nodes = parse("<script setup>\nconst a = 1\n</script>\n");
    assert!(nodes.is_empty());
}

#[test]
fn non_vue_paths_do_not_use_the_vue_frontend() {
    let nodes =
        parse_template_with_path("div flex\n  \"hi\"", Some(Path::new("page.crepus"))).unwrap();
    assert_eq!(element(&nodes[0]).tag, "div");
}

#[test]
fn v_for_rejects_missing_iterators() {
    for expression in ["item in ", "item of ", "(item, i) in   "] {
        let source = format!("<template><li v-for=\"{expression}\">{{{{ item }}}}</li></template>");
        let error = parse_template_with_path(&source, Some(Path::new("Missing.vue"))).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("could not parse v-for expression"),
            "{error}"
        );
    }
    for expression in ["item in items", "item of items", "(item) in items"] {
        let source = format!("<template><li v-for=\"{expression}\">{{{{ item }}}}</li></template>");
        assert!(parse_template_with_path(&source, Some(Path::new("Valid.vue"))).is_ok());
    }
}

#[test]
fn conditional_directives_require_nonempty_expressions() {
    for directive in ["v-if", "v-else-if"] {
        for attribute in [
            directive.to_string(),
            format!("{directive}=\"\""),
            format!("{directive}=\"   \""),
            format!("{directive}={{}}"),
        ] {
            let source = format!("<template><div v-if=\"false\" /><div {attribute} /></template>");
            let error =
                parse_template_with_path(&source, Some(Path::new("Condition.vue"))).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(&format!("{directive} requires an expression")),
                "{error}"
            );
        }
        for expression in ["false", "0", "true"] {
            let source = format!(
                "<template><div v-if=\"false\" /><div {directive}=\"{expression}\" /></template>"
            );
            assert!(parse_template_with_path(&source, Some(Path::new("Condition.vue"))).is_ok());
        }
    }
}

#[test]
fn conflicting_branch_directives_are_parse_errors() {
    let directives = ["v-if=\"true\"", "v-else-if=\"true\"", "v-else"];
    for first in directives {
        for second in directives {
            let source =
                format!("<template><div v-if=\"false\" /><div {first} {second} /></template>");
            let error =
                parse_template_with_path(&source, Some(Path::new("Branches.vue"))).unwrap_err();
            assert!(
                error.to_string().contains("only one branch directive"),
                "{first} {second}: {error}"
            );
        }
    }
    let source =
        "<template><div v-if=\"true\" v-for=\"item in items\" v-show=\"visible\" /></template>";
    assert!(parse_template_with_path(source, Some(Path::new("Branches.vue"))).is_ok());
}

#[test]
fn content_directives_require_one_nonempty_expression() {
    for directive in ["v-text", "v-html"] {
        for attribute in [
            directive.to_string(),
            format!("{directive}=\"\""),
            format!("{directive}=\"   \""),
            format!("{directive}={{}}"),
        ] {
            let source = format!("<template><div {attribute} /></template>");
            let error =
                parse_template_with_path(&source, Some(Path::new("Content.vue"))).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(&format!("{directive} requires an expression")),
                "{error}"
            );
        }
        for second in ["v-text", "v-html"] {
            let source =
                format!("<template><div {directive}=\"first\" {second}=\"second\" /></template>");
            let error =
                parse_template_with_path(&source, Some(Path::new("Content.vue"))).unwrap_err();
            assert!(
                error.to_string().contains("only one content directive"),
                "{error}"
            );
        }
        for expression in ["0", "false", "''"] {
            let source = format!("<template><div {directive}=\"{expression}\" /></template>");
            assert!(parse_template_with_path(&source, Some(Path::new("Content.vue"))).is_ok());
        }
    }
}

#[test]
fn duplicate_loop_directives_cannot_override_the_first_loop() {
    let source = r#"<template><div v-for="item in items" v-for="other in others">{{ item }}</div></template>"#;
    let error = parse_template_with_path(source, Some(Path::new("Loop.vue"))).unwrap_err();
    assert!(
        error.to_string().contains("only one v-for directive"),
        "{error}"
    );
    let nodes = parse_body(
        r#"<div v-for="item in items"><span v-for="other in others">{{ other }}</span></div>"#,
    );
    let Node::For(outer) = &nodes[0] else {
        panic!("expected outer loop")
    };
    assert_eq!(outer.pattern, "item");
    let Node::For(inner) = &element(&outer.body[0]).children[0] else {
        panic!("expected nested loop")
    };
    assert_eq!(inner.pattern, "other");
}

#[test]
fn v_model_rejects_missing_binding_expressions() {
    for attribute in ["v-model", "v-model=\"\"", "v-model=\"   \"", "v-model={}"] {
        let source = format!("<template><input {attribute} /></template>");
        let error = parse_template_with_path(&source, Some(Path::new("Model.vue"))).unwrap_err();
        assert!(
            error.to_string().contains("v-model requires an expression"),
            "{error}"
        );
    }
    for expression in ["query", "form.query"] {
        let nodes = parse_body(&format!("<input v-model=\"{expression}\" />"));
        assert_eq!(element(&nodes[0]).bindings[0].value, expression);
    }
}

#[test]
fn property_bindings_require_names_and_nonempty_expressions() {
    for attribute in [
        ":title",
        ":title=\"\"",
        "v-bind:id=\"   \"",
        ":class={}",
        ":=\"value\"",
        "v-bind:=\"value\"",
    ] {
        let source = format!("<template><div {attribute} /></template>");
        let error = parse_template_with_path(&source, Some(Path::new("Binding.vue"))).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("property name and a nonempty expression"),
            "{error}"
        );
    }
    for property in [":title", "v-bind:title", ":class", ":id"] {
        for expression in ["''", "0", "false"] {
            let source = format!("<template><div {property}=\"{expression}\" /></template>");
            assert!(parse_template_with_path(&source, Some(Path::new("Binding.vue"))).is_ok());
        }
    }
}

#[test]
fn event_directives_require_event_names() {
    for attribute in [
        "@=\"save\"",
        "v-on:=\"save\"",
        "@.prevent=\"save\"",
        "v-on:.stop=\"save\"",
    ] {
        let source = format!("<template><button {attribute} /></template>");
        let error = parse_template_with_path(&source, Some(Path::new("Event.vue"))).unwrap_err();
        assert!(
            error.to_string().contains("requires an event name"),
            "{error}"
        );
    }
    for attribute in [
        "@click=\"save\"",
        "v-on:click=\"save\"",
        "@click.prevent=\"save\"",
    ] {
        let nodes = parse_body(&format!("<button {attribute} />"));
        assert_eq!(element(&nodes[0]).event_handlers[0].event, "click");
    }
}

#[test]
fn class_list_commas_inside_escaped_quotes_stay_in_the_same_entry() {
    for quoted in [r#"'snow\',雪'"#, r#""snow\",雪""#, r#"'backslash\\'"#] {
        let source = format!("{quoted}, tail");
        assert_eq!(super::builders::split_top_level(&source), [quoted, " tail"]);
        let expression = format!("[{quoted}, 'tail']");
        let mut classes = Vec::new();
        super::builders::apply_dynamic_class(&expression, &mut classes, &mut Vec::new()).unwrap();
        assert_eq!(classes.len(), 2, "{classes:?}");
        assert_eq!(classes[1], "tail");
    }
}

#[test]
fn class_list_literals_decode_escapes_before_splitting_classes() {
    for expression in [
        r#"'one\ttwo'"#,
        r#"['one\ttwo']"#,
        r#""one\ntwo""#,
        r#"["one\ntwo"]"#,
    ] {
        let mut classes = Vec::new();
        super::builders::apply_dynamic_class(expression, &mut classes, &mut Vec::new()).unwrap();
        assert_eq!(classes, ["one", "two"], "{expression}");
    }
    let mut classes = Vec::new();
    super::builders::apply_dynamic_class(
        r#"['snow\',雪', 'path\\name']"#,
        &mut classes,
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(classes, ["snow',雪", "path\\name"]);
}

#[test]
fn quoted_class_object_keys_decode_escapes_and_preserve_inner_quotes() {
    let mut conditional = Vec::new();
    super::builders::apply_dynamic_class(
        r#"{ 'one\ttwo': active, "'quoted'": selected, 'snow\',雪': enabled }"#,
        &mut Vec::new(),
        &mut conditional,
    )
    .unwrap();
    let names: Vec<_> = conditional
        .iter()
        .map(|class| class.class.as_str())
        .collect();
    assert_eq!(names, ["one", "two", "'quoted'", "snow',雪"]);
    assert_eq!(conditional[0].condition, "active");
    assert_eq!(conditional[2].condition, "selected");
}

#[test]
fn class_object_separator_ignores_colons_in_quoted_conditions() {
    let mut conditional = Vec::new();
    super::builders::apply_dynamic_class(
        r#"{ 'hover:bg-red': state == 'a:b', plain: label == "snow\":雪" }"#,
        &mut Vec::new(),
        &mut conditional,
    )
    .unwrap();
    assert_eq!(conditional[0].class, "hover:bg-red");
    assert_eq!(conditional[0].condition, "state == 'a:b'");
    assert_eq!(conditional[1].class, "plain");
    assert_eq!(conditional[1].condition, r#"label == "snow\":雪""#);
}

#[test]
fn malformed_class_list_strings_return_errors_without_panicking() {
    for expression in ["[']", "[\"]", "['unfinished]", "[\"unfinished]"] {
        let error =
            super::builders::apply_dynamic_class(expression, &mut Vec::new(), &mut Vec::new())
                .unwrap_err();
        assert!(error.message.contains("closing quote"), "{error:?}");
    }
    for expression in ["['']", "[\"\"]", "['雪']"] {
        super::builders::apply_dynamic_class(expression, &mut Vec::new(), &mut Vec::new()).unwrap();
    }
}

#[test]
fn nested_class_arrays_flatten_with_a_bounded_depth() {
    let mut classes = Vec::new();
    let mut conditional = Vec::new();
    super::builders::apply_dynamic_class(
        "['one', ['two', { active: enabled }]]",
        &mut classes,
        &mut conditional,
    )
    .unwrap();
    assert_eq!(classes, ["one", "two"]);
    assert_eq!(conditional[0].class, "active");
    assert_eq!(conditional[0].condition, "enabled");
    let accepted = format!("{}'deep'{}", "[".repeat(65), "]".repeat(65));
    let mut boundary_classes = Vec::new();
    super::builders::apply_dynamic_class(&accepted, &mut boundary_classes, &mut Vec::new())
        .unwrap();
    assert_eq!(boundary_classes, ["deep"]);
    let expression = format!("{}'deep'{}", "[".repeat(66), "]".repeat(66));
    let error = super::builders::apply_dynamic_class(&expression, &mut Vec::new(), &mut Vec::new())
        .unwrap_err();
    assert!(error.message.contains("nesting depth"), "{error:?}");
    let mut recovered = Vec::new();
    super::builders::apply_dynamic_class("['after']", &mut recovered, &mut Vec::new()).unwrap();
    assert_eq!(recovered, ["after"]);
}

#[test]
fn false_and_null_class_values_contribute_no_classes() {
    for expression in ["false", "null", "[false, null]", "[[false], null]"] {
        let mut classes = Vec::new();
        super::builders::apply_dynamic_class(expression, &mut classes, &mut Vec::new()).unwrap();
        assert!(classes.is_empty(), "{expression}: {classes:?}");
    }
    let mut classes = Vec::new();
    super::builders::apply_dynamic_class("['false', 'null']", &mut classes, &mut Vec::new())
        .unwrap();
    assert_eq!(classes, ["false", "null"]);
}

#[test]
fn id_aliases_cannot_emit_duplicate_attributes() {
    for first in [r#"id="first""#, r#":id="first""#, r#"v-bind:id="first""#] {
        for second in [r#"id="second""#, r#":id="second""#, r#"v-bind:id="second""#] {
            let source = format!("<template><div {first} {second} /></template>");
            let error = parse_template_with_path(&source, Some(Path::new("Ids.vue"))).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("only one id attribute or binding"),
                "{error}"
            );
        }
        parse_body(&format!("<div {first} />"));
    }
}

#[test]
fn sfc_rejects_template_preprocessors_instead_of_rendering_their_source() {
    for attribute in ["lang=\"pug\"", "lang=\"md\"", "lang={format}"] {
        let source = format!("<!-- prefix -->\n<template {attribute}>p hello</template>");
        let error = super::sfc::split_sfc(&source).expect_err(&source);
        assert!(error.message.contains("template language"), "{error:?}");
        assert_eq!(error.byte_offset, source.find("<template"));
        assert!(parse_vue_sfc(&source).is_err(), "{source}");
        assert!(
            parse_template_with_path(&source, Some(Path::new("Comp.vue"))).is_err(),
            "{source}"
        );
    }
}

#[test]
fn sfc_explicit_html_template_language_preserves_markup() {
    let source = "<template lang=\"html\"><p>hello</p></template>";
    assert_eq!(
        parse_vue_sfc(source).unwrap().template.as_deref(),
        Some("<p>hello</p>")
    );
    assert_eq!(text_of(&element(&parse(source)[0]).children[0]), "hello");
}

#[test]
fn sfc_preserves_generic_parameters_without_executing_script() {
    let source = "<script setup lang=\"ts\" generic=\"T extends string, U = T\">const value: U = undefined;</script><template><p>static</p></template>";
    let (sfc, generics) = super::parse_vue_sfc_with_generics(source).unwrap();
    assert_eq!(generics[0].as_deref(), Some("T extends string, U = T"));
    assert!(sfc.scripts[0].content.contains("const value: U"));
    assert_eq!(text_of(&element(&parse(source)[0]).children[0]), "static");
    assert!(parse_vue_sfc("<script setup generic></script><template/>")
        .unwrap_err()
        .to_string()
        .contains("generic"));
}

#[test]
fn sfc_duplicate_block_attributes_fail_before_metadata_selection() {
    for block in [
        "<script setup lang=\"js\" lang=\"ts\"></script>",
        "<script setup setup=\"false\"></script>",
        "<script generic=\"T\" generic=\"U\"></script>",
        "<template lang=\"html\" lang=\"html\"/>",
        "<template lang=\"html\" lang=\"pug\"/>",
        "<style scoped scoped=\"false\"></style>",
        "<style module=\"first\" module=\"second\"/>",
    ] {
        let source = format!("<!-- prefix -->\n{block}");
        let error = super::sfc::split_sfc(&source).expect_err(block);
        assert!(error.message.contains("duplicate Vue SFC"), "{error:?}");
        assert_eq!(error.byte_offset, source.find(block));
        assert!(parse_vue_sfc(&source).is_err(), "{source}");
        assert!(
            super::parse_vue_sfc_with_generics(&source).is_err(),
            "{source}"
        );
        assert!(
            parse_template_with_path(&source, Some(Path::new("Comp.vue"))).is_err(),
            "{source}"
        );
    }
}

#[test]
fn sfc_repeated_attribute_text_and_attributes_on_different_blocks_are_valid() {
    let source = r#"<!-- <script setup setup></script> --><script lang="ts">const text = 'lang="js" lang="ts"';</script><script setup lang="ts">const value=1;</script><template lang="html"><p title='lang="js" lang="ts"'>ok</p></template><style lang="css">p{color:red}</style><style lang="css">p{color:blue}</style>"#;
    let sfc = parse_vue_sfc(source).unwrap();
    assert_eq!(sfc.scripts.len(), 2);
    assert_eq!(sfc.styles.len(), 2);
    assert_eq!(text_of(&element(&parse(source)[0]).children[0]), "ok");
}

#[test]
fn plain_nested_template_retains_element_while_structural_templates_flatten() {
    let nodes = parse_body("<Child><template><b>inert</b></template></Child>");
    let child = element(&nodes[0]);
    let template = element(&child.children[0]);
    assert_eq!(template.tag, "template");
    assert_eq!(element(&template.children[0]).tag, "b");
    let conditional = parse_body("<template v-if=\"shown\"><b>conditional</b></template>");
    let Node::If(branch) = &conditional[0] else {
        panic!("conditional template must retain its branch");
    };
    assert_eq!(element(&branch.then_children[0]).tag, "b");
    let repeated = parse_body("<template v-for=\"item in items\"><b>{{ item }}</b></template>");
    let Node::For(rows) = &repeated[0] else {
        panic!("loop template must retain its rows");
    };
    assert_eq!(element(&rows.body[0]).tag, "b");
}

#[test]
fn terminal_else_with_nested_if_cannot_accept_another_sibling_branch() {
    for trailing in [r#"<p v-else>C</p>"#, r#"<p v-else-if="true">C</p>"#] {
        for separator in ["", " \n<!-- branch separator -->\n "] {
            let source = format!(
                r#"<template><p v-if="false">A</p><template v-else><p v-if="false">B</p></template>{separator}{trailing}</template>"#
            );
            let error = parse_template_with_path(&source, Some(Path::new("Comp.vue")))
                .expect_err("a terminal else closes the sibling chain")
                .to_string();
            assert!(error.contains("without a preceding v-if"), "{error}");
        }
    }
}

#[test]
fn genuine_else_if_chains_keep_comment_adjacency_and_nested_terminal_branches() {
    let nodes = parse_body(
        r#"<p v-if="false">A</p>
<!-- first gap -->
<template v-else-if="false"><p>B</p></template>
<!-- second gap -->
<template v-else><p v-if="true">nested</p></template>"#,
    );
    assert_eq!(nodes.len(), 1);
    let Node::If(first) = &nodes[0] else {
        panic!("expected initial branch");
    };
    let Some([Node::If(second)]) = first.else_children.as_deref() else {
        panic!("expected genuine else-if branch");
    };
    let Some([Node::If(nested)]) = second.else_children.as_deref() else {
        panic!("expected terminal branch's independent nested if");
    };
    let ctx = crate::TemplateContext::new();
    assert!(!ctx.eval_condition(&first.condition).unwrap());
    assert!(!ctx.eval_condition(&second.condition).unwrap());
    assert!(ctx.eval_condition(&nested.condition).unwrap());
    assert!(nested.else_children.is_none());
    assert_eq!(
        text_of(&element(&nested.then_children[0]).children[0]),
        "nested"
    );
}

#[test]
fn ordinary_template_elements_interrupt_sibling_conditional_chains() {
    for middle in [
        r#"<template><p v-if="true">nested</p></template>"#,
        "<template></template>",
    ] {
        let source = format!(
            r#"<template><p v-if="false">A</p>{middle}<!-- gap --><p v-else>B</p></template>"#
        );
        let error = parse_template_with_path(&source, Some(Path::new("Comp.vue")))
            .expect_err("ordinary template elements interrupt the sibling chain")
            .to_string();
        assert!(error.contains("without a preceding v-if"), "{error}");
    }
}
