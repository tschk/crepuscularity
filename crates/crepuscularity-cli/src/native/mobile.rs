use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, RwLock};
use std::time::{Duration, Instant};

use console::style;
use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::{
    host, plan_hot_reload, render_template_to_ir, to_json, HotReloadEnvelope, HotReloadMessage,
};
use notify::{Event, EventKind, RecursiveMode, Watcher};

use super::build::IosBuildTarget;
use super::ir::{
    load_json_ctx, parse_var_value, resolve_template_path, CodegenArgs, CodegenPlatform, SyncArgs,
};
use super::{execute, prepend_kotlin_package};
use crate::cli::{
    MobileCodegenPlatformArg, MobileCommands, NativeBuildCommands, NativeCommands,
    NativeRunCommands,
};
use crate::error::CrepusCliError;
use crate::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobilePlatform {
    Ios,
    Android,
    All,
}

#[derive(Debug)]
pub struct MobileDevArgs {
    dir: PathBuf,
    port: u16,
    platform: MobilePlatform,
    template: PathBuf,
    ctx_file: Option<PathBuf>,
    vars: Vec<(String, String)>,
}

pub struct MobileDevState {
    pub(crate) sequence: AtomicU64,
    pub(crate) root: PathBuf,
    pub(crate) platform: MobilePlatform,
    pub(crate) template_path: PathBuf,
    pub(crate) ctx: TemplateContext,
    pub(crate) last_template: RwLock<String>,
    pub(crate) last_ir_json: RwLock<String>,
    pub(crate) last_event: RwLock<HotReloadEnvelope>,
}

pub fn mobile_platform(p: crate::cli::MobilePlatformArg) -> MobilePlatform {
    match p {
        crate::cli::MobilePlatformArg::Ios => MobilePlatform::Ios,
        crate::cli::MobilePlatformArg::Android => MobilePlatform::Android,
        crate::cli::MobilePlatformArg::All => MobilePlatform::All,
    }
}

pub fn execute_mobile(cmd: MobileCommands) -> Result<(), CrepusCliError> {
    match cmd {
        MobileCommands::New { name } => {
            scaffold_mobile_app(&name);
            Ok(())
        }
        MobileCommands::Sync {
            template,
            dir,
            vars,
            pretty,
        } => execute(NativeCommands::Sync {
            args: SyncArgs {
                template,
                dir,
                out: Vec::new(),
                no_defaults: false,
                component: None,
                ctx: None,
                vars,
                pretty,
            },
        }),
        MobileCommands::Codegen {
            template,
            dir,
            platform,
            out,
            view_name,
            vars,
        } => run_codegen_full(template, dir, platform, out, view_name, &vars),
        MobileCommands::Build {
            platform,
            dir,
            target,
            configuration,
            flavor,
        } => {
            let root = dir.clone().unwrap_or_else(|| PathBuf::from("."));
            // Auto-codegen before build.
            run_codegen_full(
                Some(PathBuf::from("views/main.crepus")),
                root,
                match mobile_platform(platform) {
                    MobilePlatform::Ios => Some(MobileCodegenPlatformArg::Ios),
                    MobilePlatform::Android => Some(MobileCodegenPlatformArg::Android),
                    MobilePlatform::All => None,
                },
                None,
                Some("CrepusGeneratedView".into()),
                &[],
            )?;
            run_mobile_build(
                mobile_platform(platform),
                dir,
                target,
                configuration,
                flavor,
            )
        }
        MobileCommands::Run {
            platform,
            dir,
            flavor,
        } => run_mobile_run(mobile_platform(platform), dir, flavor),
        MobileCommands::Doctor { platform } => {
            run_doctor(mobile_platform(platform));
            Ok(())
        }
        MobileCommands::Dev {
            dir,
            port,
            platform,
            template,
            ctx,
            vars,
        } => {
            let parsed = MobileDevArgs {
                dir,
                port,
                platform: mobile_platform(platform),
                template,
                ctx_file: ctx,
                vars: vars
                    .iter()
                    .map(|kv| {
                        kv.split_once('=')
                            .map(|(k, v)| (k.to_string(), v.to_string()))
                            .unwrap_or_else(|| {
                                ui::error(&format!("--var expects key=value, got: {kv}"))
                            })
                    })
                    .collect(),
            };
            if let Err(e) = run_dev(parsed) {
                ui::error(&e.to_string());
            }
            Ok(())
        }
    }
}

pub fn scaffold_mobile_app(name: &str) {
    let name_snake = name.replace(['-', ' '], "_");
    let name_pascal: String = name
        .split(['-', '_', ' '])
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect();

    // Step 1: scaffold with generic names.
    let _ = execute(NativeCommands::New {
        name: name.to_string(),
    });

    let root = PathBuf::from(name);
    if !root.exists() {
        return;
    }

    // Step 2: regenerate fixture + codegen (before rename, uses nativeshell paths).
    let template = PathBuf::from("views/main.crepus");
    let _ = execute(NativeCommands::Sync {
        args: SyncArgs {
            template: template.clone(),
            dir: root.clone(),
            out: Vec::new(),
            no_defaults: false,
            component: None,
            ctx: None,
            vars: Vec::new(),
            pretty: true,
        },
    });
    run_codegen_full(
        Some(template),
        root.clone(),
        None,
        None,
        Some("CrepusGeneratedView".into()),
        &[],
    )
    .unwrap_or_else(|e| ui::error(&e.to_string()));

    // Step 3: replace generic names with app-specific ones (after codegen).
    replace_scaffold_names(&root, &name_snake, &name_pascal);

    rename_scaffold_files(&root, &name_snake, &name_pascal);

    ui::success(&format!(
        "scaffolded mobile app '{}' at '{name}'",
        name_pascal,
    ));
    eprintln!();
    eprintln!("{}", style("Next steps").dim());
    eprintln!("  cd {name}");
    eprintln!("  # Edit your template:");
    eprintln!("  vim views/main.crepus");
    eprintln!("  # Regenerate + build:");
    eprintln!("  crepus mobile build --platform ios");
    eprintln!("  crepus mobile build --platform android");
}

fn replace_scaffold_names(root: &Path, name_snake: &str, name_pascal: &str) {
    let r_0 = format!("{name_snake}_actions");
    let r_1 = format!("dev.crepuscularity.{name_snake}");
    let r_2 = format!("dev.crepuscularity.{name_snake}");
    let r_3 = format!("dev_crepuscularity_{name_snake}");
    let r_4 = format!("{name_pascal}App");

    let patterns = &[
        "crepus_mobile_actions",
        "dev.crepuscularity.nativeshell",
        "dev.crepuscularity.mobile",
        "dev_crepuscularity_nativeshell",
        "CrepusMobileApp",
    ];
    let replace_with = &[
        r_0.as_str(),
        r_1.as_str(),
        r_2.as_str(),
        r_3.as_str(),
        r_4.as_str(),
    ];

    let rewrite_paths: &[&str] = &[
        "crepus.toml",
        "rust/Cargo.toml",
        "rust/src/lib.rs",
        "ios/crepus.toml",
        "ios/project.yml",
        "ios/App/CrepusMobileApp.swift",
        "android/app/build.gradle.kts",
        "android/app/src/main/AndroidManifest.xml",
        "android/app/src/main/java/dev/crepuscularity/nativeshell/MainActivity.kt",
        "android/app/src/main/java/dev/crepuscularity/nativeshell/CrepusRustActions.kt",
        "android/app/src/main/java/dev/crepuscularity/nativeshell/generated/CrepusGeneratedView.kt",
    ];

    for rel in rewrite_paths {
        let path = root.join(rel);
        if let Ok(content) = fs::read_to_string(&path) {
            let mut updated = content.clone();
            for (from, to) in patterns.iter().zip(replace_with.iter()) {
                updated = updated.replace(from, to);
            }
            if updated != content {
                let _ = fs::write(&path, updated);
            }
        }
    }
}

fn rename_scaffold_files(root: &Path, name_snake: &str, name_pascal: &str) {
    // Rename iOS app file.
    let old_swift = root.join("ios/App/CrepusMobileApp.swift");
    let new_swift = root.join(format!("ios/App/{name_pascal}App.swift"));
    if old_swift.exists() && old_swift != new_swift {
        let _ = fs::rename(&old_swift, &new_swift);
    }

    // Rename Android package directory.
    let old_pkg = root.join("android/app/src/main/java/dev/crepuscularity/nativeshell");
    let new_pkg = root.join(format!(
        "android/app/src/main/java/dev/crepuscularity/{name_snake}"
    ));
    if old_pkg.exists() && old_pkg != new_pkg {
        let _ = fs::rename(&old_pkg, &new_pkg);
    }

    // Rename Cargo.lock package name if present.
    let cargo_lock = root.join("rust/Cargo.lock");
    if let Ok(content) = fs::read_to_string(&cargo_lock) {
        let updated = content.replace("crepus_mobile_actions", &format!("{name_snake}_actions"));
        if updated != content {
            let _ = fs::write(&cargo_lock, updated);
        }
    }
}

pub fn run_codegen_full(
    template: Option<PathBuf>,
    dir: PathBuf,
    platform: Option<MobileCodegenPlatformArg>,
    out: Option<PathBuf>,
    view_name: Option<String>,
    vars: &[String],
) -> Result<(), CrepusCliError> {
    let template = template.ok_or_else(|| "expected template path".to_string())?;
    let view_name = view_name.unwrap_or_else(|| "CrepusGeneratedView".into());
    let plats = match platform {
        None => vec![MobilePlatform::Ios, MobilePlatform::Android],
        Some(MobileCodegenPlatformArg::Ios) => vec![MobilePlatform::Ios],
        Some(MobileCodegenPlatformArg::Android) => vec![MobilePlatform::Android],
    };
    let template_path = resolve_template_path(&dir, &template);
    let plat_count = plats.len();
    let single_out = out.clone();
    for plat in plats {
        let (default_out, plat_arg) = match plat {
            MobilePlatform::Ios => (
                dir.join("ios/Sources/NativeShell/Generated"),
                CodegenPlatform::SwiftUi,
            ),
            MobilePlatform::Android => (
                dir.join("android/app/src/main/java/dev/crepuscularity/nativeshell/generated"),
                CodegenPlatform::Compose,
            ),
            MobilePlatform::All => continue,
        };
        let out_dir = if plat_count == 1 {
            single_out.clone().unwrap_or(default_out)
        } else {
            default_out
        };
        let _ = execute(NativeCommands::Codegen {
            args: CodegenArgs {
                template: Some(template_path.clone()),
                platform: Some(plat_arg),
                out: Some(out_dir.clone()),
                view_name: Some(view_name.clone()),
                component: None,
                ctx: None,
                vars: vars.to_vec(),
            },
        });
        if plat == MobilePlatform::Android {
            prepend_kotlin_package(&out_dir.join(format!("{view_name}.kt")));
        }
    }
    Ok(())
}

pub fn run_mobile_build(
    platform: MobilePlatform,
    dir: Option<PathBuf>,
    target: IosBuildTarget,
    configuration: String,
    flavor: String,
) -> Result<(), CrepusCliError> {
    match platform {
        MobilePlatform::Ios => execute(NativeCommands::Build {
            platform: NativeBuildCommands::Ios {
                dir,
                target,
                configuration,
            },
        }),
        MobilePlatform::Android => execute(NativeCommands::Build {
            platform: NativeBuildCommands::Android { dir, flavor },
        }),
        MobilePlatform::All => {
            run_mobile_build(
                MobilePlatform::Ios,
                dir.clone(),
                target,
                configuration.clone(),
                flavor.clone(),
            )?;
            run_mobile_build(MobilePlatform::Android, dir, target, configuration, flavor)
        }
    }
}

pub fn run_mobile_run(
    platform: MobilePlatform,
    dir: Option<PathBuf>,
    flavor: String,
) -> Result<(), CrepusCliError> {
    match platform {
        MobilePlatform::Ios => execute(NativeCommands::Run {
            platform: NativeRunCommands::Ios { dir },
        }),
        MobilePlatform::Android => execute(NativeCommands::Run {
            platform: NativeRunCommands::Android { dir, flavor },
        }),
        MobilePlatform::All => Err(CrepusCliError::context(
            "crepus mobile run expects --platform ios or android",
        )),
    }
}

pub fn run_doctor(platform: MobilePlatform) {
    let mut ok = true;
    eprintln!("{}", style("crepus mobile doctor").cyan().bold());
    ok &= host::doctor_command("cargo", &["--version"]);
    match platform {
        MobilePlatform::Ios | MobilePlatform::All => {
            ok &= host::doctor_rust_target("aarch64-apple-ios");
            ok &= host::doctor_command("xcodebuild", &["-version"]);
            ok &= host::doctor_command("xcodegen", &["--version"]);
            ok &= host::doctor_command("xcrun", &["simctl", "list", "runtimes"]);
        }
        _ => {}
    }
    match platform {
        MobilePlatform::Android | MobilePlatform::All => {
            ok &= host::doctor_rust_target("aarch64-linux-android");
            ok &= host::doctor_command("java", &["-version"]);
            ok &= host::doctor_java17();
            ok &= host::doctor_java_home();
            ok &= host::doctor_command("gradle", &["--version"]);
            ok &= host::doctor_android_sdk();
            ok &= host::doctor_android_ndk();
        }
        _ => {}
    }
    if !ok {
        std::process::exit(1);
    }
}

pub fn run_dev(args: MobileDevArgs) -> Result<(), CrepusCliError> {
    let root = fs::canonicalize(&args.dir).unwrap_or(args.dir);
    let template_path = resolve_template_path(&root, &args.template);
    let mut ctx = TemplateContext::new();
    if let Some(path) = &args.ctx_file {
        load_json_ctx(path, &mut ctx)?;
    }
    for (key, raw) in args.vars {
        ctx.set(key, parse_var_value(&raw));
    }
    ctx.base_dir = template_path.parent().map(Path::to_path_buf);

    let initial_template = fs::read_to_string(&template_path)
        .map_err(|e| CrepusCliError::io(e, template_path.clone()))?;
    let ir = render_template_to_ir(&initial_template, &ctx)
        .map_err(|e| CrepusCliError::context(e.to_string()))?;
    let ir_json =
        to_json(&ir).map_err(|e| CrepusCliError::context(format!("serialize IR: {e}")))?;
    let event = HotReloadEnvelope {
        sequence: 0,
        message: HotReloadMessage::FullReload {
            ir,
            reason: "initial load".to_string(),
        },
    };
    let state = Arc::new(MobileDevState {
        sequence: AtomicU64::new(0),
        root: root.clone(),
        platform: args.platform,
        template_path: template_path.clone(),
        ctx,
        last_template: RwLock::new(initial_template),
        last_ir_json: RwLock::new(ir_json),
        last_event: RwLock::new(event),
    });

    sync_outputs(&root, &template_path, args.platform);
    start_mobile_watcher(root.clone(), state.clone());

    let addr = format!("127.0.0.1:{}", args.port);
    let listener = TcpListener::bind(&addr).map_err(|e| format!("bind {addr}: {e}"))?;
    eprintln!(
        "{} mobile dev server listening on http://{}",
        style("crepus").cyan().bold(),
        addr
    );
    eprintln!(
        "  {} watching {}",
        style("→").dim(),
        template_path.display()
    );
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let state = state.clone();
                std::thread::spawn(move || {
                    let _ = handle_mobile_dev_stream(&mut stream, &state);
                });
            }
            Err(e) => eprintln!("  {} mobile dev connection error: {e}", style("⚠").yellow()),
        }
    }
    Ok(())
}

pub fn start_mobile_watcher(root: PathBuf, state: Arc<MobileDevState>) {
    let (tx, rx) = mpsc::channel::<PathBuf>();
    let watch_root = root.join("views");
    std::thread::spawn(move || {
        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| match res {
                Ok(event) => {
                    if is_template_event(&event) {
                        for path in event.paths {
                            let _ = tx.send(path);
                        }
                    }
                }
                Err(e) => eprintln!("  {} mobile watcher error: {e}", style("⚠").yellow()),
            })
            .expect("crepus mobile dev: cannot create file watcher");
        let _ = watcher.watch(&watch_root, RecursiveMode::Recursive);
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    });

    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            let start = Instant::now();
            while start.elapsed() < Duration::from_millis(50) {
                if rx.try_recv().is_err() {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            refresh_mobile_state(&state);
        }
    });
}

pub fn refresh_mobile_state(state: &MobileDevState) {
    let new_template = match fs::read_to_string(&state.template_path) {
        Ok(value) => value,
        Err(e) => {
            store_event(
                state,
                HotReloadMessage::Error {
                    message: format!("read {}: {e}", state.template_path.display()),
                },
            );
            return;
        }
    };
    let old_template = state
        .last_template
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let rendered_ir = match render_template_to_ir(&new_template, &state.ctx) {
        Ok(ir) => ir,
        Err(e) => {
            store_event(
                state,
                HotReloadMessage::Error {
                    message: e.to_string(),
                },
            );
            return;
        }
    };
    let message = plan_hot_reload(&old_template, &new_template, &state.ctx);
    match &message {
        HotReloadMessage::Patch { .. } | HotReloadMessage::FullReload { .. } => {
            if let Ok(json) = to_json(&rendered_ir) {
                *state
                    .last_ir_json
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = json;
                *state
                    .last_template
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = new_template;
                sync_outputs(&state.root, &state.template_path, state.platform);
            }
        }
        _ => {}
    }
    store_event(state, message);
}

pub fn store_event(state: &MobileDevState, message: HotReloadMessage) {
    let sequence = state.sequence.fetch_add(1, Ordering::SeqCst) + 1;
    let envelope = HotReloadEnvelope { sequence, message };
    *state
        .last_event
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = envelope;
}

pub fn handle_mobile_dev_stream(
    stream: &mut TcpStream,
    state: &MobileDevState,
) -> std::io::Result<()> {
    let mut buf = [0; 4096];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let response = mobile_dev_response(path, state);
    stream.write_all(response.as_bytes())
}

pub fn mobile_dev_response(path: &str, state: &MobileDevState) -> String {
    match path {
        "/health" => {
            let payload = serde_json::json!({
                "ok": true,
                "sequence": state.sequence.load(Ordering::SeqCst)
            });
            http_response("200 OK", "application/json", &payload.to_string())
        }
        "/ir" => {
            let body = state
                .last_ir_json
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            http_response("200 OK", "application/json", &body)
        }
        "/events" => {
            let envelope = state
                .last_event
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let body = format!(
                "event: crepus-mobile\ndata: {}\n\n",
                serde_json::to_string(&envelope).unwrap_or_else(|_| "{}".to_string())
            );
            http_response("200 OK", "text/event-stream", &body)
        }
        _ => http_response("404 Not Found", "text/plain", "not found"),
    }
}

pub fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

pub fn sync_outputs(root: &Path, template_path: &Path, platform: MobilePlatform) {
    let rel = template_path
        .strip_prefix(root)
        .unwrap_or(template_path)
        .to_path_buf();
    let _ = execute(NativeCommands::Sync {
        args: SyncArgs {
            template: rel,
            dir: root.to_path_buf(),
            out: Vec::new(),
            no_defaults: false,
            component: None,
            ctx: None,
            vars: Vec::new(),
            pretty: true,
        },
    });
    let tpl = template_path.to_path_buf();
    if matches!(platform, MobilePlatform::Ios | MobilePlatform::All) {
        let out = root.join("ios/Sources/NativeShell/Generated");
        let _ = execute(NativeCommands::Codegen {
            args: CodegenArgs {
                template: Some(tpl.clone()),
                platform: Some(CodegenPlatform::SwiftUi),
                out: Some(out),
                view_name: Some("CrepusGeneratedView".into()),
                component: None,
                ctx: None,
                vars: Vec::new(),
            },
        });
    }
    if matches!(platform, MobilePlatform::Android | MobilePlatform::All) {
        let out_dir =
            root.join("android/app/src/main/java/dev/crepuscularity/nativeshell/generated");
        let _ = execute(NativeCommands::Codegen {
            args: CodegenArgs {
                template: Some(tpl),
                platform: Some(CodegenPlatform::Compose),
                out: Some(out_dir.clone()),
                view_name: Some("CrepusGeneratedView".into()),
                component: None,
                ctx: None,
                vars: Vec::new(),
            },
        });
        prepend_kotlin_package(&out_dir.join("CrepusGeneratedView.kt"));
    }
}

pub fn is_template_event(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) && event
        .paths
        .iter()
        .any(|path| path.extension().and_then(|e| e.to_str()) == Some("crepus"))
}
