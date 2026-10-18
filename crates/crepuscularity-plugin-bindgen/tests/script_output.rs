use crepuscularity_plugin_bindgen::{generate_all, BindgenOptions};
use std::{path::PathBuf, process::Command};

struct Output(PathBuf);
impl Drop for Output {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
#[cfg(target_os = "macos")]
#[ignore = "requires installed V and C compilers"]
fn generated_v_compiles_with_spaced_header_and_library_directories() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output =
        Output(std::env::temp_dir().join(format!("crepus-v-path-{}-{stamp}", std::process::id())));
    let root = output.0.join("project with spaces");
    let include = root.join("crates/abi/include");
    let library = root.join("target/debug");
    std::fs::create_dir_all(&include).unwrap();
    std::fs::create_dir_all(&library).unwrap();
    let header = include.join("custom_abi.h");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../crepuscularity-abi/include/crepuscularity_abi.h"),
        &header,
    )
    .unwrap();
    let c = root.join("empty.c");
    std::fs::write(&c, "void crepus_bindgen_probe(void) {}\n").unwrap();
    let compiled = Command::new("cc")
        .arg("-dynamiclib")
        .arg(&c)
        .arg("-o")
        .arg(library.join("libcrepuscularity_abi.dylib"))
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let manifest = root.join("plugins.toml");
    std::fs::write(
        &manifest,
        "[contract]\nabi_header = 'unused.h'\n[[package]]\nlanguage = 'v'\npath = 'v'\n",
    )
    .unwrap();
    let files = generate_all(&BindgenOptions {
        repo_root: root.clone(),
        manifest_path: manifest,
        abi_header: Some(header),
        out_dir: Some(root.clone()),
    })
    .unwrap();
    let source = std::fs::read_to_string(&files[0].path).unwrap();
    let main = root.join("main.v");
    std::fs::write(
        &main,
        source.replacen("module crepuscularity", "module main", 1) + "\nfn main() {}\n",
    )
    .unwrap();
    let compiled = Command::new("v")
        .args(["-gc", "none", "-o"])
        .arg(root.join("probe"))
        .arg(main)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
}

#[test]
#[cfg(unix)]
fn header_path_line_breaks_cannot_escape_generated_source_comments() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Output(std::env::temp_dir().join(format!(
        "crepus-header-path-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    )));
    let directory = output
        .0
        .join("header\nnot source\rnext\u{2028}line\u{2029}end ?> coding: invalid coding=invalid");
    std::fs::create_dir_all(&directory).unwrap();
    let header = directory.join("abi\nnot source.h");
    std::fs::copy(
        root.join("crates/crepuscularity-abi/include/crepuscularity_abi.h"),
        &header,
    )
    .unwrap();
    let manifest = output.0.join("plugins.toml");
    std::fs::write(&manifest, "[contract]\nabi_header = 'unused.h'\n[[package]]\nlanguage = 'rust'\npath = 'rust'\n[[package]]\nlanguage = 'php'\npath = 'php'\n[[package]]\nlanguage = 'python'\npath = 'python'\n[[package]]\nlanguage = 'java'\npath = 'java'\n").unwrap();
    let files = generate_all(&BindgenOptions {
        repo_root: root,
        manifest_path: manifest,
        abi_header: Some(header),
        out_dir: Some(output.0.clone()),
    })
    .unwrap();
    let source = std::fs::read_to_string(&files[0].path).unwrap();
    syn::parse_file(&source).expect("a valid filesystem path must remain comment text");
    let source_comment = source
        .lines()
        .find(|line| line.starts_with("// Source:"))
        .unwrap();
    assert!(
        source_comment.contains("header\\nnot source\\rnext\\x{2028}line\\x{2029}end"),
        "{source_comment}"
    );
    let php = Command::new("php")
        .args(["-r", "require $argv[1];"])
        .arg(&files[1].path)
        .output()
        .unwrap();
    assert!(
        php.status.success(),
        "{}",
        String::from_utf8_lossy(&php.stderr)
    );
    assert!(php.stdout.is_empty(), "path text escaped PHP comments");
    let python = Command::new("python3")
        .args([
            "-c",
                "import pathlib, sys; compile(pathlib.Path(sys.argv[1]).read_bytes(), sys.argv[1], 'exec')",
        ])
        .arg(&files[2].path)
        .output()
        .unwrap();
    assert!(
        python.status.success(),
        "{}",
        String::from_utf8_lossy(&python.stderr)
    );
    let java = Command::new("javac")
        .arg("-d")
        .arg(output.0.join("classes"))
        .arg(&files[3].path)
        .output()
        .unwrap();
    assert!(
        java.status.success(),
        "{}",
        String::from_utf8_lossy(&java.stderr)
    );
}

#[test]
fn generated_script_banners_preserve_syntax_and_silent_php_imports() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let output = Output(std::env::temp_dir().join(format!(
        "crepus-script-bindgen-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    )));
    std::fs::create_dir(&output.0).unwrap();
    let manifest = output.0.join("plugins.toml");
    std::fs::write(&manifest, "[contract]\nabi_header = 'crates/crepuscularity-abi/include/crepuscularity_abi.h'\n[[package]]\nlanguage = 'ruby'\npath = 'ruby'\n[[package]]\nlanguage = 'php'\npath = 'php'\n[[package]]\nlanguage = 'nim'\npath = 'nim'\n").unwrap();
    let files = generate_all(&BindgenOptions {
        repo_root: root,
        manifest_path: manifest,
        abi_header: None,
        out_dir: Some(output.0.clone()),
    })
    .unwrap();
    assert_eq!(files.len(), 3);
    let nim = std::fs::read_to_string(output.0.join("nim/abi_generated.nim")).unwrap();
    assert!(nim.starts_with("# Auto-generated"));
    assert!(nim.lines().nth(1).unwrap().starts_with("# Source:"));
    let ruby = output.0.join("ruby/abi_generated.rb");
    let checked = Command::new("ruby")
        .arg("-c")
        .arg(&ruby)
        .output()
        .expect("installed Ruby");
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert!(std::fs::read_to_string(ruby)
        .unwrap()
        .starts_with("# frozen_string_literal: true\n"));
    let php = output.0.join("php/abi_generated.php");
    let checked = Command::new("php")
        .arg("-l")
        .arg(&php)
        .output()
        .expect("installed PHP");
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let imported = Command::new("php")
        .args(["-r", "require $argv[1];"])
        .arg(php)
        .output()
        .unwrap();
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );
    assert!(
        imported.stdout.is_empty(),
        "import wrote {:?}",
        String::from_utf8_lossy(&imported.stdout)
    );
}
