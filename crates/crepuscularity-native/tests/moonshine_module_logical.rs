#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, CallExpression, Expression, ImportDeclarationSpecifier, Statement};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

#[derive(Debug)]
struct Choice {
    callee: String,
    arguments: Vec<String>,
    returned_callees: Vec<Option<String>>,
}

fn choices(source: &str, jsx: bool) -> (Vec<Choice>, String) {
    struct Calls<'s> {
        source: &'s str,
        calls: Vec<Choice>,
    }
    impl<'a> Visit<'a> for Calls<'_> {
        fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
            if call.arguments.len() >= 3
                && call
                    .arguments
                    .iter()
                    .take(3)
                    .all(|value| matches!(value, Argument::ArrowFunctionExpression(_)))
            {
                let span = call.callee.span();
                let callee = self.source[span.start as usize..span.end as usize].to_string();
                if callee.starts_with("__crepus") {
                    let arguments = call
                        .arguments
                        .iter()
                        .map(|argument| {
                            let span = argument.span();
                            self.source[span.start as usize..span.end as usize]
                                .chars()
                                .filter(|character| !character.is_whitespace())
                                .collect()
                        })
                        .collect();
                    let returned_callees = call
                        .arguments
                        .iter()
                        .map(|argument| {
                            let Argument::ArrowFunctionExpression(factory) = argument else {
                                return None;
                            };
                            let Some(Statement::ExpressionStatement(body)) =
                                factory.body.statements.first()
                            else {
                                return None;
                            };
                            let Expression::CallExpression(returned) =
                                body.expression.get_inner_expression()
                            else {
                                return None;
                            };
                            let Expression::Identifier(identifier) =
                                returned.callee.get_inner_expression()
                            else {
                                return None;
                            };
                            Some(identifier.name.to_string())
                        })
                        .collect();
                    self.calls.push(Choice {
                        callee,
                        arguments,
                        returned_callees,
                    });
                }
            }
            walk::walk_call_expression(self, call);
        }
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(
        &allocator,
        source,
        if jsx {
            SourceType::jsx()
        } else {
            SourceType::tsx()
        },
    )
    .parse();
    assert!(parsed.errors.is_empty(), "{:?}: {source}", parsed.errors);
    let mut memo = None;
    for statement in &parsed.program.body {
        if let Statement::ImportDeclaration(import) = statement {
            for specifier in import.specifiers.iter().flatten() {
                if let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier {
                    if specifier.imported.name() == "createMemo" {
                        memo = Some(specifier.local.name.to_string());
                    }
                }
            }
        }
    }
    let mut visitor = Calls {
        source,
        calls: vec![],
    };
    visitor.visit_program(&parsed.program);
    (visitor.calls, memo.expect("branch helper memo import"))
}

fn imported_alias(source: &str, name: &str) -> String {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    for statement in &parsed.program.body {
        if let Statement::ImportDeclaration(import) = statement {
            for specifier in import.specifiers.iter().flatten() {
                if let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier {
                    if specifier.imported.name() == name {
                        return specifier.local.name.to_string();
                    }
                }
            }
        }
    }
    panic!("missing import {name}: {source}");
}

fn compile_both(source: &str, extension: &str) -> Vec<String> {
    [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr]
        .into_iter()
        .map(|target| {
            compile_moonshine_module(source, Path::new(&format!("Logical.{extension}")), target)
                .unwrap()
                .source
        })
        .collect()
}

#[test]
fn logical_jsx_preserves_left_once_and_returns_raw_left_through_a_memo() {
    for operator in ["&&", "||", "??"] {
        let source =
            format!("export const App = () => <div>{{sourceRead() {operator} <Child/>}}</div>;");
        for output in compile_both(&source, "tsx") {
            let (calls, memo) = choices(&output, false);
            assert_eq!(calls.len(), 1, "{calls:?}: {output}");
            let call = &calls[0];
            assert_eq!(output.matches("sourceRead()").count(), 1, "{output}");
            assert!(call.arguments[0].contains("sourceRead()"), "{output}");
            let (rhs, left) = if operator == "&&" { (1, 2) } else { (2, 1) };
            assert!(call.arguments[rhs].contains("Child"), "{output}");
            let (parameter, body) = call.arguments[left].split_once("=>").unwrap();
            let parameter = parameter.trim_matches(['(', ')']);
            assert_eq!(
                body,
                format!("{memo}({parameter})"),
                "left result must be read, not called as an authored function: {output}"
            );
            if operator == "??" {
                assert_eq!(call.arguments[3], "true", "{output}");
            }
        }
    }
}

#[test]
fn logical_rhs_works_in_fragments_components_props_and_typescript_wrappers() {
    for source in [
        "export const App = () => <>{sourceRead() && <Child/>}</>;",
        "export const App = () => <Frame>{sourceRead() || <Child/>}</Frame>;",
        "export const App = () => <Frame view={sourceRead() ?? <Child/>}/>;",
        "export const App = () => <div>{(sourceRead() && <Child/>) as any}</div>;",
    ] {
        for output in compile_both(source, "tsx") {
            let (calls, _) = choices(&output, false);
            assert_eq!(calls.len(), 1, "{output}");
            assert_eq!(output.matches("sourceRead()").count(), 1, "{output}");
            assert!(!output.contains("<Child"), "{output}");
        }
    }
}

#[test]
fn nested_logical_and_ternary_right_branches_use_independent_helpers() {
    for expression in [
        "sourceRead() && (secondRead() || <Child/>)",
        "sourceRead() ?? (secondRead() && <Child/>)",
        "sourceRead() || (secondRead() ? <One/> : <Two/>)",
    ] {
        let source = format!("export const App = () => <div>{{{expression}}}</div>;");
        for output in compile_both(&source, "jsx") {
            let (calls, _) = choices(&output, true);
            assert_eq!(calls.len(), 2, "{calls:?}: {output}");
            assert_eq!(output.matches("sourceRead()").count(), 1, "{output}");
            assert_eq!(output.matches("secondRead()").count(), 1, "{output}");
        }
    }
}

#[test]
fn helper_aliases_are_hygienic_and_function_left_operands_stay_values() {
    let source="'use client'; const __crepusChoose0=1, __crepusRead0=2; const authored=()=>99; export const App=()=> <div>{authored && <Child/>}</div>;";
    for output in compile_both(source, "jsx") {
        let (calls, _) = choices(&output, true);
        assert_eq!(calls.len(), 1, "{output}");
        assert_ne!(calls[0].callee, "__crepusChoose0", "{output}");
        assert!(
            !output.contains("authored()"),
            "authored function was invoked: {output}"
        );
        assert!(calls[0].arguments[0].contains("=>authored"), "{output}");
        assert!(
            output.find("use client").unwrap() < output.find("import ").unwrap(),
            "{output}"
        );
        for line in output.lines().filter(|line| line.starts_with("import ")) {
            assert!(
                line.contains("@tschk/moonshine-dom/"),
                "unexpected runtime dependency: {line}"
            );
        }
    }
}

#[test]
fn jsx_left_operands_are_owned_factories_for_every_logical_operator() {
    for operator in ["&&", "||", "??"] {
        let source =
            format!("export const App = () => <div>{{<Left/> {operator} <Right/>}}</div>;");
        for output in compile_both(&source, "jsx") {
            let (calls, memo) = choices(&output, true);
            assert_eq!(calls.len(), 1, "{calls:?}: {output}");
            let left = &calls[0].arguments[0];
            assert!(left.contains("(Left,"), "{output}");
            assert_eq!(
                calls[0].returned_callees[0].as_deref(),
                Some(memo.as_str()),
                "left factory must return its owned memo: {output}"
            );
            assert!(
                left.contains(&format!("{}(", imported_alias(&output, "untrack"))),
                "left JSX construction must be untracked: {output}"
            );
            if operator == "??" {
                assert_eq!(calls[0].arguments[3], "true", "{output}");
            }
        }
    }
}

#[test]
fn nested_left_choices_return_their_accessors_without_reading_them() {
    for (expression, expected) in [
        ("(first() ? <One/> : <Two/>) && <Three/>", 2),
        ("(first() && <One/>) || <Two/>", 2),
        ("(first() ? <One/> : <Two/>) ?? (second() && <Three/>)", 3),
    ] {
        let source = format!("export const App = () => <div>{{{expression}}}</div>;");
        for output in compile_both(&source, "tsx") {
            let (calls, _) = choices(&output, false);
            assert_eq!(calls.len(), expected, "{calls:?}: {output}");
            let outer = &calls[0];
            let nested = &calls[1];
            assert_eq!(
                outer.returned_callees[0].as_deref(),
                Some(nested.callee.as_str()),
                "outer left factory must return nested accessor directly: {output}"
            );
            assert_eq!(output.matches("first()").count(), 1, "{output}");
            if expected == 3 {
                assert_eq!(
                    outer.returned_callees[2].as_deref(),
                    Some(calls[2].callee.as_str()),
                    "right nested accessor must remain independent: {output}"
                );
                assert_eq!(output.matches("second()").count(), 1, "{output}");
            }
        }
    }
}
