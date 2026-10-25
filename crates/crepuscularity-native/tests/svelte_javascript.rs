#![cfg(feature = "web-modules")]
use crepuscularity_native::{
    compile_moonshine_svelte, MoonshineModuleLanguage, MoonshineModuleTarget,
};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use std::path::Path;

fn javascript(source: &str, target: MoonshineModuleTarget) -> String {
    let output = compile_moonshine_svelte(source, Path::new("App.svelte"), target).unwrap();
    assert_eq!(output.language, MoonshineModuleLanguage::JavaScript);
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &output.source, SourceType::mjs()).parse();
    assert!(
        parsed.errors.is_empty(),
        "{:?}\n{}",
        parsed.errors,
        output.source
    );
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(
        semantic.errors.is_empty(),
        "{:?}\n{}",
        semantic.errors,
        output.source
    );
    output.source
}
#[test]
fn pure_javascript_preserves_computed_public_overrides_and_private_syntax_guards() {
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        for source in [
            "<script>class Counter{x=$state(1);['x'](){return 9}}const model=new Counter();export function read(){return model.x()}</script><p>{read()}</p>",
            "<script>class Counter{constructor(){this.x=$state(1)}['x'](){return 9}}const model=new Counter();export function read(){return model.x()}</script><p>{read()}</p>",
        ] {
            let output = javascript(source, target);
            assert!(output.contains("return 9"), "{output}");
        }
        assert!(compile_moonshine_svelte(
            "<script>class C{#x;#x}</script><p/>",
            Path::new("App.svelte"),
            target
        )
        .is_err());
        assert!(compile_moonshine_svelte(
            "<script>const x:number=1;</script><p/>",
            Path::new("App.svelte"),
            target
        )
        .is_err());
    }
}
#[test]
fn aggregate_jsdoc_and_export_methods_keep_lexical_public_types() {
    let source = r#"<script>
/** @typedef {{label:string,initial?:number}} Props */
/** @type {Props} */ let {label,initial=1}=$props();let count=$state(initial);
/** @param {number} step @returns {number} */ export function increment(step){count+=step;return count;}
</script><p>{label}:{count}</p>"#;
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = javascript(source, target);
        assert!(
            output.contains("@typedef {{label:string,initial?:number}} Props"),
            "{output}"
        );
        assert!(output.contains("const __crepusSvelteJSProps"), "{output}");
        assert!(output.contains("ComponentView<typeof"), "{output}");
        assert!(
            output.contains("[\"increment\"]:typeof increment"),
            "{output}"
        );
        assert!(output.contains("export default function"), "{output}");
        assert!(
            output.contains(
                "@param {import(\"@tschk/moonshine-dom/platform\").GlobalTypes.Parameters<import("
            ),
            "{output}"
        );
        assert!(
            output.contains(
                "@returns {import(\"@tschk/moonshine-dom/platform\").GlobalTypes.ReturnType<"
            ),
            "{output}"
        );
        assert!(output.contains("|F:"), "{output}");
        if matches!(target, MoonshineModuleTarget::Ssr) {
            assert!(
                output.contains("@returns {import(\"@tschk/moonshine-dom/ssr\").SSRChild}"),
                "{output}"
            );
        }
    }
}
#[test]
fn typed_and_mixed_blocks_keep_existing_typed_delivery() {
    for source in [
        "<script lang='ts'>let {label}:{label:string}=$props()</script><p>{label}</p>",
        "<script module lang='ts'>export const n:number=1;</script><script>const value=n;</script><p>{value}</p>",
        "<script module>export const n=1;</script><script lang='ts'>const value:number=n;</script><p>{value}</p>",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert_eq!(
                compile_moonshine_svelte(source, Path::new("App.svelte"), target)
                    .unwrap()
                    .language,
                MoonshineModuleLanguage::TypeScript
            );
        }
    }
}
#[test]
fn unsupported_jsdoc_imports_fail_before_publishing_a_false_host_free_type_surface() {
    let error = compile_moonshine_svelte(
        "<script>/** @typedef {import('svelte').Snippet} S */const value=1;</script><p>{value}</p>",
        Path::new("App.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap_err();
    assert!(
        error.contains("unsupported pure-JS Svelte JSDoc import types"),
        "{error}"
    );
}

#[test]
fn jsdoc_aggregate_satisfies_is_explicit_until_its_projection_is_owned() {
    let error = compile_moonshine_svelte(
        "<script>/** @satisfies {{label:string}} */let {label}=$props();</script><p>{label}</p>",
        Path::new("App.svelte"),
        MoonshineModuleTarget::Dom,
    )
    .unwrap_err();
    assert!(
        error.contains("unsupported JavaScript JSDoc satisfies on props"),
        "{error}"
    );
}

#[test]
fn javascript_erasure_preserves_unused_value_imports_and_own_uninitialized_fields() {
    let source="<script>import {unused} from './side.js';class C{field;}export const model=new C();</script><p/>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = javascript(source, target);
        assert!(output.contains("./side.js"), "{output}");
        assert!(output.contains("field;"), "{output}");
    }
}

#[test]
fn javascript_const_actions_keep_the_current_await_scheduler_contract() {
    use oxc_ast::{ast::*, AstKind};
    let source = "<script>class Counter{x=$state(1);['x'](){return 9}}const input=new Counter();function action(node){return {destroy(){}}}</script>{#await input then value}{@const result=value.x()}<p use:action>{result}</p>{/await}";
    let output = javascript(source, MoonshineModuleTarget::Dom);
    assert!(!output.contains("__crepusDirective0"), "{output}");
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &output, SourceType::mjs()).parse();
    let alias = parsed
        .program
        .body
        .iter()
        .find_map(|statement| {
            let Statement::ImportDeclaration(import) = statement else {
                return None;
            };
            if import.source.value != "@tschk/moonshine-svelte/await" {
                return None;
            }
            import.specifiers.iter().flatten().find_map(|specifier| {
                let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier else {
                    return None;
                };
                (specifier.imported.name() == "awaitBlock").then_some(specifier.local.name.as_str())
            })
        })
        .expect("native await runtime import");
    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    let calls = semantic
        .nodes()
        .iter()
        .filter_map(|node| {
            let AstKind::CallExpression(call) = node.kind() else {
                return None;
            };
            call.callee.is_specific_id(alias).then_some(call)
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "{output}");
    let Some(Argument::ArrowFunctionExpression(schedule)) = calls[0].arguments.get(2) else {
        panic!("grouped await lost its inline scheduler: {output}");
    };
    assert!(
        schedule.expression && schedule.params.items.len() == 1,
        "{output}"
    );
    let BindingPattern::BindingIdentifier(parameter) = &schedule.params.items[0].pattern else {
        panic!("{output}");
    };
    let Some(Statement::ExpressionStatement(body)) = schedule.body.statements.first() else {
        panic!("{output}");
    };
    assert!(
        body.expression.is_specific_id(parameter.name.as_str()),
        "{output}"
    );
}
