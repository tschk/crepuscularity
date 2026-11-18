#[cfg(all(target_arch = "wasm32", feature = "dom"))]
pub mod bindings {
    use crate::Effect;
    use wasm_bindgen::JsCast;

    fn get_el(id: &str) -> Option<web_sys::Element> {
        web_sys::window()?.document()?.get_element_by_id(id)
    }

    /// Bind element textContent to a reactive closure and return its lifecycle handle.
    pub fn bind_text(node_id: &str, f: impl Fn() -> String + 'static) -> Effect {
        let id = node_id.to_string();
        Effect::new(move || {
            let text = f();
            if let Some(el) = get_el(&id) {
                let child = el.first_child();
                if text.is_empty() {
                    if child.is_some() {
                        el.set_text_content(None);
                    }
                } else if let Some(node) = child.filter(|node| {
                    node.node_type() == web_sys::Node::TEXT_NODE && node.next_sibling().is_none()
                }) {
                    if node.node_value().as_deref() != Some(text.as_str()) {
                        node.set_node_value(Some(&text));
                    }
                } else {
                    el.set_text_content(Some(&text));
                }
            }
        })
    }

    /// Bind a form control's live value and return its lifecycle handle.
    /// Nonempty file input values cannot be assigned by the browser.
    pub fn bind_value(node_id: &str, f: impl Fn() -> String + 'static) -> Effect {
        let id = node_id.to_string();
        Effect::new(move || {
            let value = f();
            let Some(el) = get_el(&id) else {
                return;
            };
            if let Some(input) = el.dyn_ref::<web_sys::HtmlInputElement>() {
                if input.value() != value && (input.type_() != "file" || value.is_empty()) {
                    input.set_value(&value);
                }
            } else if let Some(textarea) = el.dyn_ref::<web_sys::HtmlTextAreaElement>() {
                if textarea.value() != value {
                    textarea.set_value(&value);
                }
            } else if let Some(select) = el.dyn_ref::<web_sys::HtmlSelectElement>() {
                // An empty value also represents no selection, not just an empty option.
                if select.value() != value || select.selected_index() == -1 {
                    select.set_value(&value);
                }
            }
        })
    }

    /// Bind a checkbox or radio's live checked property and return its lifecycle handle.
    pub fn bind_checked(node_id: &str, f: impl Fn() -> bool + 'static) -> Effect {
        let id = node_id.to_string();
        Effect::new(move || {
            let checked = f();
            if let Some(el) = get_el(&id) {
                if let Some(input) = el.dyn_ref::<web_sys::HtmlInputElement>() {
                    if input.checked() != checked {
                        input.set_checked(checked);
                    }
                }
            }
        })
    }

    /// Bind class presence to a reactive closure and return its lifecycle handle.
    pub fn bind_class(node_id: &str, class: &str, f: impl Fn() -> bool + 'static) -> Effect {
        let id = node_id.to_string();
        let cls = class.to_string();
        Effect::new(move || {
            let active = f();
            if let Some(el) = get_el(&id) {
                let cl = el.class_list();
                if cl.contains(&cls) != active {
                    if active {
                        let _ = cl.add_1(&cls);
                    } else {
                        let _ = cl.remove_1(&cls);
                    }
                }
            }
        })
    }

    /// Bind HTML boolean attribute presence and return its lifecycle handle.
    pub fn bind_bool_attr(node_id: &str, attr: &str, f: impl Fn() -> bool + 'static) -> Effect {
        let id = node_id.to_string();
        let attr = attr.to_string();
        Effect::new(move || {
            let present = f();
            if let Some(el) = get_el(&id) {
                if el.has_attribute(&attr) != present {
                    if present {
                        let _ = el.set_attribute(&attr, "");
                    } else {
                        let _ = el.remove_attribute(&attr);
                    }
                }
            }
        })
    }

    /// Bind an attribute to a reactive closure and return its lifecycle handle.
    pub fn bind_attr(node_id: &str, attr: &str, f: impl Fn() -> String + 'static) -> Effect {
        let id = node_id.to_string();
        let attr = attr.to_string();
        Effect::new(move || {
            let val = f();
            if let Some(el) = get_el(&id) {
                if super::safe_attribute_value(&attr, &val) {
                    if attr == "xlink:href" {
                        if el
                            .get_attribute_ns(Some("http://www.w3.org/1999/xlink"), "href")
                            .as_deref()
                            != Some(val.as_str())
                        {
                            let _ = el.set_attribute_ns(
                                Some("http://www.w3.org/1999/xlink"),
                                &attr,
                                &val,
                            );
                        }
                    } else if el.get_attribute(&attr).as_deref() != Some(val.as_str()) {
                        let _ = el.set_attribute(&attr, &val);
                    }
                } else {
                    let _ = el.remove_attribute(&attr);
                    if attr == "xlink:href" {
                        let _ =
                            el.remove_attribute_ns(Some("http://www.w3.org/1999/xlink"), "href");
                    }
                }
            }
        })
    }
}

#[cfg(any(test, all(target_arch = "wasm32", feature = "dom")))]
fn safe_attribute_value(name: &str, value: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if !matches!(
        name.as_str(),
        "href" | "src" | "srcset" | "action" | "formaction" | "poster" | "data" | "xlink:href"
    ) {
        return true;
    }
    let mut scheme = String::new();
    for character in value
        .trim_start_matches(|character: char| character <= ' ')
        .chars()
    {
        match character {
            '\t' | '\n' | '\r' => continue,
            ':' => {
                return match scheme.as_str() {
                    "javascript" | "vbscript" => false,
                    "data" => !matches!(
                        name.as_str(),
                        "href" | "xlink:href" | "action" | "formaction"
                    ),
                    _ => true,
                }
            }
            character => scheme.push(character.to_ascii_lowercase()),
        }
    }
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn reactive_url_values_match_rendering_policy() {
        for (name, value, allowed) in [
            ("HREF", "javascript:unsafe", false),
            ("src", "java\nscript:unsafe", false),
            ("action", "data:text/html,unsafe", false),
            ("src", "data:image/png;base64,abc", true),
            ("href", "/local", true),
            ("href", "https://example.com", true),
            ("title", "javascript:literal", true),
        ] {
            assert_eq!(
                super::safe_attribute_value(name, value),
                allowed,
                "{name}: {value}"
            );
        }
    }
}
