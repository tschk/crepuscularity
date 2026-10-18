#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use std::path::Path;

fn compile(
    source: &str,
    javascript: bool,
    target: MoonshineModuleTarget,
) -> Result<String, String> {
    compile_moonshine_module(
        source,
        Path::new(if javascript {
            "model.svelte.js"
        } else {
            "model.svelte.ts"
        }),
        target,
    )
    .map(|module| module.source)
}
fn valid_javascript(source: &str) {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    assert!(parsed.errors.is_empty(), "{source}\n{:?}", parsed.errors);
    let checked = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(checked.errors.is_empty(), "{source}\n{:?}", checked.errors);
}
#[test]
fn ordinary_javascript_duplicate_members_keep_source_order() {
    for source in [
        "export class Model{x(){return 1}x(){return 2}}",
        "export class Model{x=1;x=2}",
        "export class Model{get x(){return 1}get ['x'](){return 2}}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile(source, true, target).expect("valid JS public duplicates");
            valid_javascript(&output);
            let first = output.find("1").expect("first original member");
            let second = output.find("2").expect("second original member");
            assert!(first < second, "{output}");
            assert!(
                compile(source, false, target).is_err(),
                "typed duplicate must not inherit JS policy"
            );
        }
    }
}
#[test]
fn constructor_computed_members_and_generated_type_helpers_emit_javascript() {
    for key in ["'x'", "key"] {
        for member in [
            format!("[{key}](){{return 9}}"),
            format!("get [{key}](){{return 9}}"),
        ] {
            for declaration in ["", "x;"] {
                let source=format!("const key='x';export class Model{{{member};{declaration}constructor(){{this.x=$state(1)}}read(){{return this.x}}}}export function factory(input){{let [first]=$derived(input);let callback=$state(()=>first);return()=>callback()}}");
                for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
                    let output = compile(&source, true, target)
                        .expect("JS computed member plus helper erasure");
                    valid_javascript(&output);
                    assert!(!output.contains("$state("), "{output}");
                    assert!(!output.contains("$derived("), "{output}");
                    if matches!(target, MoonshineModuleTarget::Dom) {
                        assert!(
                            output.find("get x()").expect("generated accessor")
                                < output
                                    .find(if key == "'x'" { "[\"x\"]()" } else { "[key]()" })
                                    .expect("authored computed member"),
                            "{output}"
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn invalid_original_javascript_never_enters_trusted_type_ir() {
    for source in [
        "export const value:number=1",
        "interface Model{value:number};export const value=1",
        "export class Model{#x;#x}",
        "export class Model{#x;constructor(){this.#x=0;this.#x=$state(1)}}",
        "export class Model{#x=0;constructor(){this.#x=$state(1)}}",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(compile(source, true, target).is_err(), "{source}");
        }
    }
}

#[test]
fn typed_companion_keeps_original_initialization_check_representation() {
    let source = "export class Model{x:number;constructor(){console.log(this.x);this.x=$state(1)}}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output =
            compile(source, false, target).expect("typed constructor remains a typed module");
        let allocator = Allocator::default();
        assert!(
            Parser::new(&allocator, &output, SourceType::ts())
                .parse()
                .errors
                .is_empty(),
            "{output}"
        );
        assert!(
            !Parser::new(&allocator, &output, SourceType::mjs())
                .parse()
                .errors
                .is_empty(),
            "TS annotations must not be erased: {output}"
        );
        assert!(
            output.contains("number"),
            "original field type remains: {output}"
        );
    }
}
