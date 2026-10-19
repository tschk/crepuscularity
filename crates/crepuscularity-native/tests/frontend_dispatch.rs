use std::path::Path;

use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::{render_from_files, render_template_to_ir_with_path, IR_VERSION};

#[test]
fn quoted_include_paths_with_spaces_render() {
    let files = std::collections::HashMap::from([
        (
            "entry.crepus".to_string(),
            "include \"card view.crepus\" title=\"Space path\"".to_string(),
        ),
        (
            "card view.crepus".to_string(),
            "span \"{title}\"".to_string(),
        ),
    ]);
    let ir = render_from_files(&files, "entry.crepus", &TemplateContext::new()).unwrap();
    assert!(crepuscularity_native::to_json(&ir)
        .unwrap()
        .contains("Space path"));
}

#[test]
fn virtual_entries_resolve_relative_includes_from_their_directory() {
    let files = std::collections::HashMap::from([
        (
            "a/entry.crepus".to_string(),
            "include child.crepus".to_string(),
        ),
        (
            "a/ui.crepus".to_string(),
            "--- Card\ninclude child.crepus".to_string(),
        ),
        (
            "a/child.crepus".to_string(),
            "span \"Correct child\"".to_string(),
        ),
        (
            "b/child.crepus".to_string(),
            "span \"Wrong child\"".to_string(),
        ),
    ]);
    for entry in ["a/entry.crepus", "a/ui.crepus#Card"] {
        let ir = render_from_files(&files, entry, &TemplateContext::new()).unwrap();
        let json = crepuscularity_native::to_json(&ir).unwrap();
        assert!(json.contains("Correct child"));
        assert!(!json.contains("Wrong child"));
    }
}

/// `crepuscularity_wasm::parse_template_json` is a thin wrapper over
/// `render_template_to_ir_with_path`, so exercising this call from a native
/// test exercises the shared dispatch and lowering path used by that wrapper.
fn ir_json(source: &str, filename: &str) -> serde_json::Value {
    let ir =
        render_template_to_ir_with_path(source, &TemplateContext::new(), Some(Path::new(filename)))
            .unwrap_or_else(|e| panic!("{filename} should lower to View IR: {e}"));
    assert_eq!(ir.version, IR_VERSION);
    serde_json::to_value(&ir).expect("ir to Value")
}

#[test]
fn quoted_jsx_actions_keep_their_registered_names_in_view_ir() {
    for filename in ["Action.jsx", "Action.svelte", "Action.astro"] {
        for attribute in ["@click=\"save\"", "onClick=\"save\""] {
            let ir = render_template_to_ir_with_path(
                &format!("<button {attribute}>Save</button>"),
                &TemplateContext::new(),
                Some(Path::new(filename)),
            )
            .unwrap();
            let crepuscularity_native::ViewNode::Button { on_click, .. } = &ir.root[0] else {
                panic!("expected button")
            };
            assert_eq!(on_click.as_deref(), Some("save"), "{filename}: {attribute}");
        }
    }
}

#[test]
fn virtual_files_select_frontend_from_entry_name() {
    for (filename, source) in [
        (
            "Page.astro",
            "---\nconst title = \"ignored\";\n---\n<h1>Hello</h1>",
        ),
        (
            "App.vue",
            "<template><h1>Hello</h1></template><script>const title = 'ignored';</script>",
        ),
        (
            "App.svelte",
            "<script>let title = 'ignored';</script><h1>Hello</h1>",
        ),
    ] {
        let files = std::collections::HashMap::from([(filename.to_owned(), source.to_owned())]);
        let ir = render_from_files(&files, filename, &TemplateContext::new()).unwrap();
        let rendered = serde_json::to_string(&ir).unwrap();
        assert!(rendered.contains("Hello"), "{filename}: {rendered}");
        assert!(!rendered.contains("ignored"), "script rendered: {rendered}");
    }
}

#[test]
fn include_depth_limit_preserves_valid_nested_templates() {
    let mut files = std::collections::HashMap::new();
    for depth in 0..64 {
        files.insert(
            format!("{depth}.crepus"),
            format!("div\n  include {}.crepus", depth + 1),
        );
    }
    files.insert("64.crepus".to_owned(), "span\n  \"Deep leaf\"".to_owned());
    let ir = render_from_files(&files, "0.crepus", &TemplateContext::new()).unwrap();
    assert!(serde_json::to_string(&ir).unwrap().contains("Deep leaf"));

    files.insert("64.crepus".to_owned(), "include 65.crepus".to_owned());
    files.insert("65.crepus".to_owned(), "span\n  \"Too deep\"".to_owned());
    let error = render_from_files(&files, "0.crepus", &TemplateContext::new()).unwrap_err();
    assert!(error.to_string().contains("maximum include depth"));
}

#[test]
fn include_cycles_are_bounded_and_errors_release_the_depth_guard() {
    for source in [
        "div\n  include loop.crepus",
        "--- Loop\ndiv\n  include loop.crepus#Loop",
    ] {
        let entry = if source.starts_with("---") {
            "loop.crepus#Loop"
        } else {
            "loop.crepus"
        };
        let files = std::collections::HashMap::from([
            ("loop.crepus".to_owned(), source.to_owned()),
            ("good.crepus".to_owned(), "include child.crepus".to_owned()),
            (
                "child.crepus".to_owned(),
                "span\n  \"Recovered\"".to_owned(),
            ),
        ]);
        for _ in 0..2 {
            let error = render_from_files(&files, entry, &TemplateContext::new()).unwrap_err();
            assert!(
                error.to_string().contains("maximum include depth"),
                "{error}"
            );
            let recovered =
                render_from_files(&files, "good.crepus", &TemplateContext::new()).unwrap();
            assert!(serde_json::to_string(&recovered)
                .unwrap()
                .contains("Recovered"));
        }
    }
}

#[test]
fn astro_files_reach_the_view_ir() {
    let json = ir_json(
        r#"---
const title = "ignored";
---
<section class="p-4"><h1>Hello</h1></section>"#,
        "Page.astro",
    );
    let text = serde_json::to_string(&json).unwrap();
    assert!(text.contains("p-4"), "{text}");
    assert!(text.contains("Hello"), "{text}");
    assert!(
        !text.contains("ignored"),
        "frontmatter must not render: {text}"
    );
}

#[test]
fn angular_component_templates_reach_the_view_ir() {
    let json = ir_json(
        r#"<ul><li *ngFor="let item of items">Item</li></ul>"#,
        "app.component.html",
    );
    let text = serde_json::to_string(&json).unwrap();
    assert!(text.contains("Item"), "{text}");
}

#[test]
fn jsx_trailing_markup_does_not_produce_partial_view_ir() {
    for filename in ["App.jsx", "App.tsx", "App.csx", "index.html"] {
        let error = render_template_to_ir_with_path(
            "<div /></orphan><p />",
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .expect_err("unmatched closing tag should prevent lowering");
        assert!(error.to_string().contains("unexpected trailing markup"));
    }
}

#[test]
fn jsx_else_if_chain_reaches_view_ir() {
    let json = ir_json(
        "<if condition={a}><p>A</p><else-if condition={b}><p>B</p></else-if><else><p>C</p></else></if>",
        "App.tsx",
    );
    let first = &json["root"][0];
    assert_eq!(first["condition"], "a");
    let second = &first["elseChildren"][0];
    assert_eq!(second["kind"], "if");
    assert_eq!(second["condition"], "b");
    assert!(second["thenChildren"]
        .as_array()
        .is_some_and(|nodes| !nodes.is_empty()));
    assert!(second["elseChildren"]
        .as_array()
        .is_some_and(|nodes| !nodes.is_empty()));
}

#[test]
fn view_ir_rejects_event_semantics_it_cannot_represent() {
    for (filename, source, message) in [
        (
            "Action.jsx",
            "<button @click|once={save} />",
            "does not support event modifiers",
        ),
        (
            "Action.svelte",
            "<button on:click|trusted={save} />",
            "does not support event modifiers",
        ),
        (
            "Action.vue",
            "<template><button @click.prevent=\"save\" /></template>",
            "does not support event modifiers",
        ),
        (
            "Action.jsx",
            "<button onClick={first} onClick={second} />",
            "supports only one",
        ),
        (
            "Action.svelte",
            "<button on:click={first} on:click={second} />",
            "supports only one",
        ),
    ] {
        let error = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap_err();
        assert!(error.to_string().contains(message), "{filename}: {error}");
    }
}

#[test]
fn file_picker_event_aliases_cannot_overwrite_each_other() {
    for tag in ["file-picker", "filepicker", "media-picker"] {
        for handlers in [
            "@pick={first} @click={second}",
            "@click={first} @pick={second}",
        ] {
            let error = render_template_to_ir_with_path(
                &format!("<{tag} {handlers} />"),
                &TemplateContext::new(),
                Some(Path::new("Picker.jsx")),
            )
            .unwrap_err();
            assert!(
                error.to_string().contains("only one `pick` handler"),
                "{error}"
            );
        }
        for event in ["pick", "click"] {
            let ir = render_template_to_ir_with_path(
                &format!("<{tag} @{event}={{choose}} />"),
                &TemplateContext::new(),
                Some(Path::new("Picker.jsx")),
            )
            .unwrap();
            let crepuscularity_native::ViewNode::FilePicker { on_pick, .. } = &ir.root[0] else {
                panic!("expected picker");
            };
            assert_eq!(on_pick.as_deref(), Some("choose"));
        }
    }
}

#[test]
fn long_press_handlers_reach_existing_view_ir_fields() {
    for element in ["button", "div", "toggle", "img", "li"] {
        for handler in ["@longpress={hold}", "onLongPress={hold}"] {
            let ir = render_template_to_ir_with_path(
                &format!("<{element} {handler} />"),
                &TemplateContext::new(),
                Some(Path::new("Gesture.jsx")),
            )
            .unwrap();
            let value = serde_json::to_value(ir).unwrap();
            assert_eq!(
                value["root"][0]["onLongPress"], "hold",
                "{element}: {handler}"
            );
        }
    }
}

#[test]
fn invalid_long_press_targets_and_colliding_aliases_fail_lowering() {
    for (source, message) in [
        (
            "<input onLongPress={hold} />",
            "does not support long-press handlers",
        ),
        (
            "<button @longpress={first} onLongPress={second} />",
            "only one `longpress` handler",
        ),
    ] {
        let error = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new("Gesture.jsx")),
        )
        .unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
}

#[test]
fn unsupported_bare_events_are_not_silently_dropped() {
    for (filename, source, event) in [
        ("Action.crepus", "button @keydown=save", "keydown"),
        ("Action.jsx", "<button onKeyDown={save} />", "keydown"),
        ("Action.svelte", "<button on:keydown={save} />", "keydown"),
        (
            "Action.vue",
            "<template><button @keydown=\"save\" /></template>",
            "keydown",
        ),
        ("Action.astro", "<button onKeyDown={save} />", "keydown"),
        (
            "Action.ng",
            "<button (keydown)=\"save\"></button>",
            "keydown",
        ),
        ("Action.jsx", "<div onClick={save} />", "click"),
        ("Action.jsx", "<button onChange={save} />", "change"),
        ("Action.jsx", "<button onInput={save} />", "input"),
    ] {
        let error = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("does not support `{event}` handlers")),
            "{filename}: {error}"
        );
    }
}

#[test]
fn supported_bare_events_reach_their_view_ir_fields() {
    for (tag, event, field) in [
        ("button", "click", "onClick"),
        ("toggle", "change", "onChange"),
        ("checkbox", "change", "onChange"),
        ("slider", "change", "onChange"),
        ("input", "change", "onChange"),
        ("picker", "change", "onChange"),
        ("tabs", "change", "onChange"),
        ("dropzone", "drop", "onDrop"),
        ("file-picker", "pick", "onPick"),
    ] {
        let value = ir_json(&format!("<{tag} @{event}={{save}} />"), "Action.jsx");
        assert_eq!(value["root"][0][field], "save", "{tag}: {value}");
    }
}

#[test]
fn metadata_children_cannot_silently_discard_handlers() {
    for source in [
        "<button>First<span onClick={lost}>Later</span></button>",
        "<toggle label=\"Explicit\"><span onClick={lost}>Unused</span></toggle>",
        "<picker><option onClick={lost}>One</option></picker>",
        "<picker><option>First<span onClick={lost}>Later</span></option></picker>",
        "<tabs><tab value=\"one\" onClick={lost}><button onClick={kept}>Content</button></tab></tabs>",
        "<tabs><div><button onClick={lost}>Ignored</button></div></tabs>",
        "<button><if condition={false}><span onClick={lost}>Hidden</span></if></button>",
    ] {
        let error = render_template_to_ir_with_path(source,&TemplateContext::new(),Some(Path::new("App.jsx"))).unwrap_err();
        assert!(error.to_string().contains("metadata-only children"),"{source}: {error}");
    }
}

#[test]
fn rendered_tab_content_keeps_its_handlers() {
    let value = ir_json(
        "<tabs><tab value=\"one\"><button onClick={kept}>Content</button></tab></tabs>",
        "App.jsx",
    );
    assert_eq!(
        value["root"][0]["tabs"][0]["children"][0]["onClick"],
        "kept"
    );
    let value = ir_json("<button>First<span>Later</span></button>", "App.jsx");
    assert_eq!(value["root"][0]["label"], "First");
}

#[test]
fn includes_cannot_hide_handlers_in_metadata_content() {
    let files = std::collections::HashMap::from([
        ("App.crepus".into(), "button\n  include label.crepus".into()),
        (
            "label.crepus".into(),
            "span @click=lost\n  \"Label\"".into(),
        ),
    ]);
    let error = render_from_files(&files, "App.crepus", &TemplateContext::new()).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("includes in metadata-only children"),
        "{error}"
    );
}

#[test]
fn checkbox_input_types_select_checkbox_view_ir() {
    for (filename,source) in [
        ("App.crepus","input type=\"checkbox\" checked=true bind=enabled @change=save"),
        ("App.jsx","<input type=\"checkbox\" checked bind={enabled} onChange={save} />"),
        ("App.svelte","<input type=\"checkbox\" checked bind:checked={enabled} on:change={save} />"),
        ("App.vue","<template><input type=\"checkbox\" checked v-model=\"enabled\" @change=\"save\" /></template>"),
        ("App.astro","<input type=\"checkbox\" checked bind={enabled} onChange={save} />"),
        ("App.ng","<input type=\"checkbox\" checked [(ngModel)]=\"enabled\" (change)=\"save\" />"),
    ] {
        let value=ir_json(source,filename);
        assert_eq!(value["root"][0]["kind"],"checkbox","{filename}: {value}");
        assert_eq!(value["root"][0]["checked"],true,"{filename}: {value}");
        assert_eq!(value["root"][0]["bind"],"enabled","{filename}: {value}");
        assert_eq!(value["root"][0]["onChange"],"save","{filename}: {value}");
    }
    let mut ctx = TemplateContext::new();
    ctx.set("kind", "CHECKBOX");
    let ir = render_template_to_ir_with_path(
        "<input type={kind} checked={false} />",
        &ctx,
        Some(Path::new("App.jsx")),
    )
    .unwrap();
    let value = serde_json::to_value(ir).unwrap();
    assert_eq!(value["root"][0]["kind"], "checkbox");
    assert_eq!(value["root"][0]["checked"], false);
}

#[test]
fn evaluated_password_input_types_keep_secure_rendering() {
    let mut ctx = TemplateContext::new();
    ctx.set("kind", "PASSWORD");
    let ir = render_template_to_ir_with_path(
        "<input type={kind} bind={secret} />",
        &ctx,
        Some(Path::new("App.jsx")),
    )
    .unwrap();
    let value = serde_json::to_value(&ir).unwrap();
    assert_eq!(value["root"][0]["kind"], "input");
    assert_eq!(value["root"][0]["secure"], true);
    let swift = crepuscularity_native::generate_native_source(
        &ir,
        crepuscularity_native::NativeCodegenTarget::SwiftUi,
        "SecretInput",
    );
    assert!(swift.contains("SecureField("));
    let compose = crepuscularity_native::generate_native_source(
        &ir,
        crepuscularity_native::NativeCodegenTarget::Compose,
        "SecretInput",
    );
    assert!(compose.contains("PasswordVisualTransformation()"));
}

#[test]
fn every_frontend_rejects_secure_multiline_inputs() {
    let cases = [
        ("screen.crepus", "input multiline type=password bind=secret"),
        (
            "screen.jsx",
            r#"<input className="multiline" type="password" bind="secret" />"#,
        ),
        (
            "screen.vue",
            r#"<template><input class="multiline" type="password" bind="secret" /></template>"#,
        ),
        (
            "screen.svelte",
            r#"<input class="multiline" type="password" bind="secret" />"#,
        ),
        (
            "screen.astro",
            r#"<input class="multiline" type="password" bind="secret" />"#,
        ),
        (
            "screen.component.html",
            r#"<input class="multiline" type="password" bind="secret" />"#,
        ),
    ];
    for (filename, source) in cases {
        let error = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("secure multiline inputs are not supported"),
            "{filename}: {error}"
        );
    }
}

#[test]
fn boolean_control_attributes_preserve_false_across_frontends() {
    for (filename, source) in [
        ("Boolean.crepus", "input type=checkbox checked={false}\nfile-picker multiple={false}"),
        ("Boolean.jsx", "<div><input type=\"checkbox\" checked={false} /><file-picker multiple={false} /></div>"),
        ("Boolean.svelte", "<div><input type=\"checkbox\" checked={false} /><file-picker multiple={false} /></div>"),
        ("Boolean.astro", "<div><input type=\"checkbox\" checked={false} /><file-picker multiple={false} /></div>"),
        ("Boolean.vue", "<template><div><input type=\"checkbox\" :checked=\"false\" /><file-picker :multiple=\"false\" /></div></template>"),
        ("Boolean.ng", "<div><input type=\"checkbox\" [checked]=\"false\" /><file-picker [multiple]=\"false\" /></div>"),
    ] {
        let json = ir_json(source, filename);
        let nodes = if filename.ends_with(".crepus") { &json["root"] } else { &json["root"][0]["children"] };
        assert_eq!(nodes[0]["checked"], false, "{filename}: {json}");
        assert_eq!(nodes[1]["multiple"], false, "{filename}: {json}");
    }
}

#[test]
fn file_control_labels_evaluate_across_frontends() {
    let mut ctx = TemplateContext::new();
    ctx.set("caption", " Upload ");
    for tag in ["file-picker", "filepicker", "media-picker", "dropzone"] {
        for (filename, source) in [
            (
                "Label.crepus",
                format!("{tag} label={{caption}}\n  \"Fallback\""),
            ),
            (
                "Label.jsx",
                format!("<{tag} label={{caption}}>Fallback</{tag}>"),
            ),
            (
                "Label.svelte",
                format!("<{tag} label={{caption}}>Fallback</{tag}>"),
            ),
            (
                "Label.astro",
                format!("<{tag} label={{caption}}>Fallback</{tag}>"),
            ),
            (
                "Label.vue",
                format!("<template><{tag} :label=\"caption\">Fallback</{tag}></template>"),
            ),
            (
                "Label.ng",
                format!("<{tag} [label]=\"caption\">Fallback</{tag}>"),
            ),
        ] {
            let ir = render_template_to_ir_with_path(&source, &ctx, Some(Path::new(filename)))
                .unwrap_or_else(|error| panic!("{filename}: {source}: {error}"));
            let json = serde_json::to_value(ir).unwrap();
            assert_eq!(
                json["root"][0]["label"], "Upload",
                "{filename}: {source}: {json}"
            );
        }
    }
}
