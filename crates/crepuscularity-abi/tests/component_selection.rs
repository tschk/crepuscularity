use crepuscularity_abi::*;
use std::ffi::{CStr, CString};
use std::ptr;

struct Session(*mut CrepusSession);

impl Session {
    fn new() -> Self {
        Self(crepus_session_new())
    }

    fn select(&self, name: Option<&str>) {
        let name = name.map(|s| CString::new(s).unwrap());
        assert_eq!(
            crepus_session_set_component(self.0, name.as_ref().map_or(ptr::null(), |s| s.as_ptr())),
            0
        );
    }

    fn files(&self, entry: &str, files: serde_json::Value) {
        let bundle =
            CString::new(serde_json::json!({"entry":entry,"files":files}).to_string()).unwrap();
        assert_eq!(crepus_session_set_files_json(self.0, bundle.as_ptr()), 0);
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

fn take(value: *mut std::os::raw::c_char) -> String {
    assert!(!value.is_null(), "unexpected ABI error");
    let text = unsafe { CStr::from_ptr(value) }
        .to_str()
        .unwrap()
        .to_owned();
    crepus_string_free(value);
    text
}

fn only(text: &str, present: &str, absent: &str) {
    assert!(text.contains(present), "missing {present}: {text}");
    assert!(!text.contains(absent), "unexpected {absent}: {text}");
}

const COMPONENTS: &str = "--- First\nspan \"FIRST\"\n--- Second\nspan \"SECOND {count}\"";

#[test]
fn component_selection_matches_template_and_file_sessions_including_dispatch() {
    for bundled in [false, true] {
        let session = Session::new();
        if bundled {
            session.files(
                "components.crepus",
                serde_json::json!({"components.crepus": COMPONENTS}),
            );
        } else {
            let source = CString::new(COMPONENTS).unwrap();
            assert_eq!(
                crepus_session_set_template_string(session.0, source.as_ptr(), ptr::null()),
                0
            );
        }
        session.select(Some("Second"));
        let context = CString::new(r#"{"count":1}"#).unwrap();
        assert_eq!(
            crepus_session_set_context_json(session.0, context.as_ptr()),
            0
        );
        only(&session.render(), "SECOND 1", "FIRST");
        let event = CString::new(r#"{"handler":"bind:count:2"}"#).unwrap();
        only(
            &take(crepus_session_dispatch_event_json(
                session.0,
                event.as_ptr(),
            )),
            "SECOND 2",
            "FIRST",
        );
        session.select(Some("Missing"));
        assert!(crepus_session_render_ir_json(session.0).is_null());
        assert!(take(crepus_session_take_last_error(session.0)).contains("Missing"));
        session.select(Some("First"));
        only(&session.render(), "FIRST", "SECOND");
        session.select(None);
        let unselected = session.render();
        assert!(unselected.contains("FIRST") && unselected.contains("SECOND"));
    }
}

#[test]
fn explicit_selection_overrides_entry_fragment_and_clear_restores_it_with_virtual_includes() {
    let session = Session::new();
    session.files(
        "ui/components.crepus#First",
        serde_json::json!({
            "ui/components.crepus": "--- First\nspan \"FIRST\"\n--- Second\ninclude child.crepus",
            "ui/child.crepus": "span \"SECOND\""
        }),
    );
    only(&session.render(), "FIRST", "SECOND");
    session.select(Some("Second"));
    only(&session.render(), "SECOND", "FIRST");
    session.select(None);
    only(&session.render(), "FIRST", "SECOND");
    session.files(
        "plain.crepus",
        serde_json::json!({"plain.crepus": "span \"PLAIN\""}),
    );
    assert!(session.render().contains("PLAIN"));
}
