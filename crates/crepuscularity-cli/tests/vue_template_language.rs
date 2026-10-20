use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn build(site: &Path, target: &str, entry: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .args(["--entry", entry])
        .output()
        .unwrap()
}

fn assert_rejected_without_writes(site: &Path, target: &str, entry: &str, existing: bool) {
    let dist = site.join("dist");
    let sentinel = dist.join("previous-output.txt");
    if existing {
        fs::create_dir(&dist).unwrap();
        fs::write(&sentinel, b"previous output\0unchanged").unwrap();
    }
    let result = build(site, target, entry);
    let diagnostic = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "{target} accepted pug template");
    assert!(diagnostic.contains("template language"), "{diagnostic}");
    assert!(diagnostic.contains("unsupported"), "{diagnostic}");
    if existing {
        assert_eq!(fs::read(sentinel).unwrap(), b"previous output\0unchanged");
        assert_eq!(fs::read_dir(dist).unwrap().count(), 1);
    } else {
        assert!(!dist.exists(), "partial outputs: {diagnostic}");
    }
}

#[test]
fn unsupported_vue_template_languages_fail_before_output_publication() {
    for target in ["moonshine-dom", "moonshine-ssr", "react"] {
        for existing in [false, true] {
            let site = tempfile::tempdir().unwrap();
            fs::write(
                site.path().join("Entry.vue"),
                "<template lang=\"pug\">p hello</template>",
            )
            .unwrap();
            assert_rejected_without_writes(site.path(), target, "Entry.vue", existing);
        }
    }
}

#[test]
fn imported_vue_template_preprocessors_are_rejected_before_graph_writes() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for existing in [false, true] {
            let site = tempfile::tempdir().unwrap();
            fs::create_dir(site.path().join("parts")).unwrap();
            fs::write(
                site.path().join("Parent.tsx"),
                "import Child from './parts/Child.vue';export default function Parent(){return <Child/>;}",
            )
            .unwrap();
            fs::write(
                site.path().join("parts/Child.vue"),
                "<template lang=\"pug\">p hello</template>",
            )
            .unwrap();
            assert_rejected_without_writes(site.path(), target, "Parent.tsx", existing);
        }
    }
}

#[test]
fn explicit_html_vue_templates_keep_real_text_on_the_ir_route() {
    for target in ["moonshine-dom", "moonshine-ssr", "react"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(
            site.path().join("Entry.vue"),
            "<template lang=\"html\"><p>Hello HTML</p></template>",
        )
        .unwrap();
        let result = build(site.path(), target, "Entry.vue");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let dist = site.path().join("dist");
        let ir: serde_json::Value =
            serde_json::from_slice(&fs::read(dist.join("crepus-view-ir.json")).unwrap()).unwrap();
        assert!(ir.to_string().contains("Hello HTML"), "{ir}");
        let extension = if target == "react" { "tsx" } else { "ts" };
        assert!(dist
            .join(format!("CrepusEmit.{target}.{extension}"))
            .is_file());
        assert!(!dist.join("crepus-module-manifest.json").exists());
    }
}
