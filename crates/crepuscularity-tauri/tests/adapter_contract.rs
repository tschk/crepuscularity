use crepuscularity_tauri as tauri;
use serde_json::{json, Value};
use std::fs;
use std::time::Duration;

#[tauri::command(rename_all = "camelCase")]
fn payload(payload: String, first_name: &str, r#type: &str) -> String {
    format!("{payload}:{first_name}:{type}")
}

#[tauri::command(rename_all = "snake_case")]
fn r#type(first_name: String) -> String {
    first_name
}

#[tauri::command]
async fn local(__crepus_app: String, __crepus_payload: &str, __crepus_argument_0: i32) -> String {
    let value = std::rc::Rc::new(__crepus_app);
    tokio::time::sleep(Duration::from_millis(1)).await;
    format!("{value}:{__crepus_payload}:{__crepus_argument_0}")
}

#[test]
fn macros_preserve_authored_names_and_explicit_wire_case() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler!(payload, r#type, local))
        .build();
    assert_eq!(
        app.invoke(
            "payload",
            json!({"payload":"a", "firstName":"b", "type":"c"})
        )
        .unwrap(),
        "a:b:c"
    );
    assert_eq!(
        app.invoke("type", json!({"first_name":"snake"})).unwrap(),
        "snake"
    );
    assert!(app
        .invoke(
            "payload",
            json!({"payload":"a", "first_name":"b", "type":"c"})
        )
        .is_err());
    assert_eq!(
        app.invoke(
            "local",
            json!({"__crepus_app":"a", "__crepus_payload":"b", "__crepus_argument_0":7})
        )
        .unwrap(),
        "a:b:7"
    );
}

#[test]
fn async_invocation_keeps_a_single_thread_runtime_progressing() {
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let caller_task = tokio::spawn(async {
                tokio::time::sleep(Duration::from_millis(1)).await;
                42
            });
            let app = tauri::Builder::default()
                .command("panic", |_| panic!("expected worker panic"))
                .invoke_handler(tauri::generate_handler!(local))
                .build();
            assert_eq!(
                app.invoke_async(
                    "local",
                    json!({"__crepus_app":"a", "__crepus_payload":"b", "__crepus_argument_0":7})
                )
                .await
                .unwrap(),
                "a:b:7"
            );
            assert_eq!(caller_task.await.unwrap(), 42);
            assert!(app
                .invoke_async("missing", Value::Null)
                .await
                .unwrap_err()
                .contains("unknown command"));
            assert!(app
                .invoke_async("panic", Value::Null)
                .await
                .unwrap_err()
                .contains("expected worker panic"));
        });
        tx.send(()).unwrap();
    });
    rx.recv_timeout(Duration::from_secs(10))
        .expect("invocation stalled");
    worker.join().unwrap();
}

#[test]
fn dropping_captured_subscriptions_does_not_deadlock_event_registration() {
    let app = tauri::Builder::default().build();
    let child = app.listen("child", |_| {});
    let parent = app.listen("parent", move |_| {
        let _keep_alive = &child;
    });
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        drop(parent);
        tx.send(()).unwrap();
    });
    rx.recv_timeout(Duration::from_secs(2))
        .expect("listener cleanup deadlocked");
    worker.join().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let _listener = app.listen("ready", move |_| {
        tx.send(()).unwrap();
    });
    app.emit("ready", Value::Null);
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
}

#[test]
fn project_metadata_and_named_bundle_entries_follow_version_contracts() {
    for (build, expected) in [("distDir", "V1"), ("frontendDist", "V2")] {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("src-tauri")).unwrap();
        fs::create_dir(root.path().join("dist")).unwrap();
        let mut config = json!({
            "productName":"V2", "version":"2.0.0", "identifier":"dev.v2",
            "package":{"productName":"V1", "version":"1.0.0"},
            "tauri":{"bundle":{"identifier":"dev.v1"}}, "build":{}
        });
        config["build"][build] = json!("../dist");
        fs::write(
            root.path().join("src-tauri/tauri.conf.json"),
            config.to_string(),
        )
        .unwrap();
        fs::write(root.path().join("dist/crepus-bundle.json"), json!({
            "entry":"ui.crepus#Card", "files":{"ui.crepus":"--- Card\nspan \"SELECTED\"\n--- Other\nspan \"UNUSED\""}
        }).to_string()).unwrap();
        let project = tauri::TauriProject::open(root.path()).unwrap();
        let metadata = project.metadata();
        assert_eq!(metadata.product_name.as_deref(), Some(expected));
        assert_eq!(
            metadata.identifier.as_deref(),
            Some(if expected == "V1" { "dev.v1" } else { "dev.v2" })
        );
        assert_eq!(project.bundle().unwrap().entry, "ui.crepus#Card");
        #[cfg(feature = "native")]
        {
            let text = crepuscularity_native::to_json(&project.native_ir().unwrap()).unwrap();
            assert!(text.contains("SELECTED") && !text.contains("UNUSED"));
        }
    }
}

#[test]
#[cfg(unix)]
fn audit_terminates_on_link_cycles_and_detects_command_options() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("tauri.conf.json"),
        r#"{"build":{"frontendDist":"dist"}}"#,
    )
    .unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    std::os::unix::fs::symlink(".", root.path().join("src/again")).unwrap();
    fs::write(
        root.path().join("src/yes.rs"),
        "# [ tauri :: command(rename_all = \"camelCase\") ] fn yes() {}",
    )
    .unwrap();
    fs::write(root.path().join("src/no.rs"), "#[commandment] fn no() {}").unwrap();
    let report = tauri::TauriProject::open(root.path()).unwrap().audit();
    assert_eq!(
        report
            .uses
            .iter()
            .filter(|usage| usage.api == "command")
            .count(),
        1
    );
}
