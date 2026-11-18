//! Route parsed `clap` commands to module handlers.

use std::path::PathBuf;
use std::time::Instant;

#[cfg(feature = "benchmark")]
use crate::cli::{BenchmarkCheckArgs, BenchmarkCommands, BenchmarkRunArgs};
use crate::cli::{
    BrowserArg, Cli, Commands, IosCommands, MobileCommands, NativeCommands, TuiCommands,
    WebCommands, WebextCommands,
};
use crate::error::CrepusCliError;
use crate::target_build::ManifestBuildArgs;
use crate::ui;

pub fn run(cli: Cli) -> Result<(), CrepusCliError> {
    let Some(command) = cli.command else {
        print_top_level_help();
        std::process::exit(0);
    };
    match command {
        Commands::New { name } => {
            crate::new::run(&name);
            Ok(())
        }
        Commands::Init { kind, name } => {
            run_init(&kind, &name);
            Ok(())
        }
        #[cfg(feature = "desktop")]
        Commands::Dev {
            target,
            bin,
            emit_events,
            build,
        } => {
            run_dev(target, bin, emit_events, build);
            Ok(())
        }
        Commands::Build {
            build,
            target_id,
            manifest,
            all,
            selector,
        } => run_top_level_build(build, target_id, manifest, all, selector),
        #[cfg(feature = "desktop")]
        Commands::Preview { file } => {
            crate::preview::run_preview(file);
            Ok(())
        }
        Commands::Render {
            file,
            ctx,
            vars,
            component,
        } => {
            crate::render::execute(file, ctx, vars, component);
            Ok(())
        }
        Commands::Inspect {
            file,
            mode,
            vars,
            bools,
            ints,
            width,
            height,
        } => {
            crate::inspect::execute(file, mode, vars, bools, ints, width, height);
            Ok(())
        }
        Commands::Web { command } => {
            crate::web::execute(command);
            Ok(())
        }
        Commands::Webext { command } => {
            crate::webext::execute(command);
            Ok(())
        }
        Commands::Ios { command } => {
            crate::ios::execute(command);
            Ok(())
        }
        Commands::Apple { command } => {
            crate::apple_project::execute(command);
            Ok(())
        }
        Commands::Tui { command } => {
            crate::tui::execute(command);
            Ok(())
        }
        Commands::Native { command } => crate::native::execute(command),
        Commands::Mobile { command } => crate::native::execute_mobile(command),
        Commands::Tauri { command } => {
            crate::tauri::execute(command);
            Ok(())
        }
        Commands::Aurora { aurorality_args } => {
            crate::aurora::run_forwarded(&aurorality_args);
            Ok(())
        }
        Commands::Embedded { command } => {
            crate::embedded::execute(command);
            Ok(())
        }
        #[cfg(feature = "benchmark")]
        Commands::Benchmark { command, flat } => {
            run_benchmark(command, flat);
            Ok(())
        }
        Commands::Plugins { command } => {
            crate::plugins::execute(command);
            Ok(())
        }
        Commands::Flutter { command } => crate::flutter::execute(command),
        Commands::Components { command } => crate::components::execute(command),
        Commands::Moonshine { command } => crate::moonshine::execute(command),
    }
}

#[cfg(feature = "desktop")]
fn run_dev(
    target: Option<String>,
    bin: Option<String>,
    emit_events: bool,
    build: crate::build_options::BuildOptionsArgs,
) {
    let options = build.into_options_or_exit();
    crate::dev::run(target, bin, options, emit_events);
}

#[cfg(feature = "benchmark")]
fn run_benchmark(command: Option<BenchmarkCommands>, flat: BenchmarkRunArgs) {
    match command {
        Some(BenchmarkCommands::Check { args }) => {
            crate::benchmark::execute_check(benchmark_check_options(args));
        }
        Some(BenchmarkCommands::All { args }) | Some(BenchmarkCommands::Run { args }) => {
            crate::benchmark::execute_run(benchmark_run_options(args));
        }
        None => crate::benchmark::execute_run(benchmark_run_options(flat)),
    }
}

fn run_init(kind: &str, name: &str) {
    match kind {
        "web" => crate::web::execute(WebCommands::New {
            name: name.to_string(),
        }),
        "webext" | "extension" | "browser-extension" => {
            crate::webext::execute(WebextCommands::New {
                name: name.to_string(),
            })
        }
        "tui" => crate::tui::execute(TuiCommands::New {
            name: name.to_string(),
        }),
        "native" => {
            let _ = crate::native::execute(NativeCommands::New {
                name: name.to_string(),
            });
        }
        "mobile" => {
            let _ = crate::native::execute_mobile(MobileCommands::New {
                name: name.to_string(),
            });
        }
        "ios" => crate::ios::execute(IosCommands::New {
            name: name.to_string(),
        }),
        #[cfg(feature = "desktop")]
        "gpui" | "app" | "desktop" => crate::new::run(name),
        #[cfg(not(feature = "desktop"))]
        "gpui" | "app" | "desktop" => {
            ui::error("GPUI scaffold not compiled in. Rebuild crepus with --features desktop.");
        }
        "aurora" => crate::aurora::run_forwarded(&["new".to_string(), name.to_string()]),
        other => {
            let mut kinds = "web, webext, tui, native, ios, mobile, aurora".to_string();
            #[cfg(feature = "desktop")]
            {
                kinds.push_str(", gpui");
            }
            #[cfg(not(feature = "desktop"))]
            {
                kinds.push_str(", app");
            }
            ui::error(&format!("unknown init kind {other:?}; expected {kinds}"));
        }
    }
}

fn run_top_level_build(
    build: crate::build_options::BuildOptionsArgs,
    target_id: Option<String>,
    manifest: Option<PathBuf>,
    all: bool,
    selector: Option<String>,
) -> Result<(), CrepusCliError> {
    let options = build.into_options_or_exit();
    let use_manifest = target_id.is_some()
        || manifest.is_some()
        || all
        || selector.is_some()
        || crate::target_build::has_manifest_targets(manifest.clone());
    if use_manifest {
        return crate::target_build::execute(ManifestBuildArgs {
            options,
            target_id,
            selector,
            manifest,
            all,
        });
    }
    let t0 = Instant::now();
    let cwd = std::env::current_dir()
        .map_err(|e| CrepusCliError::context(format!("cannot determine current directory: {e}")))?;
    let sp = ui::spinner(if options.release() {
        "cargo build --release"
    } else {
        "cargo build"
    });
    #[cfg(feature = "desktop")]
    let outcome = crate::builder::cargo_build(&cwd, options, None);
    #[cfg(not(feature = "desktop"))]
    let outcome = crate::preview::cargo_build_fallback(&cwd, options);
    if outcome.success {
        ui::spinner_ok(&sp, "build succeeded");
        ui::done_in(t0.elapsed());
    } else {
        sp.finish_and_clear();
        return Err(CrepusCliError::context("build failed"));
    }
    Ok(())
}

fn print_top_level_help() {
    use clap::CommandFactory;
    let _ = Cli::command().print_help();
}

pub fn browser_target(b: Option<BrowserArg>) -> Option<crepuscularity_webext::BrowserTarget> {
    b.map(|v| match v {
        BrowserArg::Chromium => crepuscularity_webext::BrowserTarget::Chromium,
        BrowserArg::Firefox => crepuscularity_webext::BrowserTarget::Firefox,
        BrowserArg::Safari => crepuscularity_webext::BrowserTarget::Safari,
    })
}

#[cfg(feature = "benchmark")]
pub fn benchmark_run_options(args: BenchmarkRunArgs) -> crate::benchmark::RunOptions {
    let verbose = args.verbose && !args.json && !args.quiet;
    let measure_memory = !args.no_memory;
    crate::benchmark::RunOptions {
        config_path: args.common.config,
        suite_filter: args.common.suite,
        only: args.common.only.map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        }),
        json_out: args.json,
        dry_run: args.dry_run,
        clean: args.clean,
        repo_override: args.common.repo,
        work_override: args.work_root,
        verbose,
        measure_memory,
        #[cfg(feature = "tui")]
        no_tui: args.no_tui,
    }
}

#[cfg(feature = "benchmark")]
pub fn benchmark_check_options(args: BenchmarkCheckArgs) -> crate::benchmark::CheckOptions {
    crate::benchmark::CheckOptions {
        config_path: args.common.config,
        suite_filter: args.common.suite,
        only: args.common.only.map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        }),
        json_out: args.json,
        repo_override: args.common.repo,
    }
}
