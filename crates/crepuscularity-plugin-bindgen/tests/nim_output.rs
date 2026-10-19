use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crepuscularity_plugin_bindgen::{generate_all, BindgenOptions};

struct Output(PathBuf);
impl Output {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "crepus-nim-bindgen-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
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
    std::fs::write(&manifest, "[contract]\nabi_header = 'crates/crepuscularity-abi/include/crepuscularity_abi.h'\n[[package]]\nlanguage = 'nim'\npath = 'nim'\n").unwrap();
    BindgenOptions {
        repo_root: root,
        manifest_path: manifest,
        abi_header: header,
        out_dir: Some(output.into()),
    }
}

fn assert_generated_signatures(output: &Path) {
    let source = std::fs::read_to_string(output.join("nim/abi_generated.nim")).unwrap();
    assert!(
        source.contains(
            r#"type CrepusEventCallback* = proc(event_json: cstring; userdata: pointer) {.cdecl.}"#
        ),
        "{source}"
    );
    for expected in [
        r#"proc `crepus_session_new`*(): ptr CrepusSession {.importc: "crepus_session_new", cdecl.}"#,
        r#"proc `crepus_session_free`*(`session`: ptr CrepusSession) {.importc: "crepus_session_free", cdecl.}"#,
        r#"proc `crepus_session_set_template_string`*(`session`: ptr CrepusSession; `template_utf8`: cstring; `base_dir_utf8`: cstring): int32 {.importc: "crepus_session_set_template_string", cdecl.}"#,
        r#"proc `crepus_session_set_component`*(`session`: ptr CrepusSession; `component_utf8`: cstring): int32 {.importc: "crepus_session_set_component", cdecl.}"#,
        r#"proc `crepus_session_set_files_json`*(`session`: ptr CrepusSession; `files_json_utf8`: cstring): int32 {.importc: "crepus_session_set_files_json", cdecl.}"#,
        r#"proc `crepus_session_set_context_json`*(`session`: ptr CrepusSession; `context_json_utf8`: cstring): int32 {.importc: "crepus_session_set_context_json", cdecl.}"#,
        r#"proc `crepus_session_apply_context_patch_json`*(`session`: ptr CrepusSession; `context_json_utf8`: cstring): int32 {.importc: "crepus_session_apply_context_patch_json", cdecl.}"#,
        r#"proc `crepus_session_set_event_callback`*(`session`: ptr CrepusSession; `callback`: CrepusEventCallback; `userdata`: pointer): int32 {.importc: "crepus_session_set_event_callback", cdecl.}"#,
        r#"proc `crepus_session_render_ir_json`*(`session`: ptr CrepusSession): cstring {.importc: "crepus_session_render_ir_json", cdecl.}"#,
        r#"proc `crepus_session_dispatch_event_json`*(`session`: ptr CrepusSession; `event_json_utf8`: cstring): cstring {.importc: "crepus_session_dispatch_event_json", cdecl.}"#,
        r#"proc `crepus_session_take_last_error`*(`session`: ptr CrepusSession): cstring {.importc: "crepus_session_take_last_error", cdecl.}"#,
        r#"proc `crepus_last_error`*(): cstring {.importc: "crepus_last_error", cdecl.}"#,
        r#"proc `crepus_string_free`*(`ptr`: cstring) {.importc: "crepus_string_free", cdecl.}"#,
    ] {
        assert!(
            source.lines().any(|line| line.trim() == expected),
            "missing {expected}:\n{source}"
        );
    }
    if source.contains("`crepus_extra`") {
        assert!(source.contains(r#"proc `crepus_extra`*(`value`: int32; `userdata`: pointer; `name`: cstring): int32 {.importc: "crepus_extra", cdecl.}"#), "{source}");
    }
    if source.contains("`crepus_float`") {
        assert!(source.contains(r#"proc `crepus_float`*(`value`: float32): float32 {.importc: "crepus_float", cdecl.}"#), "{source}");
    }
    if source.contains("`crepus_double`") {
        assert!(source.contains(r#"proc `crepus_double`*(`value`: float64): float64 {.importc: "crepus_double", cdecl.}"#), "{source}");
    }
}

const SIGNATURES: &str = r#"import abi_generated

proc callback(event: cstring, userdata: pointer) {.cdecl.} = discard
let callbackValue: CrepusEventCallback = callback
let absentCallback: CrepusEventCallback = nil
let absentSession: ptr CrepusSession = nil
let constructor: proc(): ptr CrepusSession {.cdecl.} = crepus_session_new
let freeSession: proc(session: ptr CrepusSession) {.cdecl.} = crepus_session_free
let templateSetter: proc(session: ptr CrepusSession, text: cstring, base: cstring): int32 {.cdecl.} = crepus_session_set_template_string
let componentSetter: proc(session: ptr CrepusSession, name: cstring): int32 {.cdecl.} = crepus_session_set_component
let filesSetter: proc(session: ptr CrepusSession, text: cstring): int32 {.cdecl.} = crepus_session_set_files_json
let contextSetter: proc(session: ptr CrepusSession, text: cstring): int32 {.cdecl.} = crepus_session_set_context_json
let contextPatch: proc(session: ptr CrepusSession, text: cstring): int32 {.cdecl.} = crepus_session_apply_context_patch_json
let eventSetter: proc(session: ptr CrepusSession, callback: CrepusEventCallback, userdata: pointer): int32 {.cdecl.} = crepus_session_set_event_callback
let render: proc(session: ptr CrepusSession): cstring {.cdecl.} = crepus_session_render_ir_json
let dispatch: proc(session: ptr CrepusSession, event: cstring): cstring {.cdecl.} = crepus_session_dispatch_event_json
let takeError: proc(session: ptr CrepusSession): cstring {.cdecl.} = crepus_session_take_last_error
let globalError: proc(): cstring {.cdecl.} = crepus_last_error
let freeString: proc(value: cstring) {.cdecl.} = crepus_string_free
static: doAssert sizeof(int32) == 4
"#;

fn compile_import(output: &Path, extra: &str) {
    let check = output.join("nim/check.nim");
    std::fs::write(&check, format!("{SIGNATURES}\n{extra}\n")).unwrap();
    let checked = Command::new("nim")
        .args(["check", "--hints:off"])
        .arg(&check)
        .output()
        .expect("installed Nim required");
    assert!(
        checked.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&checked.stdout),
        String::from_utf8_lossy(&checked.stderr)
    );
    // Compile the complete imported function-pointer signature checks too: the
    // raw FFI prototypes retain Nim's typed calling convention across C output.
    let compiled_signatures = Command::new("nim")
        .args(["c", "--noLinking:on", "--hints:off"])
        .arg(format!(
            "--nimcache:{}",
            output.join("signature-cache").display()
        ))
        .arg(&check)
        .output()
        .expect("installed Nim and C compiler required");
    assert!(
        compiled_signatures.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&compiled_signatures.stdout),
        String::from_utf8_lossy(&compiled_signatures.stderr)
    );
    // Separately compile direct calls so the imported argument and return types
    // also cross the generated C call sites.
    let calls = output.join("nim/calls.nim");
    let extra_call = if extra.is_empty() {
        ""
    } else {
        "discard crepus_extra(1'i32, nil, \"extra\")"
    };
    std::fs::write(
        &calls,
        format!(
            r#"import abi_generated
let session: ptr CrepusSession = crepus_session_new()
discard crepus_session_set_template_string(session, "span", nil)
discard crepus_session_set_component(session, nil)
discard crepus_session_set_files_json(session, "{{}}")
discard crepus_session_set_context_json(session, "{{}}")
discard crepus_session_apply_context_patch_json(session, "{{}}")
discard crepus_session_set_event_callback(session, nil, nil)
let rendered: cstring = crepus_session_render_ir_json(session)
crepus_string_free(rendered)
crepus_string_free(crepus_session_dispatch_event_json(session, "{{}}"))
crepus_string_free(crepus_session_take_last_error(session))
crepus_string_free(crepus_last_error())
crepus_session_free(session)
{extra_call}
"#
        ),
    )
    .unwrap();
    let result = Command::new("nim")
        .args(["c", "--noLinking:on", "--hints:off"])
        .arg(format!("--nimcache:{}", output.join("nimcache").display()))
        .arg(&calls)
        .output()
        .expect("installed Nim and C compiler required");
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn actual_header_exports_all_thirteen_typed_abi_functions_to_an_importing_module_case(
    compile_external: bool,
) {
    let output = Output::new();
    let files = generate_all(&options(&output.0, None)).unwrap();
    assert_eq!(files.len(), 1);
    let generated = std::fs::read_to_string(&files[0].path).unwrap();
    assert_eq!(
        generated
            .lines()
            .filter(|line| line.trim_start().starts_with("proc "))
            .count(),
        13,
        "{generated}"
    );
    assert_generated_signatures(&output.0);
    if compile_external {
        compile_import(&output.0, "");
    }
}

#[test]
fn actual_header_exports_all_thirteen_typed_abi_functions_to_an_importing_module() {
    actual_header_exports_all_thirteen_typed_abi_functions_to_an_importing_module_case(false);
}

#[test]
#[ignore = "requires installed Nim and a C compiler"]
fn actual_header_exports_all_thirteen_typed_abi_functions_to_an_importing_module_compiler_check() {
    actual_header_exports_all_thirteen_typed_abi_functions_to_an_importing_module_case(true);
}

fn selected_header_extra_function_preserves_pointer_and_scalar_types_case(compile_external: bool) {
    let output = Output::new();
    let original = options(&output.0, None);
    let source = std::fs::read_to_string(
        original
            .repo_root
            .join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let header = output.0.join("custom.h");
    let extra = "int32_t crepus_extra(int32_t value, void *userdata, const char *name);\nfloat crepus_float(float value);\ndouble crepus_double(double value);\n";
    std::fs::write(
        &header,
        source.replacen(
            "#ifdef __cplusplus",
            &format!("{extra}\n#ifdef __cplusplus"),
            1,
        ),
    )
    .unwrap();
    let files = generate_all(&options(&output.0, Some(header))).unwrap();
    let generated = std::fs::read_to_string(&files[0].path).unwrap();
    assert_eq!(
        generated
            .lines()
            .filter(|line| line.trim_start().starts_with("proc "))
            .count(),
        16,
        "{generated}"
    );
    assert_generated_signatures(&output.0);
    let source = std::fs::read_to_string(output.0.join("nim/abi_generated.nim")).unwrap();
    for name in ["crepus_extra", "crepus_float", "crepus_double"] {
        assert!(source.contains(name), "missing {name}: {source}");
    }

    if compile_external {
        compile_import(&output.0, "let extra: proc(value: int32, userdata: pointer, name: cstring): int32 {.cdecl.} = crepus_extra\nlet floatFn: proc(value: float32): float32 {.cdecl.} = crepus_float\nlet doubleFn: proc(value: float64): float64 {.cdecl.} = crepus_double");
    }
}

#[test]
fn selected_header_extra_function_preserves_pointer_and_scalar_types() {
    selected_header_extra_function_preserves_pointer_and_scalar_types_case(false);
}

#[test]
#[ignore = "requires installed Nim and a C compiler"]
fn selected_header_extra_function_preserves_pointer_and_scalar_types_compiler_check() {
    selected_header_extra_function_preserves_pointer_and_scalar_types_case(true);
}

#[test]
fn unsupported_array_or_unknown_type_fails_without_writing_bindings() {
    for declaration in [
        "void crepus_array(const int32_t values[4]);",
        "long double crepus_unrepresentable(void);",
        "CrepusSession crepus_by_value(void);",
    ] {
        let output = Output::new();
        let original = options(&output.0, None);
        let source = std::fs::read_to_string(
            original
                .repo_root
                .join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
        )
        .unwrap()
        .replace("\r\n", "\n");
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
        assert!(
            !output.0.join("nim/abi_generated.nim").exists(),
            "unsupported input wrote a binding file"
        );
    }
}

#[test]
#[ignore = "requires installed Nim and a C compiler"]
fn generated_typed_callbacks_roundtrip_through_an_independent_real_header_c_stub() {
    let output = Output::new();
    let config = options(&output.0, None);
    let include_dir = config.repo_root.join("crates/crepuscularity-abi/include");
    generate_all(&config).unwrap();
    // This C boundary independently compiles the authoritative header. It is
    // a stub for checking ABI calling conventions, not the Rust ABI runtime.
    std::fs::write(output.0.join("nim/callback_stub.c"), r#"
#include "crepuscularity_abi.h"
#include <stdlib.h>
#include <string.h>
struct CrepusSession { CrepusEventCallback callback; void *userdata; };
CrepusSession *crepus_session_new(void) { return calloc(1, sizeof(CrepusSession)); }
void crepus_session_free(CrepusSession *session) { free(session); }
int32_t crepus_session_set_event_callback(CrepusSession *session, CrepusEventCallback callback, void *userdata) {
    session->callback = callback; session->userdata = userdata; return 0;
}
char *crepus_session_dispatch_event_json(CrepusSession *session, const char *event) {
    (void)event;
    if (session->callback) session->callback("callback payload", session->userdata);
    char *result = malloc(3); memcpy(result, "{}", 3); return result;
}
void crepus_string_free(char *value) { free(value); }
"#).unwrap();
    let check = output.0.join("nim/callback_runtime.nim");
    std::fs::write(
        &check,
        r#"import abi_generated
{.compile: "callback_stub.c".}
var calls = 0
var token = 42
proc callback(event: cstring, userdata: pointer) {.cdecl.} =
  doAssert $event == "callback payload"
  doAssert userdata == addr token
  inc calls
let typedCallback: CrepusEventCallback = callback
let session: ptr CrepusSession = crepus_session_new()
doAssert session != nil
doAssert crepus_session_set_event_callback(session, typedCallback, addr token) == 0
crepus_string_free(crepus_session_dispatch_event_json(session, "event"))
doAssert calls == 1
doAssert crepus_session_set_event_callback(session, nil, nil) == 0
crepus_string_free(crepus_session_dispatch_event_json(session, "event"))
doAssert calls == 1
crepus_session_free(session)
echo "PASS generated Nim callback ABI against independent real-header C stub"
"#,
    )
    .unwrap();
    let result = Command::new("nim")
        .args(["c", "-r", "--hints:off"])
        .arg(format!("--passC:-I{}", include_dir.display()))
        .arg(format!(
            "--nimcache:{}",
            output.0.join("runtime-cache").display()
        ))
        .arg(format!(
            "--out:{}",
            output.0.join("callback_runtime").display()
        ))
        .arg(check)
        .output()
        .expect("installed Nim and C compiler required");
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("PASS generated Nim callback ABI"));
}

#[test]
#[ignore = "requires CREPUS_ABI_LIB pointing to a freshly built Rust ABI library"]
fn generated_nim_calls_all_thirteen_exports_against_the_real_rust_abi() {
    let library = PathBuf::from(std::env::var_os("CREPUS_ABI_LIB").expect("set CREPUS_ABI_LIB"))
        .canonicalize()
        .expect("Rust ABI library must exist");
    let output = Output::new();
    generate_all(&options(&output.0, None)).unwrap();
    let check = output.0.join("nim/real_runtime.nim");
    std::fs::write(&check, r#"import abi_generated
import std/strutils
proc take(value: cstring): string =
  doAssert value != nil
  result = $value
  crepus_string_free(value)
var calls = 0
var token = 73
proc callback(event: cstring, userdata: pointer) {.cdecl.} =
  doAssert ($event).contains("bind:count:2")
  doAssert userdata == addr token
  inc calls
let session: ptr CrepusSession = crepus_session_new()
doAssert session != nil
let sourceTemplate = "input bind=count\nspan \"Count {count}\""
doAssert crepus_session_set_template_string(session, sourceTemplate.cstring, nil) == 0
doAssert crepus_session_set_context_json(session, """{"count":"0"}""") == 0
doAssert crepus_session_apply_context_patch_json(session, """{"count":"1"}""") == 0
doAssert take(crepus_session_render_ir_json(session)).contains("Count 1")
let handler: CrepusEventCallback = callback
doAssert crepus_session_set_event_callback(session, handler, addr token) == 0
doAssert take(crepus_session_dispatch_event_json(session, """{"handler":"bind:count:2"}""")).contains("Count 2")
doAssert calls == 1
doAssert crepus_session_set_event_callback(session, nil, nil) == 0
doAssert take(crepus_session_dispatch_event_json(session, """{"handler":"bind:count:3"}""")).contains("Count 3")
doAssert calls == 1
let invalidContext: int32 = crepus_session_set_context_json(session, "{")
doAssert invalidContext == -1
doAssert take(crepus_session_take_last_error(session)).len > 0
doAssert crepus_session_take_last_error(session) == nil
# A missing template pointer is an immediate signed status error. Malformed
# source is accepted by the setter and diagnosed by rendering, as Rust specifies.
let invalidTemplate: int32 = crepus_session_set_template_string(session, nil, nil)
doAssert invalidTemplate == -1
doAssert take(crepus_session_take_last_error(session)).len > 0
doAssert crepus_session_set_template_string(session, "<div>", nil) == 0
doAssert crepus_session_render_ir_json(session) == nil
doAssert take(crepus_session_take_last_error(session)).len > 0
doAssert crepus_session_set_files_json(session, """{"entry":"app.crepus","files":{"app.crepus":"span \"File works\""}}""") == 0
doAssert take(crepus_session_render_ir_json(session)).contains("File works")
let components = "--- First\nspan \"Wrong first\"\n--- Second\nspan \"Selected second\""
doAssert crepus_session_set_template_string(session, components.cstring, nil) == 0
doAssert crepus_session_set_component(session, "Second") == 0
let selected = take(crepus_session_render_ir_json(session))
doAssert selected.contains("Selected second") and not selected.contains("Wrong first")
doAssert crepus_session_set_component(session, nil) == 0
doAssert crepus_session_set_template_string(session, "span \"Reset\"", nil) == 0
doAssert take(crepus_session_render_ir_json(session)).contains("Reset")
crepus_session_free(session)
doAssert crepus_session_set_context_json(nil, "{}") == -1
doAssert take(crepus_last_error()).len > 0
doAssert crepus_last_error() == nil
echo "PASS generated Nim: all 13 exports against actual Rust ABI"
"#).unwrap();
    let executable = output.0.join(if cfg!(windows) {
        "real_runtime.exe"
    } else {
        "real_runtime"
    });
    let mut compiler = Command::new("nim");
    compiler
        .args(["c", "--hints:off"])
        .arg(format!("--passL:{}", library.display()))
        .arg(format!(
            "--nimcache:{}",
            output.0.join("real-cache").display()
        ))
        .arg(format!("--out:{}", executable.display()));
    if cfg!(unix) {
        compiler.arg(format!(
            "--passL:-Wl,-rpath,{}",
            library.parent().unwrap().display()
        ));
    }
    let compiled = compiler
        .arg(check)
        .output()
        .expect("installed Nim and C compiler required");
    assert!(
        compiled.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let result = Command::new(executable)
        .output()
        .expect("run real Rust ABI fixture");
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("all 13 exports against actual Rust ABI")
    );
}

#[test]
fn style_insensitive_function_names_fail_without_writing_bindings() {
    let output = Output::new();
    let original = options(&output.0, None);
    let source = std::fs::read_to_string(
        original
            .repo_root
            .join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let header = output.0.join("collision.h");
    std::fs::write(
        &header,
        source.replacen(
            "#ifdef __cplusplus",
            "void crepus_extra(void);\nvoid crepusExtra(void);\n#ifdef __cplusplus",
            1,
        ),
    )
    .unwrap();
    let result = generate_all(&options(&output.0, Some(header)));
    assert!(result.is_err(), "colliding Nim names were accepted");
    assert!(result.unwrap_err().contains("Nim identifier collision"));
    assert!(!output.0.join("nim/abi_generated.nim").exists());
}

fn style_insensitive_function_names_preserve_distinct_parameter_overloads_case(
    compile_external: bool,
) {
    let output = Output::new();
    let original = options(&output.0, None);
    let source = std::fs::read_to_string(
        original
            .repo_root
            .join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let header = output.0.join("overloads.h");
    std::fs::write(&header, source.replacen(
        "#ifdef __cplusplus",
        "int32_t crepus_extra(int32_t value, void *userdata, const char *name);\nvoid crepus_overload(int32_t value);\nvoid crepusOverload(const char *value);\n#ifdef __cplusplus",
        1,
    )).unwrap();
    generate_all(&options(&output.0, Some(header))).unwrap();
    assert_generated_signatures(&output.0);
    let source = std::fs::read_to_string(output.0.join("nim/abi_generated.nim")).unwrap();
    assert!(
        source.contains(r#"proc `crepus_overload`*(`value`: int32)"#),
        "{source}"
    );
    assert!(
        source.contains(r#"proc `crepusOverload`*(`value`: cstring)"#),
        "{source}"
    );

    if compile_external {
        compile_import(&output.0, "let number: proc(value: int32) {.cdecl.} = crepus_overload\nlet text: proc(value: cstring) {.cdecl.} = crepusOverload");
    }
}

#[test]
fn style_insensitive_function_names_preserve_distinct_parameter_overloads() {
    style_insensitive_function_names_preserve_distinct_parameter_overloads_case(false);
}

#[test]
#[ignore = "requires installed Nim and a C compiler"]
fn style_insensitive_function_names_preserve_distinct_parameter_overloads_compiler_check() {
    style_insensitive_function_names_preserve_distinct_parameter_overloads_case(true);
}

#[test]
fn style_insensitive_parameter_names_fail_without_writing_bindings() {
    let output = Output::new();
    let original = options(&output.0, None);
    let source = std::fs::read_to_string(
        original
            .repo_root
            .join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let header = output.0.join("parameter-collision.h");
    std::fs::write(
        &header,
        source.replacen(
            "#ifdef __cplusplus",
            "void crepus_parameters(int32_t some_value, int32_t someValue);\n#ifdef __cplusplus",
            1,
        ),
    )
    .unwrap();
    let result = generate_all(&options(&output.0, Some(header)));
    assert!(result.is_err(), "colliding Nim parameters were accepted");
    assert!(result.unwrap_err().contains("Nim parameter collision"));
    assert!(!output.0.join("nim/abi_generated.nim").exists());
}

fn parameter_first_character_case_remains_distinct_case(compile_external: bool) {
    let output = Output::new();
    let original = options(&output.0, None);
    let source = std::fs::read_to_string(
        original
            .repo_root
            .join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let header = output.0.join("parameter-case.h");
    std::fs::write(
        &header,
        source.replacen(
            "#ifdef __cplusplus",
            "void crepus_parameters(int32_t Value, int32_t value);\n#ifdef __cplusplus",
            1,
        ),
    )
    .unwrap();
    generate_all(&options(&output.0, Some(header))).unwrap();
    assert_generated_signatures(&output.0);
    let source = std::fs::read_to_string(output.0.join("nim/abi_generated.nim")).unwrap();
    assert!(
        source.contains(r#"proc `crepus_parameters`*(`Value`: int32; `value`: int32)"#),
        "{source}"
    );

    if compile_external {
        compile_import(&output.0, "");
    }
}

#[test]
fn parameter_first_character_case_remains_distinct() {
    parameter_first_character_case_remains_distinct_case(false);
}

#[test]
#[ignore = "requires installed Nim and a C compiler"]
fn parameter_first_character_case_remains_distinct_compiler_check() {
    parameter_first_character_case_remains_distinct_case(true);
}

#[test]
fn equivalent_parameter_type_aliases_fail_without_writing_bindings() {
    for (left, right) in [
        ("char", "unsigned char"),
        ("signed char", "int8_t"),
        ("int", "int32_t"),
        ("unsigned int", "uint32_t"),
        ("short", "int16_t"),
        ("unsigned short", "uint16_t"),
        ("long long", "int64_t"),
        ("unsigned long long", "uint64_t"),
        ("int *", "int32_t *"),
        ("int **", "int32_t **"),
    ] {
        let output = Output::new();
        let original = options(&output.0, None);
        let source = std::fs::read_to_string(
            original
                .repo_root
                .join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
        )
        .unwrap()
        .replace("\r\n", "\n");
        let header = output.0.join("alias-collision.h");
        let extra = format!("void crepus_alias({left} value);\nvoid crepusAlias({right} value);\n#ifdef __cplusplus");
        std::fs::write(&header, source.replacen("#ifdef __cplusplus", &extra, 1)).unwrap();
        let result = generate_all(&options(&output.0, Some(header)));
        assert!(
            result.is_err(),
            "Nim alias collision accepted: {left}/{right}"
        );
        assert!(result.unwrap_err().contains("Nim identifier collision"));
        assert!(!output.0.join("nim/abi_generated.nim").exists());
    }
}

fn distinct_long_and_character_overloads_remain_supported_case(compile_external: bool) {
    let output = Output::new();
    let original = options(&output.0, None);
    let source = std::fs::read_to_string(
        original
            .repo_root
            .join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let header = output.0.join("long-overloads.h");
    std::fs::write(
        &header,
        source.replacen(
            "#ifdef __cplusplus",
            "void crepus_alias(long value);\nvoid crepusAlias(int64_t value);\nvoid crepus_byte(unsigned char value);\nvoid crepusByte(uint8_t value);\n#ifdef __cplusplus",
            1,
        ),
    )
    .unwrap();
    generate_all(&options(&output.0, Some(header))).unwrap();
    assert_generated_signatures(&output.0);
    let source = std::fs::read_to_string(output.0.join("nim/abi_generated.nim")).unwrap();
    assert!(
        source.contains(r#"proc `crepus_alias`*(`value`: clong)"#),
        "{source}"
    );
    assert!(
        source.contains(r#"proc `crepusAlias`*(`value`: int64)"#),
        "{source}"
    );
    assert!(
        source.contains(r#"proc `crepus_byte`*(`value`: cuchar)"#),
        "{source}"
    );
    assert!(
        source.contains(r#"proc `crepusByte`*(`value`: uint8)"#),
        "{source}"
    );

    if compile_external {
        compile_import(&output.0, "");
    }
}

#[test]
fn distinct_long_and_character_overloads_remain_supported() {
    distinct_long_and_character_overloads_remain_supported_case(false);
}

#[test]
#[ignore = "requires installed Nim and a C compiler"]
fn distinct_long_and_character_overloads_remain_supported_compiler_check() {
    distinct_long_and_character_overloads_remain_supported_case(true);
}
