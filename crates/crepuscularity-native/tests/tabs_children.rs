use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

fn source(filename: &str, markup: &str) -> String {
    if filename.ends_with(".vue") {
        format!("<template>{markup}</template>")
    } else {
        markup.into()
    }
}

#[test]
fn unsupported_tabs_elements_and_text_are_not_silently_dropped() {
    for filename in [
        "Entry.crepus",
        "Entry.jsx",
        "Entry.svelte",
        "Entry.vue",
        "Entry.astro",
        "Entry.ng",
    ] {
        for tag in ["tabs", "tabview", "page-switcher"] {
            for text in [false, true] {
                let markup = if filename.ends_with(".crepus") {
                    if text {
                        format!("{tag}\n  \"Lost content\"\n  tab label=One value=one")
                    } else {
                        format!("{tag}\n  div\n    \"Lost content\"\n  tab label=One value=one")
                    }
                } else if text {
                    format!("<{tag}>Lost content<tab label='One' value='one'/></{tag}>")
                } else {
                    format!("<{tag}><div>Lost content</div><tab label='One' value='one'/></{tag}>")
                };
                let error = render_template_to_ir_with_path(
                    &source(filename, &markup),
                    &TemplateContext::new(),
                    Some(Path::new(filename)),
                )
                .unwrap_err();
                assert!(
                    error.to_string().contains("unsupported View IR tabs child"),
                    "{filename}: {markup}: {error}"
                );
            }
        }
    }
}

#[test]
fn whitespace_and_valid_panels_keep_content_and_normal_control_flow() {
    for filename in [
        "Entry.crepus",
        "Entry.jsx",
        "Entry.svelte",
        "Entry.vue",
        "Entry.astro",
        "Entry.ng",
    ] {
        let markup = if filename.ends_with(".crepus") {
            "tabs\n  tab label=One value=one\n    span \"Panel one\"\n  page label=Two value=two\n    span \"Panel two\""
        } else {
            "<tabs> \n <tab label='One' value='one'><span>Panel one</span></tab> \n <page label='Two' value='two'><span>Panel two</span></page> </tabs>"
        };
        let ir = render_template_to_ir_with_path(
            &source(filename, markup),
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        let json = serde_json::to_value(ir).unwrap();
        assert_eq!(
            json["root"][0]["tabs"].as_array().unwrap().len(),
            2,
            "{filename}"
        );
        let serialized = json.to_string();
        assert!(
            serialized.contains("Panel one") && serialized.contains("Panel two"),
            "{filename}: {serialized}"
        );
    }
    let ir=render_template_to_ir_with_path("tabs\n  tab label=One value=one\n    $: let message = 'Panel scoped'\n    if true\n      span \"{message}\"",&TemplateContext::new(),None).unwrap();
    assert!(serde_json::to_string(&ir).unwrap().contains("Panel scoped"));
}

#[test]
fn structural_metadata_and_handlers_keep_explicit_errors() {
    for (body, diagnostic) in [
        ("if true\n    tab label=One", "control flow"),
        ("for item in items\n    tab label=One", "control flow"),
        ("include child.crepus", "includes"),
        ("tab label=One @click=save", "handlers"),
        ("$: let label = 'lost'", "unsupported View IR tabs child"),
        ("{missing}", "unsupported View IR tabs child"),
    ] {
        let error = render_template_to_ir_with_path(
            &format!("tabs\n  {body}"),
            &TemplateContext::new(),
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains(diagnostic), "{body}: {error}");
    }
}
