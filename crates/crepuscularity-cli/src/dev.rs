/// `crepus dev` — hot-reload dev loop.
///
/// Thread layout:
///   main thread  → GPUI Application::run (owns DevHUD window, blocks until closed)
///   background   → file watcher + cargo build + child process management
///
/// Communication: `Arc<Mutex<HudState>>` (polled by GPUI entity every 100ms)
/// Shutdown:      `Arc<AtomicBool>` set when GPUI window closes
///
/// When `--emit-events` is passed, emits structured JSON CompilerEvents to stdout
/// for IDE/editor integration (following Equilibrium HotCompiler pattern).
///
/// When `target` is "tui", runs in terminal-only mode (no GPUI HUD).
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{recommended_watcher, Event, EventKind};

use crate::build_options::BuildOptions;
#[path = "dev_watch.rs"]
mod project_watch;

use crate::builder::{cargo_build, find_bin_selection, kill_child, BinSelection};
use crate::crepus_toml::{find_manifest_upward, load_manifest_targets};
use crate::events::CompilerEvent;
use crate::hud::{open_hud_window, DevStatus, HudState};
use crate::ui;

/// Resolve a target name to a working directory by looking up crepus.toml.
///
/// Matches against target `type` (e.g. "gpui", "tui") or `id` first.
/// Falls back to common directory conventions: `ui/gui` for gpui, `ui/tui` for tui.
fn resolve_target_dir(target: &str) -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;

    // Try crepus.toml lookup
    if let Some(mpath) = find_manifest_upward(&cwd) {
        if let Ok(Some(targets)) = load_manifest_targets(Some(mpath.clone())) {
            // Match by type or id
            for t in &targets {
                if t.target_type == target || t.id == target {
                    return Some(t.dir.clone());
                }
            }
        }
    }

    // Convention fallbacks
    let fallbacks: &[(&str, &[&str])] = &[
        ("gpui", &["ui/gui", "gui"]),
        ("tui", &["ui/tui", "tui"]),
        ("web", &["ui/web", "web"]),
        ("ios", &["ui/ios", "ios"]),
    ];
    for (kind, dirs) in fallbacks {
        if kind == &target {
            for dir in *dirs {
                let candidate = cwd.join(dir);
                if candidate.join("Cargo.toml").exists() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

pub fn run(
    target: Option<String>,
    bin_override: Option<String>,
    options: BuildOptions,
    emit_events: bool,
) {
    // Resolve target directory if a target was specified
    let cwd = if let Some(ref t) = target {
        match resolve_target_dir(t) {
            Some(dir) => dir,
            None => {
                ui::error(&format!(
                    "could not resolve target '{t}' — no matching [[targets]] in crepus.toml and no ui/{t}/ directory"
                ));
            }
        }
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    };

    let bin_name = find_bin_selection(&cwd, bin_override.as_deref()).unwrap_or_else(|| {
        ui::error("could not determine binary name — add [[bin]] to Cargo.toml or use --bin");
    });

    // Terminal-only mode for tui targets (no GPUI HUD)
    let terminal_only = target
        .as_deref()
        .map(|t| t == "tui" || t == "terminal")
        .unwrap_or(false);

    if terminal_only {
        run_terminal(cwd, bin_name, options, emit_events);
    } else {
        run_hud(cwd, bin_name, options, emit_events);
    }
}

/// Terminal-only dev loop — no GPUI HUD, just rebuild + relaunch on file change.
fn run_terminal(cwd: PathBuf, bin_name: BinSelection, options: BuildOptions, emit_events: bool) {
    eprintln!(
        "  {} dev (terminal) — {bin_name} in {}",
        ui::arrow(),
        cwd.display()
    );

    let (tx, rx) = std::sync::mpsc::channel::<PathBuf>();
    let tx_notify = tx.clone();

    let mut watcher = match recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(ev) = res {
            match ev.kind {
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                    if let Some(path) = ev.paths.into_iter().next() {
                        let _ = tx_notify.send(path);
                    }
                }
                _ => {}
            }
        }
    }) {
        Ok(w) => w,
        Err(e) => {
            ui::error(&format!("could not create file watcher: {e}"));
        }
    };

    project_watch::watch_project(&mut watcher, &cwd);

    let mut child = do_build_launch_terminal(&cwd, &bin_name, options, None, emit_events);

    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(_changed_path) => {
                // Debounce: drain events for 300 ms
                let t = Instant::now();
                while t.elapsed() < Duration::from_millis(300) {
                    while rx.try_recv().is_ok() {}
                    std::thread::sleep(Duration::from_millis(30));
                }
                project_watch::watch_project(&mut watcher, &cwd);
                eprintln!("  {} change detected — rebuilding…", ui::arrow());
                child = do_build_launch_terminal(&cwd, &bin_name, options, child, emit_events);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Some(ref mut c) = child {
                    match c.try_wait() {
                        Ok(Some(status)) => {
                            let code = status.code();
                            eprintln!("  {} app exited ({code:?})", ui::warn());
                            if emit_events {
                                CompilerEvent::process_exited(c.id(), code).emit();
                            }
                            child = None;
                            if code.is_some() {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => child = None,
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn do_build_launch_terminal(
    cwd: &PathBuf,
    bin_name: &BinSelection,
    options: BuildOptions,
    old_child: Option<Child>,
    emit_events: bool,
) -> Option<Child> {
    if let Some(mut c) = old_child {
        let pid = c.id();
        kill_child(&mut c);
        if emit_events {
            CompilerEvent::process_exited(pid, None).emit();
        }
    }

    if emit_events {
        CompilerEvent::compilation_started(vec![cwd.join("src")], Some("file_change".to_string()))
            .emit();
    }

    let t0 = Instant::now();
    let mut outcome = cargo_build(cwd, options, None);
    let elapsed_ms = t0.elapsed().as_millis() as u64;
    let bin_path = outcome.executable_for(bin_name);
    if let (true, Err(message)) = (outcome.success, &bin_path) {
        eprintln!("  {} {message}", ui::err());
        outcome.success = false;
        outcome.errors.push(crate::hud::BuildError {
            level: "error".into(),
            message: message.clone(),
            ..Default::default()
        });
    }

    if let (true, Ok((bin_name, bin_path))) = (outcome.success, bin_path) {
        if emit_events {
            CompilerEvent::compilation_success(elapsed_ms, bin_path.clone()).emit();
        }

        eprintln!(
            "  {} built in {elapsed_ms} ms — launching {bin_name}",
            ui::ok()
        );

        match Command::new(&bin_path).current_dir(cwd).spawn() {
            Ok(c) => {
                if emit_events {
                    CompilerEvent::process_launched(c.id(), bin_path).emit();
                }
                Some(c)
            }
            Err(e) => {
                ui::error(&format!("failed to launch {bin_name}: {e}"));
            }
        }
    } else {
        let count = outcome.errors.len();
        eprintln!("  {} build failed — {count} error(s)", ui::err());
        if emit_events {
            CompilerEvent::compilation_error(elapsed_ms, outcome.errors.clone()).emit();
        }
        None
    }
}

/// GPUI HUD dev loop — original behavior with DevHUD window.
fn run_hud(cwd: PathBuf, bin_name: BinSelection, options: BuildOptions, emit_events: bool) {
    let shared = Arc::new(Mutex::new(HudState::new(bin_name.name().to_owned())));
    let shutdown = Arc::new(AtomicBool::new(false));

    // Emit server started event
    if emit_events {
        CompilerEvent::dev_server_started(
            bin_name.name().to_owned(),
            vec![cwd.join("src"), cwd.join("Cargo.toml")],
        )
        .emit();
    }

    // Spawn background build+watch thread
    {
        let shared = shared.clone();
        let shutdown = shutdown.clone();
        let cwd = cwd.clone();
        let bin_name = bin_name.clone();
        std::thread::spawn(move || {
            background_loop(shared, shutdown, cwd, bin_name, options, emit_events)
        });
    }

    // Run GPUI DevHUD on main thread (blocks until window is closed)
    {
        let shared = shared.clone();
        let shutdown = shutdown.clone();
        gpui_platform::application().run(move |cx: &mut gpui::App| {
            open_hud_window(shared, shutdown, cx);
        });
    }

    // Window closed — tell background thread to stop
    shutdown.store(true, Ordering::Relaxed);
}

// ── Background loop ────────────────────────────────────────────────────────

fn background_loop(
    shared: Arc<Mutex<HudState>>,
    shutdown: Arc<AtomicBool>,
    cwd: PathBuf,
    bin_name: BinSelection,
    options: BuildOptions,
    emit_events: bool,
) {
    let (tx, rx) = std::sync::mpsc::channel::<PathBuf>();
    let tx_notify = tx.clone();

    let mut watcher = match recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(ev) = res {
            match ev.kind {
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                    // Send the first changed path for event emission
                    if let Some(path) = ev.paths.into_iter().next() {
                        let _ = tx_notify.send(path);
                    }
                }
                _ => {}
            }
        }
    }) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("  {} could not create file watcher: {e}", crate::ui::err());
            return;
        }
    };

    project_watch::watch_project(&mut watcher, &cwd);

    // Initial build + launch
    let mut child = do_build_launch(&shared, &cwd, &bin_name, options, None, emit_events);

    loop {
        if shutdown.load(Ordering::Relaxed) {
            if let Some(mut c) = child {
                kill_child(&mut c);
                if emit_events {
                    CompilerEvent::process_exited(c.id(), None).emit();
                }
            }
            break;
        }

        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(changed_path) => {
                // Emit file changed event
                if emit_events {
                    CompilerEvent::file_changed(changed_path).emit();
                }

                // Debounce: drain events for 300 ms
                let t = Instant::now();
                while t.elapsed() < Duration::from_millis(300) {
                    while rx.try_recv().is_ok() {}
                    std::thread::sleep(Duration::from_millis(30));
                }
                project_watch::watch_project(&mut watcher, &cwd);
                eprintln!("  {} change detected — rebuilding…", crate::ui::arrow());
                child = do_build_launch(&shared, &cwd, &bin_name, options, child, emit_events);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Check whether child exited on its own
                if let Some(ref mut c) = child {
                    match c.try_wait() {
                        Ok(Some(status)) => {
                            let code = status.code();
                            eprintln!("  {} app exited ({code:?})", crate::ui::warn());
                            if emit_events {
                                CompilerEvent::process_exited(c.id(), code).emit();
                            }
                            if let Ok(mut s) = shared.lock() {
                                s.status = DevStatus::Exited { code };
                            }
                            child = None;
                            if code.is_some() {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(_) => child = None,
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn do_build_launch(
    shared: &Arc<Mutex<HudState>>,
    cwd: &PathBuf,
    bin_name: &BinSelection,
    options: BuildOptions,
    old_child: Option<Child>,
    emit_events: bool,
) -> Option<Child> {
    // Signal building
    if let Ok(mut s) = shared.lock() {
        s.status = DevStatus::Building;
    }

    // Kill old child
    if let Some(mut c) = old_child {
        let pid = c.id();
        kill_child(&mut c);
        if emit_events {
            CompilerEvent::process_exited(pid, None).emit();
        }
    }

    // Emit compilation started
    if emit_events {
        CompilerEvent::compilation_started(vec![cwd.join("src")], Some("file_change".to_string()))
            .emit();
    }

    let t0 = Instant::now();
    let mut outcome = cargo_build(cwd, options, Some(shared.clone()));
    let elapsed_ms = t0.elapsed().as_millis() as u64;
    let bin_path = outcome.executable_for(bin_name);
    if let (true, Err(message)) = (outcome.success, &bin_path) {
        eprintln!("  {} {message}", ui::err());
        outcome.success = false;
        outcome.errors.push(crate::hud::BuildError {
            level: "error".into(),
            message: message.clone(),
            ..Default::default()
        });
    }

    if let (true, Ok((bin_name, bin_path))) = (outcome.success, bin_path) {
        // Emit compilation success
        if emit_events {
            CompilerEvent::compilation_success(elapsed_ms, bin_path.clone()).emit();
        }

        if let Ok(mut s) = shared.lock() {
            s.status = DevStatus::Running { elapsed_ms };
        }

        eprintln!(
            "  {} built in {elapsed_ms} ms — launching {bin_name}",
            crate::ui::ok()
        );

        match Command::new(&bin_path).current_dir(cwd).spawn() {
            Ok(c) => {
                if emit_events {
                    CompilerEvent::process_launched(c.id(), bin_path).emit();
                }
                Some(c)
            }
            Err(e) => {
                eprintln!("  {} failed to launch {bin_name}: {e}", crate::ui::err());
                if let Ok(mut s) = shared.lock() {
                    s.status = DevStatus::Failed {
                        errors: vec![crate::hud::BuildError {
                            level: "error".into(),
                            message: format!("Failed to launch binary: {e}"),
                            ..Default::default()
                        }],
                        count: 1,
                    };
                }
                None
            }
        }
    } else {
        // Emit compilation error
        if emit_events {
            CompilerEvent::compilation_error(elapsed_ms, outcome.errors.clone()).emit();
        }

        let count = outcome.errors.len();
        eprintln!("  {} build failed — {count} error(s)", crate::ui::err());
        if let Ok(mut s) = shared.lock() {
            s.status = DevStatus::Failed {
                errors: outcome.errors,
                count,
            };
        }
        None
    }
}
