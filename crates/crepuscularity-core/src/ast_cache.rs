use std::cell::RefCell;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::ast::Node;
use crate::error::CrepusError;
use crate::parser::parse_template_with_path;

thread_local! {
    static CONTENT_CACHE: RefCell<VecDeque<ContentCacheEntry>> = const { RefCell::new(VecDeque::new()) };
}

const MAX_CONTENT_CACHE_ENTRIES: usize = 64;
const MAX_CONTENT_CACHE_KEY_BYTES: usize = 256 * 1024;

struct ContentCacheEntry {
    hash: u64,
    source: String,
    path: Option<PathBuf>,
    nodes: Arc<Vec<Node>>,
}

fn content_cache_key(content: &str, path: Option<&Path>) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    path.hash(&mut hasher);
    hasher.finish()
}

/// Read a template file and return a shared, content-validated AST snapshot.
///
/// Uses the bounded content/path cache below; filesystem metadata alone cannot
/// detect equal-length edits with restored timestamps. Each call reads the file.
pub fn parse_file(path: &Path) -> Result<Arc<Vec<Node>>, CrepusError> {
    let cache_path = std::path::absolute(path)?;
    let content = std::fs::read_to_string(&cache_path).map_err(|e| {
        CrepusError::Io(std::io::Error::new(
            e.kind(),
            format!("read {:?}: {e}", path),
        ))
    })?;
    parse_content_with_path(&content, Some(&cache_path))
}

/// Parse in-memory template content, returning a shared AST snapshot.
pub fn parse_content(content: &str) -> Result<Arc<Vec<Node>>, CrepusError> {
    parse_content_with_path(content, None)
}

/// Cache template content while preserving filename-based frontend selection.
///
/// Retains up to 64 recently used snapshots per thread. Source/path keys over
/// 256 KiB bypass caching; snapshots held by callers remain valid after eviction.
pub fn parse_content_with_path(
    content: &str,
    path: Option<&Path>,
) -> Result<Arc<Vec<Node>>, CrepusError> {
    let key_bytes = content
        .len()
        .saturating_add(path.map_or(0, |path| path.as_os_str().len()));
    if key_bytes > MAX_CONTENT_CACHE_KEY_BYTES {
        return parse_template_with_path(content, path).map(Arc::new);
    }
    let key = content_cache_key(content, path);
    let cached = CONTENT_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let index = cache.iter().position(|entry| {
            entry.hash == key && entry.source == content && entry.path.as_deref() == path
        })?;
        let entry = cache.remove(index).expect("matched content cache entry");
        let nodes = Arc::clone(&entry.nodes);
        cache.push_back(entry);
        Some(nodes)
    });
    if let Some(nodes) = cached {
        return Ok(nodes);
    }

    let nodes = Arc::new(parse_template_with_path(content, path)?);
    CONTENT_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= MAX_CONTENT_CACHE_ENTRIES {
            cache.pop_front();
        }
        cache.push_back(ContentCacheEntry {
            hash: key,
            source: content.to_string(),
            path: path.map(Path::to_path_buf),
            nodes: Arc::clone(&nodes),
        });
    });

    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn content_cache_separates_frontends() {
        let source = "<template><div /></template>";
        let vue = parse_content_with_path(source, Some(Path::new("view.vue"))).unwrap();
        let jsx = parse_content_with_path(source, Some(Path::new("view.jsx"))).unwrap();
        assert!(!Arc::ptr_eq(&vue, &jsx));
        assert!(matches!(&vue[0], Node::Element(element) if element.tag == "div"));
        assert!(matches!(&jsx[0], Node::Element(element) if element.tag == "template"));
        let cached = parse_content_with_path(source, Some(Path::new("view.vue"))).unwrap();
        assert!(Arc::ptr_eq(&vue, &cached));
    }
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn file_cache_uses_absolute_identity_for_relative_paths() {
        let cwd = std::env::current_dir().unwrap();
        let dir = tempfile::tempdir_in(&cwd).unwrap();
        let path = dir.path().join("view.crepus");
        std::fs::write(&path, "span \"Shared\"").unwrap();
        let absolute = parse_file(&path).unwrap();
        let relative = parse_file(path.strip_prefix(&cwd).unwrap()).unwrap();
        assert!(Arc::ptr_eq(&absolute, &relative));
    }

    #[cfg(unix)]
    #[test]
    fn file_cache_preserves_symlink_frontend_names() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        std::fs::write(&source, "<template><div /></template>").unwrap();
        let vue_path = dir.path().join("view.vue");
        let jsx_path = dir.path().join("view.jsx");
        std::os::unix::fs::symlink(&source, &vue_path).unwrap();
        std::os::unix::fs::symlink(&source, &jsx_path).unwrap();
        let vue = parse_file(&vue_path).unwrap();
        let jsx = parse_file(&jsx_path).unwrap();
        assert!(matches!(&vue[0], Node::Element(element) if element.tag == "div"));
        assert!(matches!(&jsx[0], Node::Element(element) if element.tag == "template"));
        assert!(!Arc::ptr_eq(&vue, &jsx));
    }

    #[test]
    fn test_parse_content_cache() {
        let content1 = "div\n  span Hello";
        let content2 = "div\n  span World";

        let ast1 = parse_content(content1).unwrap();
        let ast1_cached = parse_content(content1).unwrap();
        assert!(
            Arc::ptr_eq(&ast1, &ast1_cached),
            "Cache hit should return the same Arc"
        );

        let ast2 = parse_content(content2).unwrap();
        assert!(
            !Arc::ptr_eq(&ast1, &ast2),
            "Different content should have different Arc"
        );
    }

    #[test]
    fn test_parse_file_cache() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "div\n  span File").unwrap();
        file.flush().unwrap();

        let path = file.path().to_path_buf();

        let ast1 = parse_file(&path).unwrap();
        let ast1_cached = parse_file(&path).unwrap();
        assert!(
            Arc::ptr_eq(&ast1, &ast1_cached),
            "Cache hit should return the same Arc"
        );

        // Wait a bit and modify file to change mtime and length
        std::thread::sleep(std::time::Duration::from_millis(10));
        write!(file, " Extra content").unwrap();
        file.flush().unwrap();

        let ast2 = parse_file(&path).unwrap();
        assert!(
            !Arc::ptr_eq(&ast1, &ast2),
            "Cache should miss after file modification"
        );
    }
}

#[cfg(test)]
mod bounded_content_tests {
    use super::*;

    fn isolated(test: impl FnOnce() + Send + 'static) {
        std::thread::spawn(test).join().unwrap();
    }

    #[test]
    fn lru_is_bounded_promotes_hits_and_keeps_external_snapshots_valid() {
        isolated(|| {
            let first = parse_content("span \"first\"").unwrap();
            let oldest = parse_content("span \"oldest\"").unwrap();
            for index in 2..MAX_CONTENT_CACHE_ENTRIES {
                parse_content(&format!("span \"entry{index}\"")).unwrap();
            }
            assert!(Arc::ptr_eq(
                &first,
                &parse_content("span \"first\"").unwrap()
            ));
            parse_content("span \"newest\"").unwrap();
            CONTENT_CACHE.with(|cache| {
                let cache = cache.borrow();
                assert_eq!(cache.len(), MAX_CONTENT_CACHE_ENTRIES);
                assert!(!cache.iter().any(|entry| entry.source == "span \"oldest\""));
            });
            assert!(Arc::ptr_eq(
                &first,
                &parse_content("span \"first\"").unwrap()
            ));
            assert!(!Arc::ptr_eq(
                &oldest,
                &parse_content("span \"oldest\"").unwrap()
            ));
            assert!(matches!(&oldest[0], Node::Element(element) if element.tag == "span"));
        });
    }

    #[test]
    fn malformed_and_oversized_keys_are_not_admitted() {
        isolated(|| {
            let kept = parse_content("span \"kept\"").unwrap();
            assert!(parse_content("<div>").is_err());
            CONTENT_CACHE.with(|cache| assert_eq!(cache.borrow().len(), 1));
            let oversized = format!("span \"{}\"", "x".repeat(MAX_CONTENT_CACHE_KEY_BYTES));
            let one = parse_content(&oversized).unwrap();
            let two = parse_content(&oversized).unwrap();
            assert!(!Arc::ptr_eq(&one, &two));
            let long_path = PathBuf::from("p".repeat(MAX_CONTENT_CACHE_KEY_BYTES));
            assert!(parse_content_with_path("span", Some(&long_path)).is_ok());
            CONTENT_CACHE.with(|cache| assert_eq!(cache.borrow().len(), 1));
            assert!(Arc::ptr_eq(&kept, &parse_content("span \"kept\"").unwrap()));
        });
    }

    #[test]
    fn hash_collisions_still_require_exact_source_and_path() {
        isolated(|| {
            let source = "<template><div /></template>";
            let path = Path::new("view.vue");
            let wrong = Arc::new(parse_template_with_path("span \"wrong\"", None).unwrap());
            CONTENT_CACHE.with(|cache| {
                cache.borrow_mut().extend([
                    ContentCacheEntry {
                        hash: content_cache_key(source, Some(path)),
                        source: "span \"wrong\"".into(),
                        path: Some(path.to_path_buf()),
                        nodes: Arc::clone(&wrong),
                    },
                    ContentCacheEntry {
                        hash: content_cache_key(source, Some(path)),
                        source: source.into(),
                        path: Some(PathBuf::from("view.jsx")),
                        nodes: Arc::clone(&wrong),
                    },
                ]);
            });
            let actual = parse_content_with_path(source, Some(path)).unwrap();
            assert!(!Arc::ptr_eq(&wrong, &actual));
            assert!(matches!(&actual[0], Node::Element(element) if element.tag == "div"));
            assert!(Arc::ptr_eq(
                &actual,
                &parse_content_with_path(source, Some(path)).unwrap()
            ));
            let no_path = parse_content("span").unwrap();
            let with_path =
                parse_content_with_path("span", Some(Path::new("view.crepus"))).unwrap();
            assert!(!Arc::ptr_eq(&no_path, &with_path));
            let changed =
                parse_content_with_path("span \"changed\"", Some(Path::new("view.crepus")))
                    .unwrap();
            assert!(!Arc::ptr_eq(&with_path, &changed));
        });
    }
}
