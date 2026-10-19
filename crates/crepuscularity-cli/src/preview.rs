#[cfg(feature = "desktop")]
pub(crate) fn run_preview(path: std::path::PathBuf) {
    use console::style;
    use crepuscularity_runtime::{HotReloadState, HotReloadView};
    use gpui::{bounds, point, prelude::*, px, size, WindowOptions};

    let display_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("preview")
        .to_string();

    eprintln!(
        "{} previewing {}",
        style("crepus").dim(),
        style(path.display().to_string()).cyan().bold()
    );

    gpui_platform::application().run(move |cx: &mut gpui::App| {
        let opts = WindowOptions {
            app_id: Some(format!("crepuscularity.preview.{display_name}")),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some(format!("Crepus Preview - {display_name}").into()),
                ..Default::default()
            }),
            window_bounds: Some(gpui::WindowBounds::Windowed(bounds(
                point(px(100.), px(100.)),
                size(px(1200.), px(800.)),
            ))),
            ..Default::default()
        };

        let p = path.clone();
        match cx.open_window(opts, move |_window, cx| {
            let state = cx.new(|cx| {
                HotReloadState::new_with_context_loader(p.clone(), load_preview_context, cx)
            });
            cx.new(|_| HotReloadView::new(state))
        }) {
            Ok(_) => {}
            Err(e) => eprintln!("preview: failed to open window: {e}"),
        }
    });
}

#[cfg(any(feature = "desktop", test))]
fn load_preview_context(entry: &std::path::Path) -> crepuscularity_core::context::TemplateContext {
    let mut context = crepuscularity_core::context::TemplateContext::new();
    if let Some(dir) = entry.parent() {
        load_context_toml(&dir.join("context.toml"), &mut context);
    }
    context
}

#[cfg(any(feature = "desktop", test))]
fn load_context_toml(
    path: &std::path::Path,
    ctx: &mut crepuscularity_core::context::TemplateContext,
) {
    use crepuscularity_core::context::TemplateValue;
    let Ok(content) = std::fs::read_to_string(path) else {
        return;
    };

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim();
            let val = line[eq + 1..].trim();
            if val == "true" {
                ctx.set(key, TemplateValue::Bool(true));
            } else if val == "false" {
                ctx.set(key, TemplateValue::Bool(false));
            } else if let Ok(n) = val.parse::<i64>() {
                ctx.set(key, TemplateValue::Int(n));
            } else if let Ok(f) = val.parse::<f64>() {
                ctx.set(key, TemplateValue::Float(f));
            } else {
                ctx.set(key, val.trim_matches('"').trim_matches('\'').to_string());
            }
        }
    }
}

#[cfg(not(feature = "desktop"))]
pub(crate) struct CargoBuildOutcome {
    pub success: bool,
}

#[cfg(not(feature = "desktop"))]
pub(crate) fn cargo_build_fallback(
    cwd: &std::path::Path,
    options: crate::build_options::BuildOptions,
) -> CargoBuildOutcome {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build").current_dir(cwd);
    options.apply_profile_to_command(&mut cmd);
    let success = cmd.status().map(|status| status.success()).unwrap_or(false);
    CargoBuildOutcome { success }
}

#[cfg(test)]
mod tests {
    use super::load_preview_context;
    use crepuscularity_core::context::TemplateValue;

    #[test]
    fn preview_context_reloads_typed_values_and_removes_stale_keys() {
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("view.crepus");
        let context_path = dir.path().join("context.toml");
        std::fs::write(
            &context_path,
            "# preview data\nname = 'old'\ncount = 1\nratio = 1.5\nenabled = true\nremoved = 'gone'\n",
        )
        .unwrap();
        let initial = load_preview_context(&entry);
        assert_eq!(initial.get_str("name"), "old");
        assert!(matches!(
            initial.get("removed"),
            Some(TemplateValue::Str(_))
        ));

        std::fs::write(
            &context_path,
            "name = \"new\"\ncount = 2\nratio = 2.5\nenabled = false\n",
        )
        .unwrap();
        let updated = load_preview_context(&entry);
        assert_eq!(updated.get_str("name"), "new");
        assert!(matches!(updated.get("count"), Some(TemplateValue::Int(2))));
        assert!(matches!(updated.get("ratio"), Some(TemplateValue::Float(n)) if *n == 2.5));
        assert!(matches!(
            updated.get("enabled"),
            Some(TemplateValue::Bool(false))
        ));
        assert!(updated.get("removed").is_none());

        std::fs::remove_file(context_path).unwrap();
        assert!(load_preview_context(&entry).vars.is_empty());
    }
}
