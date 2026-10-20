//! Stage one generated browser output without modifying the previous build.
// shortcut: abrupt termination can leave stages; add owner-aware recovery if interrupted builds accumulate.
use std::path::{Path, PathBuf};

pub(super) struct OutputDirectory {
    stage: Option<tempfile::TempDir>,
    destination: PathBuf,
    dist: PathBuf,
}

fn directory_or_missing(path: &Path) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(format!(
            "output path must be a directory, not a symlink or file: {}",
            path.display()
        )),
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("inspect output {}: {error}", path.display())),
    }
}

impl OutputDirectory {
    pub(super) fn new(app: &Path, leaf: &str) -> Result<Self, String> {
        if !matches!(leaf, "unpacked" | "chromium" | "firefox" | "safari") {
            return Err(format!("unsupported browser output directory: {leaf}"));
        }
        let app = std::fs::canonicalize(app)
            .map_err(|error| format!("resolve app directory {}: {error}", app.display()))?;
        if !app.is_dir() {
            return Err(format!("app path is not a directory: {}", app.display()));
        }
        let dist = app.join("dist");
        if !directory_or_missing(&dist)? {
            std::fs::create_dir(&dist)
                .map_err(|error| format!("create output parent {}: {error}", dist.display()))?;
        }
        // Recheck creation races before placing any generated files beneath it.
        directory_or_missing(&dist)?;
        let destination = dist.join(leaf);
        directory_or_missing(&destination)?;
        let stage = tempfile::Builder::new()
            .prefix(&format!(".crepus-{leaf}-stage-"))
            .tempdir_in(&dist)
            .map_err(|error| format!("create output staging directory: {error}"))?;
        Ok(Self {
            stage: Some(stage),
            destination,
            dist,
        })
    }

    pub(super) fn path(&self) -> &Path {
        self.stage.as_ref().expect("unpublished stage").path()
    }
    pub(super) fn destination(&self) -> &Path {
        &self.destination
    }

    fn safe_parent(&self) -> bool {
        directory_or_missing(&self.dist).ok() == Some(true)
            && self.dist.canonicalize().is_ok_and(|path| path == self.dist)
    }

    pub(super) fn publish(mut self) -> Result<PathBuf, String> {
        if !self.safe_parent() {
            return Err(format!(
                "output parent changed or is unsafe: {}",
                self.dist.display()
            ));
        }
        if !directory_or_missing(&self.dist)? {
            return Err(format!(
                "output parent disappeared: {}",
                self.dist.display()
            ));
        }
        let previous = if directory_or_missing(&self.destination)? {
            let backup = tempfile::Builder::new()
                .prefix(".crepus-output-backup-")
                .tempdir_in(&self.dist)
                .map_err(|error| format!("create output backup directory: {error}"))?;
            let old = backup.path().join("previous");
            std::fs::rename(&self.destination, &old).map_err(|error| {
                format!(
                    "preserve previous output {}: {error}",
                    self.destination.display()
                )
            })?;
            Some((backup, old))
        } else {
            None
        };
        if let Err(error) = std::fs::rename(self.path(), &self.destination) {
            if let Some((backup, old)) = previous {
                if let Err(rollback) = std::fs::rename(&old, &self.destination) {
                    let retained = backup.keep();
                    return Err(format!("publish output {}: {error}; restoring previous output failed: {rollback}; previous output retained at {}", self.destination.display(), retained.join("previous").display()));
                }
            }
            return Err(format!(
                "publish output {}: {error}",
                self.destination.display()
            ));
        }
        // The stage has moved: do not let TempDir cleanup target its former name.
        let _ = self.stage.take().expect("unpublished stage").keep();
        if let Some((backup, _)) = previous {
            let backup = backup.keep();
            if let Err(error) = std::fs::remove_dir_all(&backup) {
                eprintln!("[crepus-webext] output published, but previous-build cleanup failed at {}: {error}", backup.display());
            }
        }
        Ok(self.destination.clone())
    }
}

impl Drop for OutputDirectory {
    fn drop(&mut self) {
        if self.stage.is_some() && !self.safe_parent() {
            // TempDir's path cleanup must not follow a replaced ancestor symlink.
            let stage = self.stage.take().expect("unpublished stage").keep();
            eprintln!(
                "[crepus-webext] output parent changed; skipped staging cleanup at {}",
                stage.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_and_replacement_publish_only_replace_selected_browser() {
        let app = tempfile::tempdir().unwrap();
        let output = OutputDirectory::new(app.path(), "chromium").unwrap();
        assert_eq!(
            output.destination(),
            app.path().canonicalize().unwrap().join("dist/chromium")
        );
        std::fs::write(output.path().join("stale.js"), "old").unwrap();
        let destination = output.publish().unwrap();
        std::fs::create_dir(app.path().join("dist/firefox")).unwrap();
        std::fs::write(app.path().join("dist/firefox/keep"), "other").unwrap();
        std::fs::create_dir(app.path().join("dist/safari-app")).unwrap();
        std::fs::write(app.path().join("dist/safari-app/keep"), "project").unwrap();
        let replacement = OutputDirectory::new(app.path(), "chromium").unwrap();
        std::fs::write(replacement.path().join("fresh.js"), "new").unwrap();
        assert!(
            destination.join("stale.js").is_file(),
            "old build survives staging"
        );
        replacement.publish().unwrap();
        assert!(!destination.join("stale.js").exists());
        assert_eq!(
            std::fs::read_to_string(destination.join("fresh.js")).unwrap(),
            "new"
        );
        assert_eq!(
            std::fs::read_to_string(app.path().join("dist/firefox/keep")).unwrap(),
            "other"
        );
        assert_eq!(
            std::fs::read_to_string(app.path().join("dist/safari-app/keep")).unwrap(),
            "project"
        );
    }

    #[test]
    fn abandoned_stage_is_removed_and_failed_publish_restores_previous_output() {
        let app = tempfile::tempdir().unwrap();
        let output = OutputDirectory::new(app.path(), "unpacked").unwrap();
        let stage = output.path().to_owned();
        drop(output);
        assert!(!stage.exists());
        let output = OutputDirectory::new(app.path(), "unpacked").unwrap();
        std::fs::write(output.path().join("keep"), "old").unwrap();
        let destination = output.publish().unwrap();
        let broken = OutputDirectory::new(app.path(), "unpacked").unwrap();
        std::fs::remove_dir_all(broken.path()).unwrap(); // Deterministic rename failure.
        assert!(broken.publish().is_err());
        assert_eq!(
            std::fs::read_to_string(destination.join("keep")).unwrap(),
            "old"
        );
        assert_eq!(
            std::fs::read_dir(app.path().join("dist")).unwrap().count(),
            1
        );
    }

    #[test]
    fn invalid_leaf_and_regular_file_outputs_are_rejected() {
        let app = tempfile::tempdir().unwrap();
        assert!(OutputDirectory::new(app.path(), "../src").is_err());
        std::fs::write(app.path().join("dist"), "keep").unwrap();
        assert!(OutputDirectory::new(app.path(), "unpacked").is_err());
        assert_eq!(
            std::fs::read_to_string(app.path().join("dist")).unwrap(),
            "keep"
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_parents_and_leaves_including_dangling_are_not_followed() {
        use std::os::unix::fs::symlink;
        let app = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        std::fs::write(external.path().join("keep"), "external").unwrap();
        symlink(external.path(), app.path().join("dist")).unwrap();
        assert!(OutputDirectory::new(app.path(), "chromium").is_err());
        std::fs::remove_file(app.path().join("dist")).unwrap();
        std::fs::create_dir(app.path().join("dist")).unwrap();
        for target in [external.path().to_owned(), external.path().join("missing")] {
            let leaf = app.path().join("dist/chromium");
            symlink(&target, &leaf).unwrap();
            assert!(OutputDirectory::new(app.path(), "chromium").is_err());
            std::fs::remove_file(leaf).unwrap();
        }
        let pending = OutputDirectory::new(app.path(), "chromium").unwrap();
        let leaf = app.path().join("dist/chromium");
        symlink(external.path(), &leaf).unwrap();
        assert!(pending.publish().is_err(), "publication rechecks the leaf");
        assert!(std::fs::symlink_metadata(&leaf)
            .unwrap()
            .file_type()
            .is_symlink());
        std::fs::remove_file(&leaf).unwrap();
        let pending = OutputDirectory::new(app.path(), "chromium").unwrap();
        let stage_name = pending.path().file_name().unwrap().to_owned();
        std::fs::rename(app.path().join("dist"), app.path().join("old-dist")).unwrap();
        symlink(external.path(), app.path().join("dist")).unwrap();
        let unrelated = external.path().join(&stage_name);
        std::fs::create_dir(&unrelated).unwrap();
        std::fs::write(unrelated.join("keep"), "unrelated").unwrap();
        assert!(
            pending.publish().is_err(),
            "publication rechecks the parent"
        );
        assert_eq!(
            std::fs::read_to_string(unrelated.join("keep")).unwrap(),
            "unrelated"
        );
        assert!(
            app.path().join("old-dist").join(stage_name).is_dir(),
            "unsafe-parent cleanup retains the original stage"
        );
        assert_eq!(
            std::fs::read_to_string(external.path().join("keep")).unwrap(),
            "external"
        );
    }
}
