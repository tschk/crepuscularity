use std::path::Path;
use std::sync::Arc;

use crepuscularity_core::ast_cache::parse_file;

fn isolated(test: impl FnOnce() + Send + 'static) {
    std::thread::spawn(test).join().unwrap();
}

fn write(path: &Path, source: &str) {
    std::fs::write(path, source).unwrap();
}

#[test]
fn distinct_deleted_files_retain_only_the_recent_64_snapshots() {
    isolated(|| {
        let dir = tempfile::tempdir().unwrap();
        let mut snapshots = Vec::new();
        for index in 0..256 {
            let path = dir.path().join(format!("view{index}.crepus"));
            write(&path, "span \"cached\"");
            let nodes = parse_file(&path).unwrap();
            snapshots.push(Arc::downgrade(&nodes));
            drop(nodes);
            std::fs::remove_file(path).unwrap();
        }
        assert_eq!(
            snapshots
                .iter()
                .filter(|weak| weak.upgrade().is_some())
                .count(),
            64
        );
        assert!(snapshots[..192].iter().all(|weak| weak.upgrade().is_none()));
        assert!(snapshots[192..].iter().all(|weak| weak.upgrade().is_some()));
    });
}

#[test]
fn hits_promote_and_eviction_preserves_caller_owned_snapshots() {
    isolated(|| {
        let dir = tempfile::tempdir().unwrap();
        let paths = (0..65)
            .map(|index| dir.path().join(format!("view{index}.crepus")))
            .collect::<Vec<_>>();
        let mut snapshots = Vec::new();
        let mut held = None;
        for (index, path) in paths.iter().take(64).enumerate() {
            write(path, "span \"kept\"");
            let nodes = parse_file(path).unwrap();
            snapshots.push(Arc::downgrade(&nodes));
            if index == 1 {
                held = Some(nodes);
            }
        }
        let first = snapshots[0].upgrade().unwrap();
        assert!(Arc::ptr_eq(&first, &parse_file(&paths[0]).unwrap()));
        write(&paths[64], "span \"new\"");
        parse_file(&paths[64]).unwrap();
        let held = held.unwrap();
        assert!(!Arc::ptr_eq(&held, &parse_file(&paths[1]).unwrap()));
        assert_eq!(
            *held,
            crepuscularity_core::parse_template("span \"kept\"").unwrap()
        );
        assert!(snapshots[2].upgrade().is_none());
        assert!(Arc::ptr_eq(&first, &parse_file(&paths[0]).unwrap()));
    });
}

#[test]
fn oversized_sources_bypass_retention() {
    isolated(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.crepus");
        write(&path, &format!("span \"{}\"", "x".repeat(256 * 1024)));
        let first = parse_file(&path).unwrap();
        let weak = Arc::downgrade(&first);
        let second = parse_file(&path).unwrap();
        assert!(!Arc::ptr_eq(&first, &second));
        drop(first);
        assert!(weak.upgrade().is_none());
    });
}

#[test]
fn changed_sources_remain_bounded_and_errors_are_not_admitted() {
    isolated(|| {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("changing.crepus");
        let mut snapshots = Vec::new();
        for index in 0..128 {
            // Changing length guarantees distinct metadata without relying on clock resolution.
            write(&path, &format!("span \"{}\"", "x".repeat(index)));
            let current = parse_file(&path).unwrap();
            assert_eq!(
                *current,
                crepuscularity_core::parse_template(&std::fs::read_to_string(&path).unwrap())
                    .unwrap()
            );
            snapshots.push(Arc::downgrade(&current));
        }
        assert_eq!(
            snapshots
                .iter()
                .filter(|weak| weak.upgrade().is_some())
                .count(),
            64
        );
        let invalid = dir.path().join("invalid.crepus");
        write(&invalid, "<x>");
        let modified = std::fs::metadata(&invalid).unwrap().modified().unwrap();
        assert!(parse_file(&invalid).is_err());
        assert!(parse_file(&invalid).is_err());
        write(&invalid, "div");
        std::fs::File::options()
            .write(true)
            .open(&invalid)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_eq!(
            *parse_file(&invalid).unwrap(),
            crepuscularity_core::parse_template("div").unwrap()
        );
    });
}

#[test]
fn same_length_restored_mtime_edits_and_atomic_replacements_read_fresh_source() {
    isolated(|| {
        let dir = tempfile::tempdir().unwrap();
        for replace in [false, true] {
            let path = dir.path().join(if replace {
                "replace.crepus"
            } else {
                "edit.crepus"
            });
            write(&path, "span \"old\"");
            let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
            let previous = parse_file(&path).unwrap();
            let updated = if replace {
                dir.path().join("replacement.crepus")
            } else {
                path.clone()
            };
            write(&updated, "span \"new\"");
            std::fs::File::options()
                .write(true)
                .open(&updated)
                .unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(modified))
                .unwrap();
            if replace {
                std::fs::rename(updated, &path).unwrap();
            }
            let current = parse_file(&path).unwrap();
            assert!(!Arc::ptr_eq(&previous, &current));
            assert_eq!(
                *current,
                crepuscularity_core::parse_template("span \"new\"").unwrap()
            );
            assert_eq!(
                *previous,
                crepuscularity_core::parse_template("span \"old\"").unwrap()
            );
            assert!(Arc::ptr_eq(&current, &parse_file(&path).unwrap()));
            std::fs::remove_file(&path).unwrap();
            assert!(
                parse_file(&path).is_err(),
                "cached source must not hide read errors"
            );
            std::fs::write(&path, b"span \"\xff\"").unwrap();
            assert!(matches!(
                parse_file(&path),
                Err(crepuscularity_core::CrepusError::Io(error))
                    if error.kind() == std::io::ErrorKind::InvalidData
            ));
            write(&path, "span \"repaired\"");
            assert_eq!(
                *parse_file(&path).unwrap(),
                crepuscularity_core::parse_template("span \"repaired\"").unwrap()
            );
        }
    });
}
