#![cfg(feature = "ssr")]

use crepuscularity_core::TemplateContext;

#[test]
fn quoted_include_paths_with_spaces_render() {
    let files = std::collections::HashMap::from([
        (
            "entry.crepus".to_string(),
            "include \"card view.crepus\" title=\"Space path\"".to_string(),
        ),
        (
            "card view.crepus".to_string(),
            "span \"{title}\"".to_string(),
        ),
    ]);
    let ctx = TemplateContext::new();
    for html in [
        crepuscularity_web::render_from_files(&files, "entry.crepus", &ctx),
        crepuscularity_web::render_from_files_with_ssr(&files, "entry.crepus", &ctx, true),
    ] {
        assert!(html.unwrap().contains("Space path"));
    }
}

#[test]
fn named_virtual_entries_preserve_directory_defaults_and_props() {
    let files = std::collections::HashMap::from([
        ("a/ui.crepus".to_string(), "+++\n[Card.defaults]\ntitle = 'Default title'\n+++\n--- Card\nspan \"{title}\"\ninclude child.crepus".to_string()),
        ("a/child.crepus".to_string(), "span \"Correct child\"".to_string()),
        ("b/child.crepus".to_string(), "span \"Wrong child\"".to_string()),
    ]);
    for explicit in [false, true] {
        let mut ctx = TemplateContext::new();
        if explicit {
            ctx.set("title", "Provided title");
        }
        for html in [
            crepuscularity_web::render_from_files(&files, "a/ui.crepus#Card", &ctx),
            crepuscularity_web::render_from_files_with_ssr(&files, "a/ui.crepus#Card", &ctx, false),
            crepuscularity_web::render_from_files_with_ssr(&files, "a/ui.crepus#Card", &ctx, true),
        ] {
            let html = html.unwrap();
            assert!(html.contains(if explicit {
                "Provided title"
            } else {
                "Default title"
            }));
            assert!(html.contains("Correct child"));
            assert!(!html.contains("Wrong child"));
        }
    }
}

#[test]
fn virtual_entries_resolve_includes_from_their_own_directory() {
    let files = std::collections::HashMap::from([
        (
            "a/entry.crepus".to_string(),
            "include child.crepus".to_string(),
        ),
        (
            "a/child.crepus".to_string(),
            "span \"Correct child\"".to_string(),
        ),
        (
            "b/child.crepus".to_string(),
            "span \"Wrong child\"".to_string(),
        ),
    ]);
    let ctx = TemplateContext::new();
    for html in [
        crepuscularity_web::render_from_files(&files, "a/entry.crepus", &ctx),
        crepuscularity_web::render_from_files_with_ssr(&files, "a/entry.crepus", &ctx, false),
        crepuscularity_web::render_from_files_with_ssr(&files, "a/entry.crepus", &ctx, true),
    ] {
        let html = html.unwrap();
        assert!(html.contains("Correct child"));
        assert!(!html.contains("Wrong child"));
    }
}

#[test]
fn virtual_vue_entry_uses_filename_with_and_without_markers() {
    let files = std::collections::HashMap::from([(
        "view.vue".to_string(),
        "<script>const ignored = 1 < 2;</script><template><div>Entry Vue</div></template>"
            .to_string(),
    )]);
    for markers in [false, true] {
        let html = crepuscularity_web::render_from_files_with_ssr(
            &files,
            "view.vue",
            &TemplateContext::new(),
            markers,
        )
        .unwrap();
        assert!(html.contains("Entry Vue"));
        assert!(!html.contains("const ignored"));
    }
}

#[test]
fn hydration_ssr_includes_select_vue_frontend() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("child.vue"),
        "<script>const ignored = 1 < 2;</script><template><div>Included Vue</div></template>",
    )
    .unwrap();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(dir.path().to_path_buf());
    let html =
        crepuscularity_web::render_template_to_html_with_ssr("include child.vue", &ctx, true)
            .unwrap();
    assert!(html.contains("Included Vue"), "{html}");
}

#[cfg(unix)]
#[test]
fn symlinked_include_selects_the_frontend_from_its_logical_name() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("payload.crepus"),
        "<template><p v-if=\"false\">HIDDEN</p><p>VISIBLE</p></template>",
    )
    .unwrap();
    std::os::unix::fs::symlink("payload.crepus", directory.path().join("child.vue")).unwrap();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(directory.path().to_path_buf());
    for result in [
        crepuscularity_web::render_template_to_html("include child.vue", &ctx),
        crepuscularity_web::render_template_to_html_with_ssr("include child.vue", &ctx, false),
        crepuscularity_web::render_template_to_html_with_ssr("include child.vue", &ctx, true),
    ] {
        let html = result.unwrap();
        assert!(html.contains("VISIBLE"), "{html}");
        assert!(!html.contains("HIDDEN"), "{html}");
    }
}

#[cfg(unix)]
#[test]
fn physical_include_aliases_keep_distinct_frontends_in_both_orders() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("payload.crepus"),
        "<template><p v-if=\"false\">HIDDEN</p><p>VISIBLE</p></template>",
    )
    .unwrap();
    for alias in ["child.vue", "child.jsx"] {
        std::os::unix::fs::symlink("payload.crepus", directory.path().join(alias)).unwrap();
    }
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(directory.path().to_path_buf());
    for source in [
        "include child.vue\ninclude child.jsx",
        "include child.jsx\ninclude child.vue",
    ] {
        for result in [
            crepuscularity_web::render_template_to_html(source, &ctx),
            crepuscularity_web::render_template_to_html_with_ssr(source, &ctx, false),
            crepuscularity_web::render_template_to_html_with_ssr(source, &ctx, true),
        ] {
            let html = result.unwrap();
            assert_eq!(html.matches("HIDDEN").count(), 1, "{source}: {html}");
            assert_eq!(html.matches("VISIBLE").count(), 2, "{source}: {html}");
        }
    }
}
