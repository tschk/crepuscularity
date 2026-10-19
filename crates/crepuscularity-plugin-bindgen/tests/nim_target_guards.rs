use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crepuscularity_plugin_bindgen::{generate_all, BindgenOptions};

struct Fixture(PathBuf);
impl Fixture {
    fn generate(extra: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "crepus-nim-target-guards-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let header = dir.join("selected.h");
        let original = std::fs::read_to_string(
            root.join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
        )
        .unwrap();
        std::fs::write(&header, format!("{original}\n{extra}\n")).unwrap();
        let manifest = dir.join("plugins.toml");
        std::fs::write(
            &manifest,
            "[contract]\nabi_header = 'unused.h'\n[[package]]\nlanguage = 'nim'\npath = 'nim'\n",
        )
        .unwrap();
        generate_all(&BindgenOptions {
            repo_root: root,
            manifest_path: manifest,
            abi_header: Some(header),
            out_dir: Some(dir.clone()),
        })
        .unwrap();
        Self(dir)
    }
    fn source(&self) -> String {
        std::fs::read_to_string(self.0.join("nim/abi_generated.nim")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const COLLISIONS: [&str; 5] = [
    "void crepus_extra(long value); void crepusExtra(int32_t value);",
    "void crepus_extra(int32_t value); void crepusExtra(long value);",
    "void crepus_extra(int64_t value); void crepusEXtra(long value); void crepusExtra(int32_t value);",
    "void crepus_extra(unsigned long value); void crepusExtra(uint32_t value);",
    "void crepus_extra(long **value); void crepusExtra(int32_t **value);",
];

#[test]
fn target_dependent_overloads_emit_explicit_guards() {
    for extra in COLLISIONS {
        let fixture = Fixture::generate(extra);
        let source = fixture.source();
        assert!(source.contains("when "), "missing target guard: {source}");
        assert!(
            source.contains("Nim target-dependent identifier collision"),
            "missing diagnostic: {source}"
        );
        assert!(source.contains("crepusExtra"));
        assert!(
            source.contains("else:"),
            "colliding declaration must be excluded: {source}"
        );
    }
}

#[test]
#[ignore = "requires installed Nim; checks Linux and Windows types without linking"]
fn installed_nim_checks_target_dependent_overloads() {
    for extra in COLLISIONS {
        let fixture = Fixture::generate(extra);
        for (target, expected_success) in [("linux", true), ("windows", false)] {
            let mut command = Command::new("nim");
            command.args(["check", "--hints:off"]);
            command.arg(format!("--os:{target}")).arg("--cpu:amd64");
            let result = command
                .arg(fixture.0.join("nim/abi_generated.nim"))
                .output()
                .expect("installed Nim required");
            let diagnostic = format!(
                "{}{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.status.success(), expected_success, "{diagnostic}");
            if !expected_success {
                assert!(
                    diagnostic.contains("Nim target-dependent identifier collision"),
                    "{diagnostic}"
                );
                assert!(
                    !diagnostic.contains("redefinition"),
                    "guard must exclude the duplicate: {diagnostic}"
                );
            }
        }
    }
    for extra in [
        "void crepus_extra(long value); void crepusExtra(int64_t value);",
        "void crepus_extra(long value, int32_t other); void crepusExtra(int32_t value, const char *other);",
    ] {
        let fixture = Fixture::generate(extra);
        for target in ["linux", "windows"] {
            let result = Command::new("nim")
                .args(["check", "--hints:off", "--cpu:amd64"])
                .arg(format!("--os:{target}"))
                .arg(fixture.0.join("nim/abi_generated.nim"))
                .output().unwrap();
            assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        }
    }
}
