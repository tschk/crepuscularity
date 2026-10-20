#![cfg(unix)]

use crepuscularity_plugin_reference::{render_ir_with_context, ViewNode};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture {
    directory: PathBuf,
    previous_bin: Option<OsString>,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "crepus-rust-transport-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        Self {
            directory,
            previous_bin: std::env::var_os("CREPUS_BIN"),
        }
    }

    fn install(&self, script: &str) {
        let binary = self.directory.join("crepus");
        fs::write(&binary, format!("#!/usr/bin/env python3\n{script}")).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        std::env::set_var("CREPUS_BIN", binary);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        match &self.previous_bin {
            Some(value) => std::env::set_var("CREPUS_BIN", value),
            None => std::env::remove_var("CREPUS_BIN"),
        }
        let _ = fs::remove_dir_all(&self.directory);
    }
}

// One test owns process-global CREPUS_BIN for both scenarios; no parallel env mutation.
#[test]
fn large_stdin_drains_diagnostics_and_early_exit_preserves_error() {
    let fixture = Fixture::new();
    let source = fixture.directory.join("input.crepus");
    fs::write(&source, "span \"Source\"").unwrap();
    let context = HashMap::from([("blob".to_owned(), Value::String("雪".repeat(400_000)))]);
    let mut failures = Vec::new();

    // Labelled CLI fixture fills stderr before reading a >1MiB input envelope.
    // The watchdog makes a sequential-write regression fail rather than hang.
    fixture.install(r#"import json,os,sys,threading,time
threading.Thread(target=lambda: (time.sleep(5), os._exit(88)), daemon=True).start()
sys.stderr.buffer.write(('雪 warning\n' * 32768).encode('utf-8'))
sys.stderr.buffer.flush()
payload=json.loads(sys.stdin.buffer.read().decode('utf-8'))
assert payload['template'] == 'span "Source"'
assert payload['context']['blob'] == '雪' * 400000
sys.stdout.buffer.write(json.dumps({'version':7,'root':[{'kind':'text','content':'Exact payload 雪'}]},ensure_ascii=False).encode('utf-8'))
"#);
    match render_ir_with_context(source.to_str().unwrap(), &context) {
        Ok(ir) => {
            if ir.version != 7
                || !matches!(ir.root.as_slice(), [ViewNode::Text { content }] if content == "Exact payload 雪")
            {
                failures.push("large-input response changed".to_owned());
            }
        }
        Err(error) => failures.push(format!("large-input transport failed: {error}")),
    }

    // Exit before consuming stdin while the parent still has a large pending write.
    fixture.install(
        r#"import sys
sys.stderr.buffer.write('雪 early failure diagnostic\n'.encode('utf-8'))
sys.stderr.buffer.flush()
sys.exit(9)
"#,
    );
    match render_ir_with_context(source.to_str().unwrap(), &context) {
        Ok(_) => failures.push("early CLI failure accepted".to_owned()),
        Err(error) => {
            let message = error.to_string();
            if !message.contains("雪 early failure diagnostic") {
                failures.push(format!("early failure diagnostic lost: {message}"));
            }
        }
    }
    // A successful exit cannot turn a truncated stdin write into success.
    fixture.install(
        r#"import sys
sys.stdout.buffer.write(b'{"version":7,"root":[]}')
sys.stdout.buffer.flush()
sys.exit(0)
"#,
    );
    match render_ir_with_context(source.to_str().unwrap(), &context) {
        Ok(_) => failures.push("successful child hid incomplete stdin write".to_owned()),
        Err(error) => {
            if error
                .downcast_ref::<std::io::Error>()
                .map(std::io::Error::kind)
                != Some(std::io::ErrorKind::BrokenPipe)
            {
                failures.push(format!("successful early exit lost BrokenPipe: {error}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}
