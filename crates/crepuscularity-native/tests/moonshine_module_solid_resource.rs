#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use std::path::Path;

const SELECTOR: &str = "/** @jsxImportSource solid-js */\n";
const FORMS: [&str; 8] = [
    "import { createResource } from 'solid-js'; export const load = () => createResource(() => 1);",
    "import { createResource as fetchData } from 'solid-js'; export { fetchData };",
    "import { type Resource } from 'solid-js'; export type Held = Resource<string>;",
    "import type { Resource } from 'solid-js'; export type Held = Resource<string>;",
    "export { createResource as load } from 'solid-js';",
    "export type { Resource as Loaded } from 'solid-js';",
    "export type Query = typeof import('solid-js').createResource;",
    "export type Held = import('solid-js').Resource<string>;",
];

fn compile(source: &str, target: MoonshineModuleTarget) -> Result<String, String> {
    compile_moonshine_module(source, Path::new("Resource.tsx"), target).map(|module| module.source)
}

#[test]
fn explicit_solid_dom_admits_resource_forms_on_the_solid_adapter() {
    for form in FORMS {
        let output = compile(&format!("{SELECTOR}{form}"), MoonshineModuleTarget::Dom)
            .unwrap_or_else(|error| panic!("{form}: {error}"));
        assert_eq!(
            output.matches("@tschk/moonshine-solid/solid").count(),
            1,
            "{form}: {output}"
        );
        assert!(!output.contains("solid-js"), "{form}: {output}");
        assert!(
            !output.contains("moonshine-dom/reactive"),
            "{form}: {output}"
        );
    }
}

#[test]
fn explicit_solid_dom_resource_aliases_and_type_queries_are_preserved() {
    let source = format!(
        "{SELECTOR}import {{ createResource as fetchData }} from 'solid-js';\nexport {{ createResource as load }} from 'solid-js';\nexport type Query = typeof import('solid-js').createResource;\nexport const data = fetchData(() => 1);\n"
    );
    let output = compile(&source, MoonshineModuleTarget::Dom).unwrap();
    assert_eq!(
        output.matches("@tschk/moonshine-solid/solid").count(),
        3,
        "{output}"
    );
    assert!(output.contains("createResource as fetchData"), "{output}");
    assert!(output.contains("createResource as load"), "{output}");
    assert!(output.contains("typeof import("), "{output}");
    assert!(output.contains(".createResource"), "{output}");
}

#[test]
fn explicit_solid_dom_splits_resource_from_control_helpers() {
    let source = format!(
        "{SELECTOR}import {{ createResource, mapArray }} from 'solid-js';\nexport {{ createResource as load, indexArray }} from 'solid-js';\nexport const rows = () => [createResource, mapArray];\n"
    );
    let output = compile(&source, MoonshineModuleTarget::Dom).unwrap();
    assert!(
        output.contains("import { createResource } from \"@tschk/moonshine-solid/solid\";"),
        "{output}"
    );
    assert!(
        output.contains("import { mapArray } from \"@tschk/moonshine-dom/control\";"),
        "{output}"
    );
    assert!(
        output.contains("export { createResource as load } from \"@tschk/moonshine-solid/solid\";"),
        "{output}"
    );
    assert!(
        output.contains("export { indexArray } from \"@tschk/moonshine-dom/control\";"),
        "{output}"
    );
    assert!(!output.contains("solid-js"), "{output}");
}

#[test]
fn explicit_solid_ssr_routes_resource_forms_to_server_factory() {
    let mixed = [
        "import { createResource, mapArray } from 'solid-js'; export const x = [createResource, mapArray];",
        "export { createResource, indexArray } from 'solid-js';",
    ];
    for form in FORMS.into_iter().chain(mixed) {
        let output = compile(&format!("{SELECTOR}{form}"), MoonshineModuleTarget::Ssr)
            .unwrap_or_else(|error| panic!("{form}: {error}"));
        assert!(
            output.contains("@tschk/moonshine-solid/ssr"),
            "{form}: {output}"
        );
        assert!(!output.contains("solid-js"), "{form}: {output}");
        assert!(
            !output.contains("@tschk/moonshine-solid/solid"),
            "{form}: {output}"
        );
        if form.contains("mapArray") || form.contains("indexArray") {
            assert!(
                output.contains("@tschk/moonshine-dom/control"),
                "{form}: {output}"
            );
        }
    }
}

#[test]
fn resources_without_explicit_solid_source_are_rejected_in_both_targets() {
    for form in FORMS {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile(form, target).expect_err(form);
            assert!(error.contains("client resources"), "{form}: {error}");
        }
    }
}

#[test]
fn other_unsupported_solid_names_stay_rejected_with_explicit_source() {
    for name in ["createDeferred", "createContext", "lazy", "ResourceReturn"] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let source = format!("{SELECTOR}export {{ {name} }} from 'solid-js';");
            assert!(compile(&source, target).is_err(), "{name}");
        }
    }
}
