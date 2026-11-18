//! Embedded JavaScript via the official `v8` crate (the Rust bindings project is often called rusty_v8).
//! See `docs/THREADING.md` for thread constraints.
//!
//! The active [`Bridge`] is stored in a V8 context embedder slot (`CrepusBridgeSlot`).

use std::ops::Deref;
use std::pin::pin;
use std::rc::Rc;
use std::sync::{Arc, Once};

use serde_json::{json, Value};
use v8::String as V8String;
use v8::{self, Local, Script};

use crate::bridge::{Bridge, BridgeError};

static V8_INIT: Once = Once::new();

/// Bridge handle stored on the V8 context (one per [`V8Host`] / isolate).
#[derive(Clone)]
struct CrepusBridgeSlot(Arc<Bridge>);

fn v8_thread_pool_size() -> u32 {
    std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4)
        .max(1)
}

fn ensure_v8_platform() {
    V8_INIT.call_once(|| {
        // Explicit worker count: V8 uses 0 as “pick default from CPU count”; we pin to
        // available_parallelism for predictable multi-core background work (GC, parsing, etc.).
        let platform = v8::new_default_platform(v8_thread_pool_size(), false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });
}

pub struct V8Host {
    isolate: v8::OwnedIsolate,
    context: v8::Global<v8::Context>,
}

impl V8Host {
    pub fn new(bridge: Arc<Bridge>) -> Result<Self, String> {
        ensure_v8_platform();
        let mut isolate = v8::Isolate::new(Default::default());

        let context = {
            let scope = pin!(v8::HandleScope::new(&mut isolate));
            let scope = &mut scope.init();

            let local_ctx = v8::Context::new(scope, Default::default());
            let _ = local_ctx.set_slot(Rc::new(CrepusBridgeSlot(bridge.clone())));

            let scope = &mut v8::ContextScope::new(scope, local_ctx);

            let global = scope.get_current_context().global(scope);
            let crepus = v8::Object::new(scope);

            let version_key = V8String::new(scope, "version")
                .ok_or_else(|| "failed to create version key".to_string())?;
            let version_val = V8String::new(scope, env!("CARGO_PKG_VERSION"))
                .ok_or_else(|| "failed to create version value".to_string())?;
            let _ = crepus.set(scope, version_key.into(), version_val.into());

            let echo_key = V8String::new(scope, "nativeEcho")
                .ok_or_else(|| "failed to create echo key".to_string())?;
            let echo_fn = v8::Function::new(
                scope,
                |scope: &mut v8::PinScope<'_, '_>,
                 args: v8::FunctionCallbackArguments<'_>,
                 mut rv: v8::ReturnValue<'_, v8::Value>| {
                    let a0 = args.get(0);
                    let iso: &v8::Isolate = scope.deref();
                    let msg = if a0.is_string() {
                        a0.to_string(scope)
                            .map(|s| s.to_rust_string_lossy(iso))
                            .unwrap_or_else(|| "<string conversion failed>".into())
                    } else if a0.is_undefined() {
                        "undefined".into()
                    } else {
                        "<non-string>".into()
                    };
                    let reply = format!("[Crepus.nativeEcho ← Rust] {msg}");
                    if let Some(s) = V8String::new(scope, &reply) {
                        rv.set(s.into());
                    }
                },
            )
            .ok_or_else(|| "failed to create echo function".to_string())?;
            let _ = crepus.set(scope, echo_key.into(), echo_fn.into());

            let invoke_key = V8String::new(scope, "invoke")
                .ok_or_else(|| "failed to create invoke key".to_string())?;
            let invoke_fn = v8::Function::new(
                scope,
                |scope: &mut v8::PinScope<'_, '_>,
                 args: v8::FunctionCallbackArguments<'_>,
                 mut rv: v8::ReturnValue<'_, v8::Value>| {
                    let ctx = scope.get_current_context();
                    let reply = if let Some(slot) = ctx.get_slot::<CrepusBridgeSlot>() {
                        dispatch_invoke(&slot.0, scope, args)
                    } else {
                        json!({
                            "ok": false,
                            "error": BridgeError::new(
                                "missing_bridge_slot",
                                "V8 context has no Crepus bridge slot",
                            ),
                        })
                        .to_string()
                    };
                    if let Some(s) = V8String::new(scope, &reply) {
                        rv.set(s.into());
                    }
                },
            )
            .ok_or_else(|| "failed to create invoke function".to_string())?;
            let _ = crepus.set(scope, invoke_key.into(), invoke_fn.into());

            let crepus_key = V8String::new(scope, "Crepus")
                .ok_or_else(|| "failed to create Crepus key".to_string())?;
            let _ = global.set(scope, crepus_key.into(), crepus.into());

            let ctx_for_global = scope.get_current_context();
            let iso: &v8::Isolate = scope.deref();
            v8::Global::new(iso, ctx_for_global)
        };

        Ok(Self { isolate, context })
    }

    pub fn eval(&mut self, source: &str) -> Result<String, String> {
        let isolate = &mut *self.isolate;
        let hs = pin!(v8::HandleScope::new(isolate));
        let hs = &mut hs.init();
        let context = Local::new(hs, &self.context);
        let cx = &mut v8::ContextScope::new(hs, context);

        v8::tc_scope!(let try_catch, cx);

        let src = V8String::new(try_catch, source)
            .ok_or_else(|| "invalid or non-UTF-8 script source".to_string())?;
        let script = Script::compile(try_catch, src, None).ok_or_else(|| {
            try_catch
                .exception()
                .and_then(|e| e.to_string(try_catch))
                .map(|s| s.to_rust_string_lossy(try_catch))
                .unwrap_or_else(|| "script compile failed".into())
        })?;
        let result = script.run(try_catch).ok_or_else(|| {
            try_catch
                .exception()
                .and_then(|e| e.to_string(try_catch))
                .map(|s| s.to_rust_string_lossy(try_catch))
                .unwrap_or_else(|| "script run failed".into())
        })?;
        let out = result
            .to_string(try_catch)
            .ok_or_else(|| "result is not convertible to string".to_string())?;
        Ok(out.to_rust_string_lossy(try_catch))
    }
}

fn dispatch_invoke(
    bridge: &Bridge,
    scope: &mut v8::PinScope<'_, '_>,
    args: v8::FunctionCallbackArguments<'_>,
) -> String {
    let iso: &v8::Isolate = scope.deref();
    let plugin = args.get(0);
    let method = args.get(1);
    let payload_raw = args.get(2);
    if !plugin.is_string() || !method.is_string() || !payload_raw.is_string() {
        let envelope = json!({
            "ok": false,
            "error": BridgeError::with_details(
                "invalid_args",
                "Crepus.invoke(pluginId, methodName, payloadJsonString) requires three strings",
                json!({ "argc": args.length() }),
            ),
        });
        return envelope.to_string();
    }
    let plugin_id = match plugin.to_string(scope) {
        Some(s) => s.to_rust_string_lossy(iso),
        None => {
            return json!({
                "ok": false,
                "error": BridgeError::new("string_conversion", "failed to convert plugin id to string"),
            })
            .to_string();
        }
    };
    let method_name = match method.to_string(scope) {
        Some(s) => s.to_rust_string_lossy(iso),
        None => {
            return json!({
                "ok": false,
                "error": BridgeError::new("string_conversion", "failed to convert method name to string"),
            })
            .to_string();
        }
    };
    let payload_str = match payload_raw.to_string(scope) {
        Some(s) => s.to_rust_string_lossy(iso),
        None => {
            return json!({
                "ok": false,
                "error": BridgeError::new("string_conversion", "failed to convert payload to string"),
            })
            .to_string();
        }
    };

    /// Maximum accepted V8 invoke payload size (1 MB).
    const MAX_V8_PAYLOAD_SIZE: usize = 1024 * 1024;

    if payload_str.len() > MAX_V8_PAYLOAD_SIZE {
        return json!({
            "ok": false,
            "error": BridgeError::with_details(
                "payload_too_large",
                "payload exceeds 1 MB limit",
                json!({ "size": payload_str.len() }),
            ),
        })
        .to_string();
    }

    let payload: Value = match serde_json::from_str(&payload_str) {
        Ok(v) => v,
        Err(e) => {
            return json!({
                "ok": false,
                "error": BridgeError::with_details(
                    "invalid_payload",
                    "payload must be valid JSON",
                    json!({ "parse_error": e.to_string() }),
                ),
            })
            .to_string();
        }
    };

    serde_json::to_string(&bridge.invoke_envelope(&plugin_id, &method_name, &payload))
        .unwrap_or_else(|_| {
            json!({
                "ok": false,
                "error": BridgeError::new("internal", "failed to serialize invoke response"),
            })
            .to_string()
        })
}
