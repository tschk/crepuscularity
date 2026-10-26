use crepuscularity_tauri as tauri;
use tauri::Manager;

#[tauri::command]
fn greet(name: &str) -> Result<String, String> {
    Ok(format!("Hello, {name}"))
}

#[tauri::command]
async fn count(value: u32) -> u32 {
    value + 1
}

struct Prefix(String);

#[tauri::command]
fn payload(payload: String, count: i32) -> String {
    format!("{payload}:{count}")
}

#[tauri::command]
fn app(
    app: String,
    prefix: tauri::State<'_, Prefix>,
    handle: tauri::AppHandle,
    window: tauri::Window,
) -> String {
    handle.emit_all("hygiene", serde_json::json!(true)).unwrap();
    format!("{}:{app}:{}", prefix.0, window.label())
}

#[tauri::command]
async fn internal_names(
    __crepus_app: &str,
    __crepus_payload: String,
    __crepus_argument_0: i32,
    r#type: &str,
) -> String {
    format!("{__crepus_app}:{__crepus_payload}:{__crepus_argument_0}:{type}")
}

#[test]
fn authored_names_do_not_shadow_generated_bridge_inputs_or_command_functions() {
    let host = tauri::Builder::default()
        .manage(Prefix("prefix".into()))
        .invoke_handler(tauri::generate_handler!(payload, app, internal_names))
        .build();
    assert_eq!(
        host.invoke("payload", serde_json::json!({"payload":"text","count":7}))
            .unwrap(),
        "text:7"
    );
    assert_eq!(
        host.invoke("app", serde_json::json!({"app":"value"}))
            .unwrap(),
        "prefix:value:main"
    );
    assert_eq!(host.invoke("internal_names", serde_json::json!({"__crepus_app":"borrowed","__crepus_payload":"owned","__crepus_argument_0":9,"type":"raw"})).unwrap(), "borrowed:owned:9:raw");
}

#[tauri::command]
fn stateful(prefix: tauri::State<'_, Prefix>, name: String) -> String {
    format!("{} {name}", prefix.0)
}

#[tauri::command]
fn window_label(handle: tauri::AppHandle, window: tauri::Window) -> String {
    handle.emit_all("ready", serde_json::json!(true)).unwrap();
    window.label().to_string()
}

#[test]
fn standard_tauri_command_shape_routes_to_native_bridge() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler!(greet))
        .run(tauri::generate_context!())
        .unwrap();
    assert_eq!(
        app.invoke("greet", serde_json::json!({ "name": "Ada" }))
            .unwrap(),
        serde_json::json!("Hello, Ada")
    );
}

#[test]
fn async_commands_route_to_native_bridge() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler!(count))
        .build();
    assert_eq!(
        app.invoke("count", serde_json::json!({ "value": 41 }))
            .unwrap(),
        42
    );
}

#[test]
fn managed_state_routes_to_commands() {
    let app = tauri::Builder::default()
        .manage(Prefix("Hello,".into()))
        .invoke_handler(tauri::generate_handler!(stateful))
        .build();
    assert_eq!(
        app.invoke("stateful", serde_json::json!({ "name": "Ada" }))
            .unwrap(),
        "Hello, Ada"
    );
}

#[test]
fn injected_app_handle_and_window_route_to_commands() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler!(window_label))
        .build();
    assert_eq!(
        app.invoke("window_label", serde_json::Value::Null).unwrap(),
        "main"
    );
}

#[test]
fn manager_and_async_runtime_match_tauri_imports() {
    let app = tauri::Builder::default().build();
    <tauri::App as Manager>::emit_all(&app, "ready", serde_json::json!(true)).unwrap();
    assert_eq!(
        <tauri::App as Manager>::get_window(&app, "main")
            .unwrap()
            .label(),
        "main"
    );
    let handle = tauri::async_runtime::spawn(async { 42 });
    assert_eq!(tauri::block_on(handle).unwrap(), 42);
}

#[tauri::command]
async fn local_greet(prefix: tauri::State<'_, Prefix>, name: &str) -> Result<String, String> {
    // This Rc deliberately lives across an await: command futures need not be Send.
    let name = std::rc::Rc::new(name.to_string());
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    tokio::task::yield_now().await;
    if name.is_empty() {
        return Err("name is required".into());
    }
    Ok(format!("{} {name}", prefix.0))
}

struct CallerTask(std::sync::Mutex<Option<tokio::task::JoinHandle<u32>>>);

#[tauri::command]
async fn await_caller_task(task: tauri::State<'_, CallerTask>) -> Result<u32, String> {
    let handle = task
        .0
        .lock()
        .unwrap()
        .take()
        .ok_or("task already consumed")?;
    handle.await.map_err(|error| error.to_string())
}

#[test]
fn invoke_async_preserves_command_contracts_and_caller_runtime_progress() {
    for current_thread in [true, false] {
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut builder = if current_thread {
                tokio::runtime::Builder::new_current_thread()
            } else {
                let mut builder = tokio::runtime::Builder::new_multi_thread();
                builder.worker_threads(1);
                builder
            };
            let runtime = builder.enable_all().build().unwrap();
            runtime.block_on(async {
                let caller_task = tokio::spawn(async {
                    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                    42
                });
                let caller = std::thread::current().id();
                let app = tauri::Builder::default()
                    .command("sync", move |_| {
                        Ok(serde_json::json!(std::thread::current().id() == caller))
                    })
                    .manage(Prefix("Hello,".into()))
                    .manage(CallerTask(std::sync::Mutex::new(Some(caller_task))))
                    .invoke_handler(tauri::generate_handler!(local_greet, await_caller_task))
                    .build();
                assert_eq!(app.invoke("sync", serde_json::Value::Null).unwrap(), true);
                assert_eq!(
                    app.invoke_async("await_caller_task", serde_json::Value::Null)
                        .await
                        .unwrap(),
                    42
                );
                assert_eq!(
                    app.invoke_async("local_greet", serde_json::json!({ "name": "Ada" }))
                        .await
                        .unwrap(),
                    "Hello, Ada"
                );
                assert_eq!(
                    app.invoke_async("local_greet", serde_json::json!({ "name": "" }))
                        .await
                        .unwrap_err(),
                    "name is required"
                );
                assert!(app
                    .invoke_async("missing", serde_json::Value::Null)
                    .await
                    .unwrap_err()
                    .contains("unknown command"));
                assert!(app
                    .invoke_async("local_greet", serde_json::json!({ "name": 42 }))
                    .await
                    .unwrap_err()
                    .contains("invalid command argument"));
                let unmanaged = tauri::Builder::default()
                    .invoke_handler(tauri::generate_handler!(local_greet))
                    .build();
                assert!(unmanaged
                    .invoke_async("local_greet", serde_json::json!({ "name": "Ada" }))
                    .await
                    .unwrap_err()
                    .contains("unmanaged state"));
            });
            tx.send(()).unwrap();
        });
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .unwrap_or_else(|error| {
                panic!("current_thread={current_thread}: invocation panicked or stalled: {error}")
            });
        worker.join().unwrap();
    }
}

#[test]
fn invoke_async_works_without_a_caller_runtime_and_reports_worker_panics() {
    let caller = std::thread::current().id();
    let app = tauri::Builder::default()
        .command("thread", move |_| {
            Ok(serde_json::json!(std::thread::current().id() == caller))
        })
        .command("panic", |_| panic!("expected command panic"))
        .invoke_handler(tauri::generate_handler!(count))
        .build();
    assert_eq!(app.invoke("thread", serde_json::Value::Null).unwrap(), true);
    assert_eq!(
        tauri::block_on(app.invoke_async("thread", serde_json::Value::Null)).unwrap(),
        false
    );
    assert_eq!(
        tauri::block_on(app.invoke_async("count", serde_json::json!({ "value": 41 }))).unwrap(),
        42
    );
    let error = tauri::block_on(app.invoke_async("panic", serde_json::Value::Null)).unwrap_err();
    assert!(
        error.contains("task failed") && error.contains("expected command panic"),
        "{error}"
    );
}
