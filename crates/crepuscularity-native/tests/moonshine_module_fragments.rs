#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, CallExpression, Statement};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

#[derive(Debug)]
struct HostCall {
    tag: String,
    arguments: Vec<String>,
    is_fragment: bool,
    direct_fragment: bool,
}

fn calls(source: &str, jsx: bool) -> Vec<HostCall> {
    struct Calls<'s> {
        source: &'s str,
        values: Vec<HostCall>,
        fragment_name: String,
    }
    impl<'a> Visit<'a> for Calls<'_> {
        fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
            let span = call.callee.span();
            let callee = &self.source[span.start as usize..span.end as usize];
            let direct_fragment = callee == self.fragment_name;
            if direct_fragment
                || callee.starts_with("__crepusH")
                || (callee.starts_with("__crepusComponentProps")
                    && !callee.ends_with("Descriptors"))
            {
                let tag = if direct_fragment {
                    self.fragment_name.clone()
                } else {
                    call.arguments
                        .first()
                        .map(|argument| match argument {
                            Argument::Identifier(identifier) => identifier.name.to_string(),
                            _ => String::new(),
                        })
                        .unwrap_or_default()
                };
                let arguments = call
                    .arguments
                    .iter()
                    .map(|argument| {
                        let span = argument.span();
                        self.source[span.start as usize..span.end as usize].to_string()
                    })
                    .collect();
                let is_fragment = tag == self.fragment_name;
                self.values.push(HostCall {
                    tag,
                    arguments,
                    is_fragment,
                    direct_fragment,
                });
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
    let helpers: Vec<_> = parsed
        .program
        .body
        .iter()
        .filter_map(|statement| {
            let Statement::FunctionDeclaration(function) = statement else {
                return None;
            };
            let body = function.body.as_ref()?;
            let span = body.span;
            let body = source[span.start as usize..span.end as usize]
                .chars()
                .filter(|value| !value.is_whitespace())
                .collect::<String>();
            (body.contains("returnprops.children??[];")
                || body.contains("returnprops?.children??children;"))
            .then(|| function.id.as_ref().unwrap().name.to_string())
        })
        .collect();
    assert_eq!(
        helpers.len(),
        1,
        "missing unique fragment passthrough helper: {source}"
    );
    let mut visitor = Calls {
        source,
        values: vec![],
        fragment_name: helpers[0].clone(),
    };
    visitor.visit_program(&parsed.program);
    visitor.values
}

fn compile_both(source: &str, extension: &str) -> Vec<String> {
    [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr]
        .into_iter()
        .map(|target| {
            let output = compile_moonshine_module(
                source,
                Path::new(&format!("Fragments.{extension}")),
                target,
            )
            .unwrap();
            assert!(
                output.imports.is_empty(),
                "unexpected authored imports: {:?}",
                output.imports
            );
            for line in output
                .source
                .lines()
                .filter(|line| line.starts_with("import "))
            {
                assert!(
                    line.contains("@tschk/moonshine-dom/"),
                    "unexpected dependency: {line}"
                );
            }
            output.source
        })
        .collect()
}

#[test]
fn empty_single_multiple_nested_and_root_fragments_use_rest_children() {
    for (source, expected_children) in [
        ("export default () => <></>;", 0),
        ("export default () => <>Only</>;", 1),
        (
            "export default () => <><span>One</span><span>Two</span></>;",
            2,
        ),
        ("export default () => <><><span>Nested</span></></>;", 1),
    ] {
        for output in compile_both(source, "tsx") {
            let calls = calls(&output, false);
            let fragments: Vec<_> = calls.iter().filter(|call| call.is_fragment).collect();
            assert!(!fragments.is_empty(), "{output}");
            assert_eq!(
                fragments[0].arguments.len(),
                if fragments[0].direct_fragment { 1 } else { 2 } + expected_children,
                "{calls:?}: {output}"
            );
            for fragment in fragments {
                assert!(
                    !fragment.arguments[usize::from(!fragment.direct_fragment)]
                        .contains("Object.create"),
                    "fragment became custom component descriptors: {output}"
                );
            }
            assert!(!output.contains("<>"), "{output}");
        }
    }
}

#[test]
fn conditional_map_and_component_child_fragments_keep_both_lowering_contracts() {
    let source = "export const App = () => <Component>{visible() ? <>{count()}</> : rows().map(row => <><span>{row.name}</span></>)}</Component>;";
    for output in compile_both(source, "tsx") {
        let calls = calls(&output, false);
        let component = calls.iter().find(|call| call.tag == "Component").unwrap();
        assert_eq!(component.arguments.len(), 2, "{output}");
        assert!(
            component.arguments[1].contains("Object.create")
                || component.arguments[1].contains("__crepusComponentProps"),
            "{output}"
        );
        assert!(component.arguments[1].contains("children"), "{output}");
        let fragments: Vec<_> = calls.iter().filter(|call| call.is_fragment).collect();
        assert_eq!(fragments.len(), 2, "{calls:?}: {output}");
        assert!(
            fragments.iter().any(|fragment| fragment
                .arguments
                .iter()
                .skip(if fragment.direct_fragment { 1 } else { 2 })
                .any(|child| child.contains("=>") && child.contains("count()"))),
            "{output}"
        );
        assert!(output.contains("rows().map"), "{output}");
        assert!(!output.contains("<span"), "{output}");
    }
}

#[test]
fn helper_names_avoid_source_bindings_and_preserve_directives_and_hashbangs() {
    let source = "#!/usr/bin/env bun\n'use client';\nconst __crepusH0=1, __crepusFragment0=2, __crepusF0=3; export const App = () => <>{__crepusH0+__crepusFragment0+__crepusF0}</>;";
    for output in compile_both(source, "jsx") {
        assert!(output.starts_with("#!/usr/bin/env bun\n"), "{output}");
        let directive = output.find("use client").unwrap();
        assert!(directive < output.find("function ").unwrap(), "{output}");
        if let Some(import) = output.find("import ") {
            assert!(directive < import, "{output}");
        }
        let calls = calls(&output, true);
        let fragment = calls.iter().find(|call| call.is_fragment).unwrap();
        assert!(
            !["__crepusH0", "__crepusFragment0", "__crepusF0"].contains(&fragment.tag.as_str()),
            "{output}"
        );
        assert!(
            !output.contains("React") && !output.contains("react/jsx"),
            "{output}"
        );
    }
}

#[test]
fn a_fragment_used_as_the_single_component_child_stays_a_scalar_getter() {
    for output in compile_both(
        "export const App = () => <Component><><span>One</span><span>Two</span></></Component>;",
        "jsx",
    ) {
        let calls = calls(&output, true);
        let component = calls.iter().find(|call| call.tag == "Component").unwrap();
        assert_eq!(component.arguments.len(), 2, "{output}");
        assert!(component.arguments[1].contains("Object.create"), "{output}");
        assert!(component.arguments[1].contains("get:"), "{output}");
        assert!(
            component.arguments[1].contains("true)"),
            "scalar child flag missing: {output}"
        );
        assert!(
            output.contains("getOwner")
                && output.contains("runWithOwner")
                && output.contains("untrack"),
            "{output}"
        );
        let fragment = calls.iter().find(|call| call.is_fragment).unwrap();
        assert_eq!(
            fragment.arguments.len(),
            if fragment.direct_fragment { 3 } else { 4 },
            "{output}"
        );
        assert!(
            !fragment.arguments[usize::from(!fragment.direct_fragment)].contains("Object.create"),
            "{output}"
        );
    }
}

#[test]
fn only_the_generated_ssr_fragment_bypasses_the_component_host() {
    let source = "function Fragment(){return <span>Real</span>} function __crepusFragment0(){return <span>Lookalike</span>} export default()=> <><Fragment/><__crepusFragment0/></>;";
    for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
        let output = compile_moonshine_module(source, Path::new("Exact.tsx"), target)
            .unwrap()
            .source;
        let calls = calls(&output, false);
        for name in ["Fragment", "__crepusFragment0"] {
            let component = calls.iter().find(|call| call.tag == name).unwrap();
            assert!(!component.is_fragment, "{output}");
            assert!(!component.direct_fragment, "{output}");
            assert_eq!(component.arguments.len(), 2, "{output}");
        }
        let fragment = calls.iter().find(|call| call.is_fragment).unwrap();
        assert_eq!(
            fragment.direct_fragment,
            matches!(target, MoonshineModuleTarget::Ssr),
            "{output}"
        );
        if matches!(target, MoonshineModuleTarget::Ssr) {
            let allocator = Allocator::default();
            let parsed = Parser::new(&allocator, &output, SourceType::tsx()).parse();
            let helper = parsed
                .program
                .body
                .iter()
                .find_map(|statement| {
                    let Statement::FunctionDeclaration(function) = statement else {
                        return None;
                    };
                    (function.id.as_ref()?.name.as_str() == fragment.tag.as_str())
                        .then_some(function)
                })
                .unwrap();
            let annotation = helper
                .return_type
                .as_ref()
                .expect("typed SSR fragment result");
            let span = annotation.span();
            let annotation = &output[span.start as usize..span.end as usize];
            assert!(annotation.contains("SSRChild"), "{output}");
            assert!(!annotation.contains("any"), "{output}");
        }
    }
}

#[test]
fn fragment_only_ssr_modules_do_not_leave_an_unused_host_import() {
    for source in [
        "export default () => <></>;",
        "export default () => <>hello<></></>;",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            let output = compile_moonshine_module(source, Path::new("Empty.tsx"), target)
                .unwrap()
                .source;
            assert_eq!(
                output.contains("import { h as"),
                matches!(target, MoonshineModuleTarget::Dom),
                "{output}"
            );
        }
    }
}
