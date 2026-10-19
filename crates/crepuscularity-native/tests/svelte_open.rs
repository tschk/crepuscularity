#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

const TARGETS: [MoonshineModuleTarget; 2] =
    [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr];

fn compile(source: &str, target: MoonshineModuleTarget) -> String {
    compile_moonshine_svelte(source, Path::new("Open.svelte"), target)
        .unwrap_or_else(|error| panic!("{source}\n{error:?}"))
        .source
}

#[test]
fn shorthand_and_member_details_open_bindings_lower_on_both_hosts() {
    for (script, binding) in [
        ("let open=$state(false);", "bind:open"),
        ("let model=$state({open:false});", "bind:open={model.open}"),
        (
            "let model=$state({open:false});let key='open';",
            "bind:open={model[key]}",
        ),
        ("let {open=false}=$props();", "bind:open"),
    ] {
        let source = format!(
            "<script lang=\"ts\">{script}</script><details {binding} ontoggle={{(event:Event)=>event.preventDefault()}}><summary>More</summary>body</details>"
        );
        for target in TARGETS {
            let output = compile(&source, target);
            assert!(!output.contains("__crepusOpenBinding"), "{output}");
            assert!(!output.contains("bind:open"), "{output}");
            match target {
                MoonshineModuleTarget::Dom => {
                    assert!(output.contains("bindOpen"), "{output}");
                    assert!(output.contains("@tschk/moonshine-dom/binding"), "{output}");
                    assert!(output.contains("toggle"), "{output}");
                    assert!(output.contains("onCleanup"), "{output}");
                    assert!(output.contains("removeEventListener"), "{output}");
                }
                MoonshineModuleTarget::Ssr => {
                    assert!(!output.contains("bindOpen"), "{output}");
                    assert!(!output.contains("@tschk/moonshine-dom/binding"), "{output}");
                    assert!(!output.contains("addEventListener"), "{output}");
                    assert!(!output.contains("removeEventListener"), "{output}");
                    assert!(output.contains("open"), "{output}");
                    assert!(output.contains("!!"), "{output}");
                }
            }
        }
    }
}

#[test]
fn plain_script_details_open_binding_lowers_on_both_hosts() {
    let source = "<script>let open=$state(true);</script><details bind:open ontoggle={()=>console.log('toggle')}><summary>More</summary></details>";
    for target in TARGETS {
        let output = compile(source, target);
        assert!(!output.contains("__crepusOpenBinding"), "{output}");
        assert_eq!(
            matches!(target, MoonshineModuleTarget::Dom),
            output.contains("bindOpen"),
            "{output}"
        );
    }
}

#[test]
fn store_targets_reach_the_existing_store_write_lowering() {
    let source = r#"<script lang="ts">import{writable}from'svelte/store';const expanded=writable<boolean>(false);</script><details bind:open={$expanded}><summary>More</summary></details>"#;
    for target in TARGETS {
        let output = compile(source, target);
        assert!(!output.contains("from \"svelte/store\""), "{output}");
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(output.contains("setStoreValue"), "{output}");
            assert!(output.contains("bindOpen"), "{output}");
        }
    }
}

#[test]
fn each_and_control_flow_members_keep_their_owned_binding_contract() {
    for source in [
        "<script lang=\"ts\">let rows=$state([{open:false},{open:true}]);</script>{#each rows as row}<details bind:open={row.open}><summary>x</summary></details>{/each}",
        "<script lang=\"ts\">let rows=$state([{open:false}]);</script>{#each rows as row, index}<details bind:open={rows[index].open}>{index}</details>{/each}",
        "<script lang=\"ts\">let visible=$state(true);let open=$state(false);</script>{#if visible}<details bind:open><summary>x</summary></details>{/if}",
    ] {
        for target in TARGETS {
            let output = compile(source, target);
            assert!(!output.contains("__crepusOpenBinding"), "{output}");
            assert_eq!(
                matches!(target, MoonshineModuleTarget::Dom),
                output.contains("bindOpen"),
                "{output}"
            );
        }
    }
}

#[test]
fn each_items_cannot_be_bound_directly() {
    let source = "<script lang=\"ts\">let rows=$state([false]);</script>{#each rows as row}<details bind:open={row}></details>{/each}";
    for target in TARGETS {
        assert!(
            compile_moonshine_svelte(source, Path::new("Open.svelte"), target).is_err(),
            "{source}"
        );
    }
}

#[test]
fn wrong_targets_conflicts_duplicates_and_immutable_roots_are_rejected() {
    for source in [
        "<script>let open=$state(false);</script><div bind:open></div>",
        "<script>let open=$state(false);</script><dialog bind:open></dialog>",
        "<script>let open=$state(false);</script><input bind:open/>",
        "<script>let open=$state(false);</script><svg><details bind:open></details></svg>",
        "<script>let open=$state(false);let tag='details';</script><svelte:element this={tag} bind:open/>",
        "<script>let open=$state(false);</script><details open bind:open></details>",
        "<script>let open=$state(false);</script><details bind:open open={open}></details>",
        "<script>let open=$state(false);</script><details {open} bind:open></details>",
        "<script>let open=$state(false);</script><details bind:open bind:open></details>",
        "<script>let open=$state(false);let other=$state(false);</script><details bind:open={open} bind:open={other}></details>",
        "<script>const open=false;</script><details bind:open></details>",
        "<script>import open from './state.js';</script><details bind:open></details>",
        "<script>const model={open:false};let open=$state(false);</script><details bind:open={model.open || open}></details>",
        "<script>let model=$state({open:false});</script><details bind:open={model?.open}></details>",
        "<script>let model=$state({open:false});</script><details bind:open={get().open}></details>",
        "<details bind:open={missing}></details>",
        "<script>let open=$state(false);</script><details bind:open=\"\"></details>",
        "<script>let open=$state(false);</script><details bind:open={}></details>",
    ] {
        for target in TARGETS {
            assert!(
                compile_moonshine_svelte(source, Path::new("Open.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn getter_setter_function_bindings_are_not_silently_admitted() {
    for source in [
        "<script>let open=$state(false);</script><details bind:open={()=>open,(value)=>open=value}></details>",
        "<script>let open=$state(false);</script><details bind:open={[()=>open,(value)=>open=value]}></details>",
        "<script>let open=$state(false);</script><details bind:open={()=>open}></details>",
    ] {
        for target in TARGETS {
            assert!(
                compile_moonshine_svelte(source, Path::new("Open.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn component_open_bindings_keep_their_component_prop_contract() {
    let source = "<script>import Child from './Child.svelte';let open=$state(true);</script><Child bind:open/>";
    for target in TARGETS {
        let output = compile(source, target);
        assert!(!output.contains("bindOpen"), "{output}");
        assert!(!output.contains("__crepusOpenBinding"), "{output}");
        assert!(output.contains("open"), "{output}");
    }
}

#[test]
fn details_open_binding_is_distinct_from_checked_and_value_bindings() {
    let source = "<script lang=\"ts\">let open=$state(false);let checked=$state(false);let text=$state('');</script><details bind:open><summary>x</summary><input type=\"checkbox\" bind:checked/><input bind:value={text}/></details>";
    let output = compile(source, MoonshineModuleTarget::Dom);
    for helper in ["bindOpen", "bindChecked", "bindValue"] {
        assert!(output.contains(helper), "{helper}\n{output}");
    }
    let output = compile(source, MoonshineModuleTarget::Ssr);
    for helper in ["bindOpen", "bindChecked", "bindValue"] {
        assert!(!output.contains(helper), "{helper}\n{output}");
    }
}

#[test]
fn grouped_open_handlers_keep_member_reads_late_until_event_dispatch() {
    let source = r#"<script lang="ts">let open=$state(false);let handlers=$state({toggle:()=>{}});</script><details bind:open ontoggle={handlers.toggle}></details>"#;
    let output = compile(source, MoonshineModuleTarget::Dom);
    assert!(output.contains("return handlers.value.toggle;"), "{output}");
    assert!(output.contains("listen();\n\t__crepusBindOpen"), "{output}");
}
