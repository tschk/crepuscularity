#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_astro, MoonshineModuleTarget};
use std::path::Path;

const FRONT: &str = "const first={a:1};const last={b:2};const Card=(props:any)=>props.label;const readA=()=>1;const readSpread=()=>({});const readB=()=>2;const readChild=()=>3;";

fn compile_with(front: &str, template: &str) -> Result<String, String> {
    compile_moonshine_astro(
        &format!("---\n{front}\n---\n{template}"),
        Path::new("Page.astro"),
        MoonshineModuleTarget::Ssr,
    )
    .map(|module| module.source)
}

fn compile(template: &str) -> Result<String, String> {
    compile_with(FRONT, template)
}

fn ok(template: &str) -> String {
    compile(template).unwrap_or_else(|error| panic!("{template}: {error}"))
}

fn compact(output: &str) -> String {
    output.split_whitespace().collect()
}

fn element_call(output: &str, tag: &str) -> String {
    let flat = compact(output);
    let needle = format!("__crepusAstroElement(\"{tag}\",");
    flat.split_once(&needle)
        .unwrap_or_else(|| panic!("missing {needle}: {output}"))
        .1
        .to_owned()
}

fn in_order(haystack: &str, needles: &[&str]) {
    let mut at = 0;
    for needle in needles {
        let found = haystack[at..]
            .find(needle)
            .unwrap_or_else(|| panic!("`{needle}` missing or out of order in {haystack}"));
        at += found + needle.len();
    }
}

#[test]
fn intrinsic_spreads_keep_source_order_and_static_dynamic_provenance() {
    let output = ok(
        "<div id=\"a\" {...first} data-x={readA()} disabled=\"false\" {...last} checked={false}/>",
    );
    let call = element_call(&output, "div");
    in_order(
        &call,
        &[
            "__crepusAstroAttribute(\"div\",\"id\",\"a\",false)",
            "...__crepusAstroSpread(",
            "first",
            "__crepusAstroAttribute(\"div\",\"data-x\",",
            "readA()",
            ",true)",
            "__crepusAstroAttribute(\"div\",\"disabled\",\"false\",false)",
            "...__crepusAstroSpread(",
            "last",
            "__crepusAstroAttribute(\"div\",\"checked\",",
            "false",
            ",true)",
        ],
    );
    assert!(
        output.contains("sourceElement as __crepusAstroElement"),
        "{output}"
    );
    assert!(
        output.contains("sourceSpread as __crepusAstroSpread"),
        "{output}"
    );
    assert!(!output.contains("__crepusAstroH("), "{output}");
}

#[test]
fn spread_only_and_trailing_spread_elements_lower_through_entries() {
    for template in [
        "<div {...first}/>",
        "<div {...null} {...undefined} {...\"text\"}/>",
        "<p id=\"x\" {...first}>text</p>",
    ] {
        let output = ok(template);
        let call = element_call(
            &output,
            if template.starts_with("<p") {
                "p"
            } else {
                "div"
            },
        );
        assert!(call.starts_with('['), "{template}: {output}");
        assert!(
            call.contains("...__crepusAstroSpread("),
            "{template}: {output}"
        );
    }
}

#[test]
fn authored_class_and_class_list_combine_at_class_list_independent_of_spread_classes() {
    let output = ok("<div {...first} class=\"a\" class:list={['b']} {...last}/>");
    let call = element_call(&output, "div");
    assert!(!call.contains("class:list"), "{output}");
    assert_eq!(
        call.matches("__crepusAstroAttribute(\"div\",\"class\",")
            .count(),
        1,
        "{output}"
    );
    in_order(
        &call,
        &[
            "...__crepusAstroSpread(",
            "first",
            "__crepusAstroAttribute(\"div\",\"class\",__crepusAstroClass([\"a\",",
            "||undefined,true)",
            "...__crepusAstroSpread(",
            "last",
        ],
    );
    assert!(
        output.contains("classList as __crepusAstroClass"),
        "{output}"
    );

    let only_list = ok("<div {...first} class:list={['b']}/>");
    let call = element_call(&only_list, "div");
    in_order(
        &call,
        &[
            "first",
            "__crepusAstroAttribute(\"div\",\"class\",__crepusAstroClass(",
            "||undefined,true)",
        ],
    );
}

#[test]
fn component_spreads_keep_class_directives_for_runtime_normalization() {
    let output = ok("<Card {...first} class=\"a\" class:list={['b']}/>");
    assert!(output.contains("\"class:list\""), "{output}");
    assert!(!output.contains("sourceElement"), "{output}");
    assert!(!output.contains("sourceSpread"), "{output}");
}

#[test]
fn astro_props_rebind_inside_intrinsic_spreads_and_entries() {
    for template in [
        "<div {...Astro.props} title={Astro.props.title}/>",
        "{[1].map(item => <p {...Astro.props}/>)}",
        "{true && <p {...Astro.props}>{Astro.props.label}</p>}",
    ] {
        let output = ok(template);
        assert!(!output.contains("Astro.props"), "{template}: {output}");
        assert!(
            compact(&output).contains("...__crepusAstroSpread("),
            "{template}: {output}"
        );
        assert!(
            output.contains("__crepusAstroProps"),
            "{template}: {output}"
        );
    }
}

#[test]
fn nested_intrinsic_spreads_and_component_children_lower_independently() {
    let output = ok(
        "<section {...first}>{[first].map(value => <span {...value} class:list={['x']}>{true && <b {...last}/>}</span>)}<Card {...first}/></section>",
    );
    let flat = compact(&output);
    in_order(
        &flat,
        &[
            "__crepusAstroElement(\"section\",",
            "__crepusAstroElement(\"span\",",
            "__crepusAstroElement(\"b\",",
            "__crepusAstroComponent(Card,{...(",
        ],
    );
}

#[test]
fn spread_and_attribute_expressions_evaluate_in_source_order_before_children() {
    let output = ok("<div a={readA()} {...readSpread()} b={readB()}>{readChild()}</div>");
    let call = element_call(&output, "div");
    in_order(
        &call,
        &[
            "__crepusAstroAttribute(\"div\",\"a\",",
            "readA()",
            "...__crepusAstroSpread(",
            "readSpread()",
            "__crepusAstroAttribute(\"div\",\"b\",",
            "readB()",
            ",true)",
            "readChild()",
        ],
    );
}

#[test]
fn generated_helper_aliases_avoid_user_bindings() {
    let output = compile_with(
        "const first={a:1};const __crepusAstroElement=1;const __crepusAstroSpread=2;const __crepusAstroAttribute=5;const sourceElement=3;const sourceSpread=4;",
        "<p title={1} {...first}>{__crepusAstroElement}{__crepusAstroSpread}{sourceElement}{sourceSpread}</p>",
    )
    .unwrap();
    assert!(
        output.contains("sourceElement as __crepusAstroElement_"),
        "{output}"
    );
    assert!(
        output.contains("sourceAttribute as __crepusAstroAttribute_"),
        "{output}"
    );
    assert!(
        output.contains("sourceSpread as __crepusAstroSpread_"),
        "{output}"
    );
    assert!(
        compact(&output).contains("__crepusAstroElement_(\"p\","),
        "{output}"
    );
    assert!(
        compact(&output).contains("...__crepusAstroSpread_("),
        "{output}"
    );
}

#[test]
fn spread_helpers_are_imported_only_when_used() {
    let plain = ok("<div id=\"a\" title={readA()}/>");
    assert!(plain.contains("__crepusAstroH("), "{plain}");
    assert!(!plain.contains("sourceElement"), "{plain}");
    assert!(!plain.contains("sourceSpread"), "{plain}");

    let component = ok("<Card {...first}/>");
    assert!(!component.contains("sourceElement"), "{component}");
    assert!(!component.contains("sourceSpread"), "{component}");

    let spread = ok("<div {...first}/>");
    assert!(!spread.contains("h as __crepusAstroH"), "{spread}");
}

#[test]
fn outlets_and_invalid_spread_forms_remain_rejected() {
    for template in [
        "<slot {...first}/>",
        "<slot name=\"a\" {...first}/>",
        "<Card><slot slot=\"a\" {...first}/></Card>",
        "<div {...}/>",
        "<div ...={first}/>",
    ] {
        assert!(compile(template).is_err(), "{template}");
    }
}

#[test]
fn authored_attribute_validation_still_applies_next_to_spreads() {
    for template in [
        "<div {...first} id=\"a\" id=\"b\"/>",
        "<div {...first} id=\"a\" ID=\"b\"/>",
        "<div {...first} className=\"a\" class=\"b\"/>",
        "<div {...first} class=\"a\" class=\"b\" class:list={['x']}/>",
        "<div {...first} client:load/>",
        "<div {...first} data:x=\"1\"/>",
        "<div {...first} __proto__=\"x\"/>",
    ] {
        assert!(compile(template).is_err(), "{template}");
    }
}
