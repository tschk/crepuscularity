use crepuscularity_core::context::TemplateContext;
use crepuscularity_web::render_from_files;
use std::collections::HashMap;

#[test]
fn test_render_from_files_plain() {
    let mut files = HashMap::new();
    files.insert(
        "page.crepus".to_string(),
        "div\n  \"Hello Page\"".to_string(),
    );

    let ctx = TemplateContext::new();
    let res = render_from_files(&files, "page.crepus", &ctx).unwrap();
    assert!(res.contains("<div>Hello Page</div>"));
}

#[test]
fn test_render_from_files_with_component_hash() {
    let mut files = HashMap::new();
    files.insert(
        "comp.crepus".to_string(),
        "--- App\ndiv\n  \"Hello Comp\"\n".to_string(),
    );

    let ctx = TemplateContext::new();
    let res = render_from_files(&files, "comp.crepus#App", &ctx).unwrap();
    assert!(res.contains("<div>Hello Comp</div>"));
}

#[test]
fn test_render_from_files_file_not_found() {
    let files = HashMap::new();
    let ctx = TemplateContext::new();

    let res = render_from_files(&files, "missing.crepus", &ctx);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.to_string().contains("file not found in virtual fs"));
}

#[test]
fn test_render_from_files_with_component_hash_file_not_found() {
    let files = HashMap::new();
    let ctx = TemplateContext::new();

    let res = render_from_files(&files, "missing.crepus#App", &ctx);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(err.to_string().contains("file not found in virtual fs"));
}

#[cfg(feature = "parallel")]
#[test]
fn test_par_render_from_files() {
    use crepuscularity_web::par_render_from_files;
    let mut files = HashMap::new();
    files.insert("page1.crepus".to_string(), "div\n  \"P1\"".to_string());
    files.insert("page2.crepus".to_string(), "div\n  \"P2\"".to_string());

    let ctx = TemplateContext::new();
    let entries = vec!["page1.crepus", "page2.crepus"];

    let res = par_render_from_files(&files, &entries, &ctx);
    assert_eq!(res.len(), 2);

    let res_map: HashMap<_, _> = res.into_iter().collect();
    assert!(res_map
        .get("page1.crepus")
        .unwrap()
        .as_ref()
        .unwrap()
        .contains("P1"));
    assert!(res_map
        .get("page2.crepus")
        .unwrap()
        .as_ref()
        .unwrap()
        .contains("P2"));
}
