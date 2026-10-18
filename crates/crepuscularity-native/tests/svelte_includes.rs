use std::collections::HashMap;

use crepuscularity_core::TemplateContext;
use crepuscularity_native::render_from_files;
use serde_json::Value;

fn text_values(node: &Value, output: &mut Vec<String>) {
    if node["kind"] == "text" {
        output.push(node["content"].as_str().unwrap().into());
    }
    for field in [
        "root",
        "children",
        "thenChildren",
        "elseChildren",
        "itemBody",
    ] {
        if let Some(children) = node[field].as_array() {
            for child in children {
                text_values(child, output);
            }
        }
    }
}

#[test]
fn virtual_svelte_includes_dispatch_nested_vue_paths_and_isolate_props() {
    let files = HashMap::from([
        ("main.svelte".into(), "<include src=\"parts/outer.svelte\" {title} />".into()),
        ("parts/outer.svelte".into(), "<div><span>Outer {title}/{secret}</span><include path=\"nested/card.vue\" title=\"Local\" /></div>".into()),
        ("parts/nested/card.vue".into(), "<template><span>Inner {{ title }}/{{ secret }}</span></template>".into()),
    ]);
    let mut context = TemplateContext::new();
    context.set("title", "Root");
    context.set("secret", "Host secret");
    let ir = render_from_files(&files, "main.svelte", &context).unwrap();
    let mut values = vec![];
    text_values(&serde_json::to_value(ir).unwrap(), &mut values);
    assert_eq!(values, ["Outer Root/", "Inner Local/"]);
}

#[test]
fn svelte_include_slots_keep_caller_scope_and_callee_crepus_wrapper_scope() {
    let files = HashMap::from([
        ("main.svelte".into(), "<include src=\"parts/card.crepus\" title=\"Callee\">{#if show}<span>Caller {title}</span>{/if}</include>".into()),
        ("parts/card.crepus".into(), "div\n  text \"Callee: {title}\"\n  slot\n    text \"Fallback\"".into()),
    ]);
    let mut context = TemplateContext::new();
    context.set("title", "Root");
    context.set("show", true);
    let ir = render_from_files(&files, "main.svelte", &context).unwrap();
    let mut values = vec![];
    text_values(&serde_json::to_value(ir).unwrap(), &mut values);
    assert_eq!(values, ["Callee: Callee", "Caller Root"]);
}

#[test]
fn recursive_svelte_includes_use_existing_depth_guard_and_recover() {
    let mut files = HashMap::from([(
        "main.svelte".into(),
        "<include src=\"main.svelte\" />".into(),
    )]);
    let error = render_from_files(&files, "main.svelte", &TemplateContext::new()).unwrap_err();
    assert!(
        error.to_string().contains("maximum include depth"),
        "{error}"
    );
    files.insert("main.svelte".into(), "<span>Recovered</span>".into());
    assert!(render_from_files(&files, "main.svelte", &TemplateContext::new()).is_ok());
}
