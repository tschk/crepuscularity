use crepuscularity_native::{
    emit_imba_component, emit_react_component, emit_solid_component, emit_svelte_component,
    emit_vue_component, ir::TabItem, ViewIr, ViewNode, IR_VERSION,
};

fn tabs(bind: &str, populated: bool, callback: bool) -> ViewIr {
    ViewIr {
        version: IR_VERSION,
        root: vec![ViewNode::Tabs {
            bind: bind.into(),
            tabs: if populated {
                ["One", "Two"]
                    .into_iter()
                    .map(|label| TabItem {
                        value: label.to_lowercase(),
                        label: label.into(),
                        icon: None,
                        children: vec![],
                    })
                    .collect()
            } else {
                vec![]
            },
            on_change: callback.then(|| "tabs.change".into()),
            style: None,
        }],
    }
}

fn outputs(ir: &ViewIr) -> Vec<(&'static str, String)> {
    let imba = emit_imba_component(ir).unwrap();
    vec![
        ("react", emit_react_component(ir, "Tabs")),
        ("solid", emit_solid_component(ir, "Tabs")),
        ("svelte", emit_svelte_component(ir)),
        ("vue", emit_vue_component(ir)),
        ("imba", format!("{}\n{}", imba.source, imba.bindings)),
    ]
}

fn unbound_marker(target: &str) -> &'static str {
    if target == "imba" {
        "data-crepus-tabs-unbound=(\"true\")"
    } else {
        "data-crepus-tabs-unbound=\"true\""
    }
}

#[test]
fn blank_bindings_initialize_the_first_tab_and_keep_callbacks() {
    for bind in ["", " ", "\n\t"] {
        for (target, output) in outputs(&tabs(bind, true, true)) {
            assert!(
                output.contains(unbound_marker(target)),
                "{target}: {output}"
            );
            assert!(!output.contains("indexOf(undefined)"), "{target}: {output}");
            assert!(
                !output.contains("__crepusTabIndex(undefined"),
                "{target}: {output}"
            );
            assert!(
                output.contains("tabs.change"),
                "{target}: callback lost: {output}"
            );
            match target {
                "vue" => {
                    assert!(output.contains(":aria-selected=\"true\""), "{output}");
                    assert!(output.contains(":aria-selected=\"false\""), "{output}");
                    assert!(output.contains(":tabindex=\"0\""), "{output}");
                    assert!(output.contains(":hidden=\"!(true)\""), "{output}");
                }
                "imba" => {
                    assert!(output.contains("return true;"), "{output}");
                    assert!(output.contains("return false;"), "{output}");
                    assert!(output.contains("return 0;"), "{output}");
                    assert!(output.contains("return !(true);"), "{output}");
                }
                _ => {
                    assert!(output.contains("aria-selected={true}"), "{output}");
                    assert!(output.contains("aria-selected={false}"), "{output}");
                    assert!(output.contains("hidden={!(true)}"), "{output}");
                    assert!(
                        output.contains("tabIndex={0}") || output.contains("tabindex={0}"),
                        "{output}"
                    );
                }
            }
        }
    }
}

#[test]
fn controlled_tabs_keep_host_selection_without_unbound_markup() {
    for (target, output) in outputs(&tabs("active", true, true)) {
        assert!(
            !output.contains(unbound_marker(target)),
            "{target}: {output}"
        );
        let decoded = output.replace("&quot;", "\"");
        let path = if target == "solid" {
            "scope()?.active"
        } else {
            "scope?.active"
        };
        assert!(
            decoded.contains(&format!("indexOf({path})")),
            "{target}: {output}"
        );
        assert!(output.contains("tabs.change"), "{target}: {output}");
    }
}

#[test]
fn empty_unbound_groups_emit_valid_initialization_without_buttons() {
    for (target, output) in outputs(&tabs("", false, false)) {
        assert!(
            output.contains(unbound_marker(target)),
            "{target}: {output}"
        );
        assert!(!output.contains("indexOf(undefined)"), "{target}: {output}");
        assert!(!output.contains("<button"), "{target}: {output}");
        assert!(!output.contains("tabs.change"), "{target}: {output}");
    }
}
