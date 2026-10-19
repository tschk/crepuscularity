use crepuscularity_core::TemplateContext;
use crepuscularity_native::{
    emit_moonshine_component, emit_solid_component, emit_svelte_component, emit_vue_component,
    render_template_to_ir,
};

#[test]
fn inputs_and_textareas_read_host_values_and_keep_unbound_fields_editable() {
    let ir = render_template_to_ir("input bind=user.name placeholder=\"Name\" @change=save\ninput type=\"password\" bind=user.password\ntextarea bind=user.bio @change=save\nfor row in rows\n  input bind=row.name\ninput\ntextarea", &TemplateContext::new()).unwrap();
    for (name, output, scope, event) in [
        (
            "moonshine",
            emit_moonshine_component(&ir, "Fields"),
            "scope",
            r#", handlers, "save")}"#,
        ),
        (
            "solid",
            emit_solid_component(&ir, "Fields"),
            "scope()",
            "onChange={(event) => __crepusOwnHandler(handlers(), \"save\")?.save?.(event)}",
        ),
        (
            "svelte",
            emit_svelte_component(&ir),
            "scope",
            "onchange={(event) => __crepusOwnHandler(handlers, \"save\")?.save?.(event)}",
        ),
        (
            "vue",
            emit_vue_component(&ir),
            "scope",
            "@change=\"__crepusOwnHandler(handlers, \"save\")?.save?.($event)\"",
        ),
    ] {
        let output = output.replace("&quot;", "\"");
        for path in ["name", "password", "bio"] {
            let field = output
                .lines()
                .find(|line| line.contains(&format!("name=\"user.{path}\"")))
                .unwrap();
            assert!(
                field.contains("value=") || field.contains("defaultValue="),
                "{name}: {field}"
            );
            assert!(
                field.contains(&format!("({scope}?.user?.{path} ?? \"\")")),
                "{name}: {field}"
            );
        }
        assert!(
            output.contains("(_crepusItem0?.name ?? \"\")"),
            "{name}: {output}"
        );
        assert!(output.contains("type=\"password\""), "{name}: {output}");
        assert_eq!(output.matches(event).count(), 2, "{name}: {output}");
        let fields: Vec<_> = output
            .lines()
            .filter(|line| line.contains("<input") || line.contains("<textarea"))
            .collect();
        for field in fields.iter().rev().take(2) {
            assert!(!field.contains("value="), "{name}: {field}");
        }
    }
}
