use crepuscularity_native::{
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir, ViewNode,
};

#[test]
fn framework_tabs_read_selection_keep_callbacks_and_link_accessible_panels() {
    let mut ir = render_template_to_ir("text \"Seed\"", &Default::default()).unwrap();
    let tabs = ViewNode::Tabs {
        bind: "scope.active".into(),
        tabs: vec![crepuscularity_native::ir::TabItem {
            value: "two".into(),
            label: "Two".into(),
            icon: None,
            children: ir.root.clone(),
        }],
        on_change: Some("tabs.change".into()),
        style: None,
    };
    ir.root = vec![ViewNode::ForEach {
        bind: "rows".into(),
        item_name: "scope".into(),
        item_body: vec![tabs],
        style: None,
    }];
    for output in [
        emit_react_component(&ir, "Tabs"),
        emit_solid_component(&ir, "Tabs"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        let output = output.replace("&quot;", "\"");
        assert!(
            output.contains("([\"two\"].indexOf(_crepusItem0?.active) === 0)"),
            "{output}"
        );
        assert!(
            output.contains("__crepusTabIndex(_crepusItem0?.active, [\"two\"], 0)"),
            "{output}"
        );
        assert!(output.contains("aria-selected"));
        assert!(output.contains("hidden="));
        assert!(output.contains("__crepusTabsRef"));
        assert!(output.contains("__crepusTabsKey"));
        assert!(output.contains("aria-controls"));
        assert!(output.contains("aria-labelledby"));
        assert!(output.contains("?.(\"two\")"));
    }
}

#[test]
fn templates_without_tabs_do_not_include_tab_runtime() {
    let ir = render_template_to_ir("text \"Plain\"", &Default::default()).unwrap();
    for output in [
        emit_react_component(&ir, "Plain"),
        emit_solid_component(&ir, "Plain"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        assert!(!output.contains("__crepusTabs"));
    }
}

#[test]
fn duplicate_tab_values_use_the_first_matching_index() {
    let mut ir = render_template_to_ir("text \"Seed\"", &Default::default()).unwrap();
    ir.root = vec![ViewNode::Tabs {
        bind: "active".into(),
        tabs: ["First", "Duplicate"]
            .iter()
            .map(|label| crepuscularity_native::ir::TabItem {
                value: "one".into(),
                label: (*label).into(),
                icon: None,
                children: vec![],
            })
            .collect(),
        on_change: None,
        style: None,
    }];
    for output in [
        emit_react_component(&ir, "Tabs"),
        emit_solid_component(&ir, "Tabs"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        let output = output.replace("&quot;", "\"");
        assert!(
            output.contains("[\"one\",\"one\"].indexOf(scope?.active) === 0")
                || output.contains("[\"one\",\"one\"].indexOf(scope()?.active) === 0"),
            "{output}"
        );
        assert!(
            output.contains("[\"one\",\"one\"].indexOf(scope?.active) === 1")
                || output.contains("[\"one\",\"one\"].indexOf(scope()?.active) === 1"),
            "{output}"
        );
    }
}
