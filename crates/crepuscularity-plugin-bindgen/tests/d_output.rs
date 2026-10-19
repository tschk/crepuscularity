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
            "crepus-d-bindgen-{}-{stamp}-{}",
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
    std::fs::write(&manifest, "[contract]\nabi_header = 'crates/crepuscularity-abi/include/crepuscularity_abi.h'\n[[package]]\nlanguage = 'd'\npath = 'd'\n").unwrap();
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
const SIGNATURES: &str = r#"import abi_generated;
static assert(int.sizeof == 4);
alias Callback = extern(C) void function(const(char)*, void*);
static assert(is(CrepusEventCallback == Callback));
alias New = extern(C) CrepusSession* function();
alias Free = extern(C) void function(CrepusSession*);
alias Template = extern(C) int function(CrepusSession*, const(char)*, const(char)*);
alias Setter = extern(C) int function(CrepusSession*, const(char)*);
alias EventSetter = extern(C) int function(CrepusSession*, CrepusEventCallback, void*);
alias Render = extern(C) char* function(CrepusSession*);
alias Dispatch = extern(C) char* function(CrepusSession*, const(char)*);
alias Error = extern(C) char* function();
alias FreeString = extern(C) void function(char*);
static assert(is(typeof(&crepus_session_new) == New));
static assert(is(typeof(&crepus_session_free) == Free));
static assert(is(typeof(&crepus_session_set_template_string) == Template));
static assert(is(typeof(&crepus_session_set_component) == Setter));
static assert(is(typeof(&crepus_session_set_files_json) == Setter));
static assert(is(typeof(&crepus_session_set_context_json) == Setter));
static assert(is(typeof(&crepus_session_apply_context_patch_json) == Setter));
static assert(is(typeof(&crepus_session_set_event_callback) == EventSetter));
static assert(is(typeof(&crepus_session_render_ir_json) == Render));
static assert(is(typeof(&crepus_session_dispatch_event_json) == Dispatch));
static assert(is(typeof(&crepus_session_take_last_error) == Render));
static assert(is(typeof(&crepus_last_error) == Error));
static assert(is(typeof(&crepus_string_free) == FreeString));
"#;
fn compile(output: &Path, extra: &str) {
    let check = output.join("d/check.d");
    std::fs::write(&check, format!("{SIGNATURES}\n{extra}")).unwrap();
    success(
        Command::new("ldc2")
            .arg("-c")
            .arg(format!("-I{}", output.join("d").display()))
            .arg(check)
            .arg(format!("-of={}", output.join("check.o").display()))
            .output()
            .expect("installed ldc2 required"),
    );
}
#[test]
fn actual_header_exports_all_thirteen_exact_typed_d_signatures() {
    let output = Output::new();
    let files = generate_all(&options(&output.0, None)).unwrap();
    assert_eq!(files.len(), 1);
    compile(&output.0, "");
}
#[test]
fn reserved_function_name_fails_before_output() {
    for name in ["module", "foreach", "ubyte"] {
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
        assert!(error.contains("D") && error.contains(name), "{error}");
        assert!(!output.0.join("d/abi_generated.d").exists());
    }
}

#[test]
fn selected_header_additional_pointer_scalar_function_is_not_dropped() {
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
    compile(&output.0, "alias Extra = extern(C) int function(int, void*, const(char)*);\nstatic assert(is(typeof(&crepus_extra) == Extra));\nalias Float = extern(C) float function(float);\nalias Double = extern(C) double function(double);\nstatic assert(is(typeof(&crepus_float) == Float));\nstatic assert(is(typeof(&crepus_double) == Double));");
}
#[test]
fn unsupported_array_unknown_type_and_opaque_value_fail_before_output() {
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
        assert!(!output.0.join("d/abi_generated.d").exists());
    }
}
#[test]
fn generated_d_callback_roundtrips_through_independent_real_header_c_stub() {
    let output = Output::new();
    let config = options(&output.0, None);
    let include = config.repo_root.join("crates/crepuscularity-abi/include");
    let files = generate_all(&config).unwrap();
    let stub = output.0.join("stub.c");
    std::fs::write(&stub, r#"#include "crepuscularity_abi.h"
#include <stdlib.h>
#include <string.h>
struct CrepusSession { CrepusEventCallback callback; void *userdata; };
CrepusSession *crepus_session_new(void) { return calloc(1, sizeof(CrepusSession)); }
void crepus_session_free(CrepusSession *s) { free(s); }
int32_t crepus_session_set_event_callback(CrepusSession *s, CrepusEventCallback cb, void *u) { s->callback=cb; s->userdata=u; return 0; }
char *crepus_session_dispatch_event_json(CrepusSession *s, const char *e) { (void)e; if(s->callback)s->callback("callback payload",s->userdata);char *p=malloc(3);memcpy(p,"{}",3);return p; }
void crepus_string_free(char *p) { free(p); }
"#).unwrap();
    let object = output.0.join("stub.o");
    success(
        Command::new("cc")
            .arg("-c")
            .arg(&stub)
            .arg("-I")
            .arg(include)
            .arg("-o")
            .arg(&object)
            .output()
            .expect("installed C compiler required"),
    );
    let check = output.0.join("d/runtime_check.d");
    std::fs::write(
        &check,
        r#"import abi_generated;
import std.string : fromStringz;
import std.stdio : writeln;
int calls;
int token = 42;
extern(C) void callback(const(char)* event, void* userdata) {
    assert(fromStringz(event) == "callback payload");
    assert(userdata == &token);
    ++calls;
}
void main() {
    CrepusEventCallback handler = &callback;
    auto session = crepus_session_new(); assert(session !is null);
    assert(crepus_session_set_event_callback(session, handler, &token) == 0);
    auto reply = crepus_session_dispatch_event_json(session, "event".ptr);
    assert(fromStringz(reply) == "{}"); crepus_string_free(reply); assert(calls == 1);
    assert(crepus_session_set_event_callback(session, null, null) == 0);
    crepus_string_free(crepus_session_dispatch_event_json(session, "event".ptr));
    assert(calls == 1); crepus_session_free(session);
    writeln("PASS generated D callback ABI against independent real-header C stub");
}
"#,
    )
    .unwrap();
    let executable = output.0.join(if cfg!(windows) {
        "runtime.exe"
    } else {
        "runtime"
    });
    success(
        Command::new("ldc2")
            .arg(format!("-I{}", output.0.join("d").display()))
            .arg(check)
            .arg(&files[0].path)
            .arg(object)
            .arg(format!("-of={}", executable.display()))
            .output()
            .expect("installed ldc2 required"),
    );
    let result = Command::new(executable).output().unwrap();
    assert!(String::from_utf8_lossy(&result.stdout).contains("PASS generated D callback ABI"));
    success(result);
}
#[test]
#[ignore = "requires CREPUS_ABI_LIB pointing to a freshly built Rust ABI library"]
fn generated_d_calls_all_thirteen_exports_against_the_real_rust_abi() {
    let library = PathBuf::from(std::env::var_os("CREPUS_ABI_LIB").expect("set CREPUS_ABI_LIB"))
        .canonicalize()
        .expect("Rust ABI library must exist");
    let output = Output::new();
    let files = generate_all(&options(&output.0, None)).unwrap();
    let check = output.0.join("d/real_runtime.d");
    std::fs::write(&check, r#"import abi_generated;
import std.string : fromStringz;
import std.algorithm.searching : canFind;
import std.stdio : writeln;
string take(char* value) {
    assert(value !is null);
    auto result = fromStringz(value).idup;
    crepus_string_free(value);
    return result;
}
int calls;
int token = 73;
extern(C) void callback(const(char)* event, void* userdata) {
    assert(fromStringz(event).canFind("bind:count:2"));
    assert(userdata == &token);
    ++calls;
}
void main() {
    auto session = crepus_session_new(); assert(session !is null);
    auto templateText = "input bind=count\nspan \"Count {count}\"";
    assert(crepus_session_set_template_string(session, templateText.ptr, null) == 0);
    assert(crepus_session_set_context_json(session, `{"count":"0"}`.ptr) == 0);
    assert(crepus_session_apply_context_patch_json(session, `{"count":"1"}`.ptr) == 0);
    assert(take(crepus_session_render_ir_json(session)).canFind("Count 1"));
    CrepusEventCallback handler = &callback;
    assert(crepus_session_set_event_callback(session, handler, &token) == 0);
    assert(take(crepus_session_dispatch_event_json(session, `{"handler":"bind:count:2"}`.ptr)).canFind("Count 2"));
    assert(calls == 1);
    assert(crepus_session_set_event_callback(session, null, null) == 0);
    assert(take(crepus_session_dispatch_event_json(session, `{"handler":"bind:count:3"}`.ptr)).canFind("Count 3"));
    assert(calls == 1);
    int invalidContext = crepus_session_set_context_json(session, "{".ptr);
    assert(invalidContext == -1);
    assert(take(crepus_session_take_last_error(session)).length > 0);
    assert(crepus_session_take_last_error(session) is null);
    int invalidTemplate = crepus_session_set_template_string(session, null, null);
    assert(invalidTemplate == -1);
    assert(take(crepus_session_take_last_error(session)).length > 0);
    // Source syntax is validated by render, not by the template setter.
    assert(crepus_session_set_template_string(session, "<div>".ptr, null) == 0);
    assert(crepus_session_render_ir_json(session) is null);
    assert(take(crepus_session_take_last_error(session)).length > 0);
    assert(crepus_session_set_files_json(session, `{"entry":"app.crepus","files":{"app.crepus":"span \"File works\""}}`.ptr) == 0);
    assert(take(crepus_session_render_ir_json(session)).canFind("File works"));
    auto componentText = "--- First\nspan \"Wrong first\"\n--- Second\nspan \"Selected second\"";
    assert(crepus_session_set_template_string(session, componentText.ptr, null) == 0);
    assert(crepus_session_set_component(session, "Second".ptr) == 0);
    auto selected = take(crepus_session_render_ir_json(session));
    assert(selected.canFind("Selected second") && !selected.canFind("Wrong first"));
    assert(crepus_session_set_component(session, null) == 0);
    assert(crepus_session_set_template_string(session, "span \"Reset\"".ptr, null) == 0);
    assert(take(crepus_session_render_ir_json(session)).canFind("Reset"));
    crepus_session_free(session);
    assert(crepus_session_set_context_json(null, "{}".ptr) == -1);
    assert(take(crepus_last_error()).length > 0);
    assert(crepus_last_error() is null);
    writeln("PASS generated D: all 13 exports against actual Rust ABI");
}
"#).unwrap();
    let executable = output.0.join(if cfg!(windows) {
        "real_runtime.exe"
    } else {
        "real_runtime"
    });
    let mut compiler = Command::new("ldc2");
    compiler
        .arg(format!("-I{}", output.0.join("d").display()))
        .arg(check)
        .arg(&files[0].path)
        .arg(format!("-L{}", library.display()))
        .arg(format!("-of={}", executable.display()));
    if cfg!(unix) {
        compiler
            .arg("-L-rpath")
            .arg(format!("-L{}", library.parent().unwrap().display()));
    }
    success(compiler.output().expect("installed ldc2 required"));
    let result = Command::new(executable)
        .output()
        .expect("run actual Rust ABI fixture");
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("all 13 exports against actual Rust ABI")
    );
    success(result);
}
