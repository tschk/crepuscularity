use std::path::Path;

use crate::ast::*;
use crate::parser::parse_template_with_path;

fn parse(src: &str) -> Vec<Node> {
    parse_template_with_path(src, Some(Path::new("app.component.html")))
        .expect("angular template should parse")
}

fn parse_err(src: &str) -> String {
    parse_template_with_path(src, Some(Path::new("app.component.html")))
        .expect_err("expected angular parse error")
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
fn mustache_interpolation_becomes_text_parts() {
    let nodes = parse(r#"<p>Hello {{ name }}!</p>"#);
    let Node::Text(parts) = &element(&nodes[0]).children[0] else {
        panic!("expected text");
    };
    assert!(matches!(&parts[0], TextPart::Literal(s) if s == "Hello "));
    assert!(matches!(&parts[1], TextPart::Expr(e) if e == "name"));
    assert!(matches!(&parts[2], TextPart::Literal(s) if s == "!"));
}

#[test]
fn ng_if_wraps_the_element_in_an_if_block() {
    let nodes = parse(r#"<div><p *ngIf="visible">hi</p></div>"#);
    let Node::If(block) = &element(&nodes[0]).children[0] else {
        panic!("expected if");
    };
    assert_eq!(block.condition, "visible");
    assert_eq!(element(&block.then_children[0]).tag, "p");
    assert!(block.else_children.is_none());
}

#[test]
fn ng_for_wraps_the_element_in_a_for_block() {
    let nodes = parse(r#"<li *ngFor="let item of items; trackBy: byId">{{ item }}</li>"#);
    let Node::For(block) = &nodes[0] else {
        panic!("expected for");
    };
    assert_eq!(block.pattern, "item");
    assert_eq!(block.iterator, "items");
    assert_eq!(text_of(&element(&block.body[0]).children[0]), "{item}");
}

#[test]
fn property_and_event_bindings() {
    let nodes = parse(r#"<button [disabled]="busy" (click)="save()" title="go">+</button>"#);
    let el = element(&nodes[0]);
    assert_eq!(el.bindings[0].prop, "disabled");
    assert_eq!(el.bindings[0].value, "busy");
    assert_eq!(el.bindings[1].prop, "title");
    assert_eq!(el.bindings[1].value, "\"go\"");
    assert_eq!(el.event_handlers[0].event, "click");
    assert_eq!(el.event_handlers[0].handler, "save()");
    assert!(el.event_handlers[0].modifiers.is_empty());
}

#[test]
fn two_way_ng_model_becomes_a_bind_binding() {
    let nodes = parse(r#"<input type="text" [(ngModel)]="query" />"#);
    let el = element(&nodes[0]);
    assert!(el
        .bindings
        .iter()
        .any(|b| b.prop == "bind" && b.value == "query"));
}

#[test]
fn class_bindings_lower_to_conditional_and_dynamic_classes() {
    let nodes = parse(
        r#"<div class="base" [class.active]="isOn"></div>
<div [ngClass]="{ danger: bad, 'a b': other }"></div>
<div [ngClass]="['x', dyn]"></div>"#,
    );
    let first = element(&nodes[0]);
    assert_eq!(first.classes, vec!["base".to_string()]);
    assert_eq!(first.conditional_classes[0].class, "active");
    assert_eq!(first.conditional_classes[0].condition, "isOn");

    let obj = element(&nodes[1]);
    assert_eq!(obj.conditional_classes.len(), 3);
    assert_eq!(obj.conditional_classes[0].class, "danger");
    assert_eq!(obj.conditional_classes[0].condition, "bad");
    assert_eq!(obj.conditional_classes[1].class, "a");

    let arr = element(&nodes[2]);
    assert_eq!(arr.classes, vec!["x".to_string(), "{dyn}".to_string()]);
}

#[test]
fn attribute_and_inner_html_bindings() {
    let nodes = parse(r#"<div [attr.aria-label]="label" [innerHTML]="body">dropped</div>"#);
    let el = element(&nodes[0]);
    assert_eq!(el.bindings[0].prop, "aria-label");
    assert_eq!(el.bindings[0].value, "label");
    assert!(matches!(&el.children[0], Node::RawHtml(e) if e == "body"));
}

#[test]
fn interpolated_attribute_value_becomes_a_binding() {
    let nodes = parse(r#"<a href="{{ url }}">x</a>"#);
    let el = element(&nodes[0]);
    assert_eq!(el.bindings[0].prop, "href");
    assert_eq!(el.bindings[0].value, "url");
}

#[test]
fn ng_container_contributes_children_only() {
    let nodes = parse(r#"<div><ng-container *ngIf="ok"><b>a</b><i>b</i></ng-container></div>"#);
    let Node::If(block) = &element(&nodes[0]).children[0] else {
        panic!("expected if");
    };
    assert_eq!(block.then_children.len(), 2);
    assert_eq!(element(&block.then_children[0]).tag, "b");
}

#[test]
fn control_flow_if_else_if_else_blocks() {
    let nodes = parse(
        r#"<div>
  @if (a) { <p>A</p> } @else if (b) { <p>B</p> } @else { <p>C</p> }
</div>"#,
    );
    let Node::If(outer) = &element(&nodes[0]).children[0] else {
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
fn control_flow_for_block() {
    let nodes = parse(r#"<ul>@for (item of items; track item.id) { <li>{{ item }}</li> }</ul>"#);
    let Node::For(block) = &element(&nodes[0]).children[0] else {
        panic!("expected for");
    };
    assert_eq!(block.pattern, "item");
    assert_eq!(block.iterator, "items");
    assert_eq!(element(&block.body[0]).tag, "li");
}

#[test]
fn nested_control_flow_blocks() {
    let nodes = parse(r#"@for (x of xs; track x) { @if (x.on) { <b>{{ x.n }}</b> } }"#);
    let Node::For(block) = &nodes[0] else {
        panic!("expected for");
    };
    let Node::If(inner) = &block.body[0] else {
        panic!("expected nested if, got {:?}", block.body[0]);
    };
    assert_eq!(inner.condition, "x.on");
    assert_eq!(element(&inner.then_children[0]).tag, "b");
}

#[test]
fn void_elements_and_comments_are_handled() {
    let nodes = parse(
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
fn unsupported_constructs_report_errors() {
    for (src, needle) in [
        (r#"<ng-template><p>x</p></ng-template>"#, "<ng-template>"),
        (r#"<ng-content></ng-content>"#, "<ng-content>"),
        (r#"<div *ngSwitch="v"></div>"#, "ngSwitch"),
        (r#"<p *ngIf="a; else other">x</p>"#, "else"),
        (r#"<div [ngStyle]="s"></div>"#, "ngStyle"),
        (r#"<div [style.width]="w"></div>"#, "style.width"),
        (r#"<input #ref />"#, "template reference variable"),
        (r#"<button (keyup.enter)="go()"></button>"#, "pseudo-event"),
        (r#"<div [(foo)]="v"></div>"#, "two-way binding"),
        (
            r#"<li *ngFor="let item of items; let i = index"></li>"#,
            "for-loop head",
        ),
        (r#"<a href="a-{{ b }}-c">x</a>"#, "interpolation mixed"),
        (r#"@switch (v) { }"#, "@switch"),
        (
            r#"@for (x of xs; track x) { <b>a</b> } @empty { <i>none</i> }"#,
            "@empty",
        ),
        (r#"@else { <p>x</p> }"#, "without a preceding"),
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
fn dispatcher_keys_on_the_angular_naming_conventions() {
    for name in ["app.component.html", "app.ng.html", "app.ng"] {
        let nodes = parse_template_with_path(r#"<p>{{ x }}</p>"#, Some(Path::new(name)))
            .unwrap_or_else(|e| panic!("{name} should use the angular frontend: {e}"));
        assert_eq!(text_of(&element(&nodes[0]).children[0]), "{x}");
    }
}

#[test]
fn plain_html_paths_do_not_use_the_angular_frontend() {
    let err = parse_template_with_path(r#"<div *ngIf="a"></div>"#, Some(Path::new("index.html")));
    // The JSX frontend accepts `*ngIf` as an ordinary attribute, so a plain
    // `.html` file is not claimed by this frontend.
    let nodes = err.expect("jsx frontend should handle plain html");
    assert_eq!(element(&nodes[0]).tag, "div");
}

#[test]
fn ng_model_requires_nonempty_binding_expressions() {
    for attribute in [
        "[(ngModel)]",
        "[(ngModel)]=\"\"",
        "[(ngModel)]=\"   \"",
        "[(ngModel)]={}",
    ] {
        let error = parse_err(&format!("<input {attribute} />"));
        assert!(
            error.contains("[(ngModel)]` requires an expression"),
            "{error}"
        );
    }
    for expression in ["query", "form.query"] {
        let nodes = parse(&format!("<input [(ngModel)]=\"{expression}\" />"));
        assert_eq!(element(&nodes[0]).bindings[0].value, expression);
    }
}

#[test]
fn property_bindings_require_names_and_expressions() {
    for attribute in [
        "[title]",
        "[title]=\"\"",
        "[class.active]=\"   \"",
        "[ngClass]={}",
        "[innerHTML]=\"\"",
        "[]=\"value\"",
    ] {
        let error = parse_err(&format!("<div {attribute} />"));
        assert!(
            error.contains("property name and a nonempty expression"),
            "{error}"
        );
    }
    let error = parse_err(r#"<div [attr.]="value" />"#);
    assert!(error.contains("missing an attribute name"), "{error}");
    for property in [
        "title",
        "attr.title",
        "class.active",
        "ngClass",
        "innerText",
    ] {
        for expression in ["''", "0", "false"] {
            parse(&format!("<div [{property}]=\"{expression}\" />"));
        }
    }
}

#[test]
fn content_bindings_cannot_override_each_other() {
    let properties = ["innerHTML", "innerText", "textContent"];
    for first in properties {
        for second in properties {
            let source = format!("<div [{first}]=\"first\" [{second}]=\"second\" />");
            let error = parse_err(&source);
            assert!(
                error.contains("only one content binding"),
                "{first}/{second}: {error}"
            );
        }
        let nodes = parse(&format!("<div [{first}]=\"content\" />"));
        let children = &element(&nodes[0]).children;
        if first == "innerHTML" {
            assert!(matches!(&children[0], Node::RawHtml(expr) if expr == "content"));
        } else {
            assert!(
                matches!(&children[0], Node::Text(parts) if matches!(&parts[0], TextPart::Expr(expr) if expr == "content"))
            );
        }
    }
}

#[test]
fn duplicate_structural_directives_are_parse_errors() {
    for (attributes, expected) in [
        (
            r#"*ngIf="first" *ngIf="second""#,
            "only one *ngIf directive",
        ),
        (
            r#"*ngFor="let item of items" *ngFor="let other of others""#,
            "only one *ngFor directive",
        ),
    ] {
        let error = parse_err(&format!("<div {attributes} />"));
        assert!(error.contains(expected), "{error}");
    }
    parse(r#"<div *ngIf="visible"><span *ngIf="enabled" /></div>"#);
    parse(r#"<div *ngFor="let item of items"><span *ngFor="let other of others" /></div>"#);
}

#[test]
fn attribute_interpolation_rejects_empty_expressions() {
    for value in ["{{}}", "{{   }}", "  {{ }}  "] {
        let error = parse_err(&format!("<input title=\"{value}\" />"));
        assert!(
            error.contains("attribute interpolation requires an expression"),
            "{error}"
        );
    }
    for expression in ["''", "0", "false", "name"] {
        let nodes = parse(&format!("<input title=\"{{{{ {expression} }}}}\" />"));
        assert_eq!(element(&nodes[0]).bindings[0].value, expression);
    }
    let nodes = parse(r#"<input title="" />"#);
    assert_eq!(element(&nodes[0]).bindings[0].value, "\"\"");
}

#[test]
fn loop_tracking_keywords_cannot_hide_unsupported_clauses() {
    for clause in ["tracker: name", "trackByAlias: name", "tracking item.id"] {
        let error = parse_err(&format!("<div *ngFor=\"let item of items; {clause}\" />"));
        assert!(error.contains("not supported"), "{error}");
    }
    let error = parse_err(r#"<div *ngFor="let item of items; trackBy:" />"#);
    assert!(error.contains("tracking requires an expression"), "{error}");
    parse(r#"<div *ngFor="let item of items; trackBy: byId" />"#);
    parse("@for (item of items; track item.id) { <div /> }");
}

#[test]
fn loop_head_semicolons_inside_escaped_strings_are_not_clauses() {
    for iterator in [r#"'snow\';雪'"#, r#""snow\";雪""#, r#"'backslash\\'"#] {
        let head = format!("item of {iterator}; track item.id");
        let (pattern, parsed_iterator) = super::builders::parse_for_head(&head).unwrap();
        assert_eq!(pattern, "item");
        assert_eq!(parsed_iterator, iterator);
    }
    let error =
        super::builders::parse_for_head(r#"item of 'snow\';雪'; unsupported alias"#).unwrap_err();
    assert!(error.message.contains("unsupported alias"), "{error:?}");
}

#[test]
fn angular_for_blocks_preserve_quoted_semicolons_in_iterators() {
    for iterator in [r#"'snow\';雪'"#, r#""snow\";雪""#] {
        let source =
            format!("@for (item of {iterator}; track item.id) {{ <span>{{{{ item }}}}</span> }}");
        let nodes = parse(&source);
        let Node::For(block) = &nodes[0] else {
            panic!("expected loop")
        };
        assert_eq!(block.pattern, "item");
        assert_eq!(block.iterator, iterator);
    }
    let error = parse_err(r#"@for (item of 'snow\';雪'; unsupported alias) { <span /> }"#);
    assert!(error.contains("unsupported alias"), "{error}");
}

#[test]
fn event_bindings_require_handler_expressions() {
    for attribute in ["(click)", "(click)=\"\"", "(click)=\"   \"", "(click)={}"] {
        let error = parse_err(&format!("<button {attribute} />"));
        assert!(error.contains("requires a handler expression"), "{error}");
    }
    for handler in ["save", "save()", "form.save()"] {
        let nodes = parse(&format!("<button (click)=\"{handler}\" />"));
        assert_eq!(element(&nodes[0]).event_handlers[0].handler, handler);
    }
}

#[test]
fn id_aliases_cannot_emit_duplicate_attributes() {
    for first in [r#"id="first""#, r#"[id]="first""#, r#"[attr.id]="first""#] {
        for second in [
            r#"id="second""#,
            r#"[id]="second""#,
            r#"[attr.id]="second""#,
        ] {
            let error = parse_err(&format!("<div {first} {second} />"));
            assert!(
                error.contains("only one id attribute or binding"),
                "{error}"
            );
        }
        parse(&format!("<div {first} />"));
    }
}

#[test]
fn interpolated_ids_use_attribute_binding_rules() {
    let nodes = parse(r#"<div id="{{ name }}" />"#);
    let element = element(&nodes[0]);
    assert!(element.id.is_none());
    assert_eq!(element.bindings[0].prop, "id");
    assert_eq!(element.bindings[0].value, "name");
    assert!(parse_err(r#"<div id="{{ }}" />"#).contains("requires an expression"));
    assert!(parse_err(r#"<div id="prefix-{{ name }}" />"#).contains("interpolation mixed"));
}

#[test]
fn interpolated_classes_use_attribute_binding_rules() {
    let nodes = parse(r#"<div class="{{ theme }}" />"#);
    assert_eq!(element(&nodes[0]).classes, ["{theme}"]);
    assert!(parse_err(r#"<div class="{{ }}" />"#).contains("requires an expression"));
    assert!(parse_err(r#"<div class="base {{ theme }}" />"#).contains("interpolation mixed"));
}

#[test]
fn global_event_targets_fail_instead_of_becoming_local_event_names() {
    for filename in ["app.component.html", "app.ng.html", "app.ng"] {
        for event in ["window:resize", "document:click", "body:keydown"] {
            let source = format!("<button ({event})=\"handle()\">Action</button>");
            let error = parse_template_with_path(&source, Some(Path::new(filename)))
                .expect_err("global target listeners need an explicit unsupported boundary")
                .to_string();
            assert!(
                error.contains("global event target"),
                "{filename}: {event}: {error}"
            );
        }
        for event in ["click", "keydown", "custom-event"] {
            let source = format!("<button ({event})=\"handle()\">Action</button>");
            let nodes = parse_template_with_path(&source, Some(Path::new(filename))).unwrap();
            let handler = &element(&nodes[0]).event_handlers[0];
            assert_eq!(handler.event, event);
            assert_eq!(handler.handler, "handle()");
        }
    }
}

#[test]
fn else_if_chains_obey_the_angular_nesting_limit() {
    let short = "@if (false) {<p>first</p>} @else if (true) {<p>second</p>} @else {<p>last</p>}";
    let nodes = parse(short);
    let Node::If(first) = &nodes[0] else {
        panic!("expected initial condition");
    };
    let Some([Node::If(second)]) = first.else_children.as_deref() else {
        panic!("expected nested else-if");
    };
    let ctx = crate::TemplateContext::new();
    assert!(!ctx.eval_condition(&first.condition).unwrap());
    assert!(ctx.eval_condition(&second.condition).unwrap());
    assert_eq!(
        text_of(&element(&second.then_children[0]).children[0]),
        "second"
    );

    let mut source = "@if (false) {}".to_string();
    for _ in 0..=super::MAX_ANGULAR_DEPTH {
        source.push_str(" @else if (false) {}");
    }
    source.push_str(" @else {<p>last</p>}");
    assert!(parse_err(&source).contains("maximum Angular nesting depth"));
}

#[test]
fn if_alias_detection_ignores_quoted_condition_contents() {
    let ctx = crate::TemplateContext::new();
    for condition in [
        r#"' as ' == ' as '"#,
        r#"" as " == " as ""#,
        r#"'; as alias' == '; as alias'"#,
        r#"true || ' as '"#,
        r#"(true || ' as ')"#,
    ] {
        let nodes = parse(&format!("@if ({condition}) {{<p>yes</p>}}"));
        let Node::If(block) = &nodes[0] else {
            panic!("expected condition");
        };
        assert_eq!(block.condition, condition);
        assert!(ctx.eval_condition(&block.condition).unwrap(), "{condition}");
    }
}

#[test]
fn if_aliases_remain_unsupported_with_whitespace_variants() {
    for condition in [
        "true; as value",
        "true;as value",
        "true; as\tvalue",
        "true;\nas value",
        "true;\tas\nvalue",
    ] {
        let error = parse_err(&format!("@if ({condition}) {{<p>{{{{ value }}}}</p>}}"));
        assert!(
            error.contains("aliases are not supported"),
            "{condition:?}: {error}"
        );
    }
}
