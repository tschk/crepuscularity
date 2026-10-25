use std::fmt;

use js_sys::{Array, Function, Promise, Reflect};
use serde::{de::DeserializeOwned, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

pub type Result<T> = std::result::Result<T, BrowserError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserError {
    ApiUnavailable(String),
    MethodUnavailable(String),
    LastError(String),
    Serde(String),
    Js(String),
}

impl fmt::Display for BrowserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BrowserError::ApiUnavailable(name) => write!(f, "browser API unavailable: {name}"),
            BrowserError::MethodUnavailable(name) => {
                write!(f, "browser method unavailable: {name}")
            }
            BrowserError::LastError(message) => write!(f, "browser runtime error: {message}"),
            BrowserError::Serde(message) => write!(f, "browser value serde error: {message}"),
            BrowserError::Js(message) => write!(f, "browser JavaScript error: {message}"),
        }
    }
}

impl std::error::Error for BrowserError {}

impl From<JsValue> for BrowserError {
    fn from(value: JsValue) -> Self {
        BrowserError::Js(js_error_message(&value))
    }
}

pub fn browser() -> Result<JsValue> {
    let global = js_sys::global();
    for key in ["browser", "chrome"] {
        let value = Reflect::get(&global, &JsValue::from_str(key)).map_err(BrowserError::from)?;
        if !value.is_undefined() && !value.is_null() {
            return Ok(value);
        }
    }
    Err(BrowserError::ApiUnavailable("browser/chrome".to_string()))
}

pub fn has_browser_global() -> bool {
    Reflect::get(&js_sys::global(), &JsValue::from_str("browser"))
        .ok()
        .is_some_and(|value| !value.is_undefined() && !value.is_null())
}

#[wasm_bindgen(inline_js = "
export function get_path_js(root, path) {
    let current = root;
    let parts = path.split('.');
    for (let i = 0; i < parts.length; i++) {
        current = current[parts[i]];
        if (current === undefined || current === null) {
            return current;
        }
    }
    return current;
}
")]
extern "C" {
    #[wasm_bindgen(catch)]
    fn get_path_js(root: &JsValue, path: &str) -> std::result::Result<JsValue, JsValue>;
}

pub fn get_path(root: &JsValue, path: &str) -> Result<JsValue> {
    let result = get_path_js(root, path).map_err(BrowserError::from)?;
    if result.is_undefined() || result.is_null() {
        return Err(BrowserError::ApiUnavailable(path.to_string()));
    }
    Ok(result)
}

pub fn namespace(path: &str) -> Result<JsValue> {
    get_path(&browser()?, path)
}

pub fn raw_namespace(path: &str) -> Result<RawNamespace> {
    Ok(RawNamespace {
        path: path.to_string(),
        value: namespace(path)?,
    })
}

#[derive(Clone, Debug)]
pub struct RawNamespace {
    path: String,
    value: JsValue,
}

impl RawNamespace {
    pub fn value(&self) -> &JsValue {
        &self.value
    }

    pub async fn call(&self, method: &str, args: &[JsValue]) -> Result<JsValue> {
        call_method(
            &self.value,
            &format!("{}.{}", self.path, method),
            method,
            args,
        )
        .await
    }

    pub async fn call_callback(&self, method: &str, args: &[JsValue]) -> Result<JsValue> {
        call_callback_method(
            &self.value,
            &format!("{}.{}", self.path, method),
            method,
            args,
        )
        .await
    }

    pub fn on_raw<F>(&self, event: &str, handler: F) -> Result<EventListenerGuard>
    where
        F: FnMut(JsValue, JsValue, JsValue) + 'static,
    {
        add_event_listener(
            &self.value,
            &format!("{}.{}", self.path, event),
            event,
            handler,
        )
    }
}

pub async fn call_browser_method(
    target: &JsValue,
    display_name: &str,
    method: &str,
    args: &[JsValue],
) -> Result<JsValue> {
    if has_browser_global() {
        call_method(target, display_name, method, args).await
    } else {
        call_callback_method(target, display_name, method, args).await
    }
}

pub async fn call_method(
    target: &JsValue,
    display_name: &str,
    method: &str,
    args: &[JsValue],
) -> Result<JsValue> {
    let function = method_function(target, display_name, method)?;
    let result = apply(&function, target, args)?;
    if is_promise_like(&result) {
        await_promise(Promise::from(result)).await
    } else {
        check_last_error()?;
        Ok(result)
    }
}

pub fn call_method_blocking(
    target: &JsValue,
    display_name: &str,
    method: &str,
    args: &[JsValue],
) -> Result<JsValue> {
    let function = method_function(target, display_name, method)?;
    apply(&function, target, args)
}

pub async fn call_callback_method(
    target: &JsValue,
    display_name: &str,
    method: &str,
    args: &[JsValue],
) -> Result<JsValue> {
    let function = method_function(target, display_name, method)?;
    let this = target.clone();
    let args = args.to_vec();
    let promise = Promise::new(&mut |resolve, reject| {
        let callback_reject = reject.clone();
        let callback = Closure::once_into_js(move |value: JsValue| {
            if let Some(error) = last_error() {
                let _ = callback_reject.call1(&JsValue::UNDEFINED, &JsValue::from_str(&error));
            } else {
                let _ = resolve.call1(&JsValue::UNDEFINED, &value);
            }
        });
        let mut with_callback = args.clone();
        with_callback.push(callback);
        if let Err(error) = apply(&function, &this, &with_callback) {
            let _ = reject.call1(&JsValue::UNDEFINED, &JsValue::from_str(&error.to_string()));
        }
    });
    await_promise(promise).await
}

pub async fn await_promise(promise: Promise) -> Result<JsValue> {
    JsFuture::from(promise).await.map_err(BrowserError::from)
}

pub fn add_event_listener<F>(
    target: &JsValue,
    display_name: &str,
    event: &str,
    handler: F,
) -> Result<EventListenerGuard>
where
    F: FnMut(JsValue, JsValue, JsValue) + 'static,
{
    let event_value =
        Reflect::get(target, &JsValue::from_str(event)).map_err(BrowserError::from)?;
    if event_value.is_undefined() || event_value.is_null() {
        return Err(BrowserError::ApiUnavailable(display_name.to_string()));
    }
    let add = method_function(
        &event_value,
        &format!("{display_name}.addListener"),
        "addListener",
    )?;
    let remove = method_function(
        &event_value,
        &format!("{display_name}.removeListener"),
        "removeListener",
    )?;
    let closure = Closure::wrap(Box::new(handler) as Box<dyn FnMut(JsValue, JsValue, JsValue)>);
    add.call1(&event_value, closure.as_ref())
        .map_err(BrowserError::from)?;
    Ok(EventListenerGuard {
        event: event_value,
        remove,
        closure,
    })
}

pub struct EventListenerGuard {
    event: JsValue,
    remove: Function,
    closure: Closure<dyn FnMut(JsValue, JsValue, JsValue)>,
}

impl Drop for EventListenerGuard {
    fn drop(&mut self) {
        let _ = self.remove.call1(&self.event, self.closure.as_ref());
    }
}

pub fn to_js<T>(value: &T) -> Result<JsValue>
where
    T: Serialize,
{
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|error| BrowserError::Serde(error.to_string()))
}

pub fn from_js<T>(value: JsValue) -> Result<T>
where
    T: DeserializeOwned,
{
    serde_wasm_bindgen::from_value(value).map_err(|error| BrowserError::Serde(error.to_string()))
}

pub fn last_error() -> Option<String> {
    let chrome = Reflect::get(&js_sys::global(), &JsValue::from_str("chrome")).ok()?;
    let runtime = Reflect::get(&chrome, &JsValue::from_str("runtime")).ok()?;
    let last_error = Reflect::get(&runtime, &JsValue::from_str("lastError")).ok()?;
    if last_error.is_undefined() || last_error.is_null() {
        return None;
    }
    let message = Reflect::get(&last_error, &JsValue::from_str("message")).ok()?;
    message
        .as_string()
        .or_else(|| Some(js_error_message(&last_error)))
}

fn check_last_error() -> Result<()> {
    if let Some(error) = last_error() {
        Err(BrowserError::LastError(error))
    } else {
        Ok(())
    }
}

fn method_function(target: &JsValue, display_name: &str, method: &str) -> Result<Function> {
    let value = Reflect::get(target, &JsValue::from_str(method)).map_err(BrowserError::from)?;
    value
        .dyn_into::<Function>()
        .map_err(|_| BrowserError::MethodUnavailable(display_name.to_string()))
}

fn apply(function: &Function, target: &JsValue, args: &[JsValue]) -> Result<JsValue> {
    let js_args = Array::new();
    for arg in args {
        js_args.push(arg);
    }
    function
        .apply(target, &js_args)
        .map_err(BrowserError::from)
        .and_then(|value| {
            check_last_error()?;
            Ok(value)
        })
}

fn is_promise_like(value: &JsValue) -> bool {
    if value.is_undefined() || value.is_null() {
        return false;
    }
    Reflect::get(value, &JsValue::from_str("then"))
        .ok()
        .and_then(|then| then.dyn_into::<Function>().ok())
        .is_some()
}

fn js_error_message(value: &JsValue) -> String {
    if let Some(message) = value.as_string() {
        return message;
    }
    Reflect::get(value, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .unwrap_or_else(|| format!("{value:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use wasm_bindgen::JsValue;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
        is_active: bool,
    }

    #[wasm_bindgen_test]
    fn test_to_js_and_from_js() {
        let data = TestData {
            name: "test".to_string(),
            value: 42,
            is_active: true,
        };

        // Test serialization to JsValue
        let js_val: JsValue = to_js(&data).unwrap();
        assert!(!js_val.is_undefined());
        assert!(!js_val.is_null());

        // Test deserialization from JsValue back to TestData
        let deserialized: TestData = from_js(js_val).unwrap();
        assert_eq!(data, deserialized);
    }

    #[wasm_bindgen_test]
    fn test_from_js_invalid_type() {
        // Try to deserialize a number into our struct, should fail
        let js_num = JsValue::from_f64(42.0);
        let result: Result<TestData> = from_js(js_num);
        assert!(result.is_err());

        if let Err(BrowserError::Serde(msg)) = result {
            // Error message should exist but format depends on serde-wasm-bindgen
            assert!(!msg.is_empty());
        } else {
            panic!("Expected BrowserError::Serde, got {:?}", result);
        }
    }

    #[wasm_bindgen_test]
    fn test_to_js_primitives() {
        let int_val: i32 = -42;
        let js_int = to_js(&int_val).unwrap();
        assert_eq!(js_int.as_f64().unwrap(), -42.0);

        let string_val = "hello world".to_string();
        let js_string = to_js(&string_val).unwrap();
        assert_eq!(js_string.as_string().unwrap(), "hello world");

        let bool_val = true;
        let js_bool = to_js(&bool_val).unwrap();
        assert_eq!(js_bool.as_bool().unwrap(), true);
    }
}
