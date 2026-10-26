//! Structured compiler events for IDE integration.
//!
//! Emits typed JSON events to stdout when `--emit-events` is passed to `crepus dev`.
//! This follows the Equilibrium HotCompiler pattern for editor/IDE integration.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::hud::BuildError;

/// Structured compiler events, serialized as JSON for IDE consumption.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum CompilerEvent {
    /// Emitted when compilation starts.
    CompilationStarted {
        timestamp_ms: u64,
        files: Vec<PathBuf>,
        #[serde(skip_serializing_if = "Option::is_none")]
        trigger: Option<String>,
    },

    /// Emitted when compilation completes successfully.
    CompilationSuccess {
        timestamp_ms: u64,
        duration_ms: u64,
        output: PathBuf,
    },

    /// Emitted when compilation fails.
    CompilationError {
        timestamp_ms: u64,
        duration_ms: u64,
        errors: Vec<CompilerDiagnostic>,
    },

    /// Emitted when a file change is detected.
    FileChanged { timestamp_ms: u64, path: PathBuf },

    /// Emitted when the dev server starts.
    DevServerStarted {
        timestamp_ms: u64,
        project: String,
        watch_paths: Vec<PathBuf>,
    },

    /// Emitted when the child process launches.
    ProcessLaunched {
        timestamp_ms: u64,
        pid: u32,
        binary: PathBuf,
    },

    /// Emitted when the child process exits.
    ProcessExited {
        timestamp_ms: u64,
        pid: u32,
        code: Option<i32>,
    },
}

/// A compiler diagnostic (error or warning).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompilerDiagnostic {
    pub level: String,
    pub message: String,
    pub file: String,
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rendered: Option<String>,
}

impl From<BuildError> for CompilerDiagnostic {
    fn from(err: BuildError) -> Self {
        Self {
            level: err.level,
            message: err.message,
            file: err.file,
            line: err.line,
            column: None,
            rendered: err.rendered,
        }
    }
}

impl CompilerEvent {
    /// Create a timestamp in milliseconds since UNIX epoch.
    pub fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// Emit this event as a JSON line to stdout.
    pub fn emit(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            println!("{json}");
        }
    }

    /// Create a CompilationStarted event.
    pub fn compilation_started(files: Vec<PathBuf>, trigger: Option<String>) -> Self {
        Self::CompilationStarted {
            timestamp_ms: Self::now_ms(),
            files,
            trigger,
        }
    }

    /// Create a CompilationSuccess event.
    pub fn compilation_success(duration_ms: u64, output: PathBuf) -> Self {
        Self::CompilationSuccess {
            timestamp_ms: Self::now_ms(),
            duration_ms,
            output,
        }
    }

    /// Create a CompilationError event.
    pub fn compilation_error(duration_ms: u64, errors: Vec<BuildError>) -> Self {
        Self::CompilationError {
            timestamp_ms: Self::now_ms(),
            duration_ms,
            errors: errors.into_iter().map(Into::into).collect(),
        }
    }

    /// Create a FileChanged event.
    pub fn file_changed(path: PathBuf) -> Self {
        Self::FileChanged {
            timestamp_ms: Self::now_ms(),
            path,
        }
    }

    /// Create a DevServerStarted event.
    pub fn dev_server_started(project: String, watch_paths: Vec<PathBuf>) -> Self {
        Self::DevServerStarted {
            timestamp_ms: Self::now_ms(),
            project,
            watch_paths,
        }
    }

    /// Create a ProcessLaunched event.
    pub fn process_launched(pid: u32, binary: PathBuf) -> Self {
        Self::ProcessLaunched {
            timestamp_ms: Self::now_ms(),
            pid,
            binary,
        }
    }

    /// Create a ProcessExited event.
    pub fn process_exited(pid: u32, code: Option<i32>) -> Self {
        Self::ProcessExited {
            timestamp_ms: Self::now_ms(),
            pid,
            code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let event = CompilerEvent::compilation_started(
            vec![PathBuf::from("src/main.rs")],
            Some("file_change".to_string()),
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"compilation_started\""));
        assert!(json.contains("src/main.rs"));
    }

    #[test]
    fn test_error_event() {
        let errors = vec![BuildError {
            level: "error".to_string(),
            message: "cannot find value `x`".to_string(),
            file: "src/lib.rs".to_string(),
            line: 42,
            rendered: None,
        }];

        let event = CompilerEvent::compilation_error(150, errors);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"compilation_error\""));
        assert!(json.contains("cannot find value"));
    }
}
