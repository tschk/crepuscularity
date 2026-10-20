use std::{fs, path::Path, process::Command};
const P: &str = "/** @jsxImportSource solid-js */\n";
fn build(site: &Path, output: &Path, target: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_crepus"))
        .args(["web", "build", "--emit", target, "--site"])
        .arg(site)
        .args(["--entry", "App.tsx", "--out-dir"])
        .arg(output)
        .output()
        .unwrap()
}
#[test]
fn list_controls_and_imported_rows_share_the_verified_graph() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        let site = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        fs::write(site.path().join("App.tsx"),format!("{P}import {{For as Rows,Index as Slots}} from 'solid-js';import Row from './Row';export default function App(){{return <main><Rows each={{[1,2]}} fallback={{<i>empty</i>}}>{{(item,index)=><Row value={{item}} index={{index()}}/>}}</Rows><Slots each={{[1,2]}} children={{(item,index)=><Row value={{item()}} index={{index}}/>}}/></main>}}")).unwrap();
        fs::write(site.path().join("Row.tsx"),format!("{P}export default function Row(props:{{value:number;index:number}}){{return <p>{{props.value}}:{{props.index}}</p>}}")).unwrap();
        let result = build(site.path(), output.path(), target);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let source =
            fs::read_to_string(output.path().join("modules").join(target).join("App.tsx")).unwrap();
        assert!(source.contains("__crepusSolidFor") && source.contains("__crepusSolidIndex"));
        assert!(!source.contains("solid-js"));
    }
}
#[test]
fn unsupported_list_identities_preserve_existing_graph_outputs() {
    for target in ["moonshine-dom", "moonshine-ssr"] {
        for body in ["import type {For} from 'solid-js';export default()=> <For each={[]}>{row=>row}</For>;","import * as S from 'solid-js';export default()=> <S.For each={[]}>{row=>row}</S.For>;","import {For} from 'solid-js';const Alias=For;export default()=> <Alias each={[]}>{row=>row}</Alias>;"] {
            let site=tempfile::tempdir().unwrap();let output=tempfile::tempdir().unwrap();fs::write(site.path().join("App.tsx"),format!("{P}{body}")).unwrap();let facade=output.path().join(format!("CrepusEmit.{target}.ts"));fs::write(&facade,"// preserved\n").unwrap();let result=build(site.path(),output.path(),target);assert!(!result.status.success(),"{body}");assert_eq!(fs::read_to_string(facade).unwrap(),"// preserved\n");
        }
    }
}
