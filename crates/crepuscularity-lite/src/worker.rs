//! Optional **compute worker**: a second V8 isolate on a dedicated thread with a restricted bridge
//! ([`Bridge::compute_only_bridge`]) — core + app only, no FS/window until those plugins are audited
//! for concurrent `invoke`.
//!
//! Messaging is Rust-side (`post_message` / channel). A small guest shim installs `onmessage`
//! and `message` listeners so guest code can receive host messages without extra setup.

use std::sync::mpsc::{self, Receiver, Sender};

use serde_json::Value;

use crate::bridge::Bridge;
use crate::v8_host::V8Host;

/// Handle to a compute worker (second isolate). Not `Clone`; use [`WorkerRuntime::handle`] or
/// spawn another worker if multiple consumers are needed.
pub struct WorkerHandle {
    tx: Sender<WorkerRequest>,
}

enum WorkerRequest {
    Eval {
        script: String,
        reply: Sender<Result<String, String>>,
    },
    PostMessage {
        payload: Value,
        reply: Sender<Result<(), String>>,
    },
    Shutdown,
}

fn run_worker_loop(rx: Receiver<WorkerRequest>, bridge: std::sync::Arc<Bridge>, entry: String) {
    let mut host = match V8Host::new(bridge) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("crepuscularity-lite: worker V8Host::new failed: {e}");
            return;
        }
    };
    if let Err(e) = host.eval(worker_bootstrap_script()) {
        eprintln!("crepuscularity-lite: worker bootstrap eval failed: {e}");
        return;
    }
    if let Err(e) = host.eval(&entry) {
        eprintln!("crepuscularity-lite: worker entry eval failed: {e}");
        return;
    }
    loop {
        match rx.recv() {
            Ok(WorkerRequest::Eval { script, reply }) => {
                let res = host.eval(&script).map_err(|e| e.to_string());
                let _ = reply.send(res);
            }
            Ok(WorkerRequest::PostMessage { payload, reply }) => {
                let json = serde_json::to_string(&payload).unwrap_or_else(|_| "null".to_string());
                let script = format!(
                    r#"if (typeof globalThis.__crepusWorkerDeliverMessage === "function") {{
  globalThis.__crepusWorkerDeliverMessage({json});
}}"#
                );
                let res = host.eval(&script).map(|_| ()).map_err(|e| e.to_string());
                let _ = reply.send(res);
            }
            Ok(WorkerRequest::Shutdown) | Err(_) => break,
        }
    }
}

fn worker_bootstrap_script() -> &'static str {
    r#"
(() => {
  const state = {
    onmessage: null,
    listeners: new Set(),
  };

  const dispatchMessage = (payload) => {
    const event = { data: payload, type: "message", target: globalThis, currentTarget: globalThis };
    if (typeof state.onmessage === "function") {
      state.onmessage(event);
    }
    for (const listener of state.listeners) {
      listener(event);
    }
  };

  Object.defineProperty(globalThis, "onmessage", {
    configurable: true,
    enumerable: true,
    get() {
      return state.onmessage;
    },
    set(value) {
      state.onmessage = typeof value === "function" ? value : null;
    },
  });

  globalThis.addEventListener = function(type, listener) {
    if (type === "message" && typeof listener === "function") {
      state.listeners.add(listener);
    }
  };

  globalThis.removeEventListener = function(type, listener) {
    if (type === "message") {
      state.listeners.delete(listener);
    }
  };

  globalThis.__crepusWorkerDeliverMessage = dispatchMessage;
})();
"#
}

/// Build this with [`WorkerRuntime::spawn`].
pub struct WorkerRuntime {
    pub handle: WorkerHandle,
    join: Option<std::thread::JoinHandle<()>>,
}

impl WorkerRuntime {
    /// Spawn a worker thread: `entry` is evaluated once (worker bootstrap), then
    /// [`WorkerHandle::eval`] / [`WorkerHandle::post_message`] run on that isolate.
    pub fn spawn(bridge: std::sync::Arc<Bridge>, entry: impl Into<String>) -> Self {
        let (tx, rx) = mpsc::channel::<WorkerRequest>();
        let entry = entry.into();
        let join = std::thread::spawn(move || run_worker_loop(rx, bridge, entry));
        WorkerRuntime {
            handle: WorkerHandle { tx },
            join: Some(join),
        }
    }
}

impl Drop for WorkerRuntime {
    fn drop(&mut self) {
        let _ = self.handle.tx.send(WorkerRequest::Shutdown);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl WorkerHandle {
    /// Evaluate `script` in the worker isolate (serialized on the worker thread).
    pub fn eval(&self, script: &str) -> Result<String, String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(WorkerRequest::Eval {
                script: script.to_string(),
                reply: tx,
            })
            .map_err(|_| "worker channel closed".to_string())?;
        rx.recv()
            .map_err(|_| "worker reply channel closed".to_string())?
    }

    /// Deliver `payload` to guest `globalThis.__crepusWorkerDeliverMessage(payload)` if defined.
    pub fn post_message(&self, payload: Value) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(WorkerRequest::PostMessage { payload, reply: tx })
            .map_err(|_| "worker channel closed".to_string())?;
        rx.recv()
            .map_err(|_| "worker reply channel closed".to_string())?
    }
}

#[cfg(test)]
mod tests {
    use super::worker_bootstrap_script;

    #[test]
    fn worker_bootstrap_installs_message_handlers() {
        let script = worker_bootstrap_script();
        assert!(script.contains("onmessage"));
        assert!(script.contains("addEventListener"));
        assert!(script.contains("__crepusWorkerDeliverMessage"));
    }
}
