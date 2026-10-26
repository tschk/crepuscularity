use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const GENERIC_COMPONENT: &str = r#"<script setup lang="ts" generic="T extends string">const props=defineProps<{label:string}>();function identity(value:T):T{return value}</script><template><p>{{props.label}}</p></template>"#;

fn build(site: &Path, target: &str, entry: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .args(["--entry", entry])
        .output()
        .unwrap()
}

#[test]
fn generic_vue_scripts_reject_entries_and_imports_without_publishing_outputs() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for imported in [false, true] {
            for existing in [false, true] {
                let site = tempfile::tempdir().unwrap();
                let entry = if imported {
                    fs::create_dir(site.path().join("parts")).unwrap();
                    fs::write(site.path().join("parts/Child.vue"), GENERIC_COMPONENT).unwrap();
                    fs::write(
                        site.path().join("Parent.tsx"),
                        "import Child from './parts/Child.vue';export default function Parent(){return <Child label='Hello'/>;}",
                    )
                    .unwrap();
                    "Parent.tsx"
                } else {
                    fs::write(site.path().join("Entry.vue"), GENERIC_COMPONENT).unwrap();
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
                assert!(!result.status.success(), "{target} accepted generic Vue");
                assert!(diagnostic.contains("generic"), "{diagnostic}");
                assert!(diagnostic.contains("unsupported"), "{diagnostic}");
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

#[test]
fn generic_looking_comments_and_strings_do_not_reject_ordinary_setup() {
    let source = r#"<!-- <script setup generic="T">comment</script> --><script setup lang="ts">// generic="T extends string"
const example='generic="T extends string"';const props=defineProps<{label:string}>();</script><template><p>{{props.label}}</p></template>"#;
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("Entry.vue"), source).unwrap();
        let result = build(site.path(), target, "Entry.vue");
        assert!(
            result.status.success(),
            "{}",
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

#[test]
fn nonnative_react_keeps_template_only_extraction_of_generic_scripts() {
    let site = tempfile::tempdir().unwrap();
    let source = GENERIC_COMPONENT.replace("{{props.label}}", "Template only");
    fs::write(site.path().join("Entry.vue"), source).unwrap();
    let result = build(site.path(), "react", "Entry.vue");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let dist = site.path().join("dist");
    assert!(dist.join("CrepusEmit.react.tsx").is_file());
    let ir = fs::read_to_string(dist.join("crepus-view-ir.json")).unwrap();
    assert!(ir.contains("Template only"), "{ir}");
    assert!(!dist.join("crepus-module-manifest.json").exists());
}
