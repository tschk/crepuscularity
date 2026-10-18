use crepuscularity_core::parse_template;

#[test]
fn else_if_branches_validate_condition_expressions() {
    for attributes in ["", "condition={}", "test={   }", "test={true} cond={false}"] {
        let source =
            format!("<if test={{false}}><span /><else-if {attributes}><span /></else-if></if>");
        let error = parse_template(&source).unwrap_err();
        assert!(
            error.to_string().contains("condition expression"),
            "{error}"
        );
    }
    assert!(parse_template(
        "<if test={false}><span /><else-if test={true}><span /></else-if></if>"
    )
    .is_ok());
}

#[test]
fn else_branches_reject_ignored_attributes() {
    for branch in [
        "<else condition={true} />",
        "<else title=\"ignored\"><span /></else>",
    ] {
        let error =
            parse_template(&format!("<if test={{false}}><span />{branch}</if>")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("else branches do not accept attributes"),
            "{error}"
        );
    }
    assert!(parse_template("<if test={false}><span /><else /></if>").is_ok());
}

#[test]
fn condition_tags_require_one_nonempty_condition() {
    for attributes in [
        "",
        "condition={}",
        "test={   }",
        "condition={true} test={false}",
    ] {
        let error =
            parse_template(&format!("<if {attributes}><span>Lost</span></if>")).unwrap_err();
        assert!(
            error.to_string().contains("condition expression"),
            "{error}"
        );
    }
    assert!(parse_template("<if test={false}><span>Hidden</span></if>").is_ok());
}

#[test]
fn declarations_must_be_self_closing() {
    for tag in ["let", "let-default"] {
        let error = parse_template(&format!(
            "<{tag} name=\"count\" value={{1}}><span>Lost</span></{tag}>"
        ))
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("declarations must be self-closing"),
            "{error}"
        );
    }
}

#[test]
fn declarations_require_one_nonempty_value_expression() {
    for tag in ["let", "let-default"] {
        for value in ["", "value={}", "value={   }", "value={1} value={2}"] {
            let error = parse_template(&format!("<{tag} name=\"count\" {value} />")).unwrap_err();
            assert!(error.to_string().contains("value expression"), "{error}");
        }
        for value in ["value=\"\"", "value={0}", "value={false}"] {
            assert!(parse_template(&format!("<{tag} name=\"count\" {value} />")).is_ok());
        }
    }
}

#[test]
fn declaration_tags_require_one_literal_name() {
    for tag in ["let", "let-default"] {
        for attributes in [
            "",
            "name",
            "name={variable}",
            "name=\"\"",
            "name=\"a\" name=\"b\"",
        ] {
            let error = parse_template(&format!("<{tag} {attributes} value={{1}} />")).unwrap_err();
            assert!(
                error.to_string().contains("literal variable name"),
                "{error}"
            );
        }
        assert!(parse_template(&format!("<{tag} name=\"count\" value={{1}} />")).is_ok());
    }
}

#[test]
fn islands_reject_unsupported_child_content() {
    for tag in ["island", "crepus-island"] {
        let error = parse_template(&format!(
            "<{tag} src=\"module.js\"><span>Lost</span></{tag}>"
        ))
        .unwrap_err();
        assert!(error.to_string().contains("islands must be self-closing"));
    }
    assert!(parse_template("<include src=\"child.crepus\"><span>Slot</span></include>").is_ok());
}

#[test]
fn island_adapters_must_be_nonempty_literals() {
    for tag in ["island", "crepus-island"] {
        for adapter in [
            "adapter",
            "adapter={name}",
            "adapter=\"\"",
            "adapter=\"   \"",
        ] {
            let error =
                parse_template(&format!("<{tag} src=\"module.js\" {adapter} />")).unwrap_err();
            assert!(error
                .to_string()
                .contains("adapter must be a nonempty literal"));
        }
        assert!(parse_template(&format!("<{tag} src=\"module.js\" adapter=\"module\" />")).is_ok());
        assert!(parse_template(&format!("<{tag} src=\"module.js\" />")).is_ok());
    }
}

#[test]
fn directive_tags_require_one_literal_source_path() {
    for tag in ["include", "island", "crepus-island"] {
        for attributes in ["", "src=\"\"", "src={name}", "src", "src=\"a\" path=\"b\""] {
            let error = parse_template(&format!("<{tag} {attributes} />")).unwrap_err();
            assert!(error.to_string().contains("source path"), "{error}");
        }
        assert!(parse_template(&format!("<{tag} path=\"child.crepus\" />")).is_ok());
    }
}

#[test]
fn directive_tags_reject_duplicate_attributes() {
    for tag in ["include", "island", "crepus-island"] {
        for attributes in [
            "src=\"a\" src=\"b\"",
            "src=\"a\" title=\"one\" title=\"two\"",
        ] {
            let error = parse_template(&format!("<{tag} {attributes} />")).unwrap_err();
            assert!(
                error.to_string().contains("duplicate directive attribute"),
                "{error}"
            );
        }
    }
}

#[test]
fn match_tags_require_one_subject_expression() {
    for attributes in [
        "",
        "on={}",
        "value={   }",
        "on={1} value={2}",
        "on={1} on={2}",
    ] {
        let error = parse_template(&format!("<match {attributes}><case /></match>")).unwrap_err();
        assert!(error.to_string().contains("subject expression"), "{error}");
    }
    for attributes in ["on={0}", "value={false}", "on=\"\""] {
        assert!(parse_template(&format!("<match {attributes}><case /></match>")).is_ok());
    }
}

#[test]
fn for_tags_validate_variable_and_iterator_attributes() {
    for attributes in [
        "",
        "let",
        "let={item}",
        "let=\"\"",
        "var=\"   \"",
        "let=\"a\" var=\"b\"",
        "let=\"a\" let=\"b\"",
    ] {
        let error =
            parse_template(&format!("<for {attributes} in={{items}}><span /></for>")).unwrap_err();
        assert!(error.to_string().contains("loop variable"), "{error}");
    }
    for attributes in ["", "in={}", "in={   }", "in={a} in={b}"] {
        let error =
            parse_template(&format!("<for let=\"item\" {attributes}><span /></for>")).unwrap_err();
        assert!(error.to_string().contains("iterator expression"), "{error}");
    }
    for variable in ["let", "var"] {
        assert!(parse_template(&format!(
            "<for {variable}=\"item\" in={{items}}><span>{{item}}</span></for>"
        ))
        .is_ok());
    }
}

#[test]
fn supplied_case_patterns_cannot_silently_become_wildcards() {
    for attributes in [
        "pattern",
        "pattern={}",
        "when={   }",
        "match=\"\"",
        "pattern=\"   \"",
        "pattern=\"1\" when={2}",
        "pattern={1} pattern={2}",
    ] {
        let error =
            parse_template(&format!("<match on={{1}}><case {attributes} /></match>")).unwrap_err();
        assert!(error.to_string().contains("case requires"), "{error}");
    }
    for attributes in [
        "",
        "pattern=\"_\"",
        "pattern={0}",
        "when={false}",
        "match=\"1\"",
    ] {
        assert!(parse_template(&format!("<match on={{1}}><case {attributes} /></match>")).is_ok());
    }
}

#[test]
fn jsx_events_require_names_for_paired_and_self_closing_elements() {
    for source in ["<button @={save} />", "<button @|once={save}></button>"] {
        let error = parse_template(source).unwrap_err();
        assert!(error.to_string().contains("event name"), "{error}");
    }
    for source in [
        "<button @click={save} />",
        "<button @click|once={save}></button>",
    ] {
        assert!(parse_template(source).is_ok());
    }
}

#[test]
fn react_event_aliases_use_dom_names_and_capture_modifiers() {
    for (attribute, event, capture) in [
        ("onKeyDown", "keydown", false),
        ("onKeyUp", "keyup", false),
        ("onMouseDown", "mousedown", false),
        ("onDoubleClick", "dblclick", false),
        ("onClickCapture", "click", true),
        ("onKeyDownCapture", "keydown", true),
        ("onGotPointerCapture", "gotpointercapture", false),
        ("onGotPointerCaptureCapture", "gotpointercapture", true),
        ("onCustomThing", "customThing", false),
        ("onAnimationIterationCapture", "animationiteration", true),
        ("onLoadedMetadata", "loadedmetadata", false),
        ("onPointerCancel", "pointercancel", false),
    ] {
        let nodes = parse_template(&format!("<button {attribute}={{handle}} />")).unwrap();
        let crepuscularity_core::ast::Node::Element(element) = &nodes[0] else {
            panic!("expected element")
        };
        assert_eq!(element.event_handlers[0].event, event, "{attribute}");
        assert_eq!(
            element.event_handlers[0].modifiers,
            if capture { vec!["capture"] } else { vec![] },
            "{attribute}"
        );
    }
}

#[test]
fn shared_events_require_handlers_and_nonempty_modifier_entries() {
    use std::path::Path;
    for filename in ["Event.jsx", "Event.svelte", "Event.astro"] {
        for attribute in [
            "@click",
            "@click={}",
            "@click=\"\"",
            "@click=\"   \"",
            "onClick",
            "onClick={}",
            "onClick=\"\"",
            "@keydown|={save}",
            "@keydown||Enter={save}",
        ] {
            for source in [
                format!("<button {attribute} />"),
                format!("<button {attribute}></button>"),
            ] {
                assert!(
                    crepuscularity_core::parse_template_with_path(
                        &source,
                        Some(Path::new(filename))
                    )
                    .is_err(),
                    "{filename}: {source}"
                );
            }
        }
        for attribute in [
            "@click=\"save\"",
            "@click='save'",
            "onClick=\"save\"",
            "onClick={save}",
            "@click={save}",
        ] {
            let nodes = crepuscularity_core::parse_template_with_path(
                &format!("<button {attribute} />"),
                Some(Path::new(filename)),
            )
            .unwrap();
            let crepuscularity_core::ast::Node::Element(element) = &nodes[0] else {
                panic!("expected element")
            };
            assert_eq!(
                element.event_handlers[0].handler, "save",
                "{filename}: {attribute}"
            );
        }
    }
}

#[test]
fn shared_event_validation_reaches_all_element_builder_frontends() {
    use crepuscularity_core::{ast::Node, parse_template_with_path};
    use std::path::Path;
    for filename in ["Event.jsx", "Event.svelte", "Event.astro"] {
        for source in ["<button @={save} />", "<button @|once={save}></button>"] {
            let error = parse_template_with_path(source, Some(Path::new(filename))).unwrap_err();
            assert!(
                error.to_string().contains("event name"),
                "{filename}: {error}"
            );
        }
        for source in [
            "<button @click={save} />",
            "<button @click|once={save}></button>",
        ] {
            let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("expected element")
            };
            assert_eq!(element.event_handlers[0].event, "click");
            assert_eq!(element.event_handlers[0].handler, "save");
        }
    }
}

#[test]
fn shared_binding_directives_require_property_names() {
    use crepuscularity_core::{ast::Node, parse_template_with_path};
    use std::path::Path;
    for filename in ["Binding.jsx", "Binding.astro"] {
        for source in ["<input bind:={value} />", "<div bind:={value}></div>"] {
            let error = parse_template_with_path(source, Some(Path::new(filename))).unwrap_err();
            assert!(
                error.to_string().contains("property name"),
                "{filename}: {error}"
            );
        }
        let nodes =
            parse_template_with_path("<input bind:value={value} />", Some(Path::new(filename)))
                .unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("expected element")
        };
        assert_eq!(element.bindings[0].prop, "value");
        assert_eq!(element.bindings[0].value, "value");
    }
}

#[test]
fn jsx_conditional_classes_require_names_and_conditions() {
    for source in [
        "<div class:={active} />",
        "<div class:active={} />",
        "<div class:active={   }></div>",
    ] {
        let error = parse_template(source).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("conditional class directives require"),
            "{error}"
        );
    }
    for source in [
        "<div class:active={false} />",
        "<div class:active={0} />",
        "<div class:active />",
    ] {
        assert!(parse_template(source).is_ok());
    }
}

#[test]
fn when_classes_cannot_silently_drop_empty_conditions() {
    for source in ["<div when:=\"active\" />", "<div when:{}=\"active\"></div>"] {
        let error = parse_template(source).unwrap_err();
        assert!(
            error.to_string().contains("when directives require"),
            "{error}"
        );
    }
    use crepuscularity_core::ast::Node;
    let nodes = parse_template("<div when:active=\"one two\" />").unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!("expected element")
    };
    assert_eq!(element.conditional_classes.len(), 2);
    assert!(element
        .conditional_classes
        .iter()
        .all(|class| class.condition == "active"));
}

#[test]
fn when_classes_reject_unsupported_dynamic_or_boolean_values() {
    for source in [
        "<div when:active />",
        "<div when:active={classes} />",
        "<div when:active={}></div>",
    ] {
        let error = parse_template(source).unwrap_err();
        assert!(error.to_string().contains("literal class list"), "{error}");
    }
    for source in [
        "<div when:active=\"one two\" />",
        "<div when:active=\"\" />",
    ] {
        assert!(parse_template(source).is_ok());
    }
}

#[test]
fn animation_directives_require_property_names() {
    for source in ["<div animate:={300ms} />", "<div animate:={300ms}></div>"] {
        let error = parse_template(source).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("animation directives require a property name"),
            "{error}"
        );
    }
    use crepuscularity_core::ast::Node;
    let nodes = parse_template("<div animate:opacity={300ms linear repeat} />").unwrap();
    let Node::Element(element) = &nodes[0] else {
        panic!("expected element")
    };
    let animation = &element.animations[0];
    assert_eq!(animation.property, "opacity");
    assert_eq!(animation.duration_expr, "300ms");
    assert_eq!(animation.easing, "linear");
    assert!(animation.repeat);
}

#[test]
fn quoted_and_braced_animation_values_have_identical_tokens() {
    use crepuscularity_core::ast::Node;
    for value in ["\"300ms ease-in-out repeat\"", "{300ms ease-in-out repeat}"] {
        let nodes = parse_template(&format!("<div animate:opacity={value} />")).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("expected element")
        };
        let animation = &element.animations[0];
        assert_eq!(animation.duration_expr, "300ms");
        assert_eq!(animation.easing, "ease-in-out");
        assert!(animation.repeat);
    }
}

#[test]
fn animation_settings_cannot_silently_drop_trailing_tokens() {
    for value in [
        "{300ms linear forever}",
        "{300ms linear repeat extra}",
        "\"300ms linear repeat extra\"",
    ] {
        let error = parse_template(&format!("<div animate:opacity={value} />")).unwrap_err();
        assert!(
            error.to_string().contains("optional repeat only"),
            "{error}"
        );
    }
    for value in ["{300ms}", "{300ms linear}", "{300ms linear repeat}"] {
        assert!(parse_template(&format!("<div animate:opacity={value} />")).is_ok());
    }
}

#[test]
fn shared_bindings_reject_empty_braces_but_preserve_empty_literals() {
    use crepuscularity_core::{ast::Node, parse_template_with_path};
    use std::path::Path;
    for filename in ["Binding.jsx", "Binding.astro"] {
        for source in ["<input bind:value={} />", "<input bind:value={   } />"] {
            let error = parse_template_with_path(source, Some(Path::new(filename))).unwrap_err();
            assert!(
                error.to_string().contains("nonempty expression"),
                "{filename}: {error}"
            );
        }
        for value in ["{0}", "{false}", "{''}", "\"\""] {
            let source = format!("<input bind:value={value} />");
            let nodes = parse_template_with_path(&source, Some(Path::new(filename))).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("expected element")
            };
            assert!(!element.bindings[0].value.is_empty());
        }
    }
}

#[test]
fn vue_and_angular_reject_ignored_jsx_class_expressions() {
    use crepuscularity_core::parse_template_with_path;
    use std::path::Path;
    for (filename, prefix, suffix, guidance) in [
        ("Classes.vue", "<template>", "</template>", ":class"),
        ("Classes.ng.html", "", "", "[class]"),
    ] {
        for expression in ["theme", "'active'", "false"] {
            let source = format!("{prefix}<div class={{{expression}}} />{suffix}");
            let error = parse_template_with_path(&source, Some(Path::new(filename))).unwrap_err();
            assert!(error.to_string().contains(guidance), "{filename}: {error}");
        }
    }
}

#[test]
fn vue_and_angular_keep_literal_empty_and_native_class_bindings() {
    use crepuscularity_core::{ast::Node, parse_template_with_path};
    use std::path::Path;
    for (filename, prefix, suffix, binding) in [
        ("Classes.vue", "<template>", "</template>", ":class"),
        ("Classes.ng.html", "", "", "[class]"),
    ] {
        for (attribute, expected) in [
            ("class=\"base\"".to_string(), vec!["base"]),
            ("class".to_string(), vec![]),
            (format!("{binding}=\"theme\""), vec!["{theme}"]),
        ] {
            let source = format!("{prefix}<div {attribute} />{suffix}");
            let nodes = parse_template_with_path(&source, Some(Path::new(filename))).unwrap();
            let Node::Element(element) = &nodes[0] else {
                panic!("expected element")
            };
            assert_eq!(element.classes, expected, "{filename}: {attribute}");
        }
    }
}

#[test]
fn malformed_class_list_strings_fail_through_public_frontends() {
    use crepuscularity_core::parse_template_with_path;
    use std::path::Path;
    for (filename, source) in [
        (
            "Classes.vue",
            r#"<template><div :class="[']" /></template>"#,
        ),
        ("Classes.ng.html", r#"<div [ngClass]="[']" />"#),
        ("Classes.astro", r#"<div class:list={[']} />"#),
    ] {
        let result = std::panic::catch_unwind(|| {
            parse_template_with_path(source, Some(Path::new(filename)))
        });
        let error = result
            .expect("malformed class list must not panic")
            .unwrap_err();
        if !filename.ends_with(".astro") {
            assert!(
                error.to_string().contains("closing quote"),
                "{filename}: {error}"
            );
        }
    }
}

#[test]
fn dynamic_class_attributes_reject_empty_braces() {
    for attribute in ["class={}", "className={   }"] {
        let error = parse_template(&format!("<div {attribute} />")).unwrap_err();
        assert!(
            error.to_string().contains("class attributes require"),
            "{error}"
        );
    }
    for attribute in ["class=\"\"", "className={theme}", "class={false}"] {
        assert!(parse_template(&format!("<div {attribute} />")).is_ok());
    }
}

#[test]
fn id_attributes_reject_missing_or_empty_expressions() {
    for source in ["<div id />", "<div id={} />"] {
        let error = parse_template(source).unwrap_err();
        assert!(
            error.to_string().contains("id attributes require"),
            "{error}"
        );
    }
}

#[test]
fn duplicate_static_and_dynamic_ids_are_rejected() {
    use crepuscularity_core::parse_template_with_path;
    use std::path::Path;
    for filename in ["Ids.jsx", "Ids.svelte", "Ids.astro"] {
        for attributes in [
            r#"id="first" id="second""#,
            r#"id="first" id={second}"#,
            r#"id={first} id="second""#,
            "id={first} id={second}",
        ] {
            let error = parse_template_with_path(
                &format!("<div {attributes} />"),
                Some(Path::new(filename)),
            )
            .unwrap_err();
            assert!(
                error.to_string().contains("only one id attribute"),
                "{filename}: {error}"
            );
        }
    }
}

#[test]
fn bound_id_aliases_cannot_duplicate_static_or_dynamic_ids() {
    for attributes in [
        r#"id="first" bind:id={second}"#,
        r#"bind:id={first} id="second""#,
        "id={first} bind:id={second}",
        "bind:id={first} bind:id={second}",
    ] {
        let error = parse_template(&format!("<div {attributes} />")).unwrap_err();
        assert!(
            error.to_string().contains("only one id attribute"),
            "{error}"
        );
    }
    assert!(parse_template("<div bind:id={name} />").is_ok());
}

#[test]
fn vue_and_angular_ids_cannot_silently_drop_braced_values() {
    use crepuscularity_core::{ast::Node, parse_template_with_path};
    use std::path::Path;
    for filename in ["Ids.vue", "Ids.ng.html"] {
        for source in ["<div id />", "<div id={} />"] {
            let source = if filename.ends_with(".vue") {
                format!("<template>{source}</template>")
            } else {
                source.to_string()
            };
            let error = parse_template_with_path(&source, Some(Path::new(filename))).unwrap_err();
            assert!(
                error.to_string().contains("id attributes require"),
                "{error}"
            );
        }
        let source = if filename.ends_with(".vue") {
            "<template><div id={name} /></template>"
        } else {
            "<div id={name} />"
        };
        let nodes = parse_template_with_path(source, Some(Path::new(filename))).unwrap();
        let Node::Element(element) = &nodes[0] else {
            panic!("expected element")
        };
        assert_eq!(element.bindings[0].prop, "id");
        assert_eq!(element.bindings[0].value, "name");
    }
}

#[test]
fn jsx_text_nodes_require_nonempty_expressions() {
    for source in ["<div>{}</div>", "<div>{   }</div>", "<div>{=}</div>"] {
        assert!(
            crepuscularity_core::parse_template(source).is_err(),
            "{source}"
        );
    }
    assert!(crepuscularity_core::parse_template(r#"<div>{""}</div>"#).is_ok());
}

#[test]
fn markup_frontends_reject_empty_text_interpolations() {
    for (filename, source) in [
        ("Empty.svelte", "<div>{}</div>"),
        ("Empty.astro", "<div>{}</div>"),
        ("Empty.vue", "<template><div>{{   }}</div></template>"),
        ("Empty.ng.html", "<div>{{}}</div>"),
    ] {
        assert!(
            crepuscularity_core::parse_template_with_path(
                source,
                Some(std::path::Path::new(filename))
            )
            .is_err(),
            "{filename}"
        );
    }
    for (filename, source) in [
        ("Valid.vue", r#"<template><div>{{""}}</div></template>"#),
        ("Valid.ng.html", r#"<div>{{""}}</div>"#),
    ] {
        assert!(
            crepuscularity_core::parse_template_with_path(
                source,
                Some(std::path::Path::new(filename))
            )
            .is_ok(),
            "{filename}"
        );
    }
}

#[test]
fn markup_comments_must_close() {
    for filename in [
        "Comment.svelte",
        "Comment.astro",
        "Comment.vue",
        "Comment.ng.html",
    ] {
        let path = std::path::Path::new(filename);
        let prefix = if filename.ends_with(".vue") {
            "<template><div /></template>"
        } else {
            "<div />"
        };
        assert!(
            crepuscularity_core::parse_template_with_path(
                &format!("{prefix}<!-- unfinished"),
                Some(path)
            )
            .is_err(),
            "{filename}"
        );
        assert!(
            crepuscularity_core::parse_template_with_path(
                &format!("{prefix}<!-- complete -->"),
                Some(path)
            )
            .is_ok(),
            "{filename}"
        );
    }
}

#[test]
fn svelte_block_extraction_ignores_comments_and_tag_name_prefixes() {
    let component = crepuscularity_core::parser::parse_svelte_component(
        "<!-- <script>fake</script><style>fake</style> --><script>real</script><div>Content</div>",
    )
    .unwrap();
    assert_eq!(component.script.as_deref(), Some("real"));
    assert!(component.style.is_none());
    for source in [
        "<scripture>Content</scripture>",
        "<stylesheet>Content</stylesheet>",
    ] {
        let nodes = crepuscularity_core::parse_template_with_path(
            source,
            Some(std::path::Path::new("Names.svelte")),
        )
        .unwrap();
        assert!(!nodes.is_empty(), "{source}");
    }
}

#[test]
fn svelte_blocks_require_real_closing_tags() {
    for source in [
        "<script>unclosed",
        "<style>unclosed",
        "<script>text</scripture>",
        "<style>text</style junk>",
    ] {
        assert!(
            crepuscularity_core::parser::parse_svelte_component(source).is_err(),
            "{source}"
        );
    }
    let component = crepuscularity_core::parser::parse_svelte_component(
        "<script>one</scripture>two</script><div>Content</div>",
    )
    .unwrap();
    assert_eq!(component.script.as_deref(), Some("one</scripture>two"));
}

#[test]
fn svelte_top_level_rejects_unconsumed_closing_markup() {
    for source in [
        "</div>",
        "<div>Content</div></span>",
        "<div />{:else}",
        "<div />{/if}",
    ] {
        assert!(
            crepuscularity_core::parser::parse_svelte_component(source).is_err(),
            "{source}"
        );
    }
    let component = crepuscularity_core::parser::parse_svelte_component("<div /><span />").unwrap();
    assert_eq!(component.nodes.len(), 2);
}

#[test]
fn svelte_module_script_detection_uses_attributes() {
    for source in [
        r#"<script data-name="module">ordinary</script>"#,
        r#"<script data-name="module>text">ordinary</script>"#,
    ] {
        let component = crepuscularity_core::parser::parse_svelte_component(source).unwrap();
        assert_eq!(component.script.as_deref(), Some("ordinary"));
        assert!(component.module_script.is_none());
    }
    for source in [
        r#"<script module>module body</script>"#,
        r#"<script context="module">module body</script>"#,
    ] {
        let component = crepuscularity_core::parser::parse_svelte_component(source).unwrap();
        assert_eq!(component.module_script.as_deref(), Some("module body"));
        assert!(component.script.is_none());
    }
}

#[test]
fn duplicate_svelte_blocks_are_errors_and_script_scopes_are_distinct() {
    for source in [
        "<script>one</script><script>two</script>",
        "<script module>one</script><script context=\"module\">two</script>",
        "<style>one</style><style>two</style>",
    ] {
        assert!(
            crepuscularity_core::parser::parse_svelte_component(source).is_err(),
            "{source}"
        );
    }
    let component = crepuscularity_core::parser::parse_svelte_component(
        "<script>instance</script><script module>module</script>",
    )
    .unwrap();
    assert_eq!(component.script.as_deref(), Some("instance"));
    assert_eq!(component.module_script.as_deref(), Some("module"));
}

#[test]
fn svelte_blocks_are_top_level_and_tag_strings_are_not_extracted() {
    for source in [
        "<div><script>nested</script></div>",
        "{#if enabled}<style>nested</style>{/if}",
    ] {
        assert!(
            crepuscularity_core::parser::parse_svelte_component(source).is_err(),
            "{source}"
        );
    }
    for source in [
        r#"<div title="<script>fake</script>">Content</div>"#,
        r#"<div>{"<script>fake</script>"}</div>"#,
    ] {
        let component = crepuscularity_core::parser::parse_svelte_component(source).unwrap();
        assert!(component.script.is_none(), "{source}");
        assert!(!component.nodes.is_empty(), "{source}");
    }
}

#[test]
fn vue_sfc_attributes_use_exact_names_and_quoted_tag_boundaries() {
    let sfc = crepuscularity_core::parser::parse_vue_sfc(r#"<script data-lang="wrong" lang = "ts" data-note="a > setup">body</script><style scoped='false' module lang="x>y">css</style><template><div /></template>"#).unwrap();
    assert_eq!(sfc.scripts[0].lang.as_deref(), Some("ts"));
    assert!(!sfc.scripts[0].setup);
    assert_eq!(sfc.scripts[0].content, "body");
    assert!(sfc.styles[0].scoped);
    assert!(sfc.styles[0].module);
    assert_eq!(sfc.styles[0].lang.as_deref(), Some("x>y"));
    assert_eq!(sfc.styles[0].content, "css");
}

#[test]
fn vue_sfc_blocks_require_exact_closing_tags() {
    for source in [
        "<template><div /></templateExtra>",
        "<script>body</scripture>",
        "<style>css</style junk>",
    ] {
        assert!(
            crepuscularity_core::parser::parse_vue_sfc(source).is_err(),
            "{source}"
        );
    }
    let sfc = crepuscularity_core::parser::parse_vue_sfc(
        "<script>one</scripture>two</script > <template><div /></template >",
    )
    .unwrap();
    assert_eq!(sfc.scripts[0].content, "one</scripture>two");
    assert!(sfc.template.is_some());
}

#[test]
fn vue_raw_block_bodies_do_not_nest_tag_strings() {
    let script = r#"const tag = "<script>"; const comment = "<!--";"#;
    let style = r#".label::before { content: "<style> <!--"; }"#;
    let source =
        format!("<script>{script}</script><style>{style}</style><template><div /></template>");
    let sfc = crepuscularity_core::parser::parse_vue_sfc(&source).unwrap();
    assert_eq!(sfc.scripts[0].content, script);
    assert_eq!(sfc.styles[0].content, style);
}

#[test]
fn vue_template_tag_strings_do_not_change_sfc_nesting() {
    for body in [
        r#"<div title="<template> </template>">Content</div>"#,
        r#"<div>{{ "</template>" }}</div>"#,
        r#"<template v-if="enabled"><div /></template><div />"#,
    ] {
        let source = format!("<template>{body}</template>");
        let sfc = crepuscularity_core::parser::parse_vue_sfc(&source).unwrap();
        assert_eq!(sfc.template.as_deref(), Some(body));
    }
}

#[test]
fn vue_unclosed_moustache_retains_its_template_diagnostic() {
    let error = crepuscularity_core::parse_template_with_path(
        "<template><p>{{ name</p></template>",
        Some(std::path::Path::new("Bad.vue")),
    )
    .unwrap_err();
    assert!(error.to_string().contains("unclosed moustache"), "{error}");
}

#[test]
fn vue_external_sfc_blocks_are_explicitly_unsupported() {
    for source in [
        r#"<template src="view.html" />"#,
        r#"<script src="code.js"></script>"#,
        r#"<style src="theme.css"></style>"#,
    ] {
        let error = crepuscularity_core::parser::parse_vue_sfc(source).unwrap_err();
        assert!(error.to_string().contains("external Vue SFC"), "{error}");
    }
    assert!(crepuscularity_core::parser::parse_vue_sfc(
        r#"<template data-src="metadata"><div /></template>"#
    )
    .is_ok());
}

#[test]
fn vue_self_closing_sfc_blocks_are_recorded() {
    let sfc = crepuscularity_core::parser::parse_vue_sfc(
        "<template/><script setup lang=\"ts\"/><style scoped/>",
    )
    .unwrap();
    assert_eq!(sfc.template.as_deref(), Some(""));
    assert_eq!(sfc.scripts.len(), 1);
    assert!(sfc.scripts[0].setup);
    assert_eq!(sfc.scripts[0].content, "");
    assert_eq!(sfc.styles.len(), 1);
    assert!(sfc.styles[0].scoped);
    assert!(
        crepuscularity_core::parser::parse_vue_sfc("<template/><template><div /></template>")
            .is_err()
    );
}

#[test]
fn astro_frontmatter_requires_a_closing_fence() {
    let path = Some(std::path::Path::new("Fence.astro"));
    for source in ["---", "---\nconst value = 1;\n<div />"] {
        let error = crepuscularity_core::parse_template_with_path(source, path).unwrap_err();
        assert!(
            error.to_string().contains("unclosed Astro frontmatter"),
            "{error}"
        );
    }
    assert!(
        crepuscularity_core::parse_template_with_path("---\n// 雪\n---\n<div />", path).is_ok()
    );
    assert!(crepuscularity_core::parse_template_with_path("---\n---", path).is_ok());
}

#[test]
fn astro_comment_expressions_must_close_without_dropping_trailing_code() {
    let path = Some(std::path::Path::new("Comments.astro"));
    for source in [
        "<div>{/* unfinished}</div>",
        "<div>{/* complete */ value}</div>",
    ] {
        assert!(
            crepuscularity_core::parse_template_with_path(source, path).is_err(),
            "{source}"
        );
    }
    assert!(crepuscularity_core::parse_template_with_path(
        "<div>{/* complete */}Content</div>",
        path
    )
    .is_ok());
}

#[test]
fn astro_line_comments_cannot_drop_following_expressions() {
    let path = Some(std::path::Path::new("LineComment.astro"));
    assert!(
        crepuscularity_core::parse_template_with_path("<div>{// comment\nvalue}</div>", path)
            .is_err()
    );
    assert!(crepuscularity_core::parse_template_with_path(
        "<div>{// comment\n}Content</div>",
        path
    )
    .is_ok());
}

#[test]
fn astro_block_comments_can_contain_braces_and_quotes() {
    let path = Some(std::path::Path::new("CommentText.astro"));
    for source in [
        r#"<div>{/* } { " ' */}Content</div>"#,
        "<div>{ /* 雪 } */ }Content</div>",
    ] {
        assert!(
            crepuscularity_core::parse_template_with_path(source, path).is_ok(),
            "{source}"
        );
    }
}

#[test]
fn astro_line_comments_can_contain_braces_and_quotes() {
    let path = Some(std::path::Path::new("LineCommentText.astro"));
    for source in [
        "<div>{// } { \" ' 雪\n}Content</div>",
        "<div>{ // }\n }Content</div>",
    ] {
        assert!(
            crepuscularity_core::parse_template_with_path(source, path).is_ok(),
            "{source}"
        );
    }
}
