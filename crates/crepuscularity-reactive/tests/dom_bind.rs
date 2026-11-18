#[cfg(all(target_arch = "wasm32", feature = "dom"))]
mod tests {
    use crepuscularity_reactive::{bind_attr, bind_class, bind_text, hydrate_root, Signal};
    use wasm_bindgen_test::*;
    use web_sys::window;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn boolean_attributes_remove_false_values_and_stop_after_disposal() {
        use crepuscularity_reactive::bind_bool_attr;
        let document = window().unwrap().document().unwrap();
        let input = document.create_element("input").unwrap();
        input.set_id("boolean-attribute-input");
        input.set_attribute("disabled", "false").unwrap();
        document.body().unwrap().append_child(&input).unwrap();
        let signal = Signal::new(false);
        let effect = {
            let signal = signal.clone();
            bind_bool_attr("boolean-attribute-input", "disabled", move || signal.get())
        };
        assert!(!input.has_attribute("disabled"));
        signal.set(true);
        assert!(input.has_attribute("disabled"));
        signal.set(false);
        assert!(!input.has_attribute("disabled"));
        effect.dispose();
        signal.set(true);
        assert!(!input.has_attribute("disabled"));
        input.remove();
    }

    #[wasm_bindgen_test]
    fn checked_bindings_update_edited_checkboxes_and_stop_after_disposal() {
        use crepuscularity_reactive::bind_checked;
        use wasm_bindgen::JsCast;
        let document = window().unwrap().document().unwrap();
        let input: web_sys::HtmlInputElement = document
            .create_element("input")
            .unwrap()
            .dyn_into()
            .unwrap();
        input.set_id("live-checked-checkbox");
        input.set_type("checkbox");
        input.set_checked(true);
        document.body().unwrap().append_child(&input).unwrap();
        let signal = Signal::new(false);
        let effect = {
            let signal = signal.clone();
            bind_checked("live-checked-checkbox", move || signal.get())
        };
        assert!(!input.checked());
        signal.set(true);
        assert!(input.checked());
        input.set_checked(false);
        signal.set(false);
        signal.set(true);
        assert!(input.checked());
        effect.dispose();
        signal.set(false);
        assert!(input.checked());
        input.remove();
    }

    #[wasm_bindgen_test]
    fn live_values_update_edited_controls_and_stop_after_disposal() {
        use crepuscularity_reactive::bind_value;
        use wasm_bindgen::JsCast;
        let document = window().unwrap().document().unwrap();
        let signal = Signal::new("initial".to_string());
        let input: web_sys::HtmlInputElement = document
            .create_element("input")
            .unwrap()
            .dyn_into()
            .unwrap();
        input.set_id("live-value-input");
        input.set_value("edited");
        let textarea: web_sys::HtmlTextAreaElement = document
            .create_element("textarea")
            .unwrap()
            .dyn_into()
            .unwrap();
        textarea.set_id("live-value-textarea");
        textarea.set_value("edited");
        document.body().unwrap().append_child(&input).unwrap();
        document.body().unwrap().append_child(&textarea).unwrap();
        let effects: Vec<_> = ["live-value-input", "live-value-textarea"]
            .iter()
            .map(|id| {
                let signal = signal.clone();
                bind_value(id, move || signal.get())
            })
            .collect();
        assert_eq!(input.value(), "initial");
        assert_eq!(textarea.value(), "initial");
        signal.set("updated".to_string());
        assert_eq!(input.value(), "updated");
        assert_eq!(textarea.value(), "updated");
        for effect in effects {
            effect.dispose();
        }
        signal.set("disposed".to_string());
        assert_eq!(input.value(), "updated");
        assert_eq!(textarea.value(), "updated");
        input.remove();
        textarea.remove();
    }

    #[wasm_bindgen_test]
    fn select_values_override_initial_selection_recover_and_stop_after_disposal() {
        use crepuscularity_reactive::bind_value;
        use wasm_bindgen::JsCast;
        let document = window().unwrap().document().unwrap();
        let select: web_sys::HtmlSelectElement = document
            .create_element("select")
            .unwrap()
            .dyn_into()
            .unwrap();
        select.set_id("live-value-select");
        select.set_inner_html("<option value=\"a\">A</option><option value=\"b\">B</option>");
        select.set_value("b");
        document.body().unwrap().append_child(&select).unwrap();
        let value = Signal::new("a".to_string());
        let effect = bind_value("live-value-select", {
            let value = value.clone();
            move || value.get()
        });
        assert_eq!(select.value(), "a");
        value.set("missing".to_string());
        assert_eq!(select.value(), "");
        assert_eq!(select.selected_index(), -1);
        value.set("b".to_string());
        assert_eq!(select.value(), "b");
        effect.dispose();
        value.set("a".to_string());
        assert_eq!(select.value(), "b");
        select.remove();
    }

    #[wasm_bindgen_test]
    fn empty_select_values_choose_an_empty_option_after_no_selection() {
        use crepuscularity_reactive::bind_value;
        use wasm_bindgen::JsCast;
        let document = window().unwrap().document().unwrap();
        let select: web_sys::HtmlSelectElement = document
            .create_element("select")
            .unwrap()
            .dyn_into()
            .unwrap();
        select.set_id("empty-value-select");
        select.set_inner_html("<option value=''>Empty choice</option><option value='b'>B</option>");
        document.body().unwrap().append_child(&select).unwrap();
        select.set_selected_index(-1);
        assert_eq!(select.value(), "");
        let value = Signal::new(String::new());
        let effect = bind_value("empty-value-select", {
            let value = value.clone();
            move || value.get()
        });
        assert_eq!(select.selected_index(), 0);
        value.set("missing".to_string());
        assert_eq!(select.selected_index(), -1);
        value.set(String::new());
        assert_eq!(select.selected_index(), 0);
        value.set("b".to_string());
        assert_eq!(select.selected_index(), 1);
        effect.dispose();
        select.set_selected_index(-1);
        value.set(String::new());
        assert_eq!(select.selected_index(), -1);
        select.remove();
    }

    #[wasm_bindgen::prelude::wasm_bindgen(inline_js = "
export function crepusTestSeedFile(input) {
  const transfer = new DataTransfer();
  transfer.items.add(new File(['test'], 'fixture.txt', {type: 'text/plain'}));
  input.files = transfer.files;
}
export function crepusTestFileCount(input) { return input.files.length; }
export function crepusTestXlinkWidth(element) { return element.getBBox().width; }
let attributeObserver;
export function crepusTestObserveAttributes(element) {
  attributeObserver = new MutationObserver(() => {});
  attributeObserver.observe(element, {attributes: true});
}
export function crepusTestAttributeMutations() {
  const count = attributeObserver.takeRecords().length;
  attributeObserver.disconnect();
  return count;
}
let textObserver;
export function crepusTestObserveText(element) {
  textObserver = new MutationObserver(() => {});
  textObserver.observe(element, {childList: true, characterData: true, subtree: true});
}
export function crepusTestTextMutations() {
  const records = textObserver.takeRecords();
  textObserver.disconnect();
  return JSON.stringify({childList: records.filter(r => r.type === 'childList').length,
    characterData: records.filter(r => r.type === 'characterData').length});
}
")]
    extern "C" {
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = crepusTestSeedFile)]
        fn seed_test_file(input: &web_sys::HtmlInputElement);
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = crepusTestFileCount)]
        fn test_file_count(input: &web_sys::HtmlInputElement) -> u32;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = crepusTestXlinkWidth)]
        fn test_xlink_width(element: &web_sys::Element) -> f64;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = crepusTestObserveText)]
        fn observe_test_text(element: &web_sys::Element);
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = crepusTestTextMutations)]
        fn take_test_text_mutations() -> String;
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = crepusTestObserveAttributes)]
        fn observe_test_attributes(element: &web_sys::Element);
        #[wasm_bindgen::prelude::wasm_bindgen(js_name = crepusTestAttributeMutations)]
        fn take_test_attribute_mutations() -> u32;
    }

    #[wasm_bindgen_test]
    fn file_values_ignore_nonempty_assignments_clear_and_stop_after_disposal() {
        use crepuscularity_reactive::bind_value;
        use wasm_bindgen::JsCast;
        let document = window().unwrap().document().unwrap();
        let input: web_sys::HtmlInputElement = document
            .create_element("input")
            .unwrap()
            .dyn_into()
            .unwrap();
        input.set_id("live-value-file");
        input.set_type("file");
        document.body().unwrap().append_child(&input).unwrap();
        let value = Signal::new("forbidden.txt".to_string());
        let effect = bind_value("live-value-file", {
            let value = value.clone();
            move || value.get()
        });
        assert_eq!(input.value(), "");
        seed_test_file(&input);
        assert_eq!(test_file_count(&input), 1);
        value.set("still-forbidden.txt".to_string());
        assert_eq!(test_file_count(&input), 1);
        value.set(String::new());
        assert_eq!(input.value(), "");
        assert_eq!(test_file_count(&input), 0);
        effect.dispose();
        seed_test_file(&input);
        value.set("disposed.txt".to_string());
        value.set(String::new());
        assert_eq!(test_file_count(&input), 1);
        input.remove();
    }

    #[wasm_bindgen_test]
    fn xlink_attributes_keep_their_namespace_through_updates_and_disposal() {
        const XLINK: &str = "http://www.w3.org/1999/xlink";
        let document = window().unwrap().document().unwrap();
        let element = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), "use")
            .unwrap();
        element.set_id("reactive-xlink-use");
        let svg = document
            .create_element_ns(Some("http://www.w3.org/2000/svg"), "svg")
            .unwrap();
        svg.set_inner_html("<defs><rect id='first' width='32' height='20'/><rect id='second' width='64' height='20'/><rect id='recovered' width='48' height='20'/></defs>");
        svg.append_child(&element).unwrap();
        document.body().unwrap().append_child(&svg).unwrap();
        let value = Signal::new("#first".to_string());
        let effect = bind_attr("reactive-xlink-use", "xlink:href", {
            let value = value.clone();
            move || value.get()
        });
        assert_eq!(
            element.get_attribute_ns(Some(XLINK), "href").as_deref(),
            Some("#first")
        );
        assert_eq!(test_xlink_width(&element), 32.0);
        value.set("#second".to_string());
        assert_eq!(
            element.get_attribute_ns(Some(XLINK), "href").as_deref(),
            Some("#second")
        );
        assert_eq!(test_xlink_width(&element), 64.0);
        value.set("javascript:unsafe".to_string());
        assert!(!element.has_attribute_ns(Some(XLINK), "href"));
        assert_eq!(test_xlink_width(&element), 0.0);
        value.set("#recovered".to_string());
        assert_eq!(
            element.get_attribute_ns(Some(XLINK), "href").as_deref(),
            Some("#recovered")
        );
        assert_eq!(test_xlink_width(&element), 48.0);
        effect.dispose();
        value.set("#disposed".to_string());
        assert_eq!(
            element.get_attribute_ns(Some(XLINK), "href").as_deref(),
            Some("#recovered")
        );
        assert_eq!(test_xlink_width(&element), 48.0);
        svg.remove();
    }

    #[wasm_bindgen_test]
    fn test_bind_class() {
        let document = window()
            .expect("browser window")
            .document()
            .expect("document");
        let el = document.create_element("div").unwrap();
        el.set_id("test-class");
        el.set_class_name("existing");

        let body = document.body().unwrap();
        body.append_child(&el).unwrap();

        let sig = Signal::new(false);

        let sig_clone = sig.clone();
        let _effect = bind_class("test-class", "active", move || sig_clone.get());

        // Initially false, so should not have the class
        assert!(!el.class_list().contains("active"));

        // Set to true
        sig.set(true);
        assert!(el.class_list().contains("active"));

        // Set back to false
        sig.set(false);
        assert!(!el.class_list().contains("active"));

        assert!(el.class_list().contains("existing"));
        _effect.dispose();

        // Let's clean up
        body.remove_child(&el).unwrap();
    }

    #[wasm_bindgen_test]
    fn text_and_attribute_bindings_update_and_stop_after_disposal() {
        let document = window().unwrap().document().unwrap();
        let el = document.create_element("div").unwrap();
        el.set_id("test-text-attribute");
        document.body().unwrap().append_child(&el).unwrap();
        let signal = Signal::new("initial".to_string());
        let text = bind_text("test-text-attribute", {
            let signal = signal.clone();
            move || signal.get()
        });
        let attr = bind_attr("test-text-attribute", "aria-label", {
            let signal = signal.clone();
            move || signal.get()
        });
        assert_eq!(el.text_content().as_deref(), Some("initial"));
        signal.set("updated <text> 界".to_string());
        assert_eq!(el.text_content().as_deref(), Some("updated <text> 界"));
        assert_eq!(
            el.get_attribute("aria-label").as_deref(),
            Some("updated <text> 界")
        );
        assert_eq!(el.child_element_count(), 0);
        text.dispose();
        attr.dispose();
        signal.set("disposed".to_string());
        assert_eq!(el.text_content().as_deref(), Some("updated <text> 界"));
        assert_eq!(
            el.get_attribute("aria-label").as_deref(),
            Some("updated <text> 界")
        );
        el.remove();
    }

    #[wasm_bindgen_test]
    fn text_bindings_avoid_redundant_mutations_and_keep_flat_text_semantics() {
        let document = window().unwrap().document().unwrap();
        let element = document.create_element("div").unwrap();
        element.set_id("stable-reactive-text");
        document.body().unwrap().append_child(&element).unwrap();
        let tick = Signal::new(0usize);
        let text = Signal::new("stable 界".to_string());
        let effect = bind_text("stable-reactive-text", {
            let tick = tick.clone();
            let text = text.clone();
            move || {
                tick.get();
                text.get()
            }
        });
        observe_test_text(&element);
        for value in 1..=1000 {
            tick.set(value);
        }
        let mutations = take_test_text_mutations();
        wasm_bindgen_test::console_log!("1000 unchanged text updates: {}", mutations);
        let counts: serde_json::Value = serde_json::from_str(&mutations).unwrap();
        assert_eq!(counts["childList"], 0);
        assert_eq!(counts["characterData"], 0);
        let original = element.first_child().unwrap();
        observe_test_text(&element);
        text.set("changed <text>".to_string());
        let counts: serde_json::Value = serde_json::from_str(&take_test_text_mutations()).unwrap();
        assert_eq!(counts["childList"], 0);
        assert_eq!(counts["characterData"], 1);
        assert!(original.is_same_node(element.first_child().as_ref()));
        assert_eq!(element.text_content().as_deref(), Some("changed <text>"));
        for markup in [
            "<b>changed &lt;text&gt;</b>",
            "changed <!--comment-->&lt;text&gt;",
        ] {
            element.set_inner_html(markup);
            tick.update(|value| value + 1);
            assert_eq!(element.child_element_count(), 0);
            let node = element.first_child().unwrap();
            assert_eq!(node.node_type(), web_sys::Node::TEXT_NODE);
            assert!(node.next_sibling().is_none());
            assert_eq!(element.text_content().as_deref(), Some("changed <text>"));
        }
        text.set(String::new());
        assert!(element.first_child().is_none());
        element.set_inner_html("<!--empty--> ");
        tick.update(|value| value + 1);
        assert!(element.first_child().is_none());
        observe_test_text(&element);
        tick.update(|value| value + 1);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&take_test_text_mutations()).unwrap()
                ["childList"],
            0
        );
        effect.dispose();
        text.set("disposed".to_string());
        assert!(element.first_child().is_none());
        element.remove();
    }

    #[wasm_bindgen_test]
    fn hydration_reads_canonical_dom_payload_and_rejects_invalid_context() {
        use base64::Engine;
        let document = window().unwrap().document().unwrap();
        let el = document.create_element("script").unwrap();
        el.set_id("__crepus_hydration__");
        el.set_attribute("type", "application/json").unwrap();
        el.set_attribute("data-crepus-encoding", "base64").unwrap();
        document.body().unwrap().append_child(&el).unwrap();
        for payload in [
            r#"{"v":1,"ctx":{"name":"界 <text>","enabled":false},"bind":{}}"#,
            r#"{"v":1,"ctx":null,"bind":{}}"#,
        ] {
            el.set_text_content(Some(
                &base64::engine::general_purpose::STANDARD.encode(payload),
            ));
            hydrate_root(|ctx| {
                if payload.contains("null") {
                    assert!(ctx.is_empty());
                } else {
                    assert_eq!(ctx["name"], "界 <text>");
                    assert_eq!(ctx["enabled"], false);
                    assert!(!ctx.contains_key("bind"));
                }
            });
        }
        el.remove();
    }

    #[wasm_bindgen_test]
    fn hydration_preserves_legacy_application_ctx_objects() {
        use base64::Engine;
        let document = window().unwrap().document().unwrap();
        let el = document.create_element("script").unwrap();
        el.set_id("__crepus_ctx__");
        el.set_attribute("type", "application/json").unwrap();
        document.body().unwrap().append_child(&el).unwrap();
        let payload = r#"{"ctx":{"nested":1},"name":"Ada"}"#;
        for encoded in [false, true] {
            if encoded {
                el.set_attribute("data-encoding", "base64").unwrap();
                el.set_text_content(Some(
                    &base64::engine::general_purpose::STANDARD.encode(payload),
                ));
            } else {
                el.remove_attribute("data-encoding").unwrap();
                el.set_text_content(Some(payload));
            }
            hydrate_root(|ctx| {
                assert_eq!(ctx["name"], "Ada");
                assert_eq!(ctx["ctx"]["nested"], 1);
            });
        }
        el.remove();
    }

    #[wasm_bindgen_test]
    fn test_bind_class_missing_el() {
        // Missing element shouldn't panic
        let sig = Signal::new(false);
        let sig_clone = sig.clone();
        let _effect = bind_class("non-existent-class-el", "active", move || sig_clone.get());
        sig.set(true);
        sig.set(false);
    }
    #[wasm_bindgen_test]
    fn unsafe_url_updates_remove_attributes_and_safe_values_recover() {
        let document = window().unwrap().document().unwrap();
        let element = document.create_element("a").unwrap();
        element.set_id("reactive-safe-url");
        document.body().unwrap().append_child(&element).unwrap();
        let url = Signal::new("/initial".to_string());
        let read_url = url.clone();
        let effect = bind_attr("reactive-safe-url", "HREF", move || read_url.get());
        assert_eq!(element.get_attribute("href").as_deref(), Some("/initial"));
        for unsafe_url in [
            "javascript:unsafe",
            "java\nscript:unsafe",
            "data:text/html,unsafe",
        ] {
            url.set(unsafe_url.to_string());
            assert!(!element.has_attribute("href"));
        }
        url.set("/recovered".to_string());
        assert_eq!(element.get_attribute("href").as_deref(), Some("/recovered"));
        effect.dispose();
        element.remove();
    }
    #[wasm_bindgen_test]
    fn queued_dom_disposal_keeps_live_bindings_and_releases_canceled_work() {
        use crepuscularity_reactive::{batch_begin, batch_end, Memo};
        let document = window().unwrap().document().unwrap();
        let root = document.create_element("div").unwrap();
        document.body().unwrap().append_child(&root).unwrap();
        let signal = Signal::new("initial".to_owned());
        let memo = Memo::new({
            let signal = signal.clone();
            move || signal.get()
        });
        let mut elements = Vec::new();
        let mut effects = Vec::new();
        for index in 0..64 {
            let element = document.create_element("span").unwrap();
            let id = format!("queued-disposal-{index}");
            element.set_id(&id);
            root.append_child(&element).unwrap();
            effects.push(bind_text(&id, {
                let memo = memo.clone();
                move || memo.get()
            }));
            elements.push(element);
        }
        batch_begin();
        signal.set("updated".to_owned());
        // A fresh read queues the memo's DOM subscribers inside the batch.
        assert_eq!(memo.get(), "updated");
        for effect in effects.drain(..63) {
            effect.dispose();
        }
        batch_end();
        for element in &elements[..63] {
            assert_eq!(element.text_content().as_deref(), Some("initial"));
        }
        assert_eq!(elements[63].text_content().as_deref(), Some("updated"));
        effects.pop().unwrap().dispose();
        signal.set("later".to_owned());
        assert_eq!(elements[63].text_content().as_deref(), Some("updated"));
        memo.dispose();
        root.remove();
    }
    #[wasm_bindgen_test]
    fn unchanged_attributes_do_not_emit_mutations_but_external_edits_recover() {
        const XLINK: &str = "http://www.w3.org/1999/xlink";
        let document = window().unwrap().document().unwrap();
        let mut unchanged = Vec::new();
        for attr in ["title", "xlink:href", "srcdoc"] {
            let element = document.create_element("div").unwrap();
            element.set_id("stable-reactive-attribute");
            document.body().unwrap().append_child(&element).unwrap();
            let tick = Signal::new(0usize);
            let value = Signal::new("stable".to_owned());
            let effect = bind_attr("stable-reactive-attribute", attr, {
                let tick = tick.clone();
                let value = value.clone();
                move || {
                    tick.get();
                    value.get()
                }
            });
            observe_test_attributes(&element);
            for index in 1..=1000 {
                tick.set(index);
            }
            let count = take_test_attribute_mutations();
            wasm_bindgen_test::console_log!("1000 unchanged {} updates: {}", attr, count);
            unchanged.push(count);
            observe_test_attributes(&element);
            value.set("changed".to_owned());
            assert_eq!(take_test_attribute_mutations(), 1);
            if attr == "xlink:href" {
                element
                    .set_attribute_ns(Some(XLINK), attr, "external")
                    .unwrap();
            } else {
                element.set_attribute(attr, "external").unwrap();
            }
            observe_test_attributes(&element);
            tick.set(1001);
            assert_eq!(take_test_attribute_mutations(), 1);
            assert_eq!(element.get_attribute(attr).as_deref(), Some("changed"));
            effect.dispose();
            observe_test_attributes(&element);
            value.set("disposed".to_owned());
            assert_eq!(take_test_attribute_mutations(), 0);
            element.remove();
        }
        assert_eq!(unchanged, [0, 0, 0]);
    }
    #[wasm_bindgen_test]
    fn unchanged_class_presence_skips_mutations_and_external_edits_recover() {
        let document = window().unwrap().document().unwrap();
        let element = document.create_element("div").unwrap();
        element.set_id("stable-reactive-class");
        element.set_class_name("existing");
        document.body().unwrap().append_child(&element).unwrap();
        let tick = Signal::new(0usize);
        let active = Signal::new(true);
        let effect = bind_class("stable-reactive-class", "active", {
            let tick = tick.clone();
            let active = active.clone();
            move || {
                tick.get();
                active.get()
            }
        });
        let mut unchanged = Vec::new();
        for state in [true, false] {
            active.set(state);
            observe_test_attributes(&element);
            for _ in 0..1000 {
                tick.update(|value| value + 1);
            }
            let count = take_test_attribute_mutations();
            wasm_bindgen_test::console_log!("1000 unchanged class {} updates: {}", state, count);
            unchanged.push(count);
            if state {
                element.class_list().remove_1("active").unwrap();
            } else {
                element.class_list().add_1("active").unwrap();
            }
            observe_test_attributes(&element);
            tick.update(|value| value + 1);
            assert_eq!(take_test_attribute_mutations(), 1);
            assert_eq!(element.class_list().contains("active"), state);
            assert!(element.class_list().contains("existing"));
        }
        effect.dispose();
        observe_test_attributes(&element);
        active.set(true);
        assert_eq!(take_test_attribute_mutations(), 0);
        assert!(!element.class_list().contains("active"));
        element.remove();
        assert_eq!(unchanged, [0, 0]);
    }
}
