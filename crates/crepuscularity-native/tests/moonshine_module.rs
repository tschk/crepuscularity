#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_module, try_compile_moonshine_module, MoonshineModuleTarget as Target,
};
use std::path::Path;
fn compile(source: &str) -> crepuscularity_native::MoonshineModule {
    compile_moonshine_module(source, Path::new("Counter.tsx"), Target::Dom).unwrap()
}
#[test]
fn asserted_assignment_targets_keep_required_typescript_parentheses() {
    for target in [Target::Dom, Target::Ssr] {
        let output = compile_moonshine_module("export default function C(){class Klass{};(Klass as any)=class Next{};let value=1;(value satisfies number)=2;return <p>{value}</p>}", Path::new("Asserted.tsx"), target).unwrap();
        assert!(
            output.source.contains("(Klass as any) = class Next"),
            "{}",
            output.source
        );
        assert!(output.source.contains("(value satisfies number) = 2"));
    }
}
#[test]
fn counter_keeps_module_and_local_reactivity() {
    let output = compile(
        r#"import { createSignal } from '@tschk/moonshine-dom/reactive';
export default function Counter({ initial = 0 }: {initial?: number}) {
const [count,setCount] = createSignal(initial);
return <button title={count()} onClick={() => setCount(count()+1)}>Count: {count()}</button>;
}"#,
    );
    assert!(output.has_default_export && output.has_module_declarations);
    assert_eq!(output.imports, ["@tschk/moonshine-dom/reactive"]);
    assert!(output.source.contains("@tschk/moonshine-dom/dom"));
    assert!(output.source.contains("export default function Counter"));
    assert!(
        output.source.contains("initial?: number"),
        "{}",
        output.source
    );
    assert!(
        output.source.contains("title: () => count()"),
        "{}",
        output.source
    );
    assert!(output
        .source
        .contains("onClick: () => setCount(count() + 1)"));
    assert!(output.source.contains("() => count()"));
    assert!(!output.source.contains("react/jsx") && !output.source.contains("React.createElement"));
}
#[test]
fn metadata_entities_nested_jsx_and_hygienic_alias() {
    let output = compile(
        r#"'use client';
import type { Foo } from './types';
export { something } from './helper';
export * from './all';
const __crepusH0 = 1;
export const App = () => <div>{ok() ? <span>A &amp; B</span> : <p>{rows().map(row => <span>{row.name}</span>)}</p>}</div>;
const lazy = () => import('./lazy');
export { App as default };"#,
    );
    assert_eq!(output.imports, ["./types", "./helper", "./all", "./lazy"]);
    assert!(
        output.source.contains("import type { Foo }"),
        "{}",
        output.source
    );
    assert!(output.has_default_export);
    assert!(
        output.source.starts_with("\"use client\";"),
        "{}",
        output.source
    );
    assert!(output.source.contains("h as __crepusH1"));
    assert!(output.source.contains("A & B"));
    assert!(output.source.contains("() => row.name"));
    assert!(!output.source.contains("<span>"));
    assert!(output.source.contains("import(\"./lazy\")"));
}
#[test]
fn detection_and_ssr_do_not_execute_source() {
    assert!(
        try_compile_moonshine_module("<button>Hi</button>", Path::new("A.tsx"), Target::Dom)
            .unwrap()
            .is_none()
    );
    assert!(try_compile_moonshine_module(
        "export default function (",
        Path::new("A.tsx"),
        Target::Dom
    )
    .is_err());
    let output = compile_moonshine_module(
        "throw new Error('never execute'); export default () => <span/>;",
        Path::new("A.tsx"),
        Target::Ssr,
    )
    .unwrap();
    assert!(output.source.contains("@tschk/moonshine-dom/ssr"));
    assert!(output.source.contains("throw new Error"));
}
#[test]
fn unsupported_jsx_and_imports_fail_explicitly() {
    for source in [
        "/** @jsxRuntime automatic */ export default () => <div/>",
        "export default () => <div {...props}/>",
        "export default () => <svg/>",
        "export default () => <script/>",
        "export default () => <div ref={callback}/>",
        "export default () => <div dangerouslySetInnerHTML={html}/>",
        "export default () => <button onClick='code'/>",
        "export default () => <div title='a' title='b'/>",
        "export default async () => <div>{await value}</div>",
        "export default () => import(path)",
    ] {
        assert!(
            compile_moonshine_module(source, Path::new("A.tsx"), Target::Dom).is_err(),
            "{source}"
        );
    }
}

#[test]
fn type_only_default_exports_do_not_create_runtime_defaults() {
    for source in [
        "interface X {} export type { X as default };",
        "interface X {} export { type X as default };",
        "export default interface X {}",
    ] {
        let output = compile(source);
        assert!(output.has_module_declarations);
        assert!(!output.has_default_export, "{source}");
    }
    assert!(compile("export * as default from './other';").has_default_export);
    assert!(!compile("export type * as default from './other';").has_default_export);
    let output = compile("import type { X } from './types'; export const answer: X = 42;");
    assert!(output.source.contains("import type { X }"));
    assert!(output.source.contains("answer: X"));
}

#[test]
fn hashbang_directives_and_authored_literal_bytes_survive_lowering() {
    let output = compile("#!/usr/bin/env bun\n'use client';\nexport default () => <p title={'quote \\\" slash \\\\'}>a &lt; b {'\\n'}</p>;");
    assert!(
        output
            .source
            .starts_with("#!/usr/bin/env bun\n\"use client\";"),
        "{}",
        output.source
    );
    assert!(output.source.contains("a < b"));
    assert!(!output.source.contains("@oxc-project/runtime"));
    assert!(!output.source.contains("react/jsx-runtime"));
}

#[test]
fn ssr_retargets_named_reactive_lifecycle_imports_and_reexports() {
    let source = r#"import { createEffect as effect, onMount as mounted, createSignal, type Accessor } from '@tschk/moonshine-dom/reactive';
export { createEffect as sideEffect, onMount, type Setter } from '@tschk/moonshine-dom/reactive';
export default function App() { effect(() => { throw Error('browser only'); }); mounted(() => { throw Error('mount only'); }); const [count] = createSignal(2); return <span>{count()}</span>; }"#;
    let output = compile_moonshine_module(source, Path::new("A.tsx"), Target::Ssr).unwrap();
    assert!(
        !output.source.contains("@tschk/moonshine-dom/reactive"),
        "{}",
        output.source
    );
    assert!(output.source.contains("createEffect as effect"));
    assert!(output.source.contains("onMount as mounted"));
    assert!(output.source.contains("type Accessor"));
    assert!(output.source.contains("createEffect as sideEffect"));
    assert_eq!(
        output.imports,
        [
            "@tschk/moonshine-dom/reactive",
            "@tschk/moonshine-dom/reactive"
        ]
    );
    assert!(compile(source)
        .source
        .contains("@tschk/moonshine-dom/reactive"));
}

#[test]
fn ssr_rejects_unsupported_reactive_import_surfaces() {
    for source in [
        "import * as r from '@tschk/moonshine-dom/reactive'; export default r;",
        "import r from '@tschk/moonshine-dom/reactive'; export default r;",
        "export * from '@tschk/moonshine-dom/reactive';",
        "export * as r from '@tschk/moonshine-dom/reactive';",
        "export const load = () => import('@tschk/moonshine-dom/reactive');",
        "import { createRenderEffect } from '@tschk/moonshine-dom/reactive'; export { createRenderEffect };",
        "export { unknown } from '@tschk/moonshine-dom/reactive';",
    ] {
        assert!(compile_moonshine_module(source, Path::new("A.tsx"), Target::Ssr).is_err(), "{source}");
    }
}

#[test]
fn type_import_expressions_are_collected_and_retargeted_for_ssr() {
    let source = "export type P = import('./types').Props; export type A = import('@tschk/moonshine-dom/reactive').Accessor<number>;";
    let dom = compile(source);
    assert_eq!(dom.imports, ["./types", "@tschk/moonshine-dom/reactive"]);
    assert!(dom.source.contains("import(\"./types\").Props"));
    assert!(dom.source.contains("@tschk/moonshine-dom/reactive"));
    let ssr = compile_moonshine_module(source, Path::new("A.tsx"), Target::Ssr).unwrap();
    assert_eq!(ssr.imports, dom.imports);
    assert!(ssr
        .source
        .contains("import(\"@tschk/moonshine-dom/ssr\").Accessor<number>"));
    assert!(!ssr.source.contains("@tschk/moonshine-dom/reactive"));
}

#[test]
fn commonjs_module_forms_are_explicit_errors() {
    for source in [
        "export const value = require('./helper');",
        "export const value = (require)('./helper');",
        "import helper = require('./helper'); export default helper;",
        "export const resolve = require.resolve('./helper');",
    ] {
        let error = compile_moonshine_module(source, Path::new("A.tsx"), Target::Dom).unwrap_err();
        assert!(error.contains("unsupported"), "{source}: {error}");
    }
}

#[test]
fn uppercase_module_extensions_match_template_dispatch() {
    let source = "export default function Counter() { return <button>Count</button>; }";
    let output = compile_moonshine_module(source, Path::new("Counter.TSX"), Target::Dom).unwrap();
    assert!(output.source.contains("@tschk/moonshine-dom/dom"));
    assert!(try_compile_moonshine_module(
        "<div>Template</div>",
        Path::new("view.JSX"),
        Target::Dom
    )
    .unwrap()
    .is_none());
}

#[test]
fn aside_preserves_semantic_tag_and_reactive_attributes_for_both_hosts() {
    for target in [Target::Dom, Target::Ssr] {
        let output = compile_moonshine_module(
            "export default function C(props){return <aside aria-label={props.label}>{props.text}</aside>}",
            Path::new("Aside.tsx"), target,
        ).unwrap();
        assert!(output.source.contains("\"aside\""), "{}", output.source);
        assert!(
            output.source.contains("\"aria-label\""),
            "{}",
            output.source
        );
        assert!(
            output.source.contains("() => props.text"),
            "{}",
            output.source
        );
    }
}

#[test]
fn definition_lists_preserve_semantic_tags_in_both_hosts() {
    for target in [Target::Dom, Target::Ssr] {
        let output = compile_moonshine_module(
            "export default function C(props){return <dl aria-label={props.label}><dt>{props.term}</dt><dd>{props.meaning}</dd></dl>}",
            Path::new("DefinitionList.tsx"), target,
        ).unwrap();
        for tag in ["dl", "dt", "dd"] {
            assert!(
                output.source.contains(&format!("\"{tag}\"")),
                "{}",
                output.source
            );
        }
        assert!(output.source.contains("() => props.term"));
        assert!(output.source.contains("() => props.meaning"));
    }
}

#[test]
fn companion_modules_reject_unlowered_runes_without_rejecting_ordinary_javascript() {
    for target in [Target::Dom, Target::Ssr] {
        for filename in [
            "counter.svelte.ts",
            "counter.svelte.js",
            "nested/counter.SVELTE.TS",
        ] {
            for expression in ["$state(1)", "$state.raw({})"] {
                let source = format!("export const value={expression};");
                let output = compile_moonshine_module(&source, Path::new(filename), target)
                    .expect("stable exported companion state is legal");
                let helper = match (target, expression) {
                    (Target::Dom, "$state.raw({})") => "createRawState as",
                    (Target::Dom, _) => "createState as",
                    (Target::Ssr, _) => "createServerState as",
                };
                assert!(output.source.contains(helper), "{}", output.source);
                assert!(output.source.contains(").value"), "{}", output.source);
            }
            for expression in [
                "$derived.by(()=>1)",
                "$effect(()=>{})",
                "$props()",
                "$bindable()",
                "$inspect(1)",
                "$host()",
                "$state",
                "typeof $state",
            ] {
                let source = format!("export const value={expression};");
                let error =
                    compile_moonshine_module(&source, Path::new(filename), target).unwrap_err();
                assert!(error.contains(filename), "{error}");
                if expression == "$effect(()=>{})" {
                    assert!(error.contains("$effect must be a standalone"), "{error}");
                } else if expression == "$derived.by(()=>1)" {
                    assert!(error.contains("cannot export a derived binding"), "{error}");
                } else if expression == "$props()" {
                    assert!(
                        error.contains("outside component initialization"),
                        "{error}"
                    );
                } else {
                    assert!(
                        error.contains("Svelte rune") && error.contains("unsupported"),
                        "{error}"
                    );
                }
            }
            for source in [
                "// $state(1)\nexport const text='$effect()';",
                "function $state(value){return value;}export const value=$state(1);",
                "import {$state} from './custom';export const value=$state(1);",
                "export const obj={$state:1};export const value=obj.$state;",
            ] {
                assert!(
                    compile_moonshine_module(source, Path::new(filename), target).is_ok(),
                    "{filename}: {source}"
                );
            }
        }
        for source in [
            "export type State=typeof $state;",
            "export type State=$state;",
        ] {
            assert!(
                compile_moonshine_module(source, Path::new("types.svelte.ts"), target).is_ok(),
                "{source}"
            );
        }
        for filename in ["counter.ts", "counter.js", "counter.svelte-helper.ts"] {
            assert!(compile_moonshine_module(
                "export const value=$state(1)",
                Path::new(filename),
                target
            )
            .is_ok());
        }
    }
}

#[test]
fn companion_state_factories_lower_without_leaking_cells_in_exports() {
    let source="export function counter(initial){let count=$state(initial);let raw=$state.raw({step:1});let twice=$derived(count*2);let total=$derived.by(()=>twice+raw.step);return {get count(){return count},get total(){return total},increment(){count+=raw.step}}}";
    for filename in [
        "counter.svelte.ts",
        "counter.svelte.js",
        "counter.SVELTE.JS",
    ] {
        for target in [Target::Dom, Target::Ssr] {
            let output = compile_moonshine_module(source, Path::new(filename), target).unwrap();
            assert!(
                !output.source.contains("$state") && !output.source.contains("$derived"),
                "{}",
                output.source
            );
            assert!(output.source.contains("count.value"), "{}", output.source);
            assert!(output.source.contains("export function counter"));
            if matches!(target, Target::Ssr) {
                assert!(!output.source.contains("createState as"));
                assert!(!output.source.contains("createDerived as"));
                assert!(output.source.contains("createServerState as"));
                assert!(output.source.contains("createServerDerived as"));
                assert!(output.source.contains("@tschk/moonshine-svelte/ssr"));
                assert!(!output.source.contains("isSSRRendering as"));
            }
            let allocator = oxc_allocator::Allocator::default();
            let source_type = if filename.to_ascii_lowercase().ends_with(".js") {
                oxc_span::SourceType::mjs()
            } else {
                oxc_span::SourceType::ts()
            };
            let parsed = oxc_parser::Parser::new(&allocator, &output.source, source_type).parse();
            assert!(
                parsed.errors.is_empty(),
                "{}\n{:?}",
                output.source,
                parsed.errors
            );
        }
    }
}

#[test]
fn companion_sync_factory_effects_lower_on_both_hosts_and_keep_javascript_valid() {
    let source = "export function createCounter(initial=0){let count=$state(initial);$effect(()=>{globalThis.observed=count;return()=>{globalThis.cleaned=count}});return {get count(){return count},increment(){count++}}}";
    for path in [
        "counter.svelte.ts",
        "counter.svelte.js",
        "counter.svelte.JS",
    ] {
        for target in [Target::Dom, Target::Ssr] {
            let compiled = compile_moonshine_module(source, Path::new(path), target).unwrap();
            assert!(!compiled.source.contains("$effect("));
            let allocator = oxc_allocator::Allocator::default();
            let source_type = if path.ends_with(".ts") {
                oxc_span::SourceType::ts()
            } else {
                oxc_span::SourceType::mjs()
            };
            let parsed = oxc_parser::Parser::new(&allocator, &compiled.source, source_type).parse();
            assert!(
                parsed.errors.is_empty(),
                "{:?}\n{}",
                parsed.errors,
                compiled.source
            );
            if matches!(target, Target::Dom) {
                assert!(compiled.source.contains("createRuneEffect as"));
                assert!(compiled.source.contains("count.value"));
                assert!(compiled.source.contains("globalThis.cleaned"));
            } else {
                assert!(!compiled.source.contains("createRuneEffect"));
                assert!(!compiled.source.contains("globalThis.observed"));
                assert!(!compiled.source.contains("globalThis.cleaned"));
            }
        }
    }
}

#[test]
fn companion_server_effect_erasure_removes_argument_evaluation_and_browser_references() {
    let source = "export function createCounter(){let count=$state(0);$effect(effectFactoryThatMustNotRun(document));return()=>count}";
    let compiled =
        compile_moonshine_module(source, Path::new("counter.svelte.ts"), Target::Ssr).unwrap();
    for absent in [
        "effectFactoryThatMustNotRun",
        "document",
        "createRuneEffect",
        "$effect",
    ] {
        assert!(!compiled.source.contains(absent), "{}", compiled.source);
    }
}

#[test]
fn companion_effects_distinguish_ambient_runes_from_bound_functions_and_preserve_generic_factories()
{
    for target in [Target::Dom, Target::Ssr] {
        let source = "declare const $effect:(callback:()=>void)=>void;export const createCounter=<T,>(initial:T)=>{let count=$state(initial);$effect(()=>{void count});return()=>count}";
        let compiled =
            compile_moonshine_module(source, Path::new("generic.svelte.ts"), target).unwrap();
        assert!(compiled.source.contains("<T"));
        assert!(!compiled.source.contains("$effect(()"));
        let allocator = oxc_allocator::Allocator::default();
        assert!(
            oxc_parser::Parser::new(&allocator, &compiled.source, oxc_span::SourceType::ts())
                .parse()
                .errors
                .is_empty()
        );
        for source in [
            "function $effect(callback){return callback};$effect.pre=callback=>callback;export function run(){$effect(()=>1);$effect.pre(()=>2)}",
            "import $effect from './effects';export function run(){$effect(()=>1);$effect.tracking()}",
        ] {
            let compiled = compile_moonshine_module(source, Path::new("bound.svelte.js"), target).unwrap();
            assert!(compiled.source.contains("$effect("));
            assert!(!compiled.source.contains("createRuneEffect"));
        }
    }
}

#[test]
fn companion_effects_keep_unsupported_registration_forms_fail_closed() {
    for source in [
        "export const value=1;$effect(()=>{})",
        "export async function make(){$effect(()=>{})}",
        "export function* make(){$effect(()=>{})}",
        "export function make(){const value=$effect(()=>{})}",
        "export function make(){return $effect(()=>{})}",
        "export function make(){$effect(...callbacks)}",
        "export function make(){$effect<number>(()=>{})}",
        "export function make(){$effect?.(()=>{})}",
        "export function make(){$effect.pre(()=>{})}",
        "export function make(){$effect.root(()=>{})}",
        "export function make(){$effect.tracking()}",
        "export function make(){$effect.missing(()=>{})}",
    ] {
        for target in [Target::Dom, Target::Ssr] {
            assert!(
                compile_moonshine_module(source, Path::new("invalid.svelte.ts"), target).is_err(),
                "accepted {source}"
            );
        }
    }
}
