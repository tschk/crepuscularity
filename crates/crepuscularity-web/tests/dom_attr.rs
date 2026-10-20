#[cfg(all(target_arch = "wasm32", feature = "dom"))]
mod tests {
    use crepuscularity_web::dom::{ElementRef, StaticElementRef};
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_element_ref_attr_error() {
        let el = ElementRef::new("missing-attr");
        let err = el.attr("class", "foo").unwrap_err();
        let err_msg = err.as_string().unwrap();
        assert!(
            err_msg == "window.document is unavailable"
                || err_msg == "missing DOM id `#missing-attr`",
            "unexpected error message: {}",
            err_msg
        );
    }

    #[wasm_bindgen_test]
    fn test_static_element_ref_attr_error() {
        let el = StaticElementRef::new("missing-attr-static");
        let err = el.attr("class", "foo").unwrap_err();
        let err_msg = err.as_string().unwrap();
        assert!(
            err_msg == "window.document is unavailable"
                || err_msg == "missing DOM id `#missing-attr-static`",
            "unexpected error message: {}",
            err_msg
        );
    }
    #[wasm_bindgen_test]
    fn unsafe_url_attributes_fail_before_dom_lookup() {
        let element = ElementRef::new("missing-url");
        for (name, value) in [
            ("href", "javascript:alert(1)"),
            ("HREF", "javascript:alert(1)"),
            ("src", "java\nscript:alert(1)"),
            ("action", "data:text/html,unsafe"),
        ] {
            assert_eq!(
                element
                    .attr(name, value)
                    .unwrap_err()
                    .as_string()
                    .as_deref(),
                Some("unsafe URL attribute value")
            );
        }
    }
}
