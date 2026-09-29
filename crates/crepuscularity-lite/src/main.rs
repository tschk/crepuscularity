//! Demo binary: GPUI window + V8. Library API lives in `lib.rs`.

use std::sync::Arc;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use crepuscularity_lite::config::CrepusLiteConfig;
use crepuscularity_lite::host::{HostNode, HostSnapshot, HostStyle};
use crepuscularity_lite::integration::{apply_window_deferred, take_app_exit_request};
use crepuscularity_lite::{parse_hex_color, prepare_guest_source, Bridge, V8ThreadRuntime};
use gpui::AnyElement;
use gpui::ClickEvent;
use gpui::{
    bounds, div, point, prelude::*, px, rgb, size, AnyWindowHandle, App, Context, Entity,
    FontWeight, KeyDownEvent, Render, Window, WindowBounds, WindowOptions,
};
use serde_json::{json, Value};

fn bench_log_result_enabled() -> bool {
    std::env::var("CREPUS_BENCH_LOG_RESULT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn apply_host_style(mut element: gpui::Div, style: &HostStyle) -> gpui::Div {
    if let Some(direction) = style.direction.as_deref() {
        element = element.flex();
        element = match direction {
            "row" => element.flex_row(),
            _ => element.flex_col(),
        };
    }
    if matches!(style.flex_grow, Some(v) if v > 0.0) {
        element = element.flex_1();
    }
    if let Some(width) = style.width {
        element = element.w(px(width));
    }
    if let Some(height) = style.height {
        element = element.h(px(height));
    }
    if let Some(padding) = style.padding {
        element = element.p(px(padding));
    }
    if let Some(padding_x) = style.padding_x {
        element = element.px(px(padding_x));
    }
    if let Some(padding_y) = style.padding_y {
        element = element.py(px(padding_y));
    }
    if let Some(radius) = style.radius {
        element = element.rounded(px(radius));
    }
    if matches!(style.border_width, Some(v) if v > 0.0) {
        element = element.border_1();
    }
    if let Some(background) = style.background.as_deref().and_then(parse_hex_color) {
        element = element.bg(background);
    }
    if let Some(color) = style.color.as_deref().and_then(parse_hex_color) {
        element = element.text_color(color);
    }
    if let Some(color) = style.border_color.as_deref().and_then(parse_hex_color) {
        element = element.border_color(color);
    }
    if let Some(font_size) = style.font_size {
        element = element.text_size(px(font_size));
    }
    if let Some(font_weight) = style.font_weight.as_deref() {
        element = element.font_weight(match font_weight {
            "bold" => FontWeight::BOLD,
            "medium" => FontWeight::MEDIUM,
            _ => FontWeight::SEMIBOLD,
        });
    }
    element
}

fn add_gap(mut element: gpui::Div, gap: Option<f32>) -> gpui::Div {
    let rounded = gap.map(|value| value.round() as i32).unwrap_or_default();
    element = match rounded {
        4 => element.gap_1(),
        8 => element.gap_2(),
        12 => element.gap_3(),
        16 => element.gap_4(),
        20 => element.gap_5(),
        24 => element.gap_6(),
        32 => element.gap_8(),
        40 => element.gap_10(),
        48 => element.gap_12(),
        _ => element,
    };
    element
}

fn render_host_children(nodes: &[HostNode], entity: &Entity<LiteRoot>) -> Vec<AnyElement> {
    nodes
        .iter()
        .map(|node| render_host_node(node, entity.clone()))
        .collect()
}

fn host_element_id(key: &str) -> (&'static str, u64) {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    ("host", hasher.finish())
}

fn render_scroll_view(
    base: gpui::Div,
    node: &HostNode,
    entity: &Entity<LiteRoot>,
    node_id: &str,
) -> AnyElement {
    base.id(host_element_id(node_id))
        .overflow_scroll()
        .children(render_host_children(&node.children, entity))
        .into_any_element()
}

fn render_button(
    base: gpui::Div,
    node: &HostNode,
    entity: &Entity<LiteRoot>,
    node_id: &str,
) -> AnyElement {
    let mut button = base
        .id(host_element_id(node_id))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .px_4()
        .py_2();
    if node.style.background.is_none() {
        button = button.bg(rgb(0x2563eb));
    }
    if node.style.color.is_none() {
        button = button.text_color(rgb(0xf8fafc));
    }
    let label = node
        .title
        .as_ref()
        .or(node.text.as_ref())
        .cloned()
        .unwrap_or_else(|| "Button".to_string());
    if let Some(handler_id) = node.on_press.clone() {
        let click_entity = entity.clone();
        button = button.on_click(move |_: &ClickEvent, window: &mut Window, app: &mut App| {
            let payload = json!({ "source": "press" });
            app.update_entity(&click_entity, |root, cx| {
                root.dispatch_host_event(&handler_id, payload, window, cx);
            });
        });
    }
    button.child(label).into_any_element()
}

fn render_text_input(
    base: gpui::Div,
    node: &HostNode,
    entity: &Entity<LiteRoot>,
    node_id: &str,
) -> AnyElement {
    let mut field = base
        .id(host_element_id(node_id))
        .rounded_md()
        .border_1()
        .border_color(rgb(0x334155))
        .bg(rgb(0x020617))
        .px_3()
        .py_2();
    let content = node
        .value
        .as_ref()
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| node.placeholder.as_ref().map(|value| format!("<{value}>")))
        .unwrap_or_else(|| "<input>".to_string());
    if let Some(handler_id) = node.on_press.clone() {
        let click_entity = entity.clone();
        field = field.cursor_pointer().on_click(
            move |_: &ClickEvent, window: &mut Window, app: &mut App| {
                let payload = json!({ "source": "focus" });
                app.update_entity(&click_entity, |root, cx| {
                    root.dispatch_host_event(&handler_id, payload, window, cx);
                });
            },
        );
    }
    field.child(content).into_any_element()
}

fn render_container(
    base: gpui::Div,
    node: &HostNode,
    entity: &Entity<LiteRoot>,
    node_id: &str,
) -> AnyElement {
    let mut container = if node.style.direction.is_none() {
        base.id(host_element_id(node_id)).flex().flex_col()
    } else {
        base.id(host_element_id(node_id))
    };
    if let Some(handler_id) = node.on_press.clone() {
        let click_entity = entity.clone();
        container = container.cursor_pointer().on_click(
            move |_: &ClickEvent, window: &mut Window, app: &mut App| {
                let payload = json!({ "source": "press" });
                app.update_entity(&click_entity, |root, cx| {
                    root.dispatch_host_event(&handler_id, payload, window, cx);
                });
            },
        );
    }
    if let Some(handler_id) = node.on_key_down.clone() {
        let key_entity = entity.clone();
        container = container.on_key_down(
            move |event: &KeyDownEvent, window: &mut Window, app: &mut App| {
                let payload = json!({
                    "kind": "keyDown",
                    "keystroke": event.keystroke.to_string(),
                });
                app.update_entity(&key_entity, |root, cx| {
                    root.dispatch_host_event(&handler_id, payload, window, cx);
                });
            },
        );
    }
    if let Some(text) = &node.text {
        container = container.child(text.clone());
    }
    container
        .children(render_host_children(&node.children, entity))
        .into_any_element()
}

fn render_host_node(node: &HostNode, entity: Entity<LiteRoot>) -> AnyElement {
    let mut base = apply_host_style(div(), &node.style);
    base = add_gap(base, node.style.gap);
    let node_id = node
        .id
        .clone()
        .or_else(|| node.on_press.clone())
        .or_else(|| node.on_key_down.clone())
        .or_else(|| node.route.clone())
        .unwrap_or_else(|| format!("host-{}", node.node_type.to_lowercase()));

    match node.node_type.as_str() {
        "ScrollView" => render_scroll_view(base, node, &entity, &node_id),
        "Button" => render_button(base, node, &entity, &node_id),
        "Text" => base
            .child(node.text.clone().unwrap_or_default())
            .into_any_element(),
        "TextInput" => render_text_input(base, node, &entity, &node_id),
        _ => render_container(base, node, &entity, &node_id),
    }
}

fn render_host_surface(snapshot: &HostSnapshot, entity: Entity<LiteRoot>) -> AnyElement {
    if let Some(tree) = &snapshot.tree {
        render_host_node(tree, entity)
    } else {
        div()
            .flex()
            .w_full()
            .h_full()
            .bg(rgb(0x0b0f14))
            .into_any_element()
    }
}

struct LiteRoot {
    v8: V8ThreadRuntime,
    bridge: Arc<Bridge>,
    config: CrepusLiteConfig,
}

impl LiteRoot {
    fn new(_cx: &mut Context<Self>) -> Self {
        let config = CrepusLiteConfig::load_discovered();
        let bridge = config.build_bridge();
        Self {
            v8: V8ThreadRuntime::spawn(bridge.clone()).expect("V8 thread should start"),
            bridge,
            config,
        }
    }

    /// Run guest or built-in demo. Keeps the existing V8 isolate so repeated clicks preserve JS globals.
    fn run_guest(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let base = std::env::var("CREPUS_LITE_BASE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        let verbose = std::env::var("CREPUS_LITE_VERBOSE").ok().as_deref() == Some("1");
        if let Some(script) = self.config.guest_source(&base) {
            let guest_path = self.config.resolved_guest_path(&base).unwrap_or_else(|| {
                base.join(self.config.active_guest_entry().unwrap_or("guest.js"))
            });
            let script = match prepare_guest_source(&guest_path, &script) {
                Ok(script) => script,
                Err(e) => {
                    eprintln!("crepus-lite: guest compile error: {e}");
                    apply_window_deferred(&self.bridge, window);
                    cx.notify();
                    return;
                }
            };
            if verbose {
                eprintln!("crepus-lite: loading guest from {}", base.display());
                eprintln!(
                    "crepus-lite: active guest_entry={:?}",
                    self.config.active_guest_entry()
                );
                eprintln!("crepus-lite: guest source bytes={}", script.len());
            }
            let entrypoint = r#"
;(() => {
  const runner =
    (typeof CrepusGuest !== "undefined" && typeof CrepusGuest.run === "function" && CrepusGuest.run.bind(CrepusGuest)) ||
    (typeof globalThis.CrepusGuest !== "undefined" && typeof globalThis.CrepusGuest.run === "function" && globalThis.CrepusGuest.run.bind(globalThis.CrepusGuest)) ||
    (typeof run === "function" && run) ||
    (typeof globalThis.run === "function" && globalThis.run);
  if (!runner) {
    return null;
  }
  return runner();
})();
"#;
            self.run_script(format!("{script}\n{entrypoint}"), false, window, cx);
        } else {
            eprintln!(
                "crepus-lite: guest source unavailable (missing guest_entry or unreadable file)"
            );
            apply_window_deferred(&self.bridge, window);
            cx.notify();
        }
    }

    fn dispatch_host_event(
        &mut self,
        handler_id: &str,
        payload: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let handler_json = serde_json::to_string(handler_id).unwrap_or_else(|_| "\"\"".into());
        let payload_json = payload.to_string();
        let script = format!(
            "(() => {{
                if (typeof globalThis.__crepusHostDispatch !== 'function') {{
                    return JSON.stringify({{ error: 'host_dispatch_missing', handlerId: {handler_json} }}, null, 2);
                }}
                const result = globalThis.__crepusHostDispatch({handler_json}, {payload_json});
                return JSON.stringify(result ?? null, null, 2);
            }})()"
        );
        self.run_script(script, false, window, cx);
    }

    fn run_script(
        &mut self,
        script: String,
        reset_v8: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let bridge = self.bridge.clone();
        let any_wh: AnyWindowHandle = window.window_handle();
        let reply_rx = match self.v8.handle.eval_rpc(script, reset_v8, bridge.clone()) {
            Ok(rx) => rx,
            Err(e) => {
                eprintln!("crepus-lite: V8 eval request failed: {e}");
                cx.notify();
                return;
            }
        };

        cx.spawn(async move |weak_root, async_cx| {
            let result = reply_rx.recv();
            async_cx.update(|app| {
                let _ = app.update_window(any_wh, |_root_view, window, app| {
                    if let Some(entity) = weak_root.upgrade() {
                        entity.update(app, |root, cx| {
                            match result {
                                Ok(Ok(s)) => {
                                    if std::env::var("CREPUS_LITE_VERBOSE").ok().as_deref()
                                        == Some("1")
                                    {
                                        eprintln!("crepus-lite: guest returned {} bytes", s.len());
                                        eprintln!(
                                            "crepus-lite: guest result preview={}",
                                            s.lines().next().unwrap_or("<empty>")
                                        );
                                    }
                                    if bench_log_result_enabled() {
                                        let line: String =
                                            s.chars().filter(|&c| c != '\n' && c != '\r').collect();
                                        eprintln!("CREPUS_BENCH_RESULT_JSON\t{line}");
                                    }
                                }
                                Ok(Err(e)) => {
                                    eprintln!("crepus-lite: script error: {e}");
                                }
                                Err(_) => {
                                    eprintln!(
                                        "crepus-lite: script error: crepus-v8 thread disconnected"
                                    );
                                }
                            }
                            apply_window_deferred(&root.bridge, window);
                            if take_app_exit_request(&root.bridge) {
                                cx.quit();
                            } else {
                                cx.notify();
                            }
                        });
                    }
                });
            });
        })
        .detach();
    }
}

impl Render for LiteRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let host_snapshot = self.bridge.host_snapshot();
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(0x18181b))
            .text_color(rgb(0xf4f4f5))
            .child(render_host_surface(&host_snapshot, entity))
            .into_any_element()
    }
}

const EXAMPLES: &[(&str, &str)] = &[
    ("quick-start", "electron clone"),
    ("motrix", "download manager"),
    ("joplin", "note-taking"),
    ("cerebro", "brain plugins"),
];

fn example_name_from_config() -> Option<(&'static str, &'static str)> {
    let config_path = std::env::var("CREPUS_LITE_CONFIG").ok()?;
    let config_dir = std::path::Path::new(&config_path).parent()?;
    let example_name = config_dir.file_name()?.to_str()?;
    EXAMPLES
        .iter()
        .find(|(n, _)| *n == example_name)
        .map(|&(n, d)| (n, d))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let base = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let (example_name, example_desc) = if let Some((n, d)) = example_name_from_config() {
        (n.to_string(), d.to_string())
    } else {
        let mut example_idx = 0;
        for (i, (name, _)) in EXAMPLES.iter().enumerate() {
            if args.iter().any(|a| a == name) {
                example_idx = i;
            }
        }
        if let Ok(n) = args.get(1).unwrap_or(&"".into()).parse::<usize>() {
            if n >= 1 && n <= EXAMPLES.len() {
                example_idx = n - 1;
            }
        }
        (
            EXAMPLES[example_idx].0.to_string(),
            EXAMPLES[example_idx].1.to_string(),
        )
    };

    let example_dir = base.join("examples").join(&example_name);
    let config_path = example_dir.join("crepus-lite.example.toml");

    println!("\n\x1b[36m▌▌▌ crepus-lite\x1b[0m\n");
    println!("  \x1b[90m{}\x1b[0m — {}\n", example_name, example_desc);

    if example_dir.join("build.mjs").exists() {
        println!("\x1b[33m▸\x1b[0m building...\x1b[0m");
        let _ = std::process::Command::new("bun")
            .current_dir(&example_dir)
            .args(["install"])
            .status();

        let status = std::process::Command::new("bun")
            .current_dir(&example_dir)
            .args(["run", "build"])
            .status();

        if !status.map(|s| s.success()).unwrap_or(false) {
            eprintln!("\x1b[31mbuild failed\x1b[0m");
            std::process::exit(1);
        }
    }

    println!("\x1b[32m▶\x1b[0m launching GUI\n");
    if std::env::var("CREPUS_LITE_CONFIG").is_err() {
        std::env::set_var("CREPUS_LITE_CONFIG", config_path.to_str().unwrap_or("."));
    }
    if std::env::var("CREPUS_LITE_BASE").is_err() {
        std::env::set_var("CREPUS_LITE_BASE", example_dir.to_str().unwrap_or("."));
    }

    gpui_platform::application().run(move |cx: &mut App| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds(
                point(px(100.), px(100.)),
                size(px(1200.), px(800.)),
            ))),
            titlebar: Some(gpui::TitlebarOptions {
                title: Some(format!("crepus-lite — {example_name}").into()),
                ..Default::default()
            }),
            focus: true,
            show: true,
            ..Default::default()
        };

        let handle = match cx.open_window(opts, |_w, cx| cx.new(LiteRoot::new)) {
            Ok(handle) => handle,
            Err(e) => {
                eprintln!("\x1b[31mfailed to open window:\x1b[0m {e}");
                return;
            }
        };

        let wh = handle;
        cx.spawn(async move |async_cx| {
            async_cx.update(|app| {
                let _ = wh.update(app, |root: &mut LiteRoot, window, ctxt| {
                    root.run_guest(window, ctxt);
                });
            });
        })
        .detach();
    });
}
