//! Dedicated OS thread that owns a [`V8Host`] for queued guest evaluation.
//!
//! All `V8Host::eval` and isolate recreation happen on this thread only. See `docs/lite.md`.

use futures_channel::oneshot;

use std::sync::mpsc::{self, Receiver, RecvError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::bridge::Bridge;
use crate::V8Host;

/// Owns the join handle and sends [`V8ThreadRequest::Shutdown`] on drop.
struct JoinOnDrop {
    tx: Sender<QueuedRequest>,
    join: Option<JoinHandle<()>>,
    control: Arc<Mutex<WorkerControl>>,
}

impl Drop for JoinOnDrop {
    fn drop(&mut self) {
        let _ = self
            .tx
            .send(QueuedRequest::Legacy(V8ThreadRequest::Shutdown));
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
        let control = Arc::new(Mutex::new(WorkerControl::default()));
        let (handle, join) = V8ThreadHandle::spawn_inner(initial_bridge, control.clone())?;
        let tx = handle.tx_clone();
        Ok(Self {
            handle,
            _join: JoinOnDrop {
                tx,
                join: Some(join),
                control,
            },
        })
    }

    /// Discard queued work and interrupt JavaScript without waiting for the worker.
    ///
    /// Join the returned thread away from the UI thread. An in-flight native bridge
    /// callback cannot be interrupted and may continue until it returns.
    #[must_use = "join the worker off the UI thread to observe teardown completion"]
    pub fn abort(&mut self) -> Option<JoinHandle<()>> {
        {
            let mut control = self._join.control.lock().unwrap_or_else(|e| e.into_inner());
            control.aborted = true;
            if let Some(isolate) = &control.isolate {
                isolate.terminate_execution();
            }
        }
        self.handle.shutdown();
        self._join.join.take()
    }
}

#[derive(Default)]
struct WorkerControl {
    aborted: bool,
    isolate: Option<v8::IsolateHandle>,
}

fn is_aborted(control: &Mutex<WorkerControl>) -> bool {
    control.lock().unwrap_or_else(|e| e.into_inner()).aborted
}

fn publish_isolate(control: &Mutex<WorkerControl>, host: &V8Host) -> bool {
    let mut control = control.lock().unwrap_or_else(|e| e.into_inner());
    if control.aborted {
        return false;
    }
    control.isolate = Some(host.isolate_handle());
    true
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

enum EvalSource {
    Ready(String),
    Prepared(Box<dyn FnOnce() -> Result<String, String> + Send>),
}

// Both reply transports share the same FIFO and isolate.
enum QueuedRequest {
    Legacy(V8ThreadRequest),
    AsyncEval {
        source: EvalSource,
        recreate_isolate: bool,
        bridge: Arc<Bridge>,
        reply: oneshot::Sender<Result<String, String>>,
    },
}

/// Handle for sending work to the V8 thread.
pub struct V8ThreadHandle {
    tx: Sender<QueuedRequest>,
}

impl V8ThreadHandle {
    fn tx_clone(&self) -> Sender<QueuedRequest> {
        self.tx.clone()
    }

    fn spawn_inner(
        initial_bridge: Arc<Bridge>,
        control: Arc<Mutex<WorkerControl>>,
    ) -> Result<(Self, JoinHandle<()>), String> {
        let (tx, rx) = mpsc::channel::<QueuedRequest>();
        let join = std::thread::Builder::new()
            .name("crepus-v8".into())
            .spawn(move || run_v8_thread(initial_bridge, rx, control))
            .map_err(|e| format!("spawn crepus-v8 thread: {e}"))?;
        Ok((Self { tx }, join))
    }

    /// Queue an eval. Returns a blocking receiver for the single result; do not wait on the UI thread.
    pub fn eval_rpc(
        &self,
        script: String,
        recreate_isolate: bool,
        bridge: Arc<Bridge>,
    ) -> Result<Receiver<Result<String, String>>, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(QueuedRequest::Legacy(V8ThreadRequest::Eval {
                script,
                recreate_isolate,
                bridge,
                reply: reply_tx,
            }))
            .map_err(|_| "crepus-v8 thread disconnected".to_string())?;
        Ok(reply_rx)
    }

    /// Queue an eval whose reply can be awaited without blocking an executor thread.
    pub fn eval_rpc_async(
        &self,
        script: String,
        recreate_isolate: bool,
        bridge: Arc<Bridge>,
    ) -> Result<oneshot::Receiver<Result<String, String>>, String> {
        self.eval_source_async(EvalSource::Ready(script), recreate_isolate, bridge)
    }

    /// Queue trusted source preparation and evaluation together on the V8 worker.
    ///
    /// Preparation errors leave the isolate unchanged. Abort prevents evaluation,
    /// but cannot interrupt preparation already in progress.
    pub fn eval_prepared_rpc_async(
        &self,
        prepare: impl FnOnce() -> Result<String, String> + Send + 'static,
        recreate_isolate: bool,
        bridge: Arc<Bridge>,
    ) -> Result<oneshot::Receiver<Result<String, String>>, String> {
        self.eval_source_async(
            EvalSource::Prepared(Box::new(prepare)),
            recreate_isolate,
            bridge,
        )
    }

    fn eval_source_async(
        &self,
        source: EvalSource,
        recreate_isolate: bool,
        bridge: Arc<Bridge>,
    ) -> Result<oneshot::Receiver<Result<String, String>>, String> {
        let (reply, receiver) = oneshot::channel();
        self.tx
            .send(QueuedRequest::AsyncEval {
                source,
                recreate_isolate,
                bridge,
                reply,
            })
            .map_err(|_| "crepus-v8 thread disconnected".to_string())?;
        Ok(receiver)
    }

    /// Signal shutdown (optional; [`V8ThreadRuntime`] drop also sends this).
    pub fn shutdown(&self) {
        let _ = self
            .tx
            .send(QueuedRequest::Legacy(V8ThreadRequest::Shutdown));
    }
}

fn run_v8_thread(
    initial_bridge: Arc<Bridge>,
    rx: Receiver<QueuedRequest>,
    control: Arc<Mutex<WorkerControl>>,
) {
    if is_aborted(&control) {
        return;
    }
    let mut host = match V8Host::new(initial_bridge) {
        Ok(h) if publish_isolate(&control, &h) => Some(h),
        Ok(_) => return,
        Err(e) => {
            eprintln!("crepus-v8: failed to create V8Host: {e}");
            return;
        }
    };

    while !is_aborted(&control) {
        let req = match rx.recv() {
            Ok(r) => r,
            Err(RecvError) => break,
        };
        if is_aborted(&control) {
            break;
        }
        match req {
            QueuedRequest::Legacy(V8ThreadRequest::Eval {
                script,
                recreate_isolate,
                bridge,
                reply,
            }) => {
                let _ = reply.send(evaluate(
                    &mut host,
                    script,
                    recreate_isolate,
                    bridge,
                    &control,
                ));
            }
            QueuedRequest::AsyncEval {
                source,
                recreate_isolate,
                bridge,
                reply,
            } => {
                let script = match source {
                    EvalSource::Ready(script) => Ok(script),
                    EvalSource::Prepared(prepare) => prepare(),
                };
                let result = script.and_then(|script| {
                    evaluate(&mut host, script, recreate_isolate, bridge, &control)
                });
                let _ = reply.send(result);
            }
            QueuedRequest::Legacy(V8ThreadRequest::Shutdown) => break,
        }
    }
}

fn evaluate(
    host: &mut Option<V8Host>,
    script: String,
    recreate_isolate: bool,
    bridge: Arc<Bridge>,
    control: &Mutex<WorkerControl>,
) -> Result<String, String> {
    if is_aborted(control) {
        return Err("V8 worker aborted".into());
    }
    if recreate_isolate {
        control.lock().unwrap_or_else(|e| e.into_inner()).isolate = None;
        // V8 isolates must be dropped in reverse creation order.
        drop(host.take());
        let replacement = V8Host::new(bridge)?;
        if !publish_isolate(control, &replacement) {
            return Err("V8 worker aborted".into());
        }
        *host = Some(replacement);
    }
    let result = host
        .as_mut()
        .ok_or_else(|| "V8 isolate unavailable; request an isolate reset".to_string())?
        .eval(&script);
    if is_aborted(control) {
        Err("V8 worker aborted".into())
    } else {
        result
    }
}
