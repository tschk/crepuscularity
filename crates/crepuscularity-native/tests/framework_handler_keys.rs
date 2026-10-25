use crepuscularity_core::TemplateContext;
use crepuscularity_native::ir::TabItem;
use crepuscularity_native::{
    emit_moonshine_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir, ViewNode,
};

#[test]
fn framework_handlers_use_exact_flat_action_keys() {
    let mut ir = render_template_to_ir(
        "button @click=device.info \"Info\"\ntoggle @change=prefs.dark \"Dark\"",
        &TemplateContext::new(),
    )
    .unwrap();
    ir.root.push(ViewNode::Tabs {
        bind: "tab".into(),
        tabs: vec![TabItem {
            value: "one".into(),
            label: "One".into(),
            icon: None,
            children: vec![],
        }],
        on_change: Some("tab.select".into()),
        style: None,
    });
    for (name, output, object, quote) in [
        (
            "moonshine",
            emit_moonshine_component(&ir, "Actions"),
            "handlers",
            "\"",
        ),
        (
            "solid",
            emit_solid_component(&ir, "Actions"),
            "handlers()",
            "\"",
        ),
        ("svelte", emit_svelte_component(&ir), "handlers", "\""),
        ("vue", emit_vue_component(&ir), "handlers", "&quot;"),
    ] {
        for action in ["device.info", "prefs.dark", "tab.select"] {
            let expected = format!(
                "__crepusOwnHandler({object}, {quote}{action}{quote})?.[{quote}{action}{quote}]?.("
            );
            assert!(
                output.contains(&expected),
                "{name}: missing {expected:?} in {output}"
            );
        }
    }
}

#[test]
fn switches_use_activation_events_and_checkboxes_keep_change_events() {
    let ir = render_template_to_ir(
        "toggle @change=toggleAction \"Toggle\"\ncheckbox @change=checkAction \"Check\"",
        &TemplateContext::new(),
    )
    .unwrap();
    for (name, output, switch_event, checkbox_event) in [
        (
            "moonshine",
            emit_moonshine_component(&ir, "Actions"),
            "onClick={(event) => __crepusOwnHandler(handlers, \"toggleAction\")?.toggleAction?.(event)}",
            "onChange={(event) => __crepusOwnHandler(handlers, \"checkAction\")?.checkAction?.(event)}",
        ),
        (
            "solid",
            emit_solid_component(&ir, "Actions"),
            "onClick={(event) => __crepusOwnHandler(handlers(), \"toggleAction\")?.toggleAction?.(event)}",
            "onChange={(event) => __crepusOwnHandler(handlers(), \"checkAction\")?.checkAction?.(event)}",
        ),
        (
            "svelte",
            emit_svelte_component(&ir),
            "onclick={(event) => __crepusOwnHandler(handlers, \"toggleAction\")?.toggleAction?.(event)}",
            "onchange={(event) => __crepusOwnHandler(handlers, \"checkAction\")?.checkAction?.(event)}",
        ),
        (
            "vue",
            emit_vue_component(&ir),
            "@click=\"__crepusOwnHandler(handlers, &quot;toggleAction&quot;)?.toggleAction?.($event)\"",
            "@change=\"__crepusOwnHandler(handlers, &quot;checkAction&quot;)?.checkAction?.($event)\"",
        ),
    ] {
        assert!(output.contains(switch_event), "{name}: {output}");
        assert!(output.contains(checkbox_event), "{name}: {output}");
    }
}

#[test]
fn tab_callbacks_encode_values_as_javascript_strings() {
    let mut ir = render_template_to_ir("text \"Tabs\"", &TemplateContext::new()).unwrap();
    let value = "\"double\" 'single' \\\n\t\r\u{1}</button>&quot;雪";
    ir.root.push(ViewNode::Tabs {
        bind: "tab".into(),
        tabs: vec![TabItem {
            value: value.into(),
            label: "Choose".into(),
            icon: None,
            children: vec![],
        }],
        on_change: Some("select".into()),
        style: None,
    });
    let argument = serde_json::Value::String(value.into()).to_string();
    for (name, output) in [
        ("moonshine", emit_moonshine_component(&ir, "Tabs")),
        ("solid", emit_solid_component(&ir, "Tabs")),
        ("svelte", emit_svelte_component(&ir)),
        ("vue", emit_vue_component(&ir)),
    ] {
        let output = if name == "vue" {
            output
                .replace("&quot;", "\"")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&")
        } else {
            output
        };
        assert!(
            output.contains(&format!("?.({argument})")),
            "{name}: {output}"
        );
    }
}

#[test]
fn inherited_handler_names_require_explicit_registration() {
    let ir = render_template_to_ir(
        "button @click=__proto__ \"Proto\"\nbutton @click=constructor \"Constructor\"\nbutton @click=toString \"String\"",
        &TemplateContext::new(),
    ).unwrap();
    for output in [
        emit_moonshine_component(&ir, "Actions"),
        emit_solid_component(&ir, "Actions"),
        emit_svelte_component(&ir),
        emit_vue_component(&ir),
    ] {
        assert!(output.contains("globalThis.Object.prototype.hasOwnProperty.call(handlers, key)"));
        assert!(output.contains("handlers != null"));
        for key in ["__proto__", "constructor", "toString"] {
            assert!(output.contains(&format!("?.{key}?.()")), "{output}");
        }
    }
}
