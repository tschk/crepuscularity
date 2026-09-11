//! Lookup helpers for in-memory `TemplateContext::virtual_files` bundles.

use std::path::Path;

use crate::context::TemplateContext;

/// Find template source in `virtual_files` for a resolved filesystem path.
///
/// Tries, in order: exact path key, basename-only key, then a single unambiguous
/// virtual key whose path suffix matches the requested file name.
pub fn lookup_virtual_file(ctx: &TemplateContext, path: &Path) -> Option<String> {
    if let Some(key_str) = path.to_str() {
        if let Some(content) = ctx.virtual_files.get(key_str) {
            return Some(content.clone());
        }
    } else {
        let key = path.to_string_lossy();
        if let Some(content) = ctx.virtual_files.get(key.as_ref()) {
            return Some(content.clone());
        }
    }

    let file_name = path.file_name()?.to_str()?;
    if let Some(content) = ctx.virtual_files.get(file_name) {
        return Some(content.clone());
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
    Some(first.1.clone())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use crate::context::TemplateContext;
    use crate::virtual_files::lookup_virtual_file;

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
}
