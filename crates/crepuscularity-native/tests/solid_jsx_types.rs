#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use std::path::Path;
#[test]
fn solid_jsx_element_is_type_only_and_backend_specific() {
    let source="import type {JSX as J} from 'solid-js';import {createSignal} from 'solid-js';function Child(props:{label:string;children?:J.Element}):J.Element{return <p>{props.label}{props.children}</p>}export default function Parent():J.Element{const[n]=createSignal('value');return <Child label={n()}><span>child</span></Child>}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_module(source, Path::new("Typed.tsx"), target)
            .unwrap()
            .source;
        assert!(!output.contains("from \"solid-js\""), "{output}");
        assert!(output.contains("namespace J"), "{output}");
        assert!(
            output.contains(if matches!(target, MoonshineModuleTarget::Dom) {
                ".DOMChild"
            } else {
                ".SSRChild"
            }),
            "{output}"
        );
        assert!(output.contains("__crepusComponentProps(Child,"), "{output}");
        assert!(output.contains("[K in keyof P]"), "{output}");
    }
}
#[test]
fn inline_type_only_namespace_preserves_other_named_imports_and_rejects_unsupported_members() {
    let source="import {createSignal,type JSX} from 'solid-js';export default function App():JSX.Element{const[n]=createSignal(1);return <p>{n()}</p>}";
    assert!(
        compile_moonshine_module(source, Path::new("Mixed.tsx"), MoonshineModuleTarget::Dom)
            .unwrap()
            .source
            .contains("createSignal")
    );
    for ty in [
        "JSX.IntrinsicElements",
        "JSX.Element<string>",
        "JSX.Element.Other",
        "JSX",
    ] {
        let source=format!("import type {{JSX}} from 'solid-js';type Unsupported={ty};export default function App(){{return <p/>}}");
        assert!(
            compile_moonshine_module(
                &source,
                Path::new("Unsupported.tsx"),
                MoonshineModuleTarget::Dom
            )
            .is_err(),
            "{ty}"
        );
    }
    let source="import type {JSX} from 'solid-js';const value=JSX;export default function App(){return <p/>}";
    assert!(
        compile_moonshine_module(source, Path::new("Value.tsx"), MoonshineModuleTarget::Dom)
            .is_err()
    );
}
#[test]
fn javascript_component_props_keep_the_existing_factory_body() {
    let output=compile_moonshine_module("function Child(props){return <p>{props.value}</p>}export default function App(){return <Child value={1}/>}",Path::new("Plain.jsx"),MoonshineModuleTarget::Dom).unwrap().source;
    assert!(
        output.contains("globalThis.Object.create(globalThis.Object.prototype,"),
        "{output}"
    );
    assert!(!output.contains("__crepusComponentProps"), "{output}");
}

#[test]
fn named_interface_props_use_object_constraint_and_keep_cached_child_types() {
    let source = "import type {JSX} from 'solid-js';interface Props{value:number;children?:JSX.Element}function Child(props:Props):JSX.Element{return <p>{props.value}{props.children}</p>}export default function App():JSX.Element{return <Child value={1}><span>child</span></Child>}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_module(source, Path::new("Interface.tsx"), target)
            .expect("named-interface component fixture must compile")
            .source;
        assert!(output.contains("P extends object"), "{output}");
        assert!(output.contains("NoInfer<D>"), "{output}");
        assert!(!output.contains("P extends Record"), "{output}");
        assert!(output.contains("ReturnType<T[number]>"), "{output}");
    }
}
