//! Framework emit for `crepus web build --emit <target>`.
//!
//! HTML is the real production path (`build_site_wasm`). `--emit moonshine` writes
//! a real `@tschk/crepus-moonshine` app entry under `dist/`.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::{render_from_files, to_json_pretty, ViewIr};

use crate::cli::WebEmitTarget;
use crate::ui;

use super::build::{load_framework_templates, resolve_emit_paths};

pub(crate) fn build_emit_stub(
    emit: WebEmitTarget,
    site: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    entry: Option<String>,
    target_id: Option<String>,
    manifest: Option<PathBuf>,
) {
    let t0 = Instant::now();
    let paths = resolve_emit_paths(site, out_dir, entry, target_id, manifest);
    let module_target = match emit {
        WebEmitTarget::MoonshineDom => Some(crepuscularity_native::MoonshineModuleTarget::Dom),
        WebEmitTarget::MoonshineSsr => Some(crepuscularity_native::MoonshineModuleTarget::Ssr),
        _ => None,
    };
    if let Some(target) = module_target {
        let modules =
            super::module_emit::prepare(&paths.site_dir, &paths.out_dir, &paths.entry, target)
                .unwrap_or_else(|error| {
                    ui::error(&format!("compile authored module {}: {error}", paths.entry))
                });
        if let Some(modules) = modules {
            for (relative, source) in modules.files {
                let destination = paths.out_dir.join(relative);
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).unwrap_or_else(|error| {
                        ui::error(&format!("mkdir {}: {error}", parent.display()))
                    });
                }
                fs::write(&destination, source).unwrap_or_else(|error| {
                    ui::error(&format!("write {}: {error}", destination.display()))
                });
            }
            eprintln!(
                "\n{} wrote authored module graph to {}",
                ui::ok(),
                paths.out_dir.display()
            );
            ui::done_in(t0.elapsed());
            return;
        }
    }
    let mut files = std::collections::HashMap::new();
    load_framework_templates(&paths.site_dir, &paths.site_dir, &mut files);
    if files.is_empty() {
        ui::error(&format!(
            "no supported template files under {}",
            paths.site_dir.display()
        ));
    }
    let sources = super::source_validate::validate_templates(
        &paths.site_dir,
        &files,
        &paths.entry,
        module_target.is_some(),
    )
    .unwrap_or_else(|error| ui::error(&format!("validate template source: {error}")));
    let ctx = TemplateContext::new();
    // shortcut: JSX still treats include-local bindings as global; preserve lexical scopes for reactive include props.
    let ir = render_from_files(&files, &paths.entry, &ctx).unwrap_or_else(|e| {
        ui::error(&format!("lower {} to View IR: {e}", paths.entry));
    });

    // Complete lowering, serialization, and target generation before touching
    // existing artifacts. Filesystem write failures are still reported normally.
    let ir_json = to_json_pretty(&ir).unwrap_or_else(|e| {
        ui::error(&format!("serialize View IR: {e}"));
    });
    let mut imba_bindings = None;

    let (filename, body) = match emit {
        WebEmitTarget::Html => unreachable!("html uses WASM build path"),
        WebEmitTarget::Imba => {
            let component =
                crepuscularity_native::emit_imba_component(&ir).unwrap_or_else(|error| {
                    ui::error(&format!("emit Imba component: {error}"));
                });
            imba_bindings = Some(component.bindings);
            ("CrepusEmit.imba", component.source)
        }
        WebEmitTarget::React => (
            "CrepusEmit.react.tsx",
            crepuscularity_native::emit_react_component(&ir, "CrepusEmit"),
        ),
        WebEmitTarget::Moonshine => ("crepus-emit.moonshine.tsx", emit_moonshine(&ir)),
        WebEmitTarget::MoonshineDom => (
            "CrepusEmit.moonshine-dom.ts",
            crepuscularity_native::emit_moonshine_dom_component(&ir, "CrepusEmit").unwrap_or_else(
                |error| ui::error(&format!("emit Moonshine DOM component: {error}")),
            ),
        ),
        WebEmitTarget::MoonshineSsr => (
            "CrepusEmit.moonshine-ssr.ts",
            crepuscularity_native::emit_moonshine_ssr_component(&ir, "CrepusEmit").unwrap_or_else(
                |error| ui::error(&format!("emit Moonshine SSR component: {error}")),
            ),
        ),
        WebEmitTarget::Svelte => (
            "CrepusEmit.svelte",
            crepuscularity_native::emit_svelte_component(&ir),
        ),
        WebEmitTarget::Vue => (
            "CrepusEmit.vue",
            crepuscularity_native::emit_vue_component(&ir),
        ),
        WebEmitTarget::Solid => (
            "CrepusEmit.solid.tsx",
            crepuscularity_native::emit_solid_component(&ir, "CrepusEmit"),
        ),
    };

    let ir_path = paths.out_dir.join("crepus-view-ir.json");
    let out_file = paths.out_dir.join(filename);
    let mut outputs = vec![ir_path.clone(), out_file.clone()];
    if imba_bindings.is_some() {
        outputs.push(paths.out_dir.join("CrepusEmit.bindings.ts"));
    }
    let mut destinations = HashMap::new();
    for output in &outputs {
        let resolved =
            super::module_emit::absolute_resolved(output).unwrap_or_else(|error| ui::error(&error));
        if resolved.is_dir() {
            ui::error(&format!(
                "generated output {} is an existing directory",
                output.display()
            ));
        }
        if resolved.exists() && !resolved.is_file() {
            ui::error(&format!(
                "generated output {} is not a regular file",
                output.display()
            ));
        }
        if sources.contains(&resolved) {
            ui::error(&format!(
                "refusing to overwrite reachable template source {} with generated output {}",
                resolved.display(),
                output.display()
            ));
        }
        if let Some(previous) = destinations.insert(resolved, output) {
            ui::error(&format!(
                "generated outputs {} and {} resolve to the same destination",
                previous.display(),
                output.display()
            ));
        }
    }

    fs::create_dir_all(&paths.out_dir).unwrap_or_else(|e| {
        ui::error(&format!("mkdir {}: {e}", paths.out_dir.display()));
    });
    fs::write(&ir_path, &ir_json).unwrap_or_else(|e| {
        ui::error(&format!("write {}: {e}", ir_path.display()));
    });
    if let Some(bindings) = imba_bindings {
        let bindings_path = paths.out_dir.join("CrepusEmit.bindings.ts");
        fs::write(&bindings_path, bindings).unwrap_or_else(|error| {
            ui::error(&format!("write {}: {error}", bindings_path.display()));
        });
    }

    fs::write(&out_file, body).unwrap_or_else(|e| {
        ui::error(&format!("write {}: {e}", out_file.display()));
    });

    eprintln!(
        "\n{} wrote {} (+ {})",
        ui::ok(),
        out_file.display(),
        ir_path.display()
    );
    let hint = match emit {
        WebEmitTarget::Html => "",
        WebEmitTarget::Imba => {
            "Imba emit: CrepusEmit tag plus CrepusEmit.bindings.ts; core nodes only"
        }
        WebEmitTarget::React => {
            "React emit: an importable TSX component taking { scope, handlers }"
        }
        WebEmitTarget::Moonshine => {
            "Moonshine emit: App() → JSX with className; install via `crepus moonshine dep`"
        }
        WebEmitTarget::MoonshineDom => {
            "Moonshine DOM emit: a TypeScript component for @tschk/moonshine-dom/dom"
        }
        WebEmitTarget::MoonshineSsr => {
            "Moonshine SSR emit: a TypeScript component for @tschk/moonshine-dom/ssr"
        }
        WebEmitTarget::Svelte => {
            "Svelte 5 emit: a component taking { scope, handlers } via $props()"
        }
        WebEmitTarget::Vue => "Vue 3 emit: an SFC taking { scope, handlers } via defineProps()",
        WebEmitTarget::Solid => {
            "Solid emit: a component taking { scope, handlers }, Show/For control flow"
        }
    };
    if !hint.is_empty() {
        eprintln!("  {} {hint}", ui::dim("→"));
    }
    ui::done_in(t0.elapsed());
}

/// Emit a Moonshine app entry as real JSX.
///
/// The emitter lives in `crepuscularity-native` so scaffolded projects can call
/// it directly; keeping a second copy here would let the two drift apart.
fn emit_moonshine(ir: &ViewIr) -> String {
    crepuscularity_native::emit_moonshine_app(ir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crepuscularity_native::render_template_to_ir;

    fn sample_ir() -> ViewIr {
        let source = r#"stack col gap-2
 text "hi"
 button "Go"
"#;
        render_template_to_ir(source, &TemplateContext::new()).expect("ir")
    }

    #[test]
    fn moonshine_emit_writes_real_jsx_with_classnames() {
        let body = emit_moonshine(&sample_ir());
        assert!(body.contains("@tschk/moonshine/react"));
        assert!(body.contains("createApp"));
        assert!(body.contains("export function App"));
        assert!(body.contains("export function mount"));
        assert!(body.contains("data-crepus-root=\"true\""));
        // Real elements, not an IR blob handed to a runtime renderer.
        assert!(body.contains("<div className=\"col gap-2\">"));
        assert!(body.contains("<span>hi</span>"));
        assert!(body.contains("<button type=\"button\">Go</button>"));
        assert!(!body.contains("renderCrepusIr"));
        assert!(!body.contains("satisfies ViewIr"));
    }

    #[test]
    fn moonshine_emit_preserves_anchor_href_and_classes() {
        let source =
            "a href=\"https://example.com\" no-underline text-zinc-100\n span \"crepuscularity\"\n";
        let ir = render_template_to_ir(source, &TemplateContext::new()).expect("ir");
        let body = emit_moonshine(&ir);
        assert!(body.contains("<a href=\"https://example.com\""));
        assert!(body.contains("className=\"no-underline text-zinc-100\""));
        assert!(body.contains(">crepuscularity</span>"));
    }
}
