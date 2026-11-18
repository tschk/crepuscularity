//! Browser API abstraction for generating cross-browser compatible JavaScript.
//!
//! This module provides a builder pattern for constructing browser extension
//! API calls that work across Chrome, Firefox, and other browsers.

use std::collections::BTreeMap;

use serde_json::Value;

/// A program that interacts with browser APIs.
///
/// BrowserProgram provides a builder pattern for constructing sequences of
/// browser API operations (storage, messaging, etc.) that can be emitted
/// as JavaScript module code.
#[derive(Debug, Clone)]
pub struct BrowserProgram {
    bindings: Vec<BrowserBinding>,
    statements: Vec<BrowserStatement>,
}

/// A binding that fetches a value from a browser API.
#[derive(Debug, Clone)]
pub struct BrowserBinding {
    pub name: String,
    pub source: BrowserSource,
}

/// Source of a browser binding value.
#[derive(Debug, Clone)]
pub enum BrowserSource {
    /// Get value from storage
    StorageGet { area: StorageArea, key: String },
    /// Get value from runtime message
    RuntimeMessage(MessagePayload),
}

/// A statement that performs a browser API action.
#[derive(Debug, Clone)]
pub enum BrowserStatement {
    /// Set a value in storage
    StorageSet {
        area: StorageArea,
        key: String,
        value: JsExpr,
    },
    /// Send a runtime message
    RuntimeSend(MessagePayload),
    /// Log to console
    ConsoleLog(Vec<JsExpr>),
}

/// Browser storage area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageArea {
    /// Local storage (persists until cleared)
    Local,
    /// Sync storage (syncs across devices)
    Sync,
}

/// A message payload for runtime messaging.
#[derive(Debug, Clone)]
pub struct MessagePayload {
    fields: BTreeMap<String, JsExpr>,
}

/// A JavaScript expression.
#[derive(Debug, Clone)]
pub enum JsExpr {
    /// A literal JSON value
    Literal(Value),
    /// A variable reference
    Var(String),
    /// A member access using a literal property name
    Member { object: String, property: String },
    /// Raw JavaScript code (use with caution)
    Raw(String),
}

impl BrowserProgram {
    /// Create a new empty browser program.
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            statements: Vec::new(),
        }
    }

    /// Bind a value from storage to a variable.
    pub fn bind_storage(
        mut self,
        name: impl Into<String>,
        area: StorageArea,
        key: impl Into<String>,
    ) -> Self {
        self.bindings.push(BrowserBinding {
            name: name.into(),
            source: BrowserSource::StorageGet {
                area,
                key: key.into(),
            },
        });
        self
    }

    /// Bind a value from a runtime message to a variable.
    pub fn bind_runtime_message(
        mut self,
        name: impl Into<String>,
        payload: MessagePayload,
    ) -> Self {
        self.bindings.push(BrowserBinding {
            name: name.into(),
            source: BrowserSource::RuntimeMessage(payload),
        });
        self
    }

    /// Set a value in storage.
    pub fn set_storage(
        mut self,
        area: StorageArea,
        key: impl Into<String>,
        value: impl Into<JsExpr>,
    ) -> Self {
        self.statements.push(BrowserStatement::StorageSet {
            area,
            key: key.into(),
            value: value.into(),
        });
        self
    }

    /// Send a runtime message.
    pub fn send_runtime(mut self, payload: MessagePayload) -> Self {
        self.statements.push(BrowserStatement::RuntimeSend(payload));
        self
    }

    /// Log values to console.
    pub fn console_log<I, T>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<JsExpr>,
    {
        self.statements.push(BrowserStatement::ConsoleLog(
            values.into_iter().map(Into::into).collect(),
        ));
        self
    }

    /// Emit the program as a JSON data structure for CSP-safe interpretation.
    ///
    /// Returns a JSON string describing bindings and statements that the
    /// framework's `content.js` interprets without any dynamic `import()`.
    /// Use this instead of `emit_module()` to avoid blob URL CSP violations
    /// on host pages with strict `script-src` policies.
    pub fn emit_data(&self) -> String {
        let bindings: Vec<Value> = self
            .bindings
            .iter()
            .map(|b| match &b.source {
                BrowserSource::StorageGet { area, key } => serde_json::json!({
                    "name": b.name,
                    "type": "storage_get",
                    "area": area.as_js_name(),
                    "key": key,
                }),
                BrowserSource::RuntimeMessage(payload) => serde_json::json!({
                    "name": b.name,
                    "type": "runtime_message",
                    "payload": payload.fields.iter()
                        .map(|(k, v)| (k.clone(), json_expr_to_value(v)))
                        .collect::<serde_json::Map<_, _>>(),
                }),
            })
            .collect();

        let statements: Vec<Value> = self
            .statements
            .iter()
            .map(|s| match s {
                BrowserStatement::StorageSet { area, key, value } => serde_json::json!({
                    "type": "storage_set",
                    "area": area.as_js_name(),
                    "key": key,
                    "value": json_expr_to_value(value),
                }),
                BrowserStatement::RuntimeSend(payload) => serde_json::json!({
                    "type": "runtime_send",
                    "payload": payload.fields.iter()
                        .map(|(k, v)| (k.clone(), json_expr_to_value(v)))
                        .collect::<serde_json::Map<_, _>>(),
                }),
                BrowserStatement::ConsoleLog(values) => serde_json::json!({
                    "type": "console_log",
                    "args": values.iter().map(json_expr_to_value).collect::<Vec<_>>(),
                }),
            })
            .collect();

        serde_json::json!({ "bindings": bindings, "statements": statements }).to_string()
    }

    /// Emit the program as a JavaScript ES module.
    ///
    /// Literal strings escape `<` so they cannot alter an enclosing HTML script element.
    /// [`JsExpr::raw`] remains caller-authored executable source and is not HTML-safe.
    pub fn emit_module(&self) -> String {
        let mut api_name = String::from("browserApi");
        while self
            .bindings
            .iter()
            .any(|binding| sanitize_ident(&binding.name) == api_name)
        {
            api_name.push('_');
        }
        let mut js = format!("export async function runBrowserProgram({api_name}) {{\n");

        for binding in &self.bindings {
            match &binding.source {
                BrowserSource::StorageGet { area, key } => {
                    js.push_str(&format!(
                        "  const {name} = (await {api_name}.storage.{area}.get({{[{quoted_key}]: undefined}}))[{quoted_key}];\n",
                        name = sanitize_ident(&binding.name),
                        area = area.as_js_name(),
                        quoted_key = js_string(key),
                    ));
                }
                BrowserSource::RuntimeMessage(payload) => {
                    js.push_str(&format!(
                        "  const {name} = await {api_name}.runtime.sendMessage({payload});\n",
                        name = sanitize_ident(&binding.name),
                        payload = payload.to_js(),
                    ));
                }
            }
        }

        for statement in &self.statements {
            match statement {
                BrowserStatement::StorageSet { area, key, value } => {
                    js.push_str(&format!(
                        "  await {api_name}.storage.{area}.set({{[{quoted_key}]: {value}}});\n",
                        area = area.as_js_name(),
                        quoted_key = js_string(key),
                        value = value.to_js(),
                    ));
                }
                BrowserStatement::RuntimeSend(payload) => {
                    js.push_str(&format!(
                        "  await {api_name}.runtime.sendMessage({payload});\n",
                        payload = payload.to_js(),
                    ));
                }
                BrowserStatement::ConsoleLog(values) => {
                    let joined = values
                        .iter()
                        .map(JsExpr::to_js)
                        .collect::<Vec<_>>()
                        .join(", ");
                    js.push_str(&format!("  console.log({joined});\n"));
                }
            }
        }

        js.push_str("}\n");
        js
    }
}

impl Default for BrowserProgram {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageArea {
    fn as_js_name(self) -> &'static str {
        match self {
            StorageArea::Local => "local",
            StorageArea::Sync => "sync",
        }
    }
}

impl MessagePayload {
    /// Create a new empty message payload.
    pub fn new() -> Self {
        Self {
            fields: BTreeMap::new(),
        }
    }

    /// Add a field to the message.
    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<JsExpr>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }

    fn to_js(&self) -> String {
        let fields = self
            .fields
            .iter()
            .map(|(key, value)| format!("[{}]: {}", js_string(key), value.to_js()))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{{ {fields} }}")
    }
}

impl Default for MessagePayload {
    fn default() -> Self {
        Self::new()
    }
}

impl JsExpr {
    /// Create a string literal.
    pub fn string(value: impl Into<String>) -> Self {
        Self::Literal(Value::String(value.into()))
    }

    /// Create a boolean literal.
    pub fn bool(value: bool) -> Self {
        Self::Literal(Value::Bool(value))
    }

    /// Create a variable reference.
    pub fn var(name: impl Into<String>) -> Self {
        Self::Var(name.into())
    }

    /// Create a member access expression.
    pub fn member(object: impl Into<String>, property: impl Into<String>) -> Self {
        Self::Member {
            object: object.into(),
            property: property.into(),
        }
    }

    /// Create a raw JavaScript expression.
    pub fn raw(code: impl Into<String>) -> Self {
        Self::Raw(code.into())
    }

    fn to_js(&self) -> String {
        match self {
            JsExpr::Literal(value) => literal_to_js(value),
            JsExpr::Var(name) => sanitize_ident(name),
            JsExpr::Member { object, property } => {
                format!("{}[{}]", sanitize_ident(object), js_string(property))
            }
            JsExpr::Raw(code) => code.clone(),
        }
    }
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value)
        .expect("JSON string serialization")
        .replace('<', "\\u003c")
}

fn literal_to_js(value: &Value) -> String {
    match value {
        Value::String(value) => js_string(value),
        Value::Object(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|(key, value)| format!("[{}]: {}", js_string(key), literal_to_js(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(literal_to_js)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => value.to_string(),
    }
}

impl From<Value> for JsExpr {
    fn from(value: Value) -> Self {
        Self::Literal(value)
    }
}

impl From<&str> for JsExpr {
    fn from(value: &str) -> Self {
        Self::Literal(Value::String(value.to_string()))
    }
}

impl From<String> for JsExpr {
    fn from(value: String) -> Self {
        Self::Literal(Value::String(value))
    }
}

impl From<bool> for JsExpr {
    fn from(value: bool) -> Self {
        Self::Literal(Value::Bool(value))
    }
}

impl From<i64> for JsExpr {
    fn from(value: i64) -> Self {
        Self::Raw(value.to_string())
    }
}

/// Serialize a JsExpr to a JSON value for the data-driven program format.
/// Variable references use `{ "$var": "name" }`; member references use
/// `{ "$member": ["object", "property"] }` to retain literal property names.
fn json_expr_to_value(expr: &JsExpr) -> Value {
    match expr {
        JsExpr::Literal(v) => v.clone(),
        JsExpr::Var(name) => serde_json::json!({ "$var": name }),
        JsExpr::Member { object, property } => {
            serde_json::json!({ "$member": [object, property] })
        }
        JsExpr::Raw(code) => serde_json::json!({ "$raw": code }),
    }
}

fn sanitize_ident(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_program() {
        let program = BrowserProgram::new()
            .bind_storage("notes", StorageArea::Local, "myNotes")
            .set_storage(StorageArea::Local, "booted", JsExpr::bool(true))
            .console_log([JsExpr::string("Hello"), JsExpr::var("notes")]);

        let js = program.emit_module();
        assert!(js.contains("browserApi.storage.local.get"));
        assert!(js.contains("browserApi.storage.local.set"));
        assert!(js.contains("console.log"));
    }

    #[test]
    fn test_message_payload() {
        let payload = MessagePayload::new()
            .with_field("action", "test")
            .with_field("value", JsExpr::var("myVar"));

        let js = payload.to_js();
        assert!(js.contains("\"action\""));
        assert!(js.contains("myVar"));
    }

    #[test]
    fn emitted_objects_preserve_special_json_keys() {
        let nested = serde_json::json!({"__proto__": {"marker": true}});
        let program = BrowserProgram::new()
            .bind_storage("stored", StorageArea::Local, "__proto__")
            .set_storage(StorageArea::Local, "__proto__", nested.clone())
            .send_runtime(MessagePayload::new().with_field("__proto__", JsExpr::var("stored")));
        let js = program.emit_module();
        assert!(js.contains("get({[\"__proto__\"]: undefined})"), "{js}");
        assert!(
            js.contains("set({[\"__proto__\"]: {[\"__proto__\"]:"),
            "{js}"
        );
        assert!(js.contains("{ [\"__proto__\"]: stored }"), "{js}");
        assert_eq!(
            JsExpr::from(serde_json::json!([nested])).to_js(),
            "[{[\"__proto__\"]: {[\"marker\"]: true}}]"
        );
    }

    #[test]
    fn member_expressions_preserve_object_and_property_separately() {
        let member = JsExpr::member("object", "a-b");
        assert_eq!(member.to_js(), "object[\"a-b\"]");
        assert_eq!(
            json_expr_to_value(&member),
            serde_json::json!({"$member": ["object", "a-b"]})
        );
        assert_eq!(
            json_expr_to_value(&JsExpr::var("object.a-b")),
            serde_json::json!({"$var": "object.a-b"})
        );
    }

    #[test]
    fn module_api_parameter_does_not_collide_with_bindings() {
        let js = BrowserProgram::new()
            .bind_storage("browserApi", StorageArea::Local, "first")
            .bind_runtime_message("browserApi_", MessagePayload::new())
            .set_storage(StorageArea::Local, "result", JsExpr::var("browserApi"))
            .send_runtime(MessagePayload::new().with_field("value", JsExpr::raw("browserApi_")))
            .emit_module();
        assert!(
            js.starts_with("export async function runBrowserProgram(browserApi__) {"),
            "{js}"
        );
        assert!(
            js.contains("const browserApi = (await browserApi__.storage.local.get"),
            "{js}"
        );
        assert!(
            js.contains("const browserApi_ = await browserApi__.runtime.sendMessage"),
            "{js}"
        );
        assert!(js.contains("await browserApi__.storage.local.set"), "{js}");
        assert!(
            js.contains("await browserApi__.runtime.sendMessage({ [\"value\"]: browserApi_ })"),
            "{js}"
        );
    }

    #[test]
    fn dollar_identifiers_remain_distinct_from_underscore_identifiers() {
        let js = BrowserProgram::new()
            .bind_storage("$value", StorageArea::Local, "dollar")
            .bind_storage("_value", StorageArea::Local, "underscore")
            .send_runtime(
                MessagePayload::new()
                    .with_field("dollar", JsExpr::var("$value"))
                    .with_field("underscore", JsExpr::var("_value"))
                    .with_field("raw", JsExpr::raw("$value")),
            )
            .emit_module();
        assert!(js.contains("const $value ="), "{js}");
        assert!(js.contains("const _value ="), "{js}");
        assert!(js.contains("[\"dollar\"]: $value"), "{js}");
        assert!(js.contains("[\"underscore\"]: _value"), "{js}");
        assert!(js.contains("[\"raw\"]: $value"), "{js}");
    }
    #[test]
    fn authored_literal_strings_do_not_close_inline_script_elements() {
        let dangerous = "</ScRiPt><script>not code</script>";
        let composite = serde_json::json!({(dangerous): [dangerous, {"nested": dangerous}]});
        let js = BrowserProgram::new()
            .bind_storage("stored", StorageArea::Local, dangerous)
            .set_storage(StorageArea::Local, dangerous, JsExpr::from(composite))
            .send_runtime(
                MessagePayload::new()
                    .with_field(dangerous, dangerous)
                    .with_field("member", JsExpr::member("stored", dangerous)),
            )
            .console_log([JsExpr::from(dangerous)])
            .emit_module();
        assert!(!js.contains('<'), "{js}");
        assert!(js.contains("\\u003c/ScRiPt>"), "{js}");
        assert!(js.contains("console.log(\"\\u003c/ScRiPt>"), "{js}");
    }

    #[test]
    fn raw_javascript_is_not_rewritten_for_html_embedding() {
        let js = BrowserProgram::new()
            .console_log([JsExpr::raw("a < b")])
            .emit_module();
        assert!(js.contains("console.log(a < b)"), "{js}");
    }
    #[test]
    fn literal_strings_escape_every_ascii_case_of_script_closers_and_comment_openers() {
        for mask in 0..64 {
            let tag: String = "script"
                .chars()
                .enumerate()
                .map(|(index, ch)| {
                    if mask & (1 << index) != 0 {
                        ch.to_ascii_uppercase()
                    } else {
                        ch
                    }
                })
                .collect();
            let value = format!("<!--<script></{tag}>");
            let emitted = JsExpr::from(value).to_js();
            assert!(!emitted.contains('<'), "{emitted}");
            assert!(emitted.contains("\\u003c!--"), "{emitted}");
        }
    }
}
