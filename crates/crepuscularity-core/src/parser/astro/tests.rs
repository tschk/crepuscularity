use std::path::Path;

use crate::ast::*;
use crate::parser::parse_template_with_path;

fn parse(src: &str) -> Vec<Node> {
    parse_template_with_path(src, Some(Path::new("Page.astro"))).expect("astro should parse")
}

fn parse_err(src: &str) -> String {
    parse_template_with_path(src, Some(Path::new("Page.astro")))
        .expect_err("expected astro parse error")
        .to_string()
}

fn element(node: &Node) -> &Element {
    match node {
        Node::Element(el) => el,
        other => panic!("expected element, got {other:?}"),
    }
}

fn text_of(node: &Node) -> String {
    match node {
        Node::Text(parts) => parts
            .iter()
            .map(|p| match p {
                TextPart::Literal(s) => s.clone(),
                TextPart::Expr(e) => format!("{{{e}}}"),
            })
            .collect(),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn frontmatter_is_skipped_and_never_parsed_as_markup() {
    let nodes = parse(
        r#"---
import Card from "../Card.astro";
const markup = "<div class='nope'></div>";
---
<section class="p-4"><p>ok</p></section>"#,
    );
    assert_eq!(nodes.len(), 1);
    let section = element(&nodes[0]);
    assert_eq!(section.tag, "section");
    assert_eq!(section.classes, vec!["p-4".to_string()]);
    assert_eq!(element(&section.children[0]).tag, "p");
}

#[test]
fn elements_attributes_and_self_closing_tags() {
    let nodes = parse(r#"<img src="/a.png" alt="A" width={w} /><br /><hr>"#);
    let img = element(&nodes[0]);
    assert_eq!(img.tag, "img");
    assert_eq!(img.bindings[0].prop, "src");
    assert_eq!(img.bindings[0].value, "\"/a.png\"");
    assert_eq!(img.bindings[2].prop, "width");
    assert_eq!(img.bindings[2].value, "w");
    assert_eq!(element(&nodes[1]).tag, "br");
    assert_eq!(element(&nodes[2]).tag, "hr");
}

#[test]
fn interpolation_stays_inside_the_text_run() {
    let nodes = parse(r#"<p>Hello {name}!</p>"#);
    let p = element(&nodes[0]);
    assert_eq!(text_of(&p.children[0]), "Hello {name}!");
}

#[test]
fn shorthand_attribute_expands_to_a_named_binding() {
    let nodes = parse(r#"<a {href}>x</a>"#);
    let a = element(&nodes[0]);
    assert_eq!(a.bindings[0].prop, "href");
    assert_eq!(a.bindings[0].value, "href");
}

#[test]
fn logical_and_expression_becomes_an_if_block() {
    let nodes = parse(r#"<div>{show && <span>yes</span>}</div>"#);
    let div = element(&nodes[0]);
    let Node::If(block) = &div.children[0] else {
        panic!("expected if, got {:?}", div.children[0]);
    };
    assert_eq!(block.condition, "show");
    assert_eq!(element(&block.then_children[0]).tag, "span");
    assert!(block.else_children.is_none());
}

#[test]
fn ternary_expression_becomes_an_if_else_block() {
    let nodes = parse(r#"<div>{ok ? <b>y</b> : <i>n</i>}</div>"#);
    let div = element(&nodes[0]);
    let Node::If(block) = &div.children[0] else {
        panic!("expected if");
    };
    assert_eq!(block.condition, "ok");
    assert_eq!(element(&block.then_children[0]).tag, "b");
    let Some(else_nodes) = block.else_children.as_deref() else {
        panic!("expected else branch");
    };
    assert_eq!(element(&else_nodes[0]).tag, "i");
}

#[test]
fn map_call_becomes_a_for_block() {
    let nodes = parse(r#"<ul>{items.map((item) => (<li>{item.name}</li>))}</ul>"#);
    let ul = element(&nodes[0]);
    let Node::For(block) = &ul.children[0] else {
        panic!("expected for, got {:?}", ul.children[0]);
    };
    assert_eq!(block.pattern, "item");
    assert_eq!(block.iterator, "items");
    assert_eq!(element(&block.body[0]).tag, "li");
}

#[test]
fn map_call_without_parens_around_the_body() {
    let nodes = parse(r#"<ul>{posts.map(post => <li>{post.title}</li>)}</ul>"#);
    let Node::For(block) = &element(&nodes[0]).children[0] else {
        panic!("expected for");
    };
    assert_eq!(block.pattern, "post");
    assert_eq!(block.iterator, "posts");
}

#[test]
fn plain_expression_stays_an_interpolation() {
    let nodes = parse(r#"<p>{items.map(i => i.name).join(", ")}</p>"#);
    let p = element(&nodes[0]);
    assert_eq!(
        text_of(&p.children[0]),
        r#"{items.map(i => i.name).join(", ")}"#
    );
}

#[test]
fn comments_are_dropped() {
    let nodes = parse(
        r#"<div>
  <!-- an html comment with <span> inside -->
  {/* a javascript comment */}
  <b>x</b>
</div>"#,
    );
    let div = element(&nodes[0]);
    assert_eq!(div.children.len(), 1);
    assert_eq!(element(&div.children[0]).tag, "b");
}

#[test]
fn fragment_contributes_children_only() {
    let nodes = parse(r#"<div><Fragment><b>a</b><i>b</i></Fragment></div>"#);
    let div = element(&nodes[0]);
    assert_eq!(div.children.len(), 2);
    assert_eq!(element(&div.children[0]).tag, "b");
    assert_eq!(element(&div.children[1]).tag, "i");
}

#[test]
fn class_list_lowers_to_classes_and_conditional_classes() {
    let nodes = parse(
        r#"<div class="base" class:list={["a", { active: isOn, "text-red-500 font-bold": danger }, dyn]}></div>"#,
    );
    let el = element(&nodes[0]);
    assert_eq!(
        el.classes,
        vec!["base".to_string(), "a".to_string(), "{dyn}".to_string()]
    );
    assert_eq!(el.conditional_classes.len(), 3);
    assert_eq!(el.conditional_classes[0].class, "active");
    assert_eq!(el.conditional_classes[0].condition, "isOn");
    assert_eq!(el.conditional_classes[1].class, "text-red-500");
    assert_eq!(el.conditional_classes[1].condition, "danger");
    assert_eq!(el.conditional_classes[2].class, "font-bold");
}

#[test]
fn set_html_and_set_text_replace_children() {
    let nodes = parse(r#"<div set:html={body}>dropped</div><span set:text={label}></span>"#);
    let html = element(&nodes[0]);
    assert!(matches!(&html.children[0], Node::RawHtml(e) if e == "body"));
    let text = element(&nodes[1]);
    assert_eq!(text_of(&text.children[0]), "{label}");
}

#[test]
fn client_directives_are_recorded_as_bindings() {
    let nodes = parse(r#"<div client:load client:media="(max-width: 50em)"></div>"#);
    let el = element(&nodes[0]);
    assert_eq!(el.bindings[0].prop, "client:load");
    assert_eq!(el.bindings[0].value, "true");
    assert_eq!(el.bindings[1].prop, "client:media");
    assert_eq!(el.bindings[1].value, "\"(max-width: 50em)\"");
}

#[test]
fn unsupported_constructs_report_errors() {
    for (src, needle) in [
        (r#"<slot />"#, "<slot />"),
        (r#"<Card title="x" />"#, "component tag"),
        (r#"<div {...props}></div>"#, "spread attributes"),
        (r#"<div transition:name="hero"></div>"#, "transition:"),
        (r#"<div define:vars={{a: 1}}></div>"#, "define:vars"),
        (
            r#"<ul>{items.map((item, i) => <li>{i}</li>)}</ul>"#,
            "index bindings",
        ),
        (
            r#"<ul>{items.map(({ a }) => <li>{a}</li>)}</ul>"#,
            "destructuring",
        ),
        (
            r#"<ul>{items.map(i => { return <li /> })}</ul>"#,
            "block-bodied",
        ),
        (r#"<div class:active={on}></div>"#, "class:list"),
        (r#"<div><b>x</b></div"#, "expected </div>"),
    ] {
        let message = parse_err(src);
        assert!(
            message.contains(needle),
            "error for {src:?} should mention {needle:?}, got: {message}"
        );
    }
}

#[test]
fn non_astro_paths_do_not_use_the_astro_frontend() {
    let nodes =
        parse_template_with_path("div flex\n  \"hi\"", Some(Path::new("page.crepus"))).unwrap();
    assert_eq!(element(&nodes[0]).tag, "div");
}

#[test]
fn content_directives_require_a_value_and_are_mutually_exclusive() {
    for directive in ["set:html", "set:text"] {
        for attribute in [directive.to_string(), format!("{directive}={{}}")] {
            let error = parse_err(&format!("<div {attribute} />"));
            assert!(error.contains("requires an expression"), "{error}");
        }
        for second in ["set:html", "set:text"] {
            let error = parse_err(&format!(
                "<Fragment {directive}={{first}} {second}={{second}} />"
            ));
            assert!(error.contains("only one content directive"), "{error}");
        }
        for expression in ["0", "false", "''"] {
            parse(&format!("<div {directive}={{{expression}}} />"));
        }
    }
}

#[test]
fn class_list_rejects_missing_and_empty_braced_values() {
    for attribute in ["class:list", "class:list={}", "class:list={   }"] {
        let error = parse_err(&format!("<div {attribute} />"));
        assert!(error.contains("class:list requires"), "{error}");
    }
    for attribute in [
        "class:list=\"\"",
        "class:list={[]}",
        "class:list={false}",
        "class:list={''}",
    ] {
        parse(&format!("<div {attribute} />"));
    }
}

#[test]
fn unsupported_raw_text_tags_fail_before_parsing_their_bodies() {
    for source in [
        r#"<script>const text = "{secret}";</script>"#,
        r#"<script>if (a < b) { run(); }</script>"#,
        r#"<style>.x { color: red; }</style>"#,
        r#"<script src="/app.js"></script>"#,
        "<script />",
        "<style></style>",
        "<SCRIPT>text</SCRIPT>",
        "<Style />",
    ] {
        let error = parse_err(source);
        assert!(error.contains("raw-text"), "{source}: {error}");
    }
}

#[test]
fn stylesheet_links_and_ordinary_interpolation_remain_supported() {
    let nodes = parse(r#"<link rel="stylesheet" href="/app.css"/><p>{name}</p>"#);
    assert_eq!(element(&nodes[0]).tag, "link");
    assert_eq!(element(&nodes[1]).tag, "p");
    assert!(
        matches!(&element(&nodes[1]).children[0], Node::Text(parts) if matches!(&parts[0], TextPart::Expr(name) if name == "name"))
    );
}

mod split_tests {
    use crate::parser::split_astro_source;

    #[test]
    fn utf8_spans_slice_content_exactly() {
        let src = "---\nconst s = \"héllo ✓\";\n---\n<p>é</p>";
        let split = split_astro_source(src).unwrap();
        let fm = split.frontmatter.unwrap();
        assert_eq!(fm.content, "const s = \"héllo ✓\";\n");
        assert_eq!(&src[fm.span.clone()], fm.content);
        assert_eq!(split.template, "\n<p>é</p>");
        assert_eq!(&src[split.template_start..], split.template);
    }

    #[test]
    fn crlf_fences() {
        let src = "---\r\nlet a = 1;\r\n---\r\n<p/>";
        let split = split_astro_source(src).unwrap();
        let fm = split.frontmatter.unwrap();
        assert_eq!(fm.content, "let a = 1;\r\n");
        assert_eq!(split.template, "\n<p/>");
        assert_eq!(&src[split.template_start..], split.template);
    }

    #[test]
    fn empty_fence() {
        let src = "---\n---\n<p/>";
        let split = split_astro_source(src).unwrap();
        let fm = split.frontmatter.unwrap();
        assert_eq!(fm.content, "");
        assert_eq!(fm.span, 4..4);
        assert_eq!(split.template, "\n<p/>");
    }

    #[test]
    fn no_fence() {
        let src = "<p>---</p>";
        let split = split_astro_source(src).unwrap();
        assert!(split.frontmatter.is_none());
        assert_eq!(split.template, src);
        assert_eq!(split.template_start, 0);
    }

    #[test]
    fn close_at_eof() {
        let src = "---\nx\n---";
        let split = split_astro_source(src).unwrap();
        assert_eq!(split.frontmatter.unwrap().content, "x\n");
        assert_eq!(split.template, "");
        assert_eq!(split.template_start, src.len());
    }

    #[test]
    fn missing_close_errors() {
        for src in ["---", "---\nx\n<p/>", " \t---\r\n"] {
            assert!(split_astro_source(src).is_err(), "{src:?}");
        }
    }

    #[test]
    fn leading_whitespace_retains_original_byte_offsets() {
        let src = "\u{2003}\n \t--- \t\r\nconst value = 'é';\r\n \t--- \t\r\n<p/>";
        let split = split_astro_source(src).unwrap();
        let fm = split.frontmatter.unwrap();
        assert_eq!(fm.content, "const value = 'é';\r\n");
        assert_eq!(&src[fm.span], fm.content);
        assert_eq!(split.template, "\n<p/>");
        assert_eq!(&src[split.template_start..], split.template);
    }
}

#[test]
fn authored_module_components_do_not_change_legacy_template_admission() {
    for source in [
        "<Card title={name}/>",
        "<div><Card/></div>",
        "{show ? <Card/> : <Other/>}",
        "{items.map(item => <Card title={item}/>)}",
    ] {
        assert!(
            crate::parser::parse_astro_module_template(source).is_ok(),
            "{source}"
        );
        assert!(
            parse_template_with_path(source, Some(Path::new("Page.astro"))).is_err(),
            "{source}"
        );
    }
}

#[test]
fn authored_literals_and_braced_values_retain_different_origins() {
    let nodes = crate::parser::parse_astro_module_template(r#"<p id={"&amp;"} title="&amp;" class="a  b" class:list={["base", {active: ok}]}>"quoted" \n{message}</p>"#).unwrap();
    let el = element(&nodes[0]);
    assert!(el.id.is_none() && el.classes.is_empty());
    assert_eq!(el.bindings[0].origin, BindingOrigin::Expression);
    assert_eq!(el.bindings[1].origin, BindingOrigin::Literal);
    assert_eq!(el.bindings[2].value, "\"a  b\"");
    assert_eq!(el.bindings[3].prop, "class:list");
    assert!(
        matches!(&el.children[0], Node::Text(parts) if parts[0] == TextPart::Literal("\"quoted\" \\n".into()))
    );
}

#[test]
fn authored_content_directives_preserve_html_literal_and_js_expression_semantics() {
    let nodes = crate::parser::parse_astro_module_template(
        r#"<p set:text="&amp;"/><p set:text={'&amp;'}/><div set:html="&lt;b&gt;"/>"#,
    )
    .unwrap();
    assert_eq!(
        element(&nodes[0]).children,
        vec![Node::Text(vec![TextPart::Expr("\"&\"".into())])]
    );
    assert_eq!(
        element(&nodes[1]).children,
        vec![Node::Text(vec![TextPart::Expr("'&amp;'".into())])]
    );
    assert_eq!(
        element(&nodes[2]).children,
        vec![Node::RawHtml("\"<b>\"".into())]
    );
}

#[test]
fn shared_content_directives_preserve_quoted_literals_and_braced_expressions() {
    for (directive, raw_html) in [("set:text", false), ("set:html", true)] {
        for (attribute, expected) in [
            ("\"label\"", "\"label\""),
            ("\"&amp;&lt;b&gt;\"", "\"&<b>\""),
            ("\"\"", "\"\""),
            ("\"   \"", "\"   \""),
            ("{label}", "label"),
            ("{'&amp;'}", "'&amp;'"),
        ] {
            let nodes = parse(&format!("<div {directive}={attribute} />"));
            let expected_node = if raw_html {
                Node::RawHtml(expected.into())
            } else {
                Node::Text(vec![TextPart::Expr(expected.into())])
            };
            assert_eq!(
                element(&nodes[0]).children,
                vec![expected_node],
                "{directive}={attribute}"
            );
        }
    }
}

#[test]
fn html_attribute_entities_do_not_use_svelte_dynamic_character_rules() {
    assert_eq!(
        crate::parser::svelte_entities::decode_static_attribute("&notit; &not= &not_ &#0; &#10;"),
        "&notit; &not= ¬_ � \n"
    );
}

#[test]
fn authored_pre_whitespace_is_retained_through_elements_and_expressions() {
    let nodes =
        crate::parser::parse_astro_module_template("<pre>\n  {value}\n  <b> x </b>\n</pre>")
            .unwrap();
    let children = &element(&nodes[0]).children;
    assert_eq!(
        children[0],
        Node::Text(vec![TextPart::Literal("\n  ".into())])
    );
    assert_eq!(
        children[1],
        Node::Text(vec![TextPart::Expr("value".into())])
    );
    assert_eq!(
        children[2],
        Node::Text(vec![TextPart::Literal("\n  ".into())])
    );
    assert_eq!(
        element(&children[3]).children,
        vec![Node::Text(vec![TextPart::Literal(" x ".into())])]
    );
    assert_eq!(
        children[4],
        Node::Text(vec![TextPart::Literal("\n".into())])
    );
}

#[test]
fn authored_textarea_keeps_all_whitespace_including_empty_lines() {
    for body in ["\n  a\n  b\n", "\n\n", " \t "] {
        let nodes =
            crate::parser::parse_astro_module_template(&format!("<textarea>{body}</textarea>"))
                .unwrap();
        assert_eq!(
            element(&nodes[0]).children,
            vec![Node::Text(vec![TextPart::Literal(body.into())])]
        );
    }
}

#[test]
fn authored_root_inline_separators_survive_without_changing_legacy_formatting() {
    let nodes = crate::parser::parse_astro_module_template("<b>x</b> <i>y</i>").unwrap();
    assert_eq!(nodes[1], Node::Text(vec![TextPart::Literal(" ".into())]));
    let nodes = crate::parser::parse_astro_module_template("<div>\n <b>x</b>\n</div>").unwrap();
    assert_eq!(element(&nodes[0]).children.len(), 1);
    let legacy = parse("<pre>\n  a\n</pre>");
    assert_eq!(
        element(&legacy[0]).children,
        vec![Node::Text(vec![TextPart::Literal("a".into())])]
    );
}

#[test]
fn branch_markup_text_does_not_supply_javascript_punctuation() {
    for source in [
        "{ok && <p>Don't: really? a && b .map(x)</p>}",
        "{ok ? <p>Note: it's fine?</p> : <q>no: why?</q>}",
        "{items.map(item => <li>{item}'s: ready?</li>)}",
        "{ok ? (<p>it's: fine?</p>) : null}",
    ] {
        parse(source);
        crate::parser::parse_astro_module_template(source).unwrap();
    }
}

#[test]
fn nested_branch_markup_and_attributes_are_lexed_in_their_own_context() {
    parse(
        r#"{ok && <ul>{items.map(item => <li>{item ? <b title="a>b?:}">it's: good?</b> : "n/a"}</li>)}</ul>}"#,
    );
    parse("{ok ? <div><!-- it's: ? } --><br>don't: stop?</div> : null}");
}

#[test]
fn branch_scanning_keeps_js_strings_comparisons_and_comments() {
    for source in [
        "{left < right && <p>it's</p>}",
        "{ok && \"<b>it's: ?\"}",
        "{ok && /* it's } ?: */ <p>text</p>}",
        "{ok && // it's } ?: \n <p>text</p>}",
        "{ok ? `it's: ${value}?` : <p>none</p>}",
    ] {
        crate::parser::parse_astro_module_template(source).unwrap();
    }
}

#[test]
fn malformed_branch_markup_still_rejects_and_depth_recovers() {
    assert!(crate::parser::parse_astro_module_template("{ok && <p>don't}").is_err());
    let source = format!("{}x{}", "{ok && <p>".repeat(140), "</p>}".repeat(140));
    assert!(crate::parser::parse_astro_module_template(&source).is_err());
    parse("{ok && <p>it's fine</p>}");
}

#[test]
fn branch_scanner_keeps_supported_nesting_and_wide_sibling_sequences() {
    // Each nested logical tag consumes three frontend nesting levels.
    for depth in [1, 16, 64, 85] {
        let source = format!("{}x{}", "{ok && <p>".repeat(depth), "</p>}".repeat(depth));
        crate::parser::parse_astro_module_template(&source).unwrap();
    }
    let source = format!("{{ok && <div>{}</div>}}", "<b>it's: ready?</b>".repeat(256));
    let nodes = crate::parser::parse_astro_module_template(&source).unwrap();
    let Node::If(block) = &nodes[0] else {
        panic!("if block")
    };
    assert_eq!(element(&block.then_children[0]).children.len(), 256);
}

#[test]
fn template_substitutions_do_not_close_the_outer_expression() {
    for source in [
        r#"<p>{`a${`b}c${value}`}d`}</p>"#,
        r#"{ok ? `a${{key: `b}c`}.key}` : <p>no</p>}"#,
        r#"<p>{`a\`${`b\${literal}`}d`}</p>"#,
    ] {
        crate::parser::parse_astro_module_template(source).unwrap();
    }
}

#[test]
fn malformed_and_overdeep_template_substitutions_fail_without_poisoning_the_scanner() {
    assert!(crate::parser::parse_astro_module_template("<p>{`a${value}</p>").is_err());
    let source = format!("<p>{{{}x{}}}</p>", "`a${".repeat(150), "}`".repeat(150));
    assert!(crate::parser::parse_astro_module_template(&source).is_err());
    crate::parser::parse_astro_module_template("<p>{`ok${value}`}</p>").unwrap();
}

#[test]
fn nested_attribute_templates_use_expression_boundaries_without_interpolating_quoted_literals() {
    let nodes = crate::parser::parse_astro_module_template(
        r#"<p title={ `a${`b}c${value}`}d` } data-literal="{value}" />"#,
    )
    .unwrap();
    let el = element(&nodes[0]);
    assert_eq!(el.bindings[0].value, "`a${`b}c${value}`}d`");
    assert_eq!(el.bindings[1].origin, BindingOrigin::Literal);
    assert_eq!(el.bindings[1].value, "\"{value}\"");
}

#[test]
fn authored_spreads_preserve_attribute_order_without_admitting_legacy_spreads() {
    let source = "<Card {...first} label=\"explicit\" {...last}/>";
    let nodes = super::parse_astro_module_template(source).unwrap();
    let bindings = &element(&nodes[0]).bindings;
    assert_eq!(
        bindings
            .iter()
            .map(|binding| binding.prop.as_str())
            .collect::<Vec<_>>(),
        ["...", "label", "..."]
    );
    assert_eq!(bindings[0].value, "first");
    assert_eq!(bindings[2].value, "last");
    assert!(parse_err("<div {...first}/>").contains("spread attributes"));
}
