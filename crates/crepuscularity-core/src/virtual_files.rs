//! Lookup helpers for in-memory `TemplateContext::virtual_files` bundles.

use std::path::Path;

use crate::context::TemplateContext;

/// Find template source in `virtual_files` for a resolved filesystem path.
///
/// Tries, in order: exact path key, longest matching directory suffix,
/// basename-only key, then a single unambiguous
/// virtual key whose path suffix matches the requested file name.
pub fn lookup_virtual_file(ctx: &TemplateContext, path: &Path) -> Option<String> {
    lookup_virtual_file_entry(ctx, path).map(|(_, content)| content.to_owned())
}

/// Find the selected virtual bundle key and source without cloning either.
/// Resolution order and ambiguity handling match [`lookup_virtual_file`].
pub fn lookup_virtual_file_entry<'a>(
    ctx: &'a TemplateContext,
    path: &Path,
) -> Option<(&'a str, &'a str)> {
    let key = path
        .to_str()
        .map(std::borrow::Cow::Borrowed)
        .unwrap_or_else(|| path.to_string_lossy());
    if let Some((key, content)) = ctx.virtual_files.get_key_value(key.as_ref()) {
        return Some((key.as_str(), content.as_str()));
    }

    let mut best = None;
    let mut best_depth = 1;
    let mut ambiguous = false;
    for (virtual_key, content) in ctx.virtual_files.iter() {
        let virtual_path = Path::new(virtual_key);
        let depth = virtual_path.components().count();
        if depth > 1 && path.ends_with(virtual_path) {
            if depth > best_depth {
                best = Some((virtual_key.as_str(), content.as_str()));
                best_depth = depth;
                ambiguous = false;
            } else if depth == best_depth {
                ambiguous = true;
            }
        }
    }
    if best.is_some() {
        return if ambiguous { None } else { best };
    }

    let file_name = path.file_name()?.to_str()?;
    if let Some((key, content)) = ctx.virtual_files.get_key_value(file_name) {
        return Some((key.as_str(), content.as_str()));
    }

    let file_name_bytes = file_name.as_bytes();
    let file_name_len = file_name_bytes.len();

    let mut matches = ctx.virtual_files.iter().filter(|(vkey, _)| {
        let vbytes = vkey.as_bytes();
        let vlen = vbytes.len();
        if vlen == file_name_len {
            vbytes == file_name_bytes
        } else if vlen > file_name_len {
            vbytes[vlen - file_name_len - 1] == b'/'
                && &vbytes[vlen - file_name_len..] == file_name_bytes
        } else {
            false
        }
    });

    let first = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some((first.0.as_str(), first.1.as_str()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use crate::context::TemplateContext;
    use crate::virtual_files::{lookup_virtual_file, lookup_virtual_file_entry};

    #[test]
    fn resolved_paths_prefer_matching_virtual_directories() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).extend([
            ("a/child.crepus".into(), "correct".into()),
            ("b/child.crepus".into(), "other".into()),
            ("child.crepus".into(), "basename".into()),
        ]);
        assert_eq!(
            lookup_virtual_file(&ctx, Path::new("/bundle/a/child.crepus")).as_deref(),
            Some("correct")
        );
        assert_eq!(
            lookup_virtual_file(&ctx, Path::new("/bundle/not-a/child.crepus")).as_deref(),
            Some("basename")
        );
    }

    #[test]
    fn directory_suffix_uses_longest_match_and_rejects_equal_matches() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).extend([
            ("a/child.crepus".into(), "short".into()),
            ("bundle/a/child.crepus".into(), "long".into()),
        ]);
        let requested = Path::new("/root/bundle/a/child.crepus");
        assert_eq!(
            lookup_virtual_file(&ctx, requested).as_deref(),
            Some("long")
        );
        Arc::make_mut(&mut ctx.virtual_files)
            .insert("bundle//a/child.crepus".into(), "ambiguous".into());
        assert!(lookup_virtual_file(&ctx, requested).is_none());
    }

    #[test]
    fn lookup_exact_key() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).insert("child.crepus".into(), "ok".into());
        assert_eq!(
            lookup_virtual_file(&ctx, Path::new("child.crepus")).as_deref(),
            Some("ok")
        );
    }

    #[test]
    fn lookup_rejects_ambiguous_suffix() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).insert("a/child.crepus".into(), "1".into());
        Arc::make_mut(&mut ctx.virtual_files).insert("b/child.crepus".into(), "2".into());
        assert!(lookup_virtual_file(&ctx, Path::new("/virtual/child.crepus")).is_none());
    }

    #[test]
    fn lookup_exact_key_full_path() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).insert("/a/b/c.crepus".into(), "exact".into());
        assert_eq!(
            lookup_virtual_file(&ctx, Path::new("/a/b/c.crepus")).as_deref(),
            Some("exact")
        );
    }

    #[test]
    fn lookup_basename() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).insert("child.crepus".into(), "base".into());
        assert_eq!(
            lookup_virtual_file(&ctx, Path::new("/some/path/child.crepus")).as_deref(),
            Some("base")
        );
    }

    #[test]
    fn lookup_unambiguous_suffix() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).insert("dir/child.crepus".into(), "suffix".into());
        assert_eq!(
            lookup_virtual_file(&ctx, Path::new("/request/path/child.crepus")).as_deref(),
            Some("suffix")
        );
    }

    #[test]
    fn lookup_invalid_filename() {
        let ctx = TemplateContext::new();
        assert!(lookup_virtual_file(&ctx, Path::new("/")).is_none());
        assert!(lookup_virtual_file(&ctx, Path::new("..")).is_none());
        assert!(lookup_virtual_file(&ctx, Path::new("/a/b/..")).is_none());
    }

    #[test]
    fn lookup_not_found() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).insert("other.crepus".into(), "other".into());
        assert!(lookup_virtual_file(&ctx, Path::new("child.crepus")).is_none());
    }

    #[test]
    fn lookup_exact_key_takes_precedence_over_basename() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files)
            .insert("/some/path/child.crepus".into(), "exact".into());
        Arc::make_mut(&mut ctx.virtual_files).insert("child.crepus".into(), "base".into());
        assert_eq!(
            lookup_virtual_file(&ctx, Path::new("/some/path/child.crepus")).as_deref(),
            Some("exact")
        );
    }

    #[test]
    fn lookup_basename_takes_precedence_over_suffix() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).insert("child.crepus".into(), "base".into());
        Arc::make_mut(&mut ctx.virtual_files).insert("a/child.crepus".into(), "suffix".into());
        assert_eq!(
            lookup_virtual_file(&ctx, Path::new("/other/path/child.crepus")).as_deref(),
            Some("base")
        );
    }

    #[test]
    fn borrowed_lookup_reports_the_actual_key_for_each_resolution_tier() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).extend([
            ("/exact/bundle/a/child.crepus".into(), "exact".into()),
            ("a/child.crepus".into(), "short".into()),
            ("bundle/a/child.crepus".into(), "long".into()),
            ("child.crepus".into(), "basename".into()),
            ("parts/unique.crepus".into(), "unique".into()),
        ]);
        for (requested, key, content) in [
            (
                "/exact/bundle/a/child.crepus",
                "/exact/bundle/a/child.crepus",
                "exact",
            ),
            (
                "/other/bundle/a/child.crepus",
                "bundle/a/child.crepus",
                "long",
            ),
            ("/other/child.crepus", "child.crepus", "basename"),
            ("/other/unique.crepus", "parts/unique.crepus", "unique"),
        ] {
            assert_eq!(
                lookup_virtual_file_entry(&ctx, Path::new(requested)),
                Some((key, content))
            );
            assert_eq!(
                lookup_virtual_file(&ctx, Path::new(requested)).as_deref(),
                Some(content)
            );
        }
    }

    #[test]
    fn borrowed_lookup_preserves_directory_and_filename_ambiguity() {
        let mut ctx = TemplateContext::new();
        Arc::make_mut(&mut ctx.virtual_files).extend([
            ("a/child.crepus".into(), "one".into()),
            ("a//child.crepus".into(), "two".into()),
            (
                "child.crepus".into(),
                "basename cannot override ambiguous directory".into(),
            ),
            ("a/unique.crepus".into(), "one".into()),
            ("b/unique.crepus".into(), "two".into()),
        ]);
        for requested in ["/root/a/child.crepus", "/root/unique.crepus"] {
            assert_eq!(lookup_virtual_file_entry(&ctx, Path::new(requested)), None);
            assert_eq!(lookup_virtual_file(&ctx, Path::new(requested)), None);
        }
    }
}
