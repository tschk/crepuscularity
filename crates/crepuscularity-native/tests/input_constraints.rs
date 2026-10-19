use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_imba_component, emit_moonshine_dom_component, emit_moonshine_ssr_component,
    emit_react_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir_with_path,
};
use std::path::Path;

#[test]
fn input_constraints_survive_every_frontend_and_web_emitter() {
    for (filename, source) in [
        ("Field.crepus", "input type=number min=18 max=99 step=1"),
        (
            "Field.jsx",
            "<input type=\"number\" min=\"18\" max=\"99\" step=\"1\" />",
        ),
        (
            "Field.svelte",
            "<input type=\"number\" min=\"18\" max=\"99\" step=\"1\" />",
        ),
        (
            "Field.vue",
            "<template><input type=\"number\" min=\"18\" max=\"99\" step=\"1\" /></template>",
        ),
        (
            "Field.astro",
            "<input type=\"number\" min=\"18\" max=\"99\" step=\"1\" />",
        ),
        (
            "Field.ng",
            "<input type=\"number\" min=\"18\" max=\"99\" step=\"1\" />",
        ),
    ] {
        let ir = render_template_to_ir_with_path(
            source,
            &TemplateContext::new(),
            Some(Path::new(filename)),
        )
        .unwrap();
        let value = serde_json::to_value(&ir).unwrap();
        assert_eq!(value["root"][0]["style"]["formMin"], "18", "{filename}");
        assert_eq!(value["root"][0]["style"]["formMax"], "99", "{filename}");
        assert_eq!(value["root"][0]["style"]["formStep"], "1", "{filename}");
        for (host, output) in [
            ("react", emit_react_component(&ir, "Fields")),
            ("solid", emit_solid_component(&ir, "Fields")),
            ("svelte", emit_svelte_component(&ir)),
            ("vue", emit_vue_component(&ir)),
            ("imba", emit_imba_component(&ir).unwrap().source),
            ("dom", emit_moonshine_dom_component(&ir, "Fields").unwrap()),
            ("ssr", emit_moonshine_ssr_component(&ir, "Fields").unwrap()),
        ] {
            for (name, value) in [("min", "18"), ("max", "99"), ("step", "1")] {
                let attr = match host {
                    "imba" => format!("{name}=(\"{value}\")"),
                    "dom" | "ssr" => format!("{name}: \"{value}\""),
                    _ => format!("{name}=\"{value}\""),
                };
                assert_eq!(
                    output.matches(&attr).count(),
                    1,
                    "{filename} {host}: {output}"
                );
            }
        }
    }
}

#[test]
fn constraints_accept_temporal_values_and_reject_inapplicable_types() {
    for source in [
        "input type=date min=\"2026-01-01\" max=\"2026-12-31\" step=any",
        "textfield type=number min=0",
        "input type=time step=60",
    ] {
        assert!(
            render_template_to_ir_with_path(source, &TemplateContext::new(), None).is_ok(),
            "{source}"
        );
    }
    for source in [
        "input min=0",
        "input type=password max=9",
        "textarea step=1",
        "input type=number secure min=0",
        "input type=number min=0 min=1",
    ] {
        assert!(
            render_template_to_ir_with_path(source, &TemplateContext::new(), None).is_err(),
            "{source}"
        );
    }
}
