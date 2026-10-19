#![allow(clippy::module_inception)]

#[cfg(all(target_arch = "wasm32", feature = "dom"))]
pub mod dom {
    use crate::Effect;

    fn get_el(id: &str) -> Option<web_sys::Element> {
        web_sys::window()?.document()?.get_element_by_id(id)
    }

    /// Bind element textContent to a reactive closure and return its lifecycle handle.
    pub fn bind_text(node_id: &str, f: impl Fn() -> String + 'static) -> Effect {
        let id = node_id.to_string();
        Effect::new(move || {
            let text = f();
            if let Some(el) = get_el(&id) {
                el.set_text_content(Some(&text));
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
                if active {
                    let _ = cl.add_1(&cls);
                } else {
                    let _ = cl.remove_1(&cls);
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
                let _ = el.set_attribute(&attr, &val);
            }
        })
    }
}
