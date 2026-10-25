//! `crepus benchmark` — declarative build-time comparisons (see `examples/advanced/benchmarks/benchmark.toml`).

use console::style;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
#[cfg(feature = "tui")]
use std::io::IsTerminal;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::ui;

#[derive(Debug, Deserialize)]
struct BenchmarkFile {
    version: u32,
    #[serde(default)]
    settings: Settings,
    #[serde(default)]
    targets: Vec<BenchTarget>,
}

#[derive(Debug, Default, Deserialize)]
struct Settings {
    work_root: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BenchTarget {
    suite: String,
    id: String,
    label: String,
    #[serde(default)]
    optional: bool,
    builtin: Option<String>,
    site: Option<String>,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    steps: Vec<BenchStep>,
}

#[derive(Debug, Deserialize)]
struct BenchStep {
    name: String,
    #[serde(default)]
    cwd: Option<String>,
    shell: String,
}

#[derive(Serialize)]
struct JsonReport {
    summary: JsonRunSummary,
    suites: Vec<JsonSuite>,
}

#[derive(Serialize)]
struct JsonRunSummary {
    /// Sum of `total_ms` for targets that finished successfully (`skipped: false`).
    total_wall_ms_completed: u128,
    completed_target_count: usize,
    skipped_target_count: usize,
    /// Completed targets sorted slowest first (compare-across stacks).
    by_wall_time: Vec<JsonInsightRow>,
}

#[derive(Serialize)]
struct JsonInsightRow {
    suite: String,
    id: String,
    label: String,
    wall_ms: u128,
    step_sum_ms: u128,
    /// `wall_ms − step_sum_ms` (setup, shell, logging, or clock skew).
    overhead_ms: u128,
    /// `100 * wall_ms / total_wall_ms_completed` (0 if alone or no completed runs).
    share_of_completed_wall_pct: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    peak_rss_kb: Option<u64>,
}

#[derive(Serialize)]
struct JsonSuite {
    id: String,
    targets: Vec<JsonTargetResult>,
}

#[derive(Serialize)]
struct JsonTargetResult {
    id: String,
    label: String,
    optional: bool,
    skipped: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    steps: Vec<JsonStepResult>,
    /// Wall time for this target (harness around all steps).
    total_ms: u128,
    /// Sum of per-step `duration_ms` (serial work as measured).
    step_sum_ms: u128,
    /// `total_ms.saturating_sub(step_sum_ms)`.
    overhead_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    peak_rss_kb: Option<u64>,
}

struct StepRunCtx<'a> {
    target_dir: &'a Path,
    vars: &'a HashMap<String, String>,
    dry_run: bool,
    json_out: bool,
    verbose: bool,
    measure_memory: bool,
    step_total: usize,
}

fn step_aggregates(steps: &[JsonStepResult], wall_ms: u128) -> (u128, u128, Option<u64>) {
    let sum: u128 = steps.iter().map(|s| s.duration_ms).sum();
    let overhead = wall_ms.saturating_sub(sum);
    let peak = steps.iter().filter_map(|s| s.max_rss_kb).max();
    (sum, overhead, peak)
}

fn json_target_result(
    id: String,
    label: String,
    optional: bool,
    skipped: bool,
    error: Option<String>,
    steps: Vec<JsonStepResult>,
    total_ms: u128,
) -> JsonTargetResult {
    let (step_sum_ms, overhead_ms, peak_rss_kb) = step_aggregates(&steps, total_ms);
    JsonTargetResult {
        id,
        label,
        optional,
        skipped,
        error,
        steps,
        total_ms,
        step_sum_ms,
        overhead_ms,
        peak_rss_kb,
    }
}

fn compute_run_summary(suites: &[JsonSuite]) -> JsonRunSummary {
    let mut skipped_target_count = 0usize;
    let mut rows: Vec<JsonInsightRow> = Vec::new();
    for suite in suites {
        for t in &suite.targets {
            if t.skipped {
                skipped_target_count += 1;
                continue;
            }
            rows.push(JsonInsightRow {
                suite: suite.id.clone(),
                id: t.id.clone(),
                label: t.label.clone(),
                wall_ms: t.total_ms,
                step_sum_ms: t.step_sum_ms,
                overhead_ms: t.overhead_ms,
                share_of_completed_wall_pct: 0.0,
                peak_rss_kb: t.peak_rss_kb,
            });
        }
    }
    rows.sort_by_key(|b| std::cmp::Reverse(b.wall_ms));
    let total_wall_ms_completed: u128 = rows.iter().map(|r| r.wall_ms).sum();
    let completed_target_count = rows.len();
    for r in &mut rows {
        r.share_of_completed_wall_pct = if total_wall_ms_completed > 0 {
            (r.wall_ms as f64) * 100.0 / (total_wall_ms_completed as f64)
        } else {
            0.0
        };
    }
    JsonRunSummary {
        total_wall_ms_completed,
        completed_target_count,
        skipped_target_count,
        by_wall_time: rows,
    }
}

#[derive(Serialize, Clone)]
struct JsonStepResult {
    name: String,
    duration_ms: u128,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_rss_kb: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stderr_tail: Option<String>,
}

pub struct RunOptions {
    pub config_path: Option<PathBuf>,
    pub suite_filter: Option<String>,
    pub only: Option<Vec<String>>,
    pub json_out: bool,
    pub dry_run: bool,
    pub clean: bool,
    pub repo_override: Option<PathBuf>,
    pub work_override: Option<PathBuf>,
    pub verbose: bool,
    pub measure_memory: bool,
    #[cfg(feature = "tui")]
    pub no_tui: bool,
}

pub struct CheckOptions {
    pub config_path: Option<PathBuf>,
    pub suite_filter: Option<String>,
    pub only: Option<Vec<String>>,
    pub json_out: bool,
    pub repo_override: Option<PathBuf>,
}

pub fn execute_run(opts: RunOptions) {
    let RunOptions {
        config_path,
        suite_filter,
        only,
        json_out,
        dry_run,
        clean,
        repo_override,
        work_override,
        verbose,
        measure_memory,
        #[cfg(feature = "tui")]
        no_tui,
    } = opts;

    let cfg_path = resolve_config_path(config_path);
    let raw = std::fs::read_to_string(&cfg_path).unwrap_or_else(|e| {
        ui::error(&format!("cannot read {}: {e}", cfg_path.display()));
    });

    let file: BenchmarkFile = toml::from_str(&raw).unwrap_or_else(|e| {
        ui::error(&format!("benchmark.toml parse error: {e}"));
    });

    if file.version != 1 {
        ui::error(&format!(
            "unsupported benchmark.toml version {}",
            file.version
        ));
    }

    let repo = repo_override.unwrap_or_else(|| find_repo_root(&cfg_path));
    let work_base = work_override.unwrap_or_else(|| {
        let rel = file
            .settings
            .work_root
            .as_deref()
            .unwrap_or("examples/advanced/benchmarks/.work");
        resolve_path(&repo, rel)
    });

    if clean && !dry_run {
        let _ = std::fs::remove_dir_all(&work_base);
    }

    if !dry_run {
        std::fs::create_dir_all(&work_base).unwrap_or_else(|e| {
            ui::error(&format!("could not create {}: {e}", work_base.display()));
        });
    }

    let mut by_suite: HashMap<String, Vec<&BenchTarget>> = HashMap::new();
    for t in &file.targets {
        if let Some(ref f) = suite_filter {
            if f != "all" && t.suite != *f {
                continue;
            }
        }
        if let Some(ref ids) = only {
            if !ids.contains(&t.id) {
                continue;
            }
        }
        if let Some(vec) = by_suite.get_mut(&t.suite) {
            vec.push(t);
        } else {
            by_suite.insert(t.suite.clone(), vec![t]);
        }
    }

    let mut json_suites: Vec<JsonSuite> = Vec::new();
    let mut suite_keys: Vec<String> = by_suite.keys().cloned().collect();
    suite_keys.sort();

    let total_targets: usize = by_suite.values().map(|v| v.len()).sum();
    if verbose && !json_out {
        print_run_header(
            &cfg_path,
            &repo,
            &work_base,
            clean,
            dry_run,
            measure_memory,
            total_targets,
            suite_keys.len(),
        );
    }

    for suite_id in suite_keys {
        let Some(targets) = by_suite.get(&suite_id) else {
            continue;
        };
        if !json_out {
            eprintln!();
            eprintln!(
                "{} {}",
                style("suite").cyan().bold(),
                style(&suite_id).white().bold()
            );
        }

        let mut json_targets: Vec<JsonTargetResult> = Vec::new();

        for t in targets {
            let res = execute_target(
                t,
                &repo,
                &work_base,
                dry_run,
                json_out,
                verbose,
                measure_memory,
            );
            json_targets.push(res);
        }

        json_suites.push(JsonSuite {
            id: suite_id,
            targets: json_targets,
        });
    }

    let summary = compute_run_summary(&json_suites);
    if json_out {
        let report = JsonReport {
            summary,
            suites: json_suites,
        };
        match serde_json::to_string_pretty(&report) {
            Ok(json) => println!("{json}"),
            Err(e) => ui::error(&format!("failed to serialize JSON report: {e}")),
        }
    } else {
        let outcomes = benchmark_outcomes_from_suites(&json_suites);
        #[cfg(feature = "tui")]
        let drew_tui = {
            let top_completed: Vec<(String, f64, u128)> = summary
                .by_wall_time
                .iter()
                .map(|r| (r.label.clone(), r.share_of_completed_wall_pct, r.wall_ms))
                .collect();
            if !no_tui && std::io::stderr().is_terminal() {
                crate::benchmark_tui::try_show_dashboard(&outcomes, &top_completed, dry_run)
                    .unwrap_or(false)
            } else {
                false
            }
        };
        #[cfg(not(feature = "tui"))]
        let drew_tui = false;
        if !drew_tui {
            print_collapsed_plain_summary(&outcomes, &summary.by_wall_time, dry_run);
        }
        eprintln!();
        ui::success("benchmark finished");
    }
}

fn execute_target(
    t: &BenchTarget,
    repo: &Path,
    work_base: &Path,
    dry_run: bool,
    json_out: bool,
    verbose: bool,
    measure_memory: bool,
) -> JsonTargetResult {
    let target_dir = work_base.join(&t.suite).join(&t.id);
    if !dry_run {
        std::fs::create_dir_all(&target_dir).unwrap_or_else(|e| {
            ui::error(&format!("could not create {}: {e}", target_dir.display()));
        });
    }

    let step_total = planned_step_count(t);

    let mut vars = HashMap::new();
    vars.insert("REPO".into(), repo.display().to_string());
    vars.insert("WORK".into(), target_dir.display().to_string());
    vars.insert("TARGET".into(), t.id.clone());

    if !check_requires(&t.requires) {
        let msg = "missing prerequisite command";
        if t.optional {
            if !json_out {
                ui::warning(&format!("{} — skipped ({msg})", style(&t.label).yellow()));
            }
            return json_target_result(
                t.id.clone(),
                t.label.clone(),
                t.optional,
                true,
                Some(msg.into()),
                vec![],
                0,
            );
        } else {
            ui::error(msg);
        }
    }

    if !json_out {
        eprintln!("  {} {}", ui::arrow(), style(&t.label).cyan().bold());
        if verbose {
            eprintln!(
                "      {} id={}  work_dir={}",
                style("target").dim(),
                style(&t.id).dim(),
                style(target_dir.display().to_string()).dim()
            );
        }
    }

    let t0 = Instant::now();
    let ctx = StepRunCtx {
        target_dir: &target_dir,
        vars: &vars,
        dry_run,
        json_out,
        verbose,
        measure_memory,
        step_total,
    };
    let (ok, err, step_results) = if let Some(ref b) = t.builtin {
        run_builtin(b, t, repo, &ctx)
    } else {
        run_steps(&t.steps, ctx)
    };
    let total = t0.elapsed().as_millis();

    if !ok {
        let msg = err.unwrap_or_else(|| "failed".into());
        if t.optional {
            if !json_out && !dry_run && verbose {
                print_target_wall_footer(total, &step_results);
            }
            if !json_out {
                ui::warning(&format!("{} — {msg}", style(&t.label).yellow()));
            }
            return json_target_result(
                t.id.clone(),
                t.label.clone(),
                t.optional,
                true,
                Some(msg),
                step_results,
                total,
            );
        } else {
            if !json_out && !dry_run && verbose {
                print_target_wall_footer(total, &step_results);
            }
            if !json_out {
                eprintln!("    {} {}", ui::err(), style(&t.label).red());
            }
            ui::error(&msg);
        }
    }

    if !json_out && !dry_run && verbose {
        print_target_wall_footer(total, &step_results);
    }

    json_target_result(
        t.id.clone(),
        t.label.clone(),
        t.optional,
        false,
        None,
        step_results,
        total,
    )
}

fn benchmark_outcomes_from_suites(
    suites: &[JsonSuite],
) -> Vec<crate::benchmark_tui::TargetOutcomeSummary> {
    use crate::benchmark_tui::TargetOutcomeSummary;
    let mut v = Vec::new();
    for s in suites {
        for t in &s.targets {
            let (status, detail) = if t.skipped {
                ("skip", t.error.clone().unwrap_or_else(|| "skipped".into()))
            } else if t.error.is_some() {
                ("fail", t.error.as_deref().unwrap_or("").to_string())
            } else {
                ("ok", String::new())
            };
            v.push(TargetOutcomeSummary {
                suite: s.id.clone(),
                id: t.id.clone(),
                label: t.label.clone(),
                wall_ms: t.total_ms,
                status,
                detail,
            });
        }
    }
    v
}

fn print_collapsed_plain_summary(
    outcomes: &[crate::benchmark_tui::TargetOutcomeSummary],
    insight_rows: &[JsonInsightRow],
    dry_run: bool,
) {
    eprintln!();
    eprintln!(
        "{}",
        style("benchmark — all targets (use -v for live step logs; --no-tui skips dashboard)")
            .cyan()
            .bold()
    );
    for o in outcomes {
        let st = match o.status {
            "ok" => style("ok").green(),
            "skip" => style("skip").yellow(),
            "fail" => style("fail").red(),
            _ => style("?").dim(),
        };
        let note = if o.detail.is_empty() {
            o.label.as_str()
        } else {
            o.detail.as_str()
        };
        eprintln!(
            "  {:<10} {:<24} {}  {:>10}  {}",
            style(&o.suite).dim(),
            style(&o.id).cyan(),
            st,
            style(fmt_ms(o.wall_ms)).white(),
            style(print_collapsed_elide(note, 72)).dim()
        );
    }
    if dry_run {
        eprintln!(
            "  {}",
            style("insight: dry-run — re-run without --dry-run for wall times and RSS.").dim()
        );
        return;
    }
    if insight_rows.is_empty() {
        eprintln!(
            "  {}",
            style("insight: no completed targets — check skips or failures above.").dim()
        );
        return;
    }
    eprintln!();
    eprintln!(
        "{}",
        style("insight — where time went (completed targets, slowest first)")
            .cyan()
            .bold()
    );
    eprintln!(
        "  {}",
        style("% of sum(wall) · suite · wall / Σ steps / overhead").dim()
    );
    let show = insight_rows.len().min(25);
    for row in insight_rows.iter().take(show) {
        let wall = fmt_ms(row.wall_ms);
        let steps_lbl = fmt_ms(row.step_sum_ms);
        let oh = fmt_ms(row.overhead_ms);
        let rss = row.peak_rss_kb.map_or(String::new(), |k| {
            format!("  peak {:.1} MiB", k as f64 / 1024.0)
        });
        eprintln!(
            "  {:>5.1}%  {:<8}  wall {:>8}  Σ {:>8}  OH {:>8}{}",
            row.share_of_completed_wall_pct,
            style(&row.suite).dim(),
            style(wall).white(),
            style(steps_lbl).cyan(),
            style(oh).yellow(),
            style(rss).dim(),
        );
        eprintln!("          {}", style(&row.label).white().bold());
    }
    if insight_rows.len() > 25 {
        eprintln!(
            "  {}",
            style(format!(
                "… {} more rows in --json (summary.by_wall_time)",
                insight_rows.len() - 25
            ))
            .dim()
        );
    }
    eprintln!(
        "  {}",
        style("speed up Crepus: slimmer WASM graph, `cargo build --timings`, reuse CARGO_TARGET_DIR, or LTO settings in fixture crates.")
            .dim()
    );
    eprintln!(
        "  {}",
        style("compare stacks: match cold/warm cache policy (--clean) so % shares are meaningful.")
            .dim()
    );
}

fn print_collapsed_elide(s: &str, max: usize) -> String {
    let t = s.trim().replace('\n', " ");
    if t.chars().count() <= max {
        t
    } else {
        format!(
            "{}…",
            t.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn print_run_header(
    cfg_path: &Path,
    repo: &Path,
    work_base: &Path,
    clean: bool,
    dry_run: bool,
    measure_memory: bool,
    total_targets: usize,
    suite_count: usize,
) {
    eprintln!();
    eprintln!("{}", style("crepus benchmark").cyan().bold());
    eprintln!("  {} {}", style("config").dim(), cfg_path.display());
    eprintln!("  {} {}", style("repo").dim(), repo.display());
    eprintln!("  {} {}", style("work").dim(), work_base.display());
    if clean {
        eprintln!("  {}", style("clean: removed previous work dir").yellow());
    }
    if dry_run {
        eprintln!("  {}", style("dry-run: no commands executed").yellow());
    }
    eprintln!(
        "  {} {} targets in {} suites",
        style("plan").dim(),
        style(total_targets.to_string()).white(),
        suite_count
    );
    eprintln!(
        "  {}",
        style("default: compact logs + end summary (TTY: full-screen dashboard) — use -v to stream child output")
            .dim()
    );
    if measure_memory {
        eprintln!(
            "  {}",
            style("memory: peak RSS per step via ps(1) sampled while child runs (Unix) — --no-memory to skip").dim()
        );
    } else {
        eprintln!("  {}", style("memory: off (--no-memory)").dim());
    }
    eprintln!();
}

fn elide_shell_script(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.len() <= max {
        t.to_string()
    } else {
        format!("{}…", &t[..max])
    }
}

fn planned_step_count(t: &BenchTarget) -> usize {
    if let Some(ref b) = t.builtin {
        return match b.as_str() {
            "crepus-webext" => 2,
            "crepus-web" | "crepus-desktop" => 1,
            _ => 1,
        };
    }
    t.steps.len().max(1)
}

fn fmt_ms(ms: u128) -> String {
    ui::format_duration(Duration::from_millis(u64::try_from(ms).unwrap_or(u64::MAX)))
}

/// One line per step as it completes (or planned in `--dry-run`).
/// `timed`: `(ms, ok, peak_rss_kb)` — RSS from sampled `ps` while child runs (Unix).
fn print_step_line(
    json_out: bool,
    emit: bool,
    one_based: usize,
    total: usize,
    name: &str,
    timed: Option<(u128, bool, Option<u64>)>,
) {
    if json_out || total == 0 {
        return;
    }
    if !emit {
        return;
    }
    let idx = style(format!("[{one_based}/{total}]")).dim();
    let nm = style(name);
    match timed {
        None => {
            eprintln!("      {idx}  {nm}  {}", style("(planned)").dim());
        }
        Some((ms, ok, rss)) => {
            let tim = style(fmt_ms(ms)).cyan();
            let st = if ok {
                style("ok").green()
            } else {
                style("FAIL").red().bold()
            };
            if let Some(kb) = rss {
                let mib = kb as f64 / 1024.0;
                eprintln!(
                    "      {idx}  {nm}  {tim}  {st}  {}",
                    style(format!("peak {mib:.1} MiB RSS")).dim()
                );
            } else {
                eprintln!("      {idx}  {nm}  {tim}  {st}");
            }
        }
    }
}

fn print_steps_share_footer(steps: &[JsonStepResult]) {
    if steps.len() < 2 {
        return;
    }
    let sum: u128 = steps.iter().map(|s| s.duration_ms).sum();
    if sum == 0 {
        return;
    }
    eprintln!(
        "      {}",
        style("share of summed step times (where to optimize)").dim()
    );
    for s in steps {
        let pct = (s.duration_ms as f64) * 100.0 / (sum as f64);
        eprintln!("         {:>5.1}%  {}", pct, style(&s.name).dim());
    }
}

fn print_target_wall_footer(wall_ms: u128, steps: &[JsonStepResult]) {
    if steps.is_empty() {
        return;
    }
    let sum: u128 = steps.iter().map(|s| s.duration_ms).sum();
    let wall = fmt_ms(wall_ms);
    let steps_lbl = fmt_ms(sum);
    let overhead_ms = wall_ms.saturating_sub(sum);
    let oh = fmt_ms(overhead_ms);
    eprintln!(
        "      {} wall {}  ·  step times sum {}  ·  overhead {}",
        style("--").dim(),
        style(wall).white().bold(),
        style(steps_lbl).dim(),
        style(oh).yellow(),
    );
    if let Some(peak_kb) = steps.iter().filter_map(|s| s.max_rss_kb).max() {
        eprintln!(
            "      {} peak RSS (max over steps) {:.1} MiB",
            style("--").dim(),
            peak_kb as f64 / 1024.0
        );
    }
    print_steps_share_footer(steps);
}

fn resolve_config_path(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(p) = explicit {
        return p;
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for candidate in [
        cwd.join("benchmark.toml"),
        cwd.join("examples/advanced/benchmarks/benchmark.toml"),
    ] {
        if candidate.exists() {
            return candidate;
        }
    }
    ui::error("no benchmark.toml found; pass --config path");
}

fn find_repo_root(config_path: &Path) -> PathBuf {
    if let Ok(root) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
    {
        if root.status.success() {
            let s = String::from_utf8_lossy(&root.stdout).trim().to_string();
            if !s.is_empty() {
                return PathBuf::from(s);
            }
        }
    }
    config_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn resolve_path(repo: &Path, rel: &str) -> PathBuf {
    let p = Path::new(rel);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        repo.join(rel)
    }
}

fn check_requires(requires: &[String]) -> bool {
    if requires.is_empty() {
        return true;
    }
    for r in requires {
        let (ok, _, _) = run_shell(Path::new("."), r, false, false, &HashMap::new());
        if !ok {
            return false;
        }
    }
    true
}

#[cfg(unix)]
fn sample_rss_kb(pid: u32) -> Option<u64> {
    let out = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "rss="])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.split_whitespace().next()?.parse().ok()
}

#[cfg(not(unix))]
fn sample_rss_kb(_pid: u32) -> Option<u64> {
    None
}

fn wait_with_max_rss(
    child: &mut std::process::Child,
    measure_memory: bool,
) -> std::io::Result<(std::process::ExitStatus, Option<u64>)> {
    if !measure_memory {
        let s = child.wait()?;
        return Ok((s, None));
    }
    #[cfg(unix)]
    {
        let pid = child.id();
        let mut max_kb = 0_u64;
        loop {
            match child.try_wait()? {
                Some(status) => {
                    return Ok((
                        status,
                        if max_kb > 0 {
                            Some(max_kb)
                        } else {
                            sample_rss_kb(pid)
                        },
                    ));
                }
                None => {
                    if let Some(kb) = sample_rss_kb(pid) {
                        max_kb = max_kb.max(kb);
                    }
                    thread::sleep(Duration::from_millis(80));
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        let s = child.wait()?;
        Ok((s, None))
    }
}

/// Run a configured [`Command`]: capture I/O unless `inherit_io`, optionally sample child RSS (Unix).
fn exec_tracked(
    cmd: &mut Command,
    inherit_io: bool,
    measure_memory: bool,
) -> Result<(bool, String, Option<u64>), String> {
    use std::process::Stdio;
    if inherit_io {
        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
    }
    if !measure_memory {
        if inherit_io {
            return match cmd.status() {
                Ok(s) if s.success() => Ok((true, String::new(), None)),
                Ok(s) => Ok((false, format!("exited with {s}"), None)),
                Err(e) => Err(e.to_string()),
            };
        }
        let o = cmd.output().map_err(|e| e.to_string())?;
        let mut err = String::from_utf8_lossy(&o.stderr).to_string();
        if !o.stdout.is_empty() {
            if !err.is_empty() {
                err.push('\n');
            }
            err.push_str(&String::from_utf8_lossy(&o.stdout));
        }
        return Ok((o.status.success(), err, None));
    }

    if inherit_io {
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let (status, rss) = wait_with_max_rss(&mut child, true).map_err(|e| e.to_string())?;
        let ok = status.success();
        let msg = if ok {
            String::new()
        } else {
            format!("exited with {status}")
        };
        return Ok((ok, msg, rss));
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture stdout".to_string())?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture stderr".to_string())?;
    let t_out = thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::BufReader::new(stdout_pipe).read_to_string(&mut s);
        s
    });
    let t_err = thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::BufReader::new(stderr_pipe).read_to_string(&mut s);
        s
    });
    let (status, rss) = wait_with_max_rss(&mut child, true).map_err(|e| e.to_string())?;
    let stdout = t_out.join().unwrap_or_default();
    let mut err = t_err.join().unwrap_or_default();
    if !stdout.is_empty() {
        if !err.is_empty() {
            err.push('\n');
        }
        err.push_str(&stdout);
    }
    Ok((status.success(), err, rss))
}

/// Whether a step script uses shell syntax that direct exec cannot honour.
#[cfg(unix)]
fn needs_shell(script: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut chars = script.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == '\\' {
                    chars.next();
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '\'' | '"' => quote = Some(c),
                '\\' => {
                    chars.next();
                }
                '&' | '|' | ';' | '<' | '>' | '`' | '\n' => return true,
                '$' => {
                    if matches!(chars.peek(), Some('(') | Some('{')) {
                        return true;
                    }
                }
                _ => {}
            },
        }
    }
    false
}

fn run_shell(
    workdir: &Path,
    script: &str,
    inherit_io: bool,
    measure_memory: bool,
    envs: &HashMap<String, String>,
) -> (bool, String, Option<u64>) {
    #[cfg(unix)]
    {
        // A step containing shell operators has to go through a shell: exec'ing
        // the split tokens directly hands `&&` to the program as an argument,
        // which fails the step for a reason that looks like the program's fault.
        // Simple commands keep the direct path so the tracked process is the
        // program itself rather than the shell wrapping it.
        let mut cmd = if needs_shell(script) {
            let mut c = Command::new("sh");
            c.arg("-c").arg(script);
            c
        } else {
            let tokens = match shlex::split(script) {
                Some(t) if !t.is_empty() => t,
                _ => return (false, "Failed to parse command script".to_string(), None),
            };
            let mut c = Command::new(&tokens[0]);
            if tokens.len() > 1 {
                c.args(&tokens[1..]);
            }
            c
        };
        cmd.current_dir(workdir).envs(envs);
        match exec_tracked(&mut cmd, inherit_io, measure_memory) {
            Ok(x) => x,
            Err(e) => (false, e, None),
        }
    }
    #[cfg(not(unix))]
    {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let pid = std::process::id();
        let bat_name = format!("crepus_bench_{pid}_{id}.bat");
        let bat_path = workdir.join(&bat_name);

        if let Err(e) = std::fs::write(&bat_path, script) {
            return (false, format!("failed to write batch file: {e}"), None);
        }

        let mut cmd = Command::new("cmd");
        cmd.arg("/C")
            .arg(bat_path.as_os_str())
            .current_dir(workdir)
            .envs(envs);
        let _measure_memory = measure_memory; // RSS sampling not implemented on Windows

        let res = if inherit_io {
            let st = cmd.status();
            match st {
                Ok(s) if s.success() => (true, String::new(), None),
                Ok(s) => (false, format!("`cmd /C` exited with {s}"), None),
                Err(e) => (false, e.to_string(), None),
            }
        } else {
            match cmd.output() {
                Ok(out) => {
                    let mut err = String::from_utf8_lossy(&out.stderr).to_string();
                    if !out.stdout.is_empty() {
                        if !err.is_empty() {
                            err.push('\n');
                        }
                        err.push_str(&String::from_utf8_lossy(&out.stdout));
                    }
                    (out.status.success(), err, None)
                }
                Err(e) => (false, e.to_string(), None),
            }
        };

        let _ = std::fs::remove_file(&bat_path);
        res
    }
}

fn expand_vars(s: &str, vars: &HashMap<String, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("${k}"), v);
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

#[cfg(unix)]
fn shell_native_vars(s: &str, _vars: &HashMap<String, String>) -> String {
    s.to_string()
}

#[cfg(not(unix))]
fn shell_native_vars(s: &str, vars: &HashMap<String, String>) -> String {
    let mut out = s.to_string();
    for k in vars.keys() {
        out = out.replace(&format!("${k}"), &format!("%{k}%"));
        out = out.replace(&format!("${{{k}}}"), &format!("%{k}%"));
    }
    out
}

/// Step `cwd` supports `$REPO` / `$WORK` / `$TARGET` like `shell`. If the expanded path is
/// absolute, it is used as-is; otherwise it is resolved under the per-target work dir.
fn resolve_step_cwd(ctx: &StepRunCtx<'_>, step: &BenchStep) -> PathBuf {
    let raw = step.cwd.as_deref().unwrap_or(".");
    let expanded = expand_vars(raw, ctx.vars);
    let path = PathBuf::from(expanded);
    if path.is_absolute() {
        path
    } else {
        ctx.target_dir.join(path)
    }
}

fn run_steps(
    steps: &[BenchStep],
    ctx: StepRunCtx<'_>,
) -> (bool, Option<String>, Vec<JsonStepResult>) {
    let mut results = Vec::new();
    let n = ctx.step_total;
    let inherit_io = ctx.verbose && !ctx.json_out && !ctx.dry_run;
    for (i, step) in steps.iter().enumerate() {
        let one = i + 1;
        let cwd = resolve_step_cwd(&ctx, step);
        let script = shell_native_vars(&step.shell, ctx.vars);

        if ctx.dry_run {
            print_step_line(ctx.json_out, ctx.verbose, one, n, &step.name, None);
            if ctx.verbose && !ctx.json_out {
                eprintln!("            {}", style(&script).dim());
            }
            results.push(JsonStepResult {
                name: step.name.clone(),
                duration_ms: 0,
                ok: true,
                max_rss_kb: None,
                stderr_tail: None,
            });
            continue;
        }

        std::fs::create_dir_all(&cwd).ok();

        if ctx.verbose && !ctx.json_out {
            eprintln!(
                "      {} step {}/{}: {}",
                style("···").dim(),
                one,
                n,
                style(&step.name).yellow()
            );
            eprintln!("          {} {}", style("cwd").dim(), cwd.display());
            eprintln!(
                "          {} {}",
                style("sh -c").dim(),
                style(elide_shell_script(&script, 400)).dim()
            );
            eprintln!("          {}", style("— running —").dim());
        }

        let t0 = Instant::now();
        let (ok, stderr, max_rss) =
            run_shell(&cwd, &script, inherit_io, ctx.measure_memory, ctx.vars);
        let ms = t0.elapsed().as_millis();

        let tail = stderr_tail_opt(&stderr);

        results.push(JsonStepResult {
            name: step.name.clone(),
            duration_ms: ms,
            ok,
            max_rss_kb: max_rss,
            stderr_tail: tail.clone(),
        });

        print_step_line(
            ctx.json_out,
            ctx.verbose,
            one,
            n,
            &step.name,
            Some((ms, ok, max_rss)),
        );

        if !ok {
            let detail = if inherit_io && stderr.is_empty() {
                "(output was streamed; see above)".into()
            } else {
                tail.unwrap_or_else(|| "(no output)".into())
            };
            return (
                false,
                Some(format!(
                    "step '{}' failed ({}ms): {}",
                    step.name, ms, detail
                )),
                results,
            );
        }
    }
    (true, None, results)
}

fn run_builtin(
    builtin: &str,
    target: &BenchTarget,
    repo: &Path,
    ctx: &StepRunCtx<'_>,
) -> (bool, Option<String>, Vec<JsonStepResult>) {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("crepus"));
    let mut results = Vec::new();
    let inherit_io = ctx.verbose && !ctx.json_out && !ctx.dry_run;

    match builtin {
        "crepus-web" => {
            let site = target
                .site
                .as_deref()
                .unwrap_or("examples/advanced/benchmarks/crepus-web");
            let site_path = resolve_path(repo, site);
            let out_dir = ctx.target_dir.join("out");
            if ctx.dry_run {
                print_step_line(
                    ctx.json_out,
                    ctx.verbose,
                    1,
                    1,
                    "crepus web build (--site → --out-dir)",
                    None,
                );
                if ctx.verbose && !ctx.json_out {
                    eprintln!(
                        "            {}",
                        style(format!(
                            "crepus web build --site {} --out-dir {}",
                            site_path.display(),
                            out_dir.display()
                        ))
                        .dim()
                    );
                }
                return (
                    true,
                    None,
                    vec![JsonStepResult {
                        name: "crepus web build".into(),
                        duration_ms: 0,
                        ok: true,
                        max_rss_kb: None,
                        stderr_tail: None,
                    }],
                );
            }
            if ctx.verbose && !ctx.json_out {
                eprintln!(
                    "      {} {}",
                    style("···").dim(),
                    style("builtin: crepus web build").yellow()
                );
                eprintln!("          exe {}", exe.display());
                eprintln!("          --site {}", site_path.display());
                eprintln!("          --out-dir {}", out_dir.display());
                eprintln!("          {}", style("— running —").dim());
            }
            let t0 = Instant::now();
            let mut cmd = Command::new(&exe);
            cmd.args(["web", "build", "--site"])
                .arg(&site_path)
                .arg("--out-dir")
                .arg(&out_dir);
            let built = exec_tracked(&mut cmd, inherit_io, ctx.measure_memory);
            let ms = t0.elapsed().as_millis();
            let (ok, combined, rss) = match built {
                Ok(x) => x,
                Err(e) => (false, e, None),
            };
            let err = if ok {
                None
            } else if combined.is_empty() {
                Some("crepus web build failed".into())
            } else {
                Some(combined)
            };
            results.push(JsonStepResult {
                name: "crepus web build".into(),
                duration_ms: ms,
                ok,
                max_rss_kb: rss,
                stderr_tail: err.as_ref().map(|s| tail_str(s)),
            });
            print_step_line(
                ctx.json_out,
                ctx.verbose,
                1,
                1,
                "crepus web build (--site → --out-dir)",
                Some((ms, ok, rss)),
            );
            if !ok {
                return (
                    false,
                    Some(err.unwrap_or_else(|| "crepus web build failed".into())),
                    results,
                );
            }
        }
        "crepus-webext" => {
            let fixture = repo.join("examples/advanced/benchmarks/crepus-webext");
            let app_dir = ctx.target_dir.join("app");
            if ctx.dry_run {
                print_step_line(
                    ctx.json_out,
                    ctx.verbose,
                    1,
                    2,
                    "sync fixture (copy to work dir)",
                    None,
                );
                print_step_line(ctx.json_out, ctx.verbose, 2, 2, "crepus webext build", None);
                return (
                    true,
                    None,
                    vec![
                        JsonStepResult {
                            name: "sync fixture".into(),
                            duration_ms: 0,
                            ok: true,
                            max_rss_kb: None,
                            stderr_tail: None,
                        },
                        JsonStepResult {
                            name: "crepus webext build".into(),
                            duration_ms: 0,
                            ok: true,
                            max_rss_kb: None,
                            stderr_tail: None,
                        },
                    ],
                );
            }
            if ctx.verbose && !ctx.json_out {
                eprintln!(
                    "      {} {}",
                    style("···").dim(),
                    style("sync fixture: recursive copy").yellow()
                );
                eprintln!("          {} {}", style("from").dim(), fixture.display());
                eprintln!("          {} {}", style("to").dim(), app_dir.display());
            }
            let t_copy = Instant::now();
            if let Err(e) = copy_dir_all(&fixture, &app_dir) {
                let ms = t_copy.elapsed().as_millis();
                results.push(JsonStepResult {
                    name: "sync fixture".into(),
                    duration_ms: ms,
                    ok: false,
                    max_rss_kb: None,
                    stderr_tail: Some(e.to_string()),
                });
                print_step_line(
                    ctx.json_out,
                    ctx.verbose,
                    1,
                    2,
                    "sync fixture (copy to work dir)",
                    Some((ms, false, None)),
                );
                return (false, Some(format!("copy fixture: {e}")), results);
            }
            let copy_ms = t_copy.elapsed().as_millis();
            results.push(JsonStepResult {
                name: "sync fixture".into(),
                duration_ms: copy_ms,
                ok: true,
                max_rss_kb: None,
                stderr_tail: None,
            });
            print_step_line(
                ctx.json_out,
                ctx.verbose,
                1,
                2,
                "sync fixture (copy to work dir)",
                Some((copy_ms, true, None)),
            );

            if let Err(e) = patch_webext_runtime_crepuscularity_path(repo, &app_dir) {
                return (
                    false,
                    Some(format!("patch webext runtime path: {e}")),
                    results,
                );
            }

            if ctx.verbose && !ctx.json_out {
                eprintln!(
                    "      {} {}",
                    style("···").dim(),
                    style("builtin: crepus webext build").yellow()
                );
                eprintln!("          exe {}", exe.display());
                eprintln!("          --app {}", app_dir.display());
                eprintln!("          {}", style("— running —").dim());
            }
            let t_build = Instant::now();
            let mut wcmd = Command::new(&exe);
            wcmd.args(["webext", "build", "--app"]).arg(&app_dir);
            let built = exec_tracked(&mut wcmd, inherit_io, ctx.measure_memory);
            let ms = t_build.elapsed().as_millis();
            let (ok, combined, rss) = match built {
                Ok(x) => x,
                Err(e) => (false, e, None),
            };
            let err = if ok {
                None
            } else if combined.is_empty() {
                Some("webext build failed".into())
            } else {
                Some(combined)
            };
            results.push(JsonStepResult {
                name: "crepus webext build".into(),
                duration_ms: ms,
                ok,
                max_rss_kb: rss,
                stderr_tail: err.as_ref().map(|s| tail_str(s)),
            });
            print_step_line(
                ctx.json_out,
                ctx.verbose,
                2,
                2,
                "crepus webext build",
                Some((ms, ok, rss)),
            );
            if !ok {
                return (
                    false,
                    Some(err.unwrap_or_else(|| "webext build failed".into())),
                    results,
                );
            }
        }
        "crepus-desktop" => {
            let crate_dir = repo.join("examples/advanced/benchmarks/crepus-desktop");
            if !crate_dir.join("Cargo.toml").exists() {
                return (
                    false,
                    Some(format!("missing desktop fixture: {}", crate_dir.display())),
                    results,
                );
            }
            let target_sub = ctx.target_dir.join("cargo-target");
            if ctx.dry_run {
                print_step_line(
                    ctx.json_out,
                    ctx.verbose,
                    1,
                    1,
                    "cargo build --release (GPUI fixture, CARGO_TARGET_DIR in work)",
                    None,
                );
                return (
                    true,
                    None,
                    vec![JsonStepResult {
                        name: "cargo build --release (GPUI fixture)".into(),
                        duration_ms: 0,
                        ok: true,
                        max_rss_kb: None,
                        stderr_tail: None,
                    }],
                );
            }
            if ctx.verbose && !ctx.json_out {
                eprintln!(
                    "      {} {}",
                    style("···").dim(),
                    style("builtin: cargo build --release (GPUI)").yellow()
                );
                eprintln!("          cwd {}", crate_dir.display());
                eprintln!("          CARGO_TARGET_DIR {}", target_sub.display());
                eprintln!("          {}", style("— running —").dim());
            }
            let t0 = Instant::now();
            let mut cmd = Command::new("cargo");
            cmd.args(["build", "--release"])
                .current_dir(&crate_dir)
                .env("CARGO_TARGET_DIR", &target_sub);
            inject_sdkroot_env(&mut cmd);
            let built = exec_tracked(&mut cmd, inherit_io, ctx.measure_memory);
            let ms = t0.elapsed().as_millis();
            let (ok, combined, rss) = match built {
                Ok(x) => x,
                Err(e) => (false, e, None),
            };
            let err = if ok {
                None
            } else if combined.is_empty() {
                Some("desktop build failed".into())
            } else {
                Some(combined)
            };
            results.push(JsonStepResult {
                name: "cargo build --release (GPUI fixture)".into(),
                duration_ms: ms,
                ok,
                max_rss_kb: rss,
                stderr_tail: err.as_ref().map(|s| tail_str(s)),
            });
            print_step_line(
                ctx.json_out,
                ctx.verbose,
                1,
                1,
                "cargo build --release (GPUI fixture, CARGO_TARGET_DIR in work)",
                Some((ms, ok, rss)),
            );
            if !ok {
                return (
                    false,
                    Some(err.unwrap_or_else(|| "desktop build failed".into())),
                    results,
                );
            }
        }
        _ => {
            return (false, Some(format!("unknown builtin '{builtin}'")), results);
        }
    }
    (true, None, results)
}

fn stderr_tail_opt(stderr: &str) -> Option<String> {
    if stderr.trim().is_empty() {
        None
    } else {
        Some(tail_str(stderr))
    }
}

fn tail_str(s: &str) -> String {
    if s.len() > 2000 {
        s[s.len().saturating_sub(2000)..].to_string()
    } else {
        s.to_string()
    }
}

fn inject_sdkroot_env(cmd: &mut Command) {
    if std::env::var_os("SDKROOT").is_none() && cfg!(target_os = "macos") {
        if let Ok(out) = Command::new("xcrun").args(["--show-sdk-path"]).output() {
            if out.status.success() {
                let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !p.is_empty() {
                    cmd.env("SDKROOT", p);
                }
            }
        }
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    use std::fs;
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    copy_rec(src, dst)
}

fn copy_rec(src: &Path, dst: &Path) -> std::io::Result<()> {
    use rayon::prelude::*;
    use std::fs;
    use walkdir::WalkDir;

    if src.is_file() {
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p)?;
        }
        fs::copy(src, dst)?;
        return Ok(());
    }

    // `Path::is_dir` in the previous recursive implementation followed directory symlinks.
    // Preserve that behavior so benchmark fixtures can contain linked directories.
    let entries: Vec<_> = WalkDir::new(src)
        .follow_links(true)
        .into_iter()
        .collect::<Result<Vec<_>, walkdir::Error>>()
        .map_err(std::io::Error::other)?;

    for entry in entries.iter().filter(|e| e.file_type().is_dir()) {
        let relative_path = entry.path().strip_prefix(src).unwrap();
        let target_path = dst.join(relative_path);
        fs::create_dir_all(&target_path)?;
    }

    entries
        .par_iter()
        .filter(|e| !e.file_type().is_dir())
        .try_for_each(|entry| -> std::io::Result<()> {
            let relative_path = entry.path().strip_prefix(src).unwrap();
            let target_path = dst.join(relative_path);
            fs::copy(entry.path(), &target_path).map(|_| ())
        })?;

    Ok(())
}

/// After copying the benchmark fixture into `.work/.../app`, rewrite `runtime/Cargo.toml` so
/// `crepuscularity-webext` points at the real workspace crate (relative `../../../../crates/...`
/// only works from `examples/advanced/benchmarks/crepus-webext/runtime`).
fn patch_webext_runtime_crepuscularity_path(repo: &Path, app_dir: &Path) -> std::io::Result<()> {
    let toml_path = app_dir.join("runtime/Cargo.toml");
    let crate_dir = repo.join("crates/crepuscularity-webext");
    let canonical = crate_dir.canonicalize().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{}: {e}", crate_dir.display()),
        )
    })?;
    let path_str = canonical.display().to_string().replace('\\', "/");
    let content = std::fs::read_to_string(&toml_path)?;
    let new_line = format!("crepuscularity-webext = {{ path = \"{path_str}\" }}");
    let mut replaced = false;
    let mut out_lines: Vec<String> = Vec::new();
    for line in content.lines() {
        if line.trim_start().starts_with("crepuscularity-webext") {
            replaced = true;
            out_lines.push(new_line.clone());
        } else {
            out_lines.push(line.to_string());
        }
    }
    if !replaced {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "no crepuscularity-webext dependency in {}",
                toml_path.display()
            ),
        ));
    }
    let mut new_contents = out_lines.join("\n");
    if content.ends_with('\n') {
        new_contents.push('\n');
    }
    std::fs::write(&toml_path, new_contents)?;
    Ok(())
}

// ── `crepus benchmark check` — prerequisite probes ---------------------------------

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum BenchTool {
    Git,
    Cargo,
    Node,
    Npm,
    Bun,
    Dotnet,
    CargoLeptos,
    DioxusCli,
    MacosSdk,
}

impl BenchTool {
    fn slug(self) -> &'static str {
        match self {
            BenchTool::Git => "git",
            BenchTool::Cargo => "cargo",
            BenchTool::Node => "node",
            BenchTool::Npm => "npm",
            BenchTool::Bun => "bun",
            BenchTool::Dotnet => "dotnet",
            BenchTool::CargoLeptos => "cargo_leptos",
            BenchTool::DioxusCli => "dioxus_cli",
            BenchTool::MacosSdk => "macos_sdk",
        }
    }

    fn label(self) -> &'static str {
        match self {
            BenchTool::Git => "git",
            BenchTool::Cargo => "cargo (Rust toolchain)",
            BenchTool::Node => "node",
            BenchTool::Npm => "npm",
            BenchTool::Bun => "bun",
            BenchTool::Dotnet => "dotnet SDK",
            BenchTool::CargoLeptos => "cargo-leptos",
            BenchTool::DioxusCli => "dx (dioxus-cli)",
            BenchTool::MacosSdk => "Xcode CLT / macOS SDK",
        }
    }

    fn install_hint(self) -> &'static str {
        match self {
            BenchTool::Git => "https://git-scm.com/downloads",
            BenchTool::Cargo => "https://doc.rust-lang.org/cargo/getting-started/installation.html",
            BenchTool::Node => "https://nodejs.org/ (install LTS; includes npm)",
            BenchTool::Npm => "bundled with Node.js — https://nodejs.org/",
            BenchTool::Bun => "https://bun.sh/",
            BenchTool::Dotnet => {
                "Wax: `wax install dotnet` — https://github.com/tschk/wax (avoid brew dotnet shim); or Microsoft install — https://dotnet.microsoft.com/download"
            },
            BenchTool::CargoLeptos => "cargo install cargo-leptos --locked",
            BenchTool::DioxusCli => "cargo install dioxus-cli --locked",
            BenchTool::MacosSdk => {
                "xcode-select --install   (or export SDKROOT=$(xcrun --show-sdk-path))"
            }
        }
    }
}

#[derive(Serialize)]
struct CheckToolRow {
    id: &'static str,
    label: &'static str,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    install_hint: &'static str,
}

#[derive(Serialize)]
struct CheckTargetRow {
    id: String,
    suite: String,
    label: String,
    optional: bool,
    needs_tools: Vec<&'static str>,
    missing_tools: Vec<&'static str>,
    ok: bool,
}

#[derive(Serialize)]
struct CheckReport {
    config: String,
    repo: String,
    tools: Vec<CheckToolRow>,
    targets: Vec<CheckTargetRow>,
    /// False when any **non-optional** selected target is missing a tool.
    required_targets_ok: bool,
}

fn bench_tools_for_target(t: &BenchTarget) -> BTreeSet<BenchTool> {
    let mut out = BTreeSet::new();
    if let Some(ref b) = t.builtin {
        match b.as_str() {
            "crepus-web" | "crepus-webext" => {
                out.insert(BenchTool::Cargo);
            }
            "crepus-desktop" => {
                out.insert(BenchTool::Cargo);
                if cfg!(target_os = "macos") {
                    out.insert(BenchTool::MacosSdk);
                }
            }
            _ => {}
        }
    }
    for step in &t.steps {
        let s = step.shell.as_str();
        if s.contains("bun ") || s.contains("bunx ") {
            out.insert(BenchTool::Bun);
        }
        if s.contains("npm ")
            || s.contains("npx ")
            || s.contains("npm create")
            || s.contains("npm install")
            || s.contains("npm run")
        {
            out.insert(BenchTool::Node);
            out.insert(BenchTool::Npm);
        }
        if s.contains("dotnet ") {
            out.insert(BenchTool::Dotnet);
        }
        if s.contains("git clone") || (s.contains("git ") && s.contains("clone")) {
            out.insert(BenchTool::Git);
        }
        if s.contains("cargo-leptos") || s.contains("cargo leptos") {
            out.insert(BenchTool::Cargo);
            out.insert(BenchTool::CargoLeptos);
        }
        if s.contains("command -v dx")
            || s.contains(" dx ")
            || s.contains("; dx ")
            || s.starts_with("dx ")
        {
            out.insert(BenchTool::Cargo);
            out.insert(BenchTool::DioxusCli);
        }
    }
    out
}

fn probe_simple(cmd: &str, args: &[&str]) -> (bool, Option<String>) {
    let out = match Command::new(cmd).args(args).output() {
        Ok(o) => o,
        Err(_) => return (false, None),
    };
    if !out.status.success() {
        return (false, None);
    }
    let line = String::from_utf8_lossy(&out.stdout)
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string());
    if line.is_some() {
        return (true, line);
    }
    let line = String::from_utf8_lossy(&out.stderr)
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string());
    (true, line)
}

fn probe_bench_tool(tool: BenchTool) -> (bool, Option<String>) {
    match tool {
        BenchTool::Git => probe_simple("git", &["--version"]),
        BenchTool::Cargo => probe_simple("cargo", &["--version"]),
        BenchTool::Node => probe_simple("node", &["--version"]),
        BenchTool::Npm => probe_simple("npm", &["--version"]),
        BenchTool::Bun => probe_simple("bun", &["--version"]),
        BenchTool::Dotnet => probe_simple("dotnet", &["--version"]),
        BenchTool::CargoLeptos => {
            let a = probe_simple("cargo", &["leptos", "--version"]);
            if a.0 {
                return a;
            }
            probe_simple("cargo-leptos", &["--version"])
        }
        BenchTool::DioxusCli => probe_simple("dx", &["--version"]),
        BenchTool::MacosSdk => {
            #[cfg(target_os = "macos")]
            {
                if std::env::var_os("SDKROOT").is_some() {
                    return (true, Some("SDKROOT set".into()));
                }
                probe_simple("xcrun", &["--show-sdk-path"])
            }
            #[cfg(not(target_os = "macos"))]
            {
                (true, None)
            }
        }
    }
}

fn print_bench_stack_notes(selected: &[&BenchTarget], json_out: bool) {
    if json_out {
        return;
    }
    let ids: BTreeSet<&str> = selected.iter().map(|t| t.id.as_str()).collect();
    if ids.contains("electron") {
        eprintln!(
            "  {}",
            style("note: Electron benchmark uses Bun; native build deps may still apply.").dim()
        );
    }
    if ids.contains("tauri") {
        eprintln!(
            "  {}",
            style("note: Tauri needs Rust + Bun/OS prerequisites — see https://tauri.app/start/prerequisites/")
                .dim()
        );
    }
}

/// Probe CLIs required by benchmark.toml targets (respects `--suite` / `--only` like `benchmark run`).
pub fn execute_check(opts: CheckOptions) {
    let CheckOptions {
        config_path,
        suite_filter,
        only,
        json_out,
        repo_override,
    } = opts;

    let cfg_path = resolve_config_path(config_path);
    let raw = std::fs::read_to_string(&cfg_path).unwrap_or_else(|e| {
        ui::error(&format!("cannot read {}: {e}", cfg_path.display()));
    });

    let file: BenchmarkFile = toml::from_str(&raw).unwrap_or_else(|e| {
        ui::error(&format!("benchmark.toml parse error: {e}"));
    });

    if file.version != 1 {
        ui::error(&format!(
            "unsupported benchmark.toml version {}",
            file.version
        ));
    }

    let repo = repo_override.unwrap_or_else(|| find_repo_root(&cfg_path));

    let mut selected: Vec<&BenchTarget> = Vec::new();
    for t in &file.targets {
        if let Some(ref f) = suite_filter {
            if f != "all" && t.suite != *f {
                continue;
            }
        }
        if let Some(ref ids) = only {
            if !ids.contains(&t.id) {
                continue;
            }
        }
        selected.push(t);
    }

    if selected.is_empty() {
        if json_out {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "error": "no targets match --suite / --only filters"
                }))
                .unwrap_or_else(|_| "{\"error\":\"no match\"}".into())
            );
        } else {
            ui::warning("no targets match --suite / --only filters");
        }
        std::process::exit(1);
    }

    let mut tool_union: BTreeSet<BenchTool> = BTreeSet::new();
    for t in &selected {
        tool_union.extend(bench_tools_for_target(t));
    }

    let mut probe_map: BTreeMap<BenchTool, (bool, Option<String>)> = BTreeMap::new();
    for tool in &tool_union {
        probe_map.insert(*tool, probe_bench_tool(*tool));
    }

    let mut target_rows: Vec<CheckTargetRow> = Vec::new();
    let mut required_targets_ok = true;

    for t in &selected {
        let needs = bench_tools_for_target(t);
        let need_slugs: Vec<&'static str> = needs.iter().map(|x| x.slug()).collect();
        let missing: Vec<BenchTool> = needs
            .iter()
            .copied()
            .filter(|tool| !probe_map.get(tool).is_some_and(|(ok, _)| *ok))
            .collect();
        let missing_slugs: Vec<&'static str> = missing.iter().map(|x| x.slug()).collect();
        let ok = missing.is_empty();
        if !ok && !t.optional {
            required_targets_ok = false;
        }
        target_rows.push(CheckTargetRow {
            id: t.id.clone(),
            suite: t.suite.clone(),
            label: t.label.clone(),
            optional: t.optional,
            needs_tools: need_slugs,
            missing_tools: missing_slugs,
            ok,
        });
    }

    let tool_rows: Vec<CheckToolRow> = tool_union
        .iter()
        .map(|tool| {
            let (ok, ver) = &probe_map[tool];
            CheckToolRow {
                id: tool.slug(),
                label: tool.label(),
                ok: *ok,
                version: ver.clone(),
                install_hint: tool.install_hint(),
            }
        })
        .collect();

    if json_out {
        let report = CheckReport {
            config: cfg_path.display().to_string(),
            repo: repo.display().to_string(),
            tools: tool_rows,
            targets: target_rows,
            required_targets_ok,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".into())
        );
    } else {
        eprintln!();
        eprintln!("{}", style("crepus benchmark check").cyan().bold());
        eprintln!("  {} {}", style("config").dim(), cfg_path.display());
        eprintln!("  {} {}", style("repo").dim(), repo.display());
        eprintln!();
        eprintln!("{}", style("Tools").white().bold());
        for tool in &tool_union {
            let (ok, ver) = &probe_map[tool];
            let name = style(tool.label()).white();
            if *ok {
                let detail = ver.as_ref().map(|s| format!(" — {s}")).unwrap_or_default();
                eprintln!("  {} {name}{}", ui::ok(), style(detail).dim());
            } else {
                eprintln!(
                    "  {} {name}  {}",
                    ui::err(),
                    style(format!("install: {}", tool.install_hint())).yellow()
                );
            }
        }
        eprintln!();
        eprintln!("{}", style("Targets (filtered)").white().bold());
        for row in &target_rows {
            let opt = if row.optional {
                style("optional").dim().to_string()
            } else {
                style("required").red().to_string()
            };
            if row.ok {
                eprintln!(
                    "  {} {}  {}",
                    ui::ok(),
                    style(&row.id).cyan().bold(),
                    style(&row.label).white()
                );
            } else {
                eprintln!(
                    "  {} {}  {}",
                    ui::err(),
                    style(&row.id).cyan().bold(),
                    style(&row.label).white()
                );
            }
            eprintln!(
                "      {}  needs: {}",
                opt,
                style(row.needs_tools.join(", ")).dim()
            );
            if !row.ok {
                eprintln!(
                    "      {} {}",
                    ui::warn(),
                    style(format!(
                        "missing: {} — see hints above",
                        row.missing_tools.join(", ")
                    ))
                    .yellow()
                );
            }
        }
        print_bench_stack_notes(&selected, false);
        eprintln!();
        if required_targets_ok {
            ui::success("all non-optional targets have their CLI prerequisites");
        } else {
            eprintln!(
                "{}",
                style("install missing tools (or use --only to narrow targets)").yellow()
            );
        }
    }

    if !required_targets_ok {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::copy_dir_all;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    #[test]
    #[cfg(unix)]
    fn needs_shell_detects_operators_outside_quotes() {
        use super::needs_shell;
        // Every JS target in benchmark.toml is written this way; running it
        // without a shell hands `&&` to bun as a package name.
        assert!(needs_shell("bun install && bun run build"));
        assert!(needs_shell("cargo build | tee log"));
        assert!(needs_shell("a; b"));
        assert!(needs_shell("echo $(date)"));
        assert!(needs_shell("cargo build > out.txt"));
        // A plain command keeps the direct-exec path.
        assert!(!needs_shell("cargo build --release"));
        assert!(!needs_shell("bun run build"));
        // Operators inside quotes are data, not syntax.
        assert!(!needs_shell("echo 'a && b'"));
        assert!(!needs_shell("echo \"x | y\""));
    }

    #[test]
    #[cfg(unix)]
    fn shell_steps_actually_run_both_commands() {
        use super::run_shell;
        use std::collections::HashMap;
        let dir = tempfile::tempdir().expect("tempdir");
        let (ok, _out, _mem) = run_shell(
            dir.path(),
            "echo one > a.txt && echo two > b.txt",
            false,
            false,
            &HashMap::new(),
        );
        assert!(ok, "chained step should succeed");
        assert!(dir.path().join("a.txt").is_file());
        assert!(dir.path().join("b.txt").is_file());
    }

    #[test]
    #[cfg(unix)]
    fn copy_dir_all_copies_contents_of_symlinked_directories() {
        let temp = tempfile::tempdir().expect("temporary directory");
        let source = temp.path().join("source");
        let target = source.join("target");
        fs::create_dir_all(&target).expect("source target directory");
        fs::write(target.join("nested.txt"), "copied through symlink").expect("source target file");
        symlink("target", source.join("linked")).expect("directory symlink");

        let destination = temp.path().join("destination");
        copy_dir_all(&source, &destination).expect("copy source directory");

        assert_eq!(
            fs::read_to_string(destination.join("linked/nested.txt")).expect("copied symlink file"),
            "copied through symlink"
        );
    }
}
