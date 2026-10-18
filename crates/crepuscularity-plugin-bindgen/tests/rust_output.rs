use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crepuscularity_plugin_bindgen::{generate_all, BindgenOptions};

struct TempOutput(PathBuf);

impl TempOutput {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "crepus-rust-bindgen-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempOutput {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn assert_rust_output(repo_root: &Path, header: Option<PathBuf>, functions: usize, extra: &str) {
    let output = TempOutput::new();
    let manifest = output.0.join("plugins.toml");
    std::fs::write(
        &manifest,
        "[contract]\nabi_header = 'crates/crepuscularity-abi/include/crepuscularity_abi.h'\n\n[[package]]\nlanguage = 'rust'\npath = 'rust'\n",
    ).unwrap();
    let files = generate_all(&BindgenOptions {
        repo_root: repo_root.to_path_buf(),
        manifest_path: manifest,
        abi_header: header.clone(),
        out_dir: Some(output.0.clone()),
    })
    .expect("generate Rust bindings from the real ABI header");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].language, "rust");
    assert_eq!(files[0].path, output.0.join("rust/abi_generated.rs"));
    let header = std::fs::read_to_string(header.unwrap_or_else(|| {
        repo_root.join("crates/crepuscularity-abi/include/crepuscularity_abi.h")
    }))
    .unwrap();
    assert_eq!(
        header
            .lines()
            .filter(|line| line.contains("crepus_") && line.trim_end().ends_with(';'))
            .count(),
        functions,
        "update the signature checks when the ABI adds functions"
    );
    let check = output.0.join("check.rs");
    std::fs::write(
        &check,
        format!(
            r#"#[path = {generated:?}]
mod generated;
use generated::*;
use std::os::raw::{{c_char, c_void}};

unsafe extern "C" fn callback(_: *const c_char, _: *mut c_void) {{}}
const _: CrepusEventCallback = None;
const _: CrepusEventCallback = Some(callback);
const _: unsafe extern "C" fn() -> *mut CrepusSession = crepus_session_new;
const _: unsafe extern "C" fn(*mut CrepusSession) = crepus_session_free;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char, *const c_char) -> i32 = crepus_session_set_template_string;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char) -> i32 = crepus_session_set_component;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char) -> i32 = crepus_session_set_files_json;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char) -> i32 = crepus_session_set_context_json;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char) -> i32 = crepus_session_apply_context_patch_json;
const _: unsafe extern "C" fn(*mut CrepusSession, Option<unsafe extern "C" fn(*const c_char, *mut c_void)>, *mut c_void) -> i32 = crepus_session_set_event_callback;
const _: unsafe extern "C" fn(*mut CrepusSession) -> *mut c_char = crepus_session_render_ir_json;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char) -> *mut c_char = crepus_session_dispatch_event_json;
const _: unsafe extern "C" fn(*mut CrepusSession) -> *mut c_char = crepus_session_take_last_error;
const _: unsafe extern "C" fn() -> *mut c_char = crepus_last_error;
const _: unsafe extern "C" fn(*mut c_char) = crepus_string_free;
{extra}
"#,
            generated = files[0].path
        ),
    )
    .unwrap();
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    for edition in ["2021", "2024"] {
        let compiled = std::process::Command::new(&compiler)
            .args([
                "--crate-type=lib",
                "--crate-name=generated_rust_abi_check",
                "--emit=metadata",
                "-Dwarnings",
            ])
            .arg(format!("--edition={edition}"))
            .arg(&check)
            .arg("-o")
            .arg(output.0.join(format!("check-{edition}.rmeta")))
            .output()
            .expect("run rustc on generated Rust bindings");
        assert!(
            compiled.status.success(),
            "Rust {edition} generated bindings failed to compile against ABI signatures:\n{}\n{}",
            String::from_utf8_lossy(&compiled.stdout),
            String::from_utf8_lossy(&compiled.stderr)
        );
    }
}

#[test]
fn generated_rust_compiles_and_matches_every_abi_function_signature() {
    assert_rust_output(&repo_root(), None, 13, "");
}

#[test]
fn overridden_header_preserves_typedefs_and_additional_functions() {
    let root = repo_root();
    let output = TempOutput::new();
    let header = output.0.join("custom.h");
    let original = std::fs::read_to_string(
        root.join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let varied = original
        .replace(
            "typedef struct CrepusSession CrepusSession;",
            "typedef  struct  CrepusSession\n    CrepusSession ;",
        )
        .replace(
            "typedef void (*CrepusEventCallback)(const char *event_json, void *userdata);",
            "typedef void ( * CrepusEventCallback ) ( const char* event_json , void* userdata );",
        )
        .replace("char *crepus", "char*  crepus")
        .replace("CrepusSession *crepus", "CrepusSession*  crepus")
        .replace(
            "#ifdef __cplusplus\n}",
            "const char *crepus_extra(const void *userdata);\n\n#ifdef __cplusplus\n}",
        );
    assert_ne!(varied, original);
    std::fs::write(&header, &varied).unwrap();
    assert_rust_output(
        &root,
        Some(header.clone()),
        14,
        "const _: unsafe extern \"C\" fn(*const c_void) -> *const c_char = crepus_extra;",
    );

    let unsupported = varied.replace(
        "typedef void ( * CrepusEventCallback ) ( const char* event_json , void* userdata );",
        "typedef int32_t (*CrepusEventCallback)(const char *event_json, void *userdata);",
    );
    assert_ne!(unsupported, varied);
    std::fs::write(&header, unsupported).unwrap();
    let rejected_output = TempOutput::new();
    let manifest = rejected_output.0.join("plugins.toml");
    std::fs::write(
        &manifest,
        "[contract]\nabi_header = 'unused.h'\n[[package]]\nlanguage = 'rust'\npath = 'rust'\n",
    )
    .unwrap();
    assert!(
        generate_all(&BindgenOptions {
            repo_root: root,
            manifest_path: manifest,
            abi_header: Some(header),
            out_dir: Some(rejected_output.0.clone()),
        })
        .is_err(),
        "unsupported callback ABI must fail rather than generate incorrect bindings"
    );
}

#[test]
fn oversized_original_headers_are_rejected_without_writing_generated_rust() {
    let root = repo_root();
    let original = std::fs::read_to_string(
        root.join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    for limit in ["bytes", "lines"] {
        let output = TempOutput::new();
        let header = output.0.join("oversized.h");
        let source = if limit == "bytes" {
            format!("/*{}*/\n{original}", "x".repeat(10 * 1024 * 1024))
        } else {
            format!("{}{original}", "//\n".repeat(200_001))
        };
        std::fs::write(&header, source).unwrap();
        let manifest = output.0.join("plugins.toml");
        std::fs::write(
            &manifest,
            "[contract]\nabi_header = 'unused.h'\n[[package]]\nlanguage = 'rust'\npath = 'rust'\n",
        )
        .unwrap();
        let result = generate_all(&BindgenOptions {
            repo_root: root.clone(),
            manifest_path: manifest,
            abi_header: Some(header),
            out_dir: Some(output.0.clone()),
        });
        assert!(
            result.is_err(),
            "original header exceeding {limit} limit was accepted"
        );
        assert!(
            !output.0.join("rust/abi_generated.rs").exists(),
            "oversized {limit} input wrote generated Rust"
        );
    }
}
