#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use std::path::Path;

#[test]
fn solid_named_primitives_share_the_target_reactive_graph() {
    let source = r#"import { createSignal as signal, createMemo, createEffect, createComputed, batch, untrack, onCleanup, onMount, type Accessor } from 'solid-js';
export { createSignal as state, type Accessor } from 'solid-js';
export type Count = import('solid-js').Accessor<number>;
export default function Counter() {
  const [count, setCount] = signal(0);
  const doubled = createMemo(() => count() * 2);
  return <button onClick={() => batch(() => setCount(value => value + 1))}>{doubled()}</button>;
}"#;
    for (target, host) in [
        (MoonshineModuleTarget::Dom, "@tschk/moonshine-dom/reactive"),
        (MoonshineModuleTarget::Ssr, "@tschk/moonshine-dom/ssr"),
    ] {
        let output = compile_moonshine_module(source, Path::new("Counter.tsx"), target).unwrap();
        assert_eq!(output.imports, ["solid-js", "solid-js", "solid-js"]);
        assert!(!output.source.contains("solid-js"), "{}", output.source);
        assert!(output.source.contains(host), "{}", output.source);
        assert!(output.source.contains("createSignal as signal"));
        assert!(output.source.contains("() => doubled()"));
    }
}

#[test]
fn reachable_non_jsx_helpers_also_use_native_primitives() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_module(
            "export { createSignal as state } from 'solid-js';",
            Path::new("state.ts"),
            target,
        )
        .unwrap();
        assert!(!output.source.contains("solid-js"));
        assert!(output.source.contains("@tschk/moonshine-dom/"));
    }
}

#[test]
fn unsupported_solid_surfaces_fail_in_both_targets_even_without_jsx() {
    for source in [
        "import * as Solid from 'solid-js'; export {Solid};",
        "import Solid from 'solid-js'; export {Solid};",
        "import 'solid-js'; export const x = 1;",
        "export * from 'solid-js';",
        "export * as Solid from 'solid-js';",
        "export const load = () => import('solid-js');",
        "export {createStore} from 'solid-js/store';",
        "export {render} from 'solid-js/web';",
        "export {createResource} from 'solid-js';",
        "export {For} from 'solid-js';",
        "export {createRoot} from 'solid-js';",
        "export {runWithOwner} from 'solid-js';",
        "export type X = import('solid-js').Owner;",
        "export type {Setter} from 'solid-js';",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_module(source, Path::new("helper.ts"), target).is_err(),
                "{source}"
            );
        }
    }
}

#[test]
fn solid_on_imports_aliases_reexports_and_type_queries_are_targeted() {
    let source = r#"
import { on as observe, createEffect } from 'solid-js';
import type { on as OnFunction } from 'solid-js';
export { on as watch } from 'solid-js';
export type Observe = typeof import('solid-js').on;
export type ImportedObserve = typeof OnFunction;
export const install = () => createEffect(observe(() => input(), (value, previousInput, previousValue) => value, { defer: true }));
"#;
    for (target, host) in [
        (MoonshineModuleTarget::Dom, "@tschk/moonshine-dom/reactive"),
        (MoonshineModuleTarget::Ssr, "@tschk/moonshine-dom/ssr"),
    ] {
        let output = compile_moonshine_module(source, Path::new("observe.ts"), target).unwrap();
        assert_eq!(
            output.imports,
            ["solid-js", "solid-js", "solid-js", "solid-js"]
        );
        assert!(!output.source.contains("solid-js"), "{}", output.source);
        assert_eq!(output.source.matches(host).count(), 4, "{}", output.source);
        assert!(output.source.contains("on as observe"), "{}", output.source);
        assert!(output.source.contains("on as watch"), "{}", output.source);
        assert!(output.source.contains("import type"), "{}", output.source);
        assert!(
            output.source.contains("typeof OnFunction"),
            "{}",
            output.source
        );
        assert!(output.source.contains(".on"), "{}", output.source);
        assert!(output.source.contains("defer: true"), "{}", output.source);
        assert!(
            !output.source.contains("import { h as"),
            "helper module gained JSX host import: {}",
            output.source
        );
    }
}

#[test]
fn native_reactive_on_helper_imports_retarget_to_ssr_without_jsx() {
    let source = r#"
import { on as observe, createEffect } from '@tschk/moonshine-dom/reactive';
export { on as watch } from '@tschk/moonshine-dom/reactive';
export type Observe = typeof import('@tschk/moonshine-dom/reactive').on;
export const install = () => createEffect(observe([first, second], (value) => value));
"#;
    for (target, host) in [
        (MoonshineModuleTarget::Dom, "@tschk/moonshine-dom/reactive"),
        (MoonshineModuleTarget::Ssr, "@tschk/moonshine-dom/ssr"),
    ] {
        let output = compile_moonshine_module(source, Path::new("helper.ts"), target).unwrap();
        assert_eq!(output.imports, ["@tschk/moonshine-dom/reactive"; 3]);
        assert_eq!(output.source.matches(host).count(), 3, "{}", output.source);
        if matches!(target, MoonshineModuleTarget::Ssr) {
            assert!(
                !output.source.contains("@tschk/moonshine-dom/reactive"),
                "{}",
                output.source
            );
        }
        assert!(
            output.source.contains("observe([first, second]"),
            "{}",
            output.source
        );
        assert!(
            !output.source.contains("import { h as"),
            "{}",
            output.source
        );
    }
}

#[test]
fn on_can_be_reexported_as_default_without_losing_export_metadata() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_module(
            "export { on as default } from 'solid-js';",
            Path::new("on.ts"),
            target,
        )
        .unwrap();
        assert!(output.has_default_export && output.has_module_declarations);
        assert_eq!(output.imports, ["solid-js"]);
        assert!(output.source.contains("on as default"), "{}", output.source);
        assert!(!output.source.contains("solid-js"), "{}", output.source);
    }
}
