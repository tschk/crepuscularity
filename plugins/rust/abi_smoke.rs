mod abi {
    include!("abi_generated.rs");
}

use abi::*;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;

fn text(value: &str) -> CString {
    CString::new(value).unwrap()
}

unsafe fn take(pointer: *mut c_char) -> String {
    assert!(!pointer.is_null());
    let value = CStr::from_ptr(pointer).to_str().unwrap().to_owned();
    crepus_string_free(pointer);
    value
}

unsafe extern "C" fn capture(event: *const c_char, userdata: *mut c_void) {
    assert!(CStr::from_ptr(event)
        .to_str()
        .unwrap()
        .contains("bind:count:4"));
    *(userdata as *mut usize) += 1;
}

fn main() {
    unsafe {
        let session = crepus_session_new();
        assert!(!session.is_null());
        assert_eq!(
            crepus_session_set_template_string(
                session,
                text("input bind=count\nspan \"Count {count}\"").as_ptr(),
                ptr::null()
            ),
            0
        );
        assert_eq!(
            crepus_session_set_context_json(session, text(r#"{"count":"1"}"#).as_ptr()),
            0
        );
        assert_eq!(
            crepus_session_apply_context_patch_json(session, text(r#"{"count":"3"}"#).as_ptr()),
            0
        );
        assert!(take(crepus_session_render_ir_json(session)).contains("Count 3"));
        let mut calls = 0usize;
        assert_eq!(
            crepus_session_set_event_callback(
                session,
                Some(capture),
                &mut calls as *mut usize as *mut c_void
            ),
            0
        );
        assert!(take(crepus_session_dispatch_event_json(
            session,
            text(r#"{"handler":"bind:count:4"}"#).as_ptr()
        ))
        .contains("Count 4"));
        assert_eq!(calls, 1);
        assert_eq!(
            crepus_session_set_event_callback(session, None, ptr::null_mut()),
            0
        );
        take(crepus_session_dispatch_event_json(
            session,
            text(r#"{"handler":"bind:count:4"}"#).as_ptr(),
        ));
        assert_eq!(calls, 1);
        assert_eq!(
            crepus_session_set_context_json(session, text("{").as_ptr()),
            -1
        );
        assert!(!take(crepus_session_take_last_error(session)).is_empty());
        assert!(crepus_session_take_last_error(session).is_null());
        let files = r#"{"entry":"app.crepus","files":{"app.crepus":"span \"File works\""}}"#;
        assert_eq!(
            crepus_session_set_files_json(session, text(files).as_ptr()),
            0
        );
        assert!(take(crepus_session_render_ir_json(session)).contains("File works"));
        assert_eq!(
            crepus_session_set_template_string(
                session,
                text("--- First\nspan \"Wrong first\"\n--- Second\nspan \"Selected second\"")
                    .as_ptr(),
                ptr::null()
            ),
            0
        );
        assert_eq!(
            crepus_session_set_component(session, text("Second").as_ptr()),
            0
        );
        let selected = take(crepus_session_render_ir_json(session));
        assert!(selected.contains("Selected second") && !selected.contains("Wrong first"));
        assert_eq!(crepus_session_set_component(session, ptr::null()), 0);
        assert_eq!(
            crepus_session_set_template_string(
                session,
                text("span \"Template reset\"").as_ptr(),
                ptr::null()
            ),
            0
        );
        assert!(take(crepus_session_render_ir_json(session)).contains("Template reset"));
        crepus_session_free(session);
        assert_eq!(
            crepus_session_set_context_json(ptr::null_mut(), text("{}").as_ptr()),
            -1
        );
        assert!(!take(crepus_last_error()).is_empty());
        assert!(crepus_last_error().is_null());
    }
    println!("PASS generated Rust ABI: all 13 functions, state updates, owned strings, nullable callback/userdata, components and errors");
}
