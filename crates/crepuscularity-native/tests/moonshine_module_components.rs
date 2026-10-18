#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_module, MoonshineModuleTarget};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, ObjectProperty, ObjectPropertyKind};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

fn descriptors(source: &str) -> Vec<(String, String)> {
    struct Descriptors<'s> {
        source: &'s str,
        values: Vec<(String, String)>,
    }
    impl<'a> Visit<'a> for Descriptors<'_> {
        fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
            if let Expression::ObjectExpression(descriptor) = &property.value {
                for entry in &descriptor.properties {
                    if let ObjectPropertyKind::ObjectProperty(getter) = entry {
                        if getter.key.static_name().is_some_and(|name| name == "get") {
                            let name = property.key.static_name().expect("named component prop");
                            assert!(
                                matches!(
                                    &getter.value,
                                    Expression::ArrowFunctionExpression(_)
                                        | Expression::CallExpression(_)
                                ),
                                "prop must use a lexical arrow or returned cached getter"
                            );
                            let span = getter.value.span();
                            self.values.push((
                                name.to_string(),
                                compact(&self.source[span.start as usize..span.end as usize]),
                            ));
                        }
                    }
                }
            }
            walk::walk_object_property(self, property);
        }
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::tsx()).parse();
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut descriptors = Descriptors {
        source,
        values: vec![],
    };
    descriptors.visit_program(&parsed.program);
    descriptors.values
}

fn getter<'a>(values: &'a [(String, String)], name: &str) -> &'a str {
    &values
        .iter()
        .find(|(key, _)| key == name)
        .unwrap_or_else(|| panic!("missing descriptor {name}: {values:?}"))
        .1
}

fn compile_both(source: &str) -> Vec<String> {
    [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr]
        .into_iter()
        .map(|target| {
            compile_moonshine_module(source, Path::new("Components.tsx"), target)
                .unwrap()
                .source
        })
        .collect()
}

fn compact(source: &str) -> String {
    source
        .chars()
        .filter(|value| !value.is_whitespace())
        .collect()
}

#[test]
fn local_imported_and_member_components_remain_calls_with_lazy_props() {
    let source = r#"
import { Imported } from './Imported';
const UI = { Member: Imported };
function Local(props: any) { return <span>{props.value}</span>; }
export function App() {
    return <Local value={count()} onClick={handler}><Imported/><UI.Member/></Local>;
}
"#;
    for output in compile_both(source) {
        assert!(output.contains("from \"./Imported\""), "{output}");
        assert!(
            !output.contains("<Local") && !output.contains("<Imported"),
            "{output}"
        );
        let normalized = compact(&output);
        assert!(normalized.contains("(Local,"), "{output}");
        assert!(normalized.contains("(Imported,"), "{output}");
        assert!(normalized.contains("(UI.Member,"), "{output}");
        assert!(!normalized.contains("(\"Local\""), "{output}");
        let descriptors = descriptors(&output);
        assert!(
            getter(&descriptors, "value").starts_with("()=>"),
            "{output}"
        );
        assert!(
            getter(&descriptors, "onClick").starts_with("()=>"),
            "{output}"
        );
        assert!(
            getter(&descriptors, "value").contains("=>count()"),
            "{output}"
        );
        assert!(
            getter(&descriptors, "onClick").contains("=>handler"),
            "{output}"
        );
        assert!(!normalized.contains("value:()=>count()"), "{output}");
        assert!(!normalized.contains("onClick:()=>handler"), "{output}");
    }
}

#[test]
fn component_children_support_single_multiple_and_explicit_props() {
    for source in [
        "export const App = () => <Component>{count()}</Component>;",
        "export const App = () => <Component><span>Only</span></Component>;",
        "export const App = () => <Component><One/><Two/></Component>;",
        "export const App = () => <Component children={count()}/>;",
    ] {
        for output in compile_both(source) {
            let descriptors = descriptors(&output);
            let children = getter(&descriptors, "children");
            if source.contains("children={") {
                assert!(children.contains("=>count()"), "{output}");
            } else {
                assert!(children.contains("(["), "{output}");
                let single = !source.contains("<One/>");
                assert!(
                    children.ends_with(if single { ",true)" } else { ",false)" }),
                    "{output}"
                );
                if source.contains("count()") {
                    assert!(children.contains("=>count()"), "{output}");
                }
            }
            assert!(!output.contains("<Component"), "{output}");
        }
    }
}

#[test]
fn nested_components_inside_conditions_and_maps_are_fully_lowered() {
    let source = "export const App = () => <Outer>{visible() ? <Inner value={count()}/> : rows().map(row => <UI.Row value={row.name}><Leaf/></UI.Row>)}</Outer>;";
    for output in compile_both(source) {
        let normalized = compact(&output);
        for component in ["(Outer,", "(Inner,", "(UI.Row,", "(Leaf,"] {
            assert!(normalized.contains(component), "{component}: {output}");
        }
        assert!(
            !output.contains("<Inner") && !output.contains("<UI.Row"),
            "{output}"
        );
        assert!(normalized.contains("rows().map("), "{output}");
    }
}

#[test]
fn unsupported_component_boundaries_are_explicit_errors() {
    for source in [
        "export const App = () => <Component children={value}>Body</Component>;",
        "export const App = () => <Component __proto__={value}/>;",
        "export const App = () => <Component {...props}/>;",
        "export const App = () => <Component>{...children}</Component>;",
        "export const App = () => <Component<string> value='text'/>;",
    ] {
        for target in [MoonshineModuleTarget::Dom, MoonshineModuleTarget::Ssr] {
            assert!(
                compile_moonshine_module(source, Path::new("Components.tsx"), target).is_err(),
                "accepted {source}"
            );
        }
    }
}

#[test]
fn descriptor_arrows_preserve_this_arguments_and_super_from_the_rendering_method() {
    let source = r#"
class Base { field = 'base'; method() { return 'method'; } }
export class Owner extends Base {
    field = 'owner';
    render(value: string) {
        return <Component own={this.field} argument={arguments[0]} inherited={super.field} called={super.method()}>{this.field}</Component>;
    }
}
"#;
    for output in compile_both(source) {
        let descriptors = descriptors(&output);
        for (name, expression) in [
            ("own", "=>this.field"),
            ("argument", "=>arguments[0]"),
            ("inherited", "=>super.field"),
            ("called", "=>super.method()"),
            ("children", "=>this.field"),
        ] {
            assert!(
                getter(&descriptors, name).contains(expression),
                "{name}: {output}"
            );
        }
    }
}

#[test]
fn global_this_reads_remain_supported_without_shadowing_the_descriptor_intrinsic() {
    for output in
        compile_both("export const App = () => <Component value={globalThis.Number(1)}/>;")
    {
        let descriptors = descriptors(&output);
        assert!(
            getter(&descriptors, "value").contains("=>globalThis.Number(1)"),
            "{output}"
        );
    }
}

#[test]
fn direct_jsx_children_use_cached_factories_but_expression_children_remain_live() {
    let source = "export const App = () => <Frame><StaticChild/>{count()}</Frame>;";
    for output in compile_both(source) {
        let descriptors = descriptors(&output);
        let children = getter(&descriptors, "children");
        assert!(children.contains("(["), "{output}");
        assert!(children.ends_with(",false)"), "{output}");
        assert!(
            children.contains("(()=>"),
            "direct JSX must use a factory wrapper: {output}"
        );
        assert!(children.contains("(StaticChild,"), "{output}");
        assert!(
            children.contains(",()=>count()"),
            "expression factory must remain live: {output}"
        );
        assert!(
            output.contains("getOwner")
                && output.contains("runWithOwner")
                && output.contains("untrack"),
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
fn jsx_valued_props_use_cached_getters_while_ordinary_props_stay_live_arrows() {
    for output in
        compile_both("export const App = () => <Frame view={<Static/>} value={count()}/>;")
    {
        let descriptors = descriptors(&output);
        let view = getter(&descriptors, "view");
        assert!(
            !view.starts_with("()=>"),
            "JSX prop must use returned cached getter: {output}"
        );
        assert!(
            view.contains("(()=>") && view.contains("(Static,"),
            "{output}"
        );
        let value = getter(&descriptors, "value");
        assert!(
            value.starts_with("()=>") && value.contains("count()"),
            "{output}"
        );
    }
}

#[test]
fn shorthand_producing_component_props_keep_lexical_identifier_getters() {
    let source = r#"
export function App() {
    const value = 7;
    const item = { name: 'Local' };
    const onClick = () => value;
    return <Row value={value} item={item} onClick={onClick} position={index()} kind='keyed'/>;
}
"#;
    for output in compile_both(source) {
        let descriptors = descriptors(&output);
        for name in ["value", "item", "onClick"] {
            assert_eq!(
                getter(&descriptors, name),
                format!("()=>{name}"),
                "{output}"
            );
        }
        assert!(
            getter(&descriptors, "position").contains("=>index()"),
            "{output}"
        );
        assert!(getter(&descriptors, "kind").contains("keyed"), "{output}");
        assert!(output.contains("const value = 7"), "{output}");
        assert!(
            !output.contains("scope.item") && !output.contains("scope.value"),
            "{output}"
        );
    }
}

#[test]
fn authored_global_this_bindings_use_owned_platform_operations() {
    for source in [
        "const globalThis = {}; export const App = () => <Component/>;",
        "export const App = (globalThis: unknown) => <Component/>;",
        "const globalThis = {value:'authored'}; export const App = () => <Component value={globalThis.value}/>;",
        "export const App = (globalThis: {value:string}) => <Component value={globalThis.value}/>;",
        "const __crepusSourceRealm0=1;const __crepusSourceTypes0=2;const globalThis={value:'authored'};export const App=()=> <Component value={globalThis.value}/>;",
    ] {
        for output in compile_both(source) {
            assert!(output.contains("@tschk/moonshine-dom/platform"), "{output}");
            assert!(output.contains("sourceRealm as"), "{output}");
            assert!(!output.contains("globalThis.Object.create"), "{output}");
            if source.contains("globalThis.value") {
                assert!(output.contains("globalThis.value"), "{output}");
            }
            if source.contains("__crepusSourceRealm0=1") {
                assert!(output.contains("sourceRealm as __crepusSourceRealm1"), "{output}");
            }
        }
    }
}

#[test]
fn authored_noinfer_does_not_capture_generated_descriptor_types() {
    for binding in ["", "const globalThis = { authored: true }; "] {
        let source = format!(
            "type NoInfer<T> = never; {binding}function Child(props: {{value: number}}) {{ return <p>{{props.value}}</p>; }} export function App() {{ return <Child value={{1}}/>; }}"
        );
        for output in compile_both(&source) {
            let normalized = compact(&output);
            assert!(normalized.contains(".NoInfer<D>"), "{output}");
            assert!(!normalized.contains(":NoInfer<D>"), "{output}");
            assert!(normalized.contains("typeNoInfer<T,>=never"), "{output}");
        }
    }
}
