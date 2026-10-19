#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;
#[test]
fn store_reads_replacement_and_finalization_use_native_owned_reference() {
    let source = r#"<script lang="ts">import { shared, replacement } from './stores'; let current=$state.raw(shared); const api={replace:()=>current=replacement};</script><p>{$current.count}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Stores.svelte"), target)
            .unwrap()
            .source;
        assert!(
            output.contains("@tschk/moonshine-svelte/store-binding"),
            "{output}"
        );
        assert!(
            output.contains(".releaseIfChanged(current.value = replacement)"),
            "{output}"
        );
        assert!(output.contains(".get(current.value)"), "{output}");
        assert!(output.contains(".finish()"), "{output}");
        assert!(!output.contains("$current"), "{output}");
    }
}
#[test]
fn unsupported_store_mutations_and_local_store_lifetimes_are_rejected() {
    for script in [
        "({count:$current.count}=other)",
        "[$current.count]=other",
        "delete $current.count",
        "current++",
        "current ||= other",
    ] {
        let source=format!("<script>let current=$state.raw(shared); function edit(){{{script};}}</script><p>{{$current.count}}</p>");
        assert!(
            compile_moonshine_svelte(&source, Path::new("Bad.svelte"), MoonshineModuleTarget::Dom)
                .is_err(),
            "{script}"
        );
    }
    let source =
        "<script>let current=store; function replace(){current=other}</script><p>{$current}</p>";
    assert!(compile_moonshine_svelte(
        source,
        Path::new("Plain.svelte"),
        MoonshineModuleTarget::Dom
    )
    .unwrap_err()
    .contains("requires a $state"));
    let source =
        "<script>function read(){const local=store; return $local;}</script><p>{read()}</p>";
    assert!(compile_moonshine_svelte(
        source,
        Path::new("Local.svelte"),
        MoonshineModuleTarget::Dom
    )
    .is_err());
}
#[test]
fn existing_store_reference_fixture_compiles_both_targets() {
    let source = include_str!("fixtures/store-reference.svelte");
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("store-reference.svelte"), target)
            .unwrap()
            .source;
        assert!(output.contains(".finish()"));
    }
}

#[test]
fn lexical_subscription_name_shadowing_is_not_rewritten() {
    let source = "<script lang='ts'>import {current} from './stores'; function read($current:number){return $current;} const data={$current};</script><p>{$current.count}/{read(3)}</p>";
    let output = compile_moonshine_svelte(
        source,
        Path::new("Shadow.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    assert!(output.contains("read($current: number)"), "{output}");
    assert!(output.contains("return $current"), "{output}");
    assert!(
        output.contains("$current: __crepusStore0.get(current)"),
        "{output}"
    );
}

#[test]
fn prop_stores_lower_before_prop_cells_and_stable_store_sources_remain_supported() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let source = "<script lang='ts'>let {current}:{current:{subscribe(run:(value:{count:number})=>void):()=>void}}=$props();</script><p>{$current.count}</p>";
        let output = compile_moonshine_svelte(source, Path::new("PropsStore.svelte"), target)
            .unwrap()
            .source;
        assert!(output.contains(".get(current.value)"), "{output}");
        assert!(output.contains("=> current.value"), "{output}");
        for script in [
            "import {current} from './stores';",
            "import {store} from './stores'; const current=store;",
            "import {store} from './stores'; let current=$state.raw(store);",
        ] {
            let source = format!("<script>{script}</script><p>{{$current.count}}</p>");
            assert!(
                compile_moonshine_svelte(&source, Path::new("Stable.svelte"), target)
                    .unwrap()
                    .source
                    .contains(".finish()")
            );
        }
    }
}

#[test]
fn server_store_completion_wraps_bindable_commit_after_serialization() {
    let source="<script>import {store} from './store';let {count=$bindable(0)}=$props();const value=$store;count=1;</script><p>{count}/{value}</p>";
    let output = compile_moonshine_svelte(
        source,
        Path::new("BoundStore.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap()
    .source;
    assert!(
        output.contains("createServerStoreReferences as"),
        "{output}"
    );
    assert!(
        output.find("__crepusServerStores.finish()").unwrap()
            < output.find("count.commit()").unwrap(),
        "{output}"
    );
    assert!(!output.contains("createStoreReference as"), "{output}");
}

#[test]
fn server_store_completion_after_own_settlement_precedes_own_binding_commit() {
    let source="<script>import Child from './Child.svelte';import {store} from './store';let {value=$bindable()}=$props();const read=$store;</script><Child bind:value/><p>{read}</p>";
    let output = compile_moonshine_svelte(
        source,
        Path::new("Middle.svelte"),
        MoonshineModuleTarget::Ssr,
    )
    .unwrap()
    .source;
    assert!(output.contains("settle as"), "{output}");
    assert!(
        output.find("__crepusServerStores.finish()").unwrap()
            < output.find("value.commit()").unwrap(),
        "{output}"
    );
    assert!(
        !output.contains("return __crepusStoreAfterRender("),
        "{output}"
    );
}

#[test]
fn writable_store_expressions_preserve_target_specific_mutation_contract() {
    let source = r#"<script lang="ts">import {current,key} from './stores'; function write(){const direct=($current=2);const compound=($current+=3);const logical=($current ||= 4);const post=$current++;const pre=--$current;const member=($current[key()]+=5);const memberPost=$current.count++;return [direct,compound,logical,post,pre,member,memberPost];}</script><p>{$current.count}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Writable.svelte"), target)
            .unwrap()
            .source;
        assert!(output.contains("setStoreValue as"), "{output}");
        assert!(output.contains("updateStoreValue as"), "{output}");
        assert!(!output.contains("$current"), "{output}");
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(output.contains("mutateStoreValue as"), "{output}");
            assert!(output.contains("untrack as"), "{output}");
        } else {
            assert!(output.contains("mutateServerStore as"), "{output}");
            assert!(!output.contains("untrack as"), "{output}");
        }
    }
}
#[test]
fn computed_store_keys_remain_reads_and_lexical_write_shadows_remain_authored() {
    let source="<script>import {current,key} from './stores';function edit($current){$current++;} function write(){return $current[$key]+=1;}</script><p>{$current.count}</p>";
    let output =
        compile_moonshine_svelte(source, Path::new("Keys.svelte"), MoonshineModuleTarget::Dom)
            .unwrap()
            .source;
    assert!(output.contains("$current++"), "{output}");
    assert!(output.contains(".get(key)"), "{output}");
    assert!(output.contains("mutateStoreValue as"), "{output}");
}

#[test]
fn nested_writable_expressions_close_inner_mutation_before_outer_assignment() {
    let source="<script>import {number,object} from './stores';function write(){return $number=$object.count+=2;}</script><p>{$number}</p>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Nested.svelte"), target)
            .unwrap()
            .source;
        assert!(!output.contains("count += 2),"), "{output}");
    }
}

#[test]
fn store_operators_comments_and_unsupported_write_targets_are_explicit() {
    for operator in [
        "=", "+=", "-=", "*=", "/=", "%=", "**=", "<<=", ">>=", ">>>=", "|=", "^=", "&=", "||=",
        "&&=", "??=",
    ] {
        let source=format!("<script>import {{current}} from './stores';function edit(){{return $current /*store*/ {operator} /*payload*/ 2;}}</script><p>{{$current}}</p>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_svelte(&source, Path::new("Operators.svelte"), target)
                .unwrap()
                .source;
            assert!(output.contains("/*payload*/"), "{operator}: {output}");
        }
    }
    let output=compile_moonshine_svelte("<script>import {current} from './stores';function edit(){return ++/*update*/$current;}</script><p>{$current}</p>",Path::new("Comment.svelte"),MoonshineModuleTarget::Dom).unwrap().source;
    assert!(output.contains("updateStoreValue as"), "{output}");
    for statement in [
        "for($current of values){}",
        "for($current.count of values){}",
        "[$current]=values",
        "({value:$current}=other)",
        "delete $current.count",
    ] {
        let source=format!("<script>import {{current}} from './stores';function edit(){{{statement}}}</script><p>{{$current.count}}</p>");
        assert!(
            compile_moonshine_svelte(
                &source,
                Path::new("Unsupported.svelte"),
                MoonshineModuleTarget::Dom
            )
            .is_err(),
            "{statement}"
        );
    }
}
