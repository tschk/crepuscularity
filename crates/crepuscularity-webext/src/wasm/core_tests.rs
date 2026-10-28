use super::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_test::*;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct TestStruct {
    a: i32,
    b: String,
}

#[wasm_bindgen_test]
fn test_to_js_and_from_js() {
    let original = TestStruct {
        a: 42,
        b: "hello".to_string(),
    };

    let js_value = to_js(&original).expect("to_js should succeed");
    assert!(!js_value.is_undefined());
    assert!(!js_value.is_null());

    let restored: TestStruct = from_js(js_value).expect("from_js should succeed");
    assert_eq!(original, restored);
}

#[wasm_bindgen_test]
fn test_from_js_invalid_type() {
    let js_value = JsValue::from_str("not an object");
    let result: Result<TestStruct> = from_js(js_value);
    assert!(result.is_err());

    if let Err(BrowserError::Serde(_)) = result {
        // Expected error
    } else {
        panic!("Expected BrowserError::Serde, got {:?}", result);
    }
}
