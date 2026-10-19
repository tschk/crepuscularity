//! Owned include snapshots prepared off the render thread. No evaluated scope is cached.
use super::*;
use std::collections::HashMap;
use std::path::PathBuf;

pub(crate) struct PreparedIncludes {
    bundles: Vec<BundleCache>,
}

impl PreparedIncludes {
    pub(super) fn bundle(&self, ctx: &TemplateContext) -> Option<&BundleCache> {
        self.bundles
            .iter()
            .find(|bundle| Arc::ptr_eq(&bundle.virtual_files, &ctx.virtual_files))
    }
}

pub(crate) fn prepare_includes(nodes: &[Node], ctx: &TemplateContext) -> Arc<PreparedIncludes> {
    let mut cache = IncludeCache::default();
    let mut seen = HashMap::new();
    walk(nodes, ctx, 0, &mut cache, &mut seen);
    let mut slot_context = ctx;
    while let Some((nodes, slot_ctx)) = &slot_context.slot {
        walk(nodes, slot_ctx, 0, &mut cache, &mut seen);
        slot_context = slot_ctx;
    }
    Arc::new(PreparedIncludes {
        bundles: cache.bundles,
    })
}

type Seen = HashMap<(usize, PathBuf, Option<String>, Option<PathBuf>), usize>;

fn walk(
    nodes: &[Node],
    ctx: &TemplateContext,
    depth: usize,
    cache: &mut IncludeCache,
    seen: &mut Seen,
) {
    stacker::maybe_grow(512 * 1024, 2 * 1024 * 1024, || {
        walk_inner(nodes, ctx, depth, cache, seen)
    });
}

fn walk_inner(
    nodes: &[Node],
    ctx: &TemplateContext,
    depth: usize,
    cache: &mut IncludeCache,
    seen: &mut Seen,
) {
    for node in nodes {
        match node {
            Node::Element(el) => walk(&el.children, ctx, depth, cache, seen),
            Node::If(block) => {
                walk(&block.then_children, ctx, depth, cache, seen);
                if let Some(nodes) = &block.else_children {
                    walk(nodes, ctx, depth, cache, seen);
                }
            }
            Node::For(block) => walk(&block.body, ctx, depth, cache, seen),
            Node::Match(block) => {
                for arm in &block.arms {
                    walk(&arm.body, ctx, depth, cache, seen);
                }
            }
            Node::Include(inc) => {
                // Authored slots retain their caller's directory and virtual bundle.
                walk(&inc.slot, ctx, depth, cache, seen);
                if depth >= 64 {
                    continue;
                }
                let (literal, component) = inc
                    .path
                    .split_once('#')
                    .map_or((inc.path.as_str(), None), |(file, name)| (file, Some(name)));
                let path = match cache.resolve(ctx, literal) {
                    Ok(path) => path,
                    Err(error) => {
                        cache
                            .bundle(ctx)
                            .resolutions
                            .insert((ctx.base_dir.clone(), literal.to_owned()), Err(error));
                        continue;
                    }
                };
                let frontend = component.is_none().then(|| {
                    crepuscularity_core::include_paths::include_frontend_path(&path, literal)
                });
                let key = (
                    Arc::as_ptr(&ctx.virtual_files) as usize,
                    path.clone(),
                    component.map(str::to_owned),
                    frontend.clone(),
                );
                if seen.get(&key).is_some_and(|previous| *previous <= depth) {
                    continue;
                }
                seen.insert(key, depth);
                let child = include_context(ctx, &path);
                if let Some(name) = component {
                    match cache.components(ctx, &path) {
                        Ok(file) => {
                            if let Some(component) = file.components.get(name) {
                                walk(&component.nodes, &child, depth + 1, cache, seen);
                            }
                        }
                        Err(error) => {
                            cache.bundle(ctx).components.insert(path, Err(error));
                        }
                    }
                } else {
                    let frontend = frontend.expect("ordinary include frontend");
                    match cache.nodes_with_frontend(ctx, &path, &frontend) {
                        Ok(nodes) => walk(&nodes, &child, depth + 1, cache, seen),
                        Err(error) => {
                            cache.bundle(ctx).nodes.insert((path, frontend), Err(error));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crepuscularity_core::parse_template;

    fn frozen(cache: Arc<PreparedIncludes>) -> IncludeCache {
        IncludeCache {
            prepared: Some(cache),
            ..Default::default()
        }
    }

    #[test]
    fn prepared_sources_and_resolutions_survive_disk_deletion_and_refresh_explicitly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("child.crepus");
        std::fs::write(&path, "span \"before\"").unwrap();
        let mut ctx = TemplateContext::new();
        ctx.base_dir = Some(dir.path().into());
        let nodes = parse_template("include child.crepus").unwrap();
        let prepared = prepare_includes(&nodes, &ctx);
        let mut cache = frozen(Arc::clone(&prepared));
        let resolved = cache.resolve(&ctx, "child.crepus").unwrap();
        let before = cache.nodes(&ctx, &resolved).unwrap();
        std::fs::remove_file(&path).unwrap();
        let _ = render_nodes_prepared(&nodes, &ctx, Arc::clone(&prepared));
        let after_path = cache.resolve(&ctx, "child.crepus").unwrap();
        assert!(Arc::ptr_eq(
            &before,
            &cache.nodes(&ctx, &after_path).unwrap()
        ));
        assert!(frozen(prepare_includes(&nodes, &ctx))
            .nodes(&ctx, &resolved)
            .is_err());
        // Strict misses cannot read newly created files or resolve new paths.
        std::fs::write(dir.path().join("later.crepus"), "span").unwrap();
        assert!(cache.resolve(&ctx, "later.crepus").is_err());
        assert!(cache.nodes(&ctx, &dir.path().join("later.crepus")).is_err());
    }

    #[test]
    fn named_relative_and_external_slot_bundles_remain_distinct() {
        let mut ctx = TemplateContext::new();
        ctx.base_dir = Some(PathBuf::from("/prepared-root"));
        Arc::make_mut(&mut ctx.virtual_files).extend([
            (
                "named.crepus".into(),
                "--- Card\ninclude nested/child.svelte".into(),
            ),
            ("nested/child.svelte".into(), "<p>caller</p>".into()),
        ]);
        let mut slot = ctx.clone();
        Arc::make_mut(&mut slot.virtual_files)
            .insert("nested/child.svelte".into(), "<p>slot</p>".into());
        let slot_nodes = parse_template("include nested/child.svelte").unwrap();
        ctx.slot = Some((slot_nodes, Arc::new(slot.clone())));
        let nodes = parse_template("include named.crepus#Card\nslot").unwrap();
        let mut cache = frozen(prepare_includes(&nodes, &ctx));
        let path = cache.resolve(&ctx, "named.crepus").unwrap();
        assert!(cache
            .components(&ctx, &path)
            .unwrap()
            .components
            .contains_key("Card"));
        let child = cache.resolve(&slot, "nested/child.svelte").unwrap();
        let one = cache.nodes(&ctx, &child).unwrap();
        let two = cache.nodes(&slot, &child).unwrap();
        assert_ne!(one, two);
    }

    #[test]
    fn inactive_errors_are_cached_without_evaluating_conditions_and_cycles_terminate() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files)
            .insert("cycle.crepus".into(), "include cycle.crepus".into());
        let nodes =
            parse_template("if false\n  include missing.crepus\ninclude cycle.crepus").unwrap();
        let prepared = prepare_includes(&nodes, &ctx);
        let mut cache = frozen(prepared);
        let missing = cache.resolve(&ctx, "missing.crepus").unwrap();
        assert!(cache.nodes(&ctx, &missing).is_err());
        let cycle = cache.resolve(&ctx, "cycle.crepus").unwrap();
        assert!(cache.nodes(&ctx, &cycle).is_ok());
    }
    #[test]
    fn shallower_revisit_prepares_descendants_after_a_depth_limited_first_visit() {
        let mut ctx = TemplateContext::new();
        let files = Arc::make_mut(&mut ctx.virtual_files);
        for index in 0..63 {
            files.insert(
                format!("level{index}.crepus"),
                if index == 62 {
                    "include common.crepus".into()
                } else {
                    format!("include level{}.crepus", index + 1)
                },
            );
        }
        files.insert("common.crepus".into(), "include leaf.crepus".into());
        files.insert("leaf.crepus".into(), "span \"leaf\"".into());
        let nodes = parse_template("include level0.crepus\ninclude common.crepus").unwrap();
        let mut cache = frozen(prepare_includes(&nodes, &ctx));
        let common = cache.resolve(&ctx, "common.crepus").unwrap();
        let child_ctx = include_context(&ctx, &common);
        let leaf = cache.resolve(&child_ctx, "leaf.crepus").unwrap();
        assert!(cache.nodes(&child_ctx, &leaf).is_ok());
    }

    #[test]
    fn authored_slot_includes_keep_caller_relative_paths() {
        let mut ctx = TemplateContext::new();
        ctx.base_dir = Some(PathBuf::from("/prepared-slots"));
        Arc::make_mut(&mut ctx.virtual_files).extend([
            ("nested/wrapper.crepus".into(), "slot".into()),
            ("child.crepus".into(), "span \"caller child\"".into()),
        ]);
        let nodes =
            parse_template("include nested/wrapper.crepus\n  include child.crepus").unwrap();
        let mut cache = frozen(prepare_includes(&nodes, &ctx));
        let path = cache.resolve(&ctx, "child.crepus").unwrap();
        assert_eq!(path, PathBuf::from("/prepared-slots/child.crepus"));
        assert!(cache.nodes(&ctx, &path).is_ok());
    }
    #[test]
    fn external_slot_context_chain_is_not_limited_by_include_depth() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).insert("leaf.crepus".into(), "span".into());
        let leaf = ctx.clone();
        let mut nodes = parse_template("include leaf.crepus").unwrap();
        for _ in 0..70 {
            let mut outer = TemplateContext::new();
            outer.slot = Some((nodes, Arc::new(ctx)));
            ctx = outer;
            nodes = parse_template("slot").unwrap();
        }
        let mut cache = frozen(prepare_includes(&nodes, &ctx));
        let path = cache.resolve(&leaf, "leaf.crepus").unwrap();
        assert!(cache.nodes(&leaf, &path).is_ok());
    }
}

#[cfg(all(test, unix))]
mod frontend_identity_tests {
    use super::*;

    #[test]
    fn prepared_aliases_partition_frontends_and_preserve_canonical_base() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("source.txt");
        std::fs::write(
            &target,
            "<template><div v-if=\"false\">HIDDEN</div></template>",
        )
        .unwrap();
        for alias in ["child.vue", "child.jsx"] {
            std::os::unix::fs::symlink(&target, dir.path().join(alias)).unwrap();
        }
        let mut ctx = TemplateContext::new();
        ctx.base_dir = Some(dir.path().to_path_buf());
        let nodes =
            crepuscularity_core::parse_template("include child.vue\ninclude child.jsx").unwrap();
        let prepared = prepare_includes(&nodes, &ctx);
        for mut cache in [
            IncludeCache::default(),
            IncludeCache {
                prepared: Some(prepared),
                ..Default::default()
            },
        ] {
            let canonical = cache.resolve(&ctx, "child.vue").unwrap();
            assert_eq!(canonical, cache.resolve(&ctx, "child.jsx").unwrap());
            let vue = cache
                .nodes_with_frontend(
                    &ctx,
                    &canonical,
                    &crepuscularity_core::include_paths::include_frontend_path(
                        &canonical,
                        "child.vue",
                    ),
                )
                .unwrap();
            let jsx = cache
                .nodes_with_frontend(
                    &ctx,
                    &canonical,
                    &crepuscularity_core::include_paths::include_frontend_path(
                        &canonical,
                        "child.jsx",
                    ),
                )
                .unwrap();
            assert!(matches!(&vue[0], Node::If(_)));
            assert!(matches!(&jsx[0], Node::Element(element) if element.tag == "template"));
            assert!(!Arc::ptr_eq(&vue, &jsx));
            let child = include_context(&ctx, &canonical);
            assert_eq!(child.base_dir, canonical.parent().map(PathBuf::from));
        }
    }
}
