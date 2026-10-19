//! Generate consumer-language imports from `crepuscularity_abi.h` via [equilibrium-ffi].
//!
//! One-shot entry: [`generate_all`] reads `plugins/crepuscularity-plugins.toml` and writes
//! `abi_generated.<ext>` next to each package path.

use std::path::{Path, PathBuf};

use equilibrium_ffi::{generate_imports, ImportOptions, Language};
use serde::Deserialize;

mod crepus_abi;
mod csharp;
mod d;
mod header;
mod nim;
mod odin;
mod rust;

#[derive(Debug, Deserialize)]
struct PluginsManifest {
    contract: Contract,
    package: Vec<Package>,
}

#[derive(Debug, Deserialize)]
struct Contract {
    abi_header: String,
}

#[derive(Debug, Deserialize)]
struct Package {
    language: String,
    path: String,
}

#[derive(Clone, Debug)]
pub struct BindgenOptions {
    pub repo_root: PathBuf,
    pub manifest_path: PathBuf,
    pub abi_header: Option<PathBuf>,
    pub out_dir: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct GeneratedFile {
    pub language: String,
    pub path: PathBuf,
}

pub fn default_manifest_path(repo_root: &Path) -> PathBuf {
    repo_root.join("plugins/crepuscularity-plugins.toml")
}

pub fn resolve_abi_header(repo_root: &Path, contract_rel: &str) -> PathBuf {
    repo_root.join(contract_rel)
}

/// Map `crepuscularity-plugins.toml` language id → equilibrium consumer (if any).
fn equilibrium_language(id: &str) -> Option<Language> {
    // Repository adapters retain their complete ABI/callback support.
    match id {
        "go" | "python" | "java" | "kotlin" | "swift" | "php" | "ruby" | "typescript" => None,
        _ => Language::from_cli_name(id),
    }
}

fn output_filename(language: &str) -> &'static str {
    match language {
        "rust" => "abi_generated.rs",
        "zig" => "abi_generated.zig",
        "c" => "abi_generated.c",
        "cpp" => "abi_generated.cpp",
        "csharp" => "AbiGenerated.cs",
        "d" => "abi_generated.d",
        "nim" => "abi_generated.nim",
        "odin" => "abi_generated.odin",
        "hare" => "abi_generated.ha",
        "v" => "abi_generated.v",
        "go" => "abi_generated.go",
        "python" => "abi_generated.py",
        "java" => "AbiGenerated.java",
        "kotlin" => "AbiGenerated.kt",
        "swift" => "AbiGenerated.swift",
        "php" => "abi_generated.php",
        "ruby" => "abi_generated.rb",
        "typescript" => "abi_generated.ts",
        _ => "abi_generated.txt",
    }
}

fn load_manifest(path: &Path) -> Result<PluginsManifest, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    toml::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))
}

pub fn generate_all(opts: &BindgenOptions) -> Result<Vec<GeneratedFile>, String> {
    let manifest = load_manifest(&opts.manifest_path)?;
    let header = opts
        .abi_header
        .clone()
        .unwrap_or_else(|| resolve_abi_header(&opts.repo_root, &manifest.contract.abi_header));

    if !header.is_file() {
        return Err(format!(
            "ABI header not found: {} (build: cargo build -p crepuscularity-abi)",
            header.display()
        ));
    }

    let import_opts = ImportOptions::default();
    let mut prepared = Vec::new();

    for pkg in &manifest.package {
        let base = opts
            .out_dir
            .clone()
            .unwrap_or_else(|| opts.repo_root.clone());
        let pkg_dir = base.join(&pkg.path);
        let code = if pkg.language == "rust" {
            rust::generate(&header)?
        } else if pkg.language == "csharp" {
            csharp::generate(&header)?
        } else if pkg.language == "nim" {
            nim::generate(&header)?
        } else if pkg.language == "d" {
            d::generate(&header)?
        } else if pkg.language == "odin" {
            odin::generate(&header)?
        } else {
            let checked = crepus_abi::check_header(&header)?;
            if let Some(lang) = equilibrium_language(&pkg.language) {
                if checked.canonical && pkg.language == "v" {
                    generate_v_imports(&header, Some(&pkg_dir))?
                } else if checked.canonical {
                    crepus_abi::generate(&header, lang)?
                } else {
                    if let Some(error) = &checked.fallback_error {
                        return Err(error.clone());
                    }
                    let generated = generate_imports(&header, lang, &import_opts)?;
                    if !generated.warnings.is_empty() {
                        return Err(format!(
                            "{} bindings for {} are incomplete: {}",
                            pkg.language,
                            header.display(),
                            generated.warnings.join("; ")
                        ));
                    }
                    if !generated.companions.is_empty() {
                        return Err(format!(
                            "{} bindings require companion files; this backend is not supported",
                            pkg.language
                        ));
                    }
                    generated.code
                }
            } else {
                generate_extra(&pkg.language, &header)?
            }
        };

        let file_name = output_filename(&pkg.language);
        let out_path = pkg_dir.join(file_name);
        let comment = match pkg.language.as_str() {
            "python" | "ruby" | "nim" => "#",
            _ => "//",
        };
        let source_path = comment_text(&header.display().to_string());
        let banner = format!(
            "{comment} Auto-generated by `crepus plugins bindgen` (equilibrium-ffi)\n{comment} Source: {}\n\n",
            source_path
        );
        let full = match pkg.language.as_str() {
            "php" => {
                let body = code
                    .strip_prefix("<?php")
                    .ok_or("generated PHP lacks its opening tag")?;
                format!("<?php\n{banner}{body}")
            }
            "ruby" if code.starts_with("# frozen_string_literal: true\n") => {
                let (magic, body) = code.split_once('\n').unwrap();
                format!("{magic}\n{banner}{body}")
            }
            _ => format!("{banner}{code}"),
        };

        prepared.push((
            GeneratedFile {
                language: pkg.language.clone(),
                path: out_path,
            },
            full,
        ));
    }

    // A warning or unsupported later package must not replace earlier outputs.
    let mut written = Vec::new();
    for (file, content) in prepared {
        let parent = file.path.parent().expect("generated file has a parent");
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        std::fs::write(&file.path, content)
            .map_err(|e| format!("write {}: {e}", file.path.display()))?;
        written.push(file);
    }
    Ok(written)
}

fn comment_text(value: &str) -> String {
    // Avoid Java Unicode preprocessing, PHP tags and Python encoding cookies.
    value
        .escape_default()
        .to_string()
        .replace("\\u{", "\\x{")
        .replace('>', "\\x3e")
        .replace(':', "\\x3a")
        .replace('=', "\\x3d")
}

fn patch_go_cgo(code: &str) -> String {
    let preamble = r#"#cgo CFLAGS: -I${SRCDIR}/../../crates/crepuscularity-abi/include
#cgo LDFLAGS: -L${SRCDIR}/../../target/debug -lcrepuscularity_abi
#cgo darwin LDFLAGS: -Wl,-rpath,${SRCDIR}/../../target/debug
#include "crepuscularity_abi.h""#;
    if let Some(start) = code.find("/*") {
        if let Some(end) = code[start..].find("*/") {
            let before = &code[..start];
            let after = &code[start + end + 2..];
            return format!("{before}/*\n{preamble}\n*/{after}");
        }
    }
    format!(
        r#"package crepuscularity

/*
{preamble}
*/
import "C"
{code}"#
    )
}

fn generate_extra(language: &str, header: &Path) -> Result<String, String> {
    let include_name = header
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("crepuscularity_abi.h");
    match language {
        "go" => Ok(patch_go_cgo(&format!(
            r#"// Package crepuscularity — low-level ABI (generated). Link libcrepuscularity_abi.
package crepuscularity

/*
#cgo LDFLAGS: -lcrepuscularity_abi
#include "{include_name}"
*/
import "C"
"#
        ))),
        "python" => Ok(generate_python_ctypes()),
        "v" => generate_v_imports(header, None),
        "java" => Ok(generate_java_jni(include_name)),
        "kotlin" => Ok(generate_kotlin_ffi(include_name)),
        "swift" => Ok(generate_swift_module(include_name)),
        "php" => Ok(generate_php_ffi()),
        "ruby" => Ok(generate_ruby_ffi()),
        "typescript" => Ok(generate_ts_koffi_stub()),
        _ => Err(format!(
            "language {language} has no bindgen backend; add to equilibrium or extra generator"
        )),
    }
}

fn generate_v_imports(header: &Path, package_dir: Option<&Path>) -> Result<String, String> {
    let authored_path = header.to_str().ok_or("V ABI header path must be UTF-8")?;
    // Resolve output locations without normalizing away rejected directive text.
    if authored_path.contains("//") {
        return Err(format!("unsupported V ABI directive path: {authored_path}"));
    }
    let header = std::path::absolute(header)
        .map_err(|error| format!("resolve V ABI header path: {error}"))?;
    let canonical_layout =
        header.ends_with("crates/crepuscularity-abi/include/crepuscularity_abi.h");
    let in_repository_package = match package_dir {
        None => true,
        Some(package) => {
            std::path::absolute(package)
                .map_err(|error| format!("resolve V package path: {error}"))?
                == header
                    .ancestors()
                    .nth(4)
                    .unwrap_or_else(|| Path::new("."))
                    .join("plugins/v")
        }
    };
    let (include_dir, library_dir) = if canonical_layout && in_repository_package {
        (
            "@VMODROOT/../../crates/crepuscularity-abi/include".to_owned(),
            "@VMODROOT/../../target/debug".to_owned(),
        )
    } else {
        (
            header
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .display()
                .to_string(),
            header
                .ancestors()
                .nth(4)
                .unwrap_or_else(|| Path::new("."))
                .join("target/debug")
                .display()
                .to_string(),
        )
    };
    let include_name = header
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("V ABI header path requires a file name")?;
    // V parses directives and flags before passing paths to the C compiler.
    if [include_dir.as_str(), library_dir.as_str(), include_name]
        .iter()
        .any(|value| {
            value.contains(['\r', '\n', '"']) || value.contains("//") || value.contains(" #")
        })
        || [&include_dir, &library_dir].iter().any(|value| {
            value.starts_with('-')
                || value.contains(" -")
                || value.contains("$first_existing")
                || value.contains("$when_first_existing")
        })
        || library_dir.contains(',')
    {
        return Err(format!(
            "unsupported V ABI directive path: {}",
            header.display()
        ));
    }
    Ok(format!(
        r#"module crepuscularity

#flag -I {}
#flag -L {} -lcrepuscularity_abi
#flag darwin -Wl,-rpath,"{}"
#include "{}"

@[typedef]
struct C.CrepusSession {{}}
type CrepusEventCallback = fn (&char, voidptr)

fn C.crepus_session_new() &C.CrepusSession
fn C.crepus_session_free(session &C.CrepusSession)
fn C.crepus_session_set_template_string(session &C.CrepusSession, template_utf8 &char, base_dir_utf8 &char) i32
fn C.crepus_session_set_component(session &C.CrepusSession, component_utf8 &char) i32
fn C.crepus_session_set_files_json(session &C.CrepusSession, files_json_utf8 &char) i32
fn C.crepus_session_set_context_json(session &C.CrepusSession, context_json_utf8 &char) i32
fn C.crepus_session_apply_context_patch_json(session &C.CrepusSession, context_json_utf8 &char) i32
fn C.crepus_session_set_event_callback(session &C.CrepusSession, callback CrepusEventCallback, userdata voidptr) i32
fn C.crepus_session_render_ir_json(session &C.CrepusSession) &char
fn C.crepus_session_dispatch_event_json(session &C.CrepusSession, event_json_utf8 &char) &char
fn C.crepus_session_take_last_error(session &C.CrepusSession) &char
fn C.crepus_last_error() &char
fn C.crepus_string_free(ptr &char)
"#,
        include_dir, library_dir, library_dir, include_name
    ))
}

fn generate_python_ctypes() -> String {
    r#"from __future__ import annotations

import ctypes
import json
import os
import sys
from pathlib import Path
from typing import Any, Callable


def _default_lib() -> str:
    root = Path(__file__).resolve().parents[2]
    suffix = {"darwin": "dylib", "linux": "so", "win32": "dll"}.get(sys.platform, "so")
    prefix = "" if sys.platform == "win32" else "lib"
    name = prefix + "crepuscularity_abi." + suffix
    return str(root / "target" / "debug" / name)


class CrepusAbiSession:
    def __init__(self, lib_path: str | None = None) -> None:
        self._lib = ctypes.CDLL(lib_path or os.environ.get("CREPUS_ABI_LIB") or _default_lib())
        self._callback_type = ctypes.CFUNCTYPE(None, ctypes.c_char_p, ctypes.c_void_p)
        self._callback_ref: Any = None
        self._configure()
        self._ptr = self._lib.crepus_session_new()
        if not self._ptr:
            raise RuntimeError("crepus_session_new failed")

    def close(self) -> None:
        if getattr(self, "_ptr", None):
            self._lib.crepus_session_free(self._ptr)
            self._ptr = None
            self._callback_ref = None

    def __copy__(self) -> "CrepusAbiSession":
        raise TypeError("native ABI sessions cannot be copied")

    def __deepcopy__(self, memo: dict[int, Any]) -> "CrepusAbiSession":
        raise TypeError("native ABI sessions cannot be copied")

    def __enter__(self) -> "CrepusAbiSession":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def __del__(self) -> None:
        self.close()

    def _configure(self) -> None:
        self._lib.crepus_session_new.restype = ctypes.c_void_p
        self._lib.crepus_session_free.argtypes = [ctypes.c_void_p]
        self._lib.crepus_session_set_template_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
        self._lib.crepus_session_set_template_string.restype = ctypes.c_int32
        self._lib.crepus_session_set_context_json.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self._lib.crepus_session_set_context_json.restype = ctypes.c_int32
        self._lib.crepus_session_apply_context_patch_json.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self._lib.crepus_session_apply_context_patch_json.restype = ctypes.c_int32
        self._lib.crepus_session_set_event_callback.argtypes = [ctypes.c_void_p, self._callback_type, ctypes.c_void_p]
        self._lib.crepus_session_set_event_callback.restype = ctypes.c_int32
        self._lib.crepus_session_render_ir_json.argtypes = [ctypes.c_void_p]
        self._lib.crepus_session_render_ir_json.restype = ctypes.c_void_p
        self._lib.crepus_session_dispatch_event_json.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self._lib.crepus_session_dispatch_event_json.restype = ctypes.c_void_p
        self._lib.crepus_session_take_last_error.argtypes = [ctypes.c_void_p]
        self._lib.crepus_session_take_last_error.restype = ctypes.c_void_p
        self._lib.crepus_last_error.restype = ctypes.c_void_p
        self._lib.crepus_string_free.argtypes = [ctypes.c_void_p]
"#
    .to_string()
}

fn generate_java_jni(include_name: &str) -> String {
    let include_name = comment_text(include_name);
    format!(
        r#"// JNI stubs — load libcrepuscularity_abi and declare native methods (fill via javac -h).
// Include path: copy or symlink {include_name} into jni include dir.

public final class AbiGenerated {{
    static {{
        System.loadLibrary("crepuscularity_abi");
    }}

    public static native long crepus_session_new();
    public static native void crepus_session_free(long session);
    public static native int crepus_session_set_template_string(long session, String templateUtf8, String baseDirUtf8);
    public static native int crepus_session_set_context_json(long session, String contextJsonUtf8);
    public static native String crepus_session_render_ir_json(long session);
    public static native String crepus_session_dispatch_event_json(long session, String eventJsonUtf8);
}}
"#
    )
}

fn generate_kotlin_ffi(_include_name: &str) -> String {
    r#"// Kotlin/JVM — same JNI surface as AbiGenerated.java
object AbiGenerated {
    init {
        System.loadLibrary("crepuscularity_abi")
    }
    external fun crepus_session_new(): Long
    external fun crepus_session_free(session: Long)
    external fun crepus_session_set_template_string(session: Long, templateUtf8: String, baseDirUtf8: String?): Int
    external fun crepus_session_set_context_json(session: Long, contextJsonUtf8: String): Int
    external fun crepus_session_render_ir_json(session: Long): String
    external fun crepus_session_dispatch_event_json(session: Long, eventJsonUtf8: String): String
}
"#
    .to_string()
}

fn generate_swift_module(_include_name: &str) -> String {
    r#"// Swift — link libcrepuscularity_abi; expand with SwiftPM module map.
import Foundation

@_silgen_name("crepus_session_new")
func crepus_session_new() -> UnsafeMutableRawPointer?

@_silgen_name("crepus_session_free")
func crepus_session_free(_ session: UnsafeMutableRawPointer?)

@_silgen_name("crepus_session_render_ir_json")
func crepus_session_render_ir_json(_ session: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>?
"#
    .to_string()
}

fn generate_php_ffi() -> String {
    r#"<?php
// FFI — PHP 7.4+; set CREPUS_ABI_LIB to libcrepuscularity_abi path
final class AbiGenerated {
    private static ?\FFI $ffi = null;

    public static function ffi(): \FFI {
        if (self::$ffi === null) {
            $lib = getenv('CREPUS_ABI_LIB') ?: 'libcrepuscularity_abi.dylib';
            self::$ffi = \FFI::cdef(
                'typedef struct CrepusSession CrepusSession;
                 CrepusSession *crepus_session_new(void);
                 void crepus_session_free(CrepusSession *session);
                 char *crepus_session_render_ir_json(CrepusSession *session);
                 void crepus_string_free(char *ptr);',
                $lib
            );
        }
        return self::$ffi;
    }
}
"#
    .to_string()
}

fn generate_ruby_ffi() -> String {
    r#"# frozen_string_literal: true
require "fiddle"
require "fiddle/import"

module AbiGenerated
  extend Fiddle::Importer
  dlload ENV.fetch("CREPUS_ABI_LIB", "libcrepuscularity_abi.dylib")
  extern "void* crepus_session_new()"
  extern "void crepus_session_free(void*)"
  extern "char* crepus_session_render_ir_json(void*)"
  extern "void crepus_string_free(char*)"
end
"#
    .to_string()
}

fn generate_ts_koffi_stub() -> String {
    r#"// Bun/Node: use koffi or bun:ffi against libcrepuscularity_abi
// import koffi from "koffi";
// const lib = koffi.load(process.env.CREPUS_ABI_LIB ?? "libcrepuscularity_abi.dylib");
// export const crepus_session_new = lib.func("void* crepus_session_new()");
export {};
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_python_sessions_release_owned_handles() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "crepus-python-lifecycle-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        let manifest = dir.join("plugins.toml");
        std::fs::write(&manifest, "[contract]\nabi_header = 'crates/crepuscularity-abi/include/crepuscularity_abi.h'\n[[package]]\nlanguage = 'python'\npath = 'python'\n").unwrap();
        generate_all(&BindgenOptions {
            repo_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
            manifest_path: manifest,
            abi_header: None,
            out_dir: Some(dir.clone()),
        })
        .unwrap();
        let script = r#"
import copy, ctypes, gc, runpy, sys
class Function:
    def __init__(self, callback=lambda *args: None): self.callback = callback
    def __call__(self, *args): return self.callback(*args)
class Library:
    def __init__(self):
        self.freed = []
        self.next = 10
        self.fail = False
        self.crepus_session_new = Function(self.new)
        self.crepus_session_free = Function(self.freed.append)
    def new(self):
        if self.fail: return None
        self.next += 1
        return self.next
    def __getattr__(self, name):
        value = Function()
        setattr(self, name, value)
        return value
lib = Library()
ctypes.CDLL = lambda path: lib
Session = runpy.run_path(sys.argv[1])['CrepusAbiSession']
session = Session('fake')
pointer = session._ptr
session.close(); session.close()
assert lib.freed == [pointer]
del session; gc.collect()
assert lib.freed == [pointer]
with Session('fake') as session: pointer = session._ptr
assert lib.freed.count(pointer) == 1
try:
    with Session('fake') as session:
        pointer = session._ptr
        raise ValueError('expected')
except ValueError: pass
assert lib.freed.count(pointer) == 1
session = Session('fake'); pointer = session._ptr
del session; gc.collect()
assert lib.freed.count(pointer) == 1
for copier in [copy.copy, copy.deepcopy]:
    session = Session('fake'); pointer = session._ptr
    duplicate = None
    rejected = False
    try: duplicate = copier(session)
    except TypeError: rejected = True
    session.close()
    if duplicate is not None: duplicate.close()
    del session, duplicate; gc.collect()
    assert rejected, 'native ownership copying was accepted'
    assert lib.freed.count(pointer) == 1
before = list(lib.freed)
lib.fail = True
try: Session('fake')
except RuntimeError: pass
else: raise AssertionError('constructor failure was accepted')
gc.collect()
assert lib.freed == before
ctypes.CDLL = lambda path: (_ for _ in ()).throw(OSError('load failed'))
try: Session('fake')
except OSError: pass
else: raise AssertionError('load failure was accepted')
gc.collect()
assert lib.freed == before
"#;
        let output = std::process::Command::new("python3")
            .args(["-c", script])
            .arg(dir.join("python/abi_generated.py"))
            .output()
            .expect("run generated Python lifecycle");
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn v_rejects_unrepresentable_directive_paths() {
        for directory in [
            "quote\"dir",
            "line\ndir",
            "line\rdir",
            "flag -dir",
            "hash #dir",
            "$first_existing(dir)",
            "$when_first_existing(dir)",
            "$first_existing_dir",
            "$when_first_existing_dir",
        ] {
            let header = Path::new("/custom")
                .join(directory)
                .join("include/custom.h");
            assert!(generate_extra("v", &header).is_err(), "{header:?}");
        }
        assert!(generate_extra("v", Path::new("/custom/include/quote\".h")).is_err());
        assert!(generate_extra("v", Path::new("/custom//include/custom.h")).is_err());
        assert!(generate_extra("v", Path::new("/root,comma/crates/abi/include/custom.h")).is_err());
        assert!(generate_extra("v", Path::new("/space 雪 '$/include/file,ok.h")).is_ok());
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let path = Path::new(std::ffi::OsStr::from_bytes(b"/custom/\xff.h"));
            assert!(generate_extra("v", path).unwrap_err().contains("UTF-8"));
        }
    }

    #[test]
    fn generated_v_matches_compiler_checked_binding() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root = root.canonicalize().unwrap();
        // Git can check out the reference with CRLF on Windows.
        let checked = std::fs::read_to_string(root.join("plugins/v/abi_generated.v"))
            .unwrap()
            .replace("\r\n", "\n");
        let body = &checked[checked.find("module crepuscularity").unwrap()..];
        assert_eq!(
            generate_v_imports(
                &root.join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
                Some(&root.join("plugins/v")),
            )
            .unwrap(),
            body
        );
        // Use a native absolute path: Windows resolves `/custom` against the
        // current drive, so a Unix-only expected flag is not portable.
        let custom_header = root.join("custom include/custom_abi.h");
        let custom = generate_v_imports(&custom_header, None).unwrap();
        let expected_include = format!("#flag -I {}", custom_header.parent().unwrap().display());
        assert!(
            custom.lines().any(|line| line == expected_include),
            "{custom}"
        );
        assert!(custom.contains("#include \"custom_abi.h\""));
    }

    #[test]
    fn python_library_lookup_uses_platform_filenames() {
        let script = format!(
            "{}\n__file__ = '/fixture/plugins/python/abi_generated.py'\nfor platform, filename in [('darwin', 'libcrepuscularity_abi.dylib'), ('linux', 'libcrepuscularity_abi.so'), ('win32', 'crepuscularity_abi.dll')]:\n    sys.platform = platform\n    assert Path(_default_lib()).name == filename, _default_lib()\n",
            generate_python_ctypes()
        );
        let output = std::process::Command::new("python3")
            .args(["-c", &script])
            .output()
            .expect("run generated Python");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn manifest_parses() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = default_manifest_path(&root);
        let m = load_manifest(&path).expect("manifest");
        assert!(m.package.iter().any(|p| p.language == "zig"));
    }
    #[test]
    fn nested_pointer_qualifiers_preserve_each_level_in_rust_and_d() {
        struct Directory(PathBuf);
        impl Drop for Directory {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        fn shape(ty: &syn::Type) -> String {
            match ty {
                syn::Type::Ptr(pointer) => format!(
                    "{}({})",
                    if pointer.const_token.is_some() {
                        "const"
                    } else {
                        "mut"
                    },
                    shape(&pointer.elem)
                ),
                syn::Type::Path(path) => path.path.get_ident().unwrap().to_string(),
                _ => panic!("unexpected generated pointer type"),
            }
        }
        let directory = Directory(std::env::temp_dir().join(format!(
                "crepus-pointer-qualifiers-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        std::fs::create_dir(&directory.0).unwrap();
        let header = directory.0.join("abi.h");
        let mut source = String::from("typedef struct CrepusSession CrepusSession;\ntypedef void (*CrepusEventCallback)(const char *event_json, void *userdata);\n");
        let cases = [
            ("char const*", "const(c_char)", "const(char)*"),
            ("char*const*", "const(mut(c_char))", "const(char*)*"),
            (
                "const char*const*",
                "const(const(c_char))",
                "const(const(char)*)*",
            ),
            ("char**const", "mut(mut(c_char))", "char**"),
            ("char * const", "mut(c_char)", "char*"),
            ("const char * const", "const(c_char)", "const(char)*"),
            (
                "char ** const *",
                "const(mut(mut(c_char)))",
                "const(char**)*",
            ),
            (
                "char * const **",
                "mut(const(mut(c_char)))",
                "const(char*)**",
            ),
            (
                "const char * const * const *",
                "const(const(const(c_char)))",
                "const(const(const(char)*)*)*",
            ),
            ("const char *", "const(c_char)", "const(char)*"),
            ("char **", "mut(mut(c_char))", "char**"),
            ("const char **", "mut(const(c_char))", "const(char)**"),
            ("char * const *", "const(mut(c_char))", "const(char*)*"),
            (
                "const char * const *",
                "const(const(c_char))",
                "const(const(char)*)*",
            ),
            ("char const **", "mut(const(c_char))", "const(char)**"),
            (
                "const char ***",
                "mut(mut(const(c_char)))",
                "const(char)***",
            ),
            (
                "CrepusEventCallback *",
                "mut(CrepusEventCallback)",
                "CrepusEventCallback*",
            ),
            (
                "const CrepusSession **",
                "mut(const(CrepusSession))",
                "const(CrepusSession)**",
            ),
        ];
        for (index, (declaration, _, _)) in cases.iter().enumerate() {
            source.push_str(&format!(
                "{declaration} crepus_pointer_{index}({declaration} value);\n"
            ));
        }
        source.push_str("void crepus_names(char **longvalue, const char **value);\n");
        std::fs::write(&header, &source).unwrap();
        let rust = crate::rust::generate(&header).unwrap();
        let parsed = syn::parse_file(&rust).unwrap();
        let functions: Vec<_> = parsed
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::ForeignMod(module) => Some(&module.items),
                _ => None,
            })
            .flatten()
            .filter_map(|item| match item {
                syn::ForeignItem::Fn(function) => Some(function),
                _ => None,
            })
            .collect();
        assert_eq!(functions.len(), cases.len() + 1);
        for (index, (_, expected, _)) in cases.iter().enumerate() {
            let function = functions
                .iter()
                .find(|function| function.sig.ident == format!("crepus_pointer_{index}"))
                .unwrap();
            let syn::FnArg::Typed(parameter) = &function.sig.inputs[0] else {
                panic!("typed parameter");
            };
            assert_eq!(shape(&parameter.ty), *expected, "parameter {index}");
            let syn::ReturnType::Type(_, result) = &function.sig.output else {
                panic!("pointer return");
            };
            assert_eq!(shape(result), *expected, "return {index}");
        }
        let named = functions
            .iter()
            .find(|function| function.sig.ident == "crepus_names")
            .unwrap();
        for (index, expected) in ["mut(mut(c_char))", "mut(const(c_char))"]
            .iter()
            .enumerate()
        {
            let syn::FnArg::Typed(parameter) = &named.sig.inputs[index] else {
                panic!("typed parameter");
            };
            assert_eq!(shape(&parameter.ty), *expected);
        }
        let d = crate::d::generate(&header).unwrap();
        for (index, (_, _, expected)) in cases.iter().enumerate() {
            assert!(
                d.contains(&format!(
                    "extern(C) {expected} crepus_pointer_{index}({expected});"
                )),
                "{d}"
            );
        }
        assert!(
            d.contains("extern(C) void crepus_names(char**, const(char)**);"),
            "{d}"
        );
        for declaration in [
            "void crepus_rejected(volatile char **value);",
            "void crepus_rejected(char * restrict value);",
            "void crepus_rejected(char value[4]);",
        ] {
            std::fs::write(&header, format!("{source}\n{declaration}\n")).unwrap();
            assert!(
                crate::rust::generate(&header).is_err(),
                "Rust admitted {declaration}"
            );
            assert!(
                crate::d::generate(&header).is_err(),
                "D admitted {declaration}"
            );
        }
    }
}
