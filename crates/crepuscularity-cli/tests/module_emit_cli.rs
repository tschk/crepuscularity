use std::fs;
use std::process::Command;

fn build(site: &std::path::Path, target: &str, entry: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .args(["--entry", entry])
        .output()
        .unwrap()
}

#[test]
fn authored_module_graph_preserves_exports_imports_and_manifest_without_ir() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::create_dir_all(site.path().join("nested/helpers")).unwrap();
        fs::write(site.path().join("nested/Counter.tsx"), "import { createSignal } from '@tschk/moonshine-dom';\nimport { label } from './helpers/label';\nimport type { Props } from './types';\nexport const lazy = () => import('./helpers/lazy');\nexport { extra } from './helpers/extra.js';\nexport default function Counter(props: Props) { const count = createSignal(0); return <button onClick={() => count.set(count() + 1)}>{label}{count()}</button>; }\n").unwrap();
        fs::write(
            site.path().join("nested/helpers/label.ts"),
            "export const label = 'Count';\nexport { extra } from './extra.js';",
        )
        .unwrap();
        fs::write(
            site.path().join("nested/helpers/extra.js"),
            "export const extra = 1;\nexport { label } from './label';",
        )
        .unwrap();
        fs::write(
            site.path().join("nested/types.ts"),
            "export interface Props { title?: string }",
        )
        .unwrap();
        fs::write(
            site.path().join("nested/helpers/lazy.ts"),
            "export const lazyValue = 3;",
        )
        .unwrap();
        fs::write(
            site.path().join("unrelated.ts"),
            "this must never be copied",
        )
        .unwrap();
        let result = build(site.path(), target, "./nested/Counter.tsx");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let dist = site.path().join("dist");
        let facade_name = format!("CrepusEmit.{target}.ts");
        let facade = fs::read_to_string(dist.join(&facade_name)).unwrap();
        assert!(
            facade.contains(&format!(
                "export * from \"./modules/{target}/nested/Counter.tsx\""
            )),
            "{facade}"
        );
        assert!(facade.contains("export { default }"), "{facade}");
        let generated =
            fs::read_to_string(dist.join(format!("modules/{target}/nested/Counter.tsx"))).unwrap();
        assert!(generated.contains("createSignal"), "{generated}");
        assert!(generated.contains("./helpers/label"), "{generated}");
        assert!(!generated.contains("<button"), "{generated}");
        assert!(!dist.join("crepus-view-ir.json").exists());
        assert!(!dist.join(format!("modules/{target}/unrelated.ts")).exists());
        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(dist.join("crepus-module-manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["version"], 1);
        assert_eq!(manifest["target"], target);
        assert_eq!(manifest["entry"], facade_name);
        assert_eq!(manifest["sourceEntry"], "nested/Counter.tsx");
        let files: Vec<_> = manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|path| path.as_str().unwrap())
            .collect();
        let mut sorted = files.clone();
        sorted.sort();
        assert_eq!(files, sorted);
        for path in [
            facade_name.clone(),
            format!("modules/{target}/nested/Counter.tsx"),
            format!("modules/{target}/nested/helpers/label.ts"),
            format!("modules/{target}/nested/helpers/extra.js"),
            format!("modules/{target}/nested/types.ts"),
            format!("modules/{target}/nested/helpers/lazy.ts"),
        ] {
            assert!(files.contains(&path.as_str()), "missing {path}: {files:?}");
            assert!(dist.join(path).is_file());
        }
        assert!(files
            .iter()
            .all(|path| !path.starts_with('/') && !path.contains("..")));
    }
}

#[test]
fn named_only_modules_and_plain_jsx_templates_keep_distinct_outputs() {
    let site = tempfile::tempdir().unwrap();
    fs::write(
        site.path().join("Named.jsx"),
        "export function Named() { return <p>Named</p>; }",
    )
    .unwrap();
    let result = build(site.path(), "moonshine-dom", "Named.jsx");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let facade = fs::read_to_string(site.path().join("dist/CrepusEmit.moonshine-dom.ts")).unwrap();
    assert!(!facade.contains("export { default }"));
    let site = tempfile::tempdir().unwrap();
    fs::write(site.path().join("Plain.jsx"), "<p>Plain</p>").unwrap();
    let result = build(site.path(), "moonshine-dom", "Plain.jsx");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(site.path().join("dist/crepus-view-ir.json").is_file());
    assert!(!site
        .path()
        .join("dist/crepus-module-manifest.json")
        .exists());
}

#[test]
fn invalid_module_graphs_fail_before_writing_outputs() {
    for source in [
        "import './missing'; export default <p/>;",
        "import './asset.css'; export default <p/>;",
        "import '../outside'; export default <p/>;",
        "export default function Broken( { return <p/>; }",
        "export const value = import('./' + name);",
    ] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("Entry.tsx"), source).unwrap();
        let result = build(site.path(), "moonshine-dom", "Entry.tsx");
        assert!(!result.status.success(), "accepted {source}");
        assert!(
            !site.path().join("dist").exists(),
            "partial outputs for {source}"
        );
    }
    let site = tempfile::tempdir().unwrap();
    fs::write(
        site.path().join("Entry.tsx"),
        "import './helper'; export default <p/>;",
    )
    .unwrap();
    fs::write(site.path().join("helper.ts"), "export const a = 1;").unwrap();
    fs::write(site.path().join("helper.js"), "export const a = 1;").unwrap();
    assert!(!build(site.path(), "moonshine-dom", "Entry.tsx")
        .status
        .success());
    assert!(!site.path().join("dist").exists());
}

#[test]
fn solid_helper_primitives_are_bridged_and_unsupported_helpers_preserve_outputs() {
    for (target, host) in [
        ("moonshine-dom", "@tschk/moonshine-dom/reactive"),
        ("moonshine-ssr", "@tschk/moonshine-dom/ssr"),
    ] {
        let site = tempfile::tempdir().unwrap();
        fs::write(
            site.path().join("Entry.tsx"),
            "import {state} from './helper'; export default () => {const [n,setN]=state(0); return <button onClick={()=>setN(n()+1)}>{n()}</button>};",
        ).unwrap();
        fs::write(
            site.path().join("helper.ts"),
            "export {createSignal as state} from 'solid-js';",
        )
        .unwrap();
        let result = build(site.path(), target, "Entry.tsx");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let generated = site.path().join(format!("dist/modules/{target}/helper.ts"));
        let previous = fs::read_to_string(&generated).unwrap();
        assert!(previous.contains(host), "{previous}");
        assert!(!previous.contains("solid-js"));
        let manifest = site.path().join("dist/crepus-module-manifest.json");
        let previous_manifest = fs::read(&manifest).unwrap();
        fs::write(
            site.path().join("helper.ts"),
            "export {createStore as state} from 'solid-js/store';",
        )
        .unwrap();
        let result = build(site.path(), target, "Entry.tsx");
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("solid-js subpaths"));
        assert_eq!(fs::read_to_string(generated).unwrap(), previous);
        assert_eq!(fs::read(manifest).unwrap(), previous_manifest);
    }
}

#[test]
fn transitive_react_runtime_imports_preserve_the_previous_native_graph() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        let entry = site.path().join("Entry.tsx");
        let helper = site.path().join("helper.ts");
        fs::write(
            &entry,
            "import {state} from './helper'; export default () => <p>{state}</p>;",
        )
        .unwrap();
        fs::write(&helper, "export const state = 1;").unwrap();
        let result = build(site.path(), target, "Entry.tsx");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let previous: Vec<_> = [
            format!("CrepusEmit.{target}.ts"),
            "crepus-module-manifest.json".into(),
            format!("modules/{target}/Entry.tsx"),
            format!("modules/{target}/helper.ts"),
        ]
        .into_iter()
        .map(|path| {
            let path = site.path().join("dist").join(path);
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
        fs::write(
            &entry,
            "import {state} from './helper'; export default () => <p title='changed'>{state}</p>;",
        )
        .unwrap();
        fs::write(
            &helper,
            "import './new'; export {useState as state} from 'react';",
        )
        .unwrap();
        fs::write(site.path().join("new.ts"), "export const fresh = true;").unwrap();
        let result = build(site.path(), target, "Entry.tsx");
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("React"));
        for (path, bytes) in previous {
            assert_eq!(fs::read(&path).unwrap(), bytes, "{}", path.display());
        }
        assert!(!site
            .path()
            .join(format!("dist/modules/{target}/new.ts"))
            .exists());
    }
}

#[cfg(unix)]
#[test]
fn source_symlink_escape_and_output_source_overlap_are_rejected() {
    use std::os::unix::fs::symlink;
    let site = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("helper.ts"), "export const value = 1;").unwrap();
    symlink(
        outside.path().join("helper.ts"),
        site.path().join("helper.ts"),
    )
    .unwrap();
    fs::write(
        site.path().join("Entry.tsx"),
        "import './helper'; export default <p/>;",
    )
    .unwrap();
    assert!(!build(site.path(), "moonshine-dom", "Entry.tsx")
        .status
        .success());
    fs::write(site.path().join("Entry.tsx"), "export default <p/>;").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", "moonshine-dom", "--site"])
        .arg(site.path())
        .args(["--entry", "Entry.tsx", "--out-dir"])
        .arg(site.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(
        fs::read_to_string(site.path().join("Entry.tsx")).unwrap(),
        "export default <p/>;"
    );
}

#[test]
fn dom_and_ssr_graphs_coexist_in_one_output_directory() {
    let site = tempfile::tempdir().unwrap();
    fs::write(
        site.path().join("Counter.tsx"),
        "export default function Counter() { return <p>Shared</p>; }",
    )
    .unwrap();
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let result = build(site.path(), target, "Counter.tsx");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    for (target, host) in [("moonshine-dom", "/dom"), ("moonshine-ssr", "/ssr")] {
        let dist = site.path().join("dist");
        let facade = fs::read_to_string(dist.join(format!("CrepusEmit.{target}.ts"))).unwrap();
        assert!(
            facade.contains(&format!("./modules/{target}/Counter.tsx")),
            "{facade}"
        );
        let module =
            fs::read_to_string(dist.join(format!("modules/{target}/Counter.tsx"))).unwrap();
        assert!(module.contains(host), "{module}");
    }
}

#[cfg(unix)]
#[test]
fn internal_source_symlinks_and_output_symlink_escapes_fail_before_writes() {
    use std::os::unix::fs::symlink;
    let site = tempfile::tempdir().unwrap();
    fs::write(site.path().join("real.ts"), "export const value = 1;").unwrap();
    symlink(site.path().join("real.ts"), site.path().join("alias.ts")).unwrap();
    fs::write(
        site.path().join("Entry.tsx"),
        "import './alias'; export default <p/>;",
    )
    .unwrap();
    assert!(!build(site.path(), "moonshine-dom", "Entry.tsx")
        .status
        .success());
    assert!(!site.path().join("dist").exists());
    fs::write(site.path().join("Entry.tsx"), "export default <p/>;").unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir_all(site.path().join("dist/modules")).unwrap();
    symlink(
        outside.path(),
        site.path().join("dist/modules/moonshine-dom"),
    )
    .unwrap();
    assert!(!build(site.path(), "moonshine-dom", "Entry.tsx")
        .status
        .success());
    assert!(!site
        .path()
        .join("dist/CrepusEmit.moonshine-dom.ts")
        .exists());
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
}

#[cfg(unix)]
#[test]
fn internal_output_symlink_aliases_fail_before_overwriting_either_destination() {
    use std::os::unix::fs::symlink;

    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        let source = "export default function App(){return <p>App</p>;}";
        fs::write(site.path().join("App.tsx"), source).unwrap();
        let dist = site.path().join("dist");
        let module_dir = dist.join("modules").join(target);
        fs::create_dir_all(&module_dir).unwrap();
        let module = module_dir.join("App.tsx");
        fs::write(&module, "previous module").unwrap();
        let facade = dist.join(format!("CrepusEmit.{target}.ts"));
        let link_target = std::path::PathBuf::from("modules")
            .join(target)
            .join("App.tsx");
        symlink(&link_target, &facade).unwrap();
        let preserved = [
            ("crepus-module-manifest.json", "previous manifest"),
            ("crepus-view-ir.json", "previous IR"),
            ("unrelated.txt", "unrelated output"),
        ];
        for (name, contents) in preserved {
            fs::write(dist.join(name), contents).unwrap();
        }
        let before_entries = fs::read_dir(&dist).unwrap().count();
        let result = build(site.path(), target, "App.tsx");
        let diagnostic = String::from_utf8_lossy(&result.stderr);
        assert!(
            !result.status.success(),
            "aliased outputs accepted: {target}"
        );
        assert!(
            diagnostic.contains("resolve to the same destination"),
            "{diagnostic}"
        );
        assert_eq!(fs::read_to_string(&module).unwrap(), "previous module");
        assert_eq!(fs::read_to_string(&facade).unwrap(), "previous module");
        assert_eq!(fs::read_link(&facade).unwrap(), link_target);
        for (name, contents) in preserved {
            assert_eq!(fs::read_to_string(dist.join(name)).unwrap(), contents);
        }
        assert_eq!(fs::read_dir(&dist).unwrap().count(), before_entries);
        assert_eq!(fs::read_dir(&module_dir).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(site.path().join("App.tsx")).unwrap(),
            source
        );
    }
}

#[cfg(unix)]
#[test]
fn dangling_authored_output_symlinks_cannot_create_outside_files() {
    use std::os::unix::fs::symlink;

    for target in ["moonshine-dom", "moonshine-ssr"] {
        for existing_manifest in [false, true] {
            let site = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            fs::write(
                site.path().join("App.tsx"),
                "export default () => <p>App</p>;",
            )
            .unwrap();
            let dist = site.path().join("dist");
            fs::create_dir(&dist).unwrap();
            let external_target = outside.path().join("missing.ts");
            let facade = dist.join(format!("CrepusEmit.{target}.ts"));
            symlink(&external_target, &facade).unwrap();
            let manifest = dist.join("crepus-module-manifest.json");
            if existing_manifest {
                fs::write(&manifest, "previous manifest").unwrap();
            }
            let count = fs::read_dir(&dist).unwrap().count();
            let result = build(site.path(), target, "App.tsx");
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            assert!(
                !result.status.success(),
                "dangling output accepted: {target}"
            );
            assert!(diagnostic.contains("symlink"), "{diagnostic}");
            assert!(!external_target.exists());
            assert_eq!(fs::read_link(&facade).unwrap(), external_target);
            assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
            assert_eq!(fs::read_dir(&dist).unwrap().count(), count);
            assert!(!dist.join("modules").exists());
            if existing_manifest {
                assert_eq!(fs::read_to_string(&manifest).unwrap(), "previous manifest");
            } else {
                assert!(!manifest.exists());
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn dangling_react_output_symlink_fails_before_ir_publication() {
    use std::os::unix::fs::symlink;

    let site = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(site.path().join("Entry.crepus"), "text \"Static\"").unwrap();
    let dist = site.path().join("dist");
    fs::create_dir(&dist).unwrap();
    let external_target = outside.path().join("missing.tsx");
    let component = dist.join("CrepusEmit.react.tsx");
    symlink(&external_target, &component).unwrap();
    let result = build(site.path(), "react", "Entry.crepus");
    let diagnostic = String::from_utf8_lossy(&result.stderr);
    assert!(!result.status.success(), "dangling React output accepted");
    assert!(diagnostic.contains("symlink"), "{diagnostic}");
    assert!(!external_target.exists());
    assert!(!dist.join("crepus-view-ir.json").exists());
    assert_eq!(fs::read_link(&component).unwrap(), external_target);
    assert_eq!(fs::read_dir(&dist).unwrap().count(), 1);
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn authored_facade_sockets_fail_before_graph_or_manifest_writes() {
    use std::os::unix::{fs::FileTypeExt, net::UnixListener};

    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::Builder::new()
            .prefix("s")
            .tempdir_in("/tmp")
            .unwrap();
        fs::write(
            site.path().join("App.tsx"),
            "export default () => <p>App</p>;",
        )
        .unwrap();
        let dist = site.path().join("dist");
        fs::create_dir(&dist).unwrap();
        fs::write(
            dist.join("crepus-module-manifest.json"),
            "previous manifest",
        )
        .unwrap();
        fs::write(dist.join("unrelated.txt"), "unrelated").unwrap();
        let socket = dist.join(format!("CrepusEmit.{target}.ts"));
        let _listener = UnixListener::bind(&socket).unwrap();
        let count = fs::read_dir(&dist).unwrap().count();
        let result = build(site.path(), target, "App.tsx");
        let diagnostic = String::from_utf8_lossy(&result.stderr);
        assert!(!result.status.success());
        assert!(diagnostic.contains("not a regular file"), "{diagnostic}");
        assert!(fs::symlink_metadata(&socket)
            .unwrap()
            .file_type()
            .is_socket());
        assert_eq!(
            fs::read_to_string(dist.join("crepus-module-manifest.json")).unwrap(),
            "previous manifest"
        );
        assert_eq!(
            fs::read_to_string(dist.join("unrelated.txt")).unwrap(),
            "unrelated"
        );
        assert!(!dist.join("modules").exists());
        assert_eq!(fs::read_dir(&dist).unwrap().count(), count);
    }
}

#[cfg(unix)]
#[test]
fn non_regular_authored_entry_sources_fail_before_output_changes() {
    use std::os::unix::{fs::FileTypeExt, net::UnixListener};

    for entry in ["App.tsx", "Entry.vue", "Entry.svelte"] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            let site = tempfile::Builder::new()
                .prefix("s")
                .tempdir_in("/tmp")
                .unwrap();
            let source = site.path().join(entry);
            let _listener = UnixListener::bind(&source).unwrap();
            let dist = site.path().join("dist");
            fs::create_dir(&dist).unwrap();
            fs::write(
                dist.join("crepus-module-manifest.json"),
                "previous manifest",
            )
            .unwrap();
            let result = build(site.path(), target, entry);
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            assert!(!result.status.success());
            assert!(
                diagnostic.contains("source is not a regular file"),
                "{entry}: {diagnostic}"
            );
            assert!(fs::symlink_metadata(&source)
                .unwrap()
                .file_type()
                .is_socket());
            assert_eq!(
                fs::read_to_string(dist.join("crepus-module-manifest.json")).unwrap(),
                "previous manifest"
            );
            assert_eq!(fs::read_dir(&dist).unwrap().count(), 1);
        }
    }
}

#[test]
fn authored_entry_directories_are_rejected_as_non_regular_sources() {
    for entry in ["App.tsx", "Entry.vue", "Entry.svelte"] {
        for target in ["moonshine-dom", "moonshine-ssr"] {
            let site = tempfile::tempdir().unwrap();
            fs::create_dir(site.path().join(entry)).unwrap();
            let result = build(site.path(), target, entry);
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            assert!(!result.status.success());
            assert!(
                diagnostic.contains("source is not a regular file"),
                "{entry}: {diagnostic}"
            );
            assert!(!site.path().join("dist").exists());
            assert!(site.path().join(entry).is_dir());
        }
    }
}

#[test]
fn vue_entry_styles_fail_before_output_changes() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for script in ["", "<script setup>const value = 1;</script>"] {
            for style in [
                "<style scoped>.mark{color:red}</style>",
                "<style global>.mark{color:red}</style>",
                "<style scoped></style>",
            ] {
                let site = tempfile::tempdir().unwrap();
                fs::write(
                    site.path().join("Entry.vue"),
                    format!("{script}<template><p class=\"mark\">parent</p></template>{style}"),
                )
                .unwrap();
                let dist = site.path().join("dist");
                fs::create_dir(&dist).unwrap();
                fs::write(dist.join("sentinel"), "previous output").unwrap();
                let result = build(site.path(), target, "Entry.vue");
                let diagnostic = String::from_utf8_lossy(&result.stderr);
                assert!(!result.status.success(), "{target}: {script} {style}");
                assert!(
                    diagnostic.contains("native Vue styles are unsupported")
                        || diagnostic.contains("unsupported Vue style attribute"),
                    "{diagnostic}"
                );
                assert_eq!(
                    fs::read_to_string(dist.join("sentinel")).unwrap(),
                    "previous output"
                );
                assert_eq!(fs::read_dir(dist).unwrap().count(), 1);
            }
        }
    }
}

#[test]
fn imported_vue_styles_fail_before_output_creation() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(
            site.path().join("Parent.tsx"),
            "import Child from './Child.vue'; export default function Parent(){return <Child/>;}",
        )
        .unwrap();
        fs::write(
            site.path().join("Child.vue"),
            "<template><p>child</p></template><style scoped>p{color:red}</style>",
        )
        .unwrap();
        let result = build(site.path(), target, "Parent.tsx");
        let diagnostic = String::from_utf8_lossy(&result.stderr);
        assert!(!result.status.success());
        assert!(
            diagnostic.contains("native Vue styles are unsupported")
                || diagnostic.contains("unsupported Vue style attribute"),
            "{diagnostic}"
        );
        assert!(!site.path().join("dist").exists());
    }
}

#[test]
fn plain_vue_styles_are_inventoried_assets_for_entry_and_imported_scriptless_components() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for standalone in [true, false] {
            let site = tempfile::tempdir().unwrap();
            let css = "\n.child { color: rgb(0, 0, 255); }\n";
            fs::write(
                site.path().join("Child.vue"),
                format!("<template><p class=\"child\">child</p></template><style>{css}</style>"),
            )
            .unwrap();
            let entry = if standalone {
                "Child.vue"
            } else {
                fs::write(site.path().join("Parent.vue"), "<script setup lang=\"ts\">import Child from './Child.vue';const props=defineProps<{label:string}>();</script><template><section><Child/><Child/>{{props.label}}</section></template><style>.parent{color:red}</style>").unwrap();
                "Parent.vue"
            };
            let result = build(site.path(), target, entry);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let dist = site.path().join("dist");
            let manifest: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(dist.join("crepus-module-manifest.json")).unwrap(),
            )
            .unwrap();
            let assets = manifest["assets"].as_array().unwrap();
            assert_eq!(assets.len(), if standalone { 1 } else { 2 });
            assert!(assets.iter().all(|path| path
                .as_str()
                .unwrap()
                .starts_with(&format!("modules/{target}/"))));
            assert!(assets.iter().any(|path| fs::read_to_string(
                dist.join(path.as_str().unwrap())
            )
            .unwrap()
                == css));
            assert!(manifest["files"]
                .as_array()
                .unwrap()
                .iter()
                .all(|path| !path.as_str().unwrap().ends_with(".css")));
            let child =
                fs::read_to_string(dist.join(format!("modules/{target}/Child.vue.ts"))).unwrap();
            assert!(child.contains("import \"./Child.vue.ts.style-0-0.css\";"));
        }
    }
}

#[test]
fn duplicate_vue_style_attributes_preserve_previous_graph_for_both_targets() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for attrs in [
            "lang=\"css\" lang=\"scss\"",
            "lang=\"scss\" lang=\"css\"",
            "scoped=\"false\" scoped",
            "module=\"false\" module",
        ] {
            let site = tempfile::tempdir().unwrap();
            let entry = site.path().join("App.vue");
            fs::write(
                &entry,
                "<template><p>old</p></template><style>p{color:red}</style>",
            )
            .unwrap();
            let first = build(site.path(), target, "App.vue");
            assert!(
                first.status.success(),
                "{}",
                String::from_utf8_lossy(&first.stderr)
            );
            let dist = site.path().join("dist");
            let facade = dist.join(format!("CrepusEmit.{target}.ts"));
            let before = fs::read(&facade).unwrap();
            let manifest = dist.join("crepus-module-manifest.json");
            let before_manifest = fs::read(&manifest).unwrap();
            fs::write(
                &entry,
                format!("<template><p>new</p></template><style {attrs}>p{{color:blue}}</style>"),
            )
            .unwrap();
            let failed = build(site.path(), target, "App.vue");
            assert!(!failed.status.success());
            assert!(String::from_utf8_lossy(&failed.stderr)
                .contains("duplicate Vue SFC style attribute"));
            assert_eq!(fs::read(&facade).unwrap(), before);
            assert_eq!(fs::read(&manifest).unwrap(), before_manifest);
        }
    }
}

#[test]
fn empty_plain_vue_styles_do_not_publish_assets_or_stylesheet_imports() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for body in ["", "  \n  "] {
            let site = tempfile::tempdir().unwrap();
            fs::write(
                site.path().join("App.vue"),
                format!("<template><p>text</p></template><style>{body}</style>"),
            )
            .unwrap();
            let result = build(site.path(), target, "App.vue");
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let dist = site.path().join("dist");
            let manifest: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(dist.join("crepus-module-manifest.json")).unwrap(),
            )
            .unwrap();
            assert!(manifest.get("assets").is_none());
            let source =
                fs::read_to_string(dist.join(format!("modules/{target}/App.vue.ts"))).unwrap();
            assert!(!source.contains(".css"));
        }
    }
}
