#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;
fn compile(template: &str, target: MoonshineModuleTarget) -> Result<String, String> {
    compile_moonshine_svelte(&format!("<script>let value=$state(1);let rows=$state([{{id:1,value:2}}]);let promise=Promise.resolve(3);function source(){{return {{a:value,b:undefined,c:4}}}}function use(v){{return v}}</script>{template}"),Path::new("Const.svelte"),target).map(|module|module.source)
}
#[test]
fn const_fragments_erase_into_existing_callbacks_for_all_permitted_owners() {
    for template in [
        "{#if value}{@const doubled=value*2}<p>{doubled}|{doubled}</p>{:else}{@const fallback='empty'}{fallback}{/if}",
        "{#each rows as row,i(row.id)}{@const total=row.value+i}<p>{total}</p>{:else}{@const empty='empty'}{empty}{/each}",
        "{#key value}{@const local=value}<p>{local}</p>{/key}",
        "{#await promise}{@const status='pending'}{status}{:then result}{@const status=result+value}{status}{:catch error}{@const status=String(error)}{status}{/await}",
        "{#snippet item(value)}{@const value=2}<b>{value}</b>{/snippet}{@render item(1)}",
        "<Card>{#snippet heading()}{@const local=value+1}<h2>{local}</h2>{/snippet}{@const local=value+2}<b>{local}</b><i>{local}</i></Card>",
        "{#each rows as row}{#if value}{@const row=value}<b>{row}</b>{/if}<i>{row.value}</i>{/each}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile(template, target).unwrap();
            assert!(!output.contains("CrepusConstScope"), "{output}");
            assert!(!output.contains("const_scope"));
        }
    }
}
#[test]
fn const_patterns_preserve_shared_evaluation_and_target_specific_getter_policy() {
    let template = "{#if value}{@const {a}=source()}{@const {a:b=use(value),...rest}=source()}{@const [x=use(value),...tail]=[value]}<p>{a}|{a}|{b}|{rest.c}|{x}|{tail.length}</p>{/if}";
    let dom = compile(template, MoonshineModuleTarget::Dom).unwrap();
    assert!(
        dom.contains("createConstDerived as __crepusConstDerived"),
        "{dom}"
    );
    assert!(dom.contains(".value.a"), "{dom}");
    assert!(dom.contains("...rest"), "{dom}");
    let ssr = compile(template, MoonshineModuleTarget::Ssr).unwrap();
    assert!(
        !ssr.contains("createConstDerived as __crepusConstDerived"),
        "{ssr}"
    );
    assert!(ssr.contains("...rest"), "{ssr}");
}
#[test]
fn duplicate_payloads_and_readonly_bindings_reject_without_banning_property_writes() {
    for template in [
        "{#each rows as row}{@const row=1}{row}{/each}",
        "{#each rows as row,i}{@const i=1}{i}{/each}",
        "{#await promise then result}{@const result=1}{result}{/await}",
        "{#if value}{@const x=value}<button onclick={()=>x++}>write</button>{/if}",
        "{#if value}{@const x=value}<button onclick={()=>({x}={x:2})}>write</button>{/if}",
        "{#if value}{@const x=1}{@const x=2}{x}{/if}",
        "{#if value}{@const x=1,y=2}{x}{/if}",
        "<Card>{@const heading=1}{heading}{#snippet heading()}title{/snippet}</Card>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(compile(template, target).is_err(), "accepted {template}");
        }
    }
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        compile(
            "{#if value}{@const x={a:value}}<button onclick={()=>x.a++}>write</button>{/if}",
            target,
        )
        .unwrap_or_else(|error| panic!("member write: {error}"));
    }
}
#[test]
fn legacy_dependencies_sort_but_runes_keep_source_order_and_nested_shadows() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(
            "{#if value}{y}{@const y=x+1}{@const x=value}{#if value}{@const x=9}{x}{/if}{x}{/if}",
            target,
        )
        .unwrap();
        assert!(
            output.find("const y =").unwrap() < output.find("const x =").unwrap(),
            "{output}"
        );
        assert!(compile("{#if value}{@const x=y}{@const y=x}{x}{/if}", target).is_ok());
        let legacy = compile_moonshine_svelte(
            "{#if true}{@const y=x+1}{@const x=1}{y}{/if}",
            Path::new("Legacy.svelte"),
            target,
        )
        .unwrap()
        .source;
        assert!(
            legacy.find("const x =").unwrap() < legacy.find("const y =").unwrap(),
            "{legacy}"
        );
        assert!(compile_moonshine_svelte(
            "{#if true}{@const x=y}{@const y=x}{x}{/if}",
            Path::new("Legacy.svelte"),
            target
        )
        .is_err());
    }
}

#[test]
fn template_types_follow_original_script_language_not_synthetic_tsx() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for declaration in ["x:number=1", "{x}:{x:number}={x:1}", "x=1 as number"] {
            let template = format!("{{#if true}}{{@const {declaration}}}{{x}}{{/if}}");
            assert!(compile_moonshine_svelte(&template, Path::new("Js.svelte"), target).is_err());
            for prefix in [
                "<script lang=\"ts\"></script>",
                "<script module lang=\"ts\"></script>",
            ] {
                compile_moonshine_svelte(
                    &format!("{prefix}{template}"),
                    Path::new("Typed.svelte"),
                    target,
                )
                .unwrap_or_else(|error| panic!("typed {prefix}{template}: {error}"));
            }
        }
    }
}
