//! Host-side state for the GPUI-backed guest runtime.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crepuscularity_core::tailwind::{
    parse_color_rgb, parse_radius_px, parse_size_width_height, parse_spacing_px,
    parse_text_size_px, SizeToken,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::bridge::BridgeError;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HostSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree: Option<HostNode>,
    #[serde(default)]
    pub navigation: Vec<HostRoute>,
    #[serde(default)]
    pub storage_keys: Vec<String>,
    #[serde(default)]
    pub render_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_event: Option<HostEventRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HostRoute {
    pub name: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HostEventRecord {
    pub kind: String,
    #[serde(default)]
    pub handler_id: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HostNode {
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(
        default,
        rename = "className",
        alias = "class",
        skip_serializing_if = "Option::is_none"
    )]
    pub class_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_press: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_key_down: Option<String>,
    #[serde(default)]
    pub props: Value,
    #[serde(default)]
    pub style: HostStyle,
    #[serde(default)]
    pub children: Vec<HostNode>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HostStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_items: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justify_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_y: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gap: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex_grow: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overflow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overflow_x: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overflow_y: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_cols: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_rows: Option<u16>,
}

#[derive(Debug, Default)]
struct HostStateInner {
    tree: Option<HostNode>,
    navigation: Vec<HostRoute>,
    storage: HashMap<String, Value>,
    channels: HashMap<String, Vec<Value>>,
    render_count: u64,
    last_event: Option<HostEventRecord>,
}

#[derive(Debug, Default)]
pub struct HostState {
    inner: Mutex<HostStateInner>,
}

impl HostState {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn render_tree(&self, mut tree: HostNode) -> HostSnapshot {
        normalize_host_tree_classes(&mut tree);
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.tree = Some(tree);
        inner.render_count += 1;
        if inner.navigation.is_empty() {
            inner.navigation.push(HostRoute {
                name: "root".to_string(),
                params: json!({}),
            });
        }
        snapshot_from_inner(&inner)
    }

    pub fn snapshot(&self) -> HostSnapshot {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        snapshot_from_inner(&inner)
    }

    pub fn navigation_push(&self, route: HostRoute) -> HostSnapshot {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        ensure_root_route(&mut inner);
        inner.navigation.push(route);
        snapshot_from_inner(&inner)
    }

    pub fn navigation_replace(&self, route: HostRoute) -> HostSnapshot {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        ensure_root_route(&mut inner);
        if inner.navigation.is_empty() {
            inner.navigation.push(route);
        } else if let Some(last) = inner.navigation.last_mut() {
            *last = route;
        }
        snapshot_from_inner(&inner)
    }

    pub fn navigation_pop(&self) -> HostSnapshot {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        ensure_root_route(&mut inner);
        if inner.navigation.len() > 1 {
            inner.navigation.pop();
        }
        snapshot_from_inner(&inner)
    }

    pub fn storage_get(&self, key: &str) -> Option<Value> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.storage.get(key).cloned()
    }

    pub fn storage_set(&self, key: impl Into<String>, value: Value) -> HostSnapshot {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.storage.insert(key.into(), value);
        snapshot_from_inner(&inner)
    }

    pub fn storage_remove(&self, key: &str) -> HostSnapshot {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.storage.remove(key);
        snapshot_from_inner(&inner)
    }

    pub fn record_event(&self, event: HostEventRecord) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.last_event = Some(event);
    }

    pub fn channel_send(&self, channel: impl Into<String>, message: Value) -> HostSnapshot {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner
            .channels
            .entry(channel.into())
            .or_default()
            .push(message);
        snapshot_from_inner(&inner)
    }

    pub fn channel_poll(&self, channel: &str) -> Option<Value> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let queue = inner.channels.get_mut(channel)?;
        if queue.is_empty() {
            None
        } else {
            Some(queue.remove(0))
        }
    }
}

fn normalize_host_tree_classes(node: &mut HostNode) {
    let mut classes = Vec::new();
    if let Some(class_name) = &node.class_name {
        classes.extend(class_name.split_whitespace());
    }
    if let Some(class_name) = node.props.get("class").and_then(Value::as_str) {
        classes.extend(class_name.split_whitespace());
    }
    if let Some(class_name) = node.props.get("className").and_then(Value::as_str) {
        classes.extend(class_name.split_whitespace());
    }
    for class in classes {
        apply_host_class(class, &mut node.style);
    }
    for child in &mut node.children {
        normalize_host_tree_classes(child);
    }
}

fn apply_host_class(class: &str, style: &mut HostStyle) {
    match class {
        "flex" => {
            style.display.get_or_insert_with(|| "flex".to_string());
        }
        "flex-row" => {
            style.direction.get_or_insert_with(|| "row".to_string());
        }
        "flex-col" => {
            style.direction.get_or_insert_with(|| "column".to_string());
        }
        "flex-1" => {
            style.flex_grow.get_or_insert(1.0);
        }
        "items-start" => {
            style.align_items.get_or_insert_with(|| "start".to_string());
        }
        "items-center" => {
            style
                .align_items
                .get_or_insert_with(|| "center".to_string());
        }
        "items-end" => {
            style.align_items.get_or_insert_with(|| "end".to_string());
        }
        "justify-start" => {
            style
                .justify_content
                .get_or_insert_with(|| "start".to_string());
        }
        "justify-center" => {
            style
                .justify_content
                .get_or_insert_with(|| "center".to_string());
        }
        "justify-end" => {
            style
                .justify_content
                .get_or_insert_with(|| "end".to_string());
        }
        "w-full" => {
            style.width.get_or_insert(100.0);
        }
        "h-full" => {
            style.height.get_or_insert(100.0);
        }
        "border" | "border-1" => {
            style.border_width.get_or_insert(1.0);
        }
        "font-bold" => {
            style.font_weight.get_or_insert_with(|| "bold".to_string());
        }
        "font-medium" => {
            style
                .font_weight
                .get_or_insert_with(|| "medium".to_string());
        }
        _ => {
            apply_host_parametric_class(class, style);
        }
    }
}

fn apply_host_parametric_class(class: &str, style: &mut HostStyle) {
    if let Some(value) = class.strip_prefix("p-").and_then(parse_spacing_px) {
        style.padding.get_or_insert(f32::from(value));
        return;
    }
    if let Some(value) = class.strip_prefix("px-").and_then(parse_spacing_px) {
        style.padding_x.get_or_insert(f32::from(value));
        return;
    }
    if let Some(value) = class.strip_prefix("py-").and_then(parse_spacing_px) {
        style.padding_y.get_or_insert(f32::from(value));
        return;
    }
    if let Some(value) = class.strip_prefix("gap-").and_then(parse_spacing_px) {
        style.gap.get_or_insert(f32::from(value));
        return;
    }
    if let Some(value) = class.strip_prefix("w-").and_then(parse_host_size) {
        style.width.get_or_insert(value);
        return;
    }
    if let Some(value) = class.strip_prefix("h-").and_then(parse_host_size) {
        style.height.get_or_insert(value);
        return;
    }
    if let Some(value) = class.strip_prefix("rounded-").and_then(parse_radius_px) {
        style.radius.get_or_insert(f32::from(value));
        return;
    }
    if class == "rounded" {
        if let Some(value) = parse_radius_px("") {
            style.radius.get_or_insert(f32::from(value));
        }
        return;
    }
    if let Some(value) = class.strip_prefix("text-").and_then(parse_text_size_px) {
        style.font_size.get_or_insert(f32::from(value));
        return;
    }
    if let Some(color) = class.strip_prefix("bg-").and_then(parse_host_color) {
        style.background.get_or_insert(color);
        return;
    }
    if let Some(color) = class.strip_prefix("text-").and_then(parse_host_color) {
        style.color.get_or_insert(color);
        return;
    }
    if let Some(color) = class.strip_prefix("border-").and_then(parse_host_color) {
        style.border_color.get_or_insert(color);
    }
}

fn parse_host_size(raw: &str) -> Option<f32> {
    match parse_size_width_height(raw)? {
        SizeToken::Full => Some(100.0),
        SizeToken::Auto => None,
        SizeToken::Px(value) | SizeToken::Spacing(value) => Some(f32::from(value)),
        SizeToken::Fraction { num, den } => Some((f32::from(num) / f32::from(den)) * 100.0),
    }
}

fn parse_host_color(raw: &str) -> Option<String> {
    let [r, g, b] = parse_color_rgb(raw)?;
    Some(format!("#{r:02x}{g:02x}{b:02x}"))
}

fn snapshot_from_inner(inner: &HostStateInner) -> HostSnapshot {
    let mut storage_keys = inner.storage.keys().cloned().collect::<Vec<_>>();
    storage_keys.sort();
    HostSnapshot {
        tree: inner.tree.clone(),
        navigation: inner.navigation.clone(),
        storage_keys,
        render_count: inner.render_count,
        last_event: inner.last_event.clone(),
    }
}

fn ensure_root_route(inner: &mut HostStateInner) {
    if inner.navigation.is_empty() {
        inner.navigation.push(HostRoute {
            name: "root".to_string(),
            params: json!({}),
        });
    }
}

pub fn require_string_field<'a>(payload: &'a Value, key: &str) -> Result<&'a str, BridgeError> {
    payload.get(key).and_then(Value::as_str).ok_or_else(|| {
        BridgeError::with_details(
            "invalid_payload",
            format!("expected string field {key:?}"),
            payload.clone(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_messaging_queue_and_poll() {
        let host = HostState::default();
        host.channel_send("chat", json!("hello"));
        host.channel_send("chat", json!("world"));
        host.channel_send("system", json!("ping"));

        assert_eq!(host.channel_poll("chat"), Some(json!("hello")));
        assert_eq!(host.channel_poll("chat"), Some(json!("world")));
        assert_eq!(host.channel_poll("chat"), None);

        assert_eq!(host.channel_poll("system"), Some(json!("ping")));
        assert_eq!(host.channel_poll("system"), None);

        assert_eq!(host.channel_poll("non_existent"), None);
    }

    #[test]
    fn require_string_field_tests() {
        let payload = json!({
            "valid": "hello",
            "number": 42
        });

        assert_eq!(require_string_field(&payload, "valid").unwrap(), "hello");

        let err = require_string_field(&payload, "missing").unwrap_err();
        assert_eq!(err.code, "invalid_payload");
        assert_eq!(err.message, "expected string field \"missing\"");

        let err = require_string_field(&payload, "number").unwrap_err();
        assert_eq!(err.code, "invalid_payload");
        assert_eq!(err.message, "expected string field \"number\"");
    }

    #[test]
    fn storage_roundtrip_and_snapshot_keys() {
        let host = HostState::default();
        host.storage_set("alpha", json!("one"));
        host.storage_set("beta", json!({ "n": 2 }));
        assert_eq!(host.storage_get("alpha"), Some(json!("one")));
        let snap = host.snapshot();
        assert_eq!(
            snap.storage_keys,
            vec!["alpha".to_string(), "beta".to_string()]
        );
    }

    #[test]
    fn navigation_replace_and_pop_preserve_root() {
        let host = HostState::default();
        host.navigation_push(HostRoute {
            name: "inbox".to_string(),
            params: json!({ "guild": "123" }),
        });
        host.navigation_replace(HostRoute {
            name: "channel".to_string(),
            params: json!({ "id": "999" }),
        });
        let snap = host.navigation_pop();
        assert_eq!(snap.navigation.len(), 1);
        assert_eq!(snap.navigation[0].name, "root");
    }

    #[test]
    fn render_tree_increments_render_count() {
        let host = HostState::default();
        let snap = host.render_tree(HostNode {
            node_type: "View".to_string(),
            children: vec![HostNode {
                node_type: "Text".to_string(),
                text: Some("Hello".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        });
        assert_eq!(snap.render_count, 1);
        assert_eq!(snap.tree.expect("tree").children.len(), 1);
    }

    #[test]
    fn render_tree_accepts_react_class_props() {
        let tree: HostNode = serde_json::from_value(json!({
            "type": "View",
            "className": "flex flex-col gap-4 bg-zinc-950 text-white p-4 rounded-md",
            "children": [{
                "type": "Text",
                "class": "text-xl font-bold",
                "text": "Hello"
            }]
        }))
        .unwrap();

        let host = HostState::default();
        let snap = host.render_tree(tree);
        let tree = snap.tree.expect("tree");
        assert_eq!(tree.style.direction.as_deref(), Some("column"));
        assert_eq!(tree.style.gap, Some(16.0));
        assert_eq!(tree.style.background.as_deref(), Some("#09090b"));
        assert_eq!(tree.style.color.as_deref(), Some("#ffffff"));
        assert_eq!(tree.style.padding, Some(16.0));
        assert_eq!(tree.style.radius, Some(6.0));
        assert_eq!(tree.children[0].style.font_size, Some(20.0));
        assert_eq!(tree.children[0].style.font_weight.as_deref(), Some("bold"));
    }

    #[test]
    fn record_event_updates_last_event() {
        let host = HostState::default();
        let event = HostEventRecord {
            kind: "click".to_string(),
            handler_id: "btn-1".to_string(),
            payload: json!({ "x": 10 }),
        };
        host.record_event(event.clone());
        let snap = host.snapshot();
        assert_eq!(snap.last_event, Some(event));
    }

    #[test]
    fn test_apply_host_class_display() {
        let mut style = HostStyle::default();
        apply_host_class("flex", &mut style);
        assert_eq!(style.display.as_deref(), Some("flex"));
    }

    #[test]
    fn test_apply_host_class_direction() {
        let mut style = HostStyle::default();
        apply_host_class("flex-row", &mut style);
        assert_eq!(style.direction.as_deref(), Some("row"));

        let mut style = HostStyle::default();
        apply_host_class("flex-col", &mut style);
        assert_eq!(style.direction.as_deref(), Some("column"));
    }

    #[test]
    fn test_apply_host_class_align_items() {
        let mut style = HostStyle::default();
        apply_host_class("items-start", &mut style);
        assert_eq!(style.align_items.as_deref(), Some("start"));
    }

    #[test]
    fn test_apply_host_class_justify_content() {
        let mut style = HostStyle::default();
        apply_host_class("justify-start", &mut style);
        assert_eq!(style.justify_content.as_deref(), Some("start"));
    }

    #[test]
    fn test_apply_host_class_dimensions() {
        let mut style = HostStyle::default();
        apply_host_class("w-full", &mut style);
        assert_eq!(style.width, Some(100.0));

        let mut style = HostStyle::default();
        apply_host_class("h-full", &mut style);
        assert_eq!(style.height, Some(100.0));
    }

    #[test]
    fn test_apply_host_class_border() {
        let mut style = HostStyle::default();
        apply_host_class("border", &mut style);
        assert_eq!(style.border_width, Some(1.0));
    }

    #[test]
    fn test_apply_host_class_font_weight() {
        let mut style = HostStyle::default();
        apply_host_class("font-bold", &mut style);
        assert_eq!(style.font_weight.as_deref(), Some("bold"));
    }

    #[test]
    fn test_apply_host_class_parametric_fallback() {
        let mut style = HostStyle::default();
        apply_host_class("p-4", &mut style);
        assert_eq!(style.padding, Some(16.0));
    }
}
