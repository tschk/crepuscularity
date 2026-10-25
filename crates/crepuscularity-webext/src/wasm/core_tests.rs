#[cfg(test)]
mod tests {
    use crate::wasm::core::{from_js, to_js, BrowserError};
    use serde::{Deserialize, Serialize};
    use wasm_bindgen::JsValue;
    use wasm_bindgen_test::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        id: u32,
        name: String,
        active: bool,
    }

    #[wasm_bindgen_test]
    fn test_to_js_and_from_js() {
        let original = TestData {
            id: 1,
            name: "Test".to_string(),
            active: true,
        };

        let js_value = to_js(&original).expect("to_js failed");
        assert!(js_value.is_object());

        let deserialized: TestData = from_js(js_value).expect("from_js failed");
        assert_eq!(original, deserialized);
    }

    #[wasm_bindgen_test]
    fn test_from_js_error() {
        let invalid_js = JsValue::from_str("Not a valid object for TestData");

        let result: Result<TestData, BrowserError> = from_js(invalid_js);

        assert!(result.is_err());
        match result.unwrap_err() {
            BrowserError::Serde(_) => {}
            e => panic!("Expected Serde error, got {:?}", e),
        }
    }
}
