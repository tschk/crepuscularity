#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;
fn compile(module: &str, instance: &str, target: MoonshineModuleTarget) -> Result<String, String> {
    compile_moonshine_svelte(&format!("<script module lang=\"ts\">{module}</script><script lang=\"ts\">{instance}</script><p>{{read()}}</p>"),Path::new("ModuleRunes.svelte"),target).map(|module|module.source)
}
#[test]
fn shared_private_cells_rewrite_component_and_exported_function_reads() {
    let source="let shared=$state(0);let raw=$state.raw({count:1});let double=$derived(shared*2);export function increment(){shared++;raw={count:raw.count+1}}export function read(){return shared+double+raw.count}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(source, "", target).expect("module rune bindings compile");
        assert!(output.contains("shared.value"), "{output}");
        assert!(output.contains("double.value"), "{output}");
        assert!(output.contains("raw.value"), "{output}");
    }
}
#[test]
fn exported_stable_state_is_the_public_value_not_the_cell() {
    let source="export const shared:{count:number}=$state({count:1});let raw=$state.raw({count:2});export{raw as stableRaw};export let scalar=$state(3);export function read(){shared.count++;return shared.count+raw.count+scalar}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(source, "", target).expect("nonreassigned exports remain legal");
        assert!(!output.contains("shared.value"), "{output}");
        assert!(!output.contains("raw.value"), "{output}");
        assert!(!output.contains("scalar.value"), "{output}");
        assert!(output.contains("shared.count++"), "{output}");
    }
}
#[test]
fn invalid_exported_state_and_derived_bindings_fail_closed() {
    for module in [
        "export let n=$state(0);export function read(){return n++}",
        "let n=$state.raw({n:0});export{n as alias};export function read(){n={n:1};return n.n}",
        "export const n=$derived(1);function read(){return n}",
        "let n=$derived.by(()=>1);export{n as alias};function read(){return n}",
        "export let n=$state(0);export function read(){({n}={n:1});return n}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(compile(module, "", target)
                .expect_err("invalid rune exports must reject")
                .contains("export"));
        }
    }
}
#[test]
fn module_and_component_server_derived_cache_policies_remain_distinct() {
    let output = compile(
        "let n=$state(1);let shared=$derived(n*2);function read(){return shared}",
        "let local=$derived(read());",
        MoonshineModuleTarget::Ssr,
    )
    .expect("distinct cache lifetimes");
    assert_eq!(output.matches(", true)").count(), 1, "{output}");
}
#[test]
fn module_snapshots_and_factories_reuse_existing_helpers() {
    let module="const state=$state({n:0});const initial=$state.snapshot(state);export function factory(){let n=$state(0);return()=>++n}export function read(){return $state.snapshot(state).n+initial.n}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        compile(module, "", target).expect("module snapshot and synchronous factories compile");
    }
}
#[test]
fn instance_and_module_placement_guards_are_preserved() {
    for (module, instance) in [
        ("let props=$props();function read(){return 0}", ""),
        (
            "function read(){return 0}",
            "function wrong(){let n=$state(0)}",
        ),
        (
            "function read(){return 0}",
            "async function wrong(){let n=$state(0)}",
        ),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(compile(module, instance, target).is_err());
        }
    }
}
#[test]
fn runtime_rune_shadows_keep_ordinary_module_bindings() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output=compile("function $state(value:number){return value}export const shared=$state(1);function read(){return shared}","",target).expect("resolved runtime bindings are ordinary JS");
        assert!(!output.contains("createState"), "{output}");
    }
}

#[test]
fn module_pause_factories_reuse_cell_and_creation_time_cache_helpers() {
    for source in [
        "export async function factory(){let n=$state(1);await Promise.resolve();let d=$derived(n*2);return()=>[n,d]}function read(){return 0}",
        "export const factory=async()=>{let n=$state.raw({count:1});await Promise.resolve();return()=>++n.count};function read(){return 0}",
        "export function* factory(){let n=$state(1);yield n;let d=$derived(n*2);yield()=>[n,d]}function read(){return 0}",
        "export async function* factory(){let n=$state(1);await Promise.resolve();yield n;let d=$derived.by(()=>n*2);yield()=>[n,d]}function read(){return 0}",
        "export async function factory(){await Promise.resolve();let n=$state({count:1});const copy=$state.snapshot(n);await Promise.resolve();return()=>[n.count,copy.count]}function read(){return 0}",
        "export function* factory(){yield 0;let n=$state.raw({count:1});yield 1;let d=$derived.by(()=>n.count*2);yield()=>[n.count,d]}function read(){return 0}",
        "export async function* factory(){yield 0;await Promise.resolve();let n=$state.raw({count:1});const copy=$state.snapshot(n);yield()=>[n.count,copy.count]}function read(){return 0}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output=compile(source,"",target).expect("module pauses do not require synchronous component setup");
            assert!(output.contains(".value"),"{output}");
            if matches!(target,MoonshineModuleTarget::Ssr) {
                assert!(!output.contains(", true)"),"module derived cache captures its actual creation context: {output}");
            }
        }
    }
}
#[test]
fn writes_inside_paused_module_factories_still_invalidate_export_legality() {
    for source in [
        "export let n=$state(0);export async function write(){await Promise.resolve();n++}function read(){return n}",
        "let n=$state.raw({count:0});export{n};export function* write(){yield 0;n={count:1}}function read(){return n.count}",
        "export const n=$derived(1);export async function read(){await Promise.resolve();return n}",
    ] {
        for target in [MoonshineModuleTarget::Dom,MoonshineModuleTarget::Ssr] {
            assert!(compile(source,"",target).expect_err("paused factories do not relax export legality").contains("export"));
        }
    }
}
