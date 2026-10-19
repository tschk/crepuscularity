use crepuscularity_native::{
    emit_moonshine_dom_component, emit_moonshine_ssr_component, render_template_to_ir, ViewIr,
    ViewNode,
};

#[test]
fn independent_components_compile_readers_and_owned_control_flow() {
    let ir = render_template_to_ir(
        "text \"Count: {count}\"\nbutton @click=increment\n  \"Add {count}\"\nif visible\n  for scope in rows\n    text \"Row {scope.name}\"\nelse\n  text \"Hidden\"",
        &Default::default(),
    ).unwrap();
    for (target, output) in [
        ("dom", emit_moonshine_dom_component(&ir, "Counter").unwrap()),
        ("ssr", emit_moonshine_ssr_component(&ir, "Counter").unwrap()),
    ] {
        assert!(output.contains(&format!("@tschk/moonshine-dom/{target}")));
        assert!(output.contains("@tschk/moonshine-dom/control"));
        assert!(output.contains("h(\"span\""));
        assert!(output.contains("() => \"Count: \" + __crepusText(scope().count)"));
        assert!(output.contains("__crepusOwnHandler(handlers(), \"increment\")?.increment?.()"));
        assert!(output.contains("Show<any, Child>({ when: () => (scope().visible)"));
        assert!(output.contains("For<any, Child>({ each: () => toArray((scope()?.rows ?? \"\"))"));
        assert!(output.contains("__crepusText(_crepusItem0?.name)"));
        assert!(output.contains("export { __CrepusComponent as Counter }"));
        for forbidden in [
            "from \"react\"",
            "solid-js",
            "<span",
            "eval(",
            "new Function",
            "renderCrepusIr",
        ] {
            assert!(!output.contains(forbidden), "{forbidden}: {output}");
        }
    }
}

#[test]
fn component_aliases_do_not_capture_imports_and_names_are_validated() {
    let ir = render_template_to_ir("text \"Hello\"", &Default::default()).unwrap();
    for name in ["h", "For", "scope", "globalThis", "__CrepusComponent"] {
        assert!(emit_moonshine_dom_component(&ir, name)
            .unwrap()
            .contains(&format!("export {{ __CrepusComponent as {name} }}")));
    }
    for name in ["", "bad-name", "default", "a;alert(1)"] {
        assert!(emit_moonshine_dom_component(&ir, name).is_err());
    }
}

#[test]
fn toggle_keeps_identity_classes_and_long_press_metadata() {
    let ir = ViewIr {
        version: 7,
        root: vec![ViewNode::Toggle {
            label: "Enabled".into(),
            bind: Some("enabled".into()),
            checked: false,
            on_change: Some("change".into()),
            on_long_press: Some("details".into()),
            style: Some(crepuscularity_native::ir::ViewStyle {
                id: Some("enabled".into()),
                classes: vec!["p-4".into()],
                ..Default::default()
            }),
        }],
    };
    for output in [
        emit_moonshine_dom_component(&ir, "Toggle").unwrap(),
        emit_moonshine_ssr_component(&ir, "Toggle").unwrap(),
    ] {
        for expected in [
            "id: \"enabled\"",
            "class: \"p-4\"",
            "role: \"switch\"",
            "\"data-crepus-on-long-press\": \"details\"",
            "?.change?.(event)",
        ] {
            assert!(output.contains(expected), "{output}");
        }
    }
}
