#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;

#[test]
fn typed_component_helpers_preserve_module_hashbang_and_directive_order() {
    for hashbang in ["#!/usr/bin/env bun\n", ""] {
        let source = format!("{hashbang}'use client'; 'use strict'; function Child(props:{{label:string}}){{return <p>{{props.label}}</p>}} export default function Parent(){{return <Child label=\"value\"/>}}");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_module(&source, Path::new("Parent.tsx"), target)
                .expect("generated helpers must follow the module preamble");
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output.source, SourceType::tsx()).parse();
            assert!(
                parsed.errors.is_empty(),
                "{:?}: {}",
                parsed.errors,
                output.source
            );
            assert_eq!(parsed.program.hashbang.is_some(), !hashbang.is_empty());
            if !hashbang.is_empty() {
                assert!(
                    output.source.starts_with("#!/usr/bin/env bun\n"),
                    "{}",
                    output.source
                );
            }
            let directives: Vec<_> = parsed
                .program
                .directives
                .iter()
                .map(|directive| directive.expression.value.as_str())
                .collect();
            assert_eq!(
                directives,
                ["use client", "use strict"],
                "{}",
                output.source
            );
            assert!(output.has_default_export);
        }
    }
}
