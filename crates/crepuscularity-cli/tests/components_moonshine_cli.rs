//! Integration tests for `crepus components`, `crepus moonshine`, and `--emit`.

use std::process::Command;

fn crepus() -> Command {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn components_list_runs() {
    let output = crepus()
        .args(["components", "list"])
        .output()
        .expect("spawn crepus components list");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Embedded crepuscularity-components crate always has a catalog.
    assert!(
        stdout.contains("button") || stdout.contains("(no components"),
        "unexpected stdout: {stdout}"
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn components_themes_runs() {
    let output = crepus()
        .args(["components", "themes"])
        .output()
        .expect("spawn crepus components themes");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() && !stdout.contains("(no themes") {
        assert!(
            stdout.contains("zinc") || stdout.contains("dawn"),
            "unexpected themes stdout: {stdout}"
        );
    }
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn components_add_moonshine_hints_tschk_package() {
    let output = crepus()
        .args(["components", "add", "sparkline", "--target", "moonshine"])
        .output()
        .expect("spawn crepus components add");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("@tschk/moonshine-components"),
        "expected @tschk/moonshine-components hint, got: {stdout}"
    );
    assert!(
        !stdout.contains("plugins/crepuscularity-components/packages/moonshine"),
        "should not point at thin plugin package: {stdout}"
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn moonshine_dep_prints_packages() {
    let output = crepus()
        .args(["moonshine", "dep"])
        .output()
        .expect("spawn crepus moonshine dep");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("@tschk/moonshine"));
    assert!(stdout.contains("@tschk/crepus-moonshine"));
    assert!(stdout.contains("@tschk/moonshine-components"));
    assert!(stdout.contains("file:"));
    assert!(stdout.contains("git clone https://github.com/tschk/moonshine"));
    assert!(
        !stdout.contains("#path:packages/"),
        "must not use broken bun github nested-path form: {stdout}"
    );
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn moonshine_new_scaffolds_app() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // Isolate from the developer's ~/projects/moonshine so the scaffold uses
    // published npm packages without requiring a sibling checkout.
    let status = crepus()
        .current_dir(tmp.path())
        .env_remove("MOONSHINE_PATH")
        .env("HOME", tmp.path())
        .env("USERPROFILE", tmp.path())
        .args(["moonshine", "new", "Demo App", "--js"])
        .status()
        .expect("spawn crepus moonshine new");
    assert!(status.success());
    let app = tmp.path().join("demo-app");
    assert!(app.join("package.json").is_file());
    assert!(app.join("index.crepus").is_file());
    assert!(app.join("index.html").is_file());
    assert!(app.join("vite.config.ts").is_file());
    assert!(
        app.join("src/main.tsx").is_file(),
        "scaffold must write src/main.tsx"
    );
    assert!(!app.join("src/main.ts").exists());

    let pkg = std::fs::read_to_string(app.join("package.json")).expect("package.json");
    assert!(pkg.contains("@tschk/crepus-moonshine"));
    assert!(pkg.contains("@tschk/moonshine"));
    assert!(pkg.contains("@tschk/moonshine-components"));
    assert!(pkg.contains("\"react\""));
    assert!(pkg.contains("\"react-dom\""));
    assert!(pkg.contains("@vitejs/plugin-react"));
    // This test runs with an isolated HOME, so no local checkout is resolvable
    // and the scaffold must fall back to dependencies that install anywhere.
    assert!(!pkg.contains("file:"), "{pkg}");
    for name in [
        "@tschk/moonshine",
        "@tschk/crepus-moonshine",
        "@tschk/moonshine-components",
    ] {
        assert!(pkg.contains(&format!("\"{name}\": \"^0.4.49\"")), "{pkg}");
    }
    assert!(!pkg.contains("#path:packages/"));

    let main = std::fs::read_to_string(app.join("src/main.tsx")).expect("main.tsx");
    assert!(main.contains("@tschk/moonshine/react"));
    assert!(main.contains("createApp"));
    assert!(main.contains("renderCrepusIr"));
    assert!(main.contains("Sparkline"));
    assert!(main.contains("@tschk/moonshine-components"));
    assert!(!main.contains("createRoot"));

    let html = std::fs::read_to_string(app.join("index.html")).expect("index.html");
    assert!(html.contains("/src/main.tsx"));

    let vite = std::fs::read_to_string(app.join("vite.config.ts")).expect("vite");
    assert!(vite.contains("@vitejs/plugin-react"));
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn web_build_emit_moonshine_writes_entry() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let site = tmp.path().join("site");
    std::fs::create_dir_all(&site).expect("mkdir");
    std::fs::write(
        site.join("index.crepus"),
        "stack col gap-2\n text \"hi\"\n button \"Go\"\n",
    )
    .expect("write crepus");
    let out = tmp.path().join("dist");

    let status = crepus()
        .args([
            "web",
            "build",
            "--emit",
            "moonshine",
            "--site",
            site.to_str().unwrap(),
            "--out-dir",
            out.to_str().unwrap(),
        ])
        .status()
        .expect("spawn crepus web build --emit moonshine");
    assert!(
        status.success(),
        "emit moonshine should succeed without WASM"
    );
    assert!(out.join("crepus-emit.moonshine.tsx").is_file());
    assert!(!out.join("crepus-emit.moonshine.ts").exists());
    assert!(out.join("crepus-view-ir.json").is_file());
    let entry = std::fs::read_to_string(out.join("crepus-emit.moonshine.tsx")).expect("emit");
    assert!(entry.contains("@tschk/moonshine/react"));
    assert!(entry.contains("createApp"));
    assert!(entry.contains("export function App"));
    assert!(entry.contains("export function mount"));
    assert!(entry.contains("data-crepus-root=\"true\""));
    assert!(entry.contains("className=\""));
    assert!(!entry.contains("renderCrepusIr"));
    assert!(!entry.contains("satisfies ViewIr"));
}

/// Every non-HTML emit target must write its file and produce source its own
/// toolchain recognises, so `--emit` stays honest as targets are added.
#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn web_build_emits_svelte_vue_and_solid() {
    let cases = [
        ("svelte", "CrepusEmit.svelte", "$props()", "{#if"),
        ("vue", "CrepusEmit.vue", "defineProps", "v-if"),
        ("solid", "CrepusEmit.solid.tsx", "solid-js", "<Show"),
    ];

    for (target, filename, marker, control_flow) in cases {
        let tmp = tempfile::tempdir().expect("tempdir");
        let site = tmp.path().join("site");
        std::fs::create_dir_all(&site).expect("mkdir");
        std::fs::write(
            site.join("index.crepus"),
            "stack col gap-2\n if {ready}\n  text \"yes\"\n else\n  text \"no\"\n",
        )
        .expect("write crepus");
        let out = tmp.path().join("dist");

        let status = crepus()
            .args([
                "web",
                "build",
                "--emit",
                target,
                "--site",
                site.to_str().unwrap(),
                "--out-dir",
                out.to_str().unwrap(),
            ])
            .status()
            .expect("run crepus web build");
        assert!(status.success(), "{target} emit should succeed");

        let body = std::fs::read_to_string(out.join(filename))
            .unwrap_or_else(|e| panic!("{target}: read {filename}: {e}"));
        assert!(body.contains(marker), "{target} missing {marker}: {body}");
        assert!(
            body.contains(control_flow),
            "{target} missing {control_flow}: {body}"
        );
        assert!(
            !body.contains("data-crepus-if"),
            "{target} should emit real control flow: {body}"
        );
    }
}

#[test]
fn unknown_component_lists_embedded_and_disk_catalog_ids() {
    let root = tempfile::tempdir().unwrap();
    let catalog = root
        .path()
        .join("plugins/crepuscularity-components/catalog/components.json");
    std::fs::create_dir_all(catalog.parent().unwrap()).unwrap();
    std::fs::write(
        &catalog,
        r#"{"components":[{"id":"custom-component","platforms":["moonshine"]}]}"#,
    )
    .unwrap();
    let output = crepus()
        .env("CREPUS_REPO_ROOT", root.path())
        .args(["components", "add", "does-not-exist"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("custom-component"), "{stderr}");
    assert!(stderr.contains("button"), "{stderr}");
}

#[test]
fn custom_component_blank_metadata_uses_name_and_package_fallbacks() {
    let root = tempfile::tempdir().unwrap();
    let catalog = root
        .path()
        .join("plugins/crepuscularity-components/catalog/components.json");
    std::fs::create_dir_all(catalog.parent().unwrap()).unwrap();
    std::fs::write(&catalog, r#"{"components":[{"id":"custom-component","title":" ","name":"Custom Component","platforms":["moonshine"],"targets":{"moonshine":" "}}]}"#).unwrap();
    let output = crepus()
        .env("CREPUS_REPO_ROOT", root.path())
        .args([
            "components",
            "add",
            "custom-component",
            "--target",
            "moonshine",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("name:      Custom Component"), "{stdout}");
    assert!(stdout.contains("@tschk/moonshine-components"), "{stdout}");
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn moonshine_emit_resolves_bundled_includes_from_the_entry_directory() {
    let directory = tempfile::tempdir().unwrap();
    let site = directory.path().join("site");
    std::fs::create_dir_all(site.join("components")).unwrap();
    std::fs::create_dir_all(site.join("pages/components")).unwrap();
    std::fs::write(
        site.join("index.crepus"),
        "include components/card.crepus title=\"Root title\"\n  span \"Caller slot\"",
    )
    .unwrap();
    std::fs::write(
        site.join("components/card.crepus"),
        "div\n  span \"{title}\"\n  slot",
    )
    .unwrap();
    std::fs::write(
        site.join("pages/main.crepus"),
        "include components/card.crepus",
    )
    .unwrap();
    std::fs::write(
        site.join("pages/components/card.crepus"),
        "span \"Nested title\"",
    )
    .unwrap();
    for (entry, expected) in [
        ("index.crepus", "Root title"),
        ("pages/main.crepus", "Nested title"),
    ] {
        let out = directory.path().join("dist");
        let output = crepus()
            .args(["web", "build", "--emit", "moonshine", "--site"])
            .arg(&site)
            .arg("--out-dir")
            .arg(&out)
            .args(["--entry", entry])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{entry}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let ir = std::fs::read_to_string(out.join("crepus-view-ir.json")).unwrap();
        let tsx = std::fs::read_to_string(out.join("crepus-emit.moonshine.tsx")).unwrap();
        assert!(ir.contains(expected), "{ir}");
        if entry == "index.crepus" {
            assert!(ir.contains("Caller slot"), "{ir}");
            assert!(tsx.contains("Caller slot"), "{tsx}");
        } else {
            assert!(tsx.contains(expected), "{tsx}");
        }
    }
}

#[test]
#[cfg_attr(
    windows,
    ignore = "default desktop crepus.exe does not spawn reliably on Windows CI"
)]
fn framework_emit_normalizes_relative_entry_paths() {
    let site = tempfile::tempdir().unwrap();
    std::fs::write(
        site.path().join("index.crepus"),
        "span \"Normalized entry\"",
    )
    .unwrap();
    std::fs::write(
        site.path().join("cards.crepus"),
        "--- Card\nspan \"Named entry\"",
    )
    .unwrap();
    for (entry, expected) in [
        ("./index.crepus", "Normalized entry"),
        ("pages/../index.crepus", "Normalized entry"),
        ("./cards.crepus#Card", "Named entry"),
    ] {
        let out = site.path().join("dist");
        let output = crepus()
            .args(["web", "build", "--emit", "moonshine", "--site"])
            .arg(site.path())
            .args(["--entry", entry])
            .arg("--out-dir")
            .arg(&out)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{entry}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let body = std::fs::read_to_string(out.join("crepus-emit.moonshine.tsx")).unwrap();
        assert!(body.contains(expected), "{body}");
    }
    let output = crepus()
        .args(["web", "build", "--emit", "moonshine", "--site"])
        .arg(site.path())
        .args(["--entry", "../index.crepus"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("file not found"));
}

#[test]
#[cfg(unix)]
fn moonshine_local_paths_roundtrip_in_scaffold_and_dependency_snippet() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("moonshine-\"quoted\\雪-{{slug}}");
    for directory in ["packages/core", "packages/crepus-moonshine", "components"] {
        let package = root.join(directory);
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(package.join("package.json"), "{}").unwrap();
    }
    let output = crepus()
        .current_dir(tmp.path())
        .env("MOONSHINE_PATH", &root)
        .args(["moonshine", "new", "Path App", "--js"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let package: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(tmp.path().join("path-app/package.json")).unwrap(),
    )
    .expect("scaffold must be valid JSON");
    let output = crepus()
        .current_dir(tmp.path())
        .env("MOONSHINE_PATH", &root)
        .args(["moonshine", "dep"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let printed = String::from_utf8(output.stdout).unwrap();
    let start = printed.find("{\n").expect("dependency snippet");
    let snippet = serde_json::Deserializer::from_str(&printed[start..])
        .into_iter::<serde_json::Value>()
        .next()
        .unwrap()
        .expect("snippet must be valid JSON");
    for (name, directory) in [
        ("@tschk/moonshine", "packages/core"),
        ("@tschk/crepus-moonshine", "packages/crepus-moonshine"),
        ("@tschk/moonshine-components", "components"),
    ] {
        let expected = format!(
            "file:{}",
            root.join(directory).canonicalize().unwrap().display()
        );
        assert_eq!(package["dependencies"][name], expected);
        assert_eq!(snippet["dependencies"][name], expected);
    }
}
