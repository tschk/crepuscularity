//! Demo binary: GPUI window + V8. Library API lives in `lib.rs`.

use std::sync::Arc;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use crepuscularity_lite::config::CrepusLiteConfig;
use crepuscularity_lite::host::{HostNode, HostStyle};
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

fn host_overflow(value: &str) -> Option<gpui::Overflow> {
    match value {
        "visible" => Some(gpui::Overflow::Visible),
        "hidden" => Some(gpui::Overflow::Hidden),
        "clip" => Some(gpui::Overflow::Clip),
        "scroll" | "auto" => Some(gpui::Overflow::Scroll),
        _ => None,
    }
}

fn apply_host_style(mut element: gpui::Div, style: &HostStyle) -> gpui::Div {
    let overflow = style.overflow.as_deref().and_then(host_overflow);
    if let Some(value) = style
        .overflow_x
        .as_deref()
        .and_then(host_overflow)
        .or(overflow)
    {
        element.style().overflow.x = Some(value);
    }
    if let Some(value) = style
        .overflow_y
        .as_deref()
        .and_then(host_overflow)
        .or(overflow)
    {
        element.style().overflow.y = Some(value);
    }
    if let Some(opacity) = style.opacity.filter(|opacity| opacity.is_finite()) {
        element = element.opacity(opacity.clamp(0.0, 1.0));
    }
    if let Some(direction) = style.direction.as_deref() {
        element = element.flex();
        element = match direction {
            "row" => element.flex_row(),
            _ => element.flex_col(),
        };
    }
    element = match style.align_items.as_deref() {
        Some("start") => element.items_start(),
        Some("center") => element.items_center(),
        Some("end") => element.items_end(),
        Some("stretch") => element.items_stretch(),
        _ => element,
    };
    element = match style.justify_content.as_deref() {
        Some("start") => element.justify_start(),
        Some("center") => element.justify_center(),
        Some("end") => element.justify_end(),
        Some("between") => element.justify_between(),
        Some("around") => element.justify_around(),
        Some("evenly") => element.justify_evenly(),
        _ => element,
    };
    if let Some(cols) = style.grid_cols.filter(|count| *count > 0) {
        element = element.grid_cols(cols);
    }
    if let Some(rows) = style.grid_rows.filter(|count| *count > 0) {
        element = element.grid_rows(rows);
    }
    if let Some(grow) = style
        .flex_grow
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        if grow > 0.0 {
            element = element.flex_1();
        }
        element = element.flex_grow(grow);
    }
    if let Some(width) = style.width {
        if width == crepuscularity_core::tailwind::SIZE_FIT {
            element = element.w_auto();
        } else if width.is_finite() && width >= 0.0 {
            element = element.w(px(width));
        }
    } else if let Some(fraction) = style
        .width_fraction
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        element = element.w(gpui::relative(fraction));
    }
    if let Some(height) = style.height {
        if height == crepuscularity_core::tailwind::SIZE_FIT {
            element = element.h_auto();
        } else if height.is_finite() && height >= 0.0 {
            element = element.h(px(height));
        }
    } else if let Some(fraction) = style
        .height_fraction
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        element = element.h(gpui::relative(fraction));
    }
    if let Some(width) = style
        .min_width
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        element = element.min_w(px(width));
    }
    if let Some(height) = style
        .min_height
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        element = element.min_h(px(height));
    }
    if let Some(width) = style
        .max_width
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        element = element.max_w(px(width));
    }
    if let Some(height) = style
        .max_height
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        element = element.max_h(px(height));
    }
    if let Some(padding) = style
        .padding
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        element = element.p(px(padding));
    }
    if let Some(padding_x) = style
        .padding_x
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        element = element.px(px(padding_x));
    }
    if let Some(padding_y) = style
        .padding_y
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        element = element.py(px(padding_y));
    }
    if let Some(radius) = style
        .radius
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        element = element.rounded(px(radius));
    }
    if let Some(width) = style
        .border_width
        .filter(|width| width.is_finite() && *width >= 0.0)
    {
        element = element.border(px(width));
    }
    element = match style.shadow.as_deref() {
        Some("none") => element.shadow_none(),
        Some("2xs") => element.shadow_2xs(),
        Some("xs") => element.shadow_xs(),
        Some("sm") => element.shadow_sm(),
        Some("md") => element.shadow_md(),
        Some("lg") => element.shadow_lg(),
        Some("xl") => element.shadow_xl(),
        Some("2xl") => element.shadow_2xl(),
        _ => element,
    };
    if let Some(background) = style.background.as_deref().and_then(parse_hex_color) {
        element = element.bg(background);
    }
    if let Some(color) = style.color.as_deref().and_then(parse_hex_color) {
        element = element.text_color(color);
    }
    if let Some(color) = style.border_color.as_deref().and_then(parse_hex_color) {
        element = element.border_color(color);
    }
    if let Some(italic) = style.italic {
        element = if italic {
            element.italic()
        } else {
            element.not_italic()
        };
    }
    // A missing decoration inherits; zero thickness explicitly suppresses the parent style.
    if let Some(underline) = style.underline {
        if underline {
            element = element.underline();
        } else {
            element.text_style().underline = Some(gpui::UnderlineStyle::default());
        }
    }
    if let Some(strikethrough) = style.strikethrough {
        if strikethrough {
            element = element.line_through();
        } else {
            element.text_style().strikethrough = Some(gpui::StrikethroughStyle::default());
        }
    }
    let align = match style.text_align.as_deref() {
        Some("left" | "leading") => Some(gpui::TextAlign::Left),
        Some("center") => Some(gpui::TextAlign::Center),
        Some("right" | "trailing") => Some(gpui::TextAlign::Right),
        _ => None,
    };
    if let Some(align) = align {
        element = element.text_align(align);
    }
    if let Some(family) = style
        .font_family
        .as_deref()
        .map(str::trim)
        .filter(|family| !family.is_empty())
    {
        element = element.font_family(family.to_owned());
    }
    if let Some(font_size) = style
        .font_size
        .filter(|value| value.is_finite() && *value > 0.0)
    {
        element = element.text_size(px(font_size));
    }
    if let Some(font_weight) = style.font_weight.as_deref() {
        let weight = match font_weight {
            "thin" => Some(FontWeight::THIN),
            "extralight" => Some(FontWeight::EXTRA_LIGHT),
            "light" => Some(FontWeight::LIGHT),
            "normal" => Some(FontWeight::NORMAL),
            "medium" => Some(FontWeight::MEDIUM),
            "semibold" => Some(FontWeight::SEMIBOLD),
            "bold" => Some(FontWeight::BOLD),
            "extrabold" => Some(FontWeight::EXTRA_BOLD),
            "black" => Some(FontWeight::BLACK),
            _ => None,
        };
        if let Some(weight) = weight {
            element = element.font_weight(weight);
        }
    }
    match style.display.as_deref() {
        Some("none") => element.hidden(),
        Some("flex") => element.flex(),
        Some("block") => element.block(),
        Some("grid") => element.grid(),
        _ => element,
    }
}

fn add_gap(mut element: gpui::Div, gap: Option<f32>) -> gpui::Div {
    if let Some(gap) = gap.filter(|gap| gap.is_finite() && *gap >= 0.0) {
        element = element.gap(px(gap));
    }
    element
}

fn render_host_children(
    nodes: &[HostNode],
    entity: &Entity<LiteRoot>,
    parent_id: &str,
) -> Vec<AnyElement> {
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            render_host_node(node, entity.clone(), &format!("{parent_id}/{index}"))
        })
        .collect()
}

fn host_element_id(key: &str) -> (&'static str, u64) {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    ("host", hasher.finish())
}

fn attach_host_key_handler(
    mut element: gpui::Stateful<gpui::Div>,
    node: &HostNode,
    entity: &Entity<LiteRoot>,
    node_id: &str,
) -> gpui::Stateful<gpui::Div> {
    if let Some(handler_id) = node.on_key_down.clone() {
        let key_entity = entity.clone();
        let event_node_id = node_id.to_owned();
        element = element.focusable().on_key_down(
            move |event: &KeyDownEvent, window: &mut Window, app: &mut App| {
                let payload = json!({
                    "kind": "keyDown",
                    "nodeId": event_node_id,
                    "keystroke": event.keystroke.to_string(),
                });
                app.update_entity(&key_entity, |root, cx| {
                    root.dispatch_host_event(&handler_id, payload, window, cx);
                });
            },
        );
    }
    element
}

fn scroll_view_base(base: gpui::Div, style: &HostStyle) -> gpui::Div {
    let mut base = container_base(base, style);
    base.style()
        .overflow
        .x
        .get_or_insert(gpui::Overflow::Scroll);
    base.style()
        .overflow
        .y
        .get_or_insert(gpui::Overflow::Scroll);
    base
}

fn render_scroll_view(
    base: gpui::Div,
    node: &HostNode,
    entity: &Entity<LiteRoot>,
    node_id: &str,
) -> AnyElement {
    let base = scroll_view_base(base, &node.style);
    attach_host_key_handler(base.id(host_element_id(node_id)), node, entity, node_id)
        .children(render_host_children(&node.children, entity, node_id))
        .into_any_element()
}

fn button_base(base: gpui::Div, style: &HostStyle) -> gpui::Div {
    let mut base = base
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .px_4()
        .py_2();
    if style.background.is_none() {
        base = base.bg(rgb(0x2563eb));
    }
    if style.color.is_none() {
        base = base.text_color(rgb(0xf8fafc));
    }
    apply_host_style(base, style)
}

fn host_dispatch_script(handler_id: &str, payload: &Value) -> String {
    let handler_json = serde_json::to_string(handler_id).unwrap_or_else(|_| "\"\"".into());
    let payload_json = payload.to_string();
    format!(
            "(() => {{
                if (typeof globalThis.__crepusHostDispatch !== 'function') {{
                    return JSON.stringify({{ error: 'host_dispatch_missing', handlerId: {handler_json} }}, null, 2);
                }}
                const result = globalThis.__crepusHostDispatch({handler_json}, {payload_json});
                return JSON.stringify(result ?? null, null, 2);
            }})()"
        )
}

fn host_event_payload(source: &str, node_id: &str) -> Value {
    json!({"source": source, "nodeId": node_id})
}

fn render_button(
    base: gpui::Div,
    node: &HostNode,
    entity: &Entity<LiteRoot>,
    node_id: &str,
) -> AnyElement {
    let mut button = button_base(base, &node.style).id(host_element_id(node_id));
    let label = node
        .title
        .as_ref()
        .or(node.text.as_ref())
        .cloned()
        .unwrap_or_else(|| "Button".to_string());
    if let Some(handler_id) = node.on_press.clone() {
        let click_entity = entity.clone();
        let event_node_id = node_id.to_owned();
        button = button.on_click(move |_: &ClickEvent, window: &mut Window, app: &mut App| {
            let payload = host_event_payload("press", &event_node_id);
            app.update_entity(&click_entity, |root, cx| {
                root.dispatch_host_event(&handler_id, payload, window, cx);
            });
        });
    }
    attach_host_key_handler(button, node, entity, node_id)
        .child(label)
        .into_any_element()
}

fn text_input_base(base: gpui::Div, style: &HostStyle) -> gpui::Div {
    apply_host_style(
        base.rounded_md()
            .border_1()
            .border_color(rgb(0x334155))
            .bg(rgb(0x020617))
            .px_3()
            .py_2(),
        style,
    )
}

fn render_text_input(
    base: gpui::Div,
    node: &HostNode,
    entity: &Entity<LiteRoot>,
    node_id: &str,
) -> AnyElement {
    let mut field = text_input_base(base, &node.style).id(host_element_id(node_id));
    let content = node
        .value
        .as_ref()
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| node.placeholder.as_ref().map(|value| format!("<{value}>")))
        .unwrap_or_else(|| "<input>".to_string());
    if let Some(handler_id) = node.on_press.clone() {
        let click_entity = entity.clone();
        let event_node_id = node_id.to_owned();
        field = field.cursor_pointer().on_click(
            move |_: &ClickEvent, window: &mut Window, app: &mut App| {
                let payload = host_event_payload("focus", &event_node_id);
                app.update_entity(&click_entity, |root, cx| {
                    root.dispatch_host_event(&handler_id, payload, window, cx);
                });
            },
        );
    }
    attach_host_key_handler(field, node, entity, node_id)
        .child(content)
        .into_any_element()
}

fn container_base(base: gpui::Div, style: &HostStyle) -> gpui::Div {
    if style.direction.is_none()
        && !matches!(style.display.as_deref(), Some("block" | "none" | "grid"))
    {
        base.flex().flex_col()
    } else {
        base
    }
}

fn render_container(
    base: gpui::Div,
    node: &HostNode,
    entity: &Entity<LiteRoot>,
    node_id: &str,
) -> AnyElement {
    let mut container = container_base(base, &node.style).id(host_element_id(node_id));
    if let Some(handler_id) = node.on_press.clone() {
        let click_entity = entity.clone();
        let event_node_id = node_id.to_owned();
        container = container.cursor_pointer().on_click(
            move |_: &ClickEvent, window: &mut Window, app: &mut App| {
                let payload = host_event_payload("press", &event_node_id);
                app.update_entity(&click_entity, |root, cx| {
                    root.dispatch_host_event(&handler_id, payload, window, cx);
                });
            },
        );
    }
    container = attach_host_key_handler(container, node, entity, node_id);
    if let Some(text) = &node.text {
        container = container.child(text.clone());
    }
    container
        .children(render_host_children(&node.children, entity, node_id))
        .into_any_element()
}

// shortcut: positional IDs change on sibling reorder; use explicit IDs for reorderable children.
fn host_node_id(node: &HostNode, path: &str) -> String {
    node.id.clone().unwrap_or_else(|| format!("host-{path}"))
}

fn render_host_node(node: &HostNode, entity: Entity<LiteRoot>, path: &str) -> AnyElement {
    if node.style.display.as_deref() == Some("none") {
        return div().hidden().into_any_element();
    }
    let mut base = apply_host_style(div(), &node.style);
    base = add_gap(base, node.style.gap);
    let node_id = host_node_id(node, path);

    match node.node_type.as_str() {
        "ScrollView" => render_scroll_view(base, node, &entity, &node_id),
        "Button" => render_button(base, node, &entity, &node_id),
        "Text" => {
            attach_host_key_handler(base.id(host_element_id(&node_id)), node, &entity, &node_id)
                .child(node.text.clone().unwrap_or_default())
                .into_any_element()
        }
        "TextInput" => render_text_input(base, node, &entity, &node_id),
        _ => render_container(base, node, &entity, &node_id),
    }
}

fn render_host_surface(tree: Option<&HostNode>, entity: Entity<LiteRoot>) -> AnyElement {
    if let Some(tree) = tree {
        render_host_node(tree, entity, "root")
    } else {
        div()
            .flex()
            .w_full()
            .h_full()
            .bg(rgb(0x0b0f14))
            .into_any_element()
    }
}

fn prepare_guest_run(
    config: &CrepusLiteConfig,
    base: &std::path::Path,
    verbose: bool,
) -> Result<String, String> {
    let source = config.guest_source(base).ok_or_else(|| {
        "guest source unavailable (missing guest_entry or unreadable file)".to_string()
    })?;
    let guest_path = config
        .resolved_guest_path(base)
        .unwrap_or_else(|| base.join(config.active_guest_entry().unwrap_or("guest.js")));
    let script = prepare_guest_source(&guest_path, &source)
        .map_err(|error| format!("guest compile error: {error}"))?;
    if verbose {
        eprintln!("crepus-lite: loading guest from {}", base.display());
        eprintln!(
            "crepus-lite: active guest_entry={:?}",
            config.active_guest_entry()
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
    Ok(format!("{script}\n{entrypoint}"))
}

fn spawn_root_loader(
    load: impl FnOnce() -> Result<LiteRoot, String> + Send + 'static,
) -> Result<futures_channel::oneshot::Receiver<Result<LiteRoot, String>>, String> {
    let (sender, receiver) = futures_channel::oneshot::channel();
    std::thread::Builder::new()
        .name("crepus-lite-init".into())
        .spawn(move || {
            let _ = sender.send(load());
        })
        .map_err(|error| format!("spawn crepus-lite startup worker: {error}"))?;
    Ok(receiver)
}

struct LiteRoot {
    v8: V8ThreadRuntime,
    bridge: Arc<Bridge>,
    config: CrepusLiteConfig,
}

impl Drop for LiteRoot {
    fn drop(&mut self) {
        if let Some(worker) = self.v8.abort() {
            if let Err(error) = std::thread::Builder::new()
                .name("crepus-v8-join".into())
                .spawn(move || {
                    if worker.join().is_err() {
                        eprintln!("crepus-lite: V8 worker panicked during teardown");
                    }
                })
            {
                eprintln!("crepus-lite: unable to observe V8 teardown: {error}");
            }
        }
    }
}

impl LiteRoot {
    fn new(config: CrepusLiteConfig) -> Result<Self, String> {
        let bridge = config.build_bridge();
        Ok(Self {
            v8: V8ThreadRuntime::spawn(bridge.clone())?,
            bridge,
            config,
        })
    }

    /// Load and compile guest source on the same worker FIFO as guest events.
    fn run_guest(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let config = CrepusLiteConfig {
            guest_entry: self.config.active_guest_entry().map(str::to_owned),
            dev_guest_entry: None,
            ..self.config.clone()
        };
        let base = std::env::var("CREPUS_LITE_BASE")
            .ok()
            .map(std::path::PathBuf::from);
        let verbose = std::env::var("CREPUS_LITE_VERBOSE").ok().as_deref() == Some("1");
        let reply = self.v8.handle.eval_prepared_rpc_async(
            move || {
                let base = base.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                prepare_guest_run(&config, &base, verbose)
            },
            false,
            self.bridge.clone(),
        );
        self.await_script_reply(reply, window, cx);
    }

    fn dispatch_host_event(
        &mut self,
        handler_id: &str,
        payload: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let script = host_dispatch_script(handler_id, &payload);
        self.run_script(script, false, window, cx);
    }

    fn run_script(
        &mut self,
        script: String,
        reset_v8: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reply = self
            .v8
            .handle
            .eval_rpc_async(script, reset_v8, self.bridge.clone());
        self.await_script_reply(reply, window, cx);
    }

    fn await_script_reply(
        &mut self,
        reply: Result<futures_channel::oneshot::Receiver<Result<String, String>>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let any_wh: AnyWindowHandle = window.window_handle();
        let reply_rx = match reply {
            Ok(rx) => rx,
            Err(e) => {
                eprintln!("crepus-lite: V8 eval request failed: {e}");
                cx.notify();
                return;
            }
        };

        cx.spawn(async move |weak_root, async_cx| {
            let result = reply_rx.await;
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
        let tree = self.bridge.host_render_tree();
        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(rgb(0x18181b))
            .text_color(rgb(0xf4f4f5))
            .child(render_host_surface(
                tree.as_ref().map(|tree| tree.root()),
                entity,
            ))
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
        let loaded = match spawn_root_loader(|| LiteRoot::new(CrepusLiteConfig::load_discovered()))
        {
            Ok(loaded) => loaded,
            Err(error) => {
                eprintln!("crepus-lite: startup failed: {error}");
                cx.quit();
                return;
            }
        };
        cx.spawn(async move |async_cx| {
            let root = match loaded.await {
                Ok(Ok(root)) => root,
                Ok(Err(error)) => {
                    eprintln!("crepus-lite: startup failed: {error}");
                    async_cx.update(|app| app.quit());
                    return;
                }
                Err(_) => {
                    eprintln!("crepus-lite: startup worker disconnected");
                    async_cx.update(|app| app.quit());
                    return;
                }
            };
            async_cx.update(move |app| {
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

                let handle = match app.open_window(opts, move |_w, cx| cx.new(move |_| root)) {
                    Ok(handle) => handle,
                    Err(error) => {
                        eprintln!("\x1b[31mfailed to open window:\x1b[0m {error}");
                        app.quit();
                        return;
                    }
                };
                let _ = handle.update(app, |root: &mut LiteRoot, window, cx| {
                    root.run_guest(window, cx);
                });
            });
        })
        .detach();
    });
}

#[cfg(test)]
mod style_tests {
    use super::*;

    #[cfg(unix)]
    #[gpui::test]
    fn startup_config_read_yields_ui_and_preserves_capabilities(cx: &mut gpui::TestAppContext) {
        use std::io::Write;
        use std::sync::mpsc;
        use std::time::Duration;

        cx.background_executor.allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("crepus.toml");
        assert!(std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        let (progress_tx, progress_rx) = mpsc::channel();
        let writer_path = path.clone();
        let writer = std::thread::spawn(move || {
            let mut pipe = std::fs::OpenOptions::new()
                .write(true)
                .open(writer_path)
                .unwrap();
            let yielded = progress_rx.recv_timeout(Duration::from_secs(10)).is_ok();
            writeln!(
                pipe,
                "guest_entry = \"ui-yielded-{yielded}.js\"\n[capabilities]\nhost = true"
            )
            .unwrap();
        });
        let loaded =
            spawn_root_loader(move || LiteRoot::new(CrepusLiteConfig::load_from_path(&path)?))
                .unwrap();
        cx.run_until_parked();
        let _ = progress_tx.send(());
        let root = cx
            .foreground_executor
            .block_with_timeout(Duration::from_secs(10), loaded)
            .unwrap_or_else(|_| panic!("startup timed out"))
            .unwrap()
            .unwrap();
        assert_eq!(
            root.config.guest_entry.as_deref(),
            Some("ui-yielded-true.js")
        );
        assert_eq!(
            root.bridge
                .invoke_envelope("host", "storageGet", &json!({"key":"missing"}))["ok"],
            true
        );
        writer.join().unwrap();
        let (_root, cx) = cx.add_window_view(move |_, _| root);
        cx.run_until_parked();
    }

    #[gpui::test]
    fn startup_loader_reports_error_and_disconnection(cx: &mut gpui::TestAppContext) {
        use std::time::Duration;
        cx.background_executor.allow_parking();
        let loaded = spawn_root_loader(|| Err("startup marker".into())).unwrap();
        let result = cx
            .foreground_executor
            .block_with_timeout(Duration::from_secs(10), loaded)
            .unwrap_or_else(|_| panic!("startup timed out"))
            .unwrap();
        assert!(matches!(result, Err(error) if error == "startup marker"));
        let loaded = spawn_root_loader(|| panic!("startup worker marker")).unwrap();
        let result = cx
            .foreground_executor
            .block_with_timeout(Duration::from_secs(10), loaded)
            .unwrap_or_else(|_| panic!("startup timed out"));
        assert!(result.is_err());
    }

    #[test]
    fn canceled_startup_drops_the_root_and_its_bridge() {
        use std::sync::mpsc;
        use std::time::{Duration, Instant};
        let (release_tx, release_rx) = mpsc::channel();
        let (weak_tx, weak_rx) = mpsc::channel();
        let loaded = spawn_root_loader(move || {
            release_rx
                .recv_timeout(Duration::from_secs(10))
                .map_err(|e| e.to_string())?;
            let root = LiteRoot::new(CrepusLiteConfig::default())?;
            weak_tx
                .send(Arc::downgrade(&root.bridge))
                .map_err(|e| e.to_string())?;
            Ok(root)
        })
        .unwrap();
        drop(loaded);
        release_tx.send(()).unwrap();
        let bridge = weak_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while bridge.upgrade().is_some() {
            assert!(
                Instant::now() < deadline,
                "canceled startup retained its bridge"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[cfg(unix)]
    #[gpui::test]
    fn guest_file_read_and_typescript_compile_yield_ui(cx: &mut gpui::TestAppContext) {
        use std::io::Write;
        use std::sync::mpsc;
        use std::time::Duration;

        cx.background_executor.allow_parking();
        let base = std::env::var("CREPUS_LITE_BASE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().unwrap())
            .canonicalize()
            .unwrap();
        let directory = tempfile::tempdir_in(base).unwrap();
        let path = directory.path().join("guest.ts");
        assert!(std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap()
            .success());
        let config = CrepusLiteConfig {
            guest_entry: Some(path.to_str().unwrap().into()),
            ..Default::default()
        };
        let (progress_tx, progress_rx) = mpsc::channel();
        let writer = std::thread::spawn(move || {
            let mut pipe = std::fs::OpenOptions::new().write(true).open(path).unwrap();
            let yielded = progress_rx.recv_timeout(Duration::from_secs(10)).is_ok();
            writeln!(pipe, "globalThis.sourceReadYielded = {yielded};\nexport function run(): number {{ return 42; }}").unwrap();
        });
        let bridge = Bridge::compute_only_bridge();
        let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
        let (root, cx) = cx.add_window_view(move |_, _| LiteRoot {
            v8: runtime,
            bridge,
            config,
        });
        cx.update(|window, app| {
            root.update(app, |root, cx| root.run_guest(window, cx));
        });
        // The writer releases the source only after the UI call returns.
        let _ = progress_tx.send(());
        let reply = root.update(cx, |root, _| {
            root.v8
                .handle
                .eval_rpc(
                    "String(globalThis.sourceReadYielded)".into(),
                    false,
                    root.bridge.clone(),
                )
                .unwrap()
        });
        assert_eq!(
            reply
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .unwrap(),
            "true"
        );
        writer.join().unwrap();
        cx.run_until_parked();
    }

    #[test]
    fn guest_preparation_preserves_preludes_typescript_and_runner_receiver() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("prelude.js"), "const seed = 40;").unwrap();
        for (filename, source) in [
            (
                "guest.ts",
                "export function run(): number { return seed + 2; }",
            ),
            (
                "guest.js",
                "const CrepusGuest = { value: seed + 2, run() { return this.value; } }",
            ),
        ] {
            std::fs::write(directory.path().join(filename), source).unwrap();
            let config = CrepusLiteConfig {
                guest_prelude: vec!["prelude.js".into()],
                guest_entry: Some(filename.into()),
                ..Default::default()
            };
            let script = prepare_guest_run(&config, directory.path(), false).unwrap();
            let bridge = Bridge::compute_only_bridge();
            let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
            let reply = runtime.handle.eval_rpc(script, false, bridge).unwrap();
            assert_eq!(
                reply
                    .recv_timeout(std::time::Duration::from_secs(10))
                    .unwrap()
                    .unwrap(),
                "42"
            );
        }
    }

    #[test]
    fn guest_preparation_returns_read_and_compile_errors() {
        let directory = tempfile::tempdir().unwrap();
        let config = CrepusLiteConfig {
            guest_entry: Some("guest.ts".into()),
            ..Default::default()
        };
        assert!(prepare_guest_run(&config, directory.path(), false)
            .unwrap_err()
            .contains("unavailable"));
        std::fs::write(directory.path().join("guest.ts"), "export function run( {").unwrap();
        assert!(prepare_guest_run(&config, directory.path(), false)
            .unwrap_err()
            .contains("guest compile error"));
    }

    #[gpui::test]
    fn waiting_for_guest_reply_allows_ui_work_to_run(cx: &mut gpui::TestAppContext) {
        // The real V8 worker wakes the foreground task from an OS thread.
        cx.background_executor.allow_parking();
        let bridge = Bridge::with_capabilities(
            [crepuscularity_lite::Capability::Host]
                .into_iter()
                .collect(),
        );
        let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
        let test_bridge = bridge.clone();
        let (root, cx) = cx.add_window_view(move |_, _| LiteRoot {
            v8: runtime,
            bridge,
            config: CrepusLiteConfig::default(),
        });
        let script = r#"
            globalThis.uiHeartbeatObserved = false;
            const pause = new Int32Array(new SharedArrayBuffer(4));
            const deadline = Date.now() + 2000;
            while (Date.now() < deadline) {
                const reply = JSON.parse(Crepus.invoke("host", "storageGet", '{"key":"ui-heartbeat"}'));
                if (reply.data.value === true) {
                    globalThis.uiHeartbeatObserved = true;
                    break;
                }
                Atomics.wait(pause, 0, 0, 1);
            }
            uiHeartbeatObserved;
        "#;
        cx.update(|window, app| {
            root.update(app, |root, cx| {
                root.run_script(script.into(), false, window, cx)
            });
        });
        // This must park on the pending reply rather than block until the guest deadline.
        cx.run_until_parked();
        test_bridge
            .host_state()
            .storage_set("ui-heartbeat", json!(true));
        let reply = root.update(cx, |root, _| {
            root.v8
                .handle
                .eval_rpc(
                    "String(globalThis.uiHeartbeatObserved)".into(),
                    false,
                    root.bridge.clone(),
                )
                .unwrap()
        });
        assert_eq!(
            reply
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap()
                .unwrap(),
            "true"
        );
        cx.run_until_parked();
    }

    #[gpui::test]
    fn scroll_view_respects_vertical_overflow_before_click_dispatch(cx: &mut gpui::TestAppContext) {
        cx.background_executor.allow_parking();
        for (overflow, expected_after) in [
            (None, "host-scroll-parent/1"),
            (Some("hidden"), "host-scroll-parent/0"),
        ] {
            let bridge = Bridge::default_arc();
            let node: HostNode = serde_json::from_value(json!({
            "type": "ScrollView", "id": "scroll-parent", "style": {"width": 100.0, "height": 100.0, "overflowY": overflow},
            "children": [
                {"type": "Button", "text": "First", "on_press": "shared-handler", "style": {"width": 100.0, "height": 100.0, "minHeight": 100.0}},
                {"type": "Button", "text": "Second", "on_press": "shared-handler", "style": {"width": 100.0, "height": 100.0, "minHeight": 100.0}}
            ]
        })).unwrap();
            bridge.host_state().render_tree(node);
            let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
            runtime.handle.eval_rpc("globalThis.__events = []; globalThis.__crepusHostDispatch = (id, payload) => { globalThis.__events.push({id, payload}); return null; };".to_owned(), false, bridge.clone()).unwrap().recv().unwrap().unwrap();
            let (root, cx) = cx.add_window_view(move |_, _| LiteRoot {
                v8: runtime,
                bridge: bridge.clone(),
                config: CrepusLiteConfig::default(),
            });
            let point = gpui::point(px(10.0), px(10.0));
            cx.simulate_click(point, gpui::Modifiers::default());
            cx.simulate_event(gpui::ScrollWheelEvent {
                position: point,
                delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.0), px(-100.0))),
                ..Default::default()
            });
            cx.simulate_click(point, gpui::Modifiers::default());
            let output = root.update(cx, |root, _| {
                root.v8
                    .handle
                    .eval_rpc(
                        "JSON.stringify(globalThis.__events)".to_owned(),
                        false,
                        root.bridge.clone(),
                    )
                    .unwrap()
                    .recv()
                    .unwrap()
                    .unwrap()
            });
            let events: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(events.as_array().unwrap().len(), 2);
            assert_eq!(events[0]["payload"]["nodeId"], "host-scroll-parent/0");
            assert_eq!(events[1]["payload"]["nodeId"], expected_after);
        }
    }

    #[gpui::test]
    fn repeated_buttons_dispatch_distinct_node_ids(cx: &mut gpui::TestAppContext) {
        cx.background_executor.allow_parking();
        for (node_type, style, second_point) in [
            (
                "View",
                json!({"display": "grid", "gridCols": 2, "gridRows": 1, "width": 200.0, "height": 100.0}),
                gpui::point(px(110.0), px(10.0)),
            ),
            (
                "View",
                json!({"direction": "row", "width": 200.0, "height": 100.0}),
                gpui::point(px(110.0), px(10.0)),
            ),
            (
                "ScrollView",
                json!({"width": 100.0, "height": 200.0}),
                gpui::point(px(10.0), px(110.0)),
            ),
        ] {
            let bridge = Bridge::default_arc();
            let node: HostNode = serde_json::from_value(json!({
            "type": node_type, "id": "parent", "style": style,
            "children": [
                {"type": "Button", "text": "First", "on_press": "shared-handler", "style": {"width": 100.0, "height": 100.0}},
                {"type": "Button", "text": "Hidden", "className": "hidden", "on_press": "hidden-handler", "style": {"width": 100.0, "height": 100.0}},
                {"type": "Button", "text": "Second", "on_press": "shared-handler", "style": {"width": 100.0, "height": 100.0}}
            ]
        })).unwrap();
            bridge.host_state().render_tree(node);
            let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
            runtime.handle.eval_rpc("globalThis.__events = []; globalThis.__crepusHostDispatch = (id, payload) => { globalThis.__events.push({id, payload}); return null; };".to_owned(), false, bridge.clone()).unwrap().recv().unwrap().unwrap();
            let (root, cx) = cx.add_window_view(move |_, _| LiteRoot {
                v8: runtime,
                bridge: bridge.clone(),
                config: CrepusLiteConfig::default(),
            });
            cx.simulate_click(gpui::point(px(10.0), px(10.0)), gpui::Modifiers::default());
            cx.simulate_click(second_point, gpui::Modifiers::default());
            let output = root.update(cx, |root, _| {
                root.v8
                    .handle
                    .eval_rpc(
                        "JSON.stringify(globalThis.__events)".to_owned(),
                        false,
                        root.bridge.clone(),
                    )
                    .unwrap()
                    .recv()
                    .unwrap()
                    .unwrap()
            });
            let events: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(events.as_array().unwrap().len(), 2);
            for (index, node_id) in ["host-parent/0", "host-parent/2"].into_iter().enumerate() {
                assert_eq!(events[index]["id"], "shared-handler");
                assert_eq!(events[index]["payload"]["source"], "press");
                assert_eq!(events[index]["payload"]["nodeId"], node_id);
            }
        }
    }

    #[gpui::test]
    fn focused_nodes_dispatch_keyboard_payload(cx: &mut gpui::TestAppContext) {
        cx.background_executor.allow_parking();
        for node_type in ["View", "Button", "TextInput", "Text", "ScrollView"] {
            let bridge = Bridge::default_arc();
            let node: HostNode = serde_json::from_value(json!({"type": node_type, "text": "Keyboard", "id": "keyboard-node", "on_key_down": "shared-handler", "style": {"width": 100.0, "height": 100.0}})).unwrap();
            bridge.host_state().render_tree(node);
            let runtime = V8ThreadRuntime::spawn(bridge.clone()).unwrap();
            runtime.handle.eval_rpc("globalThis.__lastEvent = null; globalThis.__crepusHostDispatch = (id, payload) => { globalThis.__lastEvent = {id, payload}; return null; };".to_owned(), false, bridge.clone()).unwrap().recv().unwrap().unwrap();
            let (root, cx) = cx.add_window_view(move |_, _| LiteRoot {
                v8: runtime,
                bridge: bridge.clone(),
                config: CrepusLiteConfig::default(),
            });
            cx.simulate_click(gpui::point(px(10.0), px(10.0)), gpui::Modifiers::default());
            cx.simulate_keystrokes("a");
            let output = root.update(cx, |root, _| {
                root.v8
                    .handle
                    .eval_rpc(
                        "JSON.stringify(globalThis.__lastEvent)".to_owned(),
                        false,
                        root.bridge.clone(),
                    )
                    .unwrap()
                    .recv()
                    .unwrap()
                    .unwrap()
            });
            let event: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(event["id"], "shared-handler");
            assert_eq!(event["payload"]["kind"], "keyDown");
            assert_eq!(event["payload"]["nodeId"], "keyboard-node");
            assert_eq!(event["payload"]["keystroke"], "A");
        }
    }

    #[test]
    fn shadow_presets_reach_gpui_and_none_clears_existing_shadows() {
        let mut expected = div().shadow_lg();
        let mut element = apply_host_style(
            div(),
            &HostStyle {
                shadow: Some("lg".to_owned()),
                ..HostStyle::default()
            },
        );
        assert_eq!(element.style().box_shadow, expected.style().box_shadow);
        let mut element = apply_host_style(
            div().shadow_lg(),
            &HostStyle {
                shadow: Some("none".to_owned()),
                ..HostStyle::default()
            },
        );
        assert!(element.style().box_shadow.as_ref().unwrap().is_empty());
        let mut element = apply_host_style(
            div().shadow_lg(),
            &HostStyle {
                shadow: Some("unknown".to_owned()),
                ..HostStyle::default()
            },
        );
        assert_eq!(element.style().box_shadow, expected.style().box_shadow);
    }

    #[test]
    fn grid_style_preserves_display_and_positive_track_counts() {
        let style = HostStyle {
            display: Some("grid".to_owned()),
            grid_cols: Some(2),
            grid_rows: Some(3),
            ..HostStyle::default()
        };
        let mut element = container_base(apply_host_style(div(), &style), &style);
        assert_eq!(element.style().display, Some(gpui::Display::Grid));
        assert_eq!(element.style().grid_cols.as_ref().unwrap().repeat, 2);
        assert_eq!(element.style().grid_rows.as_ref().unwrap().repeat, 3);
    }

    #[test]
    fn production_dispatch_script_preserves_ids_and_payload_inside_v8() {
        let mut host = crepuscularity_lite::V8Host::new(Bridge::default_arc()).unwrap();
        let handler = "handler\"雪\n";
        let payload = host_event_payload("press", "node\"雪\n");
        let missing: Value =
            serde_json::from_str(&host.eval(&host_dispatch_script(handler, &payload)).unwrap())
                .unwrap();
        assert_eq!(missing["error"], "host_dispatch_missing");
        assert_eq!(missing["handlerId"], handler);
        host.eval(
            "globalThis.__crepusHostDispatch = (handlerId, payload) => ({handlerId, payload});",
        )
        .unwrap();
        let reply: Value =
            serde_json::from_str(&host.eval(&host_dispatch_script(handler, &payload)).unwrap())
                .unwrap();
        assert_eq!(reply["handlerId"], handler);
        assert_eq!(reply["payload"], payload);
    }

    #[test]
    fn event_payloads_preserve_source_and_distinguish_nodes() {
        let first = host_event_payload("press", "host-root/0");
        let second = host_event_payload("press", "host-root/1");
        assert_eq!(first["source"], "press");
        assert_ne!(first["nodeId"], second["nodeId"]);
        let focus = host_event_payload("focus", "quoted\"雪");
        let serialized = serde_json::to_string(&focus).unwrap();
        let decoded: Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(decoded["nodeId"], "quoted\"雪");
        assert_eq!(decoded["source"], "focus");
    }

    #[test]
    fn container_defaults_preserve_explicit_block_display() {
        let mut default = container_base(div(), &HostStyle::default());
        assert_eq!(default.style().display, Some(gpui::Display::Flex));
        assert_eq!(
            default.style().flex_direction,
            Some(gpui::FlexDirection::Column)
        );
        let style = HostStyle {
            display: Some("block".to_owned()),
            ..HostStyle::default()
        };
        let mut explicit = container_base(apply_host_style(div(), &style), &style);
        assert_eq!(explicit.style().display, Some(gpui::Display::Block));
        let mut button = button_base(div(), &style);
        assert_eq!(button.style().display, Some(gpui::Display::Block));
    }

    #[test]
    fn button_defaults_do_not_replace_explicit_host_styles() {
        let mut defaults = button_base(div(), &HostStyle::default());
        assert_eq!(defaults.style().align_items, Some(gpui::AlignItems::Center));
        assert_eq!(
            defaults.style().justify_content,
            Some(gpui::JustifyContent::Center)
        );
        assert_eq!(defaults.style().padding.left, Some(gpui::rems(1.0).into()));
        let style = HostStyle {
            padding: Some(2.5),
            radius: Some(3.5),
            align_items: Some("end".to_owned()),
            justify_content: Some("start".to_owned()),
            ..HostStyle::default()
        };
        let mut custom = button_base(div(), &style);
        assert_eq!(custom.style().padding.left, Some(px(2.5).into()));
        assert_eq!(custom.style().padding.top, Some(px(2.5).into()));
        assert_eq!(custom.style().corner_radii.top_left, Some(px(3.5).into()));
        assert_eq!(custom.style().align_items, Some(gpui::AlignItems::FlexEnd));
        assert_eq!(
            custom.style().justify_content,
            Some(gpui::JustifyContent::Start)
        );
    }

    #[test]
    fn input_defaults_do_not_replace_explicit_host_styles() {
        let mut defaults = text_input_base(div(), &HostStyle::default());
        assert_eq!(defaults.style().padding.left, Some(gpui::rems(0.75).into()));
        assert_eq!(defaults.style().padding.top, Some(gpui::rems(0.5).into()));
        let style = HostStyle {
            padding: Some(2.5),
            radius: Some(3.5),
            border_width: Some(4.0),
            ..HostStyle::default()
        };
        let mut custom = text_input_base(div(), &style);
        assert_eq!(custom.style().padding.left, Some(px(2.5).into()));
        assert_eq!(custom.style().padding.top, Some(px(2.5).into()));
        assert_eq!(custom.style().corner_radii.top_left, Some(px(3.5).into()));
        assert_eq!(custom.style().border_widths.top, Some(px(4.0).into()));
    }

    #[test]
    fn fractional_fixed_dimension_classes_reach_gpui() {
        let node: HostNode =
            serde_json::from_value(json!({"type": "View", "className": "w-[100.5px] h-1.75"}))
                .unwrap();
        let style = crepuscularity_lite::host::HostState::default()
            .render_tree(node)
            .tree
            .unwrap()
            .style;
        let mut element = apply_host_style(div(), &style);
        assert_eq!(element.style().size.width, Some(px(100.5).into()));
        assert_eq!(element.style().size.height, Some(px(7.0).into()));
    }

    #[test]
    fn fractional_font_classes_reach_gpui() {
        let node: HostNode = serde_json::from_value(
            json!({"type": "Text", "className": "text-[18.5px] text-red-500"}),
        )
        .unwrap();
        let style = crepuscularity_lite::host::HostState::default()
            .render_tree(node)
            .tree
            .unwrap()
            .style;
        let mut element = apply_host_style(div(), &style);
        assert_eq!(element.text_style().font_size, Some(px(18.5).into()));
    }

    #[test]
    fn fractional_padding_classes_reach_gpui_on_both_axes() {
        let node: HostNode = serde_json::from_value(
            json!({"type": "View", "className": "p-[2.5px] px-[3.5px] py-1.75"}),
        )
        .unwrap();
        let style = crepuscularity_lite::host::HostState::default()
            .render_tree(node)
            .tree
            .unwrap()
            .style;
        let mut element = apply_host_style(div(), &style);
        assert_eq!(element.style().padding.left, Some(px(3.5).into()));
        assert_eq!(element.style().padding.right, Some(px(3.5).into()));
        assert_eq!(element.style().padding.top, Some(px(7.0).into()));
        assert_eq!(element.style().padding.bottom, Some(px(7.0).into()));
    }

    #[test]
    fn explicit_font_families_reach_gpui_and_empty_names_preserve_styles() {
        for (family, expected) in [
            ("Menlo", "Menlo"),
            ("  Custom Family  ", "Custom Family"),
            ("", "Existing"),
            ("   ", "Existing"),
        ] {
            let style = HostStyle {
                font_family: Some(family.to_owned()),
                ..HostStyle::default()
            };
            let mut element = apply_host_style(div().font_family("Existing"), &style);
            assert_eq!(element.text_style().font_family.as_deref(), Some(expected));
        }
    }

    #[test]
    fn explicit_false_decoration_overrides_inherited_parent_style() {
        use gpui::Refineable;
        let mut parent = div().underline().line_through();
        let mut inherited = gpui::TextStyle::default();
        inherited.refine(parent.text_style());
        let style = HostStyle {
            underline: Some(false),
            strikethrough: Some(false),
            ..HostStyle::default()
        };
        let mut child = apply_host_style(div(), &style);
        inherited.refine(child.text_style());
        assert_eq!(inherited.underline.unwrap().thickness, px(0.0));
        assert_eq!(inherited.strikethrough.unwrap().thickness, px(0.0));
    }

    #[test]
    fn text_decorations_reach_gpui_and_explicit_false_clears_them() {
        for enabled in [true, false] {
            let style = HostStyle {
                italic: Some(enabled),
                underline: Some(enabled),
                strikethrough: Some(enabled),
                ..HostStyle::default()
            };
            let mut element = apply_host_style(div().italic().underline().line_through(), &style);
            assert_eq!(
                element.text_style().font_style,
                Some(if enabled {
                    gpui::FontStyle::Italic
                } else {
                    gpui::FontStyle::Normal
                })
            );
            let thickness = if enabled { 1.0 } else { 0.0 };
            assert_eq!(
                element.text_style().underline.unwrap().thickness,
                px(thickness)
            );
            assert_eq!(
                element.text_style().strikethrough.unwrap().thickness,
                px(thickness)
            );
        }
    }

    #[test]
    fn hidden_display_overrides_layout_direction() {
        let style = HostStyle {
            display: Some("none".to_owned()),
            direction: Some("row".to_owned()),
            ..HostStyle::default()
        };
        let mut element = apply_host_style(div(), &style);
        assert_eq!(element.style().display, Some(gpui::Display::None));
        let style = HostStyle {
            display: Some("flex".to_owned()),
            ..HostStyle::default()
        };
        let mut element = apply_host_style(div().hidden(), &style);
        assert_eq!(element.style().display, Some(gpui::Display::Flex));
    }

    #[test]
    fn overflow_axes_override_general_settings_and_ignore_unknown_values() {
        let style = HostStyle {
            overflow: Some("hidden".to_owned()),
            overflow_x: Some("auto".to_owned()),
            overflow_y: Some("unknown".to_owned()),
            ..HostStyle::default()
        };
        let mut element = apply_host_style(div(), &style);
        assert_eq!(element.style().overflow.x, Some(gpui::Overflow::Scroll));
        assert_eq!(element.style().overflow.y, Some(gpui::Overflow::Hidden));
        for (value, expected) in [
            ("visible", gpui::Overflow::Visible),
            ("clip", gpui::Overflow::Clip),
            ("scroll", gpui::Overflow::Scroll),
        ] {
            let mut element = apply_host_style(
                div(),
                &HostStyle {
                    overflow: Some(value.to_owned()),
                    ..HostStyle::default()
                },
            );
            assert_eq!(element.style().overflow.x, Some(expected));
            assert_eq!(element.style().overflow.y, Some(expected));
        }
    }

    #[test]
    fn fallback_ids_are_structural_and_explicit_ids_are_preserved() {
        let node = HostNode {
            node_type: "Button".to_owned(),
            on_press: Some("shared-handler".to_owned()),
            ..HostNode::default()
        };
        let first = host_node_id(&node, "root/0");
        let second = host_node_id(&node, "root/1");
        assert_ne!(host_element_id(&first), host_element_id(&second));
        assert_eq!(first, host_node_id(&node, "root/0"));
        assert_ne!(
            host_node_id(&node, "root/0/0"),
            host_node_id(&node, "root/1/0")
        );
        let explicit = HostNode {
            id: Some("chosen".to_owned()),
            ..node
        };
        assert_eq!(host_node_id(&explicit, "root/0"), "chosen");
    }

    #[test]
    fn flex_growth_preserves_exact_values_and_ignores_invalid_inputs() {
        for grow in [0.0, 1.0, 2.5] {
            let style = HostStyle {
                flex_grow: Some(grow),
                ..HostStyle::default()
            };
            let mut element = apply_host_style(div(), &style);
            assert_eq!(element.style().flex_grow, Some(grow));
        }
        for grow in [f32::NAN, f32::INFINITY, -1.0] {
            let style = HostStyle {
                flex_grow: Some(grow),
                ..HostStyle::default()
            };
            let mut element = apply_host_style(div().flex_grow(2.0), &style);
            assert_eq!(element.style().flex_grow, Some(2.0));
        }
    }

    #[test]
    fn alignment_and_justification_reach_gpui() {
        for (name, expected) in [
            ("start", gpui::AlignItems::FlexStart),
            ("center", gpui::AlignItems::Center),
            ("end", gpui::AlignItems::FlexEnd),
            ("stretch", gpui::AlignItems::Stretch),
        ] {
            let mut element = apply_host_style(
                div(),
                &HostStyle {
                    align_items: Some(name.to_owned()),
                    ..HostStyle::default()
                },
            );
            assert_eq!(element.style().align_items, Some(expected));
        }
        for (name, expected) in [
            ("start", gpui::JustifyContent::Start),
            ("center", gpui::JustifyContent::Center),
            ("end", gpui::JustifyContent::End),
            ("between", gpui::JustifyContent::SpaceBetween),
            ("around", gpui::JustifyContent::SpaceAround),
            ("evenly", gpui::JustifyContent::SpaceEvenly),
        ] {
            let mut element = apply_host_style(
                div(),
                &HostStyle {
                    justify_content: Some(name.to_owned()),
                    ..HostStyle::default()
                },
            );
            assert_eq!(element.style().justify_content, Some(expected));
        }
    }

    #[test]
    fn padding_radius_and_font_sizes_validate_explicit_values() {
        let style = HostStyle {
            padding: Some(2.5),
            padding_x: Some(4.5),
            padding_y: Some(6.5),
            radius: Some(3.5),
            font_size: Some(18.5),
            ..HostStyle::default()
        };
        let mut element = apply_host_style(div(), &style);
        assert_eq!(element.style().padding.left, Some(px(4.5).into()));
        assert_eq!(element.style().padding.top, Some(px(6.5).into()));
        assert_eq!(element.style().corner_radii.top_left, Some(px(3.5).into()));
        assert_eq!(element.text_style().font_size, Some(px(18.5).into()));
        for invalid in [f32::NAN, f32::INFINITY, -1.0] {
            let style = HostStyle {
                padding: Some(invalid),
                padding_x: Some(invalid),
                padding_y: Some(invalid),
                radius: Some(invalid),
                font_size: Some(invalid),
                ..HostStyle::default()
            };
            let mut element = apply_host_style(
                div().p(px(3.0)).rounded(px(2.0)).text_size(px(16.0)),
                &style,
            );
            assert_eq!(element.style().padding.top, Some(px(3.0).into()));
            assert_eq!(element.style().padding.left, Some(px(3.0).into()));
            assert_eq!(element.style().corner_radii.top_left, Some(px(2.0).into()));
            assert_eq!(element.text_style().font_size, Some(px(16.0).into()));
        }
    }

    #[test]
    fn text_alignment_reaches_gpui_and_unknown_values_preserve_styles() {
        for (name, expected) in [
            ("left", gpui::TextAlign::Left),
            ("leading", gpui::TextAlign::Left),
            ("center", gpui::TextAlign::Center),
            ("right", gpui::TextAlign::Right),
            ("trailing", gpui::TextAlign::Right),
        ] {
            let style = HostStyle {
                text_align: Some(name.to_owned()),
                ..HostStyle::default()
            };
            let mut element = apply_host_style(div(), &style);
            assert_eq!(element.text_style().text_align, Some(expected));
        }
        let style = HostStyle {
            text_align: Some("unknown".to_owned()),
            ..HostStyle::default()
        };
        let mut element = apply_host_style(div().text_align(gpui::TextAlign::Center), &style);
        assert_eq!(
            element.text_style().text_align,
            Some(gpui::TextAlign::Center)
        );
    }

    #[test]
    fn named_font_weights_reach_gpui_without_unknown_fallback() {
        for (name, expected) in [
            ("thin", FontWeight::THIN),
            ("extralight", FontWeight::EXTRA_LIGHT),
            ("light", FontWeight::LIGHT),
            ("normal", FontWeight::NORMAL),
            ("medium", FontWeight::MEDIUM),
            ("semibold", FontWeight::SEMIBOLD),
            ("bold", FontWeight::BOLD),
            ("extrabold", FontWeight::EXTRA_BOLD),
            ("black", FontWeight::BLACK),
        ] {
            let style = HostStyle {
                font_weight: Some(name.to_owned()),
                ..HostStyle::default()
            };
            let mut element = apply_host_style(div(), &style);
            assert_eq!(element.text_style().font_weight, Some(expected));
        }
        let style = HostStyle {
            font_weight: Some("unknown".to_owned()),
            ..HostStyle::default()
        };
        let mut element = apply_host_style(div().font_weight(FontWeight::NORMAL), &style);
        assert_eq!(element.text_style().font_weight, Some(FontWeight::NORMAL));
    }

    #[test]
    fn min_max_dimensions_reach_gpui_and_ignore_invalid_values() {
        let style = HostStyle {
            min_width: Some(20.5),
            min_height: Some(0.0),
            max_width: Some(300.0),
            max_height: Some(200.5),
            ..HostStyle::default()
        };
        let mut element = apply_host_style(div(), &style);
        assert_eq!(element.style().min_size.width, Some(px(20.5).into()));
        assert_eq!(element.style().min_size.height, Some(px(0.0).into()));
        assert_eq!(element.style().max_size.width, Some(px(300.0).into()));
        assert_eq!(element.style().max_size.height, Some(px(200.5).into()));
        for invalid in [f32::NAN, f32::INFINITY, -1.0] {
            let style = HostStyle {
                min_width: Some(invalid),
                min_height: Some(invalid),
                max_width: Some(invalid),
                max_height: Some(invalid),
                ..HostStyle::default()
            };
            let mut element = apply_host_style(div(), &style);
            assert_eq!(element.style().min_size.width, None);
            assert_eq!(element.style().min_size.height, None);
            assert_eq!(element.style().max_size.width, None);
            assert_eq!(element.style().max_size.height, None);
        }
    }

    #[test]
    fn dimensions_reach_gpui_as_pixels_or_proportions() {
        let mut pixels = apply_host_style(
            div(),
            &HostStyle {
                width: Some(100.0),
                height: Some(25.0),
                ..HostStyle::default()
            },
        );
        assert_eq!(pixels.style().size.width, Some(px(100.0).into()));
        assert_eq!(pixels.style().size.height, Some(px(25.0).into()));
        let mut fractions = apply_host_style(
            div(),
            &HostStyle {
                width_fraction: Some(1.0),
                height_fraction: Some(0.5),
                ..HostStyle::default()
            },
        );
        assert_eq!(
            fractions.style().size.width,
            Some(gpui::relative(1.0).into())
        );
        assert_eq!(
            fractions.style().size.height,
            Some(gpui::relative(0.5).into())
        );
    }

    #[test]
    fn border_width_and_opacity_reach_gpui_style() {
        for width in [0.0, 1.0, 2.5, 8.0] {
            let style = HostStyle {
                border_width: Some(width),
                ..HostStyle::default()
            };
            let mut element = apply_host_style(div(), &style);
            let border = &element.style().border_widths;
            assert_eq!(border.top, Some(px(width).into()));
            assert_eq!(border.right, Some(px(width).into()));
            assert_eq!(border.bottom, Some(px(width).into()));
            assert_eq!(border.left, Some(px(width).into()));
        }
        for (opacity, expected) in [(-0.5, 0.0), (0.25, 0.25), (2.0, 1.0)] {
            let style = HostStyle {
                opacity: Some(opacity),
                ..HostStyle::default()
            };
            let mut element = apply_host_style(div(), &style);
            assert_eq!(element.style().opacity, Some(expected));
        }
        for invalid in [f32::NAN, f32::INFINITY] {
            let style = HostStyle {
                border_width: Some(invalid),
                opacity: Some(invalid),
                ..HostStyle::default()
            };
            let mut element = apply_host_style(div().border(px(3.0)).opacity(0.5), &style);
            assert_eq!(element.style().border_widths.top, Some(px(3.0).into()));
            assert_eq!(element.style().opacity, Some(0.5));
        }
    }

    #[test]
    fn gaps_preserve_exact_pixels_and_ignore_invalid_values() {
        for gap in [0.0, 2.0, 6.0, 2.5, 17.0] {
            let mut element = add_gap(div(), Some(gap));
            assert_eq!(element.style().gap.width, Some(px(gap).into()));
            assert_eq!(element.style().gap.height, Some(px(gap).into()));
        }
        for gap in [f32::NAN, f32::INFINITY, -1.0] {
            let mut element = add_gap(div().gap(px(3.0)), Some(gap));
            assert_eq!(element.style().gap.width, Some(px(3.0).into()));
        }
    }
}
