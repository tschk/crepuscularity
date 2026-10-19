use crepuscularity_plugin_bindgen::{generate_all, BindgenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const LANGUAGES: [&str; 5] = ["rust", "csharp", "nim", "d", "odin"];
struct Output(PathBuf);
impl Output {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "crepus-header-bindgen-{}-{stamp}-{}",
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
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn options(output: &Path, header: PathBuf, languages: &[&str]) -> BindgenOptions {
    std::fs::create_dir_all(output).unwrap();
    let manifest = output.join("plugins.toml");
    let mut source = "[contract]\nabi_header = 'unused.h'\n".to_string();
    for language in languages {
        source.push_str(&format!(
            "[[package]]\nlanguage = '{language}'\npath = '{language}'\n"
        ));
    }
    std::fs::write(&manifest, source).unwrap();
    BindgenOptions {
        repo_root: root(),
        manifest_path: manifest,
        abi_header: Some(header),
        out_dir: Some(output.into()),
    }
}
fn normalized_output(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            !(line.starts_with("// Source:") || line.starts_with("# Source:"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn equivalent_logical_prototypes_have_identical_outputs_for_all_typed_backends() {
    let output = Output::new();
    let baseline = std::fs::read_to_string(
        root().join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap();
    let flat = format!("{baseline}\nint32_t crepus_extra(int32_t value, const char *name);\nint32_t crepus_second(void);\n");
    let multiline = format!("{baseline}\n/* 多行 UTF-8 comment\n   before return type */\nint32_t\n/* return/name separation */ crepus_extra(\n    int32_t /* 雪 */ value,\n    const char /* multi\n       line */ *name\n); int32_t crepus_second(void); // shared line\n");
    let multiline = format!(
        "// This declaration remains commented after C line splicing \\\r\nint32_t crepus_not_exported(void);\n/\\\n* int32_t crepus_also_not_exported(void); *\\\n/\n{multiline}"
    );
    for (name, source) in [("flat", flat), ("multiline", multiline)] {
        let dir = output.0.join(name);
        std::fs::create_dir(&dir).unwrap();
        // C# derives its library name from this basename, which stays identical.
        let header = dir.join("crepuscularity_abi.h");
        std::fs::write(&header, source).unwrap();
        let files = generate_all(&options(&dir.join("generated"), header, &LANGUAGES)).unwrap();
        assert_eq!(files.len(), LANGUAGES.len());
    }
    for (language, filename) in [
        ("rust", "abi_generated.rs"),
        ("csharp", "AbiGenerated.cs"),
        ("nim", "abi_generated.nim"),
        ("d", "abi_generated.d"),
        ("odin", "abi_generated.odin"),
    ] {
        let flat = std::fs::read_to_string(
            output
                .0
                .join("flat/generated")
                .join(language)
                .join(filename),
        )
        .unwrap();
        let multiline = std::fs::read_to_string(
            output
                .0
                .join("multiline/generated")
                .join(language)
                .join(filename),
        )
        .unwrap();
        assert_eq!(
            normalized_output(&flat),
            normalized_output(&multiline),
            "{language} changed a logical signature"
        );
    }
    let generated = output.0.join("multiline/generated/rust/abi_generated.rs");
    let check = output.0.join("check.rs");
    std::fs::write(
        &check,
        format!(
            r#"#[path = {generated:?}] mod generated;
use std::ffi::c_char;
const _: unsafe extern "C" fn(i32, *const c_char) -> i32 = generated::crepus_extra;
const _: unsafe extern "C" fn() -> i32 = generated::crepus_second;
fn main() {{}}
"#
        ),
    )
    .unwrap();
    let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2021", "--emit=metadata"])
        .arg(check)
        .arg("-o")
        .arg(output.0.join("check.rmeta"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn ambiguous_or_incomplete_prototypes_fail_without_writing_any_target_output() {
    let baseline = std::fs::read_to_string(
        root().join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap();
    for (case, declaration) in [
        ("unnamed_scalar", "int32_t crepus_extra(int32_t);"),
        ("unnamed_long_long", "int32_t crepus_extra(long long);"),
        (
            "unnamed_unsigned_long_long",
            "int32_t crepus_extra(unsigned long long);",
        ),
        (
            "unnamed_qualified_pointer",
            "int32_t crepus_extra(const char *);",
        ),
        ("missing_terminator", "int32_t crepus_extra(int32_t value)"),
        ("reversed_parentheses", "int32_t crepus_extra)(;"),
        (
            "unbalanced_parameters",
            "int32_t crepus_extra((int32_t value);",
        ),
        ("missing_return_type", "crepus_extra(int32_t value);"),
        (
            "struct_return_omission",
            "struct Missing *crepus_extra(void);",
        ),
        ("enum_return_omission", "enum Missing crepus_extra(void);"),
        (
            "quoted_brace_in_unsupported_struct",
            r#"struct Missing { char x[sizeof("}")]; }; int32_t crepus_extra(struct Missing *value);"#,
        ),
        (
            "inline_function_pointer",
            "void crepus_extra(void (*callback)(int32_t), void *userdata);",
        ),
    ] {
        for language in LANGUAGES {
            let output = Output::new();
            let header = output.0.join("crepuscularity_abi.h");
            std::fs::write(&header, format!("{baseline}\n{declaration}\n")).unwrap();
            let config = options(&output.0.join("generated"), header, &[language]);
            assert!(generate_all(&config).is_err(), "{language} accepted {case}");
            let package = output.0.join("generated").join(language);
            assert!(
                !package.exists() || std::fs::read_dir(package).unwrap().next().is_none(),
                "{language}/{case} wrote output"
            );
        }
    }
}

#[test]
fn raw_crlf_custom_headers_preserve_extra_exports_for_all_typed_backends() {
    let output = Output::new();
    let baseline = std::fs::read_to_string(
        root().join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let custom = baseline.replace(
        "#ifdef __cplusplus\n}",
        "int32_t crepus_crlf_extra(int32_t value, const char *name);\n\n#ifdef __cplusplus\n}",
    );
    assert_ne!(
        custom, baseline,
        "the extra export must actually be inserted"
    );
    for (name, source) in [
        ("lf", custom.clone()),
        ("crlf", custom.replace('\n', "\r\n")),
    ] {
        if name == "crlf" {
            assert!(source.contains("\r\n"));
            assert!(!source.replace("\r\n", "").contains('\n'));
        }
        let dir = output.0.join(name);
        std::fs::create_dir(&dir).unwrap();
        let header = dir.join("crepuscularity_abi.h");
        std::fs::write(&header, &source).unwrap();
        assert_eq!(std::fs::read_to_string(&header).unwrap(), source);
        let files = generate_all(&options(&dir.join("generated"), header, &LANGUAGES)).unwrap();
        assert_eq!(files.len(), LANGUAGES.len());
    }
    for (language, filename) in [
        ("rust", "abi_generated.rs"),
        ("csharp", "AbiGenerated.cs"),
        ("nim", "abi_generated.nim"),
        ("d", "abi_generated.d"),
        ("odin", "abi_generated.odin"),
    ] {
        let read = |ending: &str| {
            std::fs::read_to_string(
                output
                    .0
                    .join(ending)
                    .join("generated")
                    .join(language)
                    .join(filename),
            )
            .unwrap()
        };
        let lf = read("lf");
        let crlf = read("crlf");
        for export in [
            "crepus_crlf_extra",
            "crepus_session_new",
            "crepus_string_free",
        ] {
            assert!(crlf.contains(export), "{language} dropped {export}: {crlf}");
        }
        assert_eq!(
            normalized_output(&crlf),
            normalized_output(&lf),
            "{language} changed CRLF signatures"
        );
    }
}

#[test]
fn rust_keyword_c_exports_keep_their_identity_across_typed_targets() {
    let output = Output::new();
    let header = output.0.join("keywords.h");
    std::fs::write(
        &header,
        "int32_t match(int32_t type);\nconst char *async(const void *type);\n",
    )
    .unwrap();
    let files = generate_all(&options(&output.0.join("generated"), header, &LANGUAGES)).unwrap();
    assert_eq!(files.len(), LANGUAGES.len());
    for file in files {
        let code = std::fs::read_to_string(&file.path).unwrap();
        assert!(
            code.contains("match"),
            "{} dropped match: {code}",
            file.language
        );
        assert!(
            code.contains("async"),
            "{} dropped async: {code}",
            file.language
        );
        if file.language == "rust" {
            let syntax = syn::parse_file(&code).unwrap();
            let mut names = Vec::new();
            for item in syntax.items {
                if let syn::Item::ForeignMod(module) = item {
                    for item in module.items {
                        if let syn::ForeignItem::Fn(function) = item {
                            names.push(function.sig.ident.to_string());
                        }
                    }
                }
            }
            assert_eq!(names, ["r#match", "r#async"]);
            assert!(code.contains("*const c_char"), "{code}");
            assert!(code.contains("*const c_void"), "{code}");
        } else {
            assert!(
                !code.contains("r#"),
                "{} leaked Rust syntax: {code}",
                file.language
            );
        }
    }
}

#[test]
fn ambiguous_parameters_and_unsupported_layouts_fail_before_any_output() {
    let baseline = std::fs::read_to_string(
        root().join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    for (declaration, diagnostic) in [
        ("int32_t probe();", "use (void)"),
        ("int32_t probe( /* no prototype */ );", "use (void)"),
        ("typedef void (*Callback)();", "use (void)"),
        (
            "typedef struct Foo { const char *name; } Foo;",
            "unsupported concrete",
        ),
        ("struct Foo { char const *name; };", "unsupported concrete"),
        (
            "typedef struct __attribute__((packed)) Foo { char a; int b; } Foo;",
            "unsupported concrete",
        ),
        (
            "typedef struct Node { struct Node *next; } Node;",
            "unsupported concrete",
        ),
        ("extern int counter;", "non-function ABI"),
        ("int counter;", "non-function ABI"),
        ("int values[4];", "non-function ABI"),
        (
            "union Foo { int32_t number; float decimal; };",
            "unsupported concrete",
        ),
        ("typedef const char *Name;", "qualified non-function"),
        ("typedef char*const Name;", "qualified non-function"),
        ("const char *global_name;", "qualified non-function"),
        ("enum Mode { A = 0, B = 1 };", "unsupported concrete"),
        (
            "typedef enum Mode { A = 0, B = 1 } Mode;\nMode probe(void);",
            "unsupported concrete",
        ),
        ("typedef enum Mode Mode;", "unsupported concrete"),
    ] {
        for language in LANGUAGES {
            let output = Output::new();
            let header = output.0.join("custom.h");
            std::fs::write(&header, format!("{baseline}\n{declaration}\n")).unwrap();
            let dir = output.0.join("generated");
            let error = generate_all(&options(&dir, header, &[language])).unwrap_err();
            assert!(
                error.contains(diagnostic),
                "{language}/{declaration}: {error}"
            );
            let entries: Vec<_> = std::fs::read_dir(&dir)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect();
            assert_eq!(
                entries,
                [std::ffi::OsString::from("plugins.toml")],
                "{language}/{declaration} wrote output"
            );
        }
    }
}

#[test]
fn explicit_void_parameter_list_remains_admitted() {
    let output = Output::new();
    let header = output.0.join("void.h");
    std::fs::write(&header, "int32_t probe(void);\n").unwrap();
    let files = generate_all(&options(&output.0.join("generated"), header, &LANGUAGES)).unwrap();
    assert_eq!(files.len(), LANGUAGES.len());
    for file in files {
        assert!(std::fs::read_to_string(&file.path)
            .unwrap()
            .contains("probe"));
    }
}
