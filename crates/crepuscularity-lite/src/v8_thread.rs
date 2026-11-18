//! Dedicated OS thread that owns a [`V8Host`] so GPUI never blocks on `eval`.
//!
//! All `V8Host::eval` and isolate recreation happen on this thread only. See `docs/THREADING.md` in the repo.

use std::sync::mpsc::{self, Receiver, RecvError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::bridge::Bridge;
use crate::V8Host;

/// Owns the join handle and sends [`V8ThreadRequest::Shutdown`] on drop.
struct JoinOnDrop {
    tx: Sender<V8ThreadRequest>,
    join: Option<JoinHandle<()>>,
}

impl Drop for JoinOnDrop {
    fn drop(&mut self) {
        let _ = self.tx.send(V8ThreadRequest::Shutdown);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Spawn the V8 thread; dropping this value stops the thread (after in-flight work).
pub struct V8ThreadRuntime {
    /// Send eval RPCs here (clone freely).
    pub handle: V8ThreadHandle,
    _join: JoinOnDrop,
}

impl V8ThreadRuntime {
    pub fn spawn(initial_bridge: Arc<Bridge>) -> Result<Self, String> {
        let (handle, join) = V8ThreadHandle::spawn_inner(initial_bridge)?;
        let tx = handle.tx_clone();
        Ok(Self {
            handle,
            _join: JoinOnDrop {
                tx,
                join: Some(join),
            },
        })
    }
}

/// Commands processed sequentially on the V8 thread.
pub enum V8ThreadRequest {
    /// Run `eval(script)`. If `recreate_isolate`, replace the isolate with [`V8Host::new`] using `bridge` first.
    Eval {
        script: String,
        recreate_isolate: bool,
        bridge: Arc<Bridge>,
        reply: Sender<Result<String, String>>,
    },
    /// Stop the loop and join (used from [`Drop`]).
    Shutdown,
}

/// Handle for sending work to the V8 thread.
pub struct V8ThreadHandle {
    tx: Sender<V8ThreadRequest>,
}

impl V8ThreadHandle {
    pub(crate) fn tx_clone(&self) -> Sender<V8ThreadRequest> {
        self.tx.clone()
    }

    fn spawn_inner(initial_bridge: Arc<Bridge>) -> Result<(Self, JoinHandle<()>), String> {
        let (tx, rx) = mpsc::channel::<V8ThreadRequest>();
        let join = std::thread::Builder::new()
            .name("crepus-v8".into())
            .spawn(move || run_v8_thread(initial_bridge, rx))
            .map_err(|e| format!("spawn crepus-v8 thread: {e}"))?;
        Ok((Self { tx }, join))
    }

    /// Queue an eval. Returns a receiver for the **single** result (blocking recv OK on a background/async poll).
    pub fn eval_rpc(
        &self,
        script: String,
        recreate_isolate: bool,
        bridge: Arc<Bridge>,
    ) -> Result<Receiver<Result<String, String>>, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(V8ThreadRequest::Eval {
                script,
                recreate_isolate,
                bridge,
                reply: reply_tx,
            })
            .map_err(|_| "crepus-v8 thread disconnected".to_string())?;
        Ok(reply_rx)
    }

    /// Signal shutdown (optional; [`V8ThreadRuntime`] drop also sends this).
    pub fn shutdown(&self) {
        let _ = self.tx.send(V8ThreadRequest::Shutdown);
    }
}

fn run_v8_thread(initial_bridge: Arc<Bridge>, rx: Receiver<V8ThreadRequest>) {
    let mut host = match V8Host::new(initial_bridge) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("crepus-v8: failed to create V8Host: {e}");
            return;
        }
    };

    loop {
        let req = match rx.recv() {
            Ok(r) => r,
            Err(RecvError) => break,
        };
        match req {
            V8ThreadRequest::Eval {
                script,
                recreate_isolate,
                bridge,
                reply,
            } => {
                if recreate_isolate {
                    match V8Host::new(bridge.clone()) {
                        Ok(h) => host = h,
                        Err(e) => {
                            let _ = reply.send(Err(e));
                            continue;
                        }
                    }
                }
                let _ = reply.send(host.eval(&script));
            }
            V8ThreadRequest::Shutdown => break,
        }
    }
}
