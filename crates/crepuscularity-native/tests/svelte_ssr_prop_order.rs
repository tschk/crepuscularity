#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_svelte, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, ObjectProperty, ObjectPropertyKind};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;

#[derive(Debug)]
struct Descriptor {
    name: String,
    eager_call: Option<String>,
    getter: bool,
}

fn descriptors(source: &str) -> Vec<Descriptor> {
    #[derive(Default)]
    struct Collect(Vec<Descriptor>);
    impl<'a> Visit<'a> for Collect {
        fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
            if let Some(name) = property.key.static_name() {
                if matches!(name.as_ref(), "used" | "unused") {
                    if let Expression::ObjectExpression(descriptor) =
                        property.value.get_inner_expression()
                    {
                        let mut result = Descriptor {
                            name: name.to_string(),
                            eager_call: None,
                            getter: false,
                        };
                        for field in &descriptor.properties {
                            let ObjectPropertyKind::ObjectProperty(field) = field else {
                                continue;
                            };
                            match field.key.static_name().as_deref() {
                                Some("get") => result.getter = true,
                                Some("value") => {
                                    if let Expression::CallExpression(call) =
                                        field.value.get_inner_expression()
                                    {
                                        result.eager_call = call
                                            .callee
                                            .get_identifier_reference()
                                            .map(|id| id.name.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }
                        self.0.push(result);
                    }
                }
            }
            walk::walk_object_property(self, property);
        }
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    assert!(parsed.errors.is_empty(), "{:?}: {source}", parsed.errors);
    let mut collect = Collect::default();
    collect.visit_program(&parsed.program);
    collect.0
}

#[test]
fn ordinary_ssr_component_props_are_eager_even_when_the_child_does_not_read_them() {
    let source = "<script lang='ts'>import Child from './Child'; function read(){return 1} function unused(){return 2}</script><Child used={read()} unused={unused()}/>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_svelte(source, Path::new("Props.svelte"), target).unwrap();
        let descriptors = descriptors(&output.source);
        assert_eq!(descriptors.len(), 2, "{}", output.source);
        assert_eq!(
            descriptors
                .iter()
                .map(|d| d.name.as_str())
                .collect::<Vec<_>>(),
            ["used", "unused"]
        );
        for (descriptor, call) in descriptors.iter().zip(["read", "unused"]) {
            match target {
                MoonshineModuleTarget::Ssr => {
                    assert_eq!(
                        descriptor.eager_call.as_deref(),
                        Some(call),
                        "{descriptor:?}: {}",
                        output.source
                    );
                    assert!(!descriptor.getter, "{}", output.source);
                }
                MoonshineModuleTarget::Dom => {
                    assert!(descriptor.getter, "{}", output.source);
                    assert!(descriptor.eager_call.is_none(), "{}", output.source);
                }
            }
        }
    }
}

#[test]
fn spreads_and_component_binding_descriptors_survive_eager_ordinary_props() {
    let source = "<script lang='ts'>import Child from './Child';let value=$state('initial');function read(){return 1}function unused(){return 2}const extra={other:3};</script><Child used={read()} {...extra} bind:value unused={unused()}/>";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output =
            compile_moonshine_svelte(source, Path::new("SpreadProps.svelte"), target).unwrap();
        let descriptors = descriptors(&output.source);
        assert_eq!(descriptors.len(), 2, "{}", output.source);
        assert!(
            output.source.contains("set:"),
            "binding setter lost: {}",
            output.source
        );
        assert!(
            output.source.contains("get:"),
            "binding getter lost: {}",
            output.source
        );
        let expected = match target {
            MoonshineModuleTarget::Dom => "spreadProps",
            MoonshineModuleTarget::Ssr => "spreadServerProps",
        };
        assert!(output.source.contains(expected), "{}", output.source);
        if matches!(target, MoonshineModuleTarget::Ssr) {
            assert!(
                descriptors
                    .iter()
                    .all(|d| d.eager_call.is_some() && !d.getter),
                "{descriptors:?}: {}",
                output.source
            );
        }
    }
}
