pub mod action;
pub mod bookmarks;
pub mod commands;
pub mod core;
pub mod generated;
pub mod history;
pub mod runtime;
pub mod scripting;
pub mod storage;
pub mod tabs;
pub mod windows;

pub use core::{
    await_promise, browser, call_callback_method, call_method, get_path, last_error, namespace,
    raw_namespace, BrowserError, EventListenerGuard, Result,
};

pub mod schema {
    pub use crate::wasm_schema::PUBLIC_MV3_NAMESPACES;
}
