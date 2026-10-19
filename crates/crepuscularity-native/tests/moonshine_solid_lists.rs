#![cfg(feature = "web-modules")]

use crepuscularity_native::{
    compile_moonshine_module, MoonshineModuleLanguage, MoonshineModuleTarget,
};
use std::path::Path;

const P: &str = "/** @jsxImportSource solid-js */\n";

#[test]
fn explicit_list_aliases_lower_to_typed_independent_factories() {
    let source = format!("{P}import {{For as Rows,Index as Slots,createSignal}} from 'solid-js'; export default function App(){{const [items]=createSignal([{{label:'one'}}]);return <main><Rows each={{items()}} fallback={{<i>empty</i>}}>{{(item,index)=><p>{{item.label}}:{{index()}}</p>}}</Rows><Slots each={{items()}}>{{(item,index)=><p>{{item().label}}:{{index}}</p>}}</Slots></main>}}");
    for (target, host) in [
        (MoonshineModuleTarget::Dom, "@tschk/moonshine-solid/h"),
        (MoonshineModuleTarget::Ssr, "@tschk/moonshine-solid/ssr"),
    ] {
        let output = compile_moonshine_module(&source, Path::new("App.tsx"), target).unwrap();
        assert!(output.source.contains(host), "{}", output.source);
        assert!(output.source.contains("__crepusSolidFor"));
        assert!(output.source.contains("__crepusSolidIndex"));
        assert!(!output.source.contains("solid-js"), "{}", output.source);
        assert!(!output.source.contains("<Rows") && !output.source.contains("<Slots"));
        assert_eq!(output.language, MoonshineModuleLanguage::TypeScript);
    }
}

#[test]
fn list_validation_rejects_invalid_props_and_callbacks_before_emission() {
    for element in [
        "<For>{row=>row}</For>",
        "<For each={[]} />",
        "<For each={[]} children={row=>row}>{row=>row}</For>",
        "<For each={[]} each={[]} children={row=>row}/>",
        "<For {...props} children={row=>row}/>",
        "<For each={[]} unknown={1} children={row=>row}/>",
        "<For each={[]}>text</For>",
        "<For each={[]}>{a}{b}</For>",
    ] {
        let source = format!(
            "{P}import {{For}} from 'solid-js';export default function App(){{return {element}}}"
        );
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_module(&source, Path::new("App.tsx"), target).is_err(),
                "{element}"
            );
        }
    }
}

#[test]
fn javascript_lists_keep_javascript_and_require_explicit_provenance() {
    let source = "import {For} from 'solid-js';export default function App(){return <For each={[1]}>{row=><p>{row}</p>}</For>}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        assert!(compile_moonshine_module(source, Path::new("App.jsx"), target).is_err());
        let output =
            compile_moonshine_module(&format!("{P}{source}"), Path::new("App.jsx"), target)
                .unwrap();
        assert_eq!(output.language, MoonshineModuleLanguage::JavaScript);
        assert!(!output.source.contains("type "));
        assert!(!output.source.contains("extends __Crepus"));
    }
}

#[test]
fn unsupported_list_import_identities_fail_closed() {
    for body in [
        "import type {For} from 'solid-js';export default()=> <For each={[]}>{row=>row}</For>;",
        "import * as Solid from 'solid-js';export default()=> <Solid.For each={[]}>{row=>row}</Solid.For>;",
        "import {For} from 'solid-js';const Alias=For;export default()=> <Alias each={[]}>{row=>row}</Alias>;",
        "import {Show} from 'solid-js';export default()=> <Show when={true}>visible</Show>;",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(compile_moonshine_module(&format!("{P}{body}"), Path::new("App.tsx"), target).is_err(), "{body}");
        }
    }
}

#[test]
fn bare_intrinsic_data_attributes_keep_empty_values_without_changing_explicit_true() {
    let source = format!("{P}export default()=> <main data-marker data-valued={{true}}/>;");
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_module(&source, Path::new("App.tsx"), target).unwrap();
        assert!(
            output.source.contains("\"data-marker\": \"\""),
            "{}",
            output.source
        );
        assert!(
            output.source.contains("\"data-valued\": () => true"),
            "{}",
            output.source
        );
    }
}
