//! Checked adapter for the complete Crepus ABI, whose opaque handles and
//! callbacks are outside equilibrium's scalar-only import contract.

use std::fmt::Write as _;
use std::io::Read;
use std::path::Path;

use equilibrium_ffi::Language;

pub(crate) const HEADER: &str = include_str!("crepuscularity_abi.h");

pub(crate) struct CheckedHeader {
    pub canonical: bool,
    pub fallback_error: Option<String>,
}

pub(crate) fn check_header(path: &Path) -> Result<CheckedHeader, String> {
    // Keep the dependency's header limits, including a bounded read if the
    // file grows after metadata was inspected.
    const MAX_BYTES: u64 = 10 * 1024 * 1024;
    let mut content = String::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(MAX_BYTES + 1).read_to_string(&mut content))
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    if content.len() as u64 > MAX_BYTES || content.lines().count() > 200_000 {
        return Err(format!(
            "header exceeds binding input limits: {}",
            path.display()
        ));
    }
    // C splices escaped physical newlines before recognizing comments. Reject
    // that representation instead of comparing a different logical header.
    if content.contains("\\\n") || content.contains("\\\r") || content.contains("??/") {
        return Err("unsupported header line splicing; no bindings written".into());
    }
    let actual = tokens(&content)?;
    let expected = tokens(HEADER).expect("bundled ABI header is valid");
    if actual == expected {
        return Ok(CheckedHeader {
            canonical: true,
            fallback_error: None,
        });
    }
    if actual.iter().any(|token| {
        matches!(*token, "CrepusSession" | "CrepusEventCallback") || token.starts_with("crepus_")
    }) {
        return Err("unsupported Crepus ABI declaration: the complete schema must match (including opaque handle, callback, and all functions); no bindings written".into());
    }
    Ok(CheckedHeader {
        canonical: false,
        fallback_error: check_fallback(&content).err(),
    })
}

// Equilibrium's parser is line-oriented and omits some unsupported forms
// without a warning. Admit only declarations that it parses without losing
// functions or arguments; all other forms must fail before writing output.
fn check_fallback(content: &str) -> Result<(), String> {
    let mut stripped = String::new();
    let mut rest = content;
    while let Some(start) = rest.find('/') {
        stripped.push_str(&rest[..start]);
        rest = &rest[start..];
        if rest.starts_with("/*") {
            let end = rest.find("*/").ok_or("unterminated header comment")?;
            rest = &rest[end + 2..];
        } else if rest.starts_with("//") {
            rest = &rest[rest.find('\n').unwrap_or(rest.len())..];
        } else {
            stripped.push('/');
            rest = &rest[1..];
        }
    }
    stripped.push_str(rest);
    if tokens(content)? != tokens(&stripped)? {
        return Err("unsupported comment placement in scalar ABI header".into());
    }
    for line in stripped
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let fail = || {
            format!("unsupported scalar ABI declaration: {}; use one-line prototypes with named parameters; no bindings written", line.chars().take(120).collect::<String>())
        };
        let words = line.split_whitespace().collect::<Vec<_>>();
        if matches!(words.as_slice(), ["#ifndef", name] | ["#define", name] if identifier(name))
            || words == ["#endif"]
            || line.starts_with("#include <") && line.ends_with('>') && !line.contains(';')
        {
            continue;
        }
        let declaration = line.strip_suffix(';').ok_or_else(fail)?;
        if declaration.starts_with("typedef ") && !declaration.contains(['(', ')', '{', '}', ';']) {
            continue;
        }
        let (signature, args) = declaration.split_once('(').ok_or_else(fail)?;
        let args = args.strip_suffix(')').ok_or_else(fail)?;
        if args.contains(['(', ')', ';']) || signature.contains(';') {
            return Err(fail());
        }
        let checked_name = |part: &str| -> bool {
            let Some((ty, name)) = part.trim().rsplit_once(' ') else {
                return false;
            };
            let name = name.trim_start_matches('*');
            identifier(name)
                && !matches!(
                    name,
                    "void"
                        | "char"
                        | "short"
                        | "int"
                        | "long"
                        | "float"
                        | "double"
                        | "signed"
                        | "unsigned"
                        | "const"
                        | "volatile"
                        | "bool"
                        | "_Bool"
                )
                && !ty.trim().is_empty()
                && tokens(ty)
                    .is_ok_and(|parts| parts.iter().all(|part| *part == "*" || identifier(part)))
        };
        if !checked_name(signature)
            || args.trim() != "void"
                && (args.trim().is_empty() || !args.split(',').all(checked_name))
        {
            return Err(fail());
        }
    }
    Ok(())
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || i > 0 && b.is_ascii_digit())
}

// Compare tokens rather than stripped text: whitespace/comments may change,
// but identifiers cannot merge, and no extra declaration or directive is lost.
fn tokens(input: &str) -> Result<Vec<&str>, String> {
    let bytes = input.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
        } else if bytes[i..].starts_with(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i..].starts_with(b"/*") {
            let end = input[i + 2..]
                .find("*/")
                .ok_or("unterminated header comment")?;
            i += end + 4;
        } else {
            let start = i;
            if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
            } else if bytes[i] == b'"' || bytes[i] == b'\'' {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                if i >= bytes.len() {
                    return Err("unterminated header string".into());
                }
                i += 1;
            } else if bytes[i].is_ascii_punctuation() {
                i += 1;
            } else {
                return Err("unsupported non-ASCII header token".into());
            }
            out.push(&input[start..i]);
        }
    }
    Ok(out)
}

#[derive(Clone, Copy)]
enum Ty {
    Void,
    Status,
    Session,
    Input,
    Output,
    Callback,
    Userdata,
}
use Ty::*;

struct Function {
    name: &'static str,
    result: Ty,
    args: &'static [(&'static str, Ty)],
}

const FUNCTIONS: &[Function] = &[
    Function {
        name: "crepus_session_new",
        result: Session,
        args: &[],
    },
    Function {
        name: "crepus_session_free",
        result: Void,
        args: &[("session", Session)],
    },
    Function {
        name: "crepus_session_set_template_string",
        result: Status,
        args: &[
            ("session", Session),
            ("template_utf8", Input),
            ("base_dir_utf8", Input),
        ],
    },
    Function {
        name: "crepus_session_set_component",
        result: Status,
        args: &[("session", Session), ("component_utf8", Input)],
    },
    Function {
        name: "crepus_session_set_files_json",
        result: Status,
        args: &[("session", Session), ("files_json_utf8", Input)],
    },
    Function {
        name: "crepus_session_set_context_json",
        result: Status,
        args: &[("session", Session), ("context_json_utf8", Input)],
    },
    Function {
        name: "crepus_session_apply_context_patch_json",
        result: Status,
        args: &[("session", Session), ("context_json_utf8", Input)],
    },
    Function {
        name: "crepus_session_set_event_callback",
        result: Status,
        args: &[
            ("session", Session),
            ("callback", Callback),
            ("userdata", Userdata),
        ],
    },
    Function {
        name: "crepus_session_render_ir_json",
        result: Output,
        args: &[("session", Session)],
    },
    Function {
        name: "crepus_session_dispatch_event_json",
        result: Output,
        args: &[("session", Session), ("event_json_utf8", Input)],
    },
    Function {
        name: "crepus_session_take_last_error",
        result: Output,
        args: &[("session", Session)],
    },
    Function {
        name: "crepus_last_error",
        result: Output,
        args: &[],
    },
    Function {
        name: "crepus_string_free",
        result: Void,
        args: &[("ptr", Output)],
    },
];

fn c_type(ty: Ty) -> &'static str {
    match ty {
        Void => "void",
        Status => "int32_t",
        Session => "CrepusSession *",
        Input => "const char *",
        Output => "char *",
        Callback => "CrepusEventCallback",
        Userdata => "void *",
    }
}

fn rust_type(ty: Ty) -> &'static str {
    match ty {
        Void => "()",
        Status => "i32",
        Session => "*mut CrepusSession",
        Input => "*const c_char",
        Output => "*mut c_char",
        Callback => "CrepusEventCallback",
        Userdata => "*mut c_void",
    }
}

fn csharp_type(ty: Ty) -> &'static str {
    match ty {
        Void => "void",
        Status => "int",
        Callback => "CrepusEventCallback?",
        _ => "IntPtr",
    }
}

fn v_type(ty: Ty) -> &'static str {
    match ty {
        Void => "",
        Status => "i32",
        Session => "&C.CrepusSession",
        Input | Output => "&char",
        Callback => "CrepusEventCallback",
        Userdata => "voidptr",
    }
}

pub(crate) fn generate(header: &Path, language: Language) -> Result<String, String> {
    let include = header
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("invalid header name")?;
    let stem = header
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or("invalid header stem")?;
    if !include
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
        || !stem
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
        || stem.is_empty()
    {
        return Err("header name is not safe for generated bindings".into());
    }

    let v_header = if language == Language::V {
        let absolute = header
            .canonicalize()
            .map_err(|e| format!("resolve header: {e}"))?;
        let absolute = absolute.to_str().ok_or("non-UTF8 header path")?;
        if absolute.contains(['"', '\r', '\n']) {
            return Err("header path is not safe for generated V bindings".into());
        }
        Some(absolute.replace('\\', "/"))
    } else {
        None
    };
    let mut out = match language {
        Language::C => format!("#include \"{include}\"\n\n"),
        Language::Cpp => format!("#include \"{include}\"\n\nextern \"C\" {{\n"),
        Language::Zig => format!("const c = @cImport({{ @cInclude(\"{include}\"); }});\npub const CrepusSession = c.CrepusSession;\npub const CrepusEventCallback = c.CrepusEventCallback;\n"),
        Language::Rust => "use std::os::raw::{c_char, c_void};\n\n#[repr(C)]\npub struct CrepusSession { _private: [u8; 0] }\npub type CrepusEventCallback = Option<unsafe extern \"C\" fn(*const c_char, *mut c_void)>;\n\nunsafe extern \"C\" {\n".into(),
        Language::CSharp => "#nullable enable\nusing System;\nusing System.Runtime.InteropServices;\n\n// Keep callbacks rooted until replaced or the session is freed.\n// Returned string pointers must be released with crepus_string_free.\n[UnmanagedFunctionPointer(CallingConvention.Cdecl)]\npublic delegate void CrepusEventCallback(IntPtr event_json, IntPtr userdata);\n\npublic static class EquilibriumImports\n{\n".into(),
        Language::V => format!("#include \"{}\"\n\n@[typedef]\nstruct C.CrepusSession {{}}\ntype CrepusEventCallback = fn (&char, voidptr)\n\n", v_header.unwrap()),
        _ => return Err(format!("complete Crepus ABI bindings are not supported for {language:?}; no bindings written")),
    };
    for function in FUNCTIONS {
        let name = function.name;
        let names = function
            .args
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(", ");
        match language {
            Language::C | Language::Cpp => {
                let mut params = function
                    .args
                    .iter()
                    .map(|(name, ty)| format!("{} {name}", c_type(*ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                if params.is_empty() {
                    params.push_str("void");
                }
                let ret = if matches!(function.result, Void) {
                    ""
                } else {
                    "return "
                };
                writeln!(
                    out,
                    "{} eq_{stem}_{name}({params}) {{ {ret}{name}({names}); }}",
                    c_type(function.result)
                )
                .unwrap();
            }
            Language::Zig => writeln!(out, "pub const {name} = c.{name};").unwrap(),
            Language::Rust => {
                let params = function
                    .args
                    .iter()
                    .map(|(name, ty)| format!("{name}: {}", rust_type(*ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    out,
                    "    pub fn {name}({params}) -> {};",
                    rust_type(function.result)
                )
                .unwrap();
            }
            Language::CSharp => {
                let params = function
                    .args
                    .iter()
                    .map(|(name, ty)| format!("{} {name}", csharp_type(*ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(out, "    [DllImport(\"{stem}\", CallingConvention = CallingConvention.Cdecl)]\n    public static extern {} {name}({params});", csharp_type(function.result)).unwrap();
            }
            Language::V => {
                let params = function
                    .args
                    .iter()
                    .map(|(name, ty)| format!("{name} {}", v_type(*ty)))
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(out, "fn C.{name}({params}) {}", v_type(function.result)).unwrap();
            }
            _ => unreachable!(),
        }
    }
    if matches!(language, Language::Cpp | Language::Rust | Language::CSharp) {
        out.push_str("}\n");
    }
    Ok(out)
}
