//! Host-side state for the GPUI-backed guest runtime.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use crepuscularity_core::tailwind::{
    parse_font_size_named, parse_opacity_percentage, parse_radius_px, parse_size_width_height,
    parse_spacing_pt, parse_text_size_px, resolve_css_color, SizeToken,
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

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct HostRoute {
    pub name: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct HostEventRecord {
    pub kind: String,
    #[serde(default)]
    pub handler_id: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Default, Serialize, Deserialize, PartialEq)]
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

fn clone_host_value(value: &Value) -> Value {
    let mut cloned = Value::Null;
    let mut pending = vec![(value, &mut cloned)];
    while let Some((source, destination)) = pending.pop() {
        match source {
            Value::Array(values) => {
                *destination = Value::Array(vec![Value::Null; values.len()]);
                pending.extend(
                    values
                        .iter()
                        .zip(destination.as_array_mut().unwrap().iter_mut()),
                );
            }
            Value::Object(values) => {
                *destination = Value::Object(
                    values
                        .keys()
                        .map(|key| (key.clone(), Value::Null))
                        .collect(),
                );
                pending.extend(
                    values
                        .values()
                        .zip(destination.as_object_mut().unwrap().values_mut()),
                );
            }
            _ => *destination = source.clone(),
        }
    }
    cloned
}

fn drop_host_value(value: Value) {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(mut values) => pending.append(&mut values),
            Value::Object(values) => pending.extend(values.into_iter().map(|(_, value)| value)),
            _ => {}
        }
    }
}

impl Clone for HostRoute {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            params: clone_host_value(&self.params),
        }
    }
}

impl Clone for HostEventRecord {
    fn clone(&self) -> Self {
        Self {
            kind: self.kind.clone(),
            handler_id: self.handler_id.clone(),
            payload: clone_host_value(&self.payload),
        }
    }
}

impl HostNode {
    fn clone_without_children(&self) -> Self {
        Self {
            node_type: self.node_type.clone(),
            id: self.id.clone(),
            text: self.text.clone(),
            title: self.title.clone(),
            value: self.value.clone(),
            placeholder: self.placeholder.clone(),
            class_name: self.class_name.clone(),
            route: self.route.clone(),
            on_press: self.on_press.clone(),
            on_key_down: self.on_key_down.clone(),
            props: clone_host_value(&self.props),
            style: self.style.clone(),
            children: Vec::new(),
        }
    }
}

impl Clone for HostNode {
    fn clone(&self) -> Self {
        let mut cloned = self.clone_without_children();
        let mut pending = vec![(self, &mut cloned)];
        while let Some((source, destination)) = pending.pop() {
            destination.children = source
                .children
                .iter()
                .map(Self::clone_without_children)
                .collect();
            pending.extend(source.children.iter().zip(destination.children.iter_mut()));
        }
        cloned
    }
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
    /// Absolute pixels; SIZE_FIT means size to content. Takes precedence over width_fraction.
    pub width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Proportion of the parent width, where 1.0 fills the parent.
    pub width_fraction: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Absolute pixels; SIZE_FIT means size to content. Takes precedence over height_fraction.
    pub height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Proportion of the parent height, where 1.0 fills the parent.
    pub height_fraction: Option<f32>,
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
    tree: Option<Arc<HostRenderTree>>,
    navigation: Vec<HostRoute>,
    storage: HashMap<String, Value>,
    channels: HashMap<String, VecDeque<Value>>,
    render_count: u64,
    last_event: Option<HostEventRecord>,
}

/// Immutable, shared tree for foreground rendering without a full host snapshot.
#[derive(Debug)]
pub struct HostRenderTree {
    root: HostNode,
}

impl HostRenderTree {
    pub fn root(&self) -> &HostNode {
        &self.root
    }
}

impl Drop for HostRenderTree {
    fn drop(&mut self) {
        drop_host_tree(std::mem::take(&mut self.root));
    }
}

fn drop_host_tree(tree: HostNode) {
    let mut pending = vec![tree];
    while let Some(mut node) = pending.pop() {
        drop_host_value(std::mem::take(&mut node.props));
        pending.append(&mut node.children);
    }
}

impl Drop for HostStateInner {
    fn drop(&mut self) {
        for (_, value) in self.storage.drain() {
            drop_host_value(value);
        }
        for (_, queue) in self.channels.drain() {
            for value in queue {
                drop_host_value(value);
            }
        }
        for route in self.navigation.drain(..) {
            drop_host_value(route.params);
        }
        if let Some(event) = self.last_event.take() {
            drop_host_value(event.payload);
        }
    }
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
        let tree = Arc::new(HostRenderTree { root: tree });
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let previous = inner.tree.replace(tree);
        inner.render_count += 1;
        if inner.navigation.is_empty() {
            inner.navigation.push(HostRoute {
                name: "root".to_string(),
                params: json!({}),
            });
        }
        let snapshot = snapshot_from_inner(&inner);
        drop(inner);
        drop(previous);
        snapshot
    }

    pub fn snapshot(&self) -> HostSnapshot {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        snapshot_from_inner(&inner)
    }

    /// Share the current normalized tree without cloning nodes or enumerating storage.
    pub fn render_snapshot(&self) -> Option<Arc<HostRenderTree>> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .tree
            .clone()
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
            let previous = std::mem::replace(last, route);
            drop_host_value(previous.params);
        }
        snapshot_from_inner(&inner)
    }

    pub fn navigation_pop(&self) -> HostSnapshot {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        ensure_root_route(&mut inner);
        if inner.navigation.len() > 1 {
            if let Some(previous) = inner.navigation.pop() {
                drop_host_value(previous.params);
            }
        }
        snapshot_from_inner(&inner)
    }

    pub fn storage_get(&self, key: &str) -> Option<Value> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.storage.get(key).map(clone_host_value)
    }

    pub fn storage_set(&self, key: impl Into<String>, value: Value) -> HostSnapshot {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(previous) = inner.storage.insert(key.into(), value) {
            drop_host_value(previous);
        }
        snapshot_from_inner(&inner)
    }

    pub fn storage_remove(&self, key: &str) -> HostSnapshot {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(previous) = inner.storage.remove(key) {
            drop_host_value(previous);
        }
        snapshot_from_inner(&inner)
    }

    pub fn record_event(&self, event: HostEventRecord) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(previous) = inner.last_event.replace(event) {
            drop_host_value(previous.payload);
        }
    }

    pub fn channel_send(&self, channel: impl Into<String>, message: Value) -> HostSnapshot {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner
            .channels
            .entry(channel.into())
            .or_default()
            .push_back(message);
        snapshot_from_inner(&inner)
    }

    pub fn channel_poll(&self, channel: &str) -> Option<Value> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let queue = inner.channels.get_mut(channel)?;
        let message = queue.pop_front();
        if queue.is_empty() {
            inner.channels.remove(channel);
        }
        message
    }
}

fn normalize_host_tree_classes(node: &mut HostNode) {
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
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
        for class in classes.into_iter().rev() {
            apply_host_class(class, &mut node.style);
        }
        pending.extend(node.children.iter_mut());
    }
}

fn apply_host_class(class: &str, style: &mut HostStyle) {
    match class {
        "italic" => {
            style.italic.get_or_insert(true);
        }
        "not-italic" => {
            style.italic.get_or_insert(false);
        }
        "underline" => {
            style.underline.get_or_insert(true);
        }
        "line-through" => {
            style.strikethrough.get_or_insert(true);
        }
        "no-underline" => {
            style.underline.get_or_insert(false);
            style.strikethrough.get_or_insert(false);
        }
        "grid" => {
            style.display.get_or_insert_with(|| "grid".to_owned());
        }
        "block" => {
            style.display.get_or_insert_with(|| "block".to_owned());
        }
        "hidden" => {
            style.display.get_or_insert_with(|| "none".to_owned());
        }
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
        "items-stretch" => {
            style
                .align_items
                .get_or_insert_with(|| "stretch".to_owned());
        }
        "justify-between" => {
            style
                .justify_content
                .get_or_insert_with(|| "between".to_owned());
        }
        "justify-around" => {
            style
                .justify_content
                .get_or_insert_with(|| "around".to_owned());
        }
        "justify-evenly" => {
            style
                .justify_content
                .get_or_insert_with(|| "evenly".to_owned());
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
            apply_host_dimension("full", &mut style.width, &mut style.width_fraction);
        }
        "h-full" => {
            apply_host_dimension("full", &mut style.height, &mut style.height_fraction);
        }
        "border" | "border-1" => {
            style.border_width.get_or_insert(1.0);
        }
        _ => {
            apply_host_parametric_class(class, style);
        }
    }
}

fn apply_host_parametric_class(class: &str, style: &mut HostStyle) {
    let shadow = if class == "shadow" {
        Some("sm")
    } else {
        class.strip_prefix("shadow-")
    };
    if let Some(shadow) = shadow.filter(|value| {
        matches!(
            *value,
            "none" | "2xs" | "xs" | "sm" | "md" | "lg" | "xl" | "2xl"
        )
    }) {
        style.shadow.get_or_insert_with(|| shadow.to_owned());
        return;
    }
    for (prefix, field) in [
        ("grid-cols-", &mut style.grid_cols),
        ("grid-rows-", &mut style.grid_rows),
    ] {
        if let Some(count) = class
            .strip_prefix(prefix)
            .and_then(|value| value.parse::<u16>().ok())
            .filter(|count| *count > 0)
        {
            field.get_or_insert(count);
            return;
        }
    }
    for (prefix, field) in [
        ("overflow-x-", &mut style.overflow_x),
        ("overflow-y-", &mut style.overflow_y),
        ("overflow-", &mut style.overflow),
    ] {
        if let Some(value) = class.strip_prefix(prefix) {
            if matches!(value, "visible" | "hidden" | "clip" | "scroll" | "auto") {
                field.get_or_insert_with(|| value.to_owned());
                return;
            }
        }
    }
    if let Some(align) = class.strip_prefix("text-") {
        if matches!(align, "left" | "center" | "right") {
            style.text_align.get_or_insert_with(|| align.to_owned());
            return;
        }
    }
    if let Some(weight) = class.strip_prefix("font-") {
        if matches!(
            weight,
            "thin"
                | "extralight"
                | "light"
                | "normal"
                | "medium"
                | "semibold"
                | "bold"
                | "extrabold"
                | "black"
        ) {
            style.font_weight.get_or_insert_with(|| weight.to_owned());
            return;
        }
    }
    if let Some(opacity) = class
        .strip_prefix("opacity-")
        .and_then(parse_opacity_percentage)
    {
        style.opacity.get_or_insert(opacity);
        return;
    }
    if let Some(value) = class
        .strip_prefix("p-")
        .and_then(parse_spacing_pt)
        .filter(|value| *value >= 0.0)
    {
        style.padding.get_or_insert(value);
        return;
    }
    if let Some(value) = class
        .strip_prefix("px-")
        .and_then(parse_spacing_pt)
        .filter(|value| *value >= 0.0)
    {
        style.padding_x.get_or_insert(value);
        return;
    }
    if let Some(value) = class
        .strip_prefix("py-")
        .and_then(parse_spacing_pt)
        .filter(|value| *value >= 0.0)
    {
        style.padding_y.get_or_insert(value);
        return;
    }
    if let Some(value) = class
        .strip_prefix("gap-")
        .and_then(parse_spacing_pt)
        .filter(|gap| *gap >= 0.0)
    {
        style.gap.get_or_insert(value);
        return;
    }
    if let Some(token) = class.strip_prefix("w-") {
        apply_host_dimension(token, &mut style.width, &mut style.width_fraction);
        return;
    }
    if let Some(token) = class.strip_prefix("h-") {
        apply_host_dimension(token, &mut style.height, &mut style.height_fraction);
        return;
    }
    let radius = class
        .strip_prefix("rounded-")
        .and_then(|token| {
            if token.starts_with('[') {
                parse_spacing_pt(token)
            } else {
                parse_radius_px(token).map(f32::from)
            }
        })
        .filter(|value| value.is_finite() && *value >= 0.0);
    if let Some(value) = radius {
        style.radius.get_or_insert(value);
        return;
    }
    if class == "rounded" {
        if let Some(value) = parse_radius_px("") {
            style.radius.get_or_insert(f32::from(value));
        }
        return;
    }
    let font_size = class
        .strip_prefix("text-")
        .and_then(|token| {
            parse_font_size_named(token).or_else(|| {
                if token.starts_with('[') {
                    parse_spacing_pt(token)
                } else {
                    parse_text_size_px(token).map(f32::from)
                }
            })
        })
        .filter(|value| value.is_finite() && *value > 0.0);
    if let Some(value) = font_size {
        style.font_size.get_or_insert(value);
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
    if let Some(token) = class.strip_prefix("border-") {
        let width = if token.starts_with('[') {
            parse_spacing_pt(token)
        } else {
            token.parse::<f32>().ok()
        };
        if let Some(width) = width.filter(|width| width.is_finite() && *width >= 0.0) {
            style.border_width.get_or_insert(width);
            return;
        }
    }
    if let Some(color) = class.strip_prefix("border-").and_then(parse_host_color) {
        style.border_color.get_or_insert(color);
    }
}

fn apply_host_dimension(raw: &str, pixels: &mut Option<f32>, fraction: &mut Option<f32>) {
    if pixels.is_some() || fraction.is_some() {
        return;
    }
    match parse_size_width_height(raw) {
        Some(SizeToken::Full) => *fraction = Some(1.0),
        Some(SizeToken::Fraction { num, den }) => *fraction = Some(f32::from(num) / f32::from(den)),
        Some(SizeToken::Px(value) | SizeToken::Spacing(value)) => *pixels = Some(f32::from(value)),
        Some(SizeToken::Auto) => *pixels = Some(crepuscularity_core::tailwind::SIZE_FIT),
        None => {
            if let Some(value) = parse_spacing_pt(raw).filter(|value| *value >= 0.0) {
                *pixels = Some(value);
            }
        }
    }
}

fn parse_host_color(raw: &str) -> Option<String> {
    let mut color = raw.trim();
    while let Some(inner) = color.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        color = inner.trim();
    }
    resolve_css_color(color)
}

fn snapshot_from_inner(inner: &HostStateInner) -> HostSnapshot {
    let mut storage_keys = inner.storage.keys().cloned().collect::<Vec<_>>();
    storage_keys.sort();
    HostSnapshot {
        tree: inner.tree.as_ref().map(|tree| tree.root.clone()),
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
    fn render_snapshots_share_normalized_trees_and_keep_replaced_versions_stable() {
        let state = HostState::shared();
        assert!(state.render_snapshot().is_none());
        state.render_tree(HostNode {
            text: Some("first".into()),
            class_name: Some("p-2".into()),
            ..HostNode::default()
        });
        let first = state.render_snapshot().unwrap();
        assert_eq!(first.root().style.padding, Some(8.0));
        state.storage_set("key", json!("value"));
        let same = state.render_snapshot().unwrap();
        assert!(Arc::ptr_eq(&first, &same));
        let mut full = state.snapshot();
        full.tree.as_mut().unwrap().text = Some("caller mutation".into());
        assert_eq!(first.root().text.as_deref(), Some("first"));
        assert_eq!(full.storage_keys, ["key"]);

        let worker_state = Arc::clone(&state);
        std::thread::spawn(move || {
            worker_state.render_tree(HostNode {
                text: Some("second".into()),
                ..HostNode::default()
            });
        })
        .join()
        .unwrap();
        let second = state.render_snapshot().unwrap();
        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(second.root().text.as_deref(), Some("second"));
        assert_eq!(first.root().text.as_deref(), Some("first"));
        assert_eq!(state.snapshot().tree.as_ref(), Some(second.root()));
        drop(state);
        assert_eq!(second.root().text.as_deref(), Some("second"));
    }

    #[test]
    fn retained_render_snapshot_drops_deep_tree_on_a_small_stack() {
        let mut tree = HostNode::default();
        for _ in 0..20_000 {
            tree = HostNode {
                children: vec![tree],
                ..HostNode::default()
            };
        }
        let state = HostState::default();
        drop_host_tree(state.render_tree(tree).tree.unwrap());
        let snapshot = state.render_snapshot().unwrap();
        drop(state);
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(move || drop(snapshot))
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn deep_navigation_and_event_payloads_survive_managed_lifecycle() {
        fn deep_value() -> Value {
            let mut value = json!({"kept": true});
            for _ in 0..20_000 {
                value = Value::Array(vec![value]);
            }
            value
        }
        fn dispose(snapshot: HostSnapshot) {
            if let Some(tree) = snapshot.tree {
                drop_host_tree(tree);
            }
            for route in snapshot.navigation {
                drop_host_value(route.params);
            }
            if let Some(event) = snapshot.last_event {
                drop_host_value(event.payload);
            }
        }
        let state = HostState::default();
        dispose(state.navigation_push(HostRoute {
            name: "first".to_owned(),
            params: deep_value(),
        }));
        let replaced = state.navigation_replace(HostRoute {
            name: "second".to_owned(),
            params: deep_value(),
        });
        assert_eq!(replaced.navigation.last().unwrap().name, "second");
        dispose(replaced);
        let popped = state.navigation_pop();
        assert_eq!(popped.navigation.len(), 1);
        dispose(popped);
        dispose(state.navigation_push(HostRoute {
            name: "remaining".to_owned(),
            params: deep_value(),
        }));
        for handler in ["first", "second"] {
            state.record_event(HostEventRecord {
                kind: "press".to_owned(),
                handler_id: handler.to_owned(),
                payload: deep_value(),
            });
            let snapshot = state.snapshot();
            assert_eq!(snapshot.last_event.as_ref().unwrap().handler_id, handler);
            let mut value = &snapshot.last_event.as_ref().unwrap().payload;
            let mut depth = 0;
            while let Value::Array(values) = value {
                value = &values[0];
                depth += 1;
            }
            assert_eq!(depth, 20_000);
            assert_eq!(value, &json!({"kept": true}));
            dispose(snapshot);
        }
        drop(state);
    }

    #[test]
    fn drained_channels_release_entries_and_can_be_reused() {
        let state = HostState::default();
        for index in 0..1000 {
            let channel = format!("request-{index}");
            state.channel_send(&channel, json!(index));
            assert_eq!(state.channel_poll(&channel), Some(json!(index)));
        }
        assert!(state.inner.lock().unwrap().channels.is_empty());
        state.channel_send("reused", json!("first"));
        state.channel_send("reused", json!("second"));
        assert_eq!(state.channel_poll("reused"), Some(json!("first")));
        assert_eq!(state.inner.lock().unwrap().channels.len(), 1);
        assert_eq!(state.channel_poll("reused"), Some(json!("second")));
        assert!(state.inner.lock().unwrap().channels.is_empty());
        state.channel_send("reused", Value::Null);
        assert_eq!(state.channel_poll("reused"), Some(Value::Null));
        assert_eq!(state.channel_poll("reused"), None);
        assert!(state.inner.lock().unwrap().channels.is_empty());
    }

    #[test]
    fn channel_poll_preserves_fifo_order_and_empty_behavior() {
        let state = HostState::default();
        for index in 0..1000 {
            state.channel_send("queue", json!(index));
        }
        for index in 0..1000 {
            assert_eq!(state.channel_poll("queue"), Some(json!(index)));
        }
        assert!(state.channel_poll("queue").is_none());
        assert!(state.channel_poll("missing").is_none());
    }

    #[test]
    fn queued_deep_messages_can_be_polled_and_state_dropped() {
        fn deep_value() -> Value {
            let mut value = json!("message");
            for _ in 0..20_000 {
                value = Value::Array(vec![value]);
            }
            value
        }
        let state = HostState::default();
        state.channel_send("queue", deep_value());
        state.channel_send("queue", deep_value());
        let value = state.channel_poll("queue").unwrap();
        let mut current = &value;
        let mut depth = 0;
        while let Value::Array(values) = current {
            current = &values[0];
            depth += 1;
        }
        assert_eq!(depth, 20_000);
        assert_eq!(current, &json!("message"));
        drop_host_value(value);
        drop(state);
    }

    #[test]
    fn storage_handles_deep_values_across_get_replace_remove_and_shutdown() {
        fn deep_value() -> Value {
            let mut value = json!({"leaf": "kept"});
            for _ in 0..20_000 {
                value = Value::Array(vec![value]);
            }
            value
        }
        let state = HostState::default();
        state.storage_set("key", deep_value());
        let cloned = state.storage_get("key").unwrap();
        let mut current = &cloned;
        let mut depth = 0;
        while let Value::Array(values) = current {
            current = &values[0];
            depth += 1;
        }
        assert_eq!(depth, 20_000);
        assert_eq!(current, &json!({"leaf": "kept"}));
        drop_host_value(cloned);
        state.storage_set("key", deep_value());
        state.storage_remove("key");
        assert!(state.storage_get("key").is_none());
        state.storage_set("key", deep_value());
        drop(state);
    }

    #[test]
    fn snapshots_and_managed_replacement_handle_deep_props() {
        let mut props = json!({"leaf": "kept", "values": [1, true, null]});
        for _ in 0..20_000 {
            props = Value::Array(vec![props]);
        }
        let state = HostState::default();
        let snapshot = state.render_tree(HostNode {
            node_type: "View".to_owned(),
            props,
            ..HostNode::default()
        });
        let tree = snapshot.tree.unwrap();
        let mut props = &tree.props;
        let mut depth = 0;
        while let Value::Array(values) = props {
            props = &values[0];
            depth += 1;
        }
        assert_eq!(depth, 20_000);
        assert_eq!(props, &json!({"leaf": "kept", "values": [1, true, null]}));
        drop_host_tree(tree);
        state.render_tree(HostNode::default());
        drop(state);
    }

    #[test]
    fn managed_deep_trees_can_be_replaced_and_state_dropped() {
        fn deep_tree(label: &str) -> HostNode {
            let mut node = HostNode {
                node_type: "Text".to_owned(),
                text: Some(label.to_owned()),
                ..HostNode::default()
            };
            for _ in 0..20_000 {
                node = HostNode {
                    node_type: "View".to_owned(),
                    children: vec![node],
                    ..HostNode::default()
                };
            }
            node
        }
        let state = HostState::default();
        for (index, label) in ["first", "second"].into_iter().enumerate() {
            let snapshot = state.render_tree(deep_tree(label));
            assert_eq!(snapshot.render_count, index as u64 + 1);
            let mut tree = snapshot.tree.unwrap();
            let mut depth = 0;
            while let Some(child) = tree.children.pop() {
                tree = child;
                depth += 1;
            }
            assert_eq!(depth, 20_000);
            assert_eq!(tree.text.as_deref(), Some(label));
        }
        drop(state);
    }

    #[test]
    fn snapshots_clone_deep_child_trees_without_recursion() {
        let mut node = HostNode {
            node_type: "Text".to_owned(),
            text: Some("Leaf".to_owned()),
            ..HostNode::default()
        };
        for _ in 0..20_000 {
            node = HostNode {
                node_type: "View".to_owned(),
                class_name: Some("p-1.5".to_owned()),
                children: vec![node],
                ..HostNode::default()
            };
        }
        let state = HostState::default();
        let first = state.render_tree(node);
        let second = state.snapshot();
        let stored = state.inner.lock().unwrap().tree.take().unwrap();
        let stored = std::mem::take(&mut Arc::try_unwrap(stored).unwrap().root);
        for mut tree in [first.tree.unwrap(), second.tree.unwrap(), stored] {
            let mut depth = 0;
            while let Some(child) = tree.children.pop() {
                assert_eq!(tree.style.padding, Some(6.0));
                tree = child;
                depth += 1;
            }
            assert_eq!(depth, 20_000);
            assert_eq!(tree.text.as_deref(), Some("Leaf"));
        }
    }

    #[test]
    fn iterative_clone_preserves_node_metadata_and_child_order() {
        let node: HostNode = serde_json::from_value(json!({"type": "Button", "id": "id", "title": "Title", "text": "Text", "value": "Value", "placeholder": "Placeholder", "className": "p-2", "route": "route", "on_press": "press", "on_key_down": "key", "props": {"nested": [1, 2]}, "style": {"opacity": 0.25}, "children": [{"type": "Text", "text": "First"}, {"type": "Text", "text": "Second"}]})).unwrap();
        assert_eq!(node.clone(), node);
    }

    #[test]
    fn class_normalization_walks_deep_trees_without_recursion() {
        let mut node = HostNode {
            class_name: Some("p-1 p-1.5".to_owned()),
            ..HostNode::default()
        };
        for _ in 0..20_000 {
            node = HostNode {
                class_name: Some("p-1 p-1.5".to_owned()),
                children: vec![node],
                ..HostNode::default()
            };
        }
        normalize_host_tree_classes(&mut node);
        let mut count = 0;
        loop {
            assert_eq!(node.style.padding, Some(6.0));
            count += 1;
            match node.children.pop() {
                Some(child) => node = child,
                None => break,
            }
        }
        assert_eq!(count, 20_001);
    }

    #[test]
    fn shadow_classes_keep_presets_order_and_explicit_precedence() {
        for (classes, explicit, expected) in [
            ("shadow", None, "sm"),
            ("shadow-sm shadow-lg shadow-unknown", None, "lg"),
            ("shadow-lg shadow-none", None, "none"),
            ("shadow-lg", Some("xs"), "xs"),
        ] {
            let mut value = json!({"type": "View", "className": classes});
            if let Some(shadow) = explicit {
                value["style"] = json!({"shadow": shadow});
            }
            let node: HostNode = serde_json::from_value(value).unwrap();
            assert_eq!(
                HostState::default()
                    .render_tree(node)
                    .tree
                    .unwrap()
                    .style
                    .shadow
                    .as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn grid_classes_keep_positive_counts_and_class_order() {
        let node: HostNode = serde_json::from_value(json!({"type": "View", "className": "grid grid-cols-2 grid-cols-3 grid-cols-0 grid-rows-2 grid-rows-bad"})).unwrap();
        let style = HostState::default().render_tree(node).tree.unwrap().style;
        assert_eq!(style.display.as_deref(), Some("grid"));
        assert_eq!(style.grid_cols, Some(3));
        assert_eq!(style.grid_rows, Some(2));
    }

    #[test]
    fn block_display_respects_class_order() {
        for (classes, expected) in [
            ("flex block", "block"),
            ("block flex", "flex"),
            ("hidden block", "block"),
        ] {
            let tree: HostNode =
                serde_json::from_value(json!({"type": "View", "className": classes})).unwrap();
            assert_eq!(
                HostState::default()
                    .render_tree(tree)
                    .tree
                    .unwrap()
                    .style
                    .display
                    .as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn dimension_styles_round_trip_with_distinct_wire_units() {
        for (classes, explicit, expected) in [
            (
                "w-full h-1/2",
                json!({}),
                json!({"widthFraction": 1.0, "heightFraction": 0.5}),
            ),
            (
                "w-[100px] h-1.75",
                json!({}),
                json!({"width": 100.0, "height": 7.0}),
            ),
            ("w-full w-4", json!({}), json!({"width": 16.0})),
            ("w-4 w-full", json!({}), json!({"widthFraction": 1.0})),
            (
                "w-full h-full",
                json!({"width": 100.0, "heightFraction": 0.25}),
                json!({"width": 100.0, "heightFraction": 0.25}),
            ),
        ] {
            let node: HostNode = serde_json::from_value(
                json!({"type": "View", "className": classes, "style": explicit}),
            )
            .unwrap();
            let style = HostState::default().render_tree(node).tree.unwrap().style;
            let encoded = serde_json::to_value(&style).unwrap();
            assert_eq!(encoded, expected, "{classes}");
            let decoded: HostStyle = serde_json::from_value(encoded).unwrap();
            assert_eq!(decoded, style);
        }
    }

    #[test]
    fn fractional_fixed_dimensions_keep_pixels_and_reject_invalid_overrides() {
        let tree: HostNode = serde_json::from_value(
            json!({"type": "View", "className": "w-[100.5px] h-1.75 w-[NaNpx] h-[-1px]"}),
        )
        .unwrap();
        let style = HostState::default().render_tree(tree).tree.unwrap().style;
        assert_eq!(style.width, Some(100.5));
        assert_eq!(style.height, Some(7.0));
        assert_eq!(style.width_fraction, None);
        assert_eq!(style.height_fraction, None);
    }

    #[test]
    fn arbitrary_radii_keep_fractional_pixels_and_named_defaults() {
        for (classes, expected) in [
            (
                "rounded-md rounded-[2.5px] rounded-[NaNpx] rounded-[-2px]",
                2.5,
            ),
            ("rounded", 4.0),
            ("rounded-md", 6.0),
            ("rounded-[0px]", 0.0),
        ] {
            let tree: HostNode =
                serde_json::from_value(json!({"type": "View", "className": classes})).unwrap();
            assert_eq!(
                HostState::default()
                    .render_tree(tree)
                    .tree
                    .unwrap()
                    .style
                    .radius,
                Some(expected)
            );
        }
    }

    #[test]
    fn fractional_font_size_classes_stay_distinct_from_color_and_invalid_values() {
        let tree: HostNode = serde_json::from_value(json!({"type": "Text", "className": "text-xl text-[18.5px] text-[NaNpx] text-[infpx] text-[-2px] text-[0px] text-red-500"})).unwrap();
        let style = HostState::default().render_tree(tree).tree.unwrap().style;
        assert_eq!(style.font_size, Some(18.5));
        assert_eq!(style.color.as_deref(), Some("#fb2c36"));
    }

    #[test]
    fn padding_classes_keep_fractional_values_and_skip_invalid_overrides() {
        let tree: HostNode = serde_json::from_value(json!({"type": "View", "className": "p-[2.5px] px-[3.5px] py-1.75 p-NaN px-[-2px] py-inf"})).unwrap();
        let style = HostState::default().render_tree(tree).tree.unwrap().style;
        assert_eq!(style.padding, Some(2.5));
        assert_eq!(style.padding_x, Some(3.5));
        assert_eq!(style.padding_y, Some(7.0));
    }

    #[test]
    fn host_color_classes_preserve_css_alpha() {
        for (color, expected) in [
            ("[#1234]", "#11223344"),
            ("[#ff000080]", "#ff000080"),
            ("red-500/50", "#fb2c3680"),
            ("transparent", "#00000000"),
        ] {
            assert_eq!(parse_host_color(color).as_deref(), Some(expected));
        }
        let tree: HostNode = serde_json::from_value(
            json!({"type": "Text", "className": "bg-[#1234] text-red-500/50 border-[#f008]"}),
        )
        .unwrap();
        let style = HostState::default().render_tree(tree).tree.unwrap().style;
        assert_eq!(style.background.as_deref(), Some("#11223344"));
        assert_eq!(style.color.as_deref(), Some("#fb2c3680"));
        assert_eq!(style.border_color.as_deref(), Some("#ff000088"));
    }

    #[test]
    fn text_decoration_classes_keep_order_and_explicit_false_values() {
        for (classes, expected) in [
            ("italic underline line-through", true),
            (
                "italic not-italic underline line-through no-underline",
                false,
            ),
        ] {
            let tree: HostNode =
                serde_json::from_value(json!({"type": "Text", "className": classes})).unwrap();
            let style = HostState::default().render_tree(tree).tree.unwrap().style;
            assert_eq!(style.italic, Some(expected));
            assert_eq!(style.underline, Some(expected));
            assert_eq!(style.strikethrough, Some(expected));
        }
        let tree: HostNode = serde_json::from_value(json!({"type": "Text", "className": "italic underline line-through", "style": {"italic": false, "underline": false, "strikethrough": false}})).unwrap();
        let style = HostState::default().render_tree(tree).tree.unwrap().style;
        assert_eq!(style.italic, Some(false));
        assert_eq!(style.underline, Some(false));
        assert_eq!(style.strikethrough, Some(false));
    }

    #[test]
    fn hidden_classes_keep_order_and_explicit_display_precedence() {
        for (classes, explicit, expected) in [
            ("flex hidden", None, "none"),
            ("hidden flex", None, "flex"),
            ("hidden", Some("flex"), "flex"),
        ] {
            let mut value = json!({"type": "Button", "className": classes, "on_press": "handler"});
            if let Some(display) = explicit {
                value["style"] = json!({"display": display});
            }
            let tree: HostNode = serde_json::from_value(value).unwrap();
            assert_eq!(
                HostState::default()
                    .render_tree(tree)
                    .tree
                    .unwrap()
                    .style
                    .display
                    .as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn overflow_utilities_preserve_general_and_axis_values() {
        let tree: HostNode = serde_json::from_value(json!({"type": "View", "className": "overflow-visible overflow-hidden overflow-x-auto overflow-y-scroll overflow-y-clip"})).unwrap();
        let style = HostState::default().render_tree(tree).tree.unwrap().style;
        assert_eq!(style.overflow.as_deref(), Some("hidden"));
        assert_eq!(style.overflow_x.as_deref(), Some("auto"));
        assert_eq!(style.overflow_y.as_deref(), Some("clip"));
    }

    #[test]
    fn extended_alignment_utilities_keep_class_precedence() {
        let tree: HostNode = serde_json::from_value(json!({"type": "View", "className": "items-center items-stretch justify-between justify-around justify-evenly"})).unwrap();
        let style = HostState::default().render_tree(tree).tree.unwrap().style;
        assert_eq!(style.align_items.as_deref(), Some("stretch"));
        assert_eq!(style.justify_content.as_deref(), Some("evenly"));
    }

    #[test]
    fn alignment_classes_keep_order_and_explicit_precedence() {
        for explicit in [false, true] {
            let mut value = json!({"type": "Text", "className": "text-left text-center text-right text-red-500"});
            if explicit {
                value["style"] = json!({"textAlign": "center"});
            }
            let tree: HostNode = serde_json::from_value(value).unwrap();
            let style = HostState::default().render_tree(tree).tree.unwrap().style;
            assert_eq!(
                style.text_align.as_deref(),
                Some(if explicit { "center" } else { "right" })
            );
            assert_eq!(style.color.as_deref(), Some("#fb2c36"));
        }
    }

    #[test]
    fn font_weight_classes_cover_the_complete_ramp() {
        for weight in [
            "thin",
            "extralight",
            "light",
            "normal",
            "medium",
            "semibold",
            "bold",
            "extrabold",
            "black",
        ] {
            let tree: HostNode = serde_json::from_value(json!({"type": "Text", "className": format!("font-bold font-{weight} font-unknown")})).unwrap();
            assert_eq!(
                HostState::default()
                    .render_tree(tree)
                    .tree
                    .unwrap()
                    .style
                    .font_weight
                    .as_deref(),
                Some(weight)
            );
        }
    }

    #[test]
    fn dimensions_keep_pixels_and_proportions_distinct() {
        for (class, pixels, fraction) in [
            ("w-full", None, Some(1.0)),
            ("w-1/2", None, Some(0.5)),
            ("w-[100px]", Some(100.0), None),
            ("w-auto", Some(-2.0), None),
        ] {
            let tree: HostNode =
                serde_json::from_value(json!({"type": "View", "className": class})).unwrap();
            let style = HostState::default().render_tree(tree).tree.unwrap().style;
            assert_eq!(style.width, pixels);
            assert_eq!(style.width_fraction, fraction);
        }
    }

    #[test]
    fn gap_classes_preserve_fractional_pixels() {
        for (class, expected) in [("gap-0.5", 2.0), ("gap-1.5", 6.0), ("gap-[2.5px]", 2.5)] {
            let tree: HostNode =
                serde_json::from_value(json!({"type": "View", "className": class})).unwrap();
            assert_eq!(
                HostState::default()
                    .render_tree(tree)
                    .tree
                    .unwrap()
                    .style
                    .gap,
                Some(expected)
            );
        }
    }

    #[test]
    fn opacity_classes_are_finite_clamped_and_preserve_explicit_styles() {
        for (class, expected) in [
            ("opacity-12.5", Some(0.125)),
            ("opacity--20", Some(0.0)),
            ("opacity-120", Some(1.0)),
            ("opacity-NaN", None),
            ("opacity-inf", None),
        ] {
            let tree: HostNode =
                serde_json::from_value(json!({"type": "View", "className": class})).unwrap();
            assert_eq!(
                HostState::default()
                    .render_tree(tree)
                    .tree
                    .unwrap()
                    .style
                    .opacity,
                expected
            );
        }
        for explicit in [false, true] {
            let mut value =
                json!({"type": "View", "className": "opacity-25 opacity-50 opacity-NaN"});
            if explicit {
                value["style"] = json!({"opacity": 0.75});
            }
            let tree: HostNode = serde_json::from_value(value).unwrap();
            assert_eq!(
                HostState::default()
                    .render_tree(tree)
                    .tree
                    .unwrap()
                    .style
                    .opacity,
                Some(if explicit { 0.75 } else { 0.5 })
            );
        }
    }

    #[test]
    fn border_width_classes_are_distinct_from_colors_and_keep_precedence() {
        let tree: HostNode = serde_json::from_value(json!({
            "type": "View",
            "className": "border border-2 border-[2.5px] border-NaN border-[-2px] border-red-500"
        }))
        .unwrap();
        let style = HostState::default().render_tree(tree).tree.unwrap().style;
        assert_eq!(style.border_width, Some(2.5));
        assert_eq!(style.border_color.as_deref(), Some("#fb2c36"));
    }

    #[test]
    fn last_class_wins_while_explicit_style_fields_keep_precedence() {
        for explicit in [false, true] {
            let mut value = json!({
                "type": "View",
                "className": "flex-col flex-row p-2 p-4 bg-red-500 bg-blue-500 items-start items-end",
                "children": [{"type": "Text", "className": "text-sm text-xl", "text": "Hello"}]
            });
            if explicit {
                value["style"] =
                    json!({"padding": 3.0, "direction": "column", "background": "#123456"});
            }
            let tree: HostNode = serde_json::from_value(value).unwrap();
            let tree = HostState::default().render_tree(tree).tree.unwrap();
            assert_eq!(tree.style.padding, Some(if explicit { 3.0 } else { 16.0 }));
            assert_eq!(
                tree.style.direction.as_deref(),
                Some(if explicit { "column" } else { "row" })
            );
            assert_eq!(
                tree.style.background.as_deref(),
                Some(if explicit { "#123456" } else { "#2b7fff" })
            );
            assert_eq!(tree.style.align_items.as_deref(), Some("end"));
            assert_eq!(tree.children[0].style.font_size, Some(20.0));
        }
    }

    #[test]
    fn host_color_boundary_rejects_unicode_hex_and_handles_deep_wrappers() {
        for color in ["#aé1234", "#12345é", "#雪123", "0x🙂abc"] {
            assert_eq!(parse_host_color(color), None, "{color}");
        }
        let wrapped = format!("{}#123456{}", "[".repeat(20_000), "]".repeat(20_000));
        assert_eq!(parse_host_color(&wrapped).as_deref(), Some("#123456"));
        let tree: HostNode = serde_json::from_value(json!({
            "type": "Text",
            "className": "text-[#aé1234] bg-[#12345é]",
            "text": "Safe"
        }))
        .unwrap();
        let snapshot = HostState::default().render_tree(tree);
        let style = snapshot.tree.unwrap().style;
        assert_eq!(style.color, None);
        assert_eq!(style.background, None);
    }

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
        assert_eq!(style.width, None);
        assert_eq!(style.width_fraction, Some(1.0));

        let mut style = HostStyle::default();
        apply_host_class("h-full", &mut style);
        assert_eq!(style.height, None);
        assert_eq!(style.height_fraction, Some(1.0));
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
