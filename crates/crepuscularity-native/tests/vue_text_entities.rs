#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_vue, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_ast::ast::{StaticMemberExpression, StringLiteral};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::Path;

#[derive(Default)]
struct OutputValues {
    strings: Vec<String>,
    label_reads: usize,
}

impl<'a> Visit<'a> for OutputValues {
    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        self.strings.push(literal.value.to_string());
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if member.property.name == "label" {
            self.label_reads += 1;
        }
        walk::walk_static_member_expression(self, member);
    }
}

fn source(markup: &str) -> String {
    format!("<script setup lang=\"ts\">const props=defineProps<{{label:string}}>();</script><template>{markup}</template>")
}

#[test]
fn literal_text_entities_decode_once_without_changing_runtime_interpolations() {
    for (literal, expected) in [
        ("A &amp; B &#65;", "A & B A"),
        ("&#x1F600;", "😀"),
        ("&NotEqualTilde;", "≂\u{338}"),
        ("&unknown;", "&unknown;"),
        ("&amp;lt;", "&lt;"),
        ("A & B", "A & B"),
        ("&#0;", "\u{fffd}"),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_vue(
                &source(&format!("<p>{literal}</p><p>{{{{props.label}}}}</p>")),
                Path::new("Entities.vue"),
                target,
            )
            .unwrap();
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output.source, SourceType::tsx()).parse();
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let mut values = OutputValues::default();
            values.visit_program(&parsed.program);
            assert!(
                values.strings.iter().any(|value| value == expected),
                "{literal} -> {expected:?}: {}",
                output.source
            );
            assert_eq!(values.label_reads, 1, "{}", output.source);
            assert!(!values.strings.iter().any(|value| value == "props.label"));
        }
    }
}

#[test]
fn attribute_entities_and_unimplemented_decoded_whitespace_remain_explicit_errors() {
    for (markup, diagnostic) in [
        ("<p title=\"A &amp; B\">Text</p>", "attribute entities"),
        ("<p id=\"a&amp;b\">Text</p>", "attribute entities"),
        ("<p>a&#10;b</p>", "whitespace"),
        ("<p>a&#32;&#32;b</p>", "whitespace"),
        ("<p>&#32;Text</p>", "whitespace"),
        ("<p>Text&#32;</p>", "whitespace"),
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let error = compile_moonshine_vue(
                &source(markup),
                Path::new("UnsupportedEntities.vue"),
                target,
            )
            .unwrap_err();
            assert!(error.contains(diagnostic), "{markup}: {error}");
        }
    }
}
