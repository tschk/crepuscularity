#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use std::path::Path;

fn compile(source: &str, target: MoonshineModuleTarget) -> String {
    compile_moonshine_svelte(source, Path::new("MountMetadata.svelte"), target)
        .unwrap()
        .source
}

fn compact(source: &str) -> String {
    source.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn component_metadata_preserves_setup_local_props_types() {
    let source = r#"<script lang="ts">
const label = 'local' as const;
type Props = {label:typeof label;count?:number};
let props:Props=$props();
export function read(){return props.label}
</script><p>{props.label}</p>"#;
    for (target, runtime) in [
        (MoonshineModuleTarget::Dom, "@tschk/moonshine-svelte/mount"),
        (
            MoonshineModuleTarget::Ssr,
            "@tschk/moonshine-svelte/server-mount",
        ),
    ] {
        let output = compile(source, target);
        let generated = compact(&output);
        assert!(output.contains(runtime), "{output}");
        assert!(output.contains("ComponentView"), "{output}");
        assert!(output.contains("ComponentFactory"), "{output}");
        assert!(
            generated.contains("type__crepusSveltePublicProps=Props"),
            "{output}"
        );
        assert!(generated.contains("typeofread"), "{output}");
    }
}

#[test]
fn component_metadata_tracks_export_types_without_server_exposure() {
    let source = r#"<script lang="ts">
let value:number=1;
let doubled=$derived(value*2);
export {value,doubled};
export function identity<T>(input:T):T{return input}
</script><p>{value}:{doubled}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(source, target);
        let generated = compact(&output);
        assert!(generated.contains("typeofvalue"), "{output}");
        assert!(generated.contains("typeofidentity"), "{output}");
        assert!(generated.contains("typeofdoubled.value"), "{output}");
        assert!(
            generated.contains("readonlydoubled:")
                || generated.contains("readonly\"doubled\":")
                || generated.contains("readonly[\"doubled\"]:"),
            "{output}"
        );
        if matches!(target, MoonshineModuleTarget::Ssr) {
            assert!(!output.contains("exposeComponent"), "{output}");
            assert!(!generated.contains("set[\"value\"]"), "{output}");
            assert!(!generated.contains("get[\"doubled\"]"), "{output}");
        }
    }
}

#[test]
fn public_props_alias_avoids_decoded_authored_bindings() {
    let source = r#"<script lang="ts">
const \u005f_crepusSveltePublicProps='authored';
let {label}:{label:string}=$props();
</script><p>{label}:{\u005f_crepusSveltePublicProps}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(source, target);
        assert!(
            compact(&output).contains("type__crepusSveltePublicProps_="),
            "{output}"
        );
    }
}

#[test]
fn inferred_public_props_distinguish_required_and_defaulted_bindings() {
    let source = r#"<script lang="ts">
let {required,optional='fallback',bound=$bindable<number>()}=$props();
</script><p>{required}:{optional}:{bound}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(source, target);
        let generated = compact(&output);
        let props = generated
            .split("type__crepusSveltePublicProps=")
            .nth(1)
            .expect("inferred public props type must be emitted");
        for (key, optional) in [("required", false), ("optional", true), ("bound", true)] {
            let marker = if optional { "?:" } else { ":" };
            assert!(
                props.contains(&format!("{key}{marker}"))
                    || props.contains(&format!("\"{key}\"{marker}"))
                    || props.contains(&format!("[\"{key}\"]{marker}")),
                "{output}"
            );
        }
        assert!(
            props.contains("bound?:number")
                || props.contains("\"bound\"?:number")
                || props.contains("[\"bound\"]?:number"),
            "{output}"
        );
    }
}

#[test]
fn identifier_props_annotation_is_available_to_public_factory_metadata() {
    let source = r#"<script lang="ts">
interface InputProps {label:string;count?:number}
let props:InputProps=$props();
</script><p>{props.label}:{props.count}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(source, target);
        let generated = compact(&output);
        assert!(
            generated.contains("type__crepusSveltePublicProps=__crepusSvelteInterface"),
            "{output}"
        );
        assert!(generated.contains("label:string;count?:number"), "{output}");
        assert!(!generated.contains("interfaceInputProps"), "{output}");
    }
}

#[test]
fn typed_public_factory_remains_a_hoisted_default_function() {
    let source = r#"<script lang="ts">
let {label}:{label:string}=$props();
export function read(){return label}
</script><p>{label}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile(source, target);
        let generated = compact(&output);
        assert!(generated.contains("exportdefaultfunction"), "{output}");
        assert!(generated.contains("Parameters<"), "{output}");
        assert!(generated.contains("ComponentFactory"), "{output}");
        assert!(generated.contains(".apply(this,"), "{output}");
        assert!(!generated.contains("exportdefault("), "{output}");
    }
}
