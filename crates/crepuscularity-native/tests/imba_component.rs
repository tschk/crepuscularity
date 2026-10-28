use crepuscularity_native::{emit_imba_component, ViewIr};
use serde_json::{json, Value};

fn ir(root: Value) -> ViewIr {
    serde_json::from_value(json!({"version": 7, "root": root})).unwrap()
}

#[test]
fn imba_preserves_expressions_scope_and_exact_action_names() {
    let component = emit_imba_component(&ir(json!([
        {"kind":"text", "content":"snapshot", "bind":"user.name"},
        {"kind":"forEach", "bind":"rows", "itemName":"scope", "itemBody":[
            {"kind":"button", "label":"snapshot", "labelParts":[
                {"kind":"literal","value":"Choose "},
                {"kind":"expr","value":"scope.name"}
            ],"onClick":"device.choose","onLongPress":"hold"},
            {"kind":"if","condition":"scope.active", "thenChildren":[
                {"kind":"text","content":"Active"}
            ], "elseChildren":[]}
        ]}
    ])))
    .unwrap();
    assert!(component
        .source
        .starts_with("import * as bindings from './CrepusEmit.bindings.ts'"));
    assert!(component
        .source
        .contains("export tag CrepusEmit\n\tprop scope = {}\n\tprop handlers = {}"));
    assert!(component
        .source
        .contains("for _crepusItem0 in bindings.e1(scope)"));
    assert!(component.source.contains("bindings.e2(scope,_crepusItem0)"));
    assert!(component
        .source
        .contains("@click=(bindings.dispatch(handlers,\"device.choose\"))"));
    assert!(component
        .source
        .contains("data-crepus-on-long-press=(\"hold\")"));
    assert!(component.source.contains("\t\t\t\t\telse\n\t\t\t\t\t\tnil"));
    assert!(component
        .bindings
        .contains("__crepusText(scope.user?.name)"));
    assert!(component
        .bindings
        .contains("__crepusText(_crepusItem0?.name)"));
    assert!(component.bindings.contains("scope: any, _crepusItem0: any"));
    assert!(component
        .bindings
        .contains("Object.prototype.hasOwnProperty.call(handlers, key)"));
    assert!(component.bindings.contains("handler.apply(handlers, args)"));
    assert!(!component.source.contains("?."));
}

#[test]
fn imba_literals_do_not_interpolate_and_controls_keep_bindings() {
    let component = emit_imba_component(&ir(json!([
        {"kind":"text", "content":"{secret} \"quoted\"\n\\path"},
        {"kind":"button", "label":"{literal}", "onClick":"say.\"{hi}\\"},
        {"kind":"input", "placeholder":"Name {literal}", "bind":"user.name", "onChange":"edit"},
        {"kind":"checkbox", "label":"Enabled", "bind":"prefs.enabled", "checked":true, "onChange":"prefs.change"},
        {"kind":"toggle", "label":"Dark", "checked":false, "onChange":"prefs.dark"}
    ]))).unwrap();
    assert!(component
        .source
        .contains(r#""\u007bsecret\u007d \"quoted\"\n\\path""#));
    assert!(component
        .source
        .contains(r#"bindings.dispatch(handlers,"say.\"\u007bhi\u007d\\")"#));
    assert!(component
        .source
        .contains("@change=(bindings.dispatch(handlers,\"edit\",e))"));
    assert!(component.source.contains("html:role=(\"switch\")"));
    assert!(component.source.contains("checked=(bindings.e1(scope))"));
    assert!(component.bindings.contains("scope?.user?.name"));
    assert!(component.bindings.contains("scope?.prefs?.enabled"));
}

#[test]
fn imba_core_layout_nodes_and_empty_loop_are_emitted() {
    let component = emit_imba_component(&ir(json!([
        {"kind":"stack", "axis":"column", "style":{"classes":["flex","gap-2"]}, "children":[
            {"kind":"scroll", "axis":"column", "children":[
                {"kind":"list", "ordered":true, "children":[
                    {"kind":"listItem", "children":[
                        {"kind":"link", "href":"/hello", "children":[{"kind":"text","content":"Hello"}]}
                    ]}
                ]},
                {"kind":"image", "src":"image.png", "alt":"Example"},
                {"kind":"divider", "axis":"row"},
                {"kind":"spacer"},
                {"kind":"forEach", "bind":"empty", "itemName":"item", "itemBody":[]}
            ]}
        ]}
    ]))).unwrap();
    for expected in [
        "className=(\"flex gap-2\")",
        "<ol>",
        "<li>",
        "<a href=(\"/hello\")>",
        "<img src=(bindings.",
        "<hr>",
        "aria-hidden=(\"true\")",
        "<>\n",
        "\tnil\n",
    ] {
        assert!(component.source.contains(expected), "{}", component.source);
    }
}

#[test]
fn imba_image_urls_remain_runtime_values_instead_of_asset_imports() {
    let url = "data:image/gif;base64,AAAA";
    let component = emit_imba_component(&ir(json!([
        {"kind":"image", "src":url, "alt":"Image"}
    ])))
    .unwrap();
    assert!(component.source.contains("<img src=(bindings.e0(scope))"));
    assert!(!component.source.contains(url));
    assert!(component.bindings.contains(&format!("return \"{url}\";")));
}
