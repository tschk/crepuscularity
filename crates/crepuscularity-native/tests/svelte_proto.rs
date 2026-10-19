#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;

#[derive(Default)]
struct ProtoProperties {
    computed_reads: usize,
    raw_shorthands: usize,
    unsafe_setters: usize,
}

impl<'a> Visit<'a> for ProtoProperties {
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property
            .key
            .static_name()
            .is_some_and(|name| name == "__proto__")
        {
            if property.shorthand {
                self.raw_shorthands += 1;
            } else if property.computed {
                self.computed_reads += 1;
            } else {
                self.unsafe_setters += 1;
            }
        }
        walk::walk_object_property(self, property);
    }
}

fn properties(source: &str, target: MoonshineModuleTarget) -> ProtoProperties {
    let output = compile_moonshine_svelte(source, Path::new("Proto.svelte"), target).unwrap();
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &output.source, SourceType::tsx()).parse();
    assert!(
        parsed.errors.is_empty(),
        "{:?}: {}",
        parsed.errors,
        output.source
    );
    let mut properties = ProtoProperties::default();
    properties.visit_program(&parsed.program);
    properties
}

#[test]
fn rune_and_prop_local_shorthand_preserves_own_proto_keys() {
    for declaration in [
        "let __proto__=$state({x:1});",
        "let __proto__=$state.raw({x:1});",
        "let __proto__=$derived({x:1});",
        "let {value:__proto__}=$props();",
    ] {
        let source = format!("<script>{declaration}</script><p>{{Object.keys({{__proto__}}).join(',')}}</p><p>{{(__proto__=>Object.keys({{__proto__}}).join(','))(2)}}</p>");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let props = properties(&source, target);
            assert_eq!(props.computed_reads, 1, "{declaration}");
            assert_eq!(
                props.raw_shorthands, 1,
                "local arrow parameter must remain shorthand"
            );
            assert_eq!(props.unsafe_setters, 0, "{declaration}");
        }
    }
}

#[test]
fn snippet_proto_parameters_only_need_computed_expansion_on_dom() {
    let source = "{#snippet row(__proto__)}<p>{Object.keys({__proto__}).join(',')}</p><p>{(__proto__=>Object.keys({__proto__}).join(','))(2)}</p>{/snippet}{@render row({x:1})}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let props = properties(source, target);
        let dom = matches!(target, MoonshineModuleTarget::Dom);
        assert_eq!(props.computed_reads, usize::from(dom));
        assert_eq!(props.raw_shorthands, if dom { 1 } else { 2 });
        assert_eq!(props.unsafe_setters, 0);
    }
}

#[test]
fn destructuring_assignment_to_a_rune_is_not_an_object_prototype_setter() {
    let source = "<script>let __proto__=$state.raw(0);function assign(input){({__proto__}=input);return __proto__}</script><button onclick={()=>assign({['__proto__']:2})}>{__proto__}</button>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output =
            compile_moonshine_svelte(source, Path::new("Assignment.svelte"), target).unwrap();
        let compact: String = output.source.split_whitespace().collect();
        assert!(
            compact.contains("__proto__:__proto__.value"),
            "{}",
            output.source
        );
        let props = properties(source, target);
        assert_eq!(props.unsafe_setters, 0);
    }
}
