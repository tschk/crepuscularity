#![cfg(feature = "v8")]

use crepuscularity_lite::{Bridge, V8ThreadRuntime};
use std::{
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(10);

struct WakeThread(std::thread::Thread);
impl Wake for WakeThread {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

// Bound reply polling without an executor dependency; runtime drop still joins the worker.
fn await_reply<F: Future>(reply: F) -> F::Output {
    let waker = Waker::from(Arc::new(WakeThread(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut reply = Box::pin(reply);
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Poll::Ready(value) = reply.as_mut().poll(&mut context) {
            return value;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "V8 async reply timed out");
        std::thread::park_timeout(remaining);
    }
}

#[test]
fn legacy_and_async_requests_share_fifo_and_isolate() {
    let bridge = Bridge::default_arc();
    let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
    let first = runtime
        .handle
        .eval_rpc(
            "globalThis.order = ['legacy']; order.join(',')".into(),
            false,
            bridge.clone(),
        )
        .unwrap();
    let second = runtime
        .handle
        .eval_rpc_async(
            "order.push('async'); order.join(',')".into(),
            false,
            bridge.clone(),
        )
        .unwrap();
    let third = runtime
        .handle
        .eval_rpc(
            "order.push('legacy-again'); order.join(',')".into(),
            false,
            bridge,
        )
        .unwrap();
    // Await out of submission order: the worker must still execute FIFO.
    assert_eq!(await_reply(second).unwrap().unwrap(), "legacy,async");
    assert_eq!(
        third.recv_timeout(TIMEOUT).unwrap().unwrap(),
        "legacy,async,legacy-again"
    );
    assert_eq!(first.recv_timeout(TIMEOUT).unwrap().unwrap(), "legacy");
}

#[test]
fn async_errors_leave_isolate_usable_and_reset_recreates_it() {
    let bridge = Bridge::default_arc();
    let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
    let request = |script: &str, reset| {
        runtime
            .handle
            .eval_rpc_async(script.into(), reset, bridge.clone())
            .unwrap()
    };
    assert_eq!(
        await_reply(request("globalThis.saved = 41; saved + 1", false))
            .unwrap()
            .unwrap(),
        "42"
    );
    let error = await_reply(request("throw new Error('async transport marker')", false))
        .unwrap()
        .unwrap_err();
    assert!(error.contains("async transport marker"), "{error}");
    assert_eq!(await_reply(request("saved", false)).unwrap().unwrap(), "41");
    assert_eq!(
        await_reply(request("typeof saved", true)).unwrap().unwrap(),
        "undefined"
    );
    let syntax_error = await_reply(request("const = invalid", false))
        .unwrap()
        .unwrap_err();
    assert!(!syntax_error.is_empty());
    assert_eq!(await_reply(request("6 * 7", false)).unwrap().unwrap(), "42");
}

#[test]
fn dropping_async_receiver_does_not_cancel_evaluation_or_stop_queue() {
    let bridge = Bridge::default_arc();
    let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
    let discarded = runtime
        .handle
        .eval_rpc_async(
            "globalThis.discardedReplyRan = 7".into(),
            false,
            bridge.clone(),
        )
        .unwrap();
    drop(discarded);
    let next = runtime
        .handle
        .eval_rpc_async("discardedReplyRan + 1".into(), false, bridge)
        .unwrap();
    assert_eq!(await_reply(next).unwrap().unwrap(), "8");
}

#[test]
fn shutdown_disconnects_async_requests_without_waiting_for_reply() {
    let bridge = Bridge::default_arc();
    let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
    runtime.handle.shutdown();
    // Sending can race receiver teardown; either outcome must report disconnection.
    match runtime.handle.eval_rpc_async("1".into(), false, bridge) {
        Ok(reply) => assert!(await_reply(reply).is_err()),
        Err(error) => assert!(error.contains("disconnected"), "{error}"),
    }
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    while !condition() {
        assert!(Instant::now() < deadline, "V8 worker condition timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn abort_interrupts_running_js_and_discards_queued_reset_side_effects() {
    let bridge = Bridge::with_capabilities(
        [crepuscularity_lite::Capability::Host]
            .into_iter()
            .collect(),
    );
    let mut runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
    let running = runtime.handle.eval_rpc_async(
        r#"Crepus.invoke('host', 'storageSet', '{"key":"started","value":true}'); while (true) {}"#.into(),
        false,
        bridge.clone(),
    ).unwrap();
    wait_until(|| bridge.host_state().storage_get("started") == Some(serde_json::json!(true)));
    let queued = runtime
        .handle
        .eval_rpc_async(
            r#"Crepus.invoke('host', 'storageSet', '{"key":"queued","value":true}')"#.into(),
            true,
            bridge.clone(),
        )
        .unwrap();
    let worker = runtime.abort().unwrap();
    assert!(runtime.abort().is_none());
    drop(runtime);
    wait_until(|| worker.is_finished());
    worker.join().unwrap();
    assert!(await_reply(running).map_or(true, |result| result.is_err()));
    assert!(await_reply(queued).is_err());
    assert_eq!(bridge.host_state().storage_get("queued"), None);
}

#[test]
fn abort_can_race_initialization_and_isolate_reset() {
    let bridge = Bridge::compute_only_bridge();
    for reset in [false, true] {
        for _ in 0..8 {
            let mut runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
            let reply = runtime
                .handle
                .eval_rpc_async("while (true) {}".into(), reset, bridge.clone())
                .unwrap();
            let worker = runtime.abort().unwrap();
            drop(runtime);
            wait_until(|| worker.is_finished());
            worker.join().unwrap();
            assert!(await_reply(reply).map_or(true, |result| result.is_err()));
        }
    }
}

#[test]
fn graceful_shutdown_still_finishes_previously_queued_evaluations() {
    let bridge = Bridge::compute_only_bridge();
    let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
    let first = runtime
        .handle
        .eval_rpc_async("globalThis.value = 41".into(), false, bridge.clone())
        .unwrap();
    let second = runtime
        .handle
        .eval_rpc_async("value + 1".into(), false, bridge)
        .unwrap();
    runtime.handle.shutdown();
    drop(runtime);
    assert_eq!(await_reply(first).unwrap().unwrap(), "41");
    assert_eq!(await_reply(second).unwrap().unwrap(), "42");
}

#[test]
fn source_preparation_runs_off_caller_thread_in_guest_event_fifo() {
    use std::sync::mpsc;
    let bridge = Bridge::compute_only_bridge();
    let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
    let caller = std::thread::current().id();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first = runtime
        .handle
        .eval_rpc("globalThis.order = ['first']".into(), false, bridge.clone())
        .unwrap();
    let prepared = runtime
        .handle
        .eval_prepared_rpc_async(
            move || {
                entered_tx
                    .send(std::thread::current().id())
                    .map_err(|e| e.to_string())?;
                release_rx
                    .recv_timeout(TIMEOUT)
                    .map_err(|e| e.to_string())?;
                Ok("order.push('prepared'); order.join(',')".into())
            },
            false,
            bridge.clone(),
        )
        .unwrap();
    let event = runtime
        .handle
        .eval_rpc("order.push('event'); order.join(',')".into(), false, bridge)
        .unwrap();
    assert_ne!(entered_rx.recv_timeout(TIMEOUT).unwrap(), caller);
    assert!(matches!(event.try_recv(), Err(mpsc::TryRecvError::Empty)));
    release_tx.send(()).unwrap();
    assert_eq!(first.recv_timeout(TIMEOUT).unwrap().unwrap(), "first");
    assert_eq!(await_reply(prepared).unwrap().unwrap(), "first,prepared");
    assert_eq!(
        event.recv_timeout(TIMEOUT).unwrap().unwrap(),
        "first,prepared,event"
    );
}

#[test]
fn preparation_failure_preserves_isolate_and_next_request_runs() {
    let bridge = Bridge::compute_only_bridge();
    let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
    let first = runtime
        .handle
        .eval_rpc("globalThis.saved = 42".into(), false, bridge.clone())
        .unwrap();
    let failed = runtime
        .handle
        .eval_prepared_rpc_async(|| Err("compile marker".into()), true, bridge.clone())
        .unwrap();
    let next = runtime
        .handle
        .eval_rpc_async("saved".into(), false, bridge)
        .unwrap();
    assert_eq!(first.recv_timeout(TIMEOUT).unwrap().unwrap(), "42");
    assert_eq!(await_reply(failed).unwrap().unwrap_err(), "compile marker");
    assert_eq!(await_reply(next).unwrap().unwrap(), "42");
}

#[test]
fn abort_during_preparation_prevents_eval_and_discards_queued_preparation() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    let bridge = Bridge::with_capabilities(
        [crepuscularity_lite::Capability::Host]
            .into_iter()
            .collect(),
    );
    let mut runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let running = runtime
        .handle
        .eval_prepared_rpc_async(
            move || {
                entered_tx.send(()).map_err(|e| e.to_string())?;
                release_rx
                    .recv_timeout(TIMEOUT)
                    .map_err(|e| e.to_string())?;
                Ok(
                    r#"Crepus.invoke('host', 'storageSet', '{"key":"prepared","value":true}')"#
                        .into(),
                )
            },
            true,
            bridge.clone(),
        )
        .unwrap();
    entered_rx.recv_timeout(TIMEOUT).unwrap();
    let queued_ran = Arc::new(AtomicBool::new(false));
    let observed = queued_ran.clone();
    let queued = runtime
        .handle
        .eval_prepared_rpc_async(
            move || {
                observed.store(true, Ordering::SeqCst);
                Ok("42".into())
            },
            false,
            bridge.clone(),
        )
        .unwrap();
    let worker = runtime.abort().unwrap();
    drop(runtime);
    assert!(!worker.is_finished(), "preparation must still be waiting");
    release_tx.send(()).unwrap();
    wait_until(|| worker.is_finished());
    worker.join().unwrap();
    assert!(await_reply(running).unwrap().is_err());
    assert!(await_reply(queued).is_err());
    assert!(!queued_ran.load(Ordering::SeqCst));
    assert_eq!(bridge.host_state().storage_get("prepared"), None);
}

#[test]
fn dropping_prepared_reply_still_runs_preparation_and_evaluation() {
    use std::sync::mpsc;
    let bridge = Bridge::compute_only_bridge();
    let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let discarded = runtime
        .handle
        .eval_prepared_rpc_async(
            move || {
                entered_tx.send(()).map_err(|e| e.to_string())?;
                release_rx
                    .recv_timeout(TIMEOUT)
                    .map_err(|e| e.to_string())?;
                Ok("globalThis.preparedValue = 7".into())
            },
            false,
            bridge.clone(),
        )
        .unwrap();
    entered_rx.recv_timeout(TIMEOUT).unwrap();
    drop(discarded);
    release_tx.send(()).unwrap();
    let next = runtime
        .handle
        .eval_rpc_async("preparedValue + 1".into(), false, bridge)
        .unwrap();
    assert_eq!(await_reply(next).unwrap().unwrap(), "8");
}
