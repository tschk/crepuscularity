//! Framework emit for `crepus web build --emit <target>`.
//!
//! HTML is the real production path (`build_site_wasm`). `--emit moonshine` writes
//! a real `@tschk/crepus-moonshine` app entry under `dist/`.

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use crepuscularity_core::context::TemplateContext;
use crepuscularity_native::{render_template_to_ir, to_json_pretty, ViewIr};

use crate::cli::WebEmitTarget;
use crate::ui;

use super::build::{load_all_crepus_public, resolve_emit_paths};

pub(crate) fn build_emit_stub(
    emit: WebEmitTarget,
    site: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    entry: Option<String>,
) {
    let t0 = Instant::now();
    let paths = resolve_emit_paths(site, out_dir, entry);
    let mut files = std::collections::HashMap::new();
    load_all_crepus_public(&paths.site_dir, &paths.site_dir, &mut files);
    if files.is_empty() {
        ui::error(&format!(
            "no .crepus files under {}",
            paths.site_dir.display()
        ));
    }
    let source = files.get(&paths.entry).cloned().unwrap_or_else(|| {
        ui::error(&format!(
            "entry {:?} not found under {}",
            paths.entry,
            paths.site_dir.display()
        ))
    });

    let ctx = TemplateContext::new();
    let ir = render_template_to_ir(&source, &ctx).unwrap_or_else(|e| {
        ui::error(&format!("lower {} to View IR: {e}", paths.entry));
    });

    fs::create_dir_all(&paths.out_dir).unwrap_or_else(|e| {
        ui::error(&format!("mkdir {}: {e}", paths.out_dir.display()));
    });

    let ir_json = to_json_pretty(&ir).unwrap_or_else(|e| {
        ui::error(&format!("serialize View IR: {e}"));
    });
    let ir_path = paths.out_dir.join("crepus-view-ir.json");
    fs::write(&ir_path, &ir_json).unwrap_or_else(|e| {
        ui::error(&format!("write {}: {e}", ir_path.display()));
    });

    let (filename, body) = match emit {
        WebEmitTarget::Html => unreachable!("html uses WASM build path"),
        WebEmitTarget::Moonshine => ("crepus-emit.moonshine.tsx", emit_moonshine(&ir)),
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

    let out_file = paths.out_dir.join(filename);
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
        WebEmitTarget::Moonshine => {
            "Moonshine emit: App() → JSX with className; install via `crepus moonshine dep`"
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
