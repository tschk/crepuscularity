use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_template_to_ir_with_path;
use std::path::Path;

fn lower(
    source: &str,
    filename: &str,
) -> Result<serde_json::Value, crepuscularity_core::CrepusError> {
    render_template_to_ir_with_path(source, &TemplateContext::new(), Some(Path::new(filename)))
        .map(|ir| serde_json::to_value(ir).unwrap())
}

#[test]
fn html_file_inputs_preserve_selection_metadata_and_change_handlers_in_six_frontends() {
    for (filename, source) in [
        ("Entry.crepus", "input type=file accept=\"image/*,.txt\" multiple=true required=true name=files @change=files.pick"),
        ("Entry.jsx", "<input type='file' accept='image/*,.txt' multiple required name='files' onChange={files.pick}/>"),
        ("Entry.svelte", "<input type='file' accept='image/*,.txt' multiple required name='files' on:change={files.pick}/>"),
        ("Entry.astro", "<input type='file' accept='image/*,.txt' multiple required name='files' onChange={files.pick}/>"),
        ("Entry.vue", "<template><input type='file' accept='image/*,.txt' multiple required name='files' @change='files.pick'/></template>"),
        ("Entry.ng", "<input type='file' accept='image/*,.txt' multiple required name='files' (change)='files.pick'/>"),
    ] {
        let ir = lower(source, filename).unwrap();
        let node = &ir["root"][0];
        assert_eq!(node["kind"], "filePicker", "{filename}: {ir}");
        assert_eq!(node["accept"], serde_json::json!(["image/*", ".txt"]));
        assert_eq!(node["multiple"], true);
        assert_eq!(node["onPick"], "files.pick");
        assert_eq!(node["style"]["formName"], "files");
        assert_eq!(node["style"]["required"], true);
    }
}

#[test]
fn standard_file_multiple_uses_presence_while_custom_picker_coercion_is_unchanged() {
    for (source, expected) in [
        ("<input type='file' multiple='false'/>", true),
        ("<input type='file' multiple=''/>", true),
        ("<input type='file' multiple={false}/>", false),
        ("<input type='file' multiple={null}/>", false),
        ("<input type='file' class='multiple'/>", false),
        ("<file-picker multiple='false'/>", true),
        ("<file-picker class='multiple'/>", true),
    ] {
        let ir = lower(source, "Entry.jsx").unwrap();
        assert_eq!(ir["root"][0]["multiple"], expected, "{source}: {ir}");
    }
}

#[test]
fn standard_file_inputs_reject_value_bind_and_nonselection_events() {
    for property in ["value", "bind"] {
        for value in ["''", "'user.files'"] {
            let source = format!("<input type='file' {property}={value}/>");
            let error = lower(&source, "Entry.jsx").unwrap_err();
            assert!(
                error.to_string().contains("do not support value or bind"),
                "{error}"
            );
        }
    }
    for event in ["click", "input", "pick"] {
        for (filename, source) in [
            ("Entry.crepus", format!("input type=file @{event}=choose")),
            (
                "Entry.jsx",
                format!(
                    "<input type='file' on{}{}={{choose}}/>",
                    event[..1].to_ascii_uppercase(),
                    &event[1..]
                ),
            ),
            (
                "Entry.svelte",
                format!("<input type='file' on:{event}={{choose}}/>"),
            ),
            (
                "Entry.astro",
                format!(
                    "<input type='file' on{}{}={{choose}}/>",
                    event[..1].to_ascii_uppercase(),
                    &event[1..]
                ),
            ),
            (
                "Entry.vue",
                format!("<template><input type='file' @{event}='choose'/></template>"),
            ),
            (
                "Entry.ng",
                format!("<input type='file' ({event})='choose'/>"),
            ),
        ] {
            let error = lower(&source, filename).unwrap_err();
            assert!(error.to_string().contains(event), "{filename}: {error}");
            assert!(
                error.to_string().contains("does not support"),
                "{filename}: {error}"
            );
        }
    }
}
