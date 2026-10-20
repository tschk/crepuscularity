//! `crepus moonshine` — scaffold and dependency helper for Moonshine + Crepus apps.
//!
//! Moonshine is an external product: <https://github.com/tschk/moonshine>
//! (local checkout often at `~/projects/moonshine`). Prefer the `moonshine` CLI
//! from that repo when available; `crepus moonshine` remains a working fallback.
//!
//! Crepuscularity compiles `.crepus` → View IR and emits apps that import
//! `@tschk/crepus-moonshine` (`crepus-emit.moonshine.tsx`).
//!
//! - `crepus moonshine new <name>` (alias `crepus moon new`) scaffolds a
//!   Rust-only app: crepus templates inlined in `src/main.rs`, built into a
//!   real Moonshine site via `crepus moon build`. Pass `--js` for the legacy
//!   React + Vite scaffold with a hardcoded View IR blob in `src/main.tsx`.
//! - `crepus moonshine build` (alias `crepus moon build`) builds a Rust-only
//!   scaffold end to end: `cargo run` emits the app, then package.json /
//!   index.html / vite.config.ts / tsconfig.json are refreshed and `bun
//!   install && bun run build` runs if `bun` is on PATH.
//! - `crepus moonshine dep` prints package.json dependency snippets.

use std::env;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::cli::MoonshineCommands;
use crate::error::CrepusCliError;
use crate::scaffold;
use crate::ui;

/// Resolve a local tschk/moonshine checkout, if present.
///
/// Order: `MOONSHINE_PATH` → `../moonshine` relative to cwd → `~/projects/moonshine`.
fn resolve_moonshine_root() -> Option<PathBuf> {
    if let Ok(raw) = env::var("MOONSHINE_PATH") {
        let p = PathBuf::from(raw);
        if looks_like_moonshine_checkout(&p) {
            return Some(canonicalize_or_self(&p));
        }
    }

    let cwd_sibling = env::current_dir()
        .ok()
        .map(|cwd| cwd.join("..").join("moonshine"));
    if let Some(p) = cwd_sibling {
        if looks_like_moonshine_checkout(&p) {
            return Some(canonicalize_or_self(&p));
        }
    }

    if let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) {
        let p = PathBuf::from(home).join("projects").join("moonshine");
        if looks_like_moonshine_checkout(&p) {
            return Some(canonicalize_or_self(&p));
        }
    }

    None
}

fn looks_like_moonshine_checkout(root: &Path) -> bool {
    root.join("packages/core/package.json").is_file()
        && root
            .join("packages/crepus-moonshine/package.json")
            .is_file()
        && root.join("components/package.json").is_file()
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// `file:` dependency paths for the three @tschk packages.
///
/// Always uses absolute `file:` URLs so scaffolds outside the moonshine tree
/// (e.g. `/tmp/app`) don't get broken `../../Users/...` relatives.
fn file_dep_paths(moonshine_root: &Path, _from: Option<&Path>) -> (String, String, String) {
    let core = moonshine_root.join("packages/core");
    let crepus = moonshine_root.join("packages/crepus-moonshine");
    let components = moonshine_root.join("components");

    let fmt =
        |target: &Path| -> String { format!("file:{}", canonicalize_or_self(target).display()) };

    (fmt(&core), fmt(&crepus), fmt(&components))
}

fn package_json_template(slug: &str, core: &str, crepus: &str, components: &str) -> String {
    let slug = serde_json::json!(slug);
    let core = serde_json::json!(core);
    let crepus = serde_json::json!(crepus);
    let components = serde_json::json!(components);
    format!(
        r#"{{
  "name": {slug},
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {{
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview"
  }},
  "dependencies": {{
    "@tschk/moonshine": {core},
    "@tschk/crepus-moonshine": {crepus},
    "@tschk/moonshine-components": {components},
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  }},
  "devDependencies": {{
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.5.0",
    "typescript": "^5.8.0",
    "vite": "^6.0.0"
  }}
}}
"#
    )
}

// Used when no local moonshine checkout is found, which is the normal case
// outside this developer's machine. A `file:` path here would resolve against a
// sibling directory the user does not have, so `bun install` would fail on a
// freshly scaffolded project; the published packages are the working default.
const PLACEHOLDER_CORE: &str = "^0.4.49";
const PLACEHOLDER_CREPUS: &str = "^0.4.49";
const PLACEHOLDER_COMPONENTS: &str = "^0.4.49";

const INDEX_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{{name}}</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#;

const MAIN_TSX: &str = r##"import { createApp } from "@tschk/moonshine/react";
import { renderCrepusIr } from "@tschk/crepus-moonshine";
import type { ViewIr } from "@tschk/crepus-moonshine";
import { Sparkline } from "@tschk/moonshine-components";
import "./app.css";

const sampleIr = {
  version: 1,
  root: [
    {
      kind: "stack",
      axis: "column",
      gap: 16,
      children: [
        {
          kind: "text",
          content: "{{name}}",
          style: { fontSize: 28, fontWeight: 700 },
        },
        {
          kind: "text",
          content: "Moonshine + Crepuscularity View IR",
          style: { opacity: 0.7 },
        },
        {
          kind: "badge",
          label: "renderCrepusIr",
          tone: "accent",
        },
        {
          kind: "button",
          label: "Ping",
          onClick: "ping",
        },
      ],
    },
  ],
} as const satisfies ViewIr;

function App() {
  return (
    <main className="shell">
      {renderCrepusIr(sampleIr, {
        onAction: (handler) => console.log("action:", handler),
      })}
      <section className="spark">
        <h2>Sparkline</h2>
        <Sparkline values={[2, 4, 3, 7, 5, 9, 6, 8, 10, 7]} color="blue" height={56} />
      </section>
    </main>
  );
}

createApp({ root: App }).mount("#app");
"##;

const APP_CSS: &str = r#":root {
  color-scheme: light dark;
  font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
  background:
    radial-gradient(1200px 600px at 10% -10%, #1e3a5f55, transparent),
    radial-gradient(900px 500px at 100% 0%, #0f766e33, transparent),
    #0c1117;
  color: #e8eef6;
}

body {
  margin: 0;
  min-height: 100vh;
}

.shell {
  max-width: 36rem;
  margin: 3.5rem auto;
  padding: 0 1.25rem;
  display: grid;
  gap: 1.5rem;
}

.spark h2 {
  margin: 0 0 0.5rem;
  font-size: 0.85rem;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  opacity: 0.65;
  font-weight: 600;
}
"#;

const INDEX_CREPUS: &str = r#"div w-full min-h-screen p-8 flex flex-col gap-3
 div text-3xl font-bold
  "Hello from Moonshine + .crepus"
 div text-zinc-400
  "Scaffolded by crepus moonshine new."
"#;

const VITE_CONFIG: &str = r#"import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  publicDir: "public",
  server: { port: 5173 },
});
"#;

const TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "jsx": "react-jsx",
    "strict": true,
    "skipLibCheck": true,
    "noEmit": true,
    "isolatedModules": true,
    "resolveJsonModule": true
  },
  "include": ["src"]
}
"#;

fn readme_template(local_hint: &str) -> String {
    format!(
        r#"# {{{{name}}}}

React + Vite app scaffolded by `crepus moonshine new`.

Moonshine is a separate product: https://github.com/tschk/moonshine
(local checkout often at `~/projects/moonshine`).

When available, prefer the `moonshine` CLI from tschk/moonshine for new apps.
`crepus moonshine` remains a supported Crepuscularity fallback.

## Setup

{local_hint}

```bash
bun install
bun run dev
```

Emit a View IR app entry from `.crepus` (`dist/crepus-emit.moonshine.tsx`):

```bash
crepus web build --emit moonshine --site .
```

Dependency snippets:

```bash
crepus moonshine dep
```
"#
    )
}

pub fn execute(cmd: MoonshineCommands) -> Result<(), CrepusCliError> {
    match cmd {
        MoonshineCommands::New { name, js } => {
            if js {
                scaffold_app(&name);
            } else {
                rust_scaffold::scaffold_rust_app(&name);
            }
            Ok(())
        }
        MoonshineCommands::Dep => {
            print!("{}", dependency_snippets());
            Ok(())
        }
        MoonshineCommands::Build { dir } => rust_build::build(dir),
    }
}

fn dependency_snippets() -> String {
    let mut out = String::from(
        r#"# package.json dependencies for Moonshine + Crepuscularity
#
# Moonshine: https://github.com/tschk/moonshine
# Do NOT use bun's github-repo + nested path form — it 404s. Use file: paths instead.
"#,
    );

    if let Some(root) = resolve_moonshine_root() {
        let (core, crepus, components) = file_dep_paths(&root, env::current_dir().ok().as_deref());
        let core = serde_json::json!(core);
        let crepus = serde_json::json!(crepus);
        let components = serde_json::json!(components);
        out.push_str(&format!(
            r#"
# A) Local checkout detected at {}
#    Prefer file: paths (relative to cwd when possible):

{{
  "dependencies": {{
    "@tschk/moonshine": {core},
    "@tschk/crepus-moonshine": {crepus},
    "@tschk/moonshine-components": {components},
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  }}
}}
"#,
            root.display()
        ));
    } else {
        out.push_str(
            r#"
# A) No local moonshine checkout found.
#    Set MOONSHINE_PATH, place a checkout at ../moonshine, or ~/projects/moonshine.
#    Placeholder file: paths (clone first — see B):

{
  "dependencies": {
    "@tschk/moonshine": "file:../moonshine/packages/core",
    "@tschk/crepus-moonshine": "file:../moonshine/packages/crepus-moonshine",
    "@tschk/moonshine-components": "file:../moonshine/components",
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  }
}
"#,
        );
    }

    out.push_str(
        r#"
# B) Clone tschk/moonshine, then point file: deps at that tree:

git clone https://github.com/tschk/moonshine.git ../moonshine
# or: git clone https://github.com/tschk/moonshine.git ~/projects/moonshine

# Then either re-run `crepus moonshine dep` / `crepus moonshine new`, or set:
#   export MOONSHINE_PATH=/absolute/path/to/moonshine
"#,
    );

    out
}

fn scaffold_app(name: &str) {
    let t0 = Instant::now();
    let slug = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>();
    let base = PathBuf::from(&slug);
    if base.exists() {
        ui::error(&format!("directory already exists: {slug}"));
    }

    let app_abs = env::current_dir()
        .map(|cwd| cwd.join(&base))
        .unwrap_or_else(|_| base.clone());

    let (core, crepus, components, local_hint) = match resolve_moonshine_root() {
        Some(root) => {
            let (c, cr, co) = file_dep_paths(&root, Some(&app_abs));
            let hint = format!(
                "Local moonshine detected at `{}` — `package.json` uses `file:` deps.",
                root.display()
            );
            eprintln!("{} {hint}", ui::ok());
            (c, cr, co, hint)
        }
        None => {
            eprintln!(
                "{} no local moonshine checkout found (MOONSHINE_PATH, ../moonshine, or ~/projects/moonshine)",
                ui::warn()
            );
            eprintln!(
                "  {} scaffolding with published npm packages (^0.4.49); no local checkout is required",
                ui::dim("→")
            );
            (
                PLACEHOLDER_CORE.to_string(),
                PLACEHOLDER_CREPUS.to_string(),
                PLACEHOLDER_COMPONENTS.to_string(),
                r#"No local moonshine checkout was found. `package.json` uses the published
`@tschk/moonshine`, `@tschk/crepus-moonshine`, and `@tschk/moonshine-components`
packages at `^0.4.49`. Run `bun install`; no local checkout is required.

For local Moonshine development, clone the repository and set `MOONSHINE_PATH`
before scaffolding to select local `file:` dependencies."#
                    .to_string(),
            )
        }
    };

    scaffold::ensure_dir(&base.join("src"))
        .unwrap_or_else(|e| ui::error(&format!("create src: {e}")));
    scaffold::ensure_dir(&base.join("public"))
        .unwrap_or_else(|e| ui::error(&format!("create public: {e}")));

    let pkg = package_json_template(&slug, &core, &crepus, &components);
    scaffold::write_file(&base.join("package.json"), &pkg)
        .unwrap_or_else(|e| ui::error(&format!("write package.json: {e}")));
    scaffold::write_template(&base.join("index.html"), INDEX_HTML, &[("{{name}}", name)])
        .unwrap_or_else(|e| ui::error(&format!("write index.html: {e}")));
    scaffold::write_template(&base.join("src/main.tsx"), MAIN_TSX, &[("{{name}}", name)])
        .unwrap_or_else(|e| ui::error(&format!("write src/main.tsx: {e}")));
    scaffold::write_file(&base.join("src/app.css"), APP_CSS)
        .unwrap_or_else(|e| ui::error(&format!("write src/app.css: {e}")));
    scaffold::write_file(&base.join("public/index.crepus"), INDEX_CREPUS)
        .unwrap_or_else(|e| ui::error(&format!("write public/index.crepus: {e}")));
    // Root index.crepus for `crepus web build --emit moonshine`.
    scaffold::write_file(&base.join("index.crepus"), INDEX_CREPUS)
        .unwrap_or_else(|e| ui::error(&format!("write index.crepus: {e}")));
    scaffold::write_file(&base.join("vite.config.ts"), VITE_CONFIG)
        .unwrap_or_else(|e| ui::error(&format!("write vite.config.ts: {e}")));
    scaffold::write_file(&base.join("tsconfig.json"), TSCONFIG)
        .unwrap_or_else(|e| ui::error(&format!("write tsconfig.json: {e}")));
    let readme = readme_template(&local_hint);
    scaffold::write_template(&base.join("README.md"), &readme, &[("{{name}}", name)])
        .unwrap_or_else(|e| ui::error(&format!("write README.md: {e}")));

    eprintln!(
        "  {} prefer `moonshine` CLI (tschk/moonshine) when available; `crepus moonshine` stays supported",
        ui::dim("→")
    );
    scaffold::scaffold_success(
        &slug,
        &base,
        &[
            &format!("cd {slug}"),
            "bun install",
            "bun run dev",
            "crepus moonshine dep   # dependency snippets",
            "crepus web build --emit moonshine --site .",
        ],
    );
    ui::done_in(t0.elapsed());
}

/// `crepus moon new` — the Rust-only scaffold: crepus templates inlined as
/// raw string literals in `src/main.rs`, parsed with
/// `crepuscularity_native::render_template_to_ir` and emitted with
/// `emit_moonshine_app` / `emit_moonshine_component` at `cargo run` time.
mod rust_scaffold {
    use std::time::Instant;

    use crate::scaffold;
    use crate::ui;

    const SCAFFOLD_NATIVE_VERSION: &str = "0.7.0";

    const CARGO_TOML: &str = r#"[package]
name = "{{slug}}"
version = "0.1.0"
edition = "2021"

[dependencies]
crepuscularity-native = "{{native_version}}"
"#;

    const MAIN_RS: &str = r####"//! Generated by `crepus moon new`. crepus templates are inlined here as raw
//! string literals — edit them, then run `cargo run` (or `crepus moon build`)
//! to regenerate the Moonshine app under `generated/`.
//!
//! Drop TypeScript modules in `ts/` — they are copied into the built app by
//! `crepus moon build` and stay importable from the generated TSX.

use crepuscularity_native::{emit_moonshine_app, emit_moonshine_component, render_template_to_ir};

/// The page — becomes `generated/app.tsx` (the Moonshine app entry).
///
/// `if`, `for` and `@click` become real JSX: the emitted component takes a
/// `scope` prop supplying the values expressions read, and a `handlers` prop
/// supplying the functions events call. Both are wired in `src/main.tsx`.
const PAGE: &str = r###"div w-full min-h-screen p-8 flex flex-col gap-4
 div text-3xl font-bold
  "{{name}}"
 div text-zinc-400
  "Crepus templates inlined in src/main.rs, built with crepus moon build."
 if {count > 0}
  div text-emerald-400
   "count is above zero"
 else
  div text-zinc-500
   "count is zero"
 button @click=increment
  "Increment"
 ul
  for item in {items}
   li
    "{item}"
"###;

/// A reusable component — becomes `generated/components/Card.tsx`.
const CARD: &str = r###"div rounded-lg border p-4 flex flex-col gap-2
 div text-lg font-semibold
  "Card"
 div text-sm text-zinc-400
  "Emitted via emit_moonshine_component."
"###;

fn main() {
    let page_ir = render_template_to_ir(PAGE, &Default::default()).expect("render PAGE template");
    let app_tsx = emit_moonshine_app(&page_ir);

    let card_ir = render_template_to_ir(CARD, &Default::default()).expect("render CARD template");
    let card_tsx = emit_moonshine_component(&card_ir, "Card");

    std::fs::create_dir_all("generated/components").expect("create generated/components");
    std::fs::write("generated/app.tsx", app_tsx).expect("write generated/app.tsx");
    std::fs::write("generated/components/Card.tsx", card_tsx)
        .expect("write generated/components/Card.tsx");

    println!("wrote generated/app.tsx and generated/components/Card.tsx");
    println!("next: crepus moon build");
}
"####;

    const EXAMPLE_TS: &str = r#"// Example TypeScript module — imported by the generated app, or import it
// yourself from src/ once `crepus moon build` copies this directory in.
export function greet(name: string): string {
  return `Hello, ${name}! (from ts/example.ts)`;
}
"#;

    fn readme_template() -> &'static str {
        r#"# {{name}}

Rust-only Moonshine app scaffolded by `crepus moon new` (alias for
`crepus moonshine new`).

crepus templates are inlined as raw string literals in `src/main.rs`. Running
the project (`cargo run`) parses them with
`crepuscularity_native::render_template_to_ir` and emits a real Moonshine
React/TSX app under `generated/` via `emit_moonshine_app` /
`emit_moonshine_component`.

## Build

```bash
crepus moon build
```

This runs `cargo run` to (re)generate `generated/`, refreshes
`package.json`, `index.html`, `vite.config.ts`, and `tsconfig.json`, copies
`ts/` into the app so your TypeScript modules stay importable, and then runs
`bun install && bun run build` if `bun` is on PATH. If `bun` is missing, the
project is still fully generated — install bun and re-run `crepus moon
build`, or run the printed commands yourself.

## Add TypeScript modules or npm libraries

- Drop `.ts`/`.tsx` files in `ts/` — they're copied in and importable from
  the generated app.
- This is a real bun project after `crepus moon build`: run `bun add
  <package>` in the project directory like any other bun/Vite app.

## Add more templates

Add more `const` raw strings to `src/main.rs`, render them with
`render_template_to_ir`, and emit them with `emit_moonshine_app` (a page) or
`emit_moonshine_component(&ir, "Name")` (a reusable component) — write the
result under `generated/` and `crepus moon build` will pick it up.
"#
    }

    pub fn scaffold_rust_app(name: &str) {
        let t0 = Instant::now();
        let slug = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>();
        let base = std::path::PathBuf::from(&slug);
        if base.exists() {
            ui::error(&format!("directory already exists: {slug}"));
        }

        scaffold::ensure_dir(&base.join("src"))
            .unwrap_or_else(|e| ui::error(&format!("create src: {e}")));
        scaffold::ensure_dir(&base.join("ts"))
            .unwrap_or_else(|e| ui::error(&format!("create ts: {e}")));

        let cargo_toml = CARGO_TOML
            .replace("{{slug}}", &scaffold::cargo_package_name(&slug))
            .replace("{{native_version}}", SCAFFOLD_NATIVE_VERSION);
        scaffold::write_file(&base.join("Cargo.toml"), &cargo_toml)
            .unwrap_or_else(|e| ui::error(&format!("write Cargo.toml: {e}")));

        scaffold::write_template(&base.join("src/main.rs"), MAIN_RS, &[("{{name}}", name)])
            .unwrap_or_else(|e| ui::error(&format!("write src/main.rs: {e}")));

        scaffold::write_file(&base.join("ts/example.ts"), EXAMPLE_TS)
            .unwrap_or_else(|e| ui::error(&format!("write ts/example.ts: {e}")));

        let readme = readme_template().replace("{{name}}", name);
        scaffold::write_file(&base.join("README.md"), &readme)
            .unwrap_or_else(|e| ui::error(&format!("write README.md: {e}")));

        scaffold::scaffold_success(
            &slug,
            &base,
            &[
                &format!("cd {slug}"),
                "cargo run           # emit generated/ from the inlined crepus templates",
                "crepus moon build   # build the real Moonshine + Vite site",
            ],
        );
        ui::done_in(t0.elapsed());
    }
}

/// `crepus moon build` — builds a `crepus moon new` Rust-only project into a
/// real Moonshine + React + Vite app.
mod rust_build {
    use std::env;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Instant;

    use super::{
        file_dep_paths, package_json_template, resolve_moonshine_root, INDEX_HTML,
        PLACEHOLDER_COMPONENTS, PLACEHOLDER_CORE, PLACEHOLDER_CREPUS, TSCONFIG, VITE_CONFIG,
    };
    use crate::error::CrepusCliError;
    use crate::scaffold;
    use crate::ui;

    const MAIN_TSX_BOOTSTRAP: &str = r##"// Written once by `crepus moon build` and then left alone, so this is yours to
// edit: compose the generated app with your own components and npm libraries
// here. `src/App.tsx` and `src/components/` are regenerated from the templates
// in src/main.rs on every build.
//
// The generated App takes `scope` (the values its expressions read) and
// `handlers` (the functions its events call). State lives here in ordinary
// React; the template decides what to render from it.
import { useState } from "react";
import { createApp } from "@tschk/moonshine/react";
import { App } from "./App";

function Root() {
  const [count, setCount] = useState(0);

  return (
    <App
      scope={{ count, items: ["first", "second"] }}
      handlers={{ increment: () => setCount((n) => n + 1) }}
    />
  );
}

createApp({ root: Root }).mount("#app");
"##;

    pub fn build(dir: Option<PathBuf>) -> Result<(), CrepusCliError> {
        let t0 = Instant::now();
        let project = dir.unwrap_or_else(|| PathBuf::from("."));

        if !project.join("Cargo.toml").is_file() {
            return Err(CrepusCliError::context(format!(
                "no Cargo.toml in {} — run `crepus moon build` from a project scaffolded by \
                 `crepus moon new`, or pass --dir",
                project.display()
            )));
        }

        eprintln!(
            "{} cargo run (emit generated/ from inlined crepus templates)",
            ui::arrow()
        );
        let status = Command::new("cargo")
            .arg("run")
            .current_dir(&project)
            .status()
            .map_err(|e| {
                CrepusCliError::context(format!(
                    "failed to invoke `cargo run`: {e}. Is the Rust toolchain installed and on PATH?"
                ))
            })?;
        if !status.success() {
            return Err(CrepusCliError::command(
                status.code(),
                "`cargo run` failed — fix the errors above and re-run `crepus moon build`",
            ));
        }

        let generated_app = project.join("generated/app.tsx");
        if !generated_app.is_file() {
            return Err(CrepusCliError::context(
                "cargo run did not write generated/app.tsx — check src/main.rs calls \
                 emit_moonshine_app and writes to generated/app.tsx"
                    .to_string(),
            ));
        }

        let name = project
            .canonicalize()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "moon-app".to_string());
        let slug = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>();

        scaffold::ensure_dir(&project.join("src/components")).map_err(CrepusCliError::context)?;

        let app_tsx = std::fs::read_to_string(&generated_app).map_err(|e| e.to_string())?;
        scaffold::write_file(&project.join("src/App.tsx"), &app_tsx)
            .map_err(CrepusCliError::context)?;

        let generated_components = project.join("generated/components");
        if generated_components.is_dir() {
            copy_dir_flat(&generated_components, &project.join("src/components"))
                .map_err(CrepusCliError::context)?;
        }

        let ts_dir = project.join("ts");
        if ts_dir.is_dir() {
            let dest = project.join("src/ts");
            scaffold::ensure_dir(&dest).map_err(CrepusCliError::context)?;
            copy_dir_flat(&ts_dir, &dest).map_err(CrepusCliError::context)?;
        }

        // The entry is where generated components, hand-written TSX from `ts/`
        // and npm libraries get composed, so it is the one generated file worth
        // editing. Overwriting it every build would silently discard that work;
        // `src/App.tsx` still tracks the templates on every run.
        let entry = project.join("src/main.tsx");
        if entry.exists() {
            ui::step("kept existing src/main.tsx");
        } else {
            scaffold::write_file(&entry, MAIN_TSX_BOOTSTRAP).map_err(CrepusCliError::context)?;
        }

        let (core, crepus, components, local_hint) = match resolve_moonshine_root() {
            Some(root) => {
                let (c, cr, co) = file_dep_paths(&root, Some(project.as_path()));
                let hint = format!("local moonshine checkout at `{}`", root.display());
                (c, cr, co, hint)
            }
            None => (
                PLACEHOLDER_CORE.to_string(),
                PLACEHOLDER_CREPUS.to_string(),
                PLACEHOLDER_COMPONENTS.to_string(),
                "no local moonshine checkout found (MOONSHINE_PATH, ../moonshine, or \
                 ~/projects/moonshine) — using published npm packages (^0.4.49)"
                    .to_string(),
            ),
        };
        eprintln!("{} {local_hint}", ui::arrow());

        let pkg = package_json_template(&slug, &core, &crepus, &components);
        scaffold::write_file(&project.join("package.json"), &pkg)
            .map_err(CrepusCliError::context)?;
        scaffold::write_template(
            &project.join("index.html"),
            INDEX_HTML,
            &[("{{name}}", &name)],
        )
        .map_err(CrepusCliError::context)?;
        scaffold::write_file(&project.join("vite.config.ts"), VITE_CONFIG)
            .map_err(CrepusCliError::context)?;
        scaffold::write_file(&project.join("tsconfig.json"), TSCONFIG)
            .map_err(CrepusCliError::context)?;

        match which_bun() {
            Some(_) => {
                eprintln!("{} bun install", ui::arrow());
                run_bun(&project, &["install"])?;
                eprintln!("{} bun run build", ui::arrow());
                run_bun(&project, &["run", "build"])?;
                ui::success(&format!(
                    "built {} in {:?}",
                    project.display(),
                    t0.elapsed()
                ));
            }
            None => {
                eprintln!(
                    "{} bun not found on PATH — project generated but not built",
                    ui::warn()
                );
                eprintln!("  run next:");
                eprintln!("    cd {}", project.display());
                eprintln!("    bun install");
                eprintln!("    bun run build");
            }
        }

        Ok(())
    }

    fn which_bun() -> Option<PathBuf> {
        let path = env::var_os("PATH")?;
        env::split_paths(&path).find_map(|dir| {
            let candidate = dir.join("bun");
            candidate.is_file().then_some(candidate)
        })
    }

    fn run_bun(project: &Path, args: &[&str]) -> Result<(), CrepusCliError> {
        let status = Command::new("bun")
            .args(args)
            .current_dir(project)
            .status()
            .map_err(|e| {
                CrepusCliError::context(format!("failed to invoke `bun {args:?}`: {e}"))
            })?;
        if !status.success() {
            return Err(CrepusCliError::command(
                status.code(),
                format!("`bun {}` failed", args.join(" ")),
            ));
        }
        Ok(())
    }

    /// Copy a source tree into an existing destination, preserving logical aliases.
    /// Validate cycles, broken links and source/output overlap before copying files.
    fn copy_dir_flat(src: &Path, dest: &Path) -> Result<(), String> {
        let destination = dest.canonicalize().map_err(|e| e.to_string())?;
        let source = src.canonicalize().map_err(|e| e.to_string())?;
        if destination.starts_with(&source) {
            return Err("copy destination must not be inside the source tree".into());
        }
        let entries = walkdir::WalkDir::new(src)
            .follow_links(true)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for entry in &entries {
            let relative = entry.path().strip_prefix(src).map_err(|e| e.to_string())?;
            scaffold::validate_copy_destination(&destination, relative)
                .map_err(|e| e.to_string())?;
            let identity = entry.path().canonicalize().map_err(|e| e.to_string())?;
            if identity.starts_with(&destination) {
                return Err(format!(
                    "copy source enters the destination: {}",
                    entry.path().display()
                ));
            }
        }
        for entry in entries {
            let relative = entry.path().strip_prefix(src).map_err(|e| e.to_string())?;
            let output = dest.join(relative);
            if entry.file_type().is_dir() {
                std::fs::create_dir_all(output).map_err(|e| e.to_string())?;
            } else if entry.file_type().is_file() {
                std::fs::copy(entry.path(), output).map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    }

    #[cfg(test)]
    mod copy_tests {
        use super::*;
        #[test]
        #[cfg(unix)]
        fn contained_output_links_and_explicit_root_aliases_remain_supported() {
            use std::os::unix::fs::symlink;
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            let destination = root.path().join("destination");
            std::fs::create_dir_all(source.join("nested")).unwrap();
            std::fs::create_dir_all(destination.join("shared")).unwrap();
            std::fs::write(source.join("value.ts"), "new").unwrap();
            std::fs::write(source.join("nested/child.ts"), "child").unwrap();
            std::fs::write(destination.join("owned.ts"), "old").unwrap();
            symlink("owned.ts", destination.join("value.ts")).unwrap();
            symlink("shared", destination.join("nested")).unwrap();
            let alias = root.path().join("output-alias");
            symlink(&destination, &alias).unwrap();
            copy_dir_flat(&source, &alias).unwrap();
            assert_eq!(
                std::fs::read_to_string(destination.join("owned.ts")).unwrap(),
                "new"
            );
            assert_eq!(
                std::fs::read_to_string(destination.join("shared/child.ts")).unwrap(),
                "child"
            );
            assert!(std::fs::symlink_metadata(destination.join("value.ts"))
                .unwrap()
                .file_type()
                .is_symlink());
        }

        #[test]
        #[cfg(unix)]
        fn output_links_cannot_redirect_copies_outside_the_destination() {
            use std::os::unix::fs::symlink;
            for kind in ["file", "directory", "dangling"] {
                let root = tempfile::tempdir().unwrap();
                let source = root.path().join("source");
                let destination = root.path().join("destination");
                let outside = root.path().join("outside");
                for directory in [&source, &destination, &outside] {
                    std::fs::create_dir_all(directory).unwrap();
                }
                let target = outside.join("value.ts");
                if kind != "dangling" {
                    std::fs::write(&target, "keep").unwrap();
                }
                if kind == "directory" {
                    std::fs::create_dir(source.join("nested")).unwrap();
                    std::fs::write(source.join("nested/value.ts"), "new").unwrap();
                    symlink(&outside, destination.join("nested")).unwrap();
                } else {
                    std::fs::write(source.join("value.ts"), "new").unwrap();
                    symlink(&target, destination.join("value.ts")).unwrap();
                }
                let result = copy_dir_flat(&source, &destination);
                if kind == "dangling" {
                    assert!(!target.exists());
                } else {
                    assert_eq!(std::fs::read_to_string(target).unwrap(), "keep");
                }
                assert!(result.is_err(), "{kind}");
            }
        }

        #[test]
        #[cfg(unix)]
        fn ordinary_aliases_and_external_sources_copy_under_their_logical_paths() {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            let destination = root.path().join("destination");
            std::fs::create_dir_all(source.join("nested/deep")).unwrap();
            std::fs::create_dir_all(&destination).unwrap();
            std::fs::write(source.join("nested/deep/value.ts"), "content").unwrap();
            std::fs::write(destination.join("keep.ts"), "keep").unwrap();
            let external = tempfile::tempdir().unwrap();
            std::fs::write(external.path().join("external.ts"), "external").unwrap();
            std::os::unix::fs::symlink("nested", source.join("alias")).unwrap();
            std::os::unix::fs::symlink(external.path(), source.join("linked")).unwrap();
            copy_dir_flat(&source, &destination).unwrap();
            for path in ["nested/deep/value.ts", "alias/deep/value.ts"] {
                assert_eq!(
                    std::fs::read_to_string(destination.join(path)).unwrap(),
                    "content"
                );
            }
            assert_eq!(
                std::fs::read_to_string(destination.join("linked/external.ts")).unwrap(),
                "external"
            );
            assert_eq!(
                std::fs::read_to_string(destination.join("keep.ts")).unwrap(),
                "keep"
            );
        }

        #[test]
        #[cfg(unix)]
        fn output_overlap_and_broken_links_are_rejected_before_file_writes() {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            let destination = root.path().join("destination");
            std::fs::create_dir_all(&source).unwrap();
            std::fs::create_dir_all(&destination).unwrap();
            std::fs::write(source.join("value.ts"), "new").unwrap();
            std::fs::write(destination.join("value.ts"), "previous").unwrap();
            std::os::unix::fs::symlink(&destination, source.join("linked-output")).unwrap();
            assert!(copy_dir_flat(&source, &destination)
                .unwrap_err()
                .contains("enters the destination"));
            assert_eq!(
                std::fs::read_to_string(destination.join("value.ts")).unwrap(),
                "previous"
            );
            assert!(!destination.join("linked-output").exists());
            let nested = source.join("nested-output");
            std::fs::create_dir_all(&nested).unwrap();
            assert!(copy_dir_flat(&source, &nested)
                .unwrap_err()
                .contains("inside the source"));
            assert!(copy_dir_flat(&source, &source).is_err());
            let broken_source = root.path().join("broken-source");
            std::fs::create_dir_all(&broken_source).unwrap();
            std::fs::write(broken_source.join("value.ts"), "new").unwrap();
            std::os::unix::fs::symlink("missing", broken_source.join("broken")).unwrap();
            assert!(copy_dir_flat(&broken_source, &destination).is_err());
            assert_eq!(
                std::fs::read_to_string(destination.join("value.ts")).unwrap(),
                "previous"
            );
        }

        #[test]
        #[cfg(unix)]
        fn cyclic_copy_rejects_before_replacing_existing_files() {
            let root = tempfile::tempdir().unwrap();
            let source = root.path().join("source");
            let destination = root.path().join("destination");
            std::fs::create_dir_all(&source).unwrap();
            std::fs::create_dir_all(&destination).unwrap();
            std::fs::write(source.join("value.ts"), "new").unwrap();
            std::fs::write(destination.join("value.ts"), "previous").unwrap();
            std::os::unix::fs::symlink(".", source.join("again")).unwrap();
            let error = copy_dir_flat(&source, &destination).unwrap_err();
            assert!(error.to_lowercase().contains("loop"), "{error}");
            assert_eq!(
                std::fs::read_to_string(destination.join("value.ts")).unwrap(),
                "previous"
            );
            assert!(!destination.join("again").exists());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_mentions_moonshine_packages() {
        let snip = dependency_snippets();
        assert!(snip.contains("\"@tschk/moonshine\""));
        assert!(snip.contains("\"@tschk/crepus-moonshine\""));
        assert!(snip.contains("\"@tschk/moonshine-components\""));
        assert!(snip.contains("file:"));
        assert!(snip.contains("git clone https://github.com/tschk/moonshine"));
        assert!(!snip.contains("#path:packages/"));
    }

    #[test]
    fn package_json_includes_react() {
        let pkg = package_json_template(
            "test-app",
            PLACEHOLDER_CORE,
            PLACEHOLDER_CREPUS,
            PLACEHOLDER_COMPONENTS,
        );
        assert!(pkg.contains("\"react\""));
        assert!(pkg.contains("\"react-dom\""));
        assert!(pkg.contains("@vitejs/plugin-react"));
        for name in [
            "@tschk/moonshine",
            "@tschk/crepus-moonshine",
            "@tschk/moonshine-components",
        ] {
            assert!(pkg.contains(&format!("\"{name}\": \"^0.4.49\"")), "{pkg}");
        }
        // Without a local checkout the fallback must be installable anywhere, so
        // no `file:` path may reach a scaffolded package.json.
        assert!(!pkg.contains("file:"), "{pkg}");
    }

    #[test]
    fn dependency_values_remain_literal_json_strings() {
        for value in [
            r"file:C:\Users\name\moonshine",
            "file:/quoted\"/line\n雪/{{slug}}",
        ] {
            let package: serde_json::Value =
                serde_json::from_str(&package_json_template("test-app", value, value, value))
                    .unwrap();
            assert_eq!(package["name"], "test-app");
            for name in [
                "@tschk/moonshine",
                "@tschk/crepus-moonshine",
                "@tschk/moonshine-components",
            ] {
                assert_eq!(package["dependencies"][name], value);
            }
        }
    }

    #[test]
    fn looks_like_moonshine_rejects_empty() {
        assert!(!looks_like_moonshine_checkout(Path::new(
            "/tmp/nope-moonshine-xyz"
        )));
    }
}
