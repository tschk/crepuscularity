use crepuscularity_native::{emit_imba_component, ViewIr};
use serde_json::json;

#[test]
fn imba_tabs_use_shared_selection_and_accessibility_helpers() {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"tabs", "bind":"active", "onChange":"prefs.choose", "style":{"classes":["tabs"]},
        "tabs":[
            {"value":"first", "label":"First", "children":[{"kind":"text","content":"One"}]},
            {"value":"second", "label":"Second", "children":[{"kind":"text","content":"Two"}]}
        ]
    }]}))
    .unwrap();
    let output = emit_imba_component(&ir).unwrap();
    assert!(output.source.contains("bindings.tabsRef(self)\n\t\t<self>"));
    assert!(output
        .source
        .contains("className=(\"tabs\") data-crepus-tabs=(\"true\")"));
    assert!(output
        .source
        .contains("html:role=(\"tablist\") aria-label=(\"Tabs\") @keydown=(bindings.tabsKey(e))"));
    assert_eq!(output.source.matches("aria-selected=(bindings.").count(), 2);
    assert_eq!(output.source.matches("let __crepusHidden").count(), 2);
    assert_eq!(output.source.matches(" ? \"\" : null)").count(), 2);
    assert_eq!(
        output.source.matches("html:hidden=(__crepusHidden").count(),
        2
    );
    assert_eq!(output.source.matches(" hidden=(__crepusHidden").count(), 2);
    assert_eq!(output.source.matches("tabIndex=(0)").count(), 2);
    assert!(output
        .source
        .contains("@click=(bindings.dispatch(handlers,\"prefs.choose\",\"second\"))"));
    assert!(output.source.contains("\"One\""));
    assert!(output.source.contains("\"Two\""));
    assert!(output.bindings.contains("__crepusTabIndex("));
    assert!(output.bindings.contains("__crepusTabsRef(group)"));
    assert!(output.bindings.contains("__crepusTabsKey(event)"));
    assert!(output.bindings.contains("aria-controls"));
    assert!(output.bindings.contains("aria-labelledby"));
    assert!(output.bindings.contains("globalThis.queueMicrotask"));
}

#[test]
fn imba_tabs_inside_loops_keep_locals_and_exact_literals() {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[{
        "kind":"forEach", "bind":"rows", "itemName":"scope", "itemBody":[{
            "kind":"tabs", "bind":"scope.active", "onChange":"choose.{\"key\"}",
            "tabs":[{"value":"{one}\"", "label":"Literal {label}", "children":[]}]
        }]
    }]}))
    .unwrap();
    let output = emit_imba_component(&ir).unwrap();
    assert!(output
        .source
        .contains("aria-selected=(bindings.e1(scope,_crepusItem0))"));
    assert!(output.bindings.contains("_crepusItem0?.active"));
    assert!(output.source.contains(r#""Literal \u007blabel\u007d""#));
    assert!(output.source.contains(
        r#"bindings.dispatch(handlers,"choose.\u007b\"key\"\u007d","\u007bone\u007d\"")"#
    ));
    assert!(output.source.contains("\tnil\n"));
}

#[test]
fn imba_without_tabs_does_not_emit_lifecycle_runtime() {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[]})).unwrap();
    let output = emit_imba_component(&ir).unwrap();
    assert!(!output.source.contains("bindings.tabsRef"));
    assert!(!output.bindings.contains("function tabsRef"));
    assert!(!output.bindings.contains("__crepusTabsRef"));
}
