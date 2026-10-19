#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_astro, MoonshineModule, MoonshineModuleTarget};
use std::path::Path;

fn compile(source: &str) -> Result<MoonshineModule, String> {
    compile_moonshine_astro(source, Path::new("Page.astro"), MoonshineModuleTarget::Ssr)
}

fn ok(source: &str) -> String {
    compile(source)
        .unwrap_or_else(|error| panic!("{error}\n{source}"))
        .source
}

fn err(source: &str) -> String {
    compile(source).expect_err(source)
}

fn compact(output: &str) -> String {
    output.split_whitespace().collect()
}

#[test]
fn constants_and_props_destructuring_run_inside_the_factory() {
    let output = ok("---\nconst greeting = \"Hello\";\nconst { name = \"World\" } = Astro.props;\n---\n<h1>{greeting} {name}</h1>\n");
    assert!(!output.contains("Astro.props"), "{output}");
    assert!(output.contains("__crepusAstroProps"), "{output}");
    let factory = output.find("export default function").expect("factory");
    assert!(
        output.find("const greeting").expect("constant") > factory,
        "{output}"
    );
    assert!(output.find("Record<string, any>").is_some(), "{output}");
}

#[test]
fn declared_props_type_types_the_factory_argument() {
    let output = ok("---\ninterface Props { title: string }\nconst { title } = Astro.props;\n---\n<h1>{title}</h1>\n");
    assert!(output.contains("interface Props"), "{output}");
    assert!(!output.contains("Record<string, any>"), "{output}");
    assert!(
        output.find("interface Props").unwrap() < output.find("export default function").unwrap()
    );
    let alias = ok("---\nexport type Props = { count: number };\nconst { count } = Astro.props;\n---\n<p>{count}</p>\n");
    assert!(alias.contains("type Props"), "{alias}");
    assert!(!alias.contains("Record<string, any>"), "{alias}");
}

#[test]
fn imports_are_hoisted_and_components_are_ordinary_calls() {
    let output = ok("---\nimport Card from \"./Card.astro\";\nconst title = \"x\";\n---\n<Card title={title} />\n");
    assert!(
        output.find("Card.astro").unwrap() < output.find("export default function").unwrap(),
        "{output}"
    );
    assert!(output.contains("Card"), "{output}");
}

#[test]
fn per_render_values_do_not_escape_the_factory() {
    let output = ok("---\nlet counter = Math.random();\n---\n<p>{counter}</p>\n");
    assert!(
        output.find("export default function").unwrap() < output.find("let counter").unwrap(),
        "{output}"
    );
}

#[test]
fn scriptless_templates_compile_to_a_child_factory() {
    let output = ok("<section><p>plain</p></section>\n");
    assert!(output.contains("export default function"), "{output}");
    assert!(output.contains("plain"), "{output}");
    assert!(output.contains("Record<string, any>"), "{output}");
}

#[test]
fn ordinary_markup_text_entities_conditionals_and_maps() {
    let output = ok("---\nconst items = [\"a\", \"b\"];\nconst show = true;\n---\n<img src=\"/a.png\" alt=\"x\"><br>\n<p title=\"a &amp; b\">Tom &amp; Jerry</p>\n{show && <b>yes</b>}\n<ul>{items.map(item => <li>{item}</li>)}</ul>\n");
    assert!(output.contains("Tom & Jerry"), "{output}");
    assert!(output.contains("a & b"), "{output}");
    assert!(!output.contains("&amp;"), "{output}");
    assert!(output.contains(".map("), "{output}");
    assert!(output.contains("show"), "{output}");
    assert!(
        output.contains("\"img\"") && output.contains("\"br\""),
        "{output}"
    );
}

#[test]
fn async_functions_may_await_but_top_level_may_not() {
    let output = ok("---\nasync function load() { return await fetch(\"/x\"); }\nconst loader = load;\n---\n<p>{typeof loader}</p>\n");
    assert!(output.contains("await"), "{output}");
    assert!(err("---\nconst data = await fetch(\"/x\");\n---\n<p/>\n").contains("await"));
    assert!(err("---\nfor await (const x of source) {}\n---\n<p/>\n").contains("await"));
}

#[test]
fn unsupported_globals_are_rejected_instead_of_dropped() {
    assert!(err("---\nconst url = Astro.url;\n---\n<p/>\n").contains("Astro.url"));
    assert!(err("---\nconst all = Astro;\n---\n<p/>\n").contains("Astro"));
    assert!(err("<a href={Astro.request.url}>x</a>\n").contains("Astro"));
}

#[test]
fn hydration_events_slots_and_exports_are_rejected() {
    assert!(
        err("---\nimport Counter from \"./Counter.astro\";\n---\n<Counter client:load />\n")
            .contains("client:load")
    );
    assert!(err("<button onclick=\"go()\">x</button>\n").contains("event"));
    assert!(
        err("---\nconst go = () => {};\n---\n<button onClick={go}>x</button>\n").contains("event")
    );
    assert!(err("<slot name={dynamic} />\n").contains("dynamic"));
    assert!(err("<slot slot=\"x\" />\n").contains("immediate child"));
    ok("<Fragment><b>wrapped</b></Fragment>\n");
    assert!(err("---\nexport const prerender = true;\n---\n<p/>\n").contains("export"));
    assert!(err("---\nexport { x };\nconst x = 1;\n---\n<p/>\n").contains("export"));
    assert!(
        err("---\nimport { createSignal } from \"solid-js\";\n---\n<p/>\n")
            .contains("host runtime")
    );
    assert!(!err("<p><style>p{}</style></p>\n").is_empty());
    assert!(!err("---\nconst r = require(\"fs\");\n---\n<p/>\n").is_empty());
}

#[test]
fn dom_target_is_rejected() {
    let error = compile_moonshine_astro(
        "<p>x</p>\n",
        Path::new("Page.astro"),
        MoonshineModuleTarget::Dom,
    )
    .expect_err("dom");
    assert!(error.contains("server"), "{error}");
}

#[test]
fn raw_html_and_text_directives_keep_their_semantics() {
    let html = ok("---\nconst markup = \"<b>x</b>\";\n---\n<div set:html={markup}></div>\n");
    assert!(html.contains("rawHTML"), "{html}");
    let text = ok("---\nconst markup = \"<b>x</b>\";\n---\n<div set:text={markup}></div>\n");
    assert!(!text.contains("rawHTML"), "{text}");
}

#[test]
fn generated_names_avoid_user_bindings() {
    let output = ok("---\nconst __crepusAstroProps = 1;\nconst { a } = Astro.props;\n---\n<p>{a}{__crepusAstroProps}</p>\n");
    assert!(output.contains("__crepusAstroProps_"), "{output}");
}

#[test]
fn authored_globals_and_dynamic_loading_do_not_silently_change_context() {
    for source in [
        "---\nconst value = arguments;\n---\n<p/>",
        "---\nconst get = () => arguments;\n---\n<p/>",
        "---\nconst module = import('astro');\n---\n<p/>",
        "---\nconst module = import('./helper');\n---\n<p/>",
    ] {
        assert!(compile(source).is_err(), "{source}");
    }
    ok("---\nfunction get(){ return arguments.length; }\n---\n<p>{get()}</p>");
}

#[test]
fn authored_literal_text_and_attributes_use_the_source_preserving_parser() {
    let output = ok(r#"---
const label = "x";
---
<p title="&amp;" data-js={"&amp;"} class="a  b">"quoted" \n{label}</p>"#);
    assert!(output.contains("a  b"), "{output}");
    assert!(output.contains("@tschk/moonshine-astro/ssr"), "{output}");
    assert!(output.contains("quoted"), "{output}");
}

#[test]
fn literal_attributes_remain_values_while_braced_attributes_remain_getters() {
    let output = ok("<input readonly=\"false\" disabled={\"\"} title=\"&amp;&quot;&#10;\" />");
    assert!(output.contains("\"readonly\": \"false\""), "{output}");
    assert!(output.contains("() => value"), "{output}");
    assert!(!output.contains("\"title\": () =>"), "{output}");
}

#[test]
fn logical_and_map_prefixes_retain_javascript_values_and_precedence() {
    let output = ok("---\nconst zero=0; const a='prefix'; const b=false; const items: string[] | undefined = undefined;\n---\n{zero && <b>x</b>}{a || b && <i/>}{items?.map(item => <p>{item}</p>)}");
    assert!(output.contains("items?.map"), "{output}");
}

#[test]
fn synchronous_templates_do_not_import_reactive_choice_helpers() {
    let output = ok("---\nlet reads=0; function next(){return ++reads;}\n---\n<section>{next() && <b>{reads}</b>}<i>{reads}</i></section>");
    assert!(!output.contains("moonshine-dom/control"), "{output}");
    assert!(!output.contains("createMemo"), "{output}");
}

#[test]
fn explicit_dom_ssr_imports_are_not_retargeted_to_astro_factories() {
    let module = compile("---\nimport {h as domH} from '@tschk/moonshine-dom/ssr';\nconst child=domH('b',null,'native');\n---\n<div>{child}</div>").unwrap();
    assert!(
        module
            .imports
            .iter()
            .any(|name| name == "@tschk/moonshine-dom/ssr"),
        "{:?}",
        module.imports
    );
    assert!(
        module
            .imports
            .iter()
            .any(|name| name == "@tschk/moonshine-astro/ssr"),
        "{:?}",
        module.imports
    );
}

#[test]
fn regex_grammar_is_distinct_from_division_and_astro_markup() {
    for source in [
        r#"<p>{/a?b:c/.test("ab") ? "match" : "miss"}</p>"#,
        r#"{/}/.test("}") && <br>}"#,
        r#"{(/a?b:c/.test("ab") ? <br> : <input>)}"#,
        r#"<p title={/}/.source}>{8 / /}/.source.length}</p>"#,
        r#"<p>{(()=>{if(true) /}/.test('}');return 8})()}</p>"#,
        r#"<p>{`a${/}/.test('}') ? `b}c` : 'none'}`}</p>"#,
    ] {
        ok(source);
    }
}

#[test]
fn malformed_regular_expressions_do_not_get_silently_lowered() {
    for source in [
        r#"<p>{/[unterminated/.test("x")}</p>"#,
        r#"{/unterminated && <br>}"#,
    ] {
        assert!(compile(source).is_err(), "{source}");
    }
}

#[test]
fn class_and_class_list_merge_without_admitting_duplicates() {
    for markup in [
        "<p class=\"base\" class:list={['active']}/>",
        "<p class:list={['active']} class=\"base\"/>",
        "<Card class:list={['active']} class=\"base\"/>",
    ] {
        let output = ok(&format!(
            "---\nconst Card = (props: any) => props.class;\n---\n{markup}"
        ));
        assert!(
            output.contains("__crepusAstroClass([\"base\", (void 0, [\"active\"])])"),
            "{output}"
        );
    }
    for markup in [
        "<p class=\"a\" class=\"b\" class:list={[]}/>",
        "<p class=\"a\" class:list={[]} class:list={[]}/>",
        "<p className=\"a\" class:list={[]}/>",
    ] {
        assert!(err(&format!("---\n---\n{markup}")).contains("duplicate"));
    }
}

#[test]
fn component_class_lists_omit_empty_normalized_props() {
    let output = ok("---\nconst Card = (props: {class?: string}) => props.class;\n---\n<Card class:list={[]}/><Card class=\"\" class:list={false}/>");
    assert_eq!(
        output.matches("value ? { class: value } : {}").count(),
        2,
        "{output}"
    );
    let ordinary =
        ok("---\nconst Card = (props: {class: string}) => props.class;\n---\n<Card class=\"\"/>");
    assert!(!ordinary.contains("value ?"), "{ordinary}");
}

#[test]
fn component_calls_use_source_component_and_defer_slot_content() {
    let plain = ok("---\nimport Card from \"./Card.astro\";\nconst title = \"x\";\n---\n<Card title={title} />\n");
    let plain_compact = compact(&plain);
    assert!(
        plain.contains("sourceComponent as __crepusAstroComponent"),
        "{plain}"
    );
    assert!(
        plain_compact.contains("__crepusAstroComponent(Card,{\"title\":"),
        "{plain}"
    );
    assert!(!plain.contains("__crepusAstroSlots"), "{plain}");
    assert!(!plain.contains("sourceSlot"), "{plain}");

    let output = ok("---\nimport Layout from \"./Layout.astro\";\nconst title = \"t\";\n---\n<Layout title={title}><h1>{title}</h1><b slot=\"side\">side</b></Layout>\n");
    let output_compact = compact(&output);
    assert!(
        output_compact.contains("__crepusAstroComponent(Layout,{\"title\":"),
        "{output}"
    );
    let default = output_compact
        .find("[\"default\"]:()=>__crepusAstroChild([")
        .unwrap_or_else(|| panic!("{output}"));
    let side = output_compact
        .find("[\"side\"]:()=>__crepusAstroChild([")
        .unwrap_or_else(|| panic!("{output}"));
    assert!(default < side, "{output}");
    assert!(!output_compact.contains("\"slot\""), "{output}");
    assert!(
        !output_compact.contains("__crepusAstroH(Layout"),
        "{output}"
    );
    assert!(!output.contains("sourceSlot"), "{output}");
    assert!(!output.contains("__crepusAstroSlots"), "{output}");
}

#[test]
fn outlets_read_the_second_factory_argument_and_build_fallbacks_eagerly() {
    let output = ok("---\nconst { name } = Astro.props;\n---\n<section><slot /><slot name=\"side\">{name}</slot></section>\n");
    let output_compact = compact(&output);
    assert!(
        output.contains("sourceSlot as __crepusAstroSlot"),
        "{output}"
    );
    assert!(
        output.contains("SourceSlots as __crepusAstroSlotsType"),
        "{output}"
    );
    assert!(
        output_compact.contains(",__crepusAstroSlots:__crepusAstroSlotsType={}"),
        "{output}"
    );
    assert!(
        output_compact.contains("__crepusAstroProps:Record<string,any>,"),
        "{output}"
    );
    assert!(
        output_compact
            .contains("__crepusAstroSlot(__crepusAstroSlots,\"default\",__crepusAstroChild([]))"),
        "{output}"
    );
    assert!(
        output_compact
            .contains("__crepusAstroSlot(__crepusAstroSlots,\"side\",__crepusAstroChild(["),
        "{output}"
    );
    assert!(!output_compact.contains("\"side\",()=>"), "{output}");
    assert!(!output.contains("sourceComponent"), "{output}");
}

#[test]
fn same_named_children_concatenate_and_empty_fragments_supply_empty_slots() {
    let output = ok("---\nimport Layout from \"./Layout.astro\";\n---\n<Layout><b slot=\"a\">1</b><i>x</i><Fragment slot=\"a\"><u>2</u><s>3</s></Fragment><p slot=\"b\"/><Fragment slot=\"empty\"></Fragment><em>y</em></Layout>\n");
    let output_compact = compact(&output);
    let position = |key: &str| {
        output_compact
            .find(&format!("[\"{key}\"]:()=>__crepusAstroChild(["))
            .unwrap_or_else(|| panic!("{key}\n{output}"))
    };
    let (a, default, b, empty) = (
        position("a"),
        position("default"),
        position("b"),
        position("empty"),
    );
    assert!(a < default && default < b && b < empty, "{output}");
    let group = &output_compact[a..default];
    let first = group.find("\"b\"").unwrap_or_else(|| panic!("{output}"));
    let second = group.find("\"u\"").unwrap_or_else(|| panic!("{output}"));
    let third = group.find("\"s\"").unwrap_or_else(|| panic!("{output}"));
    assert!(first < second && second < third, "{output}");
    assert!(!group.contains("\"i\""), "{output}");
    let default_group = &output_compact[default..b];
    assert!(
        default_group.contains("\"i\"") && default_group.contains("\"em\""),
        "{output}"
    );
    assert!(
        output_compact.contains("[\"empty\"]:()=>__crepusAstroChild([])"),
        "{output}"
    );
    assert!(!output_compact.contains("\"slot\""), "{output}");
    assert!(!output.contains("Fragment"), "{output}");
}

#[test]
fn whitespace_between_named_children_does_not_supply_a_default_slot() {
    let output = ok("---\nimport Layout from \"./Layout.astro\";\n---\n<Layout> <b slot=\"a\">1</b> <i slot=\"b\">2</i> </Layout>\n");
    let output_compact = compact(&output);
    assert!(output_compact.contains("[\"a\"]:()=>"), "{output}");
    assert!(output_compact.contains("[\"b\"]:()=>"), "{output}");
    assert!(!output_compact.contains("[\"default\"]"), "{output}");
    let explicit = ok("---\nimport Layout from \"./Layout.astro\";\n---\n<Layout><Fragment slot=\"default\"></Fragment></Layout>\n");
    assert!(
        compact(&explicit).contains("[\"default\"]:()=>__crepusAstroChild([])"),
        "{explicit}"
    );
    let text = ok("---\nimport Layout from \"./Layout.astro\";\n---\n<Layout>text<b slot=\"a\">1</b></Layout>\n");
    assert!(compact(&text).contains("[\"default\"]:()=>"), "{text}");
}

#[test]
fn children_and_slots_props_stay_ordinary_beside_slot_content() {
    let output = ok("---\nimport Layout from \"./Layout.astro\";\nconst children = [\"c\"];\nconst slots = { x: 1 };\n---\n<Layout children={children} slots={slots}><b>content</b></Layout>\n");
    let output_compact = compact(&output);
    let props = output_compact
        .find("__crepusAstroComponent(Layout,{")
        .unwrap_or_else(|| panic!("{output}"));
    let children = output_compact[props..]
        .find("\"children\":")
        .unwrap_or_else(|| panic!("{output}"));
    let slots = output_compact[props..]
        .find("\"slots\":")
        .unwrap_or_else(|| panic!("{output}"));
    let default = output_compact[props..]
        .find("[\"default\"]:()=>")
        .unwrap_or_else(|| panic!("{output}"));
    assert!(children < default && slots < default, "{output}");
}

#[test]
fn forwarded_outlets_are_slot_content_of_immediate_component_children() {
    let output = ok("---\nimport Layout from \"./Layout.astro\";\n---\n<Layout><slot name=\"head\" slot=\"head\">fallback</slot><slot /></Layout>\n");
    let output_compact = compact(&output);
    assert!(
        output_compact.contains("[\"head\"]:()=>__crepusAstroChild([__crepusAstroSlot(__crepusAstroSlots,\"head\",__crepusAstroChild(["),
        "{output}"
    );
    assert!(
        output_compact.contains("[\"default\"]:()=>__crepusAstroChild([__crepusAstroSlot(__crepusAstroSlots,\"default\",__crepusAstroChild([]))"),
        "{output}"
    );
    assert!(!output_compact.contains("\"slot\""), "{output}");
    assert!(
        output_compact.contains(",__crepusAstroSlots:__crepusAstroSlotsType={}"),
        "{output}"
    );
}

#[test]
fn prototype_property_names_are_own_computed_slot_keys() {
    let output = ok("---\nimport Layout from \"./Layout.astro\";\n---\n<Layout><b slot=\"__proto__\">p</b><b slot=\"toString\">t</b></Layout>\n");
    let output_compact = compact(&output);
    assert!(output_compact.contains("[\"__proto__\"]:()=>"), "{output}");
    assert!(output_compact.contains("[\"toString\"]:()=>"), "{output}");
    let outlet = ok("<slot name=\"__proto__\" /><slot name=\"toString\" />\n");
    let outlet_compact = compact(&outlet);
    assert!(
        outlet_compact.contains("__crepusAstroSlots,\"__proto__\","),
        "{outlet}"
    );
    assert!(
        outlet_compact.contains("__crepusAstroSlots,\"toString\","),
        "{outlet}"
    );
}

#[test]
fn slot_helper_names_avoid_user_bindings() {
    let output = ok("---\nimport Layout from \"./Layout.astro\";\nconst __crepusAstroSlots = 1;\nconst __crepusAstroComponent = 2;\nconst __crepusAstroSlotsType = 3;\n---\n<Layout><slot />{__crepusAstroSlots}{__crepusAstroComponent}{__crepusAstroSlotsType}</Layout>\n");
    assert!(output.contains("__crepusAstroSlots_"), "{output}");
    assert!(output.contains("__crepusAstroComponent_"), "{output}");
    assert!(output.contains("__crepusAstroSlotsType_"), "{output}");
}

#[test]
fn unsupported_slot_variants_are_rejected_instead_of_reinterpreted() {
    let prelude = "---\nimport Layout from \"./Layout.astro\";\nconst name = \"x\";\nconst show = true;\n---\n";
    for (markup, needle) in [
        ("<Layout><b slot={name}>x</b></Layout>", "dynamic"),
        (
            "<Layout><Fragment slot={name}>x</Fragment></Layout>",
            "dynamic",
        ),
        ("<Layout><Fragment slot>x</Fragment></Layout>", "dynamic"),
        ("<slot name={name} />", "dynamic"),
        (
            "<Layout><slot name={name} slot=\"a\" /></Layout>",
            "dynamic",
        ),
        ("<Layout><slot slot={name} /></Layout>", "dynamic"),
        ("<Layout><b slot=\"\">x</b></Layout>", "empty"),
        ("<slot name=\"\" />", "empty"),
        ("<slot name=\"a\" class=\"b\" />", "unsupported attribute"),
        ("<slot name=\"a\" name=\"b\" />", "duplicate"),
        ("<div><b slot=\"a\">x</b></div>", "immediate child"),
        ("<b slot=\"a\">x</b>", "immediate child"),
        ("<Layout slot=\"a\" />", "immediate child"),
        (
            "<Layout><div><b slot=\"a\">x</b></div></Layout>",
            "immediate child",
        ),
        (
            "<Layout>{show && <b slot=\"a\">x</b>}</Layout>",
            "immediate child",
        ),
        (
            "<Layout>{[1].map(item => <b slot=\"a\">{item}</b>)}</Layout>",
            "immediate child",
        ),
        (
            "<Layout><slot><b slot=\"a\">x</b></slot></Layout>",
            "immediate child",
        ),
        (
            "<Layout><Fragment slot=\"a\"><b slot=\"b\">x</b></Fragment></Layout>",
            "immediate child",
        ),
        (
            "<Layout><div><slot slot=\"a\" /></div></Layout>",
            "immediate child",
        ),
        ("<Fragment slot=\"a\">x</Fragment>", "immediate child"),
        (
            "<div><Fragment slot=\"a\">x</Fragment></div>",
            "immediate child",
        ),
        (
            "<Layout>{show && <Fragment slot=\"a\">x</Fragment>}</Layout>",
            "immediate child",
        ),
        (
            "<Layout><Fragment slot=\"a\" class=\"b\">x</Fragment></Layout>",
            "Fragment",
        ),
    ] {
        let error = err(&format!("{prelude}{markup}"));
        assert!(error.contains(needle), "{markup}: {error}");
    }
    for markup in [
        "<Layout><b slot=\"a\" slot=\"b\">x</b></Layout>",
        "<Layout>{await load()}</Layout>",
    ] {
        assert!(compile(&format!("{prelude}{markup}")).is_err(), "{markup}");
    }
    assert!(err("---\nconst slots = Astro.slots;\n---\n<p />\n").contains("Astro.slots"));
    assert!(
        err("---\nconst html = Astro.slots.render(\"x\");\n---\n<p />\n").contains("Astro.slots")
    );
}

#[test]
fn projected_components_keep_slot_props_and_whitespace_fragments_stay_empty() {
    let output=ok("---\nimport Layout from './Layout.astro';\nimport Child from './Child.astro';\n---\n<Layout><Child slot=\"title\"/></Layout>");
    assert!(
        compact(&output).contains("__crepusAstroComponent(Child,{\"slot\":\"title\"}"),
        "{output}"
    );
    let empty=ok("---\nimport Layout from './Layout.astro';\n---\n<Layout><Fragment slot=\"default\"> </Fragment></Layout>");
    assert!(
        compact(&empty).contains("[\"default\"]:()=>__crepusAstroChild([])"),
        "{empty}"
    );
}
