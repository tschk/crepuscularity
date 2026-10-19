use crepuscularity_core::{parse_template, TemplateContext};
use crepuscularity_native::{render_nodes_to_ir, render_template_to_ir, ViewIr, IR_VERSION};
use serde_json::json;

#[test]
fn authored_url_expressions_are_retained_alongside_root_snapshots() {
    let mut ctx = TemplateContext::new();
    ctx.set("photo", "https://example.test/first.png");
    ctx.set("destination", "https://example.test/first");
    let nodes = parse_template("image src={photo}\nlink href={destination}\n  \"Open\"").unwrap();
    let ir = serde_json::to_value(render_nodes_to_ir(&nodes, &ctx).unwrap()).unwrap();
    assert_eq!(ir["root"][0]["src"], "https://example.test/first.png");
    assert_eq!(ir["root"][0]["srcBind"], "photo");
    assert_eq!(ir["root"][1]["href"], "https://example.test/first");
    assert_eq!(ir["root"][1]["hrefBind"], "destination");
}

#[test]
fn loop_urls_retain_the_authored_alias_instead_of_only_empty_snapshots() {
    let nodes = parse_template(
        "for row in rows\n  image src={row.photo}\n  link href={row.destination}\n    \"Open\"",
    )
    .unwrap();
    let ir =
        serde_json::to_value(render_nodes_to_ir(&nodes, &TemplateContext::new()).unwrap()).unwrap();
    let body = &ir["root"][0]["itemBody"];
    assert_eq!(body[0]["srcBind"], "row.photo");
    assert_eq!(body[1]["hrefBind"], "row.destination");
    assert_eq!(ir["root"][0]["itemName"], "row");
}

#[test]
fn static_urls_and_legacy_ir_omit_optional_expression_metadata() {
    let ir = render_template_to_ir(
        "image src=\"/fixed.png\"\nlink href=\"/fixed\"\n  \"Open\"",
        &TemplateContext::new(),
    )
    .unwrap();
    let json = serde_json::to_value(ir).unwrap();
    assert_eq!(json["root"][0]["src"], "/fixed.png");
    assert_eq!(json["root"][1]["href"], "/fixed");
    assert!(json["root"][0].get("srcBind").is_none());
    assert!(json["root"][1].get("hrefBind").is_none());
    let legacy = json!({"version":IR_VERSION,"root":[{"kind":"image","src":"/legacy.png"},{"kind":"link","href":"/legacy","children":[]}]});
    let parsed: ViewIr = serde_json::from_value(legacy).unwrap();
    let serialized = serde_json::to_value(parsed).unwrap();
    assert_eq!(serialized["root"][0]["src"], "/legacy.png");
    assert_eq!(serialized["root"][1]["href"], "/legacy");
    assert!(serialized["root"][0].get("srcBind").is_none());
    assert!(serialized["root"][1].get("hrefBind").is_none());
}

#[test]
fn duplicate_url_bindings_and_evaluation_errors_keep_existing_diagnostics() {
    for (source, property) in [
        ("image src=\"a\" src=\"b\"", "src"),
        ("link href=\"a\" href=\"b\"", "href"),
    ] {
        let error = render_template_to_ir(source, &TemplateContext::new())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("duplicate {property} binding")),
            "{error}"
        );
    }
    let expected = crepuscularity_core::eval::eval_expr("1 +", &TemplateContext::new())
        .unwrap_err()
        .to_string();
    for source in ["image src={1 +}", "link href={1 +}"] {
        let error = render_template_to_ir(source, &TemplateContext::new()).unwrap_err();
        assert_eq!(error.to_string(), expected, "{source}");
    }
}

#[test]
fn expression_only_url_changes_survive_mutation_roundtrip() {
    for (kind, field, bind) in [("image", "src", "srcBind"), ("link", "href", "hrefBind")] {
        let node = |expression: &str| {
            let mut node = json!({"kind":kind, field:"", bind:expression});
            if kind == "link" {
                node["children"] = json!([]);
            }
            serde_json::from_value(node).unwrap()
        };
        let mut before = ViewIr {
            version: IR_VERSION,
            root: vec![node("first")],
        };
        let after = ViewIr {
            version: IR_VERSION,
            root: vec![node("second")],
        };
        let mutations = crepuscularity_native::mutations::diff_ir(&before, &after);
        assert!(!mutations.is_empty(), "{kind}");
        crepuscularity_native::mutations::apply_mutations(&mut before, &mutations).unwrap();
        assert_eq!(before, after);
    }
}

#[test]
fn declaration_scoped_urls_keep_snapshots_without_leaking_capture_state_to_siblings() {
    let mut ctx = TemplateContext::new();
    ctx.set("photo", "/host.png");
    let ir=render_template_to_ir("image src={photo}\nsection\n  $: let local_photo = '/local.png'\n  $: let local_href = '/local'\n  image src={local_photo}\n  link href={local_href}\n    \"Local\"\nimage src={photo}",&ctx).unwrap();
    let json = serde_json::to_value(ir).unwrap();
    assert_eq!(json["root"][0]["srcBind"], "photo");
    let children = &json["root"][1]["children"];
    assert_eq!(children[0]["src"], "/local.png");
    assert_eq!(children[1]["href"], "/local");
    assert!(children[0].get("srcBind").is_none());
    assert!(children[1].get("hrefBind").is_none());
    assert_eq!(json["root"][2]["srcBind"], "photo");
    assert_eq!(ctx.get_str("photo"), "/host.png");
}

#[test]
fn included_prop_urls_keep_their_captured_values_instead_of_outer_scope_expressions() {
    let mut ctx = TemplateContext::new();
    ctx.set("photo", "/host.png");
    ctx.set("destination", "/host");
    std::sync::Arc::make_mut(&mut ctx.virtual_files).insert(
        "child.crepus".into(),
        "image src={photo}\nlink href={destination}\n  \"Child\"".into(),
    );
    let ir = render_template_to_ir(
        "include child.crepus photo='/child.png' destination='/child'\nimage src={photo}",
        &ctx,
    )
    .unwrap();
    let json = serde_json::to_value(ir).unwrap();
    assert_eq!(json["root"][0]["src"], "/child.png");
    assert_eq!(json["root"][1]["href"], "/child");
    assert!(json["root"][0].get("srcBind").is_none());
    assert!(json["root"][1].get("hrefBind").is_none());
    assert_eq!(json["root"][2]["srcBind"], "photo");
}

#[test]
fn named_component_default_urls_remain_serializable_snapshots() {
    let source="+++\n[Card.defaults]\nphoto = '/default.png'\ndestination = '/default'\n+++\n--- Card\nimage src={photo}\nlink href={destination}\n  \"Default\"";
    let ir =
        crepuscularity_native::render_component_file_to_ir(source, "Card", &TemplateContext::new())
            .unwrap();
    let json = serde_json::to_value(ir).unwrap();
    assert_eq!(json["root"][0]["src"], "/default.png");
    assert_eq!(json["root"][1]["href"], "/default");
    assert!(json["root"][0].get("srcBind").is_none());
    assert!(json["root"][1].get("hrefBind").is_none());
    let live = serde_json::to_value(
        render_template_to_ir("image src={photo}", &TemplateContext::new()).unwrap(),
    )
    .unwrap();
    assert_eq!(live["root"][0]["srcBind"], "photo");
}

#[test]
fn root_url_expression_metadata_is_preserved_across_all_frontends() {
    let mut ctx = TemplateContext::new();
    ctx.set("photo", "/bound.png");
    ctx.set("destination", "/bound");
    for (filename, source) in [
        (
            "Entry.crepus",
            "image src={photo}\nlink href={destination}\n  \"Open\"",
        ),
        (
            "Entry.jsx",
            "<><img src={photo}/><a href={destination}>Open</a></>",
        ),
        (
            "Entry.svelte",
            "<img src={photo}/><a href={destination}>Open</a>",
        ),
        (
            "Entry.astro",
            "<img src={photo}/><a href={destination}>Open</a>",
        ),
        (
            "Entry.vue",
            "<template><img :src='photo'/><a :href='destination'>Open</a></template>",
        ),
        (
            "Entry.ng",
            "<img [src]='photo'/><a [href]='destination'>Open</a>",
        ),
    ] {
        let nodes = crepuscularity_core::parse_template_with_path(
            source,
            Some(std::path::Path::new(filename)),
        )
        .unwrap();
        let ir = serde_json::to_value(render_nodes_to_ir(&nodes, &ctx).unwrap()).unwrap();
        let root = if filename == "Entry.jsx" {
            &ir["root"][0]["children"]
        } else {
            &ir["root"]
        };
        assert_eq!(root[0]["src"], "/bound.png", "{filename}: {ir}");
        assert_eq!(root[0]["srcBind"], "photo", "{filename}");
        assert_eq!(root[1]["href"], "/bound", "{filename}");
        assert_eq!(root[1]["hrefBind"], "destination", "{filename}");
    }
}
