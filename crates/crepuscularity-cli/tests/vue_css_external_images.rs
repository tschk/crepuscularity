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

fn component(css: &str) -> String {
    format!("<script setup lang=\"ts\">const props=defineProps<{{label:string}}>();</script><template><p>{{{{props.label}}}}</p></template><style>{css}</style>")
}

#[test]
fn image_set_urls_are_rejected_before_entry_or_import_outputs_change() {
    for function in ["image-set", "-webkit-image-set", "IMAGE-SET"] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            for imported in [false, true] {
                for existing in [false, true] {
                    let site = tempfile::tempdir().unwrap();
                    let source = component(&format!(
                        "p{{background-image:{function}(\"relative.png\" 1x)}}"
                    ));
                    let entry = if imported {
                        fs::create_dir(site.path().join("parts")).unwrap();
                        fs::write(site.path().join("parts/Child.vue"), source).unwrap();
                        fs::write(
                            site.path().join("Parent.tsx"),
                            "import Child from './parts/Child.vue';export default function Parent(){return <Child label='Hello'/>;}",
                        )
                        .unwrap();
                        "Parent.tsx"
                    } else {
                        fs::write(site.path().join("Entry.vue"), source).unwrap();
                        "Entry.vue"
                    };
                    let dist = site.path().join("dist");
                    let sentinel = dist.join("previous-output.txt");
                    if existing {
                        fs::create_dir(&dist).unwrap();
                        fs::write(&sentinel, b"previous output\0unchanged").unwrap();
                    }

                    let result = build(site.path(), target, entry);
                    let diagnostic = String::from_utf8_lossy(&result.stderr);
                    assert!(!result.status.success(), "{target} accepted {function}");
                    assert!(diagnostic.contains("URLs"), "{diagnostic}");
                    if imported {
                        assert!(diagnostic.contains("Child.vue"), "{diagnostic}");
                    }
                    if existing {
                        assert_eq!(fs::read(sentinel).unwrap(), b"previous output\0unchanged");
                        assert_eq!(fs::read_dir(dist).unwrap().count(), 1);
                    } else {
                        assert!(!dist.exists(), "partial outputs: {diagnostic}");
                    }
                }
            }
        }
    }
}

#[test]
fn plain_supported_css_is_inventoried_with_unchanged_source_bytes() {
    let css = "p { color: rgb(10, 20, 30); background: linear-gradient(red, blue); }\n";
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("Entry.vue"), component(css)).unwrap();
        let result = build(site.path(), target, "Entry.vue");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let dist = site.path().join("dist");
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(dist.join("crepus-module-manifest.json")).unwrap())
                .unwrap();
        let styles: Vec<_> = manifest["assets"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|file| file.as_str())
            .filter(|file| file.ends_with(".css"))
            .collect();
        assert_eq!(styles.len(), 1, "{manifest}");
        assert_eq!(fs::read(dist.join(styles[0])).unwrap(), css.as_bytes());
        assert!(!dist.join("crepus-view-ir.json").exists());
    }
}
