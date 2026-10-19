#![cfg(feature = "web-modules")]

use crepuscularity_native::{compile_moonshine_astro, MoonshineModuleTarget};
use std::path::Path;

fn compile(source: &str) -> Result<String, String> {
    compile_moonshine_astro(
        source,
        Path::new("Layout.astro"),
        MoonshineModuleTarget::Ssr,
    )
    .map(|module| module.source)
}

fn ok(source: &str) -> String {
    compile(source).unwrap_or_else(|error| panic!("{error}\n{source}"))
}

fn err(source: &str) -> String {
    compile(source).expect_err(source)
}

fn compact(output: &str) -> String {
    output.split_whitespace().collect()
}

const HAS: &str = "__crepusAstroHasSlot";
const SLOTS: &str = "__crepusAstroSlots";

#[test]
fn static_and_dynamic_frontmatter_calls_lower_to_the_imported_helper() {
    let output = ok("---\nconst n = \"x\";\nconst cond = n.length > 0;\nconst a = Astro.slots.has(\"head\");\nconst b = Astro.slots.has(n + \"y\");\nconst c = Astro.slots.has(`h${n}`);\nconst d = Astro.slots.has(cond ? \"p\" : \"q\");\nconst e = Astro.slots.has(Astro.props.name);\nconst f = Astro.slots.has(Astro.props.names[0]);\n---\n<p>{String(a)}{String(b)}{String(c)}{String(d)}{String(e)}{String(f)}</p>\n");
    let flat = compact(&output);
    assert!(!output.contains("Astro.slots"), "{output}");
    assert!(!output.contains("Astro.props"), "{output}");
    assert!(
        output.contains(&format!("sourceHasSlot as {HAS}")),
        "{output}"
    );
    assert_eq!(output.matches("sourceHasSlot as").count(), 1, "{output}");
    assert!(output.contains("@tschk/moonshine-astro/ssr"), "{output}");
    assert!(!output.contains("astro/runtime"), "{output}");
    for call in [
        format!("{HAS}({SLOTS},\"head\")"),
        format!("{HAS}({SLOTS},n+\"y\")"),
        format!("{HAS}({SLOTS},`h${{n}}`)"),
        format!("{HAS}({SLOTS},cond?\"p\":\"q\")"),
        format!("{HAS}({SLOTS},__crepusAstroProps.name)"),
        format!("{HAS}({SLOTS},__crepusAstroProps.names[0])"),
    ] {
        assert!(flat.contains(&call), "{call}\n{output}");
    }
}

#[test]
fn frontmatter_only_presence_without_an_outlet_still_gets_a_typed_slots_parameter() {
    let output =
        ok("---\nconst hasHead = Astro.slots.has(\"head\");\n---\n<p>{String(hasHead)}</p>\n");
    let flat = compact(&output);
    assert!(!output.contains("<slot"), "{output}");
    assert!(!output.contains("sourceSlot as"), "{output}");
    assert!(
        output.contains("SourceSlots as __crepusAstroSlotsType"),
        "{output}"
    );
    assert!(
        flat.contains(&format!(",{SLOTS}:__crepusAstroSlotsType={{}}")),
        "{output}"
    );
    assert!(
        flat.contains(&format!("{HAS}({SLOTS},\"head\")")),
        "{output}"
    );
}

#[test]
fn components_using_neither_presence_nor_outlets_gain_no_slots_machinery() {
    let output = ok("---\nconst { a } = Astro.props;\nconst s = { has: (n: string) => n };\nconst v = s.has(\"x\");\n---\n<p>{a}{v}</p>\n");
    for absent in ["sourceHasSlot", "sourceSlot", "SourceSlots", SLOTS, HAS] {
        assert!(!output.contains(absent), "{absent}\n{output}");
    }
}

#[test]
fn template_calls_with_nested_props_share_one_helper_import_and_slots_parameter() {
    let output = ok("---\nconst names = [\"a\", \"b\"];\n---\n<p>{String(Astro.slots.has(Astro.props.name))}</p>\n<ul>{names.map((n) => <li>{String(Astro.slots.has(n))}</li>)}</ul>\n<slot name=\"head\" />\n");
    let flat = compact(&output);
    assert!(!output.contains("Astro.slots"), "{output}");
    assert_eq!(output.matches("sourceHasSlot as").count(), 1, "{output}");
    assert_eq!(output.matches("sourceSlot as").count(), 1, "{output}");
    assert_eq!(output.matches("SourceSlots as").count(), 1, "{output}");
    assert_eq!(
        flat.matches(&format!(",{SLOTS}:__crepusAstroSlotsType={{}}"))
            .count(),
        1,
        "{output}"
    );
    assert!(
        flat.contains(&format!("{HAS}({SLOTS},__crepusAstroProps.name)")),
        "{output}"
    );
    assert!(flat.contains(&format!("{HAS}({SLOTS},n)")), "{output}");
    assert!(
        flat.contains(&format!("__crepusAstroSlot({SLOTS},\"head\",")),
        "{output}"
    );
}

#[test]
fn template_only_presence_without_frontmatter_compiles() {
    let output = ok("<p>{String(Astro.slots.has(\"head\"))}</p>\n");
    assert!(
        compact(&output).contains(&format!("{HAS}({SLOTS},\"head\")")),
        "{output}"
    );
    assert!(!output.contains("Astro.slots"), "{output}");
}

#[test]
fn argument_shapes_keep_evaluation_once_and_in_order() {
    let output = ok("---\nconst order: string[] = [];\nconst names = [\"x\"];\nconst a = Astro.slots.has((order.push(\"first\"), order.push(\"second\")) + \"\");\nconst b = Astro.slots.has(...names);\nconst c = Astro.slots.has();\nconst d = Astro.slots.has(\"k\", order.push(\"extra\"));\nconst e = Astro.slots.has(Astro.slots.has(\"inner\") ? \"yes\" : \"no\");\n---\n<p>{String(a)}</p>\n");
    let flat = compact(&output);
    assert_eq!(
        output.matches("order.push(\"first\")").count(),
        1,
        "{output}"
    );
    assert_eq!(
        output.matches("order.push(\"extra\")").count(),
        1,
        "{output}"
    );
    let first = flat.find("order.push(\"first\")").expect("first");
    let second = flat.find("order.push(\"second\")").expect("second");
    assert!(first < second, "{output}");
    assert!(
        flat.contains(&format!(
            "{HAS}({SLOTS},(order.push(\"first\"),order.push(\"second\"))+\"\")"
        )),
        "{output}"
    );
    assert!(
        flat.contains(&format!("{HAS}({SLOTS},...names)")),
        "{output}"
    );
    assert!(flat.contains(&format!("{HAS}({SLOTS})")), "{output}");
    assert!(
        flat.contains(&format!("{HAS}({SLOTS},\"k\",order.push(\"extra\"))")),
        "{output}"
    );
    assert!(
        flat.contains(&format!(
            "{HAS}({SLOTS},{HAS}({SLOTS},\"inner\")?\"yes\":\"no\")"
        )),
        "{output}"
    );
    assert!(!output.contains("Astro.slots"), "{output}");
}

#[test]
fn shadowed_local_astro_bindings_stay_untouched() {
    let module = ok("---\nconst Astro = { slots: { has: (name: string) => name } };\nconst v = Astro.slots.has(\"x\");\n---\n<p>{v}</p>\n");
    let flat = compact(&module);
    assert!(flat.contains("Astro.slots.has(\"x\")"), "{module}");
    for absent in ["sourceHasSlot", "SourceSlots", SLOTS, HAS] {
        assert!(!module.contains(absent), "{absent}\n{module}");
    }

    let parameter = ok("---\nconst check = (Astro: any) => Astro.slots.has(\"local\");\nconst real = Astro.slots.has(\"real\");\n---\n<p>{String(real)}{String(check({ slots: { has: () => 1 } }))}</p>\n");
    let flat = compact(&parameter);
    assert!(flat.contains("Astro.slots.has(\"local\")"), "{parameter}");
    assert!(
        flat.contains(&format!("{HAS}({SLOTS},\"real\")")),
        "{parameter}"
    );
    assert!(
        !flat.contains(&format!("{HAS}({SLOTS},\"local\")")),
        "{parameter}"
    );
}

#[test]
fn generated_identifiers_and_user_globals_do_not_collide() {
    let output = ok("---\nconst __crepusAstroHasSlot = 1;\nconst __crepusAstroSlots = 2;\nconst __crepusAstroSlotsType = 3;\nconst Object = { hasOwn: () => true };\nconst globalThis = { Object };\nconst present = Astro.slots.has(\"a\");\n---\n<p>{String(present)}{__crepusAstroHasSlot}{__crepusAstroSlots}{__crepusAstroSlotsType}</p>\n");
    let flat = compact(&output);
    assert!(
        output.contains("sourceHasSlot as __crepusAstroHasSlot_"),
        "{output}"
    );
    assert!(
        flat.contains("__crepusAstroHasSlot_(__crepusAstroSlots_,\"a\")"),
        "{output}"
    );
    assert!(
        flat.contains(",__crepusAstroSlots_:__crepusAstroSlotsType_={}"),
        "{output}"
    );
    assert!(!output.contains("Object.hasOwn("), "{output}");
    assert!(!output.contains("Astro.slots"), "{output}");
}

#[test]
fn emitted_source_delegates_to_own_presence_not_truthiness_or_prototype_lookup() {
    let layout = ok("---\nconst a = Astro.slots.has(\"empty\");\nconst b = Astro.slots.has(\"absent\");\nconst c = Astro.slots.has(\"toString\");\nconst d = Astro.slots.has(\"__proto__\");\n---\n<p>{String(a)}{String(b)}{String(c)}{String(d)}</p>\n");
    let flat = compact(&layout);
    for name in ["empty", "absent", "toString", "__proto__"] {
        assert!(
            flat.contains(&format!("{HAS}({SLOTS},\"{name}\")")),
            "{name}\n{layout}"
        );
    }
    for forbidden in [
        "Boolean(",
        "Object.hasOwn",
        "hasOwnProperty",
        "!!",
        &format!("{SLOTS}[\""),
        &format!("in{SLOTS}"),
    ] {
        assert!(!flat.contains(forbidden), "{forbidden}\n{layout}");
    }

    let page = ok("---\nimport Layout from \"./Layout.astro\";\n---\n<Layout><Fragment slot=\"empty\"></Fragment><b slot=\"full\">x</b></Layout>\n");
    let flat = compact(&page);
    assert!(
        flat.contains("[\"empty\"]:()=>__crepusAstroChild([])"),
        "{page}"
    );
    assert!(flat.contains("[\"full\"]:()=>"), "{page}");
    assert!(!flat.contains("\"absent\""), "{page}");
}

#[test]
fn unsupported_bag_render_reference_and_write_forms_still_reject() {
    for expression in [
        "const s = Astro.slots;",
        "const { has } = Astro.slots;",
        "const r = Astro.slots.render(\"x\");",
        "const r = Astro.slots.hasOwn(\"x\");",
        "const f = Astro.slots.has;",
        "const f = Astro.slots.has.bind(Astro.slots);",
        "const r = Astro.slots.has.call(null, \"x\");",
        "const r = Astro.slots.has.apply(null, [\"x\"]);",
        "Astro.slots.has = () => true;",
        "Astro.slots = {};",
        "delete Astro.slots.has;",
        "const r = Astro.slots[\"has\"](\"x\");",
        "const k = \"has\"; const r = Astro.slots[k](\"x\");",
        "const r = Astro[\"slots\"].has(\"x\");",
        "const r = Astro.slots?.has(\"x\");",
        "const r = Astro?.slots.has(\"x\");",
        "const r = Astro.slots.has?.(\"x\");",
        "const r = Astro.slots.has<string>(\"x\");",
        "const r = new Astro.slots.has(\"x\");",
        "const r = Astro.slots.has`x`;",
        "const r = Astro.slots.has(\"x\") && Astro.url;",
        "const r = Astro.slots.has(Astro.slots);",
        "const r = Astro.slots.has(Astro);",
    ] {
        let message = err(&format!("---\n{expression}\n---\n<p />\n"));
        assert!(message.contains("Astro"), "{expression}: {message}");
    }
    assert!(err("<p>{[1].map(Astro.slots.has)}</p>").contains("arrow function"));
    for expression in [
        "{Astro.slots}",
        "{Astro.slots.render(\"x\")}",
        "{Astro.slots.has?.(\"x\")}",
    ] {
        let message = err(&format!("<p>{expression}</p>\n"));
        assert!(message.contains("Astro"), "{expression}: {message}");
    }
    assert!(err("---\nconst r = await Astro.slots.render(\"x\");\n---\n<p />\n").contains("await"));
}
