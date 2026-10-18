use std::{fs, path::Path, process::Command};

fn build(site: &Path, target: &str, entry: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .args(["--entry", entry])
        .output()
        .unwrap()
}

fn success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn script_open(extension: &str) -> &'static str {
    if extension == "ts" {
        "<script lang=\"ts\">"
    } else {
        "<script>"
    }
}

fn typed_marker(extension: &str) -> &'static str {
    if extension == "ts" {
        "<script lang=\"ts\"></script>"
    } else {
        ""
    }
}

#[test]
fn svelte_counter_entry_emits_reachable_modules_and_version_one_manifest() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::create_dir_all(site.path().join("app")).unwrap();
        fs::create_dir_all(site.path().join("ui")).unwrap();
        fs::write(
            site.path().join("app/Counter.svelte"),
            r#"<script>
import Child from '../ui/Child.svelte';
import { label } from './label';
let count = $state(1);
let doubled = $derived(count * 2);
</script>
<button onclick={() => count++}>{label} {count} {doubled}</button><Child/>"#,
        )
        .unwrap();
        fs::write(
            site.path().join("app/label.ts"),
            "export const label = 'Count';",
        )
        .unwrap();
        fs::write(site.path().join("ui/Child.svelte"), "<span>Child</span>").unwrap();
        fs::write(
            site.path().join("Unused.svelte"),
            "<script>unsupported!</script>",
        )
        .unwrap();
        success(&build(site.path(), target, "app/Counter.svelte"));
        let dist = site.path().join("dist");
        let emitted = format!("modules/{target}/app/Counter.svelte.js");
        let source = fs::read_to_string(dist.join(&emitted)).unwrap();
        assert!(source.contains("../ui/Child.svelte.js"), "{source}");
        assert!(
            !source.contains("$state(") && !source.contains("$derived("),
            "{source}"
        );
        if target == "moonshine-ssr" {
            assert!(
                !source.contains("@tschk/moonshine-svelte/runes"),
                "{source}"
            );
            assert!(!source.contains("createState"), "{source}");
            assert!(!source.contains("createDerived"), "{source}");
        }
        assert!(dist
            .join(format!("modules/{target}/ui/Child.svelte.js"))
            .is_file());
        assert!(dist
            .join(format!("modules/{target}/app/label.ts"))
            .is_file());
        assert!(!dist
            .join(format!("modules/{target}/Unused.svelte.js"))
            .exists());
        assert!(!dist.join("crepus-view-ir.json").exists());
        let facade = fs::read_to_string(dist.join(format!("CrepusEmit.{target}.ts"))).unwrap();
        assert!(facade.contains(&format!("./{emitted}")), "{facade}");
        assert!(facade.contains("export { default }"));
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(dist.join("crepus-module-manifest.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["version"], 1);
        assert_eq!(manifest["sourceEntry"], "app/Counter.svelte");
        assert_eq!(manifest["sourceModule"], emitted);
        assert!(manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == &emitted));
        assert!(!manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .any(|path| path == "crepus-module-manifest.json"));
    }
}

#[test]
fn jsx_imports_reexports_dynamic_imports_and_type_queries_map_only_module_literals() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for extension in ["js", "ts"] {
            let site = tempfile::tempdir().unwrap();
            fs::create_dir(site.path().join("nested")).unwrap();
            fs::write(
                site.path().join("App.tsx"),
                r#"import Child from './nested/Child.svelte';
export { default as Again } from './nested/Child.svelte';
export const lazy = () => import('./nested/Child.svelte');
export type ChildModule = typeof import('./nested/Child.svelte');
export const literal = './nested/Child.svelte';
export default () => <Child/>;"#,
            )
            .unwrap();
            fs::write(
                site.path().join("nested/Child.svelte"),
                format!("{}<span>child</span>", typed_marker(extension)),
            )
            .unwrap();
            success(&build(site.path(), target, "App.tsx"));
            let source =
                fs::read_to_string(site.path().join(format!("dist/modules/{target}/App.tsx")))
                    .unwrap();
            let mapped = format!("./nested/Child.svelte.{extension}");
            assert_eq!(source.matches(&mapped).count(), 4, "{source}");
            assert!(
                source.contains("literal = \"./nested/Child.svelte\""),
                "{source}"
            );
        }
    }
}

#[test]
fn extensionless_and_directory_imports_map_resolved_svelte_sources() {
    for extension in ["js", "ts"] {
        let site = tempfile::tempdir().unwrap();
        fs::create_dir(site.path().join("widgets")).unwrap();
        fs::write(site.path().join("App.tsx"), "import A from './widgets'; import B from './Button'; export default () => <div><A/><B/></div>;").unwrap();
        fs::write(
            site.path().join("widgets/index.svelte"),
            format!("{}<span>A</span>", typed_marker(extension)),
        )
        .unwrap();
        fs::write(
            site.path().join("Button.svelte"),
            format!("{}<span>B</span>", typed_marker(extension)),
        )
        .unwrap();
        success(&build(site.path(), "moonshine-dom", "App.tsx"));
        let generated = site.path().join("dist/modules/moonshine-dom/App.tsx");
        let prior = fs::read(&generated).unwrap();
        let source = String::from_utf8(prior.clone()).unwrap();
        assert!(
            source.contains(&format!("./widgets/index.svelte.{extension}")),
            "{source}"
        );
        assert!(
            source.contains(&format!("./Button.svelte.{extension}")),
            "{source}"
        );
        fs::write(site.path().join("Button.ts"), "export default () => null;").unwrap();
        success(&build(site.path(), "moonshine-dom", "App.tsx"));
        let manifest =
            fs::read_to_string(site.path().join("dist/crepus-module-manifest.json")).unwrap();
        assert!(
            manifest.contains("modules/moonshine-dom/Button.ts"),
            "{manifest}"
        );
        assert!(
            !manifest.contains(&format!("Button.svelte.{extension}")),
            "{manifest}"
        );
        let prior = fs::read(&generated).unwrap();
        fs::write(site.path().join("Button.js"), "export default () => null;").unwrap();
        let failed = build(site.path(), "moonshine-dom", "App.tsx");
        assert!(!failed.status.success());
        assert!(String::from_utf8_lossy(&failed.stderr).contains("ambiguous"));
        assert_eq!(fs::read(generated).unwrap(), prior);
    }
}

#[test]
fn mapped_path_collisions_and_svelte_errors_preserve_existing_artifacts() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for extension in ["js", "ts"] {
            let site = tempfile::tempdir().unwrap();
            fs::write(
                site.path().join("App.tsx"),
                format!("export {{ default as A }} from './Foo.svelte'; export {{ value as B }} from './Foo.svelte.{extension}';"),
            )
            .unwrap();
            fs::write(
                site.path().join("Foo.svelte"),
                format!("{}<span>ok</span>", typed_marker(extension)),
            )
            .unwrap();
            fs::write(
                site.path().join(format!("Foo.svelte.{extension}")),
                "export const value = 1;",
            )
            .unwrap();
            success(&build(site.path(), target, "App.tsx"));
            let paths = [
                format!("dist/CrepusEmit.{target}.ts"),
                "dist/crepus-module-manifest.json".to_owned(),
                format!("dist/modules/{target}/App.tsx"),
                format!("dist/modules/{target}/Foo.svelte.__crepus0.{extension}"),
                format!("dist/modules/{target}/Foo.svelte.{extension}"),
            ];
            let prior: Vec<_> = paths
                .iter()
                .map(|path| fs::read(site.path().join(path)).unwrap())
                .collect();
            let app = String::from_utf8(prior[2].clone()).unwrap();
            assert!(
                app.contains(&format!("./Foo.svelte.__crepus0.{extension}")),
                "{app}"
            );
            assert!(app.contains(&format!("./Foo.svelte.{extension}")), "{app}");
            for (body, expected_error) in [
                ("let = ;</script><span>bad</span>", None),
                (
                    "let first=$props();let {value}=$props();</script><p>{value}</p>",
                    Some("once"),
                ),
            ] {
                fs::write(
                    site.path().join("Foo.svelte"),
                    format!("{}{body}", script_open(extension)),
                )
                .unwrap();
                let failed = build(site.path(), target, "App.tsx");
                assert!(!failed.status.success());
                if let Some(expected_error) = expected_error {
                    assert!(String::from_utf8_lossy(&failed.stderr).contains(expected_error));
                }
                for (path, expected) in paths.iter().zip(&prior) {
                    assert_eq!(
                        &fs::read(site.path().join(path)).unwrap(),
                        expected,
                        "{path}"
                    );
                }
            }
        }
    }
}

#[test]
fn reachable_plain_context_helpers_use_the_same_native_runtime() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("App.svelte"),"<script>import{get,set}from'./context';set('ready');const value=get();</script><p>{value}</p>").unwrap();
        fs::write(site.path().join("context.ts"),"import{createContext}from'svelte';export const[get,set,has]=createContext<string>();export default <number>1;").unwrap();
        success(&build(site.path(), target, "App.svelte"));
        let path = site
            .path()
            .join(format!("dist/modules/{target}/context.ts"));
        let source = fs::read_to_string(&path).unwrap();
        assert!(
            source.contains("@tschk/moonshine-svelte/svelte"),
            "{source}"
        );
        assert!(!source.contains("from \"svelte\""), "{source}");
        fs::write(site.path().join("context.ts"), "export * from 'svelte';").unwrap();
        let previous = fs::read_to_string(&path).unwrap();
        assert!(!build(site.path(), target, "App.svelte").status.success());
        assert_eq!(fs::read_to_string(&path).unwrap(), previous);
    }
}

#[test]
fn named_svelte_barrels_are_rewritten_through_the_reachable_graph() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("App.svelte"),"<script>import{make}from'./barrel';const[get,set]=make();set('ready');const value=get();</script><p>{value}</p>").unwrap();
        fs::write(site.path().join("barrel.ts"),"export{createContext as make,getContext as default,type setContext as Setter}from'svelte';").unwrap();
        success(&build(site.path(), target, "App.svelte"));
        let source =
            fs::read_to_string(site.path().join(format!("dist/modules/{target}/barrel.ts")))
                .unwrap();
        assert!(
            source.contains("@tschk/moonshine-svelte/svelte"),
            "{source}"
        );
        assert!(source.contains("getContext as default"), "{source}");
        assert!(!source.contains("from \"svelte\""), "{source}");
    }
}

#[test]
fn unlowered_runes_in_reachable_svelte_companions_fail_before_outputs() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for extension in ["ts", "js"] {
            for (diagnostic, source) in [
                (
                    "cannot export reassigned state",
                    "export let value=$state(0);export function replace(){value=1}",
                ),
                ("$derived", "export const factory = $derived;"),
                ("$effect", "export const factory = $effect.pre;"),
            ] {
                let site = tempfile::tempdir().unwrap();
                let companion = format!("state.svelte.{extension}");
                fs::write(
                    site.path().join("App.svelte"),
                    format!(
                        "<script>import * as state from './{companion}';</script><p>companion</p>"
                    ),
                )
                .unwrap();
                fs::write(site.path().join(&companion), source).unwrap();
                let output = build(site.path(), target, "App.svelte");
                assert!(!output.status.success(), "{target}: {source}");
                let stderr = String::from_utf8_lossy(&output.stderr);
                assert!(stderr.contains(&companion), "{stderr}");
                assert!(stderr.contains(diagnostic), "{stderr}");
                assert!(!site.path().join("dist/modules").exists());
                assert!(!site
                    .path()
                    .join(format!("dist/CrepusEmit.{target}.ts"))
                    .exists());
            }
        }
    }
}

#[test]
fn unlowered_companion_rune_failure_preserves_previous_module_outputs() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(
            site.path().join("App.svelte"),
            "<script>import { value } from './state.svelte.ts';</script><p>{value}</p>",
        )
        .unwrap();
        let companion = site.path().join("state.svelte.ts");
        fs::write(&companion, "export const value = 'sentinel';").unwrap();
        success(&build(site.path(), target, "App.svelte"));
        let paths = [
            format!("dist/CrepusEmit.{target}.ts"),
            "dist/crepus-module-manifest.json".to_owned(),
            format!("dist/modules/{target}/App.svelte.js"),
            format!("dist/modules/{target}/state.svelte.ts"),
        ];
        let prior: Vec<_> = paths
            .iter()
            .map(|path| fs::read(site.path().join(path)).unwrap())
            .collect();
        fs::write(
            &companion,
            "export let value=$state('replacement');export function replace(){value='next'}",
        )
        .unwrap();
        let output = build(site.path(), target, "App.svelte");
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("state.svelte.ts"), "{stderr}");
        assert!(
            stderr.contains("cannot export reassigned state"),
            "{stderr}"
        );
        for (path, expected) in paths.iter().zip(prior) {
            assert_eq!(
                fs::read(site.path().join(path)).unwrap(),
                expected,
                "{path}"
            );
        }
    }
}

#[test]
fn companion_shadowed_runes_and_type_references_and_ordinary_modules_remain_allowed() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(
            site.path().join("App.svelte"),
            "<script>import './safe.svelte.ts';import './safe.svelte.js';import './ordinary.ts';</script><p>safe</p>",
        )
        .unwrap();
        fs::write(
            site.path().join("safe.svelte.ts"),
            r#"// $state(0) is only a comment.
const text = '$effect.pre(() => {})';
function $state(value: number) { return value; }
export const value = $state(2);
export type Rune = typeof $derived;
export { text };"#,
        )
        .unwrap();
        fs::write(
            site.path().join("safe.svelte.js"),
            "const $state = value => value; export const value = $state(3);",
        )
        .unwrap();
        fs::write(
            site.path().join("ordinary.ts"),
            "export const value = $state(4);",
        )
        .unwrap();
        success(&build(site.path(), target, "App.svelte"));
        let generated = fs::read_to_string(
            site.path()
                .join(format!("dist/modules/{target}/ordinary.ts")),
        )
        .unwrap();
        assert!(generated.contains("$state(4)"), "{generated}");
        assert!(site
            .path()
            .join(format!("dist/modules/{target}/safe.svelte.ts"))
            .is_file());
        assert!(site
            .path()
            .join(format!("dist/modules/{target}/safe.svelte.js"))
            .is_file());
    }
}

#[test]
fn synchronous_companion_factories_lower_local_state_raw_and_derived_runes() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for extension in ["ts", "js"] {
            let site = tempfile::tempdir().unwrap();
            let companion = format!("counter.svelte.{extension}");
            fs::write(
                site.path().join("App.svelte"),
                format!("<script>import {{ make }} from './{companion}';const counter=make();</script><button onclick={{()=>counter.increment()}}>{{counter.count}}/{{counter.total}}</button>"),
            )
            .unwrap();
            let source = r#"export function make(initial = 1) {
  let count = $state(initial);
  let raw = $state.raw({ bonus: 1 });
  const doubled = $derived(count * 2);
  const total = $derived.by(() => doubled + raw.bonus);
  return {
    get count() { return count; },
    get total() { return total; },
    increment() { count++; raw = { bonus: raw.bonus + 1 }; }
  };
}
export const arrow = () => { let count = $state(0); return () => ++count; };"#;
            let ambient = if extension == "ts" {
                "declare const $state: { <T>(value: T): T; raw<T>(value: T): T };\ndeclare const $derived: { <T>(value: T): T; by<T>(fn: () => T): T };\n"
            } else {
                ""
            };
            fs::write(site.path().join(&companion), format!("{ambient}{source}")).unwrap();
            success(&build(site.path(), target, "App.svelte"));
            let generated = fs::read_to_string(
                site.path()
                    .join(format!("dist/modules/{target}/{companion}")),
            )
            .unwrap();
            for rune in ["$state(", "$state.raw(", "$derived(", "$derived.by("] {
                assert!(!generated.contains(rune), "{rune}: {generated}");
            }
            if target == "moonshine-ssr" {
                assert!(
                    !generated.contains("@tschk/moonshine-svelte/runes"),
                    "{generated}"
                );
                assert!(!generated.contains("createState"), "{generated}");
                assert!(!generated.contains("createDerived"), "{generated}");
            } else {
                assert!(
                    generated.contains("@tschk/moonshine-svelte/runes"),
                    "{generated}"
                );
            }
        }
    }
}

#[test]
fn same_stem_companion_factories_and_nested_components_coexist_for_both_hosts() {
    for extension in ["js", "ts"] {
        let site = tempfile::tempdir().unwrap();
        fs::create_dir(site.path().join("nested")).unwrap();
        fs::write(site.path().join("Counter.svelte"), format!("{}import{{make}}from'./Counter.svelte.{extension}';import Child from'./nested/Child.svelte';const counter=make();</script><button onclick={{()=>counter.increment()}}>{{counter.value}}</button><Child/>", script_open(extension))).unwrap();
        fs::write(site.path().join(format!("Counter.svelte.{extension}")), "export function make(){let value=$state(1);return{get value(){return value},increment(){value++}}}").unwrap();
        fs::write(
            site.path().join("nested/Child.svelte"),
            format!(
                "{}import{{label}}from'./Child.svelte.{extension}';</script><span>{{label}}</span>",
                script_open(extension)
            ),
        )
        .unwrap();
        fs::write(
            site.path().join(format!("nested/Child.svelte.{extension}")),
            "export const label='nested';",
        )
        .unwrap();
        let mut dom_artifacts = Vec::new();
        for target in ["moonshine-dom", "moonshine-ssr"] {
            success(&build(site.path(), target, "Counter.svelte"));
            let root = site.path().join(format!("dist/modules/{target}"));
            let component = format!("Counter.svelte.__crepus0.{extension}");
            let child_component = format!("nested/Child.svelte.__crepus0.{extension}");
            let counter = fs::read_to_string(root.join(&component)).unwrap();
            assert!(
                counter.contains(&format!("./Counter.svelte.{extension}")),
                "{counter}"
            );
            assert!(
                counter.contains(&format!("./nested/Child.svelte.__crepus0.{extension}")),
                "{counter}"
            );
            let child = fs::read_to_string(root.join(&child_component)).unwrap();
            assert!(
                child.contains(&format!("./Child.svelte.{extension}")),
                "{child}"
            );
            let facade_path = site.path().join(format!("dist/CrepusEmit.{target}.ts"));
            let facade = fs::read_to_string(&facade_path).unwrap();
            assert!(facade.contains(&component), "{facade}");
            for path in [
                root.join(&component),
                root.join(format!("Counter.svelte.{extension}")),
                root.join(&child_component),
                root.join(format!("nested/Child.svelte.{extension}")),
                facade_path,
            ] {
                if target == "moonshine-dom" {
                    dom_artifacts.push((path.clone(), fs::read(path).unwrap()));
                }
            }
        }
        for (path, expected) in dom_artifacts {
            assert_eq!(fs::read(path).unwrap(), expected);
        }
    }
}

#[test]
fn remapped_components_skip_reserved_suffix_files_and_directories_deterministically() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for extension in ["js", "ts"] {
            let site = tempfile::tempdir().unwrap();
            fs::create_dir(
                site.path()
                    .join(format!("Card.svelte.__crepus1.{extension}")),
            )
            .unwrap();
            fs::write(
                site.path().join("Card.svelte"),
                format!("{}<span>card</span>", typed_marker(extension)),
            )
            .unwrap();
            for path in [
                format!("Card.svelte.{extension}"),
                format!("Card.svelte.__crepus0.{extension}"),
                format!("Card.svelte.__crepus1.{extension}/helper.ts"),
            ] {
                fs::write(site.path().join(path), "export const value=1;").unwrap();
            }
            let declarations = [
                "export {default as Card} from './Card.svelte';".to_owned(),
                format!("export {{value as Companion}} from './Card.svelte.{extension}';"),
                format!("export {{value as Reserved}} from './Card.svelte.__crepus0.{extension}';"),
                format!(
                    "export {{value as Nested}} from './Card.svelte.__crepus1.{extension}/helper.ts';"
                ),
            ];
            fs::write(site.path().join("App.tsx"), declarations.join("\n")).unwrap();
            success(&build(site.path(), target, "App.tsx"));
            let root = site.path().join(format!("dist/modules/{target}"));
            let component_path = root.join(format!("Card.svelte.__crepus2.{extension}"));
            let component = fs::read(&component_path).unwrap();
            let manifest = fs::read(site.path().join("dist/crepus-module-manifest.json")).unwrap();
            let source = fs::read_to_string(root.join("App.tsx")).unwrap();
            assert!(
                source.contains(&format!("./Card.svelte.__crepus2.{extension}")),
                "{source}"
            );
            assert!(root
                .join(format!("Card.svelte.__crepus1.{extension}/helper.ts"))
                .is_file());
            fs::write(
                site.path().join("App.tsx"),
                declarations
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
            .unwrap();
            success(&build(site.path(), target, "App.tsx"));
            assert_eq!(fs::read(component_path).unwrap(), component);
            assert_eq!(
                fs::read(site.path().join("dist/crepus-module-manifest.json")).unwrap(),
                manifest
            );
        }
    }
}

#[test]
fn remapped_component_output_does_not_block_another_source_directory() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for extension in ["js", "ts"] {
            let site = tempfile::tempdir().unwrap();
            fs::create_dir(site.path().join(format!("A.svelte.{extension}"))).unwrap();
            fs::write(
                site.path().join("A.svelte"),
                format!("{}<span>A</span>", typed_marker(extension)),
            )
            .unwrap();
            fs::write(
                site.path().join(format!("A.svelte.{extension}/B.svelte")),
                format!("{}<span>B</span>", typed_marker(extension)),
            )
            .unwrap();
            fs::write(
                site.path().join("App.tsx"),
                format!("export{{default as A}}from'./A.svelte';export{{default as B}}from'./A.svelte.{extension}/B.svelte';"),
            )
            .unwrap();
            success(&build(site.path(), target, "App.tsx"));
            let root = site.path().join(format!("dist/modules/{target}"));
            assert!(root
                .join(format!("A.svelte.__crepus0.{extension}"))
                .is_file());
            assert!(root
                .join(format!("A.svelte.{extension}/B.svelte.{extension}"))
                .is_file());
            let source = fs::read_to_string(root.join("App.tsx")).unwrap();
            assert!(
                source.contains(&format!("./A.svelte.__crepus0.{extension}")),
                "{source}"
            );
            assert!(
                source.contains(&format!("./A.svelte.{extension}/B.svelte.{extension}")),
                "{source}"
            );
        }
    }
}

#[test]
fn same_stem_javascript_factories_and_jsx_helpers_do_not_hijack_component_imports() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::create_dir(site.path().join("nested")).unwrap();
        fs::write(site.path().join("Card.svelte"), "<script>import{make}from'./Card.svelte.js';import Child from'./nested/Child.svelte';const counter=make();</script><button onclick={()=>counter.increment()}>{counter.value}</button><Child/>").unwrap();
        fs::write(site.path().join("Card.svelte.js"), "export function make(){let value=$state(1);$effect(()=>{void value});return{get value(){return value},increment(){value++}}}").unwrap();
        fs::write(
            site.path().join("nested/Child.svelte"),
            "<script>import{label}from'./Child.svelte.jsx';</script><p>{label}</p>",
        )
        .unwrap();
        fs::write(
            site.path().join("nested/Child.svelte.jsx"),
            "export const label='helper';",
        )
        .unwrap();
        success(&build(site.path(), target, "Card.svelte"));
        let root = site.path().join(format!("dist/modules/{target}"));
        let card = fs::read_to_string(root.join("Card.svelte.__crepus0.js")).unwrap();
        assert!(card.contains("./Card.svelte.js"), "{card}");
        assert!(
            card.contains("./nested/Child.svelte.__crepus0.js"),
            "{card}"
        );
        let child = fs::read_to_string(root.join("nested/Child.svelte.__crepus0.js")).unwrap();
        assert!(child.contains("./Child.svelte.jsx"), "{child}");
        assert!(root.join("Card.svelte.js").is_file());
        assert!(root.join("nested/Child.svelte.jsx").is_file());
        assert!(!root.join("Card.svelte.ts").exists());
        assert!(!root.join("nested/Child.svelte.ts").exists());
        let facade =
            fs::read_to_string(site.path().join(format!("dist/CrepusEmit.{target}.ts"))).unwrap();
        assert!(facade.contains("Card.svelte.__crepus0.js"));
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(site.path().join("dist/crepus-module-manifest.json")).unwrap(),
        )
        .unwrap();
        for leaf in [
            "Card.svelte.__crepus0.js",
            "Card.svelte.js",
            "nested/Child.svelte.__crepus0.js",
            "nested/Child.svelte.jsx",
        ] {
            assert!(manifest["files"]
                .as_array()
                .unwrap()
                .iter()
                .any(|path| path == &format!("modules/{target}/{leaf}")));
        }
        assert_eq!(manifest["sourceEntry"], "Card.svelte");
        assert_eq!(
            manifest["sourceModule"],
            format!("modules/{target}/Card.svelte.__crepus0.js")
        );
    }
}

#[test]
fn javascript_and_jsx_candidate_stems_are_reserved_and_failed_rebuilds_preserve_artifacts() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(site.path().join("Card.svelte"), "<p>card</p>").unwrap();
        for path in [
            "Card.svelte.js",
            "Card.svelte.__crepus0.js",
            "Card.svelte.__crepus1.jsx",
        ] {
            fs::write(site.path().join(path), "export const value=1;").unwrap();
        }
        let declarations = [
            "export{default as Card}from'./Card.svelte';",
            "export{value as Companion}from'./Card.svelte.js';",
            "export{value as First}from'./Card.svelte.__crepus0.js';",
            "export{value as Second}from'./Card.svelte.__crepus1.jsx';",
        ];
        fs::write(site.path().join("App.tsx"), declarations.join("\n")).unwrap();
        success(&build(site.path(), target, "App.tsx"));
        let paths = [
            format!("dist/modules/{target}/Card.svelte.__crepus2.js"),
            format!("dist/modules/{target}/Card.svelte.js"),
            format!("dist/modules/{target}/Card.svelte.__crepus0.js"),
            format!("dist/modules/{target}/Card.svelte.__crepus1.jsx"),
            format!("dist/CrepusEmit.{target}.ts"),
            "dist/crepus-module-manifest.json".to_string(),
        ];
        let prior = paths
            .iter()
            .map(|path| fs::read(site.path().join(path)).unwrap())
            .collect::<Vec<_>>();
        fs::write(
            site.path().join("App.tsx"),
            declarations
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        success(&build(site.path(), target, "App.tsx"));
        for (path, bytes) in paths.iter().zip(&prior) {
            assert_eq!(&fs::read(site.path().join(path)).unwrap(), bytes, "{path}");
        }
        fs::write(
            site.path().join("Card.svelte"),
            "<script>let = ;</script><p/>",
        )
        .unwrap();
        assert!(!build(site.path(), target, "App.tsx").status.success());
        for (path, bytes) in paths.iter().zip(&prior) {
            assert_eq!(&fs::read(site.path().join(path)).unwrap(), bytes, "{path}");
        }
    }
}

#[test]
fn stale_typescript_outputs_cannot_shadow_javascript_companions_on_rebuild() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        fs::write(
            site.path().join("Card.svelte"),
            "<script lang=\"ts\"></script><p>first build</p>",
        )
        .unwrap();
        success(&build(site.path(), target, "Card.svelte"));
        let stale = format!("dist/modules/{target}/Card.svelte.ts");
        let facade = format!("dist/CrepusEmit.{target}.ts");
        let manifest = "dist/crepus-module-manifest.json".to_string();
        let paths = [stale, facade, manifest];
        let prior = paths
            .iter()
            .map(|path| fs::read(site.path().join(path)).unwrap())
            .collect::<Vec<_>>();
        fs::write(
            site.path().join("Card.svelte"),
            "<script>import{value}from'./Card.svelte.js';</script><p>{value}</p>",
        )
        .unwrap();
        fs::write(site.path().join("Card.svelte.js"), "export const value=2;").unwrap();
        let failed = build(site.path(), target, "Card.svelte");
        assert!(!failed.status.success());
        let diagnostic = String::from_utf8_lossy(&failed.stderr).to_lowercase();
        assert!(diagnostic.contains("shadow"), "{diagnostic}");
        assert!(
            diagnostic.contains("empty") && diagnostic.contains("output"),
            "{diagnostic}"
        );
        for (path, bytes) in paths.iter().zip(&prior) {
            assert_eq!(&fs::read(site.path().join(path)).unwrap(), bytes, "{path}");
        }
        let root = site.path().join(format!("dist/modules/{target}"));
        assert!(!root.join("Card.svelte.__crepus0.ts").exists());
        assert!(!root.join("Card.svelte.js").exists());
    }
}

#[test]
fn preexisting_typescript_shadow_sentinels_survive_failed_javascript_graph_preflight() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for (extension, shadow_extension) in [("js", "ts"), ("jsx", "tsx")] {
            let site = tempfile::tempdir().unwrap();
            fs::write(site.path().join("Card.svelte"), format!("<script>import{{value}}from'./Card.svelte.{extension}';</script><p>{{value}}</p>")).unwrap();
            fs::write(
                site.path().join(format!("Card.svelte.{extension}")),
                "export const value=1;",
            )
            .unwrap();
            let root = site.path().join(format!("dist/modules/{target}"));
            fs::create_dir_all(&root).unwrap();
            let paths = [
                root.join(format!("Card.svelte.{shadow_extension}")),
                site.path().join(format!("dist/CrepusEmit.{target}.ts")),
                site.path().join("dist/crepus-module-manifest.json"),
            ];
            for path in &paths {
                fs::write(path, b"preexisting sentinel").unwrap();
            }
            let failed = build(site.path(), target, "Card.svelte");
            assert!(!failed.status.success());
            let diagnostic = String::from_utf8_lossy(&failed.stderr).to_lowercase();
            assert!(
                diagnostic.contains("shadow") && diagnostic.contains("empty"),
                "{diagnostic}"
            );
            for path in paths {
                assert_eq!(fs::read(path).unwrap(), b"preexisting sentinel");
            }
            assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
            assert_eq!(fs::read_dir(site.path().join("dist")).unwrap().count(), 3);
        }
    }
}
