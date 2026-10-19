use std::fs;
use std::process::Command;

const SELECTOR: &str = "/** @jsxImportSource solid-js */\n";

fn build(site: &std::path::Path, output: &std::path::Path, target: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .args(["--entry", "App.tsx", "--out-dir"])
        .arg(output)
        .output()
        .unwrap()
}

#[test]
fn relative_component_aliases_and_named_barrels_compile_for_both_renderers() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        fs::write(site.path().join("App.tsx"), format!("{SELECTOR}import {{Child as Label}} from './barrel'; export default function App(){{return <Label label=\"ok\"/>}}")).unwrap();
        fs::write(
            site.path().join("barrel.ts"),
            format!("{SELECTOR}export {{default as Child}} from './Child';"),
        )
        .unwrap();
        fs::write(site.path().join("Child.tsx"), format!("{SELECTOR}export default function Child(props:{{label:string}}){{return <span>{{props.label}}</span>}}")).unwrap();
        let result = build(site.path(), output.path(), target);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let child =
            fs::read_to_string(output.path().join("modules").join(target).join("Child.tsx"))
                .unwrap();
        assert!(child.contains("@tschk/moonshine-solid/"));
        assert!(!child.contains("solid-js"));
    }
}

#[test]
fn invalid_component_graphs_preserve_existing_output() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for child in [
            "export default function Child(){return <span/>}",
            "/** @jsxImportSource solid-js */\nexport default async function Child(){return <span/>}",
            "/** @jsxImportSource solid-js */\nexport const Other=()=> <span/>;",
            "/** @jsxImportSource solid-js */\nlet Child=()=> <span/>;export {Child as default};",
            "/** @jsxImportSource solid-js */\nfunction Child(){return <span/>} Child=()=> <div/>;export default Child;",
        ] {
            let site = tempfile::tempdir().unwrap();
            let output = tempfile::tempdir().unwrap();
            fs::write(site.path().join("App.tsx"),format!("{SELECTOR}import Child from './Child';export default function App(){{return <Child/>}}")).unwrap();
            fs::write(site.path().join("Child.tsx"),child).unwrap();
            let facade=output.path().join(format!("CrepusEmit.{target}.ts"));
            fs::write(&facade,"// preserved\n").unwrap();
            let result=build(site.path(),output.path(),target);
            assert!(!result.status.success(),"unexpected admission: {child}");
            assert_eq!(fs::read_to_string(facade).unwrap(),"// preserved\n");
        }
    }
}

#[test]
fn default_reexports_and_imported_default_entries_keep_verified_provenance() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for barrel in [
            "export {default} from './Child';",
            "import Child from './Child';export default Child;",
            "import {Child as Label} from './Child';export {Label as default};",
        ] {
            let site = tempfile::tempdir().unwrap();
            let output = tempfile::tempdir().unwrap();
            fs::write(site.path().join("App.tsx"), format!("{SELECTOR}{barrel}")).unwrap();
            fs::write(site.path().join("Child.tsx"),format!("{SELECTOR}export function Child(){{return <span>ok</span>}} export default Child;")).unwrap();
            let result = build(site.path(), output.path(), target);
            assert!(
                result.status.success(),
                "{barrel}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}
