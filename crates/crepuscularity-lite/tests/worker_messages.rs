#![cfg(feature = "v8")]

use crepuscularity_lite::{Bridge, WorkerRuntime};
use serde_json::{json, Value};

#[test]
fn worker_messages_preserve_json_keys_and_values_without_mutable_json_global() {
    let worker = WorkerRuntime::spawn(
        Bridge::compute_only_bridge(),
        r#"const stringifyOriginal = JSON.stringify;
        globalThis.onmessage = event => { globalThis.received = stringifyOriginal(event.data); };
        globalThis.JSON = { parse() { throw new Error('guest JSON.parse must not be used'); } };
        "#,
    );
    let payloads = [
        json!({"__proto__": 42, "nested": {"__proto__": {"value": "雪"}}, "array": [{"__proto__": false}, null, 3]}),
        json!(["雪 🦋", "quotes\" and slash\\", "line\nnext\r", {"__proto__": "own"}]),
        json!("scalar 雪\n\"\\"),
        json!(42),
        json!(true),
        Value::Null,
    ];
    for payload in payloads {
        worker.handle.post_message(payload.clone()).unwrap();
        let serialized = worker.handle.eval("globalThis.received").unwrap();
        let actual: Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(actual, payload);
    }
}

#[test]
fn throwing_worker_message_handler_does_not_prevent_later_delivery() {
    let worker = WorkerRuntime::spawn(
        Bridge::compute_only_bridge(),
        r#"globalThis.onmessage = event => {
          if (event.data.fail) throw new Error('message fixture failure');
          globalThis.received = JSON.stringify(event.data);
        };"#,
    );
    let error = worker
        .handle
        .post_message(json!({"fail": true}))
        .unwrap_err();
    assert!(error.contains("message fixture failure"), "{error}");
    let expected = json!({"value": "later 雪", "__proto__": 7});
    worker.handle.post_message(expected.clone()).unwrap();
    let actual: Value =
        serde_json::from_str(&worker.handle.eval("globalThis.received").unwrap()).unwrap();
    assert_eq!(actual, expected);
}
