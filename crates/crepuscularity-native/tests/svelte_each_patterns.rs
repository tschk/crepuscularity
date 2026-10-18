#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;
fn compile(body: &str, target: MoonshineModuleTarget) -> Result<String, String> {
    let source = format!("<script lang=\"ts\">let rows=$state([{{id:'a',name:'one',nested:{{label:'value'}},callback:()=>1}}]);</script>{{#each rows as {body}{{/each}}");
    compile_moonshine_svelte(&source, Path::new("EachPattern.svelte"), target)
        .map(|module| module.source)
}
#[test]
fn flat_aliases_keep_typed_accessors_and_server_snapshot_callbacks() {
    let body="{id,name:label,nested}, index (id)}<input bind:value={nested.label}/><p>{index}:{label}:{label}:{id}</p>";
    let dom = compile(body, MoonshineModuleTarget::Dom)
        .expect("flat object item and member binding must compile");
    assert!(dom.contains("__crepusEachFields"), "{dom}");
    assert!(dom.contains("__crepusEachItem"), "{dom}");
    assert!(!dom.contains("SnapshotChildren"), "{dom}");
    let server =
        compile(body, MoonshineModuleTarget::Ssr).expect("server flat item must snapshot once");
    assert!(server.contains("SnapshotChildren"), "{server}");
    assert!(!server.contains("__crepusEachFields"), "{server}");
    assert!(!server.contains("keyedEach"), "{server}");
}
#[test]
fn readonly_alias_and_index_assignments_and_direct_bindings_fail_closed() {
    for body in [
        "{name:label}, index (index)}<button onclick={()=>label='x'}>write</button>",
        "{name:label}, index (index)}<button onclick={()=>index++}>write</button>",
        "{name:label}, index (index)}<button onclick={()=>({label}={label:'x'})}>write</button>",
        "{name:label}, index (index)}<input bind:value={label}/>",
        "{name:label}, index (index)}<input bind:value={(label)}/>",
        "{name:label}, index (index)}<input bind:value={index}/>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile(body, target)
                .expect_err("readonly each binding must not silently become writable");
            assert!(error.contains("each bindings"), "{body}: {error}");
        }
    }
}
#[test]
fn computed_and_numeric_patterns_remain_explicit_diagnostics() {
    for item in ["{[name]:label}", "{0:label}"] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile(&format!("{item}, index (index)}}<p>value</p>"), target)
                .expect_err("unsupported patterns must not be accepted");
            assert!(error.contains("unsupported"), "{item}: {error}");
        }
    }
}
#[test]
fn flat_callback_calls_and_own_proto_shorthand_preserve_javascript_semantics() {
    let source="<script lang=\"ts\">let rows=$state.raw([{id:'a',['__proto__']:'own',callback:function(){return this===undefined}}]);</script>{#each rows as {id,__proto__,callback}, index (id)}<p>{callback()}:{Object.hasOwn({__proto__},'__proto__')}:{__proto__}:{index}</p>{/each}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Proto.svelte"), target)
            .expect("own proto fields remain ordinary data")
            .source;
        assert!(output.contains("[\"__proto__\"]:"), "{output}");
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(output.contains("(0, __crepusEachFields"), "{output}");
        }
    }
}

#[test]
fn decoded_each_binding_does_not_capture_nested_each_helper() {
    let source = r"<script>const rows=[{value:1}];</script>{#each rows as {value:\u005f_CrepusEachBlock0}}{#each rows as row}<p>{row.value}</p>{/each}{/each}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("EachHygiene.svelte"), target)
            .unwrap()
            .source;
        assert!(!output.contains("function __CrepusEachBlock0<"), "{output}");
        assert!(!output.contains("function __CrepusEachBlock0("), "{output}");
    }
}

#[test]
fn decoded_header_bindings_do_not_capture_each_callback_helpers() {
    for (binding, helper) in [
        ("__crepusEachChildren", "__crepusEachChildren"),
        (
            "__crepusEachSnapshotChildren",
            "__crepusEachSnapshotChildren",
        ),
        ("__crepusUnusedEachIndex", "__crepusUnusedEachIndex"),
    ] {
        let escaped = format!("\\u005f{}", &binding[1..]);
        let source = format!("<script>const rows=[{{value:1}}];</script>{{#each rows as {{value:{escaped}}}}}{{#each rows as {{value:inner}}}}<p>{{inner}}</p>{{/each}}{{/each}}");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output =
                compile_moonshine_svelte(&source, Path::new("HeaderHelpers.svelte"), target)
                    .expect("decoded header bindings must not collide with compiler callbacks")
                    .source;
            // Destructured SSR emits snapshot callbacks; DOM emits live callbacks.
            let emitted = match helper {
                "__crepusEachChildren" => matches!(target, MoonshineModuleTarget::Dom),
                "__crepusEachSnapshotChildren" => matches!(target, MoonshineModuleTarget::Ssr),
                _ => true,
            };
            if emitted {
                assert!(output.contains(&format!("{helper}_")), "{output}");
            }
            assert!(!output.contains(&format!("function {helper}<")), "{output}");
        }
    }
}

#[test]
fn decoded_script_names_do_not_capture_each_projection_locals() {
    for binding in ["__crepusEachItem0", "__crepusEachFields1"] {
        let escaped = format!("\\u005f{}", &binding[1..]);
        let source = format!("<script>const {escaped}=1;const rows=[{{value:1}}];</script>{{#each rows as {{value:label}}}}<p>{{{escaped}}}</p>{{/each}}");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output =
                compile_moonshine_svelte(&source, Path::new("ProjectionHelpers.svelte"), target)
                    .unwrap()
                    .source;
            if matches!(target, MoonshineModuleTarget::Dom) {
                assert!(output.contains(&format!("{binding}_")), "{output}");
            }
        }
    }
}

#[test]
fn decoded_script_binding_does_not_capture_key_block_helper() {
    let source = r"<script>const \u005f_CrepusKeyBlock0=1;</script>{#key 1}<p>{\u005f_CrepusKeyBlock0}</p>{/key}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("KeyHygiene.svelte"), target)
            .unwrap()
            .source;
        assert!(!output.contains("function __CrepusKeyBlock0<"), "{output}");
        assert!(!output.contains("function __CrepusKeyBlock0("), "{output}");
        assert!(!output.contains("For as __CrepusKeyBlock0"), "{output}");
    }
}
