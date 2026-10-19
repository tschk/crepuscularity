#![cfg(feature = "web-modules")]
use crepuscularity_native::{compile_moonshine_astro, MoonshineModuleTarget};
use std::path::Path;
fn compile(template: &str) -> Result<String, String> {
    compile_moonshine_astro(&format!("---\nconst first={{label:'first'}};const last={{label:'last'}};const Card=(props:any)=>props.label;\n---\n{template}"),Path::new("Page.astro"),MoonshineModuleTarget::Ssr).map(|m|m.source)
}
#[test]
fn component_spreads_keep_source_order_and_prop_rebinding() {
    for template in [
        "<Card {...first} label=\"explicit\"/>",
        "<Card label=\"explicit\" {...last}/>",
        "<Card {...first} {...last}/>",
        "<Card {...Astro.props}/>",
        r#"<Card {...null} {...undefined} {..."text"}/>"#,
        "{true && <Card {...first}/>} ",
        "{[first].map(value=><Card {...value}/>)}",
    ] {
        let output = compile(template).unwrap_or_else(|e| panic!("{template}: {e}"));
        assert!(output.contains("..."), "{output}");
        assert!(!output.contains("Astro.props"), "{output}");
        assert!(!output.contains("astro/runtime"), "{output}");
    }
    let output = compile("<Card {...first} label=\"explicit\" {...last}/>").unwrap();
    let call = output.rsplit("__crepusAstroComponent(").next().unwrap();
    let first = call.find("first").unwrap();
    let label = call.find("explicit").unwrap();
    let last = call.find("last").unwrap();
    assert!(first < label && label < last, "{output}");
}
#[test]
fn class_directives_are_left_on_the_final_spread_props_for_runtime_normalization() {
    let out = compile("<Card {...first} class=\"a\" class:list={['b']}/>").unwrap();
    assert!(out.contains("\"class:list\""), "{out}");
    assert!(out.contains("\"class\""), "{out}");
}
#[test]
fn spreads_do_not_admit_outlets_or_empty_expressions() {
    for template in [
        "<slot {...first}/>",
        "<Card {...}/>",
        "<Fragment {...first}/>",
        "<Card ...={first}/>",
        "<Card {...first} label=\"a\" label=\"b\"/>",
    ] {
        assert!(compile(template).is_err(), "{template}");
    }
}
