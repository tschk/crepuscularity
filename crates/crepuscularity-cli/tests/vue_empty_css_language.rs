use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn build(site: &Path, target: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .args(["--entry", "Entry.vue"])
        .output()
        .unwrap()
}

fn source(attributes: &str, css: &str) -> String {
    format!("<template><p>Plain CSS</p></template><style {attributes}>{css}</style>")
}

#[test]
fn empty_style_language_has_identical_module_and_asset_output_to_plain_css() {
    let css = "p { color: rgb(10, 20, 30); }\n";
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let mut baseline = None;
        for attributes in ["", "lang", "lang=\"css\"", "lang=\"\""] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join("Entry.vue"), source(attributes, css)).unwrap();
            let result = build(site.path(), target);
            assert!(
                result.status.success(),
                "{attributes}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            let dist = site.path().join("dist");
            let manifest: serde_json::Value = serde_json::from_slice(
                &fs::read(dist.join("crepus-module-manifest.json")).unwrap(),
            )
            .unwrap();
            let assets = manifest["assets"].as_array().unwrap();
            assert_eq!(assets.len(), 1, "{manifest}");
            let asset_path = assets[0].as_str().unwrap();
            assert!(asset_path.ends_with(".css"), "{asset_path}");
            let asset = fs::read(dist.join(asset_path)).unwrap();
            assert_eq!(asset, css.as_bytes());
            let module = fs::read(dist.join(format!("modules/{target}/Entry.vue.ts"))).unwrap();
            let actual = (module, asset, asset_path.to_string());
            if let Some(expected) = &baseline {
                assert_eq!(&actual, expected, "{target} {attributes}");
            } else {
                baseline = Some(actual);
            }
        }
    }
}

#[test]
fn empty_css_body_with_empty_language_does_not_publish_assets() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("Entry.vue"), source("lang=\"\"", "")).unwrap();
        let result = build(site.path(), target);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let dist = site.path().join("dist");
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(dist.join("crepus-module-manifest.json")).unwrap())
                .unwrap();
        assert!(
            manifest
                .get("assets")
                .is_none_or(|assets| assets.as_array().unwrap().is_empty()),
            "{manifest}"
        );
        assert!(
            manifest["files"]
                .as_array()
                .unwrap()
                .iter()
                .all(|file| !file.as_str().unwrap().contains(".css")),
            "{manifest}"
        );
    }
}

#[test]
fn empty_language_does_not_bypass_other_style_restrictions() {
    for (attributes, css) in [
        ("lang=\"\" scoped", "p{color:red}"),
        ("lang=\"\" module", "p{color:red}"),
        (
            "lang=\"\"",
            "p{background-image:image-set(\"relative.png\" 1x)}",
        ),
    ] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join("Entry.vue"), source(attributes, css)).unwrap();
            let dist = site.path().join("dist");
            fs::create_dir(&dist).unwrap();
            let sentinel = dist.join("previous-output.txt");
            fs::write(&sentinel, b"previous output\0unchanged").unwrap();
            let result = build(site.path(), target);
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            assert!(!result.status.success(), "accepted {attributes}: {css}");
            assert!(
                diagnostic.contains("unsupported") || diagnostic.contains("does not yet support"),
                "{diagnostic}"
            );
            assert_eq!(fs::read(sentinel).unwrap(), b"previous output\0unchanged");
            assert_eq!(fs::read_dir(dist).unwrap().count(), 1);
        }
    }
}
