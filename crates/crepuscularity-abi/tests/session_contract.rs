use crepuscularity_abi::*;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;

struct Session(*mut CrepusSession);

impl Session {
    fn new() -> Self {
        let session = Self(crepus_session_new());
        let source = CString::new("span \"Count {count}\"").unwrap();
        assert_eq!(
            crepus_session_set_template_string(session.0, source.as_ptr(), ptr::null()),
            0
        );
        let initial = CString::new(r#"{"count":1}"#).unwrap();
        assert_eq!(
            crepus_session_set_context_json(session.0, initial.as_ptr()),
            0
        );
        session
    }

    fn render(&self) -> String {
        take(crepus_session_render_ir_json(self.0))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        crepus_session_free(self.0);
    }
}

fn take(value: *mut c_char) -> String {
    assert!(!value.is_null());
    let text = unsafe { CStr::from_ptr(value) }
        .to_str()
        .unwrap()
        .to_owned();
    crepus_string_free(value);
    text
}

#[test]
fn invalid_updates_are_atomic_and_preserve_numeric_token_contracts() {
    let session = Session::new();
    let before = session.render();
    for invalid in [
        r#"{"count":9,"zbad":[1]}"#,
        r#"{"count":9,"zbad":18446744073709551615}"#,
        r#"{"count":9,"zbad":18446744073709551616}"#,
        r#"{"count":9,"zbad":-9223372036854775809}"#,
        r#"{"count":9,"zbad":1e400}"#,
    ] {
        let raw = CString::new(invalid).unwrap();
        for update in [
            crepus_session_set_context_json,
            crepus_session_apply_context_patch_json,
        ] {
            assert_eq!(update(session.0, raw.as_ptr()), -1, "{invalid}");
            assert!(!take(crepus_session_take_last_error(session.0)).is_empty());
            assert_eq!(session.render(), before, "{invalid}");
        }
        let event = CString::new(format!(
            r#"{{"handler":"bind:count:2","context":{invalid}}}"#
        ))
        .unwrap();
        assert!(crepus_session_dispatch_event_json(session.0, event.as_ptr()).is_null());
        assert_eq!(session.render(), before, "{invalid}");
    }
    for value in [
        "-9223372036854775808",
        "9223372036854775807",
        "1e20",
        "1.25",
    ] {
        let raw = CString::new(format!(r#"{{"count":{value}}}"#)).unwrap();
        assert_eq!(
            crepus_session_set_context_json(session.0, raw.as_ptr()),
            0,
            "{value}"
        );
        assert!(!session.render().is_empty());
    }
}

#[test]
fn event_aliases_update_bindings_and_keep_handler_precedence() {
    for event in [
        "bind:count:2",
        r#""bind:count:2""#,
        r#"{"handler":"bind:count:2"}"#,
        r#"{"event":"bind:count:2"}"#,
        r#"{"handler":"bind:count:2","event":"bind:count:9"}"#,
    ] {
        let session = Session::new();
        let event = CString::new(event).unwrap();
        let result = take(crepus_session_dispatch_event_json(
            session.0,
            event.as_ptr(),
        ));
        let result: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(result["handler"], "bind:count:2");
        assert!(result["ir"].to_string().contains("Count 2"));
    }
}

#[test]
fn component_override_and_clear_preserve_virtual_entry_selection() {
    let session = Session::new();
    let files = CString::new(serde_json::json!({
        "entry": "ui/components.crepus#First",
        "files": {
            "ui/components.crepus": "--- First\nspan \"FIRST\"\n--- Second\ninclude child.crepus",
            "ui/child.crepus": "span \"SECOND\""
        }
    }).to_string()).unwrap();
    assert_eq!(crepus_session_set_files_json(session.0, files.as_ptr()), 0);
    for (selection, present, absent) in [
        (Some("Second"), "SECOND", "FIRST"),
        (None, "FIRST", "SECOND"),
    ] {
        let selection = selection.map(|name| CString::new(name).unwrap());
        assert_eq!(
            crepus_session_set_component(
                session.0,
                selection.as_ref().map_or(ptr::null(), |name| name.as_ptr())
            ),
            0
        );
        let rendered = session.render();
        assert!(rendered.contains(present), "{rendered}");
        assert!(!rendered.contains(absent), "{rendered}");
    }
}

extern "C" fn reenter(_: *const c_char, userdata: *mut c_void) {
    let session = userdata.cast::<CrepusSession>();
    assert!(crepus_session_render_ir_json(session).is_null());
    assert!(take(crepus_last_error()).contains("re-entered"));
    crepus_session_free(session);
    assert!(take(crepus_last_error()).contains("cannot be freed"));
    let other = Session::new();
    assert!(other.render().contains("Count 1"));
}

#[test]
fn callbacks_reject_same_session_reentry_without_invalidating_either_session() {
    let session = Session::new();
    assert_eq!(
        crepus_session_set_event_callback(session.0, Some(reenter), session.0.cast()),
        0
    );
    let event = CString::new("bind:count:2").unwrap();
    assert!(take(crepus_session_dispatch_event_json(
        session.0,
        event.as_ptr()
    ))
    .contains("Count 2"));
    assert!(session.render().contains("Count 2"));
}

#[test]
fn deep_ir_uses_the_owned_serializer() {
    let session = Session::new();
    let source = CString::new(format!(
        "{}leaf{}",
        "<div>".repeat(250),
        "</div>".repeat(250)
    ))
    .unwrap();
    assert_eq!(
        crepus_session_set_template_string(session.0, source.as_ptr(), ptr::null()),
        0
    );
    let result = session.render();
    assert_eq!(result.matches("\"kind\":\"stack\"").count(), 250);
    assert!(result.contains("\"content\":\"leaf\""));
}
