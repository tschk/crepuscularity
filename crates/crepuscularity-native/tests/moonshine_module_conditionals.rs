#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, CallExpression, Expression, ImportDeclarationSpecifier, Statement};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

#[derive(Debug)]
struct BranchCall {
    callee: String,
    arguments: Vec<String>,
    returned_callees: Vec<Option<String>>,
}

fn branch_calls(source: &str, jsx: bool) -> Vec<BranchCall> {
    struct Calls<'s> {
        source: &'s str,
        values: Vec<BranchCall>,
    }
    impl<'a> Visit<'a> for Calls<'_> {
        fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
            if call.arguments.len() >= 3
                && call
                    .arguments
                    .iter()
                    .take(3)
                    .all(|argument| matches!(argument, Argument::ArrowFunctionExpression(_)))
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
                    self.values.push(BranchCall {
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
    let mut calls = Calls {
        source,
        values: vec![],
    };
    calls.visit_program(&parsed.program);
    calls.values
}

fn untrack_alias(source: &str) -> String {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    for statement in &parsed.program.body {
        if let Statement::ImportDeclaration(import) = statement {
            for specifier in import.specifiers.iter().flatten() {
                if let ImportDeclarationSpecifier::ImportSpecifier(specifier) = specifier {
                    if specifier.imported.name() == "untrack" {
                        return specifier.local.name.to_string();
                    }
                }
            }
        }
    }
    panic!("missing untrack import: {source}");
}

fn compile_both(source: &str, extension: &str) -> Vec<String> {
    [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr]
        .into_iter()
        .map(|target| {
            compile_moonshine_module(
                source,
                Path::new(&format!("Conditionals.{extension}")),
                target,
            )
            .unwrap()
            .source
        })
        .collect()
}

#[test]
fn conditional_jsx_uses_branch_factories_in_every_child_and_prop_position() {
    for source in [
        "export const App = () => <div>{condition() ? <input/> : <span>Off</span>}</div>;",
        "export const App = () => <>{condition() ? <Counter/> : <span>Off</span>}</>;",
        "export const App = () => <Frame>{condition() ? <Counter/> : <span>Off</span>}</Frame>;",
        "export const App = () => <Frame view={condition() ? <Counter/> : <span>Off</span>}/>;",
        "export const App = () => <div>{(condition() ? <Counter/> : <span>Off</span>) as any}</div>;",
    ] {
        for output in compile_both(source, "tsx") {
            let calls = branch_calls(&output, false);
            assert_eq!(
                calls.len(),
                1,
                "conditional must create an owned branch helper: {calls:?}: {output}"
            );
            assert!(calls[0].arguments[0].contains("condition()"), "{output}");
            assert!(
                calls[0].arguments[1].contains("__crepusH")
                    || calls[0].arguments[1].contains("__crepusComponentProps"),
                "{output}"
            );
            assert!(
                calls[0].arguments[2].contains("__crepusH")
                    || calls[0].arguments[2].contains("__crepusComponentProps"),
                "{output}"
            );
            let untrack = format!("{}(", untrack_alias(&output));
            assert!(
                calls[0].arguments[1].contains(&untrack),
                "JSX branch construction must be untracked: {output}"
            );
            assert!(
                calls[0].arguments[2].contains(&untrack),
                "JSX fallback construction must be untracked: {output}"
            );
            assert!(
                output.contains("getOwner") && output.contains("untrack"),
                "{output}"
            );
        }
    }
}

#[test]
fn scalar_alternates_stay_live_and_nested_conditionals_get_independent_helpers() {
    for output in compile_both(
        "export const App = () => <Frame>{condition() ? <Counter/> : value()}</Frame>;",
        "jsx",
    ) {
        let calls = branch_calls(&output, true);
        assert_eq!(calls.len(), 1, "{output}");
        assert!(calls[0].arguments[2].contains("value()"), "{output}");
        let untrack = format!("{}(", untrack_alias(&output));
        assert!(calls[0].arguments[1].contains(&untrack), "{output}");
        assert!(
            !calls[0].arguments[2].contains(&untrack),
            "scalar alternate must retain dependency tracking: {output}"
        );
        assert!(
            calls[0].arguments[2].contains("=>"),
            "scalar alternate must remain a factory: {output}"
        );
    }
    for output in compile_both(
        "export const App = () => <div>{condition() ? (nested() ? <One/> : <Two/>) : <Three/>}</div>;",
        "tsx",
    ) {
        let calls = branch_calls(&output, false);
        assert_eq!(calls.len(), 2, "{calls:?}: {output}");
        let outer = calls.iter().find(|call| call.arguments[0].contains("condition()")).unwrap();
        let nested = calls.iter().find(|call| call.arguments[0].contains("nested()")).unwrap();
        assert_eq!(outer.returned_callees[1].as_deref(), Some(nested.callee.as_str()),
            "outer factory must return the nested accessor without invoking it: {output}");
    }
}

#[test]
fn branch_helper_aliases_are_hygienic_and_plain_jsx_stays_javascript() {
    let source = "'use client'; const __crepusChoose0=1, __crepusConditional0=2, __crepusH0=3; export const App = () => <div>{condition() ? <Counter/> : <span>{__crepusChoose0+__crepusConditional0+__crepusH0}</span>}</div>;";
    for output in compile_both(source, "jsx") {
        let calls = branch_calls(&output, true);
        assert_eq!(calls.len(), 1, "{output}");
        assert!(
            !["__crepusChoose0", "__crepusConditional0", "__crepusH0"]
                .contains(&calls[0].callee.as_str()),
            "{output}"
        );
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
fn comment_punctuation_does_not_replace_real_ternary_separators() {
    for source in [
        "export const App = () => <div>{condition() /* ? : */ ? /* : ? */ <Child/> /* : ? */ : fallback()}</div>;",
        "export const App = () => <div>{condition() // ? :\u{2028}? <Child/> // : ?\u{2028}: fallback()}</div>;",
        "export const App = () => <div>{condition() // ? :\u{2029}? <Child/> // : ?\u{2029}: fallback()}</div>;",
    ] {
        for output in compile_both(source, "jsx") {
            let calls = branch_calls(&output, true);
            assert_eq!(calls.len(), 1, "{output}");
            assert!(calls[0].arguments[0].contains("condition()"), "{output}");
            assert!(calls[0].arguments[1].contains("Child"), "{output}");
            assert!(calls[0].arguments[2].contains("fallback()"), "{output}");
        }
    }
}
