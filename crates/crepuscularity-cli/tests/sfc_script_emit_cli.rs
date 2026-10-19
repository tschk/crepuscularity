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
fn native_targets_reject_normal_vue_script_blocks_before_creating_outputs() {
    for (entry, source) in [
        (
            "Entry.vue",
            "<script>export default {};</script><template><p>Static</p></template>",
        ),
        (
            "Entry.vue",
            "<script></script><template><p>Static</p></template>",
        ),
    ] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join(entry), source).unwrap();
            let result = build(site.path(), target, entry);
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            assert!(!result.status.success(), "{target} accepted {source}");
            assert!(diagnostic.contains(entry), "{diagnostic}");
            assert!(
                diagnostic.to_ascii_lowercase().contains("script"),
                "{diagnostic}"
            );
            assert!(
                !site.path().join("dist").exists(),
                "partial outputs: {diagnostic}"
            );
        }
    }
}

#[test]
fn propsless_vue_setup_uses_authored_modules_for_dom_and_ssr() {
    for (source, has_value) in [
        ("<script setup></script><template><p>Static</p></template>", false),
        ("<script setup lang=\"ts\"></script><template><p>Static</p></template>", false),
        ("<script setup>const value=1;</script><template><p>{{value}}</p></template>", true),
        ("<script setup lang=\"ts\">const value:number=1;</script><template><p>{{value}}</p></template>", true),
    ] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join("Entry.vue"), source).unwrap();
            let result = build(site.path(), target, "Entry.vue");
            assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
            let dist = site.path().join("dist");
            let manifest = fs::read_to_string(dist.join("crepus-module-manifest.json")).unwrap();
            assert!(manifest.contains(&format!("modules/{target}/Entry.vue.ts")), "{manifest}");
            assert!(dist.join(format!("CrepusEmit.{target}.ts")).is_file());
            assert!(!dist.join("crepus-view-ir.json").exists());
            let module = fs::read_to_string(dist.join(format!("modules/{target}/Entry.vue.ts"))).unwrap();
            let compact: String = module.split_whitespace().collect();
            assert!(compact.contains("__crepusVueProps0:{}"), "{module}");
            if has_value {
                assert!(compact.contains("constvalue=1") || compact.contains("constvalue:number=1"), "{module}");
                assert!(compact.contains("(value)"), "interpolation must retain value: {module}");
            } else {
                assert!(module.contains("Static"), "{module}");
            }
        }
    }
}

#[test]
fn failed_script_preflight_preserves_preexisting_ir_manifest_and_component() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(
            site.path().join("Entry.svelte"),
            "<script>let state = $state.eager({count: 0});</script><p>Static</p>",
        )
        .unwrap();
        let dist = site.path().join("dist");
        fs::create_dir(&dist).unwrap();
        let files = [
            ("crepus-view-ir.json".to_string(), "previous IR"),
            (
                "crepus-module-manifest.json".to_string(),
                "previous manifest",
            ),
            (format!("CrepusEmit.{target}.ts"), "previous component"),
        ];
        for (filename, contents) in &files {
            fs::write(dist.join(filename), contents).unwrap();
        }
        assert!(!build(site.path(), target, "Entry.svelte").status.success());
        assert_eq!(fs::read_dir(&dist).unwrap().count(), files.len());
        for (filename, contents) in files {
            assert_eq!(fs::read_to_string(dist.join(filename)).unwrap(), contents);
        }
    }
}

#[test]
fn reachable_nested_and_named_component_includes_report_script_source_paths() {
    for (child, source) in [
        (
            "parts/child.svelte",
            "<script>let value=1;</script><p>Child</p>",
        ),
        (
            "parts/child.vue",
            "<script setup></script><template><p>Child</p></template>",
        ),
    ] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            for named in [false, true] {
                let site = tempfile::tempdir().unwrap();
                fs::create_dir(site.path().join("parts")).unwrap();
                fs::write(site.path().join(child), source).unwrap();
                fs::write(
                    site.path().join("parts/outer.crepus"),
                    format!("include {}", child.strip_prefix("parts/").unwrap()),
                )
                .unwrap();
                let entry = if named {
                    "main.crepus#Card"
                } else {
                    "main.crepus"
                };
                let main = if named {
                    "--- Card\ninclude parts/outer.crepus\n--- Other\ntext \"Unused\""
                } else {
                    "include parts/outer.crepus"
                };
                fs::write(site.path().join("main.crepus"), main).unwrap();
                let result = build(site.path(), target, entry);
                let diagnostic = String::from_utf8_lossy(&result.stderr);
                assert!(
                    !result.status.success(),
                    "{target} accepted {entry}/{child}"
                );
                assert!(diagnostic.contains(child), "{diagnostic}");
                assert!(!site.path().join("dist").exists());
            }
        }
    }
}

#[test]
fn unused_scripts_and_script_looking_comments_do_not_block_static_native_sfc() {
    for (entry, source) in [
        ("Entry.svelte", "<!-- <script>not executable</script> --><p title='<script>literal</script>'>Static</p>"),
        ("Entry.vue", "<!-- <script>not executable</script> --><template><p title='<script>literal</script>'>Static</p></template>"),
    ] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join(entry), source).unwrap();
            fs::write(site.path().join("unused.svelte"), "<script>let value=1;</script><p>Unused</p>").unwrap();
            let result = build(site.path(), target, entry);
            assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
            assert!(site.path().join(format!("dist/CrepusEmit.{target}.ts")).is_file());
            assert_eq!(site.path().join("dist/crepus-view-ir.json").is_file(), entry.ends_with(".vue"));
        }
    }
}

#[test]
fn react_template_only_sfc_script_extraction_remains_compatible() {
    for (entry, source) in [
        ("Entry.svelte", "<script>let value=1;</script><p>Static</p>"),
        (
            "Entry.vue",
            "<script setup>const value=1;</script><template><p>Static</p></template>",
        ),
    ] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join(entry), source).unwrap();
        let result = build(site.path(), "react", entry);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(site.path().join("dist/CrepusEmit.react.tsx").is_file());
    }
}

#[test]
fn static_vue_entries_keep_template_ir_route_without_an_authored_manifest() {
    for source in [
        "<template><p>Static Vue</p></template>",
        "<!-- <style scoped>p{color:red}</style> --><template><p title='<style>literal</style>'>Static Vue</p></template>",
        "<!-- <script setup>comment only</script> --><template><p title='<script>literal</script>'>Static Vue</p></template>",
    ] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join("Entry.vue"), source).unwrap();
            let result = build(site.path(), target, "Entry.vue");
            assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
            let dist = site.path().join("dist");
            assert!(dist.join(format!("CrepusEmit.{target}.ts")).is_file());
            assert!(dist.join("crepus-view-ir.json").is_file());
            assert!(!dist.join("crepus-module-manifest.json").exists());
            let ir = fs::read_to_string(dist.join("crepus-view-ir.json")).unwrap();
            assert!(ir.contains("Static Vue"), "{ir}");
        }
    }
}

#[test]
fn typed_scripted_vue_entry_keeps_the_authored_module_route() {
    let source = "<script setup lang=\"ts\">const props = defineProps<{ label: string }>();</script><template><p>{{ props.label }}</p></template>";
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
        assert!(dist.join(format!("CrepusEmit.{target}.ts")).is_file());
        assert!(dist.join("crepus-module-manifest.json").is_file());
        assert!(!dist.join("crepus-view-ir.json").exists());
        let manifest = fs::read_to_string(dist.join("crepus-module-manifest.json")).unwrap();
        assert!(manifest.contains("Entry.vue"), "{manifest}");
    }
}

#[test]
fn authored_graphs_compile_scriptless_vue_children_without_switching_to_ir() {
    for (entry, source) in [
        ("Parent.tsx", "import Child from './Child.vue';export default function Parent(){return <Child/>;}"),
        ("Parent.vue", "<script setup lang=\"ts\">import Child from './Child.vue';const props=defineProps<{label?:string}>();</script><template><p>{{props.label}}</p></template>"),
    ] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join(entry), source).unwrap();
            fs::write(site.path().join("Child.vue"), "<template><p>Static child</p></template>").unwrap();
            let result = build(site.path(), target, entry);
            assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
            let dist = site.path().join("dist");
            assert!(!dist.join("crepus-view-ir.json").exists());
            let manifest = fs::read_to_string(dist.join("crepus-module-manifest.json")).unwrap();
            assert!(manifest.contains(&format!("modules/{target}/Child.vue.ts")), "{manifest}");
            let child = fs::read_to_string(dist.join(format!("modules/{target}/Child.vue.ts"))).unwrap();
            let host = if target == "moonshine-dom" { "@tschk/moonshine-dom/dom" } else { "@tschk/moonshine-dom/ssr" };
            assert!(child.contains(host), "{child}");
            assert!(!child.contains("defineProps"), "{child}");
            assert!(!child.contains("script setup"), "{child}");
            let parent_key = if entry.ends_with(".vue") { "Parent.vue.ts" } else { entry };
            let parent = fs::read_to_string(dist.join(format!("modules/{target}/{parent_key}"))).unwrap();
            assert!(parent.contains("./Child.vue.ts"), "{parent}");
        }
    }
}

#[test]
fn dynamic_vue_host_import_rejection_preserves_existing_outputs() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("Entry.vue"), "<script setup lang=\"ts\">const props=defineProps<{label:string}>();const load=()=>import('vue');</script><template><p>{{props.label}}</p></template>").unwrap();
        let dist = site.path().join("dist");
        fs::create_dir(&dist).unwrap();
        let output = dist.join("crepus-module-manifest.json");
        fs::write(&output, "existing manifest").unwrap();
        let result = build(site.path(), target, "Entry.vue");
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("dynamic Vue host imports"));
        assert_eq!(fs::read_to_string(output).unwrap(), "existing manifest");
        assert_eq!(fs::read_dir(dist).unwrap().count(), 1);
    }
}

#[test]
fn excessive_acyclic_vue_type_import_depth_fails_before_output_publication() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for previous_output in [false, true] {
            let site = tempfile::tempdir().unwrap();
            let root = "<script setup lang=\"ts\">import type {Props} from './level0';const props=defineProps<Props>();</script><template><p>{{props.label}}</p></template>";
            fs::write(site.path().join("Root.vue"), root).unwrap();
            let mut modules = Vec::new();
            for level in 0..65 {
                let source = if level == 64 {
                    "export interface Props {label:string}".to_string()
                } else {
                    format!("export type {{Props}} from './level{}';", level + 1)
                };
                let path = site.path().join(format!("level{level}.ts"));
                fs::write(&path, &source).unwrap();
                modules.push((path, source));
            }
            let dist = site.path().join("dist");
            if previous_output {
                fs::create_dir(&dist).unwrap();
                fs::write(
                    dist.join("crepus-module-manifest.json"),
                    "previous manifest",
                )
                .unwrap();
            }
            let result = build(site.path(), target, "Root.vue");
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            assert!(!result.status.success(), "excessive type depth accepted");
            assert!(
                diagnostic.contains("props type resolution exceeds 64"),
                "{diagnostic}"
            );
            if previous_output {
                assert_eq!(
                    fs::read_to_string(dist.join("crepus-module-manifest.json")).unwrap(),
                    "previous manifest"
                );
                assert_eq!(fs::read_dir(&dist).unwrap().count(), 1);
            } else {
                assert!(!dist.exists());
            }
            assert_eq!(
                fs::read_to_string(site.path().join("Root.vue")).unwrap(),
                root
            );
            for (path, source) in modules {
                assert_eq!(fs::read_to_string(path).unwrap(), source);
            }
        }
    }
}

#[test]
fn vue_parent_component_imports_keep_scripted_vue_and_named_tsx_modules() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("Parent.vue"), "<script setup lang=\"ts\">import Child from './Child.vue';import {Child as Alias} from './helper.tsx';const props=defineProps<{label?:string}>();</script><template><Child :label=\"props.label\"/><Alias label=\"Static\"/></template>").unwrap();
        fs::write(site.path().join("Child.vue"), "<script setup lang=\"ts\">const props=defineProps<{label?:string}>();</script><template><p>{{props.label}}</p></template>").unwrap();
        fs::write(
            site.path().join("helper.tsx"),
            "export function Child(props:{label:string}){return <b>{props.label}</b>;}",
        )
        .unwrap();
        let result = build(site.path(), target, "Parent.vue");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let dist = site.path().join("dist");
        assert!(!dist.join("crepus-view-ir.json").exists());
        let manifest = fs::read_to_string(dist.join("crepus-module-manifest.json")).unwrap();
        for module in ["Parent.vue.ts", "Child.vue.ts", "helper.tsx"] {
            assert!(
                manifest.contains(&format!("modules/{target}/{module}")),
                "{manifest}"
            );
            assert!(dist.join(format!("modules/{target}/{module}")).is_file());
        }
        let parent =
            fs::read_to_string(dist.join(format!("modules/{target}/Parent.vue.ts"))).unwrap();
        assert!(parent.contains("./Child.vue.ts"), "{parent}");
        assert!(parent.contains("./helper.tsx"), "{parent}");
    }
}
