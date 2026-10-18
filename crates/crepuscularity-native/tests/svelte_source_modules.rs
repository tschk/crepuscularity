#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_module, compile_moonshine_svelte, rewrite_moonshine_imports,
    MoonshineModuleTarget,
};
use std::{collections::HashMap, path::Path};

#[test]
fn instance_imports_module_exports_and_lifecycle_survive_native_compilation() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(r#"<script module>export const shared = 3;</script>
<script lang="ts">import Child from './Child.svelte'; import { onDestroy as cleanup, onMount } from 'svelte';
let props = $props(); let count: number = $state(0); let doubled = $derived(count*2);
cleanup(()=>console.log('destroy')); onMount(()=>()=>console.log('unmount'));</script>
<button onclick={()=>count++}>{count}/{doubled}</button><Child count={count} title={props.title}/>"#, Path::new("Counter.svelte"), target).unwrap();
        assert!(compiled.has_default_export);
        assert!(compiled.source.contains("export const shared"));
        assert!(compiled
            .imports
            .iter()
            .any(|value| value == "./Child.svelte"));
        assert!(compiled.source.contains("@tschk/moonshine-svelte/svelte"));
        assert!(!compiled.source.contains("$state("));
        assert!(!compiled.source.contains("$derived("));
    }
}

#[test]
fn unsupported_framework_runtime_forms_do_not_escape_as_external_imports() {
    for source in [
        "<script>const load=()=>import('svelte');</script><p>bad</p>",
        "<script module>export * from 'svelte/store';</script><p>bad</p>",
        "<script>import * as s from 'svelte';</script><p>bad</p>",
        "<script>import { createEventDispatcher } from 'svelte';</script><p>bad</p>",
        "<script module>export default 1;</script><p>bad</p>",
        "<script>export let name='legacy';</script><p>{name}</p>",
    ] {
        assert!(
            compile_moonshine_svelte(source, Path::new("Bad.svelte"), MoonshineModuleTarget::Dom)
                .is_err(),
            "accepted {source}"
        );
    }
}

#[test]
fn import_rewrites_cover_only_module_specifiers_with_escaped_names() {
    let source = r#"import X from './a.svelte'; export {default} from './a.svelte'; export * from './a.svelte';
const lazy=()=>import('./a.svelte'); type A=typeof import('./a.svelte'); const literal='./a.svelte'; // './a.svelte'
"#;
    let target = "./a.svelte.ts";
    let rewritten = rewrite_moonshine_imports(
        source,
        Path::new("file.ts"),
        &HashMap::from([("./a.svelte".into(), target.into())]),
    )
    .unwrap();
    assert_eq!(rewritten.matches(target).count(), 5);
    assert!(rewritten.contains("const literal='./a.svelte'"));
    assert!(rewritten.contains("// './a.svelte'"));
}

#[test]
fn destructured_props_defaults_rest_and_derived_by_compile_for_both_hosts() {
    let source = r#"<script lang="ts">
interface Props { count?: number; title?: string; note: string }
let {count=1,title:label='fallback',...rest}:Props=$props();
let doubled=$derived.by(()=>{const value=count;return value*2});
const description={count,label};
function shadow(count:number){return count}
</script><button onclick={()=>count++}>{count.toFixed(0)}/{label}/{rest.note}/{doubled}</button><button onclick={()=>doubled+=2}>override</button>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Child.svelte"), target).unwrap();
        let dom = matches!(target, MoonshineModuleTarget::Dom);
        assert_eq!(compiled.source.contains("createDerived"), dom);
        assert_eq!(
            compiled.source.contains("@tschk/moonshine-svelte/runes"),
            dom
        );
        assert_eq!(compiled.source.contains("count.value++"), dom);
        assert_eq!(compiled.source.contains("doubled.value += 2"), dom);
        assert!(compiled.source.contains("count.value.toFixed(0)"));
        assert!(!compiled.source.contains("$props("));
        assert!(!compiled.source.contains("$derived.by"));
        assert_eq!(compiled.source.contains("createServerDerived as"), !dom);
        assert_eq!(
            compiled.source.contains("@tschk/moonshine-svelte/ssr"),
            !dom
        );
        assert!(!compiled.source.contains("function __crepusSvelteDerived"));
    }
}

#[test]
fn unsupported_prop_patterns_fail_before_source_is_emitted() {
    for source in [
        "<script>let {count:{nested}}=$props();</script><p/>",
        "<script>let key='count';let {[key]:count}=$props();</script><p/>",
        "<script>let {count=$bindable(0,1)}=$props();</script><p/>",
        "<script>const {count}=$props();count++;</script><p/>",
    ] {
        assert!(
            compile_moonshine_svelte(source, Path::new("Bad.svelte"), MoonshineModuleTarget::Dom)
                .is_err(),
            "accepted {source}"
        );
    }
}

#[test]
fn deep_state_snapshots_and_effects_lower_to_native_helpers_and_erase_on_server() {
    let source = r#"<script lang="ts">
let state=$state({nested:{count:1},items:[1,2]});
let empty=$state<number>();
function makeEffect(){return()=>{state.nested.count;return()=>console.log('cleanup')}}
$effect(makeEffect());
$effect(()=>{$effect(()=>state.items.length)});
</script><button onclick={()=>{state.items.push(3);empty=1;console.log($state.snapshot(state))}}>{state.nested.count}</button>"#;
    let dom =
        compile_moonshine_svelte(source, Path::new("Deep.svelte"), MoonshineModuleTarget::Dom)
            .unwrap();
    assert!(dom.source.contains("createState"));
    assert!(dom.source.contains("stateSnapshot"));
    assert!(dom.source.contains("createRuneEffect"));
    assert!(dom.source.contains("state.value.items.push(3)"));
    let ssr =
        compile_moonshine_svelte(source, Path::new("Deep.svelte"), MoonshineModuleTarget::Ssr)
            .unwrap();
    assert!(!ssr.source.contains("createRuneEffect"));
    assert!(!ssr.source.contains("makeEffect();"));
    assert!(!ssr.source.contains("$effect("));
    assert!(ssr.source.contains("function makeEffect()"));
}

#[test]
fn unsupported_effect_variants_and_result_bindings_fail_for_both_hosts() {
    for source in [
        "<script>let stop=$effect.pre(()=>{});</script><p/>",
        "<script>let stop=$effect(()=>{});</script><p/>",
        "<script module>$effect(()=>{});</script><p/>",
        "<script>$effect(()=>{$effect.pending()});</script><p/>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_svelte(source, Path::new("Bad.svelte"), target).is_err(),
                "accepted {source}"
            );
        }
    }
}

#[test]
fn raw_state_preserves_member_mutations_and_reactive_replacement_for_each_host() {
    let source = r#"<script lang="ts">
const initial={count:1};
let state=$state.raw(initial);
let optional=$state.raw<number>();
let callback=$state.raw(()=>42);
function replace(){state={count:2};optional=3;callback=()=>43}
</script><button onclick={replace}>{state.count}/{optional}/{callback()}</button>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Raw.svelte"), target).unwrap();
        assert!(!compiled.source.contains("$state.raw"));
        assert!(!compiled.source.contains("createState"));
        assert!(compiled.source.contains("state.value.count"));
        assert!(compiled.source.contains("optional.value = 3"));
        assert_eq!(
            compiled.source.contains("createRawState"),
            matches!(target, MoonshineModuleTarget::Dom)
        );
    }
}

#[test]
fn props_aliases_use_target_specific_views_without_rune_runtime_imports() {
    let source = r#"<script lang="ts">interface Props {title:string};let props:Props=$props();</script><p>{props.title}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Alias.svelte"), target).unwrap();
        assert!(!compiled.source.contains("$props("));
        assert!(!compiled.source.contains("createState"));
        assert!(!compiled.source.contains("createProps"));
        assert!(compiled.source.contains("props: __crepusSvelteInterface"));
        assert!(compiled.source.contains("title: string"));
        assert!(compiled.source.contains("__crepusSvelteRest"));
        assert_eq!(
            compiled.source.contains("globalThis.Proxy"),
            matches!(target, MoonshineModuleTarget::Dom)
        );
    }
}

#[test]
fn reactive_component_tags_use_owned_selection_while_imports_stay_static() {
    let source = r#"<script>import Static from './Static.svelte';let Current=$state.raw(Static);let {box}=$props();</script><Static/><Current/><box.Child/>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("Dynamic.svelte"), target).unwrap();
        assert!(compiled.source.contains("mapArray"));
        assert!(compiled.source.contains("@tschk/moonshine-dom/control"));
        assert!(!compiled.source.contains("<Current"));
        assert!(!compiled.source.contains("<box"));
    }
}

#[test]
fn authored_key_blocks_use_imported_owned_control_for_both_hosts() {
    let source = r#"<script>import Child from './Child.svelte';let key=$state(1);</script>{#key key}<Child value={key}/>{/key}"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Key.svelte"), target).unwrap();
        assert!(compiled.source.contains("@tschk/moonshine-dom/control"));
        assert!(compiled.source.contains("For as"));
        assert!(compiled.source.contains("key.value"));
        assert!(!compiled.source.contains("#key"));
    }
}

#[test]
fn server_key_expressions_are_validated_but_never_evaluated() {
    let source = "<script>function readKey(){throw new Error('key must not run')}</script>{#key readKey()}<p>outer</p>{#key readKey()}<p>inner</p>{/key}{/key}";
    let dom = compile_moonshine_svelte(source, Path::new("Key.svelte"), MoonshineModuleTarget::Dom)
        .unwrap();
    let ssr = compile_moonshine_svelte(source, Path::new("Key.svelte"), MoonshineModuleTarget::Ssr)
        .unwrap();
    assert_eq!(dom.source.matches("readKey()").count(), 3);
    assert_eq!(ssr.source.matches("readKey()").count(), 1); // declaration only
    assert!(ssr.source.contains("outer"));
    assert!(ssr.source.contains("inner"));
    assert!(compile_moonshine_svelte(
        "{#key $effect.pre(()=>{})}<p/>{/key}",
        Path::new("Invalid.svelte"),
        MoonshineModuleTarget::Ssr
    )
    .is_err());
}

#[test]
fn effect_tracking_queries_are_native_on_client_and_constant_false_on_server() {
    let source = r#"<script module>export const outside=$effect.tracking();</script><script>let count=$state(0);let value=$derived($effect.tracking()?count:0);$effect(()=>console.log(value,$effect.tracking()));</script><p>{String($effect.tracking())}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("Tracking.svelte"), target).unwrap();
        assert!(!compiled.source.contains("$effect"));
        assert_eq!(
            compiled.source.contains("runeTracking"),
            matches!(target, MoonshineModuleTarget::Dom)
        );
        if matches!(target, MoonshineModuleTarget::Ssr) {
            assert!(compiled.source.contains("outside = false"));
        }
    }
}

#[test]
fn context_helpers_bridge_named_aliases_and_keep_generic_calls() {
    let source = r#"<script module lang="ts">import{createContext}from'svelte';export const [get,set,has]=createContext<string>();</script><script>import{getContext as read,getAllContexts as contexts,setContext as provide,hasContext}from'svelte';provide('name','value');const value=read('name');</script><p>{value}/{String(hasContext('name'))}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("Context.svelte"), target).unwrap();
        assert!(compiled.source.contains("@tschk/moonshine-svelte/svelte"));
        assert!(compiled.source.contains("getContext as read"));
        assert!(compiled.source.contains("getAllContexts as contexts"));
        assert!(compiled.source.contains("createContext<string>()"));
        assert!(!compiled.imports.iter().any(|path| path == "svelte"));
    }
}

#[test]
fn svelte_helper_type_imports_stay_type_only_after_runtime_splitting() {
    let source = r#"<script module lang="ts">import type{getContext as Read,untrack as Detached}from'svelte';export type Reader=typeof Read;export type Detach=typeof Detached;</script><p>ready</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("TypeImports.svelte"), target).unwrap();
        let rewritten: Vec<_> = compiled
            .source
            .lines()
            .filter(|line| {
                line.contains("import") && (line.contains("Read") || line.contains("Detached"))
            })
            .collect();
        assert_eq!(rewritten.len(), 2, "{}", compiled.source);
        assert!(
            rewritten
                .iter()
                .all(|line| line.starts_with("import type ")),
            "{}",
            compiled.source
        );
        assert!(compiled.source.contains("export type Reader"));
    }
}

#[test]
fn svelte_module_scripts_can_reexport_named_helpers_but_not_a_runtime_default() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(
            "<script module>export{createContext as make}from'svelte';</script><p/>",
            Path::new("Barrel.svelte"),
            target,
        )
        .unwrap();
        assert!(compiled.has_default_export);
        assert!(compiled.source.contains("@tschk/moonshine-svelte/svelte"));
        assert!(!compiled.imports.iter().any(|path| path == "svelte"));
        assert!(compile_moonshine_svelte(
            "<script module>export{getContext as default}from'svelte';</script><p/>",
            Path::new("Bad.svelte"),
            target
        )
        .is_err());
    }
}

#[test]
fn checked_bindings_rewrite_state_members_and_mutable_props_before_host_lowering() {
    for script in [
        "let checked=$state(false);",
        "let model=$state({checked:false});",
        "let {checked=false}=$props();",
    ] {
        let binding = if script.contains("model") {
            "model.checked"
        } else {
            "checked"
        };
        let source = format!(
            "<script lang=\"ts\">{script}</script><form onreset={{(event:Event)=>event.preventDefault()}}><input type=\"checkbox\" bind:checked={{{binding}}}/></form>"
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let compiled =
                compile_moonshine_svelte(&source, Path::new("Checked.svelte"), target).unwrap();
            assert!(!compiled.source.contains("__crepusCheckedBinding"));
            if matches!(target, MoonshineModuleTarget::Dom) {
                assert!(compiled.source.contains("@tschk/moonshine-dom/binding"));
                assert!(!compiled.source.contains("__crepusSvelteDynamic"));
            } else {
                assert!(!compiled.source.contains("@tschk/moonshine-dom/binding"));
            }
        }
    }
}

#[test]
fn checked_bindings_reject_nonassignable_and_missing_targets_on_both_hosts() {
    for (script, binding) in [
        ("const checked=false;", "checked"),
        ("", "missing"),
        ("let checked=$state(false);", "checked || false"),
        ("let model=$state({checked:false});", "model?.checked"),
    ] {
        let source = format!(
            "<script lang=\"ts\">{script}</script><form onreset={{(event:Event)=>event.preventDefault()}}><input type=\"checkbox\" bind:checked={{{binding}}}/></form>"
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_svelte(&source, Path::new("Invalid.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn value_bindings_share_owned_lowering_with_checked_bindings() {
    let source = r#"<script lang="ts">let {value='start'}=$props();let model=$state({number:2});let checked=$state(false);</script><input bind:value oninput={(event:Event)=>event.preventDefault()}/><input type="number" bind:value={model.number}/><textarea bind:value defaultValue="unused"/><input type="checkbox" bind:checked/>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("Values.svelte"), target).unwrap();
        assert!(!compiled.source.contains("__crepusValueBinding"));
        assert!(!compiled.source.contains("__crepusCheckedBinding"));
        assert!(!compiled.source.contains("__crepusSvelteDynamic"));
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(compiled.source.contains("bindValue"));
            assert!(compiled.source.contains("bindChecked"));
        } else {
            assert!(!compiled.source.contains("@tschk/moonshine-dom/binding"));
        }
    }
}

#[test]
fn value_bindings_reject_invalid_targets_and_unsupported_controls() {
    for source in [
        r#"<script>const value='fixed';</script><input bind:value/>"#,
        r#"<input bind:value={missing}/>"#,
        r#"<script>let value=$state('');</script><input bind:value={value||''}/>"#,
        r#"<script>let value=$state(0);</script><input type="file" bind:value/>"#,
        r#"<script>let value=$state('');</script><textarea bind:value>default</textarea>"#,
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_svelte(source, Path::new("InvalidValue.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn form_binding_member_targets_require_identifier_roots() {
    for (tag, directive) in [("<input", "value"), ("<input type=\"checkbox\"", "checked")] {
        for expression in [
            "get().value",
            "new Object().value",
            "({value:false}).value",
            "this.value",
            "[false][0]",
        ] {
            let source = format!("{tag} bind:{directive}={{{expression}}}/>");
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                assert!(
                    compile_moonshine_svelte(&source, Path::new("InvalidMember.svelte"), target)
                        .is_err(),
                    "{source}"
                );
            }
        }
        for expression in ["model.value", "model[choose()]", "model[key?.name]"] {
            let source = format!(
                "<script>let model=$state({{value:false}});let key={{name:'value'}};function choose(){{return 'value'}}</script>{tag} bind:{directive}={{{expression}}}/>"
            );
            for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                compile_moonshine_svelte(&source, Path::new("Member.svelte"), target).unwrap();
            }
        }
    }
}

#[test]
fn manual_effect_roots_support_module_expression_and_optional_calls() {
    let source = r#"<script module lang="ts">export const stopModule=$effect.root(moduleFactory());</script><script lang="ts">let value=$state(0);const stop=$effect?.root?.<void>(()=>{$effect(()=>value);return()=>cleanup()});const immediate=$effect.root(()=>{})();const name=$effect.root(()=>{}).name;</script><button onclick={()=>{$effect.root(()=>{$effect(()=>value)});value++}}>run</button>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Root.svelte"), target).unwrap();
        assert!(!compiled.source.contains("$effect"));
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(compiled.source.contains("createRuneRoot"));
            assert!(compiled.source.contains("moduleFactory()"));
            assert!(compiled.source.contains("createRuneEffect"));
        } else {
            assert!(!compiled.source.contains("moduleFactory()"));
            assert!(!compiled.source.contains("cleanup()"));
            assert!(!compiled.source.contains("@tschk/moonshine-svelte/runes"));
        }
    }
}

#[test]
fn manual_effect_roots_reject_invalid_argument_shapes_and_references() {
    for expression in [
        "$effect.root()",
        "$effect.root(first,second)",
        "$effect.root(...callbacks)",
        "$effect.root",
        "$effect['root'](()=>{})",
    ] {
        let source = format!("<script>const stop={expression};</script><p/>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_svelte(&source, Path::new("InvalidRoot.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn server_intrinsic_event_factories_are_erased_but_component_props_are_retained() {
    let source = r#"<script>import Child from './Child.svelte';let value=$state('');</script><button onclick={makeIntrinsic()}>click</button><input bind:value oninput={makeBinding()}/><Child onclick={makeComponent()}/>"#;
    let dom = compile_moonshine_svelte(
        source,
        Path::new("Events.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap();
    let ssr = compile_moonshine_svelte(
        source,
        Path::new("Events.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap();
    for factory in ["makeIntrinsic()", "makeBinding()", "makeComponent()"] {
        assert!(dom.source.contains(factory));
    }
    assert!(!ssr.source.contains("makeIntrinsic()"));
    assert!(!ssr.source.contains("makeBinding()"));
    assert!(ssr.source.contains("makeComponent()"));
}

#[test]
fn dom_svelte_events_defer_factories_and_preserve_live_handler_reads() {
    let source = r#"<script>import Child from './Child.svelte';let value=$state('');let callback=$state.raw(()=>{});function make(){return()=>{}}</script><button onclick={make()}>factory</button><button onclick={callback}>live</button><input bind:value oninput={make()}/><Child onclick={make()}/>"#;
    let dom = compile_moonshine_svelte(
        source,
        Path::new("LazyEvents.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap();
    assert!(dom.source.contains("createDerived"));
    assert!(dom.source.contains(".apply(this"));
    assert!(!dom.source.contains("$state"));
    let ssr = compile_moonshine_svelte(
        source,
        Path::new("LazyEvents.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap();
    assert!(!ssr.source.contains("createDerived"));
    assert!(!ssr.source.contains("__crepusSvelteEvent"));
}

#[test]
fn component_prop_factories_allocate_owned_accessors_only_in_dom() {
    let source = r#"<script lang="ts">import Child from './Child.svelte';let count=$state(0);function make(){return ()=>count;}</script><Child callback={make()} active={count===1} value={count} stable={()=>count}/>{#key count}<Child callback={make()}/>{/key}"#;
    let dom = compile_moonshine_svelte(
        source,
        Path::new("ComponentProps.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap();
    assert!(dom
        .source
        .contains("createDerived as __crepusSvelteComponentPropDerived"));
    assert!(dom.source.contains("get: __crepusSvelteComponentProp("));
    assert!(!dom
        .source
        .contains("get: () => __crepusSvelteComponentProp("));
    assert!(!dom.source.contains("$state"));
    let ssr = compile_moonshine_svelte(
        source,
        Path::new("ComponentProps.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap();
    assert!(!ssr.source.contains("__crepusSvelteComponentProp"));
    assert!(ssr.source.contains("make()"));
}

#[test]
fn component_references_use_symbol_props_and_instance_export_descriptors() {
    let parent = r#"<script>import Child from './Child.svelte';let instance=$state();let box=$state({current:undefined});</script><Child bind:this={instance}/><Child bind:this={box.current}/>"#;
    let child = r#"<script>import {onMount} from 'svelte';let count=$state(0);onMount(()=>{});$effect(()=>count);export const kind='counter';export function increment(){count++};export function read(){return count};export class Klass{};</script><p>{count}</p>"#;
    for source in [parent, child, "<p>empty exports</p>"] {
        let dom = compile_moonshine_svelte(
            source,
            Path::new("Reference.svelte"),
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert!(dom.source.contains("exposeComponent"));
        if source == child {
            let exposure = dom.source.find("__crepusSvelteExpose0(").unwrap();
            assert!(dom.source.find("onMount(").unwrap() < exposure);
            assert!(dom.source.find("__crepusSvelteEffect(").unwrap() < exposure);
        }
        assert!(!dom.source.contains("__crepusSvelteExposure"));
        let ssr = compile_moonshine_svelte(
            source,
            Path::new("Reference.svelte"),
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert!(!ssr.source.contains("component-instance"));
        assert!(!ssr.source.contains("__crepusComponentBinding"));
    }
    let dom = compile_moonshine_svelte(
        parent,
        Path::new("Reference.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap();
    assert!(dom.source.contains("[__crepusSvelteComponentRef]"));
    assert_eq!(dom.source.matches("publication: \"post\"").count(), 2);
    assert!(!dom.source.contains("__crepusComponentBinding"));
    assert!(dom
        .source
        .contains("instance.value = __crepusReferenceValue"));
    let exported_class =
        compile_moonshine_svelte(child, Path::new("Class.svelte"), MoonshineModuleTarget::Dom)
            .unwrap();
    assert!(exported_class
        .source
        .contains("Klass = __crepusExportValue"));
    for source in [
        "<script>import Child from './Child.svelte';const instance=undefined;</script><Child bind:this={instance}/>",
        "<script>import Child from './Child.svelte';let box={};</script><Child bind:this={box?.current}/>",
        "<script>export const count=$derived(1);</script><p/>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_svelte(source, Path::new("Invalid.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn snippet_prop_annotations_keep_type_only_imports_for_both_targets() {
    for import in [
        "import {type Snippet as Row} from 'svelte';",
        "import type {Snippet as Row} from 'svelte';",
    ] {
        let source = format!(
            "<script lang=\"ts\">{import}let{{row}}:{{row:Row<[string]>}}=$props();</script>{{@render row('one')}}"
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let compiled =
                compile_moonshine_svelte(&source, Path::new("Typed.svelte"), target).unwrap();
            assert!(
                compiled.source.contains("Snippet as Row"),
                "{}",
                compiled.source
            );
            assert!(
                compiled.source.contains("Row<[string]>"),
                "{}",
                compiled.source
            );
            assert!(compiled.source.contains("@tschk/moonshine-svelte/svelte"));
            assert!(!compiled.source.contains("from \"svelte\""));
        }
    }
}

#[test]
fn root_snippets_keep_target_specific_argument_contracts_and_owned_rendering() {
    let source = r#"<script lang="ts">import Card from './Card.svelte';let value=$state('one');function make(){return value}</script>{@render row(make())}{#snippet row(value: string)}<p>{value}</p>{/snippet}<Card body={row} value={value}/>{@render row?.(value)}"#;
    let dom = compile_moonshine_svelte(
        source,
        Path::new("Snippets.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap();
    assert!(dom
        .source
        .contains("mapArray as __crepusSvelteSnippetRuntime"));
    assert!(dom
        .source
        .contains("createDerived as __crepusSvelteSnippetDerived"));
    assert!(dom.source.contains("readonly value: string"));
    assert!(!dom.source.contains("$state"));
    let ssr = compile_moonshine_svelte(
        source,
        Path::new("Snippets.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap();
    assert!(!ssr.source.contains("defer as __crepusSvelteSnippetRuntime"));
    assert!(ssr.source.contains("return props.call();"));
    assert!(ssr.source.contains("value: string"));
    assert!(!ssr.source.contains("createDerived"));
    assert!(ssr.source.contains("row(make())"));
    let member = "<script>let value=$state({text:'one'});</script>{#snippet row(item)}<input bind:value={item.text}/>{/snippet}{@render row(value)}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        assert!(compile_moonshine_svelte(member, Path::new("Member.svelte"), target).is_ok());
    }
}

#[test]
fn unsupported_snippet_parameter_and_render_forms_are_hard_errors() {
    for source in [
        "{#snippet row(value)}<input bind:value={value}/>{/snippet}",
        "{#snippet row(value)}<Child bind:this={value}/>{/snippet}",
        "{#snippet row(value)}<button onclick={()=>value++}/>{/snippet}",
        "{#snippet row(value=1)}<p>{value}</p>{/snippet}",
        "{#snippet row({value})}<p>{value}</p>{/snippet}",
        "{#snippet row(...values)}<p/>{/snippet}",
        "<script>function row(){}</script>{#snippet row()}<p/>{/snippet}",
        "{@render value}",
        "{@render object?.row()}",
        "{@render row(...items)}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_svelte(source, Path::new("Invalid.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn raw_html_uses_owned_content_without_relaxing_ordinary_jsx() {
    let source = r#"<script lang="ts">let markup=$state('<b>one</b>');</script><div>{@html markup}</div><section>before{@html markup}<span>{markup}</span></section>{@html markup}"#;
    let dom = compile_moonshine_svelte(source, Path::new("Raw.svelte"), MoonshineModuleTarget::Dom)
        .unwrap();
    assert!(dom.source.contains("htmlContent as"));
    assert!(dom.source.contains("[\"ref\"]:"));
    assert!(!dom.source.contains("[\"ref\"]: () =>"));
    assert!(dom.source.contains("rawHTML as"));
    assert!(dom.source.contains("@tschk/moonshine-dom/html"));
    let ssr = compile_moonshine_svelte(source, Path::new("Raw.svelte"), MoonshineModuleTarget::Ssr)
        .unwrap();
    assert!(ssr.source.contains("rawHTML as"));
    assert!(ssr.source.contains("defer as"));
    assert!(!ssr.source.contains("htmlContent"));
    assert!(!ssr.source.contains("@tschk/moonshine-dom/html"));
    let references = compile_moonshine_svelte("<script>import Child from './Child.svelte';let child;</script><Child bind:this={child}/><div>{@html '<b>one</b>'}</div>", Path::new("References.svelte"), MoonshineModuleTarget::Dom).unwrap();
    assert!(references.source.contains("componentRef as"));
    assert!(references.source.contains("htmlContent as"));
    let snippet = "<script lang=\"ts\"></script>{#snippet row(markup:string)}<div>{@html markup}</div>{/snippet}{@render row('<em>one</em>')}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        assert!(compile_moonshine_svelte(snippet, Path::new("Snippet.svelte"), target).is_ok());
        assert!(compile_moonshine_module(
            "export default function View(){return <div innerHTML={'raw'}/>}",
            Path::new("Raw.tsx"),
            target
        )
        .is_err());
        assert!(compile_moonshine_svelte(
            "<svg>{@html markup}</svg>",
            Path::new("Svg.svelte"),
            target
        )
        .is_err());
    }
}

#[test]
fn authored_javascript_boundaries_preserve_comments_regex_and_inner_quotes() {
    for template in [
        "<p>{value /* } */ + 2}</p>",
        "<p>{value // }\n + 2}</p>",
        "<p>{value // }\n}</p>",
        "<input title={value /* } */ + 2}/>",
        "<input title=\"x{value /* } */ + 2}y\"/>",
        "<input title=\"x{value // }\n}y\"/>",
        "<input title=\"{value ? \"}\" : \"{\"}\"/>",
        "<p>{(/}/).source}{(/{/).source}{(/[/*]/).source}{(/[//]/).source}</p>",
        "<p>{8 / /}/.source.length}{8 / 2 / 2}</p>",
        "<p>{`a${`b${value}`}c`}</p>",
        "{#if value /* } */}<p/>{/if}",
        "{#key value /* } */}<p/>{/key}",
        "{@html value /* } */}",
    ] {
        let source = format!("<script>let value=$state(6);</script>{template}");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            compile_moonshine_svelte(&source, Path::new("Boundary.svelte"), target)
                .unwrap_or_else(|error| panic!("{template}: {error}"));
        }
    }
    for template in [
        "<p>{value /* }</p>",
        "<p>{value // }</p>",
        "<p>{(/}/)</p>",
        "<input title=\"x{value /* }\"/>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_svelte(template, Path::new("Invalid.svelte"), target).is_err()
            );
        }
    }
}

#[test]
fn typed_comment_snippet_boundary_is_an_explicit_native_typescript_extension() {
    // Pinned Svelte rejects the brace inside this comment. Native parsing keeps
    // the complete comment; this checks that extension, not host parity.
    let template =
        "{#snippet row(item /* ) } */:number)}<p>{item}</p>{/snippet}{@render row(value /* } */)}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let typed = format!("<script lang=\"ts\">let value=$state(6);</script>{template}");
        compile_moonshine_svelte(&typed, Path::new("TypedBoundary.svelte"), target).unwrap();
        let original = format!("<script>let value=$state(6);</script>{template}");
        assert!(compile_moonshine_svelte(
            &original,
            Path::new("JavaScriptBoundary.svelte"),
            target
        )
        .is_err());
    }
}

#[test]
fn typed_event_and_raw_snippet_parameters_do_not_bypass_javascript_validation() {
    for source in [
        "<script>let checked=$state(false);</script><form onreset={(event:Event)=>event.preventDefault()}><input type=\"checkbox\" bind:checked={checked}/></form>",
        "{#snippet row(markup:string)}<div>{@html markup}</div>{/snippet}{@render row('<em>one</em>')}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_svelte(source, Path::new("JavaScript.svelte"), target)
                .expect_err("TypeScript annotations require authored TypeScript mode");
            assert!(error.contains("invalid authored JavaScript Svelte template expression"), "{error}");
        }
    }
}

#[test]
fn whitespace_cleanup_precedes_implicit_component_children() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for body in [" \n\t ", "&#13;&#10;", " <!-- removed --> "] {
            let source = format!(
                "<script>import Child from './Child.svelte';</script><Child>{body}</Child>"
            );
            let compiled =
                compile_moonshine_svelte(&source, Path::new("Empty.svelte"), target).unwrap();
            assert!(
                !compiled.source.contains("children: {"),
                "{}",
                compiled.source
            );
            assert!(
                !compiled.source.contains("__CrepusText"),
                "{}",
                compiled.source
            );
        }
        let source =
            "<script>import Child from './Child.svelte';</script><pre><Child> \n\t </Child></pre>";
        let compiled =
            compile_moonshine_svelte(source, Path::new("Preserved.svelte"), target).unwrap();
        assert!(
            compiled.source.contains("children: {"),
            "{}",
            compiled.source
        );
    }
}

#[test]
fn pre_structural_ssr_anchor_also_applies_without_authored_text_markers() {
    let source = "<script>import Child from './Child.svelte';</script><pre><Child/></pre>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Pre.svelte"), target).unwrap();
        assert_eq!(
            compiled.source.contains("<!---->"),
            matches!(target, MoonshineModuleTarget::Ssr)
        );
        let direct = compile_moonshine_svelte(
            "<script>let value='\\nplain';</script><pre>{value}</pre>",
            Path::new("Direct.svelte"),
            target,
        )
        .unwrap();
        assert!(!direct.source.contains("<!---->"));
    }
}

#[test]
fn explicit_textarea_values_preserve_raw_dom_updates_and_ssr_prefix_provenance() {
    for (value, compensate) in [
        ("{readValue()}", false),
        ("\"{readValue()}\"", false),
        ("\"\n{readValue()}\"", true),
        ("\"\r\n{readValue()}\"", true),
        ("\"&#10;{readValue()}\"", true),
        ("\"\r{readValue()}\"", false),
        ("\"{readValue()}\n\"", false),
    ] {
        let source = format!("<textarea value={value}/>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let out = compile_moonshine_svelte(&source, Path::new("Explicit.svelte"), target)
                .unwrap()
                .source;
            assert!(!out.contains("__crepusTextareaValue"), "{out}");
            assert_eq!(out.matches("readValue()").count(), 1, "{out}");
            match target {
                MoonshineModuleTarget::Dom => {
                    assert!(out.contains("__crepusTextareaUpdate("), "{out}")
                }
                MoonshineModuleTarget::Ssr => assert_eq!(
                    out.contains("function __crepusTextareaSsrValue"),
                    !compensate,
                    "{out}"
                ),
            }
        }
    }
    let native = compile_moonshine_module(
        "export default function C(){return <textarea value={readValue()}/>}",
        Path::new("Native.tsx"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert!(!native.contains("TextareaUpdate"), "{native}");
}

#[test]
fn static_textarea_defaults_preserve_live_values_with_owned_dom_updates() {
    for body in ["BODY", "\n", "\r\n", "&#10;", "&lt;b&gt;"] {
        let source = format!("<textarea defaultValue={{readDefault()}}>{body}</textarea>");
        let out = compile_moonshine_svelte(
            &source,
            Path::new("StaticBody.svelte"),
            MoonshineModuleTarget::Dom,
        )
        .unwrap()
        .source;
        assert!(out.contains("__crepusTextareaUpdate("), "{out}");
        assert!(
            out.contains("createRenderEffect as __crepusTextareaUpdateEffect"),
            "{out}"
        );
        assert_eq!(out.matches("readDefault()").count(), 1, "{out}");
    }
    for source in [
        "<textarea defaultValue={readDefault()}/>",
        "<textarea defaultValue={readDefault()}>{readBody()}</textarea>",
    ] {
        let out = compile_moonshine_svelte(
            source,
            Path::new("OtherBody.svelte"),
            MoonshineModuleTarget::Dom,
        )
        .unwrap()
        .source;
        assert_eq!(
            out.contains("__crepusTextareaUpdateEffect"),
            source.contains("readBody"),
            "{out}"
        );
    }
    let source = "<form>\n<textarea defaultValue='A'>first</textarea>\n<textarea defaultValue={readDefault()}>second</textarea>\n<output>tail</output></form>";
    let out = compile_moonshine_svelte(
        source,
        Path::new("Siblings.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &out, oxc_span::SourceType::tsx()).parse();
    assert!(parsed.errors.is_empty());
    let semantic = oxc_semantic::SemanticBuilder::new()
        .build(&parsed.program)
        .semantic;
    assert_eq!(semantic.nodes().iter().filter(|node| matches!(node.kind(), oxc_ast::AstKind::CallExpression(call) if call.callee.is_specific_id("__crepusTextareaUpdate"))).count(), 2, "{out}");
}

#[test]
fn server_textarea_defaults_are_erased_without_reading_them_or_hiding_the_body() {
    for body in ["", "BODY", "{readBody()}"] {
        for key in ["defaultValue", "defaultvalue", "DEFAULTVALUE"] {
            let source = format!("<textarea {key}={{readDefault()}}>{body}</textarea>");
            let out = compile_moonshine_svelte(
                &source,
                Path::new("Defaults.svelte"),
                MoonshineModuleTarget::Ssr,
            )
            .unwrap()
            .source;
            assert_eq!(out.contains("readDefault"), key != "defaultValue", "{out}");
            assert_eq!(
                out.contains("readBody()"),
                body.contains("readBody"),
                "{out}"
            );
            assert_eq!(out.contains("BODY"), body == "BODY", "{out}");
        }
    }
    let out = compile_moonshine_svelte("<script>import Child from './Child.svelte';</script><Child defaultValue={readDefault()}/><input defaultValue={readInput()}/>", Path::new("Other.svelte"), MoonshineModuleTarget::Ssr).unwrap().source;
    assert!(out.contains("readDefault()"));
    assert!(out.contains("readInput()"));
    let out = compile_moonshine_module("export default function C(){return <textarea defaultValue={readDefault()}>BODY</textarea>}", Path::new("Native.tsx"), MoonshineModuleTarget::Ssr).unwrap().source;
    assert!(out.contains("readDefault()"));
}

#[test]
fn static_entity_text_preserves_textarea_defaults_without_a_live_value() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(
            "<textarea>&NewLine;&amp;copy;&#123;value&#125;&lt;b&gt;</textarea>",
            Path::new("Entities.svelte"),
            target,
        )
        .unwrap();
        assert!(
            !compiled.source.contains("__CrepusText"),
            "{}",
            compiled.source
        );
        assert!(!compiled.source.contains("value:"), "{}", compiled.source);
        assert!(
            !compiled.source.contains("&NewLine;"),
            "{}",
            compiled.source
        );
        assert!(
            compiled.source.contains("&copy;{value}<b>"),
            "{}",
            compiled.source
        );
        assert!(!compiled.source.contains("literal:"));
    }
}

#[test]
fn textarea_literal_and_expression_line_breaks_keep_separate_ssr_rules() {
    for (body, strip) in [
        ("{value}", true),
        ("\n\n{value}", false),
        ("&#10;{value}", true),
        ("\n&#10;{value}", false),
        ("\n\r{value}", true),
        ("\r\r{value}", true),
        ("\r&#10;{value}", true),
    ] {
        let source = format!("<script>let value='text';</script><textarea>{body}</textarea>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let compiled =
                compile_moonshine_svelte(&source, Path::new("Breaks.svelte"), target).unwrap();
            assert!(!compiled.source.contains("leadingNewlineStripped"));
            assert_eq!(
                compiled.source.contains("stripLeadingNewline: true"),
                strip && matches!(target, MoonshineModuleTarget::Ssr),
                "{}",
                compiled.source
            );
        }
    }
}

#[test]
fn dynamic_textarea_bodies_use_live_values_only_on_the_dom_host() {
    let source = r#"<script>let text=$state('start');</script><textarea defaultValue="reset">before&amp;{text}</textarea>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("Textarea.svelte"), target).unwrap();
        let allocator = oxc_allocator::Allocator::default();
        let parsed =
            oxc_parser::Parser::new(&allocator, &compiled.source, oxc_span::SourceType::tsx())
                .parse();
        assert!(parsed.errors.is_empty());
        let built = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
        let calls: Vec<_> = built.semantic.nodes().iter().filter_map(|node| {
            let oxc_ast::AstKind::CallExpression(call) = node.kind() else { return None };
            let Some(oxc_ast::ast::Expression::Identifier(callee)) = Some(call.callee.get_inner_expression()) else { return None; };
            let symbol = built.semantic.scoping().get_reference(callee.reference_id.get()?).symbol_id()?;
            let offset = parsed.program.body.iter().find_map(|statement| {
                let oxc_ast::ast::Statement::ImportDeclaration(import) = statement else { return None; };
                if import.source.value != "@tschk/moonshine-dom/dom" && import.source.value != "@tschk/moonshine-dom/ssr" { return None; }
                import.specifiers.iter().flatten().find_map(|specifier| {
                    let oxc_ast::ast::ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else { return None; };
                    if specifier.local.symbol_id.get() != Some(symbol) { return None; }
                    match specifier.imported.name().as_str() { "hNS" => Some(1), "h" | "hSource" => Some(0), _ => None }
                })
            })?;
            matches!(call.arguments.get(offset), Some(oxc_ast::ast::Argument::StringLiteral(tag)) if tag.value == "textarea").then_some((call,offset))
        }).collect();
        assert_eq!(calls.len(), 1);
        let (call, offset) = calls[0];
        let has_prop = |key: &str| matches!(call.arguments.get(offset + 1), Some(oxc_ast::ast::Argument::ObjectExpression(props)) if props.properties.iter().any(|property| matches!(property, oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property) if property.key.static_name().is_some_and(|name| name == key))));
        let dom = matches!(target, MoonshineModuleTarget::Dom);
        assert_eq!(has_prop("defaultValue"), dom);
        assert_eq!(has_prop("value"), dom);
        assert_eq!(call.arguments.len(), offset + if dom { 2 } else { 3 });
    }
}

#[test]
fn textarea_tag_looking_content_never_becomes_component_references() {
    let source = r#"<script>let value='text';</script><textarea><Missing/><!--literal--><script>literal</script><b>{value}</b></textarea>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("Literal.svelte"), target).unwrap();
        let allocator = oxc_allocator::Allocator::default();
        let parsed =
            oxc_parser::Parser::new(&allocator, &compiled.source, oxc_span::SourceType::tsx())
                .parse();
        assert!(parsed.errors.is_empty());
        let built = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
        assert!(!built.semantic.nodes().iter().any(|node| matches!(node.kind(), oxc_ast::AstKind::IdentifierReference(id) if id.name == "Missing")));
        assert!(compiled.source.contains("literal"));
    }
}

#[test]
fn text_values_use_target_specific_coercion_without_changing_jsx_children() {
    let source = r#"<script lang="ts">let value=$state.raw<unknown>(true);const String=()=>{throw new Error('shadowed')};function fn(){throw new Error('not a child accessor')}</script><p>{value}</p>{fn}<span>before{value}&copy;&amp;after</span>"#;
    let dom =
        compile_moonshine_svelte(source, Path::new("Text.svelte"), MoonshineModuleTarget::Dom)
            .unwrap();
    assert!(dom.source.contains("let text = \"\""));
    assert!(!dom.source.contains("globalThis.String"));
    assert!(dom.source.contains('©'));
    assert!(!dom.source.contains("&copy;"));
    let ssr =
        compile_moonshine_svelte(source, Path::new("Text.svelte"), MoonshineModuleTarget::Ssr)
            .unwrap();
    assert!(ssr.source.contains("scopeSSRChild as"));
    assert!(!ssr.source.contains("escapeHtml as"));
    assert!(!ssr.source.contains("defer as"));
    assert!(!ssr.source.contains("rawHTML as"));
    assert!(!ssr.source.contains("document.createTextNode"));
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for shadow in [
            "<script>const globalThis={value:true};</script><p>{globalThis.value}</p>",
            "<script module>const globalThis={value:true};</script><p>{globalThis.value}</p>",
        ] {
            assert!(compile_moonshine_svelte(shadow, Path::new("Shadow.svelte"), target).is_ok());
        }
    }
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let jsx = compile_moonshine_module(
            "export default function View(){return <p>{true}{()=>false}</p>}",
            Path::new("Text.tsx"),
            target,
        )
        .unwrap();
        assert!(!jsx.source.contains("__CrepusText"));
        assert!(!jsx.source.contains("globalThis.String"));
    }
}

#[test]
fn mixed_attributes_lower_generated_coercion_before_prop_analysis() {
    let source = r#"<script lang="ts">let value=$state.raw<unknown>(true); function first(){return 1} function second(){return value}</script><p title="x{first(),second()}y" data-value="{value}" data-comment="x{value // closing punctuation must survive
}"/>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Attrs.svelte"), target).unwrap();
        assert!(!compiled.source.contains("__crepusAttributeText"));
        assert_eq!(
            compiled.source.contains("`${"),
            matches!(target, MoonshineModuleTarget::Dom)
        );
    }
}

#[test]
fn authored_component_spreads_use_target_specific_prop_layers() {
    let source = r#"<script lang="ts">import Child from './Child.svelte';let props=$state.raw<Record<string,unknown>|null>({value:1});</script><Child before={1} {...props} middle="two" {...(props /* } */)} after={3}><Child {...null}/></Child>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("Spread.svelte"), target).unwrap();
        let helper = match target {
            MoonshineModuleTarget::Dom => "spreadProps as",
            MoonshineModuleTarget::Ssr => "spreadServerProps as",
        };
        assert!(compiled.source.contains(helper), "{}", compiled.source);
        assert!(!compiled.source.contains("__crepusSvelteSpread0"));
        assert!(
            compile_moonshine_svelte("<title {...props}/>", Path::new("Bad.svelte"), target).is_err()
        );
        assert!(compile_moonshine_module(
            "import Child from './Child';export default function App(){return <Child {...props}/>}",
            Path::new("Spread.tsx"),
            target
        )
        .is_err());
    }
}

#[test]
fn authored_component_bodies_are_callable_snippets_and_conflicts_still_error() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte("<script>import Child from './Child.svelte';let value=$state(1);</script><Child><p>{value}</p></Child>", Path::new("Children.svelte"), target).unwrap();
        assert!(compiled.source.contains("children: {"));
        assert!(compiled.source.contains("() =>"));
        assert!(compile_moonshine_svelte("<script>import Child from './Child.svelte';</script><Child children={snippet}><p/></Child>", Path::new("Conflict.svelte"), target).is_err());
    }
}

#[test]
fn component_value_bindings_keep_descriptors_and_server_setup_outside_settle() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let source = "<script>import Child from './Child.svelte';let value=$state(1);let props=$state({extra:2});globalThis.setup();</script><p>{value}</p><Child {...props} bind:value/><p>{value}</p>";
        let compiled =
            compile_moonshine_svelte(source, Path::new("Parent.svelte"), target).unwrap();
        assert!(!compiled.source.contains("__crepusComponentBindingProp"));
        assert_eq!(compiled.source.matches("globalThis.setup()").count(), 1);
        assert!(compiled.source.contains("set:"));
        assert_eq!(
            compiled.source.contains("settle as"),
            matches!(target, MoonshineModuleTarget::Ssr)
        );
        let child = compile_moonshine_svelte("<script lang='ts'>let {value:local=$bindable<number>(3)}:{value?:number}=$props();local++;</script><p>{local}</p>", Path::new("Child.svelte"), target).unwrap();
        assert!(!child.source.contains("$bindable"));
        assert!(child
            .source
            .contains(if matches!(target, MoonshineModuleTarget::Dom) {
                "createProp as"
            } else {
                "createServerProp as"
            }));
        assert_eq!(
            child.source.contains("afterRender as"),
            matches!(target, MoonshineModuleTarget::Ssr)
        );
        for invalid in [
            "<script>import Child from './Child.svelte';const value=1;</script><Child bind:value/>",
            "<script>import Child from './Child.svelte';let value;</script><Child bind:value={value?.x}/>",
            "<script>import Child from './Child.svelte';</script>{#snippet row(value)}<Child bind:value/>{/snippet}",
        ] {
            assert!(
                compile_moonshine_svelte(invalid, Path::new("Invalid.svelte"), target).is_err(),
                "{invalid}"
            );
        }
    }
}

#[test]
fn named_component_bindings_preserve_prop_names_and_intrinsic_boundaries() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let source = "<script>import Child from './Child.svelte';let open=$state(true);let checked=$state(false);let label=$state('name');</script><Child bind:open bind:checked bind:aria-label={label} bind:__proto__={label} {...props}/>";
        let compiled = compile_moonshine_svelte(source, Path::new("Named.svelte"), target).unwrap();
        for key in ["open", "checked", "aria-label", "__proto__"] {
            assert!(
                compiled.source.contains(&format!("[\"{key}\"]")),
                "{}",
                compiled.source
            );
        }
        assert!(!compiled.source.contains("__crepusComponentBindingProp"));
        for prop in [
            "ref",
            "children",
            "innerHTML",
            "outerHTML",
            "dangerouslySetInnerHTML",
        ] {
            let source = format!(
                "<script>import Child from './Child.svelte';let value=$state('name');</script><Child bind:{prop}={{value}}/>"
            );
            assert!(
                compile_moonshine_svelte(&source, Path::new("Ordinary.svelte"), target).is_ok(),
                "{prop}"
            );
        }
        for invalid in [
            "<input bind:open={value}/>",
            "<script>import Child from './Child.svelte';let value;</script><Child bind:children={value}><p/></Child>",
            "<script>import Child from './Child.svelte';</script>{#snippet row(value)}<Child bind:open={value}/>{/snippet}",
        ] {
            assert!(
                compile_moonshine_svelte(invalid, Path::new("Invalid.svelte"), target).is_err(),
                "{invalid}"
            );
        }
    }
}

#[test]
fn reserved_component_props_are_ordinary_without_relaxing_intrinsics_or_jsx() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for prop in [
            "ref",
            "innerHTML",
            "outerHTML",
            "dangerouslySetInnerHTML",
            "__proto__",
        ] {
            let source = format!(
                "<script>import Child from './Child.svelte';let value=$state('safe');</script><Child {prop}={{value}}/>"
            );
            let compiled =
                compile_moonshine_svelte(&source, Path::new("Props.svelte"), target).unwrap();
            if prop == "__proto__" {
                assert!(
                    compiled.source.contains("[\"__proto__\"]"),
                    "{}",
                    compiled.source
                );
            }
            let intrinsic = format!("<div {prop}={{value}}/>");
            assert!(
                compile_moonshine_svelte(&intrinsic, Path::new("Bad.svelte"), target).is_err(),
                "{prop}"
            );
            let jsx = format!(
                "import Child from './Child';export default function App(){{return <Child {prop}={{value}}/>}}"
            );
            assert!(
                compile_moonshine_module(&jsx, Path::new("Props.tsx"), target).is_err(),
                "{prop}"
            );
        }
    }
}

#[test]
fn pre_effects_and_preless_parents_use_dom_scheduler_only() {
    for source in [
        "<script>let count=$state(0);function factory(){return()=>count}$effect.pre(factory());</script><p>{count}</p>",
        "<script>import Child from './Child.svelte';let shown=$state(true);</script>{#if shown}<Child/>{/if}",
    ] {
        let dom = compile_moonshine_svelte(
            source,
            Path::new("Scheduled.svelte"),
            MoonshineModuleTarget::Dom,
        )
        .unwrap();
        assert!(dom.source.contains("withRuneScheduler"));
        assert!(
            dom.imports
                .iter()
                .any(|s| s == "@tschk/moonshine-svelte/pre")
        );
        let ssr = compile_moonshine_svelte(
            source,
            Path::new("Scheduled.svelte"),
            MoonshineModuleTarget::Ssr,
        )
        .unwrap();
        assert!(!ssr.source.contains("withRuneScheduler"));
        assert!(!ssr.source.contains("createRunePreEffect"));
        assert!(!ssr.source.contains("factory();"));
        assert!(
            !ssr.imports
                .iter()
                .any(|s| s == "@tschk/moonshine-svelte/pre")
        );
    }
    let generic = compile_moonshine_module(
        "export default function App(){return <p/>}",
        Path::new("App.tsx"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap();
    assert!(!generic.source.contains("withRuneScheduler"));
}

#[test]
fn transparent_effect_callees_preserve_registration_and_server_erasure() {
    let source = r#"<script lang="ts">
let count=$state(0);
function factory(){return()=>count}
($effect.pre as any)(factory());
($effect!)(factory());
const tracking=($effect.tracking as ()=>boolean)();
</script><p>{count}/{tracking}</p>"#;
    let dom = compile_moonshine_svelte(
        source,
        Path::new("Wrapped.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap();
    for helper in ["createRunePreEffect", "createRuneEffect", "runeTracking"] {
        assert!(dom.source.contains(helper), "{}", dom.source);
    }
    assert!(!dom.source.contains("$effect"));
    let ssr = compile_moonshine_svelte(
        source,
        Path::new("Wrapped.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap();
    assert!(!ssr.source.contains("factory();"));
    assert!(!ssr.source.contains("$effect"));
    assert!(!ssr.source.contains("createRuneEffect"));
    assert!(!ssr.source.contains("createRunePreEffect"));
}

#[test]
fn snapshot_callee_wrappers_and_trailing_commas_compile_for_both_hosts() {
    let source = r#"<script lang="ts">
let state=$state({n:1});
const first=$state.snapshot(state,);
const second=($state.snapshot as any)(state,);
const third=($state as any).snapshot(state);
const fourth=($state.snapshot)(/* head */state, // tail
);
const nested=$state.snapshot({copy:($state.snapshot)(state,)},);
function read(){return ($state /* clone */ .snapshot!)(state,)}
</script><p>{first.n}/{second.n}/{third.n}/{fourth.n}/{nested.copy.n}/{read().n}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("Snapshot.svelte"), target).unwrap();
        assert!(!compiled.source.contains("$state"));
        assert!(compiled.source.contains("stateSnapshot"));
    }
}

#[test]
fn component_function_bindings_capture_pairs_without_server_retry() {
    let source = r#"<script lang="ts">
import Child from './Child.svelte';
let status=$state('closed');
function get(){return status==='open'}
function set(value:boolean){status=value?'open':'closed'}
</script><i>{status}</i><Child bind:open={get,set}/><b>{status}</b>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Pair.svelte"), target).unwrap();
        assert!(compiled.source.contains("[\"open\"]"));
        assert!(!compiled.source.contains("__crepusComponentBindingProp"));
        if matches!(target, MoonshineModuleTarget::Ssr) {
            assert!(!compiled.source.contains("BindingSettle"));
            assert!(!compiled.source.contains("BindingInvalidate"));
        }
    }
    for invalid in ["(get,set)", "get,set,other", "makeGet(<Inner/>),set"] {
        let source = format!(
            "<script>import Child from './Child.svelte';const get=()=>true,set=()=>{{}},other=()=>{{}};</script><Child bind:open={{{invalid}}}/>"
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(compile_moonshine_svelte(&source, Path::new("Bad.svelte"), target).is_err());
        }
    }
}

#[test]
fn authored_ssr_conditionals_attach_branches_without_changing_dom_or_jsx() {
    let source = "<script>let mode = $state(0);</script>{#if mode === 0}<b>first</b>{:else if mode === 1}<i>second</i>{:else}<u>last</u>{/if}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("Choice.svelte"), target).unwrap();
        let server = matches!(target, MoonshineModuleTarget::Ssr);
        assert_eq!(
            compiled.source.contains("const active ="),
            server,
            "{}",
            compiled.source
        );
        assert_eq!(
            compiled.source.contains("Show as"),
            !server,
            "{}",
            compiled.source
        );
    }
    let generic = compile_moonshine_module(
        "export default function C(){return <main>{mode ? <b/> : <i/>}</main>}",
        Path::new("Choice.tsx"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap();
    assert!(generic.source.contains("Show as"), "{}", generic.source);
    assert!(!generic.source.contains("const active ="));
}

#[test]
fn authored_aside_supports_text_and_owned_raw_html() {
    let source = "<script>let label=$state('related');let markup=$state('<b>note</b>');</script><aside aria-label={label}>{label}</aside><aside>{@html markup}</aside>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Aside.svelte"), target).unwrap();
        assert!(output.source.contains("\"aside\""), "{}", output.source);
        assert!(
            output.source.contains("\"aria-label\""),
            "{}",
            output.source
        );
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(
                output.source.contains("htmlContent as"),
                "{}",
                output.source
            );
            assert!(!output.source.contains("rawHTML as"), "{}", output.source);
        } else {
            assert!(output.source.contains("rawHTML as"), "{}", output.source);
        }
    }
}

#[test]
fn authored_definition_lists_keep_raw_html_on_the_description_owner() {
    let source = "<script>let term=$state('Term');let description=$state('<b>Meaning</b>');</script><dl><dt>{term}</dt><dd>{@html description}</dd></dl>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("List.svelte"), target).unwrap();
        for tag in ["dl", "dt", "dd"] {
            assert!(
                output.source.contains(&format!("\"{tag}\"")),
                "{}",
                output.source
            );
        }
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(output.source.contains("htmlContent as"));
            assert!(!output.source.contains("rawHTML as"));
        } else {
            assert!(output.source.contains("rawHTML as"));
        }
    }
}

#[test]
fn type_only_rune_aliases_do_not_shadow_component_runtime_runes() {
    let source = "<script lang='ts'>type $state={value:number};let item:$state={value:1};let count=$state(1);</script><p>{item.value}/{count}</p>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Typed.svelte"), target).unwrap();
        assert!(output.source.contains("type $state"), "{}", output.source);
        assert!(output.source.contains("count.value"), "{}", output.source);
        assert!(!output.source.contains("$state(1)"), "{}", output.source);
    }
}

#[test]
fn source_state_and_derived_use_shared_server_helpers_with_hygienic_aliases() {
    let source = r#"<script lang="ts">
const __crepusSvelteCell = 'authored state alias';
const __crepusSvelteDerived = 'authored derived alias';
let count = $state(1,);
let raw = $state.raw({ n: 2 },);
let doubled = $derived(count * 2,);
let combined = $derived.by(() => doubled + raw.n,);
</script><button onclick={()=>{count++;raw={n:raw.n+1}}}>{count}/{combined}/{__crepusSvelteCell}/{__crepusSvelteDerived}</button>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output =
            compile_moonshine_svelte(source, Path::new("SharedHelpers.svelte"), target).unwrap();
        for rune in ["$state(", "$state.raw(", "$derived(", "$derived.by("] {
            assert!(!output.source.contains(rune), "{rune}: {}", output.source);
        }
        assert!(output.source.contains("const __crepusSvelteCell ="));
        assert!(output.source.contains("const __crepusSvelteDerived ="));
        if matches!(target, MoonshineModuleTarget::Ssr) {
            assert!(output.source.contains("@tschk/moonshine-svelte/ssr"));
            assert!(output.source.contains("createServerState as"));
            assert!(output.source.contains("createServerDerived as"));
            assert!(!output.source.contains("@tschk/moonshine-svelte/runes"));
            assert!(!output.source.contains("function __crepusSvelteCell"));
            assert!(!output.source.contains("function __crepusSvelteDerived"));
            let derived_alias = output
                .source
                .split("createServerDerived as ")
                .nth(1)
                .unwrap()
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                .next()
                .unwrap();
            assert_ne!(derived_alias, "__crepusSvelteDerived");
            let compact: String = output.source.split_whitespace().collect();
            assert_eq!(compact.matches(&format!("{derived_alias}(")).count(), 2);
            assert!(compact.matches(",true)").count() >= 2, "{}", output.source);
        } else {
            assert!(output.source.contains("@tschk/moonshine-svelte/runes"));
            assert!(output.source.contains("createState as"));
            assert!(output.source.contains("createRawState as"));
            assert!(output.source.contains("createDerived as"));
            assert!(!output.source.contains("createServerState"));
            assert!(!output.source.contains("createServerDerived"));
        }
    }
}

#[test]
fn companion_factory_server_derived_uses_shared_creation_sensitive_cache_default() {
    let source = r#"export function make() {
let count=$state(1,);let raw=$state.raw({n:2},);
const doubled=$derived(count*2,);const total=$derived.by(()=>doubled+raw.n,);
return {get value(){return total},increment(){count++;raw={n:raw.n+1}}};
}"#;
    let output = compile_moonshine_module(
        source,
        Path::new("counter.svelte.ts"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap();
    assert!(output.source.contains("@tschk/moonshine-svelte/ssr"));
    assert!(output.source.contains("createServerState as"));
    assert!(output.source.contains("createServerDerived as"));
    assert!(!output.source.contains("@tschk/moonshine-svelte/runes"));
    assert!(!output.source.contains("isSSRRendering"));
    assert!(!output.source.contains("getComponentContext"));
    assert!(!output.source.contains("function __crepusSvelteDerived"));
    let compact: String = output.source.split_whitespace().collect();
    assert!(!compact.contains(",true)"), "{}", output.source);
    for rune in ["$state(", "$state.raw(", "$derived(", "$derived.by("] {
        assert!(!output.source.contains(rune), "{rune}: {}", output.source);
    }
}

#[test]
fn no_argument_component_state_initializers_ignore_authored_undefined_bindings() {
    let source = r#"<script lang="ts">
const undefined=7;
let empty=$state();let raw=$state.raw();
let typed=$state<number>();let typedRaw=$state.raw<number>();
</script><p>{empty}/{raw}/{typed}/{typedRaw}/{undefined}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output =
            compile_moonshine_svelte(source, Path::new("EmptyCells.svelte"), target).unwrap();
        let compact: String = output.source.split_whitespace().collect();
        assert!(compact.contains("constundefined=7;"), "{}", output.source);
        assert_eq!(compact.matches("(void0)").count(), 4, "{}", output.source);
        assert!(
            compact.matches("|undefined>(void0)").count() >= 2,
            "{}",
            output.source
        );
        assert!(!compact.contains("(undefined)"), "{}", output.source);
        assert!(!output.source.contains("$state("));
        assert!(!output.source.contains("$state.raw("));
    }
}

#[test]
fn no_argument_companion_state_initializers_ignore_authored_undefined_bindings() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for extension in ["ts", "js"] {
            let typed = if extension == "ts" {
                "let typed=$state<number>();let typedRaw=$state.raw<number>();"
            } else {
                ""
            };
            let source = format!(
                "export function make(){{const undefined=7;let empty=$state();let raw=$state.raw();{typed}return{{get empty(){{return empty}},get raw(){{return raw}},authored:undefined}}}}"
            );
            let output = compile_moonshine_module(
                &source,
                Path::new(&format!("empty.svelte.{extension}")),
                target,
            )
            .unwrap();
            let compact: String = output.source.split_whitespace().collect();
            assert!(compact.contains("constundefined=7;"), "{}", output.source);
            assert!(compact.contains("authored:undefined"), "{}", output.source);
            assert_eq!(
                compact.matches("(void0)").count(),
                if extension == "ts" { 4 } else { 2 },
                "{}",
                output.source
            );
            if extension == "ts" {
                assert!(
                    compact.matches("|undefined>(void0)").count() >= 2,
                    "{}",
                    output.source
                );
            }
            assert!(!compact.contains("(undefined)"), "{}", output.source);
            assert!(!output.source.contains("$state("));
            assert!(!output.source.contains("$state.raw("));
        }
    }
}

fn assert_named_snippet_output_parses(source: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::tsx()).parse();
    assert!(parsed.errors.is_empty(), "{:?}\n{source}", parsed.errors);
    assert!(!source.contains("{#snippet"));
}

#[test]
fn named_snippet_occurrences_capture_the_parent_owner() {
    let source = "<script>import Child from './Child.svelte';</script><Child>{#snippet body(item)}<p>{item}</p>{/snippet}</Child>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Owned.svelte"), target).unwrap();
        assert_named_snippet_output_parses(&compiled.source);
        assert!(
            compiled.source.contains("__crepusOnce0(() => (() => {"),
            "{}",
            compiled.source
        );
        assert!(compiled.source.contains("__crepusRunWithOwner0(owner"));
    }
}

#[test]
fn component_child_named_snippets_are_occurrence_local_and_keep_live_parameters() {
    let source = r#"<script>import Child from './Child.svelte';let count=$state(1);</script>
{#snippet row(item)}<i>root:{item}</i>{/snippet}
{@render row(count)}
<Child selected={row}>{#snippet row(item)}<b>one:{item}/{count}</b>{/snippet}</Child>
<Child>{#snippet row(item)}<em>two:{item}</em>{/snippet}</Child>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_svelte(source, Path::new("Named.svelte"), target).unwrap();
        assert_named_snippet_output_parses(&compiled.source);
        assert!(compiled.imports.iter().any(|path| path == "./Child.svelte"));
        assert!(compiled.source.contains("selected:"));
        assert!(compiled.source.contains("row:"));
        for literal in ["root:", "one:", "two:"] {
            assert!(compiled.source.contains(literal), "{}", compiled.source);
        }
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(compiled.source.contains("item.value"));
        }
    }
}

#[test]
fn named_only_component_bodies_do_not_create_implicit_children() {
    for body in [
        " \n <!-- ignored --> {#snippet row(item)}<p>{item}</p>{/snippet} \n ",
        "{#snippet children(item)}<p>{item}</p>{/snippet}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = format!(
                "<script>import Child from './Child.svelte';</script><Child>{body}</Child>"
            );
            let compiled =
                compile_moonshine_svelte(&source, Path::new("Named.svelte"), target).unwrap();
            assert_named_snippet_output_parses(&compiled.source);
            if body.contains("snippet children") {
                assert!(compiled.source.contains("children: {"));
            } else {
                assert!(compiled.source.contains("row:"));
                assert!(
                    !compiled.source.contains("children: {"),
                    "{}",
                    compiled.source
                );
            }
        }
    }
}

#[test]
fn named_snippets_coexist_with_ordinary_body_nested_occurrences_and_script_shadowing() {
    for markup in [
        "<Child>{#snippet row(item)}<p>{item}</p>{/snippet}<strong>body</strong></Child>",
        "<Child>{#snippet row(item)}<Child>{#snippet row(other)}<b>{item}/{other}</b>{/snippet}</Child>{/snippet}</Child>",
        "<Child selected={row}>{#snippet row(item)}<p>{item}</p>{/snippet}</Child>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = format!(
                "<script>import Child from './Child.svelte';const row='script';</script>{markup}"
            );
            let compiled =
                compile_moonshine_svelte(&source, Path::new("Named.svelte"), target).unwrap();
            assert_named_snippet_output_parses(&compiled.source);
            assert!(compiled.source.contains("row:"));
            if markup.contains("<strong>") {
                assert!(compiled.source.contains("children: {"));
            }
        }
    }
}

#[test]
fn component_child_named_snippet_conflicts_and_unsupported_nesting_fail() {
    // Empty nested declarations have no runtime body and are admitted by the existing compiler.
    for markup in [
        "<div>{#snippet row()}{/snippet}</div>",
        "<Child><section>{#snippet row()}{/snippet}</section></Child>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = format!(
                "<script>import Child from './Child.svelte';let value=$state(1);</script>{markup}"
            );
            let compiled =
                compile_moonshine_svelte(&source, Path::new("Empty.svelte"), target).unwrap();
            assert_named_snippet_output_parses(&compiled.source);
            assert!(!compiled.source.contains("createRawSnippet"));
        }
    }
    for markup in [
        "{#if true}{#snippet row()}{/snippet}{/if}",
        "{#key value}{#snippet row()}{/snippet}{/key}",
        "{#snippet outer()}{#snippet inner()}{/snippet}{/snippet}",
        "<Child row={value}>{#snippet row()}{/snippet}</Child>",
        "<Child bind:row={value}>{#snippet row()}{/snippet}</Child>",
        "<Child>{#snippet row()}{/snippet}{#snippet row()}{/snippet}</Child>",
        "<Child>{#snippet children()}{/snippet}<p>body</p></Child>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = format!(
                "<script>import Child from './Child.svelte';let value=$state(1);</script>{markup}"
            );
            assert!(
                compile_moonshine_svelte(&source, Path::new("Invalid.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn named_snippet_parameters_cannot_be_direct_binding_targets_but_members_can() {
    for body in [
        "<input bind:value={item}/>",
        "<Child bind:open={item}/>",
        "<Child bind:this={item}/>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = format!(
                "<script>import Child from './Child.svelte';</script><Child>{{#snippet row(item)}}{body}{{/snippet}}</Child>"
            );
            assert!(
                compile_moonshine_svelte(&source, Path::new("Invalid.svelte"), target).is_err(),
                "{source}"
            );
        }
    }
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let source = "<script>import Child from './Child.svelte';</script><Child>{#snippet row(item)}<input bind:value={item.text}/><Child bind:open={item.open}/>{/snippet}</Child>";
        let compiled =
            compile_moonshine_svelte(source, Path::new("Members.svelte"), target).unwrap();
        assert_named_snippet_output_parses(&compiled.source);
    }
}

fn conditional_memo_imports(source: &str) -> Vec<(String, String)> {
    use oxc_ast::ast::{ImportDeclarationSpecifier, Statement};
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::tsx()).parse();
    assert!(parsed.errors.is_empty(), "{:?}\n{source}", parsed.errors);
    let mut imports = Vec::new();
    for statement in &parsed.program.body {
        let Statement::ImportDeclaration(declaration) = statement else {
            continue;
        };
        for specifier in declaration.specifiers.iter().flatten() {
            let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                continue;
            };
            if specifier.local.name.starts_with("__crepusMemo") {
                imports.push((
                    declaration.source.value.to_string(),
                    specifier.imported.name().to_string(),
                ));
            }
        }
    }
    imports
}

#[test]
fn authored_dom_conditional_memos_are_scheduled_without_changing_ssr_or_jsx() {
    let source = r#"<script>import Child from './Child.svelte';let box=$state({child:{child:true}});</script>
{#if box.child}<Child node={box.child}/>{:else}<p>empty</p>{/if}"#;
    let child = r#"<script>let {node}=$props();</script>{#if node.child && node}<b>present</b>{:else}<i>missing</i>{/if}"#;
    for source in [source, child] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let compiled =
                compile_moonshine_svelte(source, Path::new("Conditional.svelte"), target).unwrap();
            let imports = conditional_memo_imports(&compiled.source);
            assert!(!imports.is_empty(), "{}", compiled.source);
            let expected = if matches!(target, MoonshineModuleTarget::Dom) {
                ("@tschk/moonshine-dom/reactive", "createRenderMemo")
            } else {
                ("@tschk/moonshine-dom/ssr", "createMemo")
            };
            assert!(
                imports
                    .iter()
                    .all(|(path, name)| path == expected.0 && name == expected.1),
                "{imports:?}\n{}",
                compiled.source
            );
        }
    }
    let jsx = "export default function View(props){return <main>{props.node ? <b/> : <i/>}{props.node && <span/>}</main>}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled = compile_moonshine_module(jsx, Path::new("View.tsx"), target).unwrap();
        let imports = conditional_memo_imports(&compiled.source);
        assert!(!imports.is_empty());
        assert!(
            imports.iter().all(|(_, name)| name == "createMemo"),
            "{imports:?}"
        );
    }
}

fn generated_presence_literal<'a>(
    expression: &'a oxc_ast::ast::Expression<'a>,
    semantic: &'a oxc_semantic::Semantic<'a>,
) -> Option<bool> {
    use oxc_ast::{ast::Expression, AstKind};
    match expression.get_inner_expression() {
        Expression::BooleanLiteral(value) if value.value => Some(true),
        Expression::StringLiteral(value) if value.value.is_empty() => Some(false),
        Expression::ArrowFunctionExpression(arrow) => {
            generated_presence_literal(arrow.get_expression()?, semantic)
        }
        Expression::CallExpression(call) => {
            if let Expression::ArrowFunctionExpression(arrow) = call.callee.get_inner_expression() {
                return generated_presence_literal(arrow.get_expression()?, semantic);
            }
            None
        }
        Expression::Identifier(id) => {
            let symbol = semantic
                .scoping()
                .get_reference(id.reference_id.get()?)
                .symbol_id()?;
            semantic.nodes().iter().find_map(|node| {
                if let AstKind::VariableDeclarator(declaration) = node.kind() {
                    if let oxc_ast::ast::BindingPattern::BindingIdentifier(id) = &declaration.id {
                        if id.symbol_id.get() == Some(symbol) {
                            return declaration
                                .init
                                .as_ref()
                                .and_then(|value| generated_presence_literal(value, semantic));
                        }
                    }
                    return None;
                }
                let AstKind::AssignmentExpression(assignment) = node.kind() else {
                    return None;
                };
                let oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(target) =
                    &assignment.left
                else {
                    return None;
                };
                if semantic
                    .scoping()
                    .get_reference(target.reference_id.get()?)
                    .symbol_id()
                    != Some(symbol)
                {
                    return None;
                }
                match assignment.right.get_inner_expression() {
                    Expression::Identifier(_) => None,
                    value => generated_presence_literal(value, semantic),
                }
            })
        }
        _ => None,
    }
}
fn generated_attribute_imports(
    program: &oxc_ast::ast::Program<'_>,
) -> std::collections::HashSet<oxc_syntax::symbol::SymbolId> {
    generated_function_imports(program, "@tschk/moonshine-dom/dom", "setAttributeValue")
}

fn generated_function_imports(
    program: &oxc_ast::ast::Program<'_>,
    module: &str,
    name: &str,
) -> std::collections::HashSet<oxc_syntax::symbol::SymbolId> {
    program
        .body
        .iter()
        .filter_map(|statement| {
            let oxc_ast::ast::Statement::ImportDeclaration(import) = statement else {
                return None;
            };
            if import.source.value != module {
                return None;
            }
            Some(import.specifiers.iter().flatten().filter_map(|specifier| {
                let oxc_ast::ast::ImportDeclarationSpecifier::ImportSpecifier(specifier) =
                    specifier
                else {
                    return None;
                };
                (specifier.imported.name() == name)
                    .then(|| specifier.local.symbol_id.get())
                    .flatten()
            }))
        })
        .flatten()
        .collect()
}

#[test]
fn authored_intrinsic_bare_attributes_keep_svelte_presence_values() {
    let source = r#"<script>import Child from './Child.svelte';let flag=$state(true);const onclick=()=>{};</script>
<input disabled required data-marker aria-label checked defaultChecked defaultValue checked-value={true}/>
<input checked="false"/><input checked=""/><input checked={flag}/><input checked="{flag}"/>
<button {onclick}>click</button><button onclick="{onclick}">quoted</button><Child enabled onready text="literal"/>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let compiled =
            compile_moonshine_svelte(source, Path::new("Presence.svelte"), target).unwrap();
        assert_named_snippet_output_parses(&compiled.source);
        let allocator = oxc_allocator::Allocator::default();
        let parsed =
            oxc_parser::Parser::new(&allocator, &compiled.source, oxc_span::SourceType::tsx())
                .parse();
        let semantic = oxc_semantic::SemanticBuilder::new()
            .build(&parsed.program)
            .semantic;
        let mut empty_keys = std::collections::HashSet::new();
        let mut true_keys = std::collections::HashSet::new();
        let attribute_imports = generated_attribute_imports(&parsed.program);
        for node in semantic.nodes().iter() {
            let (key, value) = match node.kind() {
                oxc_ast::AstKind::ObjectProperty(property) => {
                    let Some(key) = property.key.static_name() else {
                        continue;
                    };
                    (key.to_string(), &property.value)
                }
                oxc_ast::AstKind::CallExpression(call) => {
                    let oxc_ast::ast::Expression::Identifier(callee) =
                        call.callee.get_inner_expression()
                    else {
                        continue;
                    };
                    let symbol = semantic
                        .scoping()
                        .get_reference(callee.reference_id.get().unwrap())
                        .symbol_id();
                    if !symbol.is_some_and(|symbol| attribute_imports.contains(&symbol)) {
                        continue;
                    }
                    let Some(oxc_ast::ast::Argument::StringLiteral(key)) = call.arguments.get(1)
                    else {
                        continue;
                    };
                    let Some(value) = call
                        .arguments
                        .get(2)
                        .and_then(|value| value.as_expression())
                    else {
                        continue;
                    };
                    (key.value.to_string(), value)
                }
                _ => continue,
            };
            match generated_presence_literal(value, &semantic) {
                Some(true) => {
                    true_keys.insert(key);
                }
                Some(false) => {
                    empty_keys.insert(key);
                }
                None => {}
            }
        }
        for key in ["disabled", "required", "data-marker", "aria-label"] {
            assert!(
                empty_keys.contains(key),
                "missing empty presence value {key}: {}",
                compiled.source
            );
        }
        for key in ["checked", "defaultChecked", "defaultValue", "checked-value"] {
            assert!(
                true_keys.contains(key),
                "missing true property {key}: {}",
                compiled.source
            );
        }
        assert!(compiled.source.contains("flag.value"));
        assert!(compiled.source.contains("enabled:"));
        assert!(compiled.source.contains("onready:"));
    }
}

#[test]
fn authored_intrinsic_static_event_attributes_fail_without_changing_component_props_or_jsx() {
    for markup in [
        "<button onclick/>",
        "<button onclick=\"handler\"/>",
        "<input oninput=\"\"/>",
        "<button onclick=\"text{handler}\"/>",
        "<button onclick=\"{handler}{other}\"/>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_svelte(markup, Path::new("Event.svelte"), target).is_err(),
                "{markup}"
            );
        }
    }
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let component = compile_moonshine_svelte("<script>import Child from './Child.svelte';</script><Child onclick oninput=\"literal\"/>", Path::new("Props.svelte"), target).unwrap();
        assert_named_snippet_output_parses(&component.source);
        let jsx = compile_moonshine_module(
            "export default function View(){return <input disabled data-marker/>}",
            Path::new("View.tsx"),
            target,
        )
        .unwrap();
        assert!(jsx.source.contains("disabled: true"));
        assert!(jsx.source.contains("\"data-marker\": true"));
    }
}

#[test]
fn authored_bare_value_exceptions_are_limited_to_textarea_select_and_option() {
    for (tag, expected_true) in [
        ("textarea", true),
        ("select", true),
        ("option", true),
        ("input", false),
        ("progress", false),
        ("meter", false),
        ("li", false),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = if tag == "input" {
                "<input value/>".to_string()
            } else {
                format!("<{tag} value></{tag}>")
            };
            let compiled =
                compile_moonshine_svelte(&source, Path::new("Value.svelte"), target).unwrap();
            let allocator = oxc_allocator::Allocator::default();
            let parsed =
                oxc_parser::Parser::new(&allocator, &compiled.source, oxc_span::SourceType::tsx())
                    .parse();
            assert!(parsed.errors.is_empty());
            let semantic = oxc_semantic::SemanticBuilder::new()
                .build(&parsed.program)
                .semantic;
            let attribute_imports = generated_attribute_imports(&parsed.program);
            let option_imports = generated_function_imports(
                &parsed.program,
                "@tschk/moonshine-svelte/binding",
                "setOptionValue",
            );
            let found = semantic.nodes().iter().any(|node| {
                if tag == "textarea" && matches!(target, MoonshineModuleTarget::Dom) {
                    if let oxc_ast::AstKind::CallExpression(call) = node.kind() {
                        if call.callee.is_specific_id("__crepusTextareaUpdate") {
                            return matches!(call.arguments.get(1), Some(oxc_ast::ast::Argument::ArrowFunctionExpression(arrow)) if matches!(arrow.get_expression().map(|value|value.get_inner_expression()), Some(oxc_ast::ast::Expression::BooleanLiteral(value)) if value.value));
                        }
                    }
                }
                if let oxc_ast::AstKind::CallExpression(call) = node.kind() {
                    if let oxc_ast::ast::Expression::Identifier(callee) = call.callee.get_inner_expression() {
                        let symbol = semantic.scoping().get_reference(callee.reference_id.get().unwrap()).symbol_id();
                        if tag == "option" && symbol.is_some_and(|symbol| option_imports.contains(&symbol)) {
                            return call.arguments.get(1).and_then(|value|value.as_expression()).and_then(|value| generated_presence_literal(value, &semantic)) == Some(expected_true);
                        }
                        if symbol.is_some_and(|symbol| attribute_imports.contains(&symbol))
                            && matches!(call.arguments.get(1), Some(oxc_ast::ast::Argument::StringLiteral(key)) if key.value == "value") {
                            return call.arguments.get(2).and_then(|value|value.as_expression()).and_then(|value| generated_presence_literal(value, &semantic)) == Some(expected_true);
                        }
                    }
                }
                let oxc_ast::AstKind::ObjectProperty(property) = node.kind() else { return false; };
                if !matches!(&property.key, oxc_ast::ast::PropertyKey::StaticIdentifier(key) if key.name=="value") { return false; }
                let mut value = property.value.get_inner_expression();
                if let oxc_ast::ast::Expression::CallExpression(call) = value {
                    if matches!(target, MoonshineModuleTarget::Dom)
                        && call.callee.is_specific_id("__crepusTextareaMemo")
                    {
                        let Some(oxc_ast::ast::Argument::ArrowFunctionExpression(arrow)) = call.arguments.first() else { return false; };
                        let Some(expression) = arrow.get_expression() else { return false; };
                        value = expression.get_inner_expression();
                    }
                }
                if let oxc_ast::ast::Expression::ArrowFunctionExpression(arrow) = value {
                    if let Some(expression) = arrow.get_expression() { value = expression.get_inner_expression(); }
                }
                if expected_true { matches!(value, oxc_ast::ast::Expression::BooleanLiteral(value) if value.value) }
                else { matches!(value, oxc_ast::ast::Expression::StringLiteral(value) if value.value.is_empty()) }
            });
            assert!(found, "{tag}: {}", compiled.source);
        }
    }
}

#[test]
fn static_date_time_and_color_value_bindings_remain_supported() {
    for kind in ["date", "datetime-local", "month", "week", "time", "color"] {
        let source =
            format!("<script>let value=$state('');</script><input type=\"{kind}\" bind:value/>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output =
                compile_moonshine_svelte(&source, Path::new("Dates.svelte"), target).unwrap();
            assert_named_snippet_output_parses(&output.source);
            assert!(
                output.source.contains(&format!("\"{kind}\"")),
                "{}",
                output.source
            );
            assert_eq!(
                output.source.contains("bindValue"),
                matches!(target, MoonshineModuleTarget::Dom)
            );
        }
    }
}
