//! Typed CLI surface (`clap`). Command handlers live in sibling modules.

use std::path::PathBuf;

#[cfg(feature = "benchmark")]
use clap::Args;
use clap::{Parser, Subcommand, ValueEnum};

use crate::build_options::BuildOptionsArgs;

#[derive(Clone, Debug, ValueEnum, Default)]
pub enum InspectMode {
    Ast,
    Render,
    Ir,
    Ctx,
    #[default]
    Preview,
}

#[derive(Parser, Debug)]
#[command(
    name = "crepus",
    version,
    about = "Crepuscularity CLI — scaffold and build multi-target .crepus apps",
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    New {
        name: String,
    },
    Init {
        kind: String,
        name: String,
    },
    #[cfg(feature = "desktop")]
    Dev {
        /// Target kind or id from crepus.toml (e.g. "gpui", "tui", "ios").
        /// When given, resolves the target directory from crepus.toml
        /// and runs the dev loop there. "tui" uses terminal-only mode
        /// (no GPUI HUD); "gpui" opens the DevHUD window.
        target: Option<String>,
        #[arg(long)]
        bin: Option<String>,
        #[arg(long)]
        emit_events: bool,
        #[command(flatten)]
        build: BuildOptionsArgs,
    },
    Build {
        #[command(flatten)]
        build: BuildOptionsArgs,
        #[arg(long = "target", short = 't')]
        target_id: Option<String>,
        #[arg(long)]
        manifest: Option<PathBuf>,
        #[arg(long)]
        all: bool,
        selector: Option<String>,
    },
    #[cfg(feature = "desktop")]
    Preview {
        file: PathBuf,
    },
    Render {
        file: PathBuf,
        #[arg(long)]
        ctx: Option<PathBuf>,
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
        #[arg(long)]
        component: Option<String>,
    },
    /// Inspect a `.crepus` file (AST / HTML / View IR / context / live preview).
    /// Replaces the old `crepus-dev` binary.
    Inspect {
        file: PathBuf,
        #[arg(long, value_enum, default_value_t = InspectMode::Preview)]
        mode: InspectMode,
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
        #[arg(long = "bool", value_name = "KEY=BOOL")]
        bools: Vec<String>,
        #[arg(long = "int", value_name = "KEY=INT")]
        ints: Vec<String>,
        #[arg(long, default_value_t = 1200.0)]
        width: f32,
        #[arg(long, default_value_t = 800.0)]
        height: f32,
    },
    Web {
        #[command(subcommand)]
        command: WebCommands,
    },
    Webext {
        #[command(subcommand)]
        command: WebextCommands,
    },
    Ios {
        #[command(subcommand)]
        command: IosCommands,
    },
    Apple {
        #[command(subcommand)]
        command: AppleCommands,
    },
    Tui {
        #[command(subcommand)]
        command: TuiCommands,
    },
    Native {
        #[command(subcommand)]
        command: NativeCommands,
    },
    Mobile {
        #[command(subcommand)]
        command: MobileCommands,
    },
    Tauri {
        #[command(subcommand)]
        command: TauriCommands,
    },
    Aurora {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        aurorality_args: Vec<String>,
    },
    Embedded {
        #[command(subcommand)]
        command: EmbeddedCommands,
    },
    /// Declarative benchmark.toml runs (flags work without `all`/`run` subcommand)
    #[cfg(feature = "benchmark")]
    Benchmark {
        #[command(subcommand)]
        command: Option<BenchmarkCommands>,
        #[command(flatten)]
        flat: BenchmarkRunArgs,
    },
    /// Plugin packages and ABI bindgen (`equilibrium-ffi`, like `eq generate`)
    Plugins {
        #[command(subcommand)]
        command: PluginsCommands,
    },
    /// Flutter runtime renderer (`crepuscularity-flutter`) dependency helper.
    Flutter {
        #[command(subcommand)]
        command: FlutterCommands,
    },
    /// Shared UI component catalog (`crepuscularity-components`).
    Components {
        #[command(subcommand)]
        command: ComponentsCommands,
    },
    /// Moonshine + Crepuscularity web runtime helper.
    #[command(alias = "moon")]
    Moonshine {
        #[command(subcommand)]
        command: MoonshineCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum FlutterCommands {
    /// Print the pubspec dependency spec for the Flutter renderer (for CI).
    Dep {
        /// Emit a hosted pub.dev dependency at this version instead of a git ref.
        #[arg(long)]
        version: Option<String>,
        /// Git ref to pin when emitting a git dependency (default: main).
        #[arg(long, default_value = "main")]
        git_ref: String,
    },
    /// Add the Flutter renderer dependency to a Flutter app's pubspec.yaml.
    Add {
        /// App directory containing pubspec.yaml (default: current directory).
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Emit a hosted pub.dev dependency at this version instead of a git ref.
        #[arg(long)]
        version: Option<String>,
        /// Git ref to pin when adding a git dependency (default: main).
        #[arg(long, default_value = "main")]
        git_ref: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ComponentsCommands {
    /// List components from plugins/crepuscularity-components/catalog/components.json.
    List,
    /// Print install / path hints for a catalog component.
    Add {
        /// Component id from the catalog (e.g. `button`).
        id: String,
        /// Restrict hints to one target runtime.
        #[arg(long, value_enum)]
        target: Option<ComponentTarget>,
    },
    /// List theme names under catalog/themes/.
    Themes,
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum ComponentTarget {
    Flutter,
    Svelte,
    Moonshine,
    Gpui,
}

#[derive(Subcommand, Debug)]
pub enum MoonshineCommands {
    /// Scaffold a Moonshine + Crepus app under cwd.
    ///
    /// By default this scaffolds a Rust-only project: crepus templates are
    /// inlined in `src/main.rs` and built into a real Moonshine site via
    /// `crepus moon build`. Pass `--js` for the legacy React + Vite scaffold
    /// with a hardcoded View IR blob in `src/main.tsx`.
    New {
        name: String,
        /// Scaffold the legacy JavaScript/TSX app instead of the Rust-only app.
        #[arg(long)]
        js: bool,
    },
    /// Print package.json dependency snippets for moonshine + crepus packages.
    Dep,
    /// Build a Rust-only `crepus moon new` project into a real Moonshine site.
    ///
    /// Runs `cargo run` in the project to emit the generated app, refreshes
    /// package.json/index.html/vite.config.ts/tsconfig.json, copies `ts/`
    /// into the app, then runs `bun install && bun run build` if `bun` is
    /// on PATH.
    Build {
        /// Project directory (default: current directory).
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

/// Output target for `crepus web build --emit`.
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq, Default)]
pub enum WebEmitTarget {
    #[default]
    Html,
    Moonshine,
    Svelte,
    Vue,
    Solid,
}

#[derive(Subcommand, Debug)]
pub enum WebCommands {
    New {
        name: String,
    },
    Build {
        #[command(flatten)]
        build: BuildOptionsArgs,
        #[arg(long)]
        site: Option<PathBuf>,
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[arg(short = 'o', long = "output", alias = "out")]
        output: Option<PathBuf>,
        #[arg(long)]
        entry: Option<String>,
        #[arg(long = "target", short = 't')]
        target_id: Option<String>,
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// Framework emit target (default: html WASM site). Non-html writes a stub under dist/.
        #[arg(long, value_enum, default_value_t = WebEmitTarget::Html)]
        emit: WebEmitTarget,
    },
    #[command(alias = "serve")]
    Dev {
        #[arg(long)]
        site: Option<PathBuf>,
        #[arg(long, default_value_t = 4000)]
        port: u16,
        #[arg(long, default_value = "index.crepus")]
        entry: String,
        #[arg(long = "target", short = 't')]
        target_id: Option<String>,
        #[arg(long)]
        manifest: Option<PathBuf>,
        /// Accepted for parity with `web build`; only `html` is meaningful for the dev server.
        #[arg(long, value_enum, default_value_t = WebEmitTarget::Html)]
        emit: WebEmitTarget,
    },
    #[command(name = "build-full")]
    BuildFull {
        #[arg(long)]
        site: Option<PathBuf>,
        #[arg(long)]
        wasm: bool,
        #[arg(long)]
        server: bool,
        /// Accepted for parity with `web build`; `build-full` always emits the HTML/WASM site.
        #[arg(long, value_enum, default_value_t = WebEmitTarget::Html)]
        emit: WebEmitTarget,
    },
}

#[derive(Subcommand, Debug)]
pub enum WebextCommands {
    New {
        name: String,
    },
    Build {
        #[command(flatten)]
        build: BuildOptionsArgs,
        #[arg(long)]
        app: Option<PathBuf>,
        #[arg(long)]
        browser: Option<BrowserArg>,
    },
    Dev {
        #[command(flatten)]
        build: BuildOptionsArgs,
        #[arg(long)]
        app: Option<PathBuf>,
        #[arg(long)]
        browser: Option<BrowserArg>,
    },
    Manifest {
        #[arg(long)]
        app: Option<PathBuf>,
        #[arg(long)]
        browser: Option<BrowserArg>,
    },
}

#[derive(Clone, Debug, ValueEnum)]
pub enum BrowserArg {
    Chromium,
    Firefox,
    Safari,
}

#[derive(Subcommand, Debug)]
pub enum IosCommands {
    New {
        name: String,
    },
    #[command(alias = "gen")]
    Generate {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        spec: Option<String>,
    },
    Build {
        #[command(flatten)]
        build: BuildOptionsArgs,
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long)]
        spec: Option<String>,
        #[arg(long)]
        scheme: Option<String>,
        #[arg(long)]
        destination: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum AppleCommands {
    Generate {
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    Build {
        #[command(flatten)]
        build: BuildOptionsArgs,
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum TuiCommands {
    New {
        name: String,
    },
    Build {
        #[command(flatten)]
        build: BuildOptionsArgs,
        #[arg(last = true)]
        extra: Vec<String>,
    },
    Run {
        #[command(flatten)]
        build: BuildOptionsArgs,
        #[arg(last = true)]
        extra: Vec<String>,
    },
    Preview {
        file: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum NativeCommands {
    New {
        name: String,
    },
    Add {
        capability: String,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    Extension {
        #[command(subcommand)]
        extension: NativeExtensionCommands,
    },
    Ir {
        #[command(flatten)]
        args: crate::native::IrArgs,
    },
    Sync {
        #[command(flatten)]
        args: crate::native::SyncArgs,
    },
    Codegen {
        #[command(flatten)]
        args: crate::native::CodegenArgs,
    },
    Build {
        #[command(subcommand)]
        platform: NativeBuildCommands,
    },
    Run {
        #[command(subcommand)]
        platform: NativeRunCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum NativeExtensionCommands {
    #[command(name = "ios-share")]
    IosShare {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, default_value = "CrepusShareExtension")]
        name: String,
    },
    #[command(name = "macos-share")]
    MacosShare {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, default_value = "CrepusMacShareExtension")]
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum NativeBuildCommands {
    Ios {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long, default_value = "simulator")]
        target: crate::native::IosBuildTarget,
        #[arg(long, default_value = "Debug")]
        configuration: String,
    },
    Android {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long, default_value = "Debug")]
        flavor: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum NativeRunCommands {
    Ios {
        #[arg(long)]
        dir: Option<PathBuf>,
    },
    Android {
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long, default_value = "Debug")]
        flavor: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum MobileCommands {
    New {
        name: String,
    },
    Dev {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long, default_value_t = 4001)]
        port: u16,
        #[arg(long, default_value = "all")]
        platform: MobilePlatformArg,
        #[arg(long, default_value = "views/main.crepus")]
        template: PathBuf,
        #[arg(long)]
        ctx: Option<PathBuf>,
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
    },
    Build {
        #[arg(long, default_value = "all")]
        platform: MobilePlatformArg,
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long, default_value = "simulator")]
        target: crate::native::IosBuildTarget,
        #[arg(long, default_value = "Debug")]
        configuration: String,
        #[arg(long, default_value = "Debug")]
        flavor: String,
    },
    Run {
        #[arg(long, default_value = "android")]
        platform: MobilePlatformArg,
        #[arg(long)]
        dir: Option<PathBuf>,
        #[arg(long, default_value = "Debug")]
        flavor: String,
    },
    Doctor {
        #[arg(long, default_value = "all")]
        platform: MobilePlatformArg,
    },
    Sync {
        #[arg(default_value = "views/main.crepus")]
        template: PathBuf,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
        #[arg(long)]
        pretty: bool,
    },
    Codegen {
        template: Option<PathBuf>,
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        platform: Option<MobileCodegenPlatformArg>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        view_name: Option<String>,
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum TauriCommands {
    Audit {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    Convert {
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "all")]
        target: TauriTargetArg,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub enum TauriTargetArg {
    All,
    Desktop,
    Mobile,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum MobilePlatformArg {
    #[value(name = "ios", alias = "swift", alias = "swiftui")]
    Ios,
    #[value(name = "android", alias = "compose", alias = "kotlin")]
    Android,
    All,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum MobileCodegenPlatformArg {
    #[value(name = "ios", alias = "swift", alias = "swiftui")]
    Ios,
    #[value(name = "android", alias = "compose", alias = "kotlin")]
    Android,
}

#[derive(Subcommand, Debug)]
pub enum EmbeddedCommands {
    Check {
        file: PathBuf,
        #[arg(long)]
        component: Option<String>,
    },
    Snapshot {
        file: PathBuf,
        #[arg(long)]
        width: u16,
        #[arg(long)]
        height: u16,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        ctx: Option<PathBuf>,
        #[arg(long = "var", value_name = "KEY=VALUE")]
        vars: Vec<String>,
        #[arg(long)]
        component: Option<String>,
    },
}

#[cfg(feature = "benchmark")]
#[derive(Subcommand, Debug)]
pub enum BenchmarkCommands {
    All {
        #[command(flatten)]
        args: BenchmarkRunArgs,
    },
    Run {
        #[command(flatten)]
        args: BenchmarkRunArgs,
    },
    Check {
        #[command(flatten)]
        args: BenchmarkCheckArgs,
    },
}

#[cfg(feature = "benchmark")]
#[derive(Args, Debug, Clone, Default)]
pub struct BenchmarkCommonArgs {
    #[arg(short = 'c', long = "config")]
    pub config: Option<PathBuf>,
    #[arg(short = 's', long = "suite")]
    pub suite: Option<String>,
    #[arg(long)]
    pub only: Option<String>,
    #[arg(long)]
    pub repo: Option<PathBuf>,
}

#[cfg(feature = "benchmark")]
#[derive(Args, Debug, Clone, Default)]
pub struct BenchmarkRunArgs {
    #[command(flatten)]
    pub common: BenchmarkCommonArgs,
    #[arg(long)]
    pub json: bool,
    #[arg(short = 'n', long = "dry-run")]
    pub dry_run: bool,
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
    #[arg(long)]
    pub clean: bool,
    #[arg(long = "work-root")]
    pub work_root: Option<PathBuf>,
    #[arg(long)]
    pub memory: bool,
    #[arg(long = "no-memory")]
    pub no_memory: bool,
    #[cfg(feature = "tui")]
    #[arg(long = "no-tui")]
    pub no_tui: bool,
}

#[cfg(feature = "benchmark")]
#[derive(Args, Debug, Clone, Default)]
pub struct BenchmarkCheckArgs {
    #[command(flatten)]
    pub common: BenchmarkCommonArgs,
    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum PluginsCommands {
    /// Generate ABI bindings for every package in crepuscularity-plugins.toml (equilibrium-ffi)
    Bindgen {
        #[arg(long, help = "Path to crepuscularity-plugins.toml")]
        manifest: Option<PathBuf>,
        #[arg(long, help = "Override ABI header (default: contract.abi_header)")]
        abi_header: Option<PathBuf>,
        #[arg(long, help = "Base output dir (default: <repo>/plugins)")]
        out_dir: Option<PathBuf>,
    },
    /// Run each package `test` command from the manifest
    Test {
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
}

pub fn parse() -> Cli {
    Cli::parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_components_list() {
        let cli = Cli::try_parse_from(["crepus", "components", "list"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Components {
                command: ComponentsCommands::List
            })
        ));
    }

    #[test]
    fn parses_components_add_with_target() {
        let cli = Cli::try_parse_from([
            "crepus",
            "components",
            "add",
            "button",
            "--target",
            "moonshine",
        ])
        .expect("parse");
        match cli.command {
            Some(Commands::Components {
                command: ComponentsCommands::Add { id, target },
            }) => {
                assert_eq!(id, "button");
                assert_eq!(target, Some(ComponentTarget::Moonshine));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parses_moonshine_dep() {
        let cli = Cli::try_parse_from(["crepus", "moonshine", "dep"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Moonshine {
                command: MoonshineCommands::Dep
            })
        ));
    }

    #[test]
    fn parses_moon_alias_new_same_as_moonshine_new() {
        let moon = Cli::try_parse_from(["crepus", "moon", "new", "x"]).expect("parse moon");
        let moonshine =
            Cli::try_parse_from(["crepus", "moonshine", "new", "x"]).expect("parse moonshine");
        match (moon.command, moonshine.command) {
            (
                Some(Commands::Moonshine {
                    command: MoonshineCommands::New { name: n1, js: j1 },
                }),
                Some(Commands::Moonshine {
                    command: MoonshineCommands::New { name: n2, js: j2 },
                }),
            ) => {
                assert_eq!(n1, "x");
                assert_eq!(n2, "x");
                assert_eq!(j1, j2);
                assert!(!j1);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parses_moonshine_new_js_flag() {
        let cli = Cli::try_parse_from(["crepus", "moonshine", "new", "x", "--js"]).expect("parse");
        match cli.command {
            Some(Commands::Moonshine {
                command: MoonshineCommands::New { name, js },
            }) => {
                assert_eq!(name, "x");
                assert!(js);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn parses_moon_build() {
        let cli = Cli::try_parse_from(["crepus", "moon", "build"]).expect("parse");
        assert!(matches!(
            cli.command,
            Some(Commands::Moonshine {
                command: MoonshineCommands::Build { dir: None }
            })
        ));
    }

    #[test]
    fn parses_web_build_emit_moonshine() {
        let cli =
            Cli::try_parse_from(["crepus", "web", "build", "--emit", "moonshine"]).expect("parse");
        match cli.command {
            Some(Commands::Web {
                command: WebCommands::Build { emit, .. },
            }) => assert_eq!(emit, WebEmitTarget::Moonshine),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn web_build_emit_defaults_to_html() {
        let cli = Cli::try_parse_from(["crepus", "web", "build"]).expect("parse");
        match cli.command {
            Some(Commands::Web {
                command: WebCommands::Build { emit, .. },
            }) => assert_eq!(emit, WebEmitTarget::Html),
            other => panic!("unexpected: {other:?}"),
        }
    }
}
