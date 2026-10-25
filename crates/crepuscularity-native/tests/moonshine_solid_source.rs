#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use std::path::Path;
fn compile(source: &str, target: MoonshineModuleTarget) -> Result<String, String> {
    compile_moonshine_module(source, Path::new("Solid.tsx"), target).map(|module| module.source)
}
#[test]
fn explicit_leading_source_uses_independent_factory_and_consumes_only_selector() {
    for (target, factory) in [
        (MoonshineModuleTarget::Dom, "@tschk/moonshine-solid/h"),
        (MoonshineModuleTarget::Ssr, "@tschk/moonshine-solid/ssr"),
    ] {
        let source = "/** before\n * @jsxImportSource solid-js\n * after\n */\nimport {createSignal} from 'solid-js'; function Child(){return <span>child</span>} export default function App(){const [read]=createSignal(true); return <div><Child/>{read()?<b>yes</b>:<i>no</i>}</div>}";
        let output = compile(source, target).expect("explicit source compiles");
        assert!(output.contains(factory), "{output}");
        assert!(!output.contains("@jsxImportSource"), "{output}");
        assert!(
            output.contains("before") && output.contains("after"),
            "{output}"
        );
        assert!(!output.contains("solid-js/jsx-runtime"), "{output}");
        if matches!(target, MoonshineModuleTarget::Dom) {
            assert!(
                output.contains("@tschk/moonshine-solid/control"),
                "{output}"
            );
            assert!(output.contains("@tschk/moonshine-solid/solid"), "{output}");
        }
    }
}
#[test]
fn unmarked_source_keeps_existing_native_factory_and_imports() {
    let output=compile("import{createSignal}from'solid-js';export default function App(){const[n]=createSignal(0);return <p>{n()}</p>}",MoonshineModuleTarget::Dom).expect("old subset");
    assert!(output.contains("@tschk/moonshine-dom/dom"));
    assert!(output.contains("@tschk/moonshine-dom/reactive"));
    assert!(!output.contains("moonshine-solid"));
}
#[test]
fn pragma_conflicts_duplicates_late_and_quoted_mentions_fail_closed() {
    for prefix in [
        "/** @jsxImportSource react */",
        "/** @jsx h */",
        "/** @jsxRuntime automatic */",
        "/** \"@jsxImportSource solid-js\" */",
        "/** @jsxImportSource solid-js */\n/** @jsxImportSource solid-js */",
        "const earlier=1;/** @jsxImportSource solid-js */",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile(&format!("{prefix}\nexport default()=> <p/>"), target).is_err(),
                "{prefix}"
            );
        }
    }
}
#[test]
fn component_identity_must_be_local_stable_and_synchronous() {
    for source in [
        "import Child from './Child.svelte';export default()=> <Child/>",
        "import{Child}from'./Child';export default()=> <Child/>",
        "const Child=()=> <p/>;const Alias=Child;export default()=> <Alias/>",
        "export default(props:{Child:()=>unknown})=> <props.Child/>",
        "let Child=()=> <p/>;export default()=> <Child/>",
        "function Child(){return <p/>}Child=()=> <i/>;export default()=> <Child/>",
        "async function Child(){return <p/>}export default()=> <Child/>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile(
                &format!("/** @jsxImportSource solid-js */\n{source}"),
                target,
            )
            .expect_err(source);
            assert!(error.contains("component"), "{error}");
        }
    }
}
#[test]
fn selected_create_root_is_admitted_but_default_guard_stays() {
    let source = "import{createRoot}from'solid-js';export default()=>createRoot(stop=><p/>)";
    assert!(compile(source, MoonshineModuleTarget::Dom).is_err());
    assert!(compile(
        &format!("/** @jsxImportSource solid-js */\n{source}"),
        MoonshineModuleTarget::Dom
    )
    .expect("selected createRoot")
    .contains("@tschk/moonshine-solid/solid"));
}
#[test]
fn known_foreign_component_runtime_edges_reject_but_type_and_companion_utilities_remain_neutral() {
    for body in ["import Foreign from './Foreign.svelte';function Local(){return Foreign({})}export default()=> <Local/>","import{Foreign}from'./Foreign.Vue';export default()=> <p/>","export{default as Foreign}from'./Foreign.vue';export default()=> <p/>","export*from'./Foreign.svelte';export default()=> <p/>","export const load=()=>import('./Foreign.vue');export default()=> <p/>"]{
  for target in [MoonshineModuleTarget::Dom,MoonshineModuleTarget::Ssr]{let error=compile(&format!("/** @jsxImportSource solid-js */\n{body}"),target).expect_err(body);assert!(error.contains("imported component ownership"),"{error}");}
 }
    for body in [
        "import type{Props}from'./Foreign.svelte';export default()=> <p/>",
        "import{type Props}from'./Foreign.vue';export default()=> <p/>",
        "export type{Props}from'./Foreign.vue';export default()=> <p/>",
        "export{type Props}from'./Foreign.svelte';export default()=> <p/>",
        "import{value}from'./utility.svelte.ts';export default()=> <p>{value}</p>",
        "import{value}from'./utility.svelte.js';export default()=> <p>{value}</p>",
    ] {
        compile(
            &format!("/** @jsxImportSource solid-js */\n{body}"),
            MoonshineModuleTarget::Dom,
        )
        .expect(body);
    }
}

#[test]
fn selected_default_entry_requires_a_stable_synchronous_identity() {
    for body in [
        "export default async function App(){return <p/>}",
        "export default function App(){return <p/>}App=async()=> <i/>",
        "async function App(){return <p/>}export{App as default}",
        "export{default}from'./Other'",
        "export * as default from'./Other'",
        "export default function* App(){yield <p/>}",
        "export default async()=> <p/>",
        "const App=async()=> <p/>;export default App",
        "function* App(){yield <p/>}export default App",
        "let App=()=> <p/>;export default App",
        "const App=()=> <p/>;export default (()=>App)()",
        "function App(){return <p/>}App=()=> <i/>;export default App",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile(&format!("/** @jsxImportSource solid-js */\n{body}"), target)
                .expect_err(body);
            assert!(error.contains("component"), "{error}");
        }
    }
    for body in [
        "async function load(){return 1}function App(){return <p/>}export default App",
        "function App(){return <p/>}export{App as default}",
        "export type * as default from'./Other'",
        "function* values(){yield 1}const App=()=> <p/>;export default App",
        "export default function App(){return <p/>}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            compile(&format!("/** @jsxImportSource solid-js */\n{body}"), target).expect(body);
        }
    }
}

#[test]
fn leading_selectors_preserve_directive_preambles_and_original_edit_offsets() {
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    for source in [
        "/** keep before\n * @jsxImportSource solid-js\n * keep after\n */\n'use strict'; export default()=> <p>ok</p>;",
        "#!/usr/bin/env bun\n/** @jsxImportSource solid-js */\n'use client';'use strict';export default()=> <><p>ok</p></>;",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output=compile(source,target).expect("leading selector and directives");
            let allocator=Allocator::default();
            let parsed=Parser::new(&allocator,&output,SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(),"{:?}: {output}",parsed.errors);
            assert!(parsed.program.directives.iter().any(|directive| directive.expression.value=="use strict"),"{output}");
            if source.starts_with("#!") {assert!(parsed.program.hashbang.is_some(),"{output}");}
            if source.contains("keep before") {assert!(output.contains("keep before")&&output.contains("keep after"),"{output}");}
        }
    }
    for source in [
        "/** @jsxImportSource solid-js */",
        "/** @jsxImportSource solid-js */\n'use strict';",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            compile(source, target).expect("selector-only or directive-only module");
        }
    }
}

#[test]
fn selectors_after_a_directive_are_late_even_without_body_statements() {
    for source in [
        "'use strict';\n/** @jsxImportSource solid-js */\nexport default()=> <p/>;",
        "'use strict';\n/** @jsxImportSource solid-js */",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile(source, target).unwrap_err();
            assert!(error.contains("precede the first statement"), "{error}");
        }
    }
}

#[test]
fn explicit_selectors_route_through_module_detection_without_imports_or_exports() {
    use crepuscularity_native::try_compile_moonshine_module;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for source in [
            "/** @jsxImportSource solid-js */",
            "/** @jsxImportSource solid-js */\n'use strict';",
        ] {
            let output = try_compile_moonshine_module(source, Path::new("Selected.tsx"), target)
                .unwrap()
                .expect("explicit module selector");
            assert!(
                !output.source.contains("data-crepus-root"),
                "{}",
                output.source
            );
        }
        let error = try_compile_moonshine_module(
            "'use strict';/** @jsxImportSource solid-js */",
            Path::new("Late.tsx"),
            target,
        )
        .unwrap_err();
        assert!(error.contains("precede the first statement"), "{error}");
        for template in [
            "<p>{/* @jsx is ordinary comment text */}template</p>",
            "<p>{/* mentions @jsxImportSource solid-js in prose */}template</p>",
        ] {
            assert!(
                try_compile_moonshine_module(template, Path::new("Template.tsx"), target)
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            try_compile_moonshine_module("<p>template</p>", Path::new("Template.tsx"), target)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn component_redeclaration_initializers_are_writes_but_bare_var_is_not() {
    for body in [
        "function Child(){return <p/>}var Child=async()=> <i/>;return <Child/>",
        "function Child(){return <p/>}var [Child]=[async()=> <i/>];return <Child/>",
        "function Child(){return <p/>}var {value:Child}={value:async()=> <i/>};return <Child/>",
        "function Child(){return <p/>}async function Child(){return <i/>}return <Child/>",
        "async function Child(){return <i/>}function Child(){return <p/>}return <Child/>",
        "function Child(){return <p/>}function* Child(){yield <i/>}return <Child/>",
    ] {
        let input =
            format!("/** @jsxImportSource solid-js */\nexport default function App(){{{body}}}");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile(&input, target).expect_err(&input);
            assert!(error.contains("redeclaration writes"), "{input}: {error}");
        }
    }
    for body in [
        "function Child(){return <p/>}var Child;return <Child/>",
        "const Child=()=> <p/>;return <Child/>",
        "function Child(){return <p/>}var unrelated=async()=>1;return <Child/>",
    ] {
        let input =
            format!("/** @jsxImportSource solid-js */\nexport default function App(){{{body}}}");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            compile(&input, target).expect(&input);
        }
    }
}

#[test]
fn explicit_owner_imports_and_aliases_use_the_independent_target_facade() {
    let source = "/** @jsxImportSource solid-js */\nimport {getOwner as capture, runWithOwner as resume, type Owner} from 'solid-js'; export function callback(){const scope: Owner|null = capture(); return () => resume(scope, () => 42);} export default () => <p>owner</p>;";
    for (target, facade) in [
        (MoonshineModuleTarget::Dom, "@tschk/moonshine-solid/solid"),
        (MoonshineModuleTarget::Ssr, "@tschk/moonshine-solid/ssr"),
    ] {
        let output = compile(source, target).expect("explicit native owner contract");
        assert!(output.contains(facade), "{output}");
        assert!(output.contains("getOwner as capture"), "{output}");
        assert!(output.contains("runWithOwner as resume"), "{output}");
        assert!(!output.contains("from 'solid-js'"), "{output}");
        assert!(compile(
            "import {getOwner} from 'solid-js'; export const owner = getOwner();",
            target
        )
        .is_err());
    }
}

#[test]
fn explicit_solid_computed_uses_the_independent_runtime() {
    let source = "/** @jsxImportSource solid-js */\nimport {createComputed,createSignal} from 'solid-js'; export default function App(){const [count]=createSignal(2);const [double,setDouble]=createSignal(0);createComputed(()=>setDouble(count()*2));return <p>{double()}</p>}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(source, target).expect("computed source compiles");
        assert!(output.contains("createComputed"), "{output}");
        assert!(!output.contains("from \"solid-js\""), "{output}");
    }
}
