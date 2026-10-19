#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn intrinsic_spread_dom_emits_explicit_owned_installer() {
    let source = "<script>let props=$props();</script><button title=\"before\" {...props} data-tail=\"after\">text</button>";
    let output = compile_moonshine_svelte(
        source,
        Path::new("Spread.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap();
    assert!(
        output.source.contains("installElementSpread"),
        "{}",
        output.source
    );
    assert!(!output.source.contains("__crepusSvelteElementSpread"));
}

#[test]
fn intrinsic_spread_ssr_emits_explicit_attribute_entries() {
    let source = "<script>let props=$props();</script><button title=\"before\" {...props} data-tail=\"after\">text</button>";
    let output = compile_moonshine_svelte(
        source,
        Path::new("Spread.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap();
    assert!(
        output.source.contains("spreadServerEntries"),
        "{}",
        output.source
    );
}

#[test]
fn server_spread_snapshots_precede_later_attribute_reads() {
    let output = compile_moonshine_svelte(
        "<script>let props=$props();function later(){return 1;}</script><p {...props} data-after={later()}/>",
        Path::new("Order.svelte"), MoonshineModuleTarget::Ssr,
    ).unwrap();
    let snapshot = output
        .source
        .find("Object.assign(")
        .expect("eager snapshot");
    let after = output
        .source
        .rfind("later()")
        .expect("later attribute read");
    assert!(snapshot < after, "{}", output.source);
}

#[test]
fn nested_intrinsic_and_component_spreads_keep_independent_helpers_and_hygiene() {
    let source = r#"<script>
import Child from './Child.svelte';
let props=$props();
const __crepusSpreadAttributeEntries=1;
const __crepusSpreadAttributeElement=2;
const __crepusSvelteElementSpread0=3;
</script><section {...props}><Child {...props}><p {...props}/></Child><div {...props}><span {...props}/></div></section>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("Nested.svelte"), target).unwrap();
        let helper = match target {
            MoonshineModuleTarget::Dom => "installElementSpread",
            MoonshineModuleTarget::Ssr => "spreadServerEntries",
        };
        assert!(compiled.source.contains(helper), "{}", compiled.source);
        assert!(compiled
            .imports
            .iter()
            .any(|value| value == "./Child.svelte"));
        match target {
            MoonshineModuleTarget::Dom => assert!(compiled.source.contains("spreadProps as")),
            MoonshineModuleTarget::Ssr => {
                assert!(compiled.source.contains("spreadServerProps as"));
                assert!(compiled
                    .source
                    .contains("as __crepusSpreadAttributeEntries_"));
                assert!(compiled
                    .source
                    .contains("as __crepusSpreadAttributeElement_"));
            }
        }
    }
}

#[test]
fn form_spreads_use_native_dom_and_typed_ssr_contracts() {
    for tag in ["input", "textarea", "select", "option"] {
        let source = format!("<script>let props=$props();</script><{tag} {{...props}}/>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_svelte(&source, Path::new("Form.svelte"), target).unwrap();
            let expected = if matches!(target, MoonshineModuleTarget::Dom) { "installElementSpread" } else { "spreadServerElement" };
            assert!(output.source.contains(expected), "{}", output.source);
            assert!(!output.source.contains("__crepusSvelteElementSpread"));
        }
    }
}

#[test]
fn value_and_checked_spreads_keep_binding_installers_out_of_attribute_layers() {
    for source in [
        "<script>let attrs=$props();let value=$state('bound');</script><input {...attrs} bind:value/>",
        "<script>let attrs=$props();let value=$state('bound');</script><textarea bind:value {...attrs}/>",
        "<script>let attrs=$props();let checked=$state(false);</script><input type=\"checkbox\" bind:checked {...attrs}/>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_svelte(source, Path::new("Bound.svelte"), target).unwrap();
            if matches!(target, MoonshineModuleTarget::Dom) {
                assert!(output.source.contains("installBoundElementSpread"), "{}", output.source);
                assert!(output.source.contains("boundSpread("), "{}", output.source);
            } else {
                assert!(output.source.contains("spreadServerElement"), "{}", output.source);
            }
        }
    }
}

#[test]
fn select_binding_spreads_use_typed_form_ssr_without_erasing_option_identity() {
    let source = "<script>let attrs=$props();const item={id:1};let value=$state(item);</script><select {...attrs} bind:value><option value={item}>Same</option></select>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Select.svelte"), target).unwrap();
        let expected = if matches!(target, MoonshineModuleTarget::Dom) { "installBoundElementSpread" } else { "spreadServerElement" };
        assert!(output.source.contains(expected), "{}", output.source);
    }
}

#[test]
fn file_binding_spreads_keep_server_model_reads_unreachable() {
    let source = "<script lang=\"ts\">let attrs=$props();let files=$state<FileList|null>(null);</script><input type=\"file\" bind:files {...attrs}/>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output=compile_moonshine_svelte(source,Path::new("Files.svelte"),target).unwrap();
        let expected=if matches!(target,MoonshineModuleTarget::Dom){"installBoundElementSpread"}else{"spreadServerElement"};
        assert!(output.source.contains(expected),"{}",output.source);
    }
}

#[test]
fn open_binding_spreads_preserve_owned_dom_and_boolean_ssr_paths() {
    let source="<script>let attrs=$props();let open=$state(false);</script><details {...attrs} bind:open><summary>Title</summary>Body</details>";
    for target in [MoonshineModuleTarget::Dom,MoonshineModuleTarget::Ssr] {
        let output=compile_moonshine_svelte(source,Path::new("Open.svelte"),target).unwrap();
        let expected=if matches!(target,MoonshineModuleTarget::Dom){"installBoundElementSpread"}else{"spreadServerEntries"};
        assert!(output.source.contains(expected),"{}",output.source);
    }
}

#[test]
fn group_binding_spreads_preserve_owned_dom_and_ordered_ssr_contracts() {
    let source = r#"<script lang="ts">
let attrs=$props();
let selected=$state<string[]>(['a']);
let choice=$state('a');
let dynamicVal=$state('b');
</script>
<input type="checkbox" {...attrs} onchange={(event)=>selected.push(event.currentTarget.value)} bind:group={selected}/>
<input type="checkbox" value={dynamicVal} {...attrs} bind:group={selected}/>
<input type="radio" value="a" bind:group={choice} {...attrs}/>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("GroupSpread.svelte"), target).unwrap();
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(output.source.contains("installBoundElementSpread"), "{}", output.source);
            assert!(output.source.contains("createInputGroup"), "{}", output.source);
            assert!(!output.source.contains("__installGroupMember"), "{}", output.source);
            assert!(output.source.contains(".bind(__groupInput, () => dynamicVal.value)"), "{}", output.source);
        } else {
            assert!(output.source.contains("spreadServerElement"), "{}", output.source);
            assert!(!output.source.contains("hSourceEntries"), "{}", output.source);
            assert!(output.source.contains(".includes("), "{}", output.source);
        }
    }
}
