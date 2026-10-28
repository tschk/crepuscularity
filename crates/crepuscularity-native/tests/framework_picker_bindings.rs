use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir,
};

#[test]
fn pickers_read_host_values_without_changing_action_dispatch() {
    let ir = render_template_to_ir("picker bind=prefs.choice @change=choose\n  option value=\"one\" \"One\"\n  option value=\"two\" \"Two\"\nfor row in rows\n  picker bind=row.choice\n    option value=\"one\" \"One\"\npicker", &TemplateContext::new()).unwrap();
    for (name, output, scope, event) in [
        (
            "moonshine",
            emit_moonshine_component(&ir, "Choices"),
            "scope",
            "onChange={(event) => __crepusOwnHandler(handlers, \"choose\")?.choose?.(event)}",
        ),
        (
            "solid",
            emit_solid_component(&ir, "Choices"),
            "scope()",
            "onChange={(event) => __crepusOwnHandler(handlers(), \"choose\")?.choose?.(event)}",
        ),
        (
            "svelte",
            emit_svelte_component(&ir),
            "scope",
            "onchange={(event) => __crepusOwnHandler(handlers, \"choose\")?.choose?.(event)}",
        ),
        (
            "vue",
            emit_vue_component(&ir),
            "scope",
            "@change=\"__crepusOwnHandler(handlers, \"choose\")?.choose?.($event)\"",
        ),
    ] {
        let output = output.replace("&quot;", "\"");
        assert!(
            output.contains(&format!("({scope}?.prefs?.choice ?? \"\")")),
            "{name}: {output}"
        );
        assert!(
            output.contains("(_crepusItem0?.choice ?? \"\")"),
            "{name}: {output}"
        );
        assert!(output.contains(event), "{name}: {output}");
        let unbound = output
            .lines()
            .rfind(|line| line.contains("<select"))
            .unwrap();
        assert!(!unbound.contains("value="), "{name}: {unbound}");
    }
}
