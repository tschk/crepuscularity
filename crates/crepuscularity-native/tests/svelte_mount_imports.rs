#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn ordinary_launchers_route_mount_imports_by_target() {
    for (target, runtime) in [
        (MoonshineModuleTarget::Dom, "@tschk/moonshine-svelte/mount"),
        (
            MoonshineModuleTarget::Ssr,
            "@tschk/moonshine-svelte/server-mount",
        ),
    ] {
        for path in ["launcher.ts", "launcher.tsx"] {
            let source = "import {mount as start,unmount as stop,onMount as ready} from 'svelte';export {start,stop,ready};";
            let output = compile_moonshine_module(source, Path::new(path), target).unwrap();
            let compact: String = output
                .source
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            assert!(output.source.contains(runtime), "{}", output.source);
            assert!(
                output.source.contains("@tschk/moonshine-svelte/svelte"),
                "{}",
                output.source
            );
            for specifier in ["mountasstart", "unmountasstop", "onMountasready"] {
                assert!(compact.contains(specifier), "{}", output.source);
            }
            assert!(!output.source.contains("from 'svelte'"));
            assert!(!output.source.contains("from \"svelte\""));
        }
    }
}

#[test]
fn ordinary_barrels_route_mount_reexports_by_target() {
    for (target, runtime) in [
        (MoonshineModuleTarget::Dom, "@tschk/moonshine-svelte/mount"),
        (
            MoonshineModuleTarget::Ssr,
            "@tschk/moonshine-svelte/server-mount",
        ),
    ] {
        let source = "export {mount as start,unmount as stop,onMount as ready} from 'svelte';";
        let output = compile_moonshine_module(source, Path::new("barrel.ts"), target).unwrap();
        let compact: String = output
            .source
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(output.source.contains(runtime), "{}", output.source);
        assert!(
            output.source.contains("@tschk/moonshine-svelte/svelte"),
            "{}",
            output.source
        );
        for specifier in ["mountasstart", "unmountasstop", "onMountasready"] {
            assert!(compact.contains(specifier), "{}", output.source);
        }
    }
}

#[test]
fn unsupported_hydration_and_native_branding_are_not_official_svelte_helpers() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for source in [
            "import {hydrate} from 'svelte';",
            "export {hydrate} from 'svelte';",
            "import {defineComponent} from 'svelte';",
            "export {defineComponent} from 'svelte';",
            "import {componentView} from 'svelte';",
        ] {
            assert!(
                compile_moonshine_module(source, Path::new("unsupported.ts"), target).is_err(),
                "unexpectedly accepted {source}"
            );
        }
    }
}
