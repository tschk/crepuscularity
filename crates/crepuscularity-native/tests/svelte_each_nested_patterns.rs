#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;
fn compile(body: &str, target: MoonshineModuleTarget) -> Result<String, String> {
    let source = format!("<script lang=\"ts\">let rows=$state([{{id:'a',nested:{{label:'one',child:{{value:'x'}},callback:()=>1}}}}]);</script>{{#each rows as {body}{{/each}}");
    compile_moonshine_svelte(&source, Path::new("NestedEach.svelte"), target)
        .map(|module| module.source)
}
#[test]
fn nested_static_paths_preserve_live_projection_and_snapshot_types() {
    let body="{id,nested:{label:title,child,callback}},index(id)}<input bind:value={child.value}/><p>{title}:{title}:{callback()}:{index}</p>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(body, target).expect("nested static projections must compile");
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(output.contains("[\"nested\"][\"label\"]"), "{output}");
            assert!(output.contains("(0, __crepusEachFields"), "{output}");
            assert!(!output.contains("SnapshotChildren"), "{output}");
        } else {
            assert!(output.contains("SnapshotChildren"), "{output}");
            assert!(!output.contains("__crepusEachFields"), "{output}");
        }
    }
}
#[test]
fn nested_leaf_aliases_are_readonly_and_not_type_queries() {
    for child in [
        "<button onclick={()=>title='x'}>x</button>",
        "<button onclick={()=>title++}>x</button>",
        "<input bind:value={title}/>",
        "<input bind:value={(title)}/>",
        "<p>{(()=>{type T=typeof title;return ''})()}</p>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile(
                &format!("{{nested:{{label:title}}}},index(index)}}{child}"),
                target,
            )
            .expect_err("nested aliases must not become writable");
            assert!(error.contains("each bindings"), "{error}");
        }
    }
}
#[test]
fn nested_defaults_rest_and_arrays_preserve_both_target_syntax() {
    for pattern in [
        "{nested:{label='fallback'}}",
        "{nested:{label,...rest}}",
        "{nested:[label]}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile(&format!("{pattern},index(index)}}<p>x</p>"), target)
                .expect("supported nested patterns must lower");
            let allocator = oxc_allocator::Allocator::default();
            let parsed =
                oxc_parser::Parser::new(&allocator, &output, oxc_span::SourceType::ts()).parse();
            assert!(
                parsed.errors.is_empty(),
                "{pattern}: {:?}\n{output}",
                parsed.errors
            );
        }
    }
}
#[test]
fn unsupported_nested_pattern_forms_remain_diagnostics() {
    for pattern in ["{nested:{[key]:label}}", "{nested:{0:label}}"] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile(&format!("{pattern},index(index)}}<p>x</p>"), target)
                    .expect_err("computed and numeric nested keys remain unsupported")
                    .contains("unsupported")
            );
        }
    }
}
#[test]
fn lexical_shadows_and_nested_aliases_do_not_conflict() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        compile("{nested:{label:title}},index(index)}<p>{((title:string)=>title)('shadow')}:{title}</p>",target).expect("shadowed names remain distinct symbols");
    }
}

#[test]
fn nested_tag_references_use_an_unbound_value_without_losing_inference() {
    let source="<script lang=\"ts\">const rows=[{id:'a',nested:{get tag(){return function(this:void,strings:TemplateStringsArray,value:string){return value}}}}];</script>{#each rows as {id,nested:{tag}},index(id)}<p>{tag`before${'value'}`}</p>{/each}";
    let output =
        compile_moonshine_svelte(source, Path::new("Tag.svelte"), MoonshineModuleTarget::Dom)
            .expect("nested tag must compile")
            .source;
    assert!(output.contains("=> __crepusEachFields"), "{output}");
    assert!(output.contains("this: void"), "{output}");
}

#[test]
fn instantiated_each_projection_calls_stay_unbound() {
    let source = r#"<script lang="ts">const rows=[{callback:function<T>(this:void,value:T){return value}}];</script>{#each rows as {callback}}<p>{(callback<string>)('value')}</p>{/each}"#;
    let output = compile_moonshine_svelte(
        source,
        Path::new("Instantiation.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap()
    .source;
    let compact: String = output.chars().filter(|ch| !ch.is_whitespace()).collect();
    assert!(compact.contains("(0,__crepusEachFields"), "{output}");
}

#[test]
fn decoded_script_names_do_not_capture_each_runtime_imports() {
    for binding in [
        "__crepusEachMemo",
        "__crepusEachKeyed",
        "__crepusEachChild",
        "__crepusEachIndex",
        "__crepusEachShow",
        "__crepusEachPrepare",
        "__crepusEachPrepared",
    ] {
        let escaped = format!("\\u005f{}", &binding[1..]);
        let source = format!("<script lang=\"ts\">const {escaped}=1;const rows=[{{value:1}}];</script>{{#each rows as row (row.value)}}<p>{{row.value}}:{{{escaped}}}</p>{{:else}}<p>empty</p>{{/each}}");
        let output = compile_moonshine_svelte(
            &source,
            Path::new("Imports.svelte"),
            MoonshineModuleTarget::Dom,
        )
        .unwrap()
        .source;
        let allocator = oxc_allocator::Allocator::default();
        let parsed =
            oxc_parser::Parser::new(&allocator, &output, oxc_span::SourceType::ts()).parse();
        assert!(
            parsed.errors.is_empty(),
            "{binding}: {:?}\n{output}",
            parsed.errors
        );
        let checked = oxc_semantic::SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        assert!(
            checked.errors.is_empty(),
            "{binding}: {:?}\n{output}",
            checked.errors
        );
    }
}
