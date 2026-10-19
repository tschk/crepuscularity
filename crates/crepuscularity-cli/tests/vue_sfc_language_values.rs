use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn component(language: &str) -> String {
    format!("<script setup {language}>const props=defineProps<{{label:string}}>();</script><template><p>{{{{props.label}}}}</p></template>")
}

fn build(site: &Path, target: &str, entry: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .args(["--entry", entry])
        .output()
        .unwrap()
}

#[test]
fn literal_unsupported_script_languages_cannot_fall_back_to_javascript() {
    for language in ["lang={ts}", "lang={}", "lang=false"] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            for imported in [false, true] {
                for existing in [false, true] {
                    let site = tempfile::tempdir().unwrap();
                    let entry = if imported {
                        fs::create_dir(site.path().join("parts")).unwrap();
                        fs::write(site.path().join("parts/Child.vue"), component(language))
                            .unwrap();
                        fs::write(
                            site.path().join("Parent.tsx"),
                            "import Child from './parts/Child.vue';export default function Parent(){return <Child label='Hello'/>;}",
                        )
                        .unwrap();
                        "Parent.tsx"
                    } else {
                        fs::write(site.path().join("Entry.vue"), component(language)).unwrap();
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
                    assert!(!result.status.success(), "{target} accepted {language}");
                    assert!(
                        diagnostic.contains("requires one JS/TS script setup block"),
                        "{diagnostic}"
                    );
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
fn supported_literal_and_bare_script_language_forms_compile() {
    for language in [
        "lang=js",
        "lang='js'",
        "lang=ts",
        "lang=\"ts\"",
        "lang",
        "lang=\"\"",
    ] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join("Entry.vue"), component(language)).unwrap();
            let result = build(site.path(), target, "Entry.vue");
            assert!(
                result.status.success(),
                "{language}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            let dist = site.path().join("dist");
            assert!(dist.join("crepus-module-manifest.json").is_file());
            assert!(dist
                .join(format!("modules/{target}/Entry.vue.ts"))
                .is_file());
            assert!(!dist.join("crepus-view-ir.json").exists());
        }
    }
}

#[test]
fn react_template_extraction_preserves_support_for_other_script_languages() {
    for language in ["lang=coffee", "lang={ts}", "lang={}", "lang=false"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(
            site.path().join("Entry.vue"),
            component(language).replace("{{props.label}}", "Template only"),
        )
        .unwrap();
        let result = build(site.path(), "react", "Entry.vue");
        assert!(
            result.status.success(),
            "{language}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let dist = site.path().join("dist");
        assert!(dist.join("CrepusEmit.react.tsx").is_file());
        let ir = fs::read_to_string(dist.join("crepus-view-ir.json")).unwrap();
        assert!(ir.contains("Template only"), "{ir}");
        assert!(!dist.join("crepus-module-manifest.json").exists());
    }
}
