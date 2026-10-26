//! Shared clipboard backend used by the Rust plugin and the host UI.

pub fn read_text() -> Result<String, String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.get_text().map_err(|e| e.to_string())
}

pub fn write_text(text: &str) -> Result<(), String> {
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_write_text_happy_path() {
        let res = write_text("test_clipboard_content");
        if res.is_ok() {
            let read = read_text().unwrap();
            assert_eq!(read, "test_clipboard_content");
        } else {
            // If clipboard is not available on the system, skip happy path assertion
            println!("Skipping happy path as clipboard initialization failed");
        }
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    #[test]
    fn test_read_write_text_error_path() {
        use std::env;
        use std::process::Command;

        if env::var("CREPUSCULARITY_CLIPBOARD_TEST_SUBPROCESS").is_ok() {
            // We are inside the subprocess with a cleared environment.
            let write_res = write_text("test");
            let read_res = read_text();

            assert!(
                write_res.is_err(),
                "write_text should fail without a valid display environment"
            );
            assert!(
                read_res.is_err(),
                "read_text should fail without a valid display environment"
            );

            let write_err = write_res.unwrap_err();
            assert!(!write_err.is_empty(), "Error message should not be empty");

            let read_err = read_res.unwrap_err();
            assert!(!read_err.is_empty(), "Error message should not be empty");
            return;
        }

        // Spawn a subprocess to run this exact test, but with no environment variables.
        // This simulates a headless environment on Linux where DISPLAY and WAYLAND_DISPLAY are missing,
        // which reliably causes arboard::Clipboard::new() to fail.
        let exe = env::current_exe().unwrap();
        let output = Command::new(exe)
            // Run just this test module/function. We use `--exact` to avoid running others.
            .arg("clipboard::tests::test_read_write_text_error_path")
            .arg("--exact")
            .arg("--nocapture")
            .env_clear()
            // Set the flag to indicate we're in the subprocess
            .env("CREPUSCULARITY_CLIPBOARD_TEST_SUBPROCESS", "1")
            .output()
            .unwrap();

        if !output.status.success() {
            panic!(
                "Subprocess test failed:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
