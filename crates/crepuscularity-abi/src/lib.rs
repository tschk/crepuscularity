//! C ABI for View IR rendering sessions.
//!
//! # Thread safety
//!
//! [`CrepusSession`] is **not** `Sync`. Use one session per thread, or serialize
//! all calls to a shared session with an external mutex.
//!
//! # Context numbers
//!
//! Integer JSON tokens must fit signed 64-bit storage. Decimal and exponent
//! tokens use finite double precision. Invalid context updates are atomic.
//!
//! # Panics
//!
//! This crate is built with `panic = "abort"` so unwinding never crosses the C ABI.
//!
//! # Event callbacks
//!
//! Pointers passed to [`CrepusEventCallback`] are valid until the next
//! `crepus_session_dispatch_event` call on the same session (or until the session
//! is freed). Do not retain them past that point.
//! Callbacks must not call any session function on the session being dispatched,
//! including freeing it. Dispatch still holds an exclusive reference to it.
//! Such calls are rejected with an error in [`crepus_last_error`]; a rejected
//! free leaves the session alive.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::path::PathBuf;
use std::ptr;

use crepuscularity_core::context::{TemplateContext, TemplateValue};
use crepuscularity_native::{
    render_component_file_to_ir, render_from_files, render_template_to_ir, to_json_owned,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Maximum accepted JSON payload size (10 MB) for ABI inputs.
const MAX_JSON_SIZE: usize = 10 * 1024 * 1024;
/// Maximum number of files accepted in a `set_files_json` bundle.
const MAX_FILES: usize = 10_000;

// Safety: all `#[no_mangle] extern "C"` functions validate null pointers before
// dereferencing. The `session` pointer passed to every function except
// `crepus_session_new` MUST have been returned by `crepus_session_new` and MUST
// NOT have been freed via `crepus_session_free`. String pointers (`template_utf8`,
// `event_json_utf8`, etc.) MUST point to valid null-terminated UTF-8 C strings.
// `crepus_string_free` MUST only be called on pointers returned by this crate;
// after calling it the pointer is invalid and MUST NOT be used again.
// Event callback JSON pointers are valid only until the next dispatch on that session.

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
    static ACTIVE_SESSIONS: RefCell<HashSet<*mut CrepusSession>> = RefCell::new(HashSet::new());
}

struct SessionCall(*mut CrepusSession);

impl Drop for SessionCall {
    fn drop(&mut self) {
        ACTIVE_SESSIONS.with(|sessions| sessions.borrow_mut().remove(&self.0));
    }
}

fn session_is_active(session: *mut CrepusSession) -> bool {
    ACTIVE_SESSIONS.with(|sessions| sessions.borrow().contains(&session))
}

pub type CrepusEventCallback = extern "C" fn(event_json: *const c_char, userdata: *mut c_void);

#[derive(Default)]
pub struct CrepusSession {
    source: Option<Source>,
    component: Option<String>,
    context: TemplateContext,
    callback: Option<CrepusEventCallback>,
    userdata: *mut c_void,
    last_error: Option<String>,
    /// Keeps the most recent event callback payload alive until the next dispatch.
    last_event_payload: Option<CString>,
}

enum Source {
    Template {
        template: String,
        base_dir: Option<PathBuf>,
    },
    Files {
        entry: String,
        files: HashMap<String, String>,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilesEnvelope {
    entry: String,
    files: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventEnvelope {
    handler: Option<String>,
    event: Option<String>,
    payload: Option<Value>,
    context: Option<Value>,
}

#[no_mangle]
pub extern "C" fn crepus_session_new() -> *mut CrepusSession {
    Box::into_raw(Box::new(CrepusSession::default()))
}

#[no_mangle]
pub extern "C" fn crepus_session_free(session: *mut CrepusSession) {
    if session.is_null() {
        return;
    }
    if session_is_active(session) {
        set_error_for(
            session,
            "session cannot be freed during its event callback".to_string(),
        );
        return;
    }
    drop_session(session);
}

#[no_mangle]
pub extern "C" fn crepus_session_set_template_string(
    session: *mut CrepusSession,
    template_utf8: *const c_char,
    base_dir_utf8: *const c_char,
) -> i32 {
    with_session(session, |session| {
        let template = read_required(template_utf8, "template")?;
        let base_dir = read_optional(base_dir_utf8).map(PathBuf::from);
        session.source = Some(Source::Template { template, base_dir });
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn crepus_session_set_component(
    session: *mut CrepusSession,
    component_utf8: *const c_char,
) -> i32 {
    with_session(session, |session| {
        session.component = read_optional(component_utf8);
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn crepus_session_set_files_json(
    session: *mut CrepusSession,
    files_json_utf8: *const c_char,
) -> i32 {
    with_session(session, |session| {
        let raw = read_required_json(files_json_utf8, "files_json")?;
        let env: FilesEnvelope =
            serde_json::from_str(&raw).map_err(|e| format!("files JSON: {e}"))?;
        if env.files.len() > MAX_FILES {
            return Err(format!(
                "files JSON contains {} entries (max {MAX_FILES})",
                env.files.len()
            ));
        }
        session.source = Some(Source::Files {
            entry: env.entry,
            files: env.files,
        });
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn crepus_session_set_context_json(
    session: *mut CrepusSession,
    context_json_utf8: *const c_char,
) -> i32 {
    with_session(session, |session| {
        let raw = read_required_json(context_json_utf8, "context_json")?;
        validate_context_numbers(&raw, 0)?;
        let value: Value = serde_json::from_str(&raw).map_err(|e| format!("context JSON: {e}"))?;
        let mut ctx = TemplateContext::new();
        merge_json_ctx(&value, &mut ctx)?;
        session.context.vars = ctx.vars;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn crepus_session_apply_context_patch_json(
    session: *mut CrepusSession,
    context_json_utf8: *const c_char,
) -> i32 {
    with_session(session, |session| {
        let raw = read_required_json(context_json_utf8, "context_json")?;
        validate_context_numbers(&raw, 0)?;
        let value: Value = serde_json::from_str(&raw).map_err(|e| format!("context JSON: {e}"))?;
        merge_json_ctx(&value, &mut session.context)?;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn crepus_session_set_event_callback(
    session: *mut CrepusSession,
    callback: Option<CrepusEventCallback>,
    userdata: *mut c_void,
) -> i32 {
    with_session(session, |session| {
        session.callback = callback;
        session.userdata = userdata;
        Ok(())
    })
}

#[no_mangle]
pub extern "C" fn crepus_session_render_ir_json(session: *mut CrepusSession) -> *mut c_char {
    match with_session_result(session, render_session) {
        Ok(out) => into_c_string(out),
        Err(e) => {
            set_error_for(session, e);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn crepus_session_dispatch_event_json(
    session: *mut CrepusSession,
    event_json_utf8: *const c_char,
) -> *mut c_char {
    match with_session_result(session, |session| dispatch_event(session, event_json_utf8)) {
        Ok(out) => into_c_string(out),
        Err(e) => {
            set_error_for(session, e);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn crepus_session_take_last_error(session: *mut CrepusSession) -> *mut c_char {
    if session.is_null() {
        return crepus_last_error();
    }
    match with_session_result(session, |session| Ok(session.last_error.take())) {
        Ok(Some(error)) => into_c_string(error),
        Ok(None) => ptr::null_mut(),
        Err(e) => {
            set_error_for(session, e);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn crepus_last_error() -> *mut c_char {
    LAST_ERROR
        .with(|slot| slot.borrow_mut().take())
        .map(into_c_string)
        .unwrap_or(ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn crepus_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    drop_c_string(ptr);
}

fn drop_session(session: *mut CrepusSession) {
    unsafe {
        drop(Box::from_raw(session));
    }
}

fn drop_c_string(ptr: *mut c_char) {
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

fn dispatch_event(
    session: &mut CrepusSession,
    event_json_utf8: *const c_char,
) -> Result<String, String> {
    let raw = read_required_json(event_json_utf8, "event_json")?;
    let event = parse_event(&raw)?;

    if let Some(context) = &event.context {
        merge_json_ctx(context, &mut session.context)?;
    }
    let handler = event
        .handler
        .or(event.event)
        .unwrap_or_else(|| "event".to_string());
    if let Some((key, value)) = bind_update(&handler) {
        session.context.set(key, TemplateValue::Str(value));
    }
    let payload = json!({
        "kind": "event",
        "handler": handler,
        "payload": event.payload.unwrap_or(Value::Null),
    });
    let payload_json =
        serde_json::to_string(&payload).map_err(|e| format!("serialize event: {e}"))?;

    if let Some(callback) = session.callback {
        let c_payload = CString::new(payload_json.clone())
            .map_err(|_| "event payload contains interior NUL".to_string())?;
        let ptr = c_payload.as_ptr();
        session.last_event_payload = Some(c_payload);
        callback(ptr, session.userdata);
    }

    let ir_json = render_session(session)?;
    let out = json!({
        "kind": "event",
        "handler": handler,
        "ir": serde_json::from_str::<Value>(&ir_json).map_err(|e| format!("parse rendered IR: {e}"))?,
    });
    serde_json::to_string(&out).map_err(|e| format!("serialize event result: {e}"))
}

fn parse_event(raw: &str) -> Result<EventEnvelope, String> {
    if let Ok(fields) = serde_json::from_str::<HashMap<String, &serde_json::value::RawValue>>(raw) {
        if let Some(context) = fields.get("context") {
            validate_context_numbers(context.get(), 0)?;
        }
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(value) => match value {
            Value::String(handler) => Ok(EventEnvelope {
                handler: Some(handler),
                event: None,
                payload: None,
                context: None,
            }),
            Value::Object(_) => {
                serde_json::from_value(value).map_err(|e| format!("event JSON: {e}"))
            }
            _ => Err("event JSON must be a string or object".to_string()),
        },
        Err(error) if serde_json::from_str::<&serde_json::value::RawValue>(raw).is_ok() => {
            Err(format!("event JSON: {error}"))
        }
        Err(_) => Ok(EventEnvelope {
            handler: Some(raw.to_string()),
            event: None,
            payload: None,
            context: None,
        }),
    }
}

fn bind_update(handler: &str) -> Option<(String, String)> {
    let rest = handler.strip_prefix("bind:")?;
    let (key, value) = rest.split_once(':')?;
    Some((key.to_string(), value.to_string()))
}

fn render_session(session: &mut CrepusSession) -> Result<String, String> {
    let source = session
        .source
        .as_ref()
        .ok_or_else(|| "session source is not set".to_string())?;
    let ir = match source {
        Source::Template { template, base_dir } => {
            let mut ctx = session.context.clone();
            ctx.base_dir = base_dir.clone();
            if let Some(component) = &session.component {
                render_component_file_to_ir(template, component, &ctx).map_err(|e| e.to_string())?
            } else {
                render_template_to_ir(template, &ctx).map_err(|e| e.to_string())?
            }
        }
        Source::Files { entry, files } => {
            let selected_entry = session.component.as_ref().map(|component| {
                let path = entry
                    .split_once('#')
                    .map_or(entry.as_str(), |(path, _)| path);
                format!("{path}#{component}")
            });
            render_from_files(
                files,
                selected_entry.as_deref().unwrap_or(entry),
                &session.context,
            )
            .map_err(|e| e.to_string())?
        }
    };
    to_json_owned(ir).map_err(|e| format!("serialize IR: {e}"))
}

fn with_session<F>(session: *mut CrepusSession, f: F) -> i32
where
    F: FnOnce(&mut CrepusSession) -> Result<(), String>,
{
    match with_session_result(session, f) {
        Ok(()) => 0,
        Err(e) => {
            set_error_for(session, e);
            -1
        }
    }
}

fn with_session_result<F, T>(session: *mut CrepusSession, f: F) -> Result<T, String>
where
    F: FnOnce(&mut CrepusSession) -> Result<T, String>,
{
    if session.is_null() {
        return Err("session pointer is null".to_string());
    }
    if !ACTIVE_SESSIONS.with(|sessions| sessions.borrow_mut().insert(session)) {
        return Err("session cannot be re-entered during its event callback".to_string());
    }
    let _call = SessionCall(session);
    let session = unsafe { &mut *session };
    f(session)
}

fn read_required(ptr: *const c_char, label: &str) -> Result<String, String> {
    read_optional(ptr).ok_or_else(|| format!("{label} pointer is null"))
}

/// Read a required JSON string and enforce the maximum payload size.
fn read_required_json(ptr: *const c_char, label: &str) -> Result<String, String> {
    let raw = read_required(ptr, label)?;
    if raw.len() > MAX_JSON_SIZE {
        return Err(format!(
            "{label} exceeds {MAX_JSON_SIZE} byte limit (got {})",
            raw.len()
        ));
    }
    Ok(raw)
}

fn read_optional(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn validate_context_numbers(raw: &str, depth: usize) -> Result<(), String> {
    if depth > 128 {
        return Err("context JSON nesting exceeds 128 levels".to_string());
    }
    let raw = raw.trim();
    match raw.as_bytes().first() {
        Some(b'{') => {
            let fields: HashMap<String, &serde_json::value::RawValue> =
                serde_json::from_str(raw).map_err(|error| format!("context JSON: {error}"))?;
            for value in fields.values() {
                validate_context_numbers(value.get(), depth + 1)?;
            }
        }
        Some(b'[') => {
            let items: Vec<&serde_json::value::RawValue> =
                serde_json::from_str(raw).map_err(|error| format!("context JSON: {error}"))?;
            for value in items {
                validate_context_numbers(value.get(), depth + 1)?;
            }
        }
        Some(b'-' | b'0'..=b'9')
            if !raw.contains(['.', 'e', 'E']) && raw.parse::<i64>().is_err() =>
        {
            return Err(format!(
                "integer exceeds signed 64-bit context range: {raw}"
            ));
        }
        _ => {}
    }
    Ok(())
}

fn merge_json_ctx(value: &Value, ctx: &mut TemplateContext) -> Result<(), String> {
    let Some(obj) = value.as_object() else {
        return Err("context must be a JSON object".to_string());
    };
    let updates = obj
        .iter()
        .map(|(key, value)| json_to_template_value(value).map(|value| (key.clone(), value)))
        .collect::<Result<Vec<_>, _>>()?;
    ctx.vars.extend(updates);
    Ok(())
}

fn json_to_template_value(value: &Value) -> Result<TemplateValue, String> {
    match value {
        Value::Null => Ok(TemplateValue::Null),
        Value::Bool(v) => Ok(TemplateValue::Bool(*v)),
        Value::Number(v) => {
            if let Some(n) = v.as_i64() {
                Ok(TemplateValue::Int(n))
            } else if v.is_u64() {
                Err(format!("integer exceeds signed 64-bit context range: {v}"))
            } else if let Some(n) = v.as_f64() {
                Ok(TemplateValue::Float(n))
            } else {
                Err(format!("unsupported number: {v}"))
            }
        }
        Value::String(v) => Ok(TemplateValue::Str(v.clone())),
        Value::Array(values) => {
            let mut items = Vec::new();
            for item in values {
                let Some(obj) = item.as_object() else {
                    return Err("context arrays must contain objects".to_string());
                };
                let mut child = TemplateContext::new();
                for (key, value) in obj {
                    child.set(key.clone(), json_to_template_scalar(value)?);
                }
                items.push(child);
            }
            Ok(TemplateValue::List(items))
        }
        Value::Object(_) => {
            Err("context object values are only supported inside arrays".to_string())
        }
    }
}

fn json_to_template_scalar(value: &Value) -> Result<TemplateValue, String> {
    match value {
        Value::Array(_) | Value::Object(_) => {
            Err("loop item fields must be scalar JSON values".to_string())
        }
        _ => json_to_template_value(value),
    }
}

fn set_error_for(session: *mut CrepusSession, error: String) {
    if session.is_null() || session_is_active(session) {
        LAST_ERROR.with(|slot| *slot.borrow_mut() = Some(error));
    } else {
        unsafe { &mut *session }.last_error = Some(error);
    }
}

fn into_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(value) => value.into_raw(),
        Err(e) => {
            LAST_ERROR.with(|slot| {
                *slot.borrow_mut() = Some(format!("string contains interior NUL: {e}"))
            });
            ptr::null_mut()
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn oversized_context_integers_are_rejected_without_changing_state() {
        let session = super::crepus_session_new();
        let initial = CString::new(r#"{"count":1}"#).unwrap();
        assert_eq!(
            super::crepus_session_set_context_json(session, initial.as_ptr()),
            0
        );
        for integer in [
            "18446744073709551615",
            "18446744073709551616",
            "-9223372036854775809",
            "99999999999999999999999999999999999999999999999999",
        ] {
            let invalid = CString::new(format!(r#"{{"count":9,"oversized":{integer}}}"#)).unwrap();
            for update in [
                super::crepus_session_set_context_json,
                super::crepus_session_apply_context_patch_json,
            ] {
                assert_eq!(update(session, invalid.as_ptr()), -1);
                assert!(take_string(super::crepus_session_take_last_error(session))
                    .contains("signed 64-bit"));
                super::with_session_result(session, |session| {
                    assert!(matches!(
                        session.context.vars.get("count"),
                        Some(super::TemplateValue::Int(1))
                    ));
                    assert!(!session.context.vars.contains_key("oversized"));
                    Ok(())
                })
                .unwrap();
            }
            let event = CString::new(format!(
                r#"{{"context":{{"count":9,"oversized":{integer}}}}}"#
            ))
            .unwrap();
            assert!(super::crepus_session_dispatch_event_json(session, event.as_ptr()).is_null());
            assert!(take_string(super::crepus_session_take_last_error(session))
                .contains("signed 64-bit"));
        }
        super::crepus_session_free(session);
        assert!(
            super::validate_context_numbers(r#"{"rows":[{"id":18446744073709551616}]}"#, 0)
                .is_err()
        );
        assert!(super::validate_context_numbers(
            r#"{"text":"18446744073709551616","decimal":1e20,"rows":[{"count":2}]}"#,
            0
        )
        .is_ok());
        for integer in [i64::MIN, i64::MAX] {
            assert!(
                matches!(super::json_to_template_value(&serde_json::json!(integer)), Ok(super::TemplateValue::Int(n)) if n == integer)
            );
        }
        assert!(matches!(
            super::json_to_template_value(&serde_json::json!(1e20)),
            Ok(super::TemplateValue::Float(_))
        ));
    }

    #[test]
    fn context_decoding_preserves_float_boundary_bits() {
        let session = super::crepus_session_new();
        let context = CString::new(r#"{"decimal":-9.223372036854775e18}"#).unwrap();
        assert_eq!(
            super::crepus_session_set_context_json(session, context.as_ptr()),
            0
        );
        let bits = super::with_session_result(session, |session| {
            match session.context.vars.get("decimal") {
                Some(super::TemplateValue::Float(value)) => Ok(value.to_bits()),
                other => Err(format!("expected float, got {other:?}")),
            }
        })
        .unwrap();
        super::crepus_session_free(session);
        assert_eq!(bits, (-9.223372036854775e18_f64).to_bits());
    }

    #[test]
    fn abi_serializes_deep_ir_on_a_default_test_thread() {
        let source = CString::new(format!(
            "{}leaf{}",
            "<div>".repeat(250),
            "</div>".repeat(250)
        ))
        .unwrap();
        let session = super::crepus_session_new();
        assert_eq!(
            super::crepus_session_set_template_string(session, source.as_ptr(), std::ptr::null()),
            0
        );
        let json = take_string(super::crepus_session_render_ir_json(session));
        super::crepus_session_free(session);
        assert_eq!(json.matches("\"kind\":\"stack\"").count(), 250);
        assert!(json.contains("\"content\":\"leaf\""));
    }

    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_void};
    use std::sync::Mutex;

    static EVENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

    extern "C" fn capture(event_json: *const c_char, _userdata: *mut c_void) {
        let event = unsafe { CStr::from_ptr(event_json) }
            .to_string_lossy()
            .into_owned();
        EVENTS.lock().unwrap().push(event);
    }

    fn take_string(ptr: *mut c_char) -> String {
        assert!(!ptr.is_null());
        let value = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        super::crepus_string_free(ptr);
        value
    }

    #[test]
    fn active_sessions_reject_reentry_and_remain_usable() {
        let session = super::crepus_session_new();
        let template = CString::new("span\n  \"Original {count}\"").unwrap();
        assert_eq!(
            super::crepus_session_set_template_string(session, template.as_ptr(), std::ptr::null()),
            0
        );
        let initial = CString::new(r#"{"count":1}"#).unwrap();
        assert_eq!(
            super::crepus_session_set_context_json(session, initial.as_ptr()),
            0
        );
        super::with_session_result(session, |_active| {
            let context = CString::new(r#"{"count":9}"#).unwrap();
            assert_eq!(
                super::crepus_session_set_context_json(session, context.as_ptr()),
                -1
            );
            assert!(take_string(super::crepus_last_error()).contains("re-entered"));
            assert!(super::crepus_session_render_ir_json(session).is_null());
            assert!(take_string(super::crepus_last_error()).contains("re-entered"));
            super::crepus_session_free(session);
            assert!(take_string(super::crepus_last_error()).contains("cannot be freed"));
            Ok(())
        })
        .unwrap();
        assert!(super::crepus_last_error().is_null());
        assert!(super::crepus_session_take_last_error(session).is_null());
        assert!(take_string(super::crepus_session_render_ir_json(session)).contains("Original 1"));
        super::crepus_session_free(session);
    }

    #[test]
    fn renders_ir_json_from_session() {
        let session = super::crepus_session_new();
        let template = CString::new("button @click=\"increment\"\n  \"Tap {count}\"").unwrap();
        let context = CString::new(r#"{"count":1}"#).unwrap();
        assert_eq!(
            super::crepus_session_set_template_string(session, template.as_ptr(), std::ptr::null()),
            0
        );
        assert_eq!(
            super::crepus_session_set_context_json(session, context.as_ptr()),
            0
        );
        let out = take_string(super::crepus_session_render_ir_json(session));
        assert!(out.contains(r#""onClick":"increment""#));
        assert!(out.contains("Tap 1"));
        super::crepus_session_free(session);
    }

    #[test]
    fn dispatches_event_callback_and_rerenders() {
        EVENTS.lock().unwrap().clear();
        let session = super::crepus_session_new();
        let template = CString::new("input bind=count\nspan\n  \"Count {count}\"").unwrap();
        let context = CString::new(r#"{"count":"1"}"#).unwrap();
        let event = CString::new(r#"{"handler":"bind:count:2"}"#).unwrap();
        assert_eq!(
            super::crepus_session_set_template_string(session, template.as_ptr(), std::ptr::null()),
            0
        );
        assert_eq!(
            super::crepus_session_set_context_json(session, context.as_ptr()),
            0
        );
        assert_eq!(
            super::crepus_session_set_event_callback(session, Some(capture), std::ptr::null_mut()),
            0
        );
        let out = take_string(super::crepus_session_dispatch_event_json(
            session,
            event.as_ptr(),
        ));
        assert!(out.contains(r#""handler":"bind:count:2""#));
        assert!(out.contains("Count 2"));
        assert_eq!(EVENTS.lock().unwrap().len(), 1);
        super::crepus_session_free(session);
    }

    #[test]
    fn binding_event_spellings_share_callback_and_render_semantics() {
        extern "C" fn record(json: *const c_char, userdata: *mut c_void) {
            let events = unsafe { &mut *userdata.cast::<Vec<String>>() };
            events.push(
                unsafe { CStr::from_ptr(json) }
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        for source in [
            "bind:count:2",
            r#""bind:count:2""#,
            r#"{"handler":"bind:count:2"}"#,
            r#"{"event":"bind:count:2"}"#,
            r#"{"handler":"bind:count:2","event":"bind:count:9"}"#,
        ] {
            let session = super::crepus_session_new();
            let template = CString::new("span\n  \"Count {count}\"").unwrap();
            let context = CString::new(r#"{"count":"1"}"#).unwrap();
            let mut events = Vec::<String>::new();
            assert_eq!(
                super::crepus_session_set_template_string(
                    session,
                    template.as_ptr(),
                    std::ptr::null()
                ),
                0
            );
            assert_eq!(
                super::crepus_session_set_context_json(session, context.as_ptr()),
                0
            );
            assert_eq!(
                super::crepus_session_set_event_callback(
                    session,
                    Some(record),
                    std::ptr::from_mut(&mut events).cast()
                ),
                0
            );
            let event = CString::new(source).unwrap();
            let output = take_string(super::crepus_session_dispatch_event_json(
                session,
                event.as_ptr(),
            ));
            super::crepus_session_free(session);
            assert!(output.contains("Count 2"), "{source}: {output}");
            let output: serde_json::Value = serde_json::from_str(&output).unwrap();
            assert_eq!(output["handler"], "bind:count:2", "{source}");
            assert_eq!(events.len(), 1, "{source}");
            let callback: serde_json::Value = serde_json::from_str(&events[0]).unwrap();
            assert_eq!(callback["handler"], "bind:count:2", "{source}");
        }
    }

    #[test]
    fn unrepresentable_event_numbers_fail_before_callbacks_or_state_changes() {
        extern "C" fn count_event(_: *const c_char, userdata: *mut c_void) {
            unsafe {
                *userdata.cast::<usize>() += 1;
            }
        }
        let session = super::crepus_session_new();
        let template = CString::new("span \"Count {count}\"").unwrap();
        let initial = CString::new(r#"{"count":1}"#).unwrap();
        assert_eq!(
            super::crepus_session_set_template_string(session, template.as_ptr(), std::ptr::null()),
            0
        );
        assert_eq!(
            super::crepus_session_set_context_json(session, initial.as_ptr()),
            0
        );
        let mut callbacks = 0usize;
        assert_eq!(
            super::crepus_session_set_event_callback(
                session,
                Some(count_event),
                std::ptr::from_mut(&mut callbacks).cast()
            ),
            0
        );
        let before = take_string(super::crepus_session_render_ir_json(session));
        for source in [
            r#"{"handler":"bind:count:2","context":{"count":9,"bad":1e400}}"#,
            r#"{"handler":"bind:count:2","context":{"count":9,"bad":-1e400}}"#,
            r#"{"handler":"bind:count:2","context":{"count":9},"payload":1e400}"#,
            r#"{"handler":"bind:count:2","context":{"count":9},"payload":{"bad":-1e400}}"#,
        ] {
            let event = CString::new(source).unwrap();
            assert!(
                super::crepus_session_dispatch_event_json(session, event.as_ptr()).is_null(),
                "{source}"
            );
            let error = take_string(super::crepus_session_take_last_error(session));
            assert!(error.contains("event JSON:"), "{error}");
            assert_eq!(callbacks, 0, "{source}");
            assert_eq!(
                take_string(super::crepus_session_render_ir_json(session)),
                before,
                "{source}"
            );
        }
        for (index, source) in ["save", r#""save""#, r#"{"legacy":"#]
            .into_iter()
            .enumerate()
        {
            let event = CString::new(source).unwrap();
            let output: serde_json::Value = serde_json::from_str(&take_string(
                super::crepus_session_dispatch_event_json(session, event.as_ptr()),
            ))
            .unwrap();
            let expected = if source == r#""save""# {
                "save"
            } else {
                source
            };
            assert_eq!(output["handler"], expected);
            assert_eq!(callbacks, index + 1);
        }
        super::crepus_session_free(session);
    }

    #[test]
    fn invalid_context_patches_leave_session_state_unchanged() {
        let session = super::crepus_session_new();
        let template = CString::new("span\n  \"Count {count}\"").unwrap();
        let context = CString::new(r#"{"count":1}"#).unwrap();
        assert_eq!(
            super::crepus_session_set_template_string(session, template.as_ptr(), std::ptr::null()),
            0
        );
        assert_eq!(
            super::crepus_session_set_context_json(session, context.as_ptr()),
            0
        );
        let before = take_string(super::crepus_session_render_ir_json(session));

        let patch = CString::new(r#"{"count":9,"zbad":[1]}"#).unwrap();
        assert_eq!(
            super::crepus_session_apply_context_patch_json(session, patch.as_ptr()),
            -1
        );
        assert!(take_string(super::crepus_session_take_last_error(session))
            .contains("arrays must contain objects"));
        assert_eq!(
            take_string(super::crepus_session_render_ir_json(session)),
            before
        );

        let event = CString::new(r#"{"context":{"count":9,"zbad":[1]}}"#).unwrap();
        assert!(super::crepus_session_dispatch_event_json(session, event.as_ptr()).is_null());
        assert_eq!(
            take_string(super::crepus_session_render_ir_json(session)),
            before
        );
        super::crepus_session_free(session);
    }

    #[test]
    fn null_sessions_report_consumable_errors() {
        let null = std::ptr::null_mut();
        assert_eq!(
            super::crepus_session_set_context_json(null, std::ptr::null()),
            -1
        );
        assert_eq!(
            take_string(super::crepus_session_take_last_error(null)),
            "session pointer is null"
        );
        assert!(super::crepus_last_error().is_null());
        assert!(super::crepus_session_render_ir_json(null).is_null());
        assert_eq!(
            take_string(super::crepus_last_error()),
            "session pointer is null"
        );
        assert!(super::crepus_session_dispatch_event_json(null, std::ptr::null()).is_null());
        assert_eq!(
            take_string(super::crepus_last_error()),
            "session pointer is null"
        );
        super::crepus_session_free(null);
        super::crepus_string_free(std::ptr::null_mut());
    }

    #[test]
    fn reports_errors_without_panics() {
        let session = super::crepus_session_new();
        assert!(super::crepus_session_render_ir_json(session).is_null());
        let error = take_string(super::crepus_session_take_last_error(session));
        assert!(error.contains("session source is not set"));
        super::crepus_session_free(session);
    }
}
