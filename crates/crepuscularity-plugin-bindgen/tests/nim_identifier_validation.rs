use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crepuscularity_plugin_bindgen::{generate_all, BindgenOptions};

struct Fixture(PathBuf);
impl Fixture {
    fn prepare(extra: &str) -> (Self, BindgenOptions) {
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
        let options = BindgenOptions {
            repo_root: root,
            manifest_path: manifest,
            abi_header: Some(header),
            out_dir: Some(dir.clone()),
        };
        (Self(dir), options)
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

#[test]
fn invalid_nim_function_names_fail_before_output() {
    for name in ["crepus__extra", "_extra", "extra_"] {
        let (fixture, options) = Fixture::prepare(&format!("void {name}(int32_t value);"));
        let error = generate_all(&options).expect_err("Nim cannot represent this C function name");
        assert!(error.contains("Nim") && error.contains(name), "{error}");
        assert!(!fixture.0.join("nim/abi_generated.nim").exists());
    }
}

#[test]
fn invalid_nim_parameter_names_fail_before_output() {
    for name in ["some__value", "_value", "value_"] {
        let (fixture, options) = Fixture::prepare(&format!("void crepus_extra(int32_t {name});"));
        let error = generate_all(&options).expect_err("Nim cannot represent this C parameter name");
        assert!(error.contains("Nim") && error.contains(name), "{error}");
        assert!(!fixture.0.join("nim/abi_generated.nim").exists());
    }
}

#[test]
fn ordinary_underscores_camel_case_and_keywords_remain_supported() {
    let (fixture, options) = Fixture::prepare("void crepus_extra(int32_t some_value); void crepusOther(int32_t otherValue); void defer(int32_t defer);");
    generate_all(&options).unwrap();
    let source = fixture.source();
    for name in [
        "crepus_extra",
        "some_value",
        "crepusOther",
        "otherValue",
        "defer",
    ] {
        assert!(source.contains(&format!("`{name}`")), "{source}");
    }
}

#[test]
#[ignore = "requires installed Nim"]
fn valid_identifier_controls_compile_with_nim() {
    let (fixture, options) = Fixture::prepare("void crepus_extra(int32_t some_value); void crepusOther(int32_t otherValue); void defer(int32_t defer);");
    generate_all(&options).unwrap();
    let result = Command::new("nim")
        .args(["check", "--hints:off"])
        .arg(fixture.0.join("nim/abi_generated.nim"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
