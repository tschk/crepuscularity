use std::process::Command;
use std::sync::Once;

static BUILD_SLIM: Once = Once::new();

/// Returns a `Command` for the `crepus` binary built without the `desktop`
/// feature. On Windows CI the full binary links GPUI DLLs that aren't present;
/// the tauri/native commands don't need them.
fn crepus() -> Command {
    BUILD_SLIM.call_once(|| {
        let status = Command::new(env!("CARGO"))
            .args([
                "build",
                "--locked",
                "-p",
                "crepuscularity-cli",
                "--no-default-features",
            ])
            .status()
            .expect("cargo build slim crepus");
        assert!(status.success(), "slim crepus build failed");
    });
    let pkg_dir = env!("CARGO_MANIFEST_DIR");
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let bin = std::path::Path::new(pkg_dir)
        .join("../../target")
        .join(profile)
        .join(if cfg!(windows) {
            "crepus.exe"
        } else {
            "crepus"
        });
    Command::new(bin)
}

/// Run a command and panic with full diagnostics (exit code, stderr, stdout) on failure.
fn assert_success(out: &std::process::Output, label: &str) {
    if !out.status.success() {
        panic!(
            "[{label}] exit={status} stderr={stderr:?} stdout={stdout:?}",
            status = out.status,
            stderr = String::from_utf8_lossy(&out.stderr),
            stdout = String::from_utf8_lossy(&out.stdout),
        );
    }
}

fn fixture(version: u8) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("src-tauri")).unwrap();
    std::fs::create_dir_all(root.path().join("dist")).unwrap();
    let config = if version == 1 {
        r#"{"build":{"distDir":"../dist"}}"#
    } else {
        r#"{"build":{"frontendDist":"../dist"}}"#
    };
    std::fs::write(root.path().join("src-tauri/tauri.conf.json"), config).unwrap();
    std::fs::write(
        root.path().join("dist/crepus-bundle.json"),
        r#"{"entry":"index.crepus","files":{"index.crepus":"div flex flex-col\n  \"Hello\""}}"#,
    )
    .unwrap();
    root
}

#[test]
fn converts_v1_and_v2_static_crepus_bundles() {
    for version in [1, 2] {
        let source = fixture(version);
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().join("native");
        let out = crepus()
            .args([
                "tauri",
                "convert",
                "--dir",
                source.path().to_str().unwrap(),
                "--out",
                destination.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_success(&out, "converts_v1_and_v2");
        assert!(destination.join("fixture.json").is_file());
        assert!(destination
            .join("ios/Sources/NativeShell/Generated/CrepusGeneratedView.swift")
            .is_file());
        assert!(destination.join("android/app/src/main/java/dev/crepuscularity/nativeshell/generated/CrepusGeneratedView.kt").is_file());
    }
}

#[test]
fn webview_conversion_targets_mobile_and_omits_desktop_from_all() {
    let source = fixture(2);
    std::fs::write(
        source.path().join("dist/crepus-bundle.json"),
        r#"{"entry":"index.crepus","files":{"index.crepus":"embed https://example.com adapter=\"webview\""}}"#,
    )
    .unwrap();
    for target in ["mobile", "all"] {
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().join("native");
        let out = crepus()
            .args([
                "tauri",
                "convert",
                "--target",
                target,
                "--dir",
                source.path().to_str().unwrap(),
                "--out",
                destination.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_success(&out, "webview_conversion_targets_mobile");
        assert!(destination.join("ios").is_dir());
        assert!(destination.join("android").is_dir());
        assert!(!destination.join("desktop").exists());
        if target == "all" {
            assert!(String::from_utf8_lossy(&out.stderr).contains("omit GPUI desktop output"));
        }
    }
}

#[test]
fn webview_conversion_rejects_desktop() {
    let source = fixture(2);
    std::fs::write(
        source.path().join("dist/crepus-bundle.json"),
        r#"{"entry":"index.crepus","files":{"index.crepus":"embed https://example.com adapter=\"webview\""}}"#,
    )
    .unwrap();
    let output = tempfile::tempdir().unwrap();
    let destination = output.path().join("native");
    let out = crepus()
        .args([
            "tauri",
            "convert",
            "--target",
            "desktop",
            "--dir",
            source.path().to_str().unwrap(),
            "--out",
            destination.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("GPUI has no WebView host"));
}

#[test]
fn converts_shipped_examples() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for name in ["tauri-v1-crepus", "tauri-v2-crepus", "tauri-v2-multiwindow"] {
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().join("native");
        let out = crepus()
            .args([
                "tauri",
                "convert",
                "--dir",
                root.join("examples")
                    .join("advanced")
                    .join(name)
                    .to_str()
                    .unwrap(),
                "--out",
                destination.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_success(&out, "converts_shipped_examples");
        assert!(destination.join("fixture.json").is_file());
    }
}

#[test]
fn preserves_nested_bundle_entries_for_native_sync() {
    let source = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(source.path().join("src-tauri")).unwrap();
    std::fs::create_dir_all(source.path().join("dist")).unwrap();
    std::fs::write(
        source.path().join("src-tauri/tauri.conf.json"),
        r#"{"build":{"frontendDist":"../dist"}}"#,
    )
    .unwrap();
    std::fs::write(
        source.path().join("dist/crepus-bundle.json"),
        r#"{"entry":"app/index.crepus","files":{"app/index.crepus":"include card.crepus#Card","app/card.crepus":"--- Card\ndiv\n  \"Nested\""}}"#,
    )
    .unwrap();
    let output = tempfile::tempdir().unwrap();
    let destination = output.path().join("native");
    let converted = crepus()
        .args([
            "tauri",
            "convert",
            "--dir",
            source.path().to_str().unwrap(),
            "--out",
            destination.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_success(&converted, "preserves_nested_convert");
    let sync = crepus()
        .args([
            "native",
            "ir",
            destination.join("views/main.crepus").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_success(&sync, "preserves_nested_sync");
}

#[test]
fn rejects_unsafe_bundle_without_creating_output() {
    let source = fixture(2);
    std::fs::write(
        source.path().join("dist/crepus-bundle.json"),
        r#"{"entry":"../bad.crepus","files":{"../bad.crepus":"div"}}"#,
    )
    .unwrap();
    let output = tempfile::tempdir().unwrap();
    let destination = output.path().join("native");
    let converted = crepus()
        .args([
            "tauri",
            "convert",
            "--dir",
            source.path().to_str().unwrap(),
            "--out",
            destination.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!converted.status.success());
    assert!(!destination.exists());
}

#[test]
fn audit_and_conversion_reject_unadapted_tauri_commands() {
    let source = fixture(2);
    std::fs::write(
        source.path().join("src-tauri/lib.rs"),
        "#[tauri::command]\nfn greet() {}\n",
    )
    .unwrap();
    let audit = crepus()
        .args(["tauri", "audit", "--dir", source.path().to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!audit.status.success());
    assert!(String::from_utf8_lossy(&audit.stdout).contains("command"));
    let output = tempfile::tempdir().unwrap();
    let destination = output.path().join("native");
    let converted = crepus()
        .args([
            "tauri",
            "convert",
            "--dir",
            source.path().to_str().unwrap(),
            "--out",
            destination.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!converted.status.success());
    assert!(!destination.exists());
}

#[test]
fn conversion_preserves_tauri_application_metadata() {
    let source = fixture(2);
    std::fs::write(
        source.path().join("src-tauri/tauri.conf.json"),
        r#"{"productName":"Desk","version":"2.3.4","identifier":"dev.example.desk","build":{"frontendDist":"../dist"},"app":{"windows":[{"label":"main","title":"Desk Window","width":800,"height":600},{"label":"settings","title":"Settings","width":480,"height":360}]}}"#,
    )
    .unwrap();
    let output = tempfile::tempdir().unwrap();
    let destination = output.path().join("native");
    let converted = crepus()
        .args([
            "tauri",
            "convert",
            "--dir",
            source.path().to_str().unwrap(),
            "--out",
            destination.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_success(&converted, "conversion_preserves_metadata");
    assert!(
        std::fs::read_to_string(destination.join("android/app/src/main/AndroidManifest.xml"))
            .unwrap()
            .contains("Desk Window")
    );
    assert!(destination
        .join("android/app/src/main/res/values/themes.xml")
        .is_file());
    assert!(
        std::fs::read_to_string(destination.join("android/app/build.gradle.kts"))
            .unwrap()
            .contains("applicationId = \"dev.example.desk\"")
    );
    let desktop = std::fs::read_to_string(destination.join("desktop/src/main.rs")).unwrap();
    assert_eq!(desktop.matches("cx.open_window").count(), 2);
    assert!(desktop.contains("\"settings\", \"Settings\""));
    assert!(desktop.contains("px(480.), px(360.)"));
}

#[test]
fn conversion_preserves_main_sources_and_relative_graphs_in_both_view_roots() {
    for (entry, files, prefix, expected) in [
        (
            "main.crepus",
            vec![
                (
                    "main.crepus",
                    "text \"Direct main\"\ninclude \"nested/child.crepus\"\n",
                ),
                ("nested/child.crepus", "text \"Nested child\"\n"),
            ],
            "",
            vec!["Direct main", "Nested child"],
        ),
        (
            "index.crepus",
            vec![
                (
                    "index.crepus",
                    "include \"main.crepus\"\ninclude \"nested/child.crepus\"\n",
                ),
                ("main.crepus", "text \"Included main\"\n"),
                ("nested/child.crepus", "include \"parts/leaf.crepus\"\n"),
                ("nested/parts/leaf.crepus", "text \"Deep leaf\"\n"),
            ],
            "__crepus_source/",
            vec!["Included main", "Deep leaf"],
        ),
        (
            "page folder/start page.crepus",
            vec![
                (
                    "page folder/start page.crepus",
                    "include \"parts/child file.crepus\"\n",
                ),
                (
                    "page folder/parts/child file.crepus",
                    "text \"Space path\"\n",
                ),
                ("main.crepus", "text \"Unused main preserved\"\n"),
            ],
            "__crepus_source/",
            vec!["Space path"],
        ),
    ] {
        let source = fixture(2);
        let file_map: serde_json::Map<String, serde_json::Value> = files
            .iter()
            .map(|(path, content)| (path.to_string(), serde_json::json!(content)))
            .collect();
        std::fs::write(
            source.path().join("dist/crepus-bundle.json"),
            serde_json::to_vec(&serde_json::json!({ "entry": entry, "files": file_map })).unwrap(),
        )
        .unwrap();
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().join("converted with spaces");
        // Use the test build directly; this regression must not start a nested Cargo build.
        let converted = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .args(["tauri", "convert", "--target", "all", "--dir"])
            .arg(source.path())
            .arg("--out")
            .arg(&destination)
            .output()
            .unwrap();
        assert_success(&converted, entry);
        for view_root in [destination.join("views"), destination.join("desktop/views")] {
            for (path, content) in &files {
                assert_eq!(
                    std::fs::read_to_string(view_root.join(prefix).join(path)).unwrap(),
                    *content,
                    "source changed: {entry}: {path}"
                );
            }
            let main = view_root.join("main.crepus");
            let text = std::fs::read_to_string(&main).unwrap();
            let mut ctx = crepuscularity_core::context::TemplateContext::new();
            ctx.base_dir = Some(view_root.clone());
            let rendered =
                crepuscularity_native::render_template_to_ir_with_path(&text, &ctx, Some(&main))
                    .unwrap_or_else(|error| panic!("{entry} at {}: {error}", main.display()));
            let json = crepuscularity_native::to_json(&rendered).unwrap();
            for content in &expected {
                assert!(json.contains(content), "missing {content:?}: {json}");
            }
            if prefix.is_empty() {
                assert_eq!(text, files[0].1);
            }
        }
    }
}

#[test]
fn converted_android_label_escapes_metadata_and_retains_title_precedence() {
    for (product_name, window_title, expected) in [
        (
            Some("AT&T \"Docs\" <雪> 'Team'"),
            None,
            "AT&amp;T &quot;Docs&quot; &lt;雪&gt; &#39;Team&#39;",
        ),
        (
            Some("Product fallback"),
            Some("Window & \"雪\" <Title>"),
            "Window &amp; &quot;雪&quot; &lt;Title&gt;",
        ),
        (Some("Plain product"), None, "Plain product"),
        (None, None, "NativeShell"),
    ] {
        let source = fixture(2);
        let mut config = serde_json::json!({"build":{"frontendDist":"../dist"}});
        if let Some(product_name) = product_name {
            config["productName"] = serde_json::json!(product_name);
        }
        if let Some(title) = window_title {
            config["app"] = serde_json::json!({"windows":[{"label":"main","title":title}]});
        }
        std::fs::write(
            source.path().join("src-tauri/tauri.conf.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().join("converted");
        let converted = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .args(["tauri", "convert", "--target", "mobile", "--dir"])
            .arg(source.path())
            .arg("--out")
            .arg(&destination)
            .output()
            .unwrap();
        assert_success(&converted, "XML metadata label");
        let manifest =
            std::fs::read_to_string(destination.join("android/app/src/main/AndroidManifest.xml"))
                .unwrap();
        let label = manifest
            .lines()
            .find(|line| line.contains("android:label="))
            .unwrap()
            .trim();
        assert_eq!(label, format!("android:label=\"{expected}\""));
    }
}

#[test]
fn conversion_preserves_v1_nested_application_metadata() {
    for title in [None, Some("V1 Window")] {
        let source = fixture(1);
        let mut config = serde_json::json!({
            "build": {"distDir": "../dist"},
            "package": {"productName": "V1 Product", "version": "2.3.4"},
            "tauri": {"bundle": {"identifier": "dev.example.v1"}}
        });
        if let Some(title) = title {
            config["tauri"]["windows"] = serde_json::json!([{"label":"main", "title":title}]);
        }
        std::fs::write(
            source.path().join("src-tauri/tauri.conf.json"),
            serde_json::to_vec(&config).unwrap(),
        )
        .unwrap();
        let output = tempfile::tempdir().unwrap();
        let destination = output.path().join("native");
        let converted = Command::new(env!("CARGO_BIN_EXE_crepus"))
            .args(["tauri", "convert", "--target", "mobile", "--dir"])
            .arg(source.path())
            .arg("--out")
            .arg(&destination)
            .output()
            .unwrap();
        assert_success(&converted, "v1 nested metadata");
        let manifest =
            std::fs::read_to_string(destination.join("android/app/src/main/AndroidManifest.xml"))
                .unwrap();
        assert!(
            manifest.contains(&format!(
                "android:label=\"{}\"",
                title.unwrap_or("V1 Product")
            )),
            "{manifest}"
        );
        let gradle =
            std::fs::read_to_string(destination.join("android/app/build.gradle.kts")).unwrap();
        assert!(
            gradle.contains("applicationId = \"dev.example.v1\""),
            "{gradle}"
        );
        assert!(gradle.contains("versionName = \"2.3.4\""), "{gradle}");
        let project = std::fs::read_to_string(destination.join("ios/project.yml")).unwrap();
        assert!(
            project.contains("PRODUCT_BUNDLE_IDENTIFIER: dev.example.v1"),
            "{project}"
        );
    }
}
