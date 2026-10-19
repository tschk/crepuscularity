use crepuscularity_native::{emit_moonshine_dom_component, emit_moonshine_ssr_component, ViewIr};
use serde_json::{json, Value};

fn outputs(node: Value) -> [String; 2] {
    let ir: ViewIr = serde_json::from_value(json!({"version":7,"root":[node]})).unwrap();
    [
        emit_moonshine_dom_component(&ir, "TabsView").unwrap(),
        emit_moonshine_ssr_component(&ir, "TabsView").unwrap(),
    ]
}

#[test]
fn direct_host_tabs_keep_reactive_selection_callbacks_and_accessible_identity() {
    let action = "tabs.choose\"\\{key}";
    let value = "two\"\\{value}";
    for output in outputs(json!({
        "kind":"tabs", "bind":"prefs.tab", "onChange":action,
        "style":{"id":"tabs-root","classes":["rounded","p-2"],"ariaLabel":"Choose section"},
        "tabs":[{"value":"one","label":"First","children":[{"kind":"text","content":"First panel"}]},{"value":value,"label":"Second {literal}","children":[{"kind":"text","content":"Second panel"}]}]
    })) {
        for retained in [
            "tabs-root",
            "rounded p-2",
            "Choose section",
            "tablist",
            "tabpanel",
            "First panel",
            "Second panel",
            "Second {literal}",
            "__crepusTabsKey",
            "__crepusTabsRef",
            "__crepusTabIndex",
            "scope()?.prefs?.tab",
            "aria-selected",
            "hidden: () =>",
            "aria-controls",
            "aria-labelledby",
            "__crepusTabsPrefix",
            "randomUUID",
        ] {
            assert!(output.contains(retained), "missing {retained}: {output}");
        }
        assert_eq!(output.matches("Choose section").count(), 1, "{output}");
        assert!(
            output.contains(&format!(
                "__crepusOwnHandler(handlers(), {})",
                serde_json::to_string(action).unwrap()
            )),
            "{output}"
        );
        assert!(
            output.contains(&serde_json::to_string(value).unwrap()),
            "{output}"
        );
        assert!(
            !output.contains("\"data-crepus-tabs-unbound\": \"true\""),
            "{output}"
        );
    }
}

#[test]
fn unbound_tabs_have_local_selection_without_requiring_callbacks() {
    for output in outputs(json!({"kind":"tabs","bind":"  ","tabs":[
        {"value":"one","label":"First","children":[]},
        {"value":"two","label":"Second","children":[]}
    ]})) {
        assert!(
            output.contains("\"data-crepus-tabs-unbound\": \"true\""),
            "{output}"
        );
        assert!(!output.contains("indexOf(undefined)"), "{output}");
        assert!(
            !output.contains("__crepusOwnHandler(handlers()"),
            "{output}"
        );
        assert!(output.contains("aria-controls"), "{output}");
        assert!(output.contains("aria-labelledby"), "{output}");
    }
}

#[test]
fn empty_tab_groups_are_valid_for_both_direct_hosts() {
    for output in outputs(json!({"kind":"tabs","bind":"","tabs":[]})) {
        assert!(output.contains("tablist"), "{output}");
        assert!(!output.contains("NaN"), "{output}");
        assert!(!output.contains("indexOf(undefined)"), "{output}");
    }
}
