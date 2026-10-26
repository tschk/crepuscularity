//! Shared CLI output helpers — consistent styling across all commands.
//!
//! Theme mirrors ../wax: console::style for colours, indicatif for spinners.

use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::error::CrepusCliError;

// ── Symbols ─────────────────────────────────────────────────────────────────

pub fn ok() -> impl std::fmt::Display {
    style("✓").green().bold()
}
pub fn err() -> impl std::fmt::Display {
    style("✗").red().bold()
}
pub fn warn() -> impl std::fmt::Display {
    style("!").yellow().bold()
}
pub fn arrow() -> impl std::fmt::Display {
    style("→").dim()
}

// ── One-liner helpers ────────────────────────────────────────────────────────

pub fn success(msg: &str) {
    eprintln!("{} {}", ok(), msg);
}

pub fn error(msg: &str) -> ! {
    eprintln!("{} {}", err(), style(msg).red());
    std::process::exit(1);
}

pub fn report_error(error: &CrepusCliError) -> ! {
    eprintln!("{} {}", err(), style(error).red());
    std::process::exit(1);
}

pub fn warning(msg: &str) {
    eprintln!("  {} {}", warn(), style(msg).yellow());
}

pub fn step(label: &str) {
    eprintln!("  {} {}", arrow(), style(label).dim());
}

pub fn label(text: &str) -> impl std::fmt::Display + '_ {
    style(text).cyan().bold()
}

pub fn dim(text: &str) -> impl std::fmt::Display + '_ {
    style(text).dim()
}

// ── Timing ───────────────────────────────────────────────────────────────────

pub fn format_duration(elapsed: Duration) -> String {
    let ms = elapsed.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", elapsed.as_secs_f64())
    }
}

/// Print a final "done in X" line.
pub fn done_in(elapsed: Duration) {
    eprintln!(
        "\n  {} done in {}",
        style("·").dim(),
        style(format_duration(elapsed)).cyan()
    );
}

// ── Spinner ──────────────────────────────────────────────────────────────────

pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ "),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_message(msg.to_string());
    pb
}

/// Finish a spinner with a ✓ line and clear it.
pub fn spinner_ok(pb: &ProgressBar, msg: &str) {
    pb.finish_and_clear();
    eprintln!("  {} {}", ok(), msg);
}

/// Finish a spinner with a ✗ line, print error, and exit.
pub fn spinner_err(pb: &ProgressBar, msg: &str) -> ! {
    pb.finish_and_clear();
    error(msg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_format_duration() {
        // Under 1000ms
        assert_eq!(format_duration(Duration::from_millis(0)), "0ms");
        assert_eq!(format_duration(Duration::from_millis(500)), "500ms");
        assert_eq!(format_duration(Duration::from_millis(999)), "999ms");

        // 1000ms and above
        assert_eq!(format_duration(Duration::from_millis(1000)), "1.0s");
        assert_eq!(format_duration(Duration::from_millis(1500)), "1.5s");
        assert_eq!(format_duration(Duration::from_millis(10050)), "10.1s");
    }

    #[test]
    fn test_ui_symbols() {
        assert_eq!(console::strip_ansi_codes(&ok().to_string()), "✓");
        assert_eq!(console::strip_ansi_codes(&err().to_string()), "✗");
        assert_eq!(console::strip_ansi_codes(&warn().to_string()), "!");
        assert_eq!(console::strip_ansi_codes(&arrow().to_string()), "→");
    }
}
