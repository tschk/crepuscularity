use crepuscularity_tauri as tauri;

#[tauri::command]
fn r#type() -> u32 {
    1
}

#[test]
fn raw_command_identifiers_use_the_unprefixed_wire_name() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler!(r#type))
        .build();
    assert_eq!(
        app.invoke("type", tauri::serde_json::Value::Null).unwrap(),
        1
    );
}

struct Prefix(String);

#[tauri::command]
#[allow(non_snake_case)]
fn exact_names(foo_bar: i32, fooBar: i32) -> (i32, i32) {
    (foo_bar, fooBar)
}

#[tauri::command(rename_all = "camelCase")]
#[allow(non_snake_case)]
fn injection_names(
    foo_bar: tauri::State<'_, Prefix>,
    fooBar: String,
    app_handle: tauri::AppHandle,
    appHandle: String,
    main_window: tauri::Window,
    mainWindow: String,
) -> String {
    drop(app_handle);
    format!(
        "{}:{fooBar}:{appHandle}:{}:{mainWindow}",
        foo_bar.0,
        main_window.label()
    )
}

#[test]
fn exact_names_stay_distinct_and_injected_arguments_do_not_claim_wire_keys() {
    let app = tauri::Builder::default()
        .manage(Prefix("state".into()))
        .invoke_handler(tauri::generate_handler!(exact_names, injection_names))
        .build();
    assert_eq!(
        app.invoke(
            "exact_names",
            tauri::serde_json::json!({"foo_bar":1,"fooBar":2})
        )
        .unwrap(),
        tauri::serde_json::json!([1, 2])
    );
    assert_eq!(
        app.invoke(
            "injection_names",
            tauri::serde_json::json!({"fooBar":"value","appHandle":"handle","mainWindow":"window"})
        )
        .unwrap(),
        "state:value:handle:main:window"
    );
}

#[tauri::command(rename_all = "camelCase")]
#[allow(non_snake_case)]
fn camel(first_name: &str, HTTP_response: String, r#type: String, élève_name: String) -> String {
    format!("{first_name}:{HTTP_response}:{type}:{élève_name}")
}

#[tauri::command(rename_all = "snake_case")]
#[allow(non_snake_case)]
async fn snake(firstName: String, HTTPResponse: String, überName: String) -> String {
    format!("{firstName}:{HTTPResponse}:{überName}")
}

#[tauri::command(rename_all = "camelCase")]
async fn injected(
    first_name: String,
    optional_value: Option<String>,
    prefix: tauri::State<'_, Prefix>,
    handle: tauri::AppHandle,
    window: tauri::Window,
) -> Result<String, String> {
    handle.emit_all("named", tauri::serde_json::json!(true))?;
    Ok(format!(
        "{}:{first_name}:{optional_value:?}:{}",
        prefix.0,
        window.label()
    ))
}

#[test]
fn explicit_argument_case_handles_borrowed_raw_acronym_and_unicode_names() {
    let app = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler!(camel, snake))
        .build();
    assert_eq!(
        app.invoke(
            "camel",
            tauri::serde_json::json!({
                "firstName":"Ada", "httpResponse":"ok", "type":"raw", "élèveName":"unicode"
            })
        )
        .unwrap(),
        "Ada:ok:raw:unicode"
    );
    assert_eq!(
        app.invoke(
            "snake",
            tauri::serde_json::json!({
                "first_name":"Ada", "http_response":"ok", "über_name":"unicode"
            })
        )
        .unwrap(),
        "Ada:ok:unicode"
    );
}

#[test]
fn renamed_payload_arguments_keep_injection_optional_values_and_diagnostics() {
    let app = tauri::Builder::default()
        .manage(Prefix("prefix".into()))
        .invoke_handler(tauri::generate_handler!(injected))
        .build();
    assert_eq!(
        app.invoke("injected", tauri::serde_json::json!({"firstName":"Ada"}))
            .unwrap(),
        "prefix:Ada:None:main"
    );
    assert_eq!(
        app.invoke(
            "injected",
            tauri::serde_json::json!({"firstName":"Ada", "optionalValue":"present"})
        )
        .unwrap(),
        "prefix:Ada:Some(\"present\"):main"
    );
    let error = app
        .invoke(
            "injected",
            tauri::serde_json::json!({"first_name":"wrong key"}),
        )
        .unwrap_err();
    assert!(error.contains("firstName"), "{error}");
}
