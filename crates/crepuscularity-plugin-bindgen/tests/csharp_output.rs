use std::path::{Path, PathBuf};
use std::process::Command;
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
            "crepus-csharp-bindgen-{}-{stamp}-{}",
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

fn compile_csharp(generated: &Path, check: &Path, output: &Path) {
    let installed = Command::new("dotnet")
        .arg("--list-sdks")
        .output()
        .expect("installed .NET 8 SDK is required to verify generated C#");
    assert!(installed.status.success(), "dotnet --list-sdks failed");
    let listing = String::from_utf8(installed.stdout).unwrap();
    let (version, sdk_base) = listing
        .lines()
        .filter_map(|line| {
            let (version, base) = line.split_once(" [")?;
            version
                .starts_with("8.")
                .then(|| (version, PathBuf::from(base.trim_end_matches(']'))))
        })
        .next_back()
        .expect("installed .NET 8 SDK is required to verify generated C#");
    let compiler = sdk_base.join(version).join("Roslyn/bincore/csc.dll");
    let packs = sdk_base
        .parent()
        .unwrap()
        .join("packs/Microsoft.NETCore.App.Ref");
    let mut versions: Vec<_> = std::fs::read_dir(&packs)
        .expect("installed .NET reference packs")
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("8.")
        })
        .collect();
    versions.sort_by_key(|path| {
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect::<Vec<_>>()
    });
    let references = versions
        .last()
        .expect("installed .NET 8 reference pack")
        .join("ref/net8.0");
    let mut command = Command::new("dotnet");
    command
        .arg(compiler)
        .args(["-nologo", "-noconfig", "-nostdlib+", "-target:library"]);
    command.arg(format!("-out:{}", output.display()));
    for entry in std::fs::read_dir(references).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "dll") {
            command.arg(format!("-r:{}", path.display()));
        }
    }
    let compiled = command
        .arg(generated)
        .arg(check)
        .output()
        .expect("run offline C# compiler");
    assert!(
        compiled.status.success(),
        "generated C# ABI signature compilation failed:\n{}\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
}

fn assert_csharp_output(header: Option<PathBuf>, functions: usize, extra_checks: &str) {
    let library = header
        .as_deref()
        .and_then(Path::file_stem)
        .and_then(|stem| stem.to_str())
        .unwrap_or("crepuscularity_abi")
        .to_string();
    let named_argument_check = if header.is_none() {
        "static int NamedCallback(IntPtr session, CrepusEventCallback callback) => EquilibriumImports.crepus_session_set_event_callback(session: session, callback: callback, userdata: IntPtr.Zero);\nstatic int NamedClear(IntPtr session) => EquilibriumImports.crepus_session_set_event_callback(session: session, callback: null, userdata: IntPtr.Zero);"
    } else {
        ""
    };
    let output = TempOutput::new();
    let manifest = output.0.join("plugins.toml");
    std::fs::write(&manifest, "[contract]\nabi_header = 'crates/crepuscularity-abi/include/crepuscularity_abi.h'\n[[package]]\nlanguage = 'csharp'\npath = 'csharp'\n").unwrap();
    let files = generate_all(&BindgenOptions {
        repo_root: repo_root(),
        manifest_path: manifest,
        abi_header: header,
        out_dir: Some(output.0.clone()),
    })
    .expect("generate fresh C# bindings");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, output.0.join("csharp/AbiGenerated.cs"));
    let generated = std::fs::read_to_string(&files[0].path).unwrap();
    let imports: Vec<_> = generated
        .lines()
        .filter(|line| line.contains("[DllImport("))
        .collect();
    assert_eq!(imports.len(), functions, "{generated}");
    for import in imports {
        assert!(import.contains("CallingConvention.Cdecl"), "{import}");
        assert!(
            import.contains(&format!("DllImport(@\"{}\"", library.replace('"', "\"\"")))
                || import.contains(&format!("DllImport(\"{library}\"")),
            "{import}"
        );
    }
    let check = output.0.join("Check.cs");
    std::fs::write(&check, format!(r#"#nullable enable
using System;
internal static class Check {{
    static readonly Func<IntPtr> New = EquilibriumImports.crepus_session_new;
    static readonly Action<IntPtr> Free = EquilibriumImports.crepus_session_free;
    static readonly Func<IntPtr, IntPtr, IntPtr, int> Template = EquilibriumImports.crepus_session_set_template_string;
    static readonly Func<IntPtr, IntPtr, int> Component = EquilibriumImports.crepus_session_set_component;
    static readonly Func<IntPtr, IntPtr, int> Files = EquilibriumImports.crepus_session_set_files_json;
    static readonly Func<IntPtr, IntPtr, int> Context = EquilibriumImports.crepus_session_set_context_json;
    static readonly Func<IntPtr, IntPtr, int> Patch = EquilibriumImports.crepus_session_apply_context_patch_json;
    static readonly Func<IntPtr, CrepusEventCallback?, IntPtr, int> Callback = EquilibriumImports.crepus_session_set_event_callback;
    static readonly Func<IntPtr, IntPtr> Render = EquilibriumImports.crepus_session_render_ir_json;
    static readonly Func<IntPtr, IntPtr, IntPtr> Dispatch = EquilibriumImports.crepus_session_dispatch_event_json;
    static readonly Func<IntPtr, IntPtr> TakeError = EquilibriumImports.crepus_session_take_last_error;
    static readonly Func<IntPtr> LastError = EquilibriumImports.crepus_last_error;
    static readonly Action<IntPtr> FreeString = EquilibriumImports.crepus_string_free;
    static int ClearCallback(IntPtr session) => EquilibriumImports.crepus_session_set_event_callback(session, null, IntPtr.Zero);
    {extra_checks}
    {named_argument_check}
}}
"#)).unwrap();
    compile_csharp(&files[0].path, &check, &output.0.join("Generated.dll"));
}

#[test]
fn generated_csharp_compiles_with_all_abi_function_signatures() {
    assert_csharp_output(None, 13, "");
}

#[test]
fn overridden_csharp_header_keeps_extra_pointer_and_scalar_functions() {
    let output = TempOutput::new();
    let header = output.0.join("custom.h");
    let original = std::fs::read_to_string(
        repo_root().join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let custom = original.replace("#ifdef __cplusplus\n}", "const char *crepus_extra(const void *userdata);\nint32_t crepus_extra_status(int32_t value);\nvoid CallingConvention(void);\n\n#ifdef __cplusplus\n}");
    assert_ne!(custom, original);
    std::fs::write(&header, custom).unwrap();
    assert_csharp_output(Some(header), 16, "static readonly Func<IntPtr, IntPtr> Extra = EquilibriumImports.crepus_extra;\nstatic readonly Func<int, int> ExtraStatus = EquilibriumImports.crepus_extra_status;");
}

#[test]
fn csharp_fixed_width_scalars_keep_their_native_calling_types() {
    let output = TempOutput::new();
    let header = output.0.join("scalars.h");
    let original = std::fs::read_to_string(
        repo_root().join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let custom = original.replace("#ifdef __cplusplus\n}", "uint32_t crepus_unsigned(uint32_t value);\nfloat crepus_float(float value);\ndouble crepus_double(double value);\nint64_t crepus_signed_wide(int64_t value);\nuint64_t crepus_unsigned_wide(uint64_t value);\n\n#ifdef __cplusplus\n}");
    assert_ne!(custom, original);
    std::fs::write(&header, custom).unwrap();
    assert_csharp_output(Some(header), 18, "static readonly Func<uint,uint> Unsigned = EquilibriumImports.crepus_unsigned;\nstatic readonly Func<float,float> Float = EquilibriumImports.crepus_float;\nstatic readonly Func<double,double> Double = EquilibriumImports.crepus_double;\nstatic readonly Func<long,long> SignedWide = EquilibriumImports.crepus_signed_wide;\nstatic readonly Func<ulong,ulong> UnsignedWide = EquilibriumImports.crepus_unsigned_wide;");
}

#[test]
fn csharp_rejects_unsupported_declarations_before_writing_bindings() {
    let original = std::fs::read_to_string(
        repo_root().join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let incompatible = original.replace(
        "typedef void (*CrepusEventCallback)(const char *event_json, void *userdata);",
        "typedef int32_t (*CrepusEventCallback)(const char *event_json, void *userdata);",
    );
    let unsupported = original.replace(
        "#ifdef __cplusplus\n}",
        "typedef int MyStatus;\nMyStatus crepus_status_alias(void);\n\n#ifdef __cplusplus\n}",
    );
    let ambiguous = original.replace(
        "#ifdef __cplusplus\n}",
        "long crepus_platform_long(long value);\n\n#ifdef __cplusplus\n}",
    );
    let enclosing_type_collision = original.replace(
        "#ifdef __cplusplus\n}",
        "void EquilibriumImports(void);\n\n#ifdef __cplusplus\n}",
    );
    for rejected in [
        incompatible,
        unsupported,
        ambiguous,
        enclosing_type_collision,
    ] {
        assert_ne!(rejected, original);
        let output = TempOutput::new();
        let header = output.0.join("custom.h");
        std::fs::write(&header, rejected).unwrap();
        let manifest = output.0.join("plugins.toml");
        std::fs::write(&manifest,
            "[contract]\nabi_header = 'unused.h'\n[[package]]\nlanguage = 'csharp'\npath = 'csharp'\n",
        ).unwrap();
        assert!(
            generate_all(&BindgenOptions {
                repo_root: repo_root(),
                manifest_path: manifest,
                abi_header: Some(header),
                out_dir: Some(output.0.clone()),
            })
            .is_err(),
            "unsupported declarations must not silently drop ABI functions"
        );
        assert!(!output.0.join("csharp/AbiGenerated.cs").exists());
    }
}

#[cfg(unix)]
#[test]
fn quoted_header_name_uses_typed_escaping_without_relaxing_canonical_policy() {
    let output = TempOutput::new();
    let header = output.0.join("custom\"abi.h");
    let original = std::fs::read_to_string(
        repo_root().join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    std::fs::write(&header, &original).unwrap();
    let manifest = output.0.join("plugins.toml");
    std::fs::write(
        &manifest,
        "[contract]\nabi_header = 'unused.h'\n[[package]]\nlanguage = 'csharp'\npath = 'csharp'\n",
    )
    .unwrap();
    let error = generate_all(&BindgenOptions {
        repo_root: repo_root(),
        manifest_path: manifest,
        abi_header: Some(header.clone()),
        out_dir: Some(output.0.clone()),
    })
    .expect_err("canonical header name policy remains strict");
    assert!(error.contains("header name is not safe"), "{error}");
    assert!(!output.0.join("csharp/AbiGenerated.cs").exists());

    let custom = format!("{original}\nint32_t crepus_extra(int32_t value);\n");
    std::fs::write(&header, custom).unwrap();
    assert_csharp_output(
        Some(header),
        14,
        "static readonly Func<int, int> Extra = EquilibriumImports.crepus_extra;",
    );
}
