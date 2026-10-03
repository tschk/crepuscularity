use crepuscularity_plugin_bindgen::{generate_all, BindgenOptions};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const HEADER: &str = include_str!("../src/crepuscularity_abi.h");
const NAMES: &[&str] = &[
    "crepus_session_new",
    "crepus_session_free",
    "crepus_session_set_template_string",
    "crepus_session_set_component",
    "crepus_session_set_files_json",
    "crepus_session_set_context_json",
    "crepus_session_apply_context_patch_json",
    "crepus_session_set_event_callback",
    "crepus_session_render_ir_json",
    "crepus_session_dispatch_event_json",
    "crepus_session_take_last_error",
    "crepus_last_error",
    "crepus_string_free",
];

fn fixture(root: &Path, header: &str, languages: &[&str]) -> BindgenOptions {
    fs::write(root.join("crepuscularity_abi.h"), header).unwrap();
    let mut manifest = "[contract]\nabi_header = 'crepuscularity_abi.h'\n".to_string();
    for language in languages {
        manifest.push_str(&format!(
            "\n[[package]]\nlanguage = '{language}'\npath = '{language}'\n"
        ));
    }
    let manifest_path = root.join("plugins.toml");
    fs::write(&manifest_path, manifest).unwrap();
    BindgenOptions {
        repo_root: root.into(),
        manifest_path,
        abi_header: None,
        out_dir: Some(root.join("out")),
    }
}

fn run(command: &mut Command) {
    let result = command
        .output()
        .unwrap_or_else(|e| panic!("{command:?}: {e}"));
    assert!(
        result.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn canonical_inventory_and_types_are_complete() {
    let dir = tempfile::tempdir().unwrap();
    let opts = fixture(
        dir.path(),
        HEADER,
        &["c", "cpp", "zig", "rust", "csharp", "v"],
    );
    let files = generate_all(&opts).unwrap();
    assert_eq!(files.len(), 6);
    for file in files {
        let code = fs::read_to_string(&file.path).unwrap();
        for name in NAMES {
            assert!(code.contains(name), "{} is missing {name}", file.language);
        }
        match file.language.as_str() {
            "rust" => {
                assert!(code.contains("Option<unsafe extern \"C\" fn(*const c_char, *mut c_void)>"));
                assert!(code.contains("-> i32;"));
                assert!(code.contains("-> *mut c_char;"));
            }
            "csharp" => {
                assert!(code.contains("extern int crepus_session_set_event_callback"));
                assert!(code.contains("CrepusEventCallback? callback"));
                assert!(code.contains("extern IntPtr crepus_session_render_ir_json"));
                assert_eq!(code.matches("CallingConvention.Cdecl").count(), 14);
            }
            "v" => assert!(code.contains("fn C.crepus_session_set_component(session &C.CrepusSession, component_utf8 &char) i32")),
            _ => {}
        }
    }
}

#[test]
fn whitespace_comments_and_multiline_declarations_preserve_the_schema() {
    let dir = tempfile::tempdir().unwrap();
    let expanded = HEADER
        .replace(
            "CrepusSession *session",
            "CrepusSession/*handle*/\n* session",
        )
        .replace("(void)", "(\n void \n)");
    let opts = fixture(dir.path(), &expanded, &["rust"]);
    assert_eq!(generate_all(&opts).unwrap().len(), 1);
}

#[test]
fn changed_or_extra_crepus_declarations_fail_before_writing() {
    let variants = [
        HEADER.replace("void *userdata", "long userdata"),
        HEADER.replace("CrepusSession *session", "Unknown *session"),
        HEADER.replace("void crepus_string_free(char *ptr);", ""),
        format!("{HEADER}\nint unexpected(int value);\n"),
        HEADER.replace("crepus_session_free", "crepus_session_free$bad"),
        HEADER.replace("const char *event_json", "const char **event_json"),
        HEADER.replace(
            "#include <stdint.h>",
            "#include <stdint.h>\n#define int32_t long",
        ),
    ];
    for header in variants {
        let dir = tempfile::tempdir().unwrap();
        let opts = fixture(dir.path(), &header, &["rust"]);
        assert!(generate_all(&opts)
            .unwrap_err()
            .contains("unsupported Crepus ABI"));
        assert!(!dir.path().join("out").exists());
    }
}

#[test]
fn unsupported_fallback_signature_is_an_error_with_no_partial_writes() {
    let dir = tempfile::tempdir().unwrap();
    let header = "typedef struct Bad Bad;\nBad unsupported_by_value(void);\nvoid primitive_control(int value);\n";
    let opts = fixture(dir.path(), header, &["python", "c"]);
    let existing = dir.path().join("out/python/abi_generated.py");
    fs::create_dir_all(existing.parent().unwrap()).unwrap();
    fs::write(&existing, "preserve this user's file\n").unwrap();
    let error = generate_all(&opts).unwrap_err();
    assert!(error.contains("unsupported_by_value"), "{error}");
    assert_eq!(
        fs::read_to_string(existing).unwrap(),
        "preserve this user's file\n"
    );
    assert!(!dir.path().join("out/c").exists());
}

#[test]
fn fallback_parser_omissions_fail_without_replacing_outputs() {
    for header in [
        "typedef struct Bad Bad;\nBad unsupported_by_value(\n void\n);\nvoid primitive_control(int value);\n",
        "void unsupported_by_value(Bad);\nvoid primitive_control(int value);\n",
        "int unnamed(int);\nvoid primitive_control(int value);\n",
        "int variadic(int value, ...);\n",
        "int first(void); int second(void);\n",
        "int\ttabbed(int value);\n",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let opts = fixture(dir.path(), header, &["python", "c"]);
        let existing = dir.path().join("out/python/abi_generated.py");
        fs::create_dir_all(existing.parent().unwrap()).unwrap();
        fs::write(&existing, "preserve this file").unwrap();
        assert!(generate_all(&opts).unwrap_err().contains("unsupported scalar ABI declaration"));
        assert_eq!(fs::read_to_string(existing).unwrap(), "preserve this file");
        assert!(!dir.path().join("out/c").exists());
    }
}

#[test]
fn escaped_newlines_cannot_hide_contract_changes_inside_comments() {
    for splice in ["\\\n", "\\\r\n", "??/\n"] {
        let header = HEADER.replace(
            "#include <stdint.h>",
            &format!("#include <stdint.h>\n/*\n*{splice}/\n#define int32_t int64_t\n/*\n*/"),
        );
        let dir = tempfile::tempdir().unwrap();
        let opts = fixture(dir.path(), &header, &["rust", "csharp"]);
        assert!(generate_all(&opts).unwrap_err().contains("line splicing"));
        assert!(!dir.path().join("out").exists());
    }
}

#[test]
fn unknown_later_backend_does_not_replace_earlier_files() {
    let dir = tempfile::tempdir().unwrap();
    let opts = fixture(dir.path(), HEADER, &["rust", "unknown"]);
    assert!(generate_all(&opts).is_err());
    assert!(!dir.path().join("out").exists());
}

#[test]
fn primitive_fallback_and_repository_typescript_route_remain_available() {
    let dir = tempfile::tempdir().unwrap();
    let opts = fixture(
        dir.path(),
        "int primitive_control(int value);\n",
        &["c", "typescript"],
    );
    let files = generate_all(&opts).unwrap();
    let c = fs::read_to_string(&files[0].path).unwrap();
    assert!(c.contains("int eq_crepuscularity_abi_primitive_control(int value)"));
    let ts = fs::read_to_string(&files[1].path).unwrap();
    assert!(ts.contains("koffi"));
    assert!(!ts.contains("--ffi"));
}

#[test]
fn checked_contract_tracks_the_exported_header_in_the_workspace() {
    let exported = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crepuscularity-abi/include/crepuscularity_abi.h");
    // The sibling crate is absent in a crates.io package. The package carries
    // the contract; the workspace gate additionally detects upstream drift.
    if exported.exists() {
        assert_eq!(fs::read_to_string(exported).unwrap(), HEADER);
    }
}

#[test]
fn generated_rust_types_compile_as_foreign_function_signatures() {
    let dir = tempfile::tempdir().unwrap();
    let opts = fixture(dir.path(), HEADER, &["rust"]);
    let generated = generate_all(&opts).unwrap().remove(0).path;
    let code = format!(
        r#"include!({generated:?});
const _: unsafe extern "C" fn() -> *mut CrepusSession = crepus_session_new;
const _: unsafe extern "C" fn(*mut CrepusSession) = crepus_session_free;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char, *const c_char) -> i32 = crepus_session_set_template_string;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char) -> i32 = crepus_session_set_component;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char) -> i32 = crepus_session_set_files_json;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char) -> i32 = crepus_session_set_context_json;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char) -> i32 = crepus_session_apply_context_patch_json;
const _: unsafe extern "C" fn(*mut CrepusSession, CrepusEventCallback, *mut c_void) -> i32 = crepus_session_set_event_callback;
const _: unsafe extern "C" fn(*mut CrepusSession) -> *mut c_char = crepus_session_render_ir_json;
const _: unsafe extern "C" fn(*mut CrepusSession, *const c_char) -> *mut c_char = crepus_session_dispatch_event_json;
const _: unsafe extern "C" fn(*mut CrepusSession) -> *mut c_char = crepus_session_take_last_error;
const _: unsafe extern "C" fn() -> *mut c_char = crepus_last_error;
const _: unsafe extern "C" fn(*mut c_char) = crepus_string_free;
const _: CrepusEventCallback = None;
"#
    );
    let source = dir.path().join("signatures.rs");
    fs::write(&source, code).unwrap();
    run(Command::new("rustc")
        .args([
            "--edition=2021",
            "--crate-type=lib",
            "--emit=metadata",
            "-Dwarnings",
        ])
        .arg(source)
        .arg("-o")
        .arg(dir.path().join("signatures.rmeta")));
}

#[cfg(unix)]
#[test]
fn c_and_cpp_wrappers_compile_against_the_real_abi_declarations() {
    let dir = tempfile::tempdir().unwrap();
    let opts = fixture(dir.path(), HEADER, &["c", "cpp"]);
    for file in generate_all(&opts).unwrap() {
        let compiler = if file.language == "cpp" { "c++" } else { "cc" };
        run(Command::new(compiler)
            .args(["-Werror", "-Wall", "-Wextra", "-c", "-I"])
            .arg(dir.path())
            .arg(file.path)
            .arg("-o")
            .arg(dir.path().join(format!("{}.o", file.language))));
    }
}

#[test]
#[ignore = "requires the Zig compiler"]
fn generated_zig_bindings_compile() {
    let dir = tempfile::tempdir().unwrap();
    let opts = fixture(dir.path(), HEADER, &["zig"]);
    let generated = generate_all(&opts).unwrap().remove(0).path;
    let mut code = "const abi = @import(\"out/zig/abi_generated.zig\");\ncomptime {\n".to_string();
    for name in NAMES {
        code.push_str(&format!("    _ = abi.{name};\n"));
    }
    code.push_str("}\n");
    let source = dir.path().join("signatures.zig");
    fs::write(&source, code).unwrap();
    assert!(generated.exists());
    run(Command::new("zig")
        .args(["build-obj", "-lc", "-I"])
        .arg(dir.path())
        .arg(&source)
        .arg(format!(
            "-femit-bin={}",
            dir.path().join("signatures.o").display()
        ))
        .arg("--cache-dir")
        .arg(dir.path().join("zig-cache"))
        .arg("--global-cache-dir")
        .arg(dir.path().join("zig-global-cache")));
}

#[test]
#[ignore = "requires V, a C compiler, and a built crepuscularity-abi in CREPUS_ABI_LIB_DIR"]
fn generated_v_bindings_compile() {
    let library_dir = std::env::var("CREPUS_ABI_LIB_DIR")
        .expect("build crepuscularity-abi and set CREPUS_ABI_LIB_DIR");
    let dir = tempfile::Builder::new()
        .prefix("crepus bindings ")
        .tempdir()
        .unwrap();
    let opts = fixture(dir.path(), HEADER, &["v"]);
    let generated = generate_all(&opts).unwrap().remove(0).path;
    let mut code = fs::read_to_string(generated).unwrap();
    code.push_str("\nfn main() {\n");
    for name in NAMES {
        code.push_str(&format!("    _ = C.{name}\n"));
    }
    code.push_str("}\n");
    let source = dir.path().join("signatures.v");
    fs::write(&source, code).unwrap();
    run(Command::new("v")
        .args(["-gc", "none", "-show-c-output", "-ldflags"])
        .arg(format!("-L\"{library_dir}\" -lcrepuscularity_abi"))
        .arg("-o")
        .arg(dir.path().join("signatures"))
        .arg(source));
}

#[test]
#[ignore = "requires the .NET 8 SDK"]
fn generated_csharp_bindings_compile() {
    let dir = tempfile::tempdir().unwrap();
    let opts = fixture(dir.path(), HEADER, &["csharp"]);
    generate_all(&opts).unwrap();
    let project = dir.path().join("Bindings.csproj");
    fs::write(&project, "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><TargetFramework>net8.0</TargetFramework><TreatWarningsAsErrors>true</TreatWarningsAsErrors></PropertyGroup></Project>").unwrap();
    let config = dir.path().join("NuGet.Config");
    fs::write(
        &config,
        "<configuration><packageSources><clear /></packageSources></configuration>",
    )
    .unwrap();
    run(Command::new("dotnet")
        .args(["restore", "--configfile"])
        .arg(config)
        .arg(&project));
    run(Command::new("dotnet")
        .args(["build", "--no-restore"])
        .arg(project));
}

#[test]
#[ignore = "requires a built crepuscularity-abi in CREPUS_ABI_LIB_DIR"]
fn generated_rust_bindings_run_the_real_session_lifecycle() {
    let library_dir = std::env::var_os("CREPUS_ABI_LIB_DIR")
        .expect("build crepuscularity-abi and set CREPUS_ABI_LIB_DIR");
    let dir = tempfile::tempdir().unwrap();
    let opts = fixture(dir.path(), HEADER, &["rust"]);
    let generated = generate_all(&opts).unwrap().remove(0).path;
    let code = format!(
        r#"include!({generated:?});
use std::ffi::{{CStr, CString}};
unsafe extern "C" fn capture(event: *const c_char, userdata: *mut c_void) {{
    assert!(CStr::from_ptr(event).to_str().unwrap().contains("bind:count:2"));
    *(userdata as *mut usize) += 1;
}}
unsafe fn owned(ptr: *mut c_char) -> String {{
    assert!(!ptr.is_null());
    let text = CStr::from_ptr(ptr).to_str().unwrap().to_owned();
    crepus_string_free(ptr);
    text
}}
fn main() {{ unsafe {{
    let session = crepus_session_new();
    assert!(!session.is_null());
    assert!(crepus_session_render_ir_json(session).is_null());
    assert!(owned(crepus_session_take_last_error(session)).contains("session source is not set"));
    let template = CString::new("input bind=count\nspan\n  \"Count {{count}}\"").unwrap();
    let context = CString::new("{{\"count\":\"1\"}}").unwrap();
    assert_eq!(crepus_session_set_template_string(session, template.as_ptr(), std::ptr::null()), 0);
    assert_eq!(crepus_session_set_context_json(session, context.as_ptr()), 0);
    assert!(owned(crepus_session_render_ir_json(session)).contains("Count 1"));
    let mut calls = 0usize;
    assert_eq!(crepus_session_set_event_callback(session, Some(capture), (&mut calls as *mut usize).cast()), 0);
    let event = CString::new("{{\"handler\":\"bind:count:2\"}}").unwrap();
    assert!(owned(crepus_session_dispatch_event_json(session, event.as_ptr())).contains("Count 2"));
    assert_eq!(calls, 1);
    assert_eq!(crepus_session_set_event_callback(session, None, std::ptr::null_mut()), 0);
    assert!(owned(crepus_session_dispatch_event_json(session, event.as_ptr())).contains("Count 2"));
    assert_eq!(calls, 1);
    crepus_session_free(session);
}} }}
"#
    );
    let source = dir.path().join("lifecycle.rs");
    let binary = dir.path().join("lifecycle");
    fs::write(&source, code).unwrap();
    run(Command::new("rustc")
        .args(["--edition=2021", "-Dwarnings", "-L"])
        .arg(&library_dir)
        .args(["-l", "dylib=crepuscularity_abi"])
        .arg(source)
        .arg("-o")
        .arg(&binary));
    run(Command::new(binary)
        .env("DYLD_LIBRARY_PATH", &library_dir)
        .env("LD_LIBRARY_PATH", &library_dir));
}
