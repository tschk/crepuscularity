//! Host tooling probes for native/mobile shells (xcode/java/android/rustup).
//!
//! Peeled out of the CLI so scaffold/build/doctor can share one implementation.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn doctor_command(command: &str, args: &[&str]) -> bool {
    let mut cmd = Command::new(command);
    cmd.args(args);
    if command == "gradle" {
        configure_java_home(&mut cmd);
    }
    match cmd.output() {
        Ok(out) if out.status.success() => {
            eprintln!("✓ {command}");
            true
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("✗ {command}: {}", stderr.trim());
            false
        }
        Err(e) => {
            eprintln!("✗ {command}: {e}");
            false
        }
    }
}

pub fn configure_java_home(cmd: &mut Command) {
    match std::env::var("JAVA_HOME") {
        Ok(raw) if PathBuf::from(&raw).join("bin/java").exists() => {}
        _ => {
            if let Some(java_home) = discover_java_home() {
                cmd.env("JAVA_HOME", java_home);
            } else {
                cmd.env_remove("JAVA_HOME");
            }
        }
    }
}

pub fn discover_java_home() -> Option<String> {
    for candidate in [
        "/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home",
        "/opt/homebrew/opt/openjdk/libexec/openjdk.jdk/Contents/Home",
    ] {
        let path = Path::new(candidate);
        if path.join("bin/java").exists() {
            return Some(candidate.to_string());
        }
    }
    if let Ok(out) = Command::new("/usr/libexec/java_home")
        .args(["-v", "17"])
        .output()
    {
        if out.status.success() {
            let path = String::from_utf8(out.stdout).ok()?;
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    let java = std::fs::canonicalize("/opt/homebrew/bin/java")
        .or_else(|_| std::fs::canonicalize("/usr/bin/java"))
        .ok()?;
    let home = java.parent()?.parent()?;
    if home.join("bin/java").exists() {
        Some(home.display().to_string())
    } else {
        None
    }
}

pub fn doctor_rust_target(target: &str) -> bool {
    match Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
    {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.lines().any(|line| line.trim() == target) {
                eprintln!("✓ rust target {target}");
                true
            } else {
                eprintln!("✗ rust target {target}: install with `rustup target add {target}`");
                false
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("✗ rustup: {}", stderr.trim());
            false
        }
        Err(e) => {
            eprintln!("✗ rustup: {e}");
            false
        }
    }
}

pub fn doctor_java17() -> bool {
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let java = PathBuf::from(&java_home).join("bin/java");
        if java_version_at_least(&java, 17) {
            eprintln!("✓ Java 17 {}", java_home);
            true
        } else if java_version_at_least(Path::new("java"), 17) {
            eprintln!("✓ Java 17 java on PATH");
            true
        } else {
            eprintln!("✗ Java 17: JAVA_HOME does not point to Java 17+");
            false
        }
    } else {
        match Command::new("/usr/libexec/java_home")
            .args(["-v", "17"])
            .output()
        {
            Ok(out) if out.status.success() => {
                let path = String::from_utf8_lossy(&out.stdout);
                eprintln!("✓ Java 17 {}", path.trim());
                true
            }
            _ if java_version_at_least(Path::new("java"), 17) => {
                eprintln!("✓ Java 17 java on PATH");
                true
            }
            _ => {
                eprintln!("✗ Java 17: install openjdk@17 and expose it to Gradle");
                false
            }
        }
    }
}

pub fn java_version_at_least(java: &Path, major: u32) -> bool {
    let Ok(out) = Command::new(java).arg("-version").output() else {
        return false;
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    text.split(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .find_map(parse_java_major)
        .is_some_and(|found| found >= major)
}

pub fn parse_java_major(raw: &str) -> Option<u32> {
    if raw.is_empty() {
        return None;
    }
    let first = raw.split('.').next()?;
    first.parse().ok()
}

pub fn doctor_java_home() -> bool {
    match std::env::var("JAVA_HOME") {
        Ok(raw) => {
            let path = PathBuf::from(&raw);
            if path.join("bin/java").exists() {
                eprintln!("✓ JAVA_HOME {}", path.display());
                true
            } else if Path::new("java").exists() || java_version_at_least(Path::new("java"), 17) {
                eprintln!("✓ JAVA_HOME invalid; Gradle will use java from PATH");
                true
            } else {
                eprintln!("✗ JAVA_HOME {} does not contain bin/java", path.display());
                false
            }
        }
        Err(_) => {
            eprintln!("✓ JAVA_HOME not set; Gradle will use java from PATH");
            true
        }
    }
}

pub fn doctor_android_sdk() -> bool {
    let sdk = std::env::var("ANDROID_HOME")
        .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
        .ok()
        .map(PathBuf::from);
    match sdk {
        Some(path) if path.join("platforms").exists() => {
            eprintln!("✓ Android SDK {}", path.display());
            true
        }
        Some(path) => {
            eprintln!("✗ Android SDK {} missing platforms/", path.display());
            false
        }
        None => {
            eprintln!("✗ Android SDK: set ANDROID_HOME or ANDROID_SDK_ROOT");
            false
        }
    }
}

pub fn doctor_android_ndk() -> bool {
    if let Ok(path) = std::env::var("ANDROID_NDK_HOME").map(PathBuf::from) {
        if android_ndk_clang(&path).is_some() {
            eprintln!("✓ Android NDK {}", path.display());
            return true;
        }
    }
    let sdk = std::env::var("ANDROID_HOME")
        .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
        .ok()
        .map(PathBuf::from);
    if let Some(sdk) = sdk {
        let ndk = sdk.join("ndk");
        if let Some(path) = latest_android_ndk(&ndk) {
            eprintln!("✓ Android NDK {}", path.display());
            return true;
        }
    }
    eprintln!("✗ Android NDK: install via Android Studio SDK Manager or set ANDROID_NDK_HOME");
    false
}

pub fn latest_android_ndk(ndk_dir: &Path) -> Option<PathBuf> {
    let mut entries = ndk_dir
        .read_dir()
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && android_ndk_clang(path).is_some())
        .collect::<Vec<_>>();
    entries.sort();
    entries.pop()
}

pub fn android_ndk_clang(ndk: &Path) -> Option<PathBuf> {
    let prebuilt = ndk.join("toolchains/llvm/prebuilt");
    prebuilt
        .read_dir()
        .ok()?
        .flatten()
        .map(|entry| {
            entry
                .path()
                .join("bin")
                .join("aarch64-linux-android26-clang")
        })
        .find(|path| path.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_java_major_valid() {
        assert_eq!(parse_java_major("17.0.2"), Some(17));
        assert_eq!(parse_java_major("11"), Some(11));
        assert_eq!(parse_java_major("1.8.0_292"), Some(1));
    }

    #[test]
    fn test_parse_java_major_invalid() {
        assert_eq!(parse_java_major(""), None);
        assert_eq!(parse_java_major("abc"), None);
        assert_eq!(parse_java_major("abc.def"), None);
        assert_eq!(parse_java_major("  "), None);
    }

    // Workaround for 'ExecutableFileBusy': instead of returning the `NamedTempFile` (which holds a write handle),
    // we return the path and use a guard to clean it up. Or we can just persist the file in a temp dir.
    #[cfg(unix)]
    struct FakeJavaGuard {
        _dir: tempfile::TempDir,
        path: std::path::PathBuf,
    }

    #[cfg(unix)]
    impl FakeJavaGuard {
        fn new(output: &str) -> Self {
            use std::io::Write;
            use std::os::unix::fs::PermissionsExt;
            let dir = tempfile::Builder::new().tempdir().unwrap();
            let path = dir.path().join("fake_java");
            let mut file = std::fs::File::create(&path).unwrap();
            writeln!(file, "#!/bin/sh\ncat << 'EOF'\n{output}\nEOF").unwrap();
            file.sync_all().unwrap();
            drop(file);
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
            Self { _dir: dir, path }
        }
        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_java_version_at_least_success() {
        let fake_java = FakeJavaGuard::new("openjdk version \"17.0.2\" 2022-01-18");
        assert!(java_version_at_least(fake_java.path(), 17));
    }

    #[test]
    #[cfg(unix)]
    fn test_java_version_at_least_failure() {
        let fake_java = FakeJavaGuard::new("openjdk version \"11.0.12\" 2021-07-20");
        assert!(!java_version_at_least(fake_java.path(), 17));
    }

    #[test]
    #[cfg(unix)]
    fn test_java_version_at_least_invalid_output() {
        let fake_java = FakeJavaGuard::new("not a java version string");
        assert!(!java_version_at_least(fake_java.path(), 17));
    }

    #[test]
    #[cfg(unix)]
    fn test_java_version_at_least_command_fails() {
        assert!(!java_version_at_least(
            Path::new("/does/not/exist/java"),
            17
        ));
    }
}
