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

#[test]
fn duplicate_vue_block_attributes_fail_before_output_publication() {
    for (source, block, key, imported) in [
        (
            "<script setup lang=\"js\" lang=\"ts\">const props=defineProps<{label:string}>();</script><template><p>{{props.label}}</p></template>",
            "script",
            "lang",
            false,
        ),
        (
            "<script setup setup=\"false\" lang=\"ts\">const props=defineProps<{label:string}>();</script><template><p>{{props.label}}</p></template>",
            "script",
            "setup",
            false,
        ),
        (
            "<template lang=\"html\" lang=\"html\"><p>Static child</p></template>",
            "template",
            "lang",
            false,
        ),
        (
            "<script setup lang=\"ts\" lang=\"ts\">const props=defineProps<{label:string}>();</script><template><p>{{props.label}}</p></template>",
            "script",
            "lang",
            true,
        ),
    ] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            for existing in [false, true] {
                let site = tempfile::tempdir().unwrap();
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
                assert!(!result.status.success(), "{target} accepted {source}");
                assert!(
                    diagnostic.contains(&format!("duplicate Vue SFC {block} attribute")),
                    "{diagnostic}"
                );
                assert!(diagnostic.contains(key), "{diagnostic}");
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
fn duplicate_looking_attributes_in_comments_and_strings_are_not_block_attributes() {
    let source = r#"<!-- <script setup setup lang="js" lang="ts">comment</script> --><script setup lang="ts">// lang="js" lang="ts"
const example='setup setup lang="js" lang="ts"';const props=defineProps<{label:string}>();</script><template><p title='lang="html" lang="html"'>{{props.label}}</p></template>"#;
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
