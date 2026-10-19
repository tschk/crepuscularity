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
    unreachable_depth: usize,
    unreachable_shorthands: usize,
}

impl<'a> Visit<'a> for ProtoProperties {
    fn visit_logical_expression(&mut self, expression: &LogicalExpression<'a>) {
        let unreachable = expression.operator.as_str() == "&&"
            && matches!(&expression.left, Expression::BooleanLiteral(value) if !value.value);
        self.unreachable_depth += usize::from(unreachable);
        walk::walk_logical_expression(self, expression);
        self.unreachable_depth -= usize::from(unreachable);
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property
            .key
            .static_name()
            .is_some_and(|name| name == "__proto__")
        {
            if property.shorthand {
                self.raw_shorthands += 1;
                self.unreachable_shorthands += usize::from(self.unreachable_depth > 0);
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
    let output = compile_moonshine_svelte(source, Path::new("EachProto.svelte"), target).unwrap();
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
fn each_item_and_index_shorthand_preserve_proto_as_an_own_data_property() {
    for header in ["rows as __proto__", "rows as item, __proto__"] {
        let source = format!("<script>const rows=[{{id:1}}];</script>{{#each {header}}}<p>{{Object.keys({{__proto__}}).join(',')}}</p><p>{{(__proto__ => Object.keys({{__proto__}}).join(','))(2)}}</p>{{/each}}");
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let props = properties(&source, target);
            assert_eq!(props.computed_reads, 1, "{header}");
            assert_eq!(
                props.raw_shorthands, 1,
                "local parameter must stay shorthand"
            );
            assert_eq!(props.unsafe_setters, 0, "{header}");
        }
    }
}

#[test]
fn keyed_each_callback_keeps_its_raw_item_shorthand() {
    let source = "<script>const rows=[{id:1}];</script>{#each rows as __proto__ (Object.keys({__proto__}).join(','))}<p>{Object.keys({__proto__}).join(',')}</p>{/each}";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let props = properties(source, target);
        assert_eq!(props.computed_reads, 1);
        // SSR retains the raw key callback only in an unreachable type-checking branch.
        assert_eq!(props.raw_shorthands, 1);
        assert_eq!(
            props.unreachable_shorthands,
            usize::from(matches!(target, MoonshineModuleTarget::Ssr))
        );
        assert_eq!(props.unsafe_setters, 0);
    }
}
