//! Native bridge to the **`in`** toolchain (spawn + filesystem helpers for docs-site / polyglot workflows).
//!
//! Plugin id: `"inauguration"`. Opt in via `[capabilities] inauguration = true` in `crepus-lite.toml`.
//!
//! Does not link the `inauguration` crate; shells out to `in` on `PATH` when `processRun` is used.

use std::process::Command;

use serde_json::{json, Value};

use crate::bridge::{BridgeError, Capability, NativePlugin};

pub struct InaugurationPlugin {
    sandbox_root: std::path::PathBuf,
}

impl InaugurationPlugin {
    pub fn new() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        Self { sandbox_root: root }
    }
}

impl Default for InaugurationPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl NativePlugin for InaugurationPlugin {
    fn id(&self) -> &'static str {
        "inauguration"
    }

    fn capability(&self) -> Capability {
        Capability::Inauguration
    }

    fn methods(&self) -> &'static [&'static str] {
        &["ping", "whichIn", "processRun", "readFile", "writeFile"]
    }

    fn invoke(&self, method: &str, payload: &Value) -> Result<Value, BridgeError> {
        match method {
            "ping" => Ok(json!({ "ok": true, "plugin": "inauguration" })),
            "whichIn" => which_in(),
            "processRun" => process_run(payload),
            "readFile" => read_file(payload, &self.sandbox_root),
            "writeFile" => write_file(payload, &self.sandbox_root),
            _ => Err(BridgeError::new(
                "internal",
                "method routed but not handled",
            )),
        }
    }
}

fn require_str(payload: &Value, key: &str) -> Result<String, BridgeError> {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            BridgeError::new("invalid_argument", format!("missing or non-string `{key}`"))
        })
}

fn which_in() -> Result<Value, BridgeError> {
    let cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    let path = Command::new(cmd)
        .arg("in")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let first = stdout.lines().next().unwrap_or("");
            if first.is_empty() {
                None
            } else {
                Some(first.to_string())
            }
        });
    Ok(json!({
        "found": path.is_some(),
        "path": path,
    }))
}

fn process_run(payload: &Value) -> Result<Value, BridgeError> {
    let command = require_str(payload, "command")?;
    let parts = shlex::split(&command)
        .ok_or_else(|| BridgeError::new("invalid_argument", "malformed command string"))?;
    let (program, args) = parts
        .split_first()
        .ok_or_else(|| BridgeError::new("invalid_argument", "empty command"))?;

    let is_in = program == "in" || program == "in.exe";

    if !is_in {
        return Err(BridgeError::new(
            "access_denied",
            "inauguration plugin may only execute the `in` toolchain",
        ));
    }

    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| BridgeError::new("spawn_failed", e.to_string()))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Ok(json!({
        "exitCode": output.status.code().unwrap_or(-1),
        "stdout": stdout,
        "stderr": stderr,
        "success": output.status.success(),
    }))
}

fn read_file(payload: &Value, sandbox_root: &std::path::Path) -> Result<Value, BridgeError> {
    let path = require_str(payload, "path")?;
    let resolved = crate::fs_paths::resolve_under_sandbox(sandbox_root, &path)?;
    let text = std::fs::read_to_string(&resolved)
        .map_err(|e| BridgeError::new("io_error", format!("read `{path}`: {e}")))?;
    Ok(json!({ "path": path, "text": text }))
}

fn write_file(payload: &Value, sandbox_root: &std::path::Path) -> Result<Value, BridgeError> {
    let path = require_str(payload, "path")?;
    let text = require_str(payload, "text")?;
    let resolved = crate::fs_paths::resolve_under_sandbox(sandbox_root, &path)?;
    std::fs::write(&resolved, text.as_bytes())
        .map_err(|e| BridgeError::new("io_error", format!("write `{path}`: {e}")))?;
    Ok(json!({ "path": path, "bytes": text.len() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_returns_plugin_id() {
        let p = InaugurationPlugin::new();
        let out = p.invoke("ping", &json!({})).expect("ping");
        assert_eq!(
            out.get("plugin").and_then(|v| v.as_str()),
            Some("inauguration")
        );
    }

    #[test]
    fn rejects_path_traversal() {
        let p = InaugurationPlugin::new();
        let payload = json!({ "path": "../etc/passwd" });
        let res = p.invoke("readFile", &payload);
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert_eq!(err.code, "path_escape");
    }

    #[test]
    fn rejects_arbitrary_commands() {
        let p = InaugurationPlugin::new();
        let payload = json!({ "command": "echo hello" });
        let res = p.invoke("processRun", &payload);
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert_eq!(err.code, "access_denied");

        // Also reject arbitrary commands with spaces or quotes handled improperly previously
        let payload = json!({ "command": "rm -rf /" });
        let res = p.invoke("processRun", &payload);
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert_eq!(err.code, "access_denied");
    }
}
