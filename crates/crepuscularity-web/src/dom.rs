use wasm_bindgen::JsValue;

#[derive(Clone, Copy, Debug)]
pub struct StaticElementRef {
    id: &'static str,
}

impl StaticElementRef {
    pub const fn new(id: &'static str) -> Self {
        Self { id }
    }

    pub const fn id(&self) -> &'static str {
        self.id
    }

    pub fn text(&self, text: &str) -> Result<(), JsValue> {
        by_id(self.id).text(text)
    }

    pub fn html(&self, html: &str) -> Result<(), JsValue> {
        by_id(self.id).html(html)
    }

    pub fn attr(&self, name: &str, value: &str) -> Result<(), JsValue> {
        by_id(self.id).attr(name, value)
    }
}

#[derive(Clone, Debug)]
pub struct ElementRef {
    id: String,
}

impl ElementRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn get(&self) -> Result<web_sys::Element, JsValue> {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| JsValue::from_str("window.document is unavailable"))?;
        document
            .get_element_by_id(&self.id)
            .ok_or_else(|| JsValue::from_str(&format!("missing DOM id `#{}`", self.id)))
    }

    pub fn text(&self, text: &str) -> Result<(), JsValue> {
        self.get()?.set_text_content(Some(text));
        Ok(())
    }

    pub fn html(&self, html: &str) -> Result<(), JsValue> {
        // Sanitize with ammonia before setting innerHTML to prevent XSS.
        // The SSR path already uses ammonia::clean; this mirrors that protection
        // for the WASM DOM API.
        let sanitized = ammonia::clean(html);
        self.get()?.set_inner_html(&sanitized);
        Ok(())
    }

    pub fn attr(&self, name: &str, value: &str) -> Result<(), JsValue> {
        let normalized = name.to_ascii_lowercase();
        if crate::is_url_attr(&normalized) && !crate::is_safe_url_value(&normalized, value) {
            return Err(JsValue::from_str("unsafe URL attribute value"));
        }
        self.get()?.set_attribute(name, value)
    }
}

pub fn by_id(id: impl Into<String>) -> ElementRef {
    ElementRef::new(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_ref_new() {
        assert_eq!(ElementRef::new("my-id").id(), "my-id");
        assert_eq!(
            ElementRef::new(String::from("another-id")).id(),
            "another-id"
        );
    }
}
