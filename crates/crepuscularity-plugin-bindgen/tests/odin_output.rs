use crepuscularity_plugin_bindgen::{generate_all, BindgenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

struct Output(PathBuf);
impl Output {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "crepus-odin-bindgen-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn options(output: &Path, header: Option<PathBuf>) -> BindgenOptions {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let manifest = output.join("plugins.toml");
    std::fs::write(&manifest, "[contract]\nabi_header = 'crates/crepuscularity-abi/include/crepuscularity_abi.h'\n[[package]]\nlanguage = 'odin'\npath = 'odin'\n").unwrap();
    BindgenOptions {
        repo_root: root,
        manifest_path: manifest,
        abi_header: header,
        out_dir: Some(output.into()),
    }
}
fn success(result: std::process::Output) {
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
const SIGNATURES: &str = r#"package check
import abi "../odin"
main :: proc() {
    #assert(size_of(i32) == 4)
    constructor: proc "c" () -> ^abi.CrepusSession = abi.crepus_session_new
    free_session: proc "c" (^abi.CrepusSession) = abi.crepus_session_free
    template_setter: proc "c" (^abi.CrepusSession, cstring, cstring) -> i32 = abi.crepus_session_set_template_string
    component: proc "c" (^abi.CrepusSession, cstring) -> i32 = abi.crepus_session_set_component
    files: proc "c" (^abi.CrepusSession, cstring) -> i32 = abi.crepus_session_set_files_json
    context_setter: proc "c" (^abi.CrepusSession, cstring) -> i32 = abi.crepus_session_set_context_json
    patch: proc "c" (^abi.CrepusSession, cstring) -> i32 = abi.crepus_session_apply_context_patch_json
    event_setter: proc "c" (^abi.CrepusSession, abi.CrepusEventCallback, rawptr) -> i32 = abi.crepus_session_set_event_callback
    render: proc "c" (^abi.CrepusSession) -> cstring = abi.crepus_session_render_ir_json
    dispatch: proc "c" (^abi.CrepusSession, cstring) -> cstring = abi.crepus_session_dispatch_event_json
    take_error: proc "c" (^abi.CrepusSession) -> cstring = abi.crepus_session_take_last_error
    global_error: proc "c" () -> cstring = abi.crepus_last_error
    free_string: proc "c" (cstring) = abi.crepus_string_free
    callback: abi.CrepusEventCallback = nil
    _ = constructor; _ = free_session; _ = template_setter; _ = component
    _ = files; _ = context_setter; _ = patch; _ = event_setter; _ = render
    _ = dispatch; _ = take_error; _ = global_error; _ = free_string; _ = callback
}
"#;
fn check(output: &Path, extra: &str) {
    let dir = output.join("check");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("check.odin"), format!("{SIGNATURES}\n{extra}")).unwrap();
    success(
        Command::new("odin")
            .arg("check")
            .arg(dir)
            .output()
            .expect("installed Odin required"),
    );
}
#[test]
fn actual_header_exports_all_thirteen_exact_typed_odin_signatures() {
    let output = Output::new();
    generate_all(&options(&output.0, None)).unwrap();
    check(&output.0, "");
}
#[test]
fn reserved_function_name_fails_before_output() {
    for name in ["defer", "context", "or_return", "notin"] {
        let output = Output::new();
        let config = options(&output.0, None);
        let source = std::fs::read_to_string(
            config
                .repo_root
                .join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
        )
        .unwrap();
        let header = output.0.join("reserved.h");
        std::fs::write(
            &header,
            source.replacen(
                "#ifdef __cplusplus",
                &format!("int32_t {name}(void);\n#ifdef __cplusplus"),
                1,
            ),
        )
        .unwrap();
        let error = generate_all(&options(&output.0, Some(header))).unwrap_err();
        assert!(error.contains("Odin") && error.contains(name), "{error}");
        assert!(!output.0.join("odin/abi_generated.odin").exists());
    }
}

#[test]
fn selected_header_extra_pointer_scalar_function_is_preserved() {
    let output = Output::new();
    let config = options(&output.0, None);
    let source = std::fs::read_to_string(
        config
            .repo_root
            .join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap();
    let header = output.0.join("custom.h");
    std::fs::write(&header, source.replacen("#ifdef __cplusplus", "int32_t crepus_extra(int32_t value, void *userdata, const char *name);\nfloat crepus_float(float value);\ndouble crepus_double(double value);\n#ifdef __cplusplus", 1)).unwrap();
    generate_all(&options(&output.0, Some(header))).unwrap();
    check(
        &output.0,
        "extra: proc \"c\" (i32, rawptr, cstring) -> i32 = abi.crepus_extra\nfloatFn: proc \"c\" (f32) -> f32 = abi.crepus_float\ndoubleFn: proc \"c\" (f64) -> f64 = abi.crepus_double",
    );
}
#[test]
fn unsupported_arrays_unknown_types_and_opaque_values_do_not_write_bindings() {
    for declaration in [
        "void crepus_array(const int32_t values[4]);",
        "long double crepus_unknown(void);",
        "CrepusSession crepus_by_value(void);",
    ] {
        let output = Output::new();
        let config = options(&output.0, None);
        let source = std::fs::read_to_string(
            config
                .repo_root
                .join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
        )
        .unwrap();
        let header = output.0.join("unsupported.h");
        std::fs::write(
            &header,
            source.replacen(
                "#ifdef __cplusplus",
                &format!("{declaration}\n#ifdef __cplusplus"),
                1,
            ),
        )
        .unwrap();
        assert!(
            generate_all(&options(&output.0, Some(header))).is_err(),
            "{declaration}"
        );
        assert!(!output.0.join("odin/abi_generated.odin").exists());
    }
}
const CALLBACK: &str = r#"calls: int
payload: cstring
userdata_seen: rawptr
callback :: proc "c" (event: cstring, userdata: rawptr) {
    calls += 1; payload = event; userdata_seen = userdata
}
"#;
fn run(output: &Path, source: &str, library_dir: &Path) -> std::process::Output {
    let dir = output.join("runtime");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("main.odin"), source).unwrap();
    let exe = output.join(if cfg!(windows) {
        "runtime.exe"
    } else {
        "runtime-check"
    });
    let mut flags = format!("-L{}", library_dir.display());
    if cfg!(unix) {
        flags.push_str(&format!(" -Wl,-rpath,{}", library_dir.display()));
    }
    let mut compiler = Command::new("odin");
    compiler
        .arg("build")
        .arg(dir)
        .arg(format!("-out:{}", exe.display()))
        .arg(format!("-extra-linker-flags:{flags}"));
    // Odin's linker driver does not quote SDK paths containing spaces.
    #[cfg(target_os = "macos")]
    if Path::new("/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk").is_dir() {
        compiler
            .env("DEVELOPER_DIR", "/Library/Developer/CommandLineTools")
            .env(
                "SDKROOT",
                "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk",
            );
    }
    success(compiler.output().expect("installed Odin required"));
    Command::new(exe).output().unwrap()
}
#[test]
fn generated_odin_callback_uses_an_independent_real_header_c_stub() {
    let output = Output::new();
    let config = options(&output.0, None);
    generate_all(&config).unwrap();
    let stub = output.0.join("stub.c");
    std::fs::write(&stub, r#"#include "crepuscularity_abi.h"
#include <stdlib.h>
#include <string.h>
struct CrepusSession { CrepusEventCallback callback; void *userdata; };
CrepusSession *crepus_session_new(void) { return calloc(1,sizeof(CrepusSession)); }
void crepus_session_free(CrepusSession *s) { free(s); }
int32_t crepus_session_set_event_callback(CrepusSession *s,CrepusEventCallback cb,void *u){s->callback=cb;s->userdata=u;return 0;}
char *crepus_session_dispatch_event_json(CrepusSession *s,const char *e){(void)e;if(s->callback)s->callback("callback payload",s->userdata);char*p=malloc(3);memcpy(p,"{}",3);return p;}
void crepus_string_free(char*p){free(p);}
"#).unwrap();
    let object = output.0.join("stub.o");
    success(
        Command::new("cc")
            .arg("-c")
            .arg(stub)
            .arg("-I")
            .arg(config.repo_root.join("crates/crepuscularity-abi/include"))
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap(),
    );
    let mut archiver = if cfg!(target_os = "macos") {
        let mut command = Command::new("xcrun");
        command.args(["libtool", "-static", "-o"]);
        command
    } else {
        let mut command = Command::new("ar");
        command.arg("rcs");
        command
    };
    success(
        archiver
            .arg(output.0.join("libcrepuscularity_abi.a"))
            .arg(object)
            .output()
            .unwrap(),
    );
    let source = format!(
        r#"package main
import abi "../odin"
import "core:fmt"
{CALLBACK}
main :: proc() {{
    token := 42
    handler: abi.CrepusEventCallback = callback
    session := abi.crepus_session_new(); assert(session != nil)
    assert(abi.crepus_session_set_event_callback(session, handler, &token) == 0)
    reply := abi.crepus_session_dispatch_event_json(session, "event")
    assert(string(reply) == "{{}}")
    assert(calls == 1 && string(payload) == "callback payload" && userdata_seen == rawptr(&token))
    abi.crepus_string_free(reply)
    assert(abi.crepus_session_set_event_callback(session, nil, nil) == 0)
    abi.crepus_string_free(abi.crepus_session_dispatch_event_json(session, "event"))
    assert(calls == 1); abi.crepus_session_free(session)
    fmt.println("PASS generated Odin callback ABI against independent real-header C stub")
}}
"#
    );
    let result = run(&output.0, &source, &output.0);
    assert!(String::from_utf8_lossy(&result.stdout).contains("PASS generated Odin callback ABI"));
    success(result);
}

#[test]
#[ignore = "requires CREPUS_ABI_LIB pointing to a freshly built Rust ABI library"]
fn generated_odin_calls_all_thirteen_exports_against_the_real_rust_abi() {
    let library = PathBuf::from(std::env::var_os("CREPUS_ABI_LIB").expect("set CREPUS_ABI_LIB"))
        .canonicalize()
        .expect("Rust ABI library must exist");
    let expected_name = format!(
        "{}crepuscularity_abi{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    assert_eq!(
        library.file_name().unwrap(),
        std::ffi::OsStr::new(&expected_name),
        "Odin links the conventional ABI library name"
    );
    let output = Output::new();
    generate_all(&options(&output.0, None)).unwrap();
    let source = format!(
        r#"package main
import abi "../odin"
import "core:fmt"
import "core:strings"
{CALLBACK}
has :: proc(value: cstring, expected: string) -> bool {{
    assert(value != nil)
    // string(cstring) borrows: inspect before freeing the Rust allocation.
    assert(len(string(value)) > 0)
    found := strings.contains(string(value), expected)
    abi.crepus_string_free(value)
    return found
}}
main :: proc() {{
    session := abi.crepus_session_new(); assert(session != nil)
    assert(abi.crepus_session_set_template_string(session, "input bind=count\nspan \"Count {{count}}\"", nil) == 0)
    assert(abi.crepus_session_set_context_json(session, `{{"count":"0"}}`) == 0)
    assert(abi.crepus_session_apply_context_patch_json(session, `{{"count":"1"}}`) == 0)
    assert(has(abi.crepus_session_render_ir_json(session), "Count 1"))
    token := 73
    handler: abi.CrepusEventCallback = callback
    assert(abi.crepus_session_set_event_callback(session, handler, &token) == 0)
    assert(has(abi.crepus_session_dispatch_event_json(session, `{{"handler":"bind:count:2"}}`), "Count 2"))
    assert(calls == 1 && strings.contains(string(payload), "bind:count:2") && userdata_seen == rawptr(&token))
    assert(abi.crepus_session_set_event_callback(session, nil, nil) == 0)
    assert(has(abi.crepus_session_dispatch_event_json(session, `{{"handler":"bind:count:3"}}`), "Count 3"))
    assert(calls == 1)
    invalid_context: i32 = abi.crepus_session_set_context_json(session, "{{")
    assert(invalid_context == -1)
    assert(has(abi.crepus_session_take_last_error(session), ""))
    assert(abi.crepus_session_take_last_error(session) == nil)
    invalid_template: i32 = abi.crepus_session_set_template_string(session, nil, nil)
    assert(invalid_template == -1)
    assert(has(abi.crepus_session_take_last_error(session), ""))
    // Malformed source is diagnosed at render, not by the setter.
    assert(abi.crepus_session_set_template_string(session, "<div>", nil) == 0)
    assert(abi.crepus_session_render_ir_json(session) == nil)
    assert(has(abi.crepus_session_take_last_error(session), ""))
    assert(abi.crepus_session_set_files_json(session, `{{"entry":"app.crepus","files":{{"app.crepus":"span \"File works\""}}}}`) == 0)
    assert(has(abi.crepus_session_render_ir_json(session), "File works"))
    assert(abi.crepus_session_set_template_string(session, "--- First\nspan \"Wrong first\"\n--- Second\nspan \"Selected second\"", nil) == 0)
    assert(abi.crepus_session_set_component(session, "Second") == 0)
    selected := abi.crepus_session_render_ir_json(session)
    assert(selected != nil && strings.contains(string(selected), "Selected second") && !strings.contains(string(selected), "Wrong first"))
    abi.crepus_string_free(selected)
    assert(abi.crepus_session_set_component(session, nil) == 0)
    assert(abi.crepus_session_set_template_string(session, "span \"Reset\"", nil) == 0)
    assert(has(abi.crepus_session_render_ir_json(session), "Reset"))
    abi.crepus_session_free(session)
    assert(abi.crepus_session_set_context_json(nil, "{{}}") == -1)
    assert(has(abi.crepus_last_error(), "")); assert(abi.crepus_last_error() == nil)
    fmt.println("PASS generated Odin: all 13 exports against actual Rust ABI")
}}
"#
    );
    let result = run(&output.0, &source, library.parent().unwrap());
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("all 13 exports against actual Rust ABI")
    );
    success(result);
}
