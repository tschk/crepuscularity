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
    // placeholder file: paths (still valid package.json content).
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
    assert!(pkg.contains("\"@tschk/moonshine\": \"^0.3.4\""), "{pkg}");
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
