use super::*;

fn include(source: &str) -> IncludeNode {
    let mut nodes = parse_template(source).unwrap();
    let Node::Include(include) = nodes.remove(0) else {
        panic!("expected include directive");
    };
    include
}

#[test]
fn repeated_ordinary_expansion_shares_ast_but_evaluates_props_each_time() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("child.crepus");
    std::fs::write(&path, "span \"{title}\"").unwrap();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(dir.path().into());
    let directive = include("include child.crepus title={current}");
    let mut cache = IncludeCache::default();
    ctx.set("current", "FIRST");
    let (first, first_ctx) = expand_include(&directive, &ctx, 0, &mut cache).unwrap();
    // All source reads for this file are already complete for this render.
    std::fs::remove_file(&path).unwrap();
    ctx.set("current", "SECOND");
    let (second, second_ctx) = expand_include(&directive, &ctx, 0, &mut cache).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first_ctx.get_str("title"), "FIRST");
    assert_eq!(second_ctx.get_str("title"), "SECOND");
    assert!(expand_include(&directive, &ctx, 0, &mut IncludeCache::default()).is_err());
    assert!(
        !first.is_empty(),
        "retained AST remains valid after the file disappears"
    );
}

#[test]
fn named_file_parses_all_components_once_and_reapplies_frontmatter_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cards.crepus");
    std::fs::write(&path, "+++\n[Card.defaults]\ntitle = 'DEFAULT'\n[Other.defaults]\ntitle = 'OTHER-DEFAULT'\n+++\n--- Card\nspan \"{title}\"\n--- Other\nspan \"{title}\"").unwrap();
    let mut ctx = TemplateContext::new();
    ctx.base_dir = Some(dir.path().into());
    let mut cache = IncludeCache::default();
    let directive = include("include cards.crepus#Card title={current}");
    ctx.set("current", "FIRST");
    let (first, first_ctx) = expand_include(&directive, &ctx, 0, &mut cache).unwrap();
    // The first named lookup already parsed every definition in the file.
    let components = cache.bundles[0]
        .files
        .values()
        .next()
        .unwrap()
        .components
        .as_ref()
        .unwrap();
    assert_eq!(components.len(), 2);
    let other_ast = Arc::clone(&components["Other"].nodes);
    std::fs::remove_file(&path).unwrap();
    ctx.set("current", "SECOND");
    let (second, second_ctx) = expand_include(&directive, &ctx, 0, &mut cache).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first_ctx.get_str("title"), "FIRST");
    assert_eq!(second_ctx.get_str("title"), "SECOND");
    let (default_ast, default_ctx) =
        expand_include(&include("include cards.crepus#Card"), &ctx, 0, &mut cache).unwrap();
    assert!(Arc::ptr_eq(&first, &default_ast));
    assert_eq!(default_ctx.get_str("title"), "DEFAULT");
    let other_directive = include("include cards.crepus#Other");
    let (other, other_ctx) = expand_include(&other_directive, &ctx, 0, &mut cache).unwrap();
    assert!(Arc::ptr_eq(&other_ast, &other));
    assert_eq!(other_ctx.get_str("title"), "OTHER-DEFAULT");
    assert!(expand_include(&other_directive, &ctx, 0, &mut IncludeCache::default()).is_err());
}

#[test]
fn repeated_virtual_expansions_share_ordinary_and_named_asts() {
    let mut ctx = TemplateContext::new();
    Arc::make_mut(&mut ctx.virtual_files)
        .insert("ordinary.crepus".into(), "span \"ordinary\"".into());
    Arc::make_mut(&mut ctx.virtual_files)
        .insert("named.crepus".into(), "--- Card\nspan \"named\"".into());
    let mut cache = IncludeCache::default();
    for source in ["include ordinary.crepus", "include named.crepus#Card"] {
        let directive = include(source);
        let (first, _) = expand_include(&directive, &ctx, 0, &mut cache).unwrap();
        let (second, _) = expand_include(&directive, &ctx, 0, &mut cache).unwrap();
        assert!(Arc::ptr_eq(&first, &second), "{source}");
        let (fresh, _) = expand_include(&directive, &ctx, 0, &mut IncludeCache::default()).unwrap();
        assert!(!Arc::ptr_eq(&first, &fresh), "cache lifetime is one render");
    }
}
