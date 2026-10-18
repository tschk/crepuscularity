//! JSON-serializable view intermediate representation (platform-neutral).

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Bumped when the JSON schema gains incompatible fields; shells should check `version`.
/// Bumped for the `id` field added to `ViewStyle`.
pub const IR_VERSION: u32 = 7;

/// Root document from parsing + lowering (see `crepuscularity_native::render_template_to_ir`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ViewIr {
    pub version: u32,
    pub root: Vec<ViewNode>,
}

/// Portable layout/theming hints mapped from Tailwind-like classes (see [`crate::style`]).
///
/// **Sizing sentinel values** — `width`, `height`, `max_width`, `max_height`:
/// - `> 0.0`  → absolute points
/// - `-1.0`   → fill parent (`maxWidth/maxHeight: .infinity` in SwiftUI)
/// - `-2.0`   → fit content (`fixedSize()` in SwiftUI)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ViewStyle {
    // ── Source identity ──────────────────────────────────────────────────
    /// Original `#id` from the template, preserved verbatim so web targets can
    /// emit `id` and runtimes can bind to the element by selector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    // ── Source classes ───────────────────────────────────────────────────
    /// Original class tokens from the template, preserved verbatim so web
    /// targets can emit `className` instead of re-deriving CSS from hints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,

    // ── Padding ──────────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_horizontal: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_vertical: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_top: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_bottom: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_left: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding_right: Option<f32>,

    // ── Margin ───────────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_horizontal: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_vertical: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_top: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_bottom: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_left: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_right: Option<f32>,

    // ── Sizing ───────────────────────────────────────────────────────────
    /// Absolute pts, -1.0 = fill, -2.0 = fit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_height: Option<f32>,
    /// Absolute pts or -1.0 for fill
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_height: Option<f32>,
    /// aspect-square → 1.0, aspect-video → 16/9
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<f32>,

    // ── Typography ───────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<String>,
    /// Line-height multiplier (1.0 = none, 1.5 = normal, 2.0 = loose)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f32>,
    /// Letter spacing in points
    #[serde(skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<f32>,
    /// "uppercase" | "lowercase" | "capitalize" | "normal"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_transform: Option<String>,
    /// "sans" | "serif" | "mono"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,

    // ── Color ────────────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foreground_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,

    // ── Border ───────────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_width: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_color: Option<String>,

    // ── Opacity / visibility ─────────────────────────────────────────────
    /// 0.0–1.0
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overflow_hidden: Option<bool>,

    // ── Flex ─────────────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex_grow: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex_shrink: Option<f32>,
    /// "start" | "end" | "center" | "stretch" | "baseline"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_self: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex_wrap: Option<bool>,
    /// "row" | "column" — explicit flex direction
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flex_direction: Option<String>,

    // ── Position & Layering ─────────────────────────────────────────────
    /// "static" | "relative" | "absolute" | "fixed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<f32>,

    // ── Transform ────────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translate_x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translate_y: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_y: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotate: Option<f32>,

    // ── Shadow ────────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_radius: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_offset_x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_offset_y: Option<f32>,

    // ── Text Layout ─────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_overflow: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub white_space: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_clamp: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_select: Option<String>,

    // ── Gradients ───────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_gradient_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_gradient_from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_gradient_to: Option<String>,

    // ── Media ───────────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_fit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_position: Option<String>,
}

impl ViewStyle {
    /// Derived from `PartialEq` + `Default` so a newly added field cannot be
    /// forgotten here and silently change what gets serialized.
    fn is_effectively_empty(&self) -> bool {
        *self == Self::default()
    }

    pub(crate) fn opt(self) -> Option<Self> {
        if self.is_effectively_empty() {
            None
        } else {
            Some(self)
        }
    }
}

/// A node in the platform-neutral tree. Serialized with `kind` for Swift/Kotlin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ViewNode {
    #[serde(rename = "text")]
    Text {
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "if")]
    If {
        condition: String,
        then_children: Vec<ViewNode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        else_children: Option<Vec<ViewNode>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "forEach")]
    ForEach {
        bind: String,
        item_name: String,
        item_body: Vec<ViewNode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "stack")]
    Stack {
        axis: StackAxis,
        #[serde(skip_serializing_if = "Option::is_none")]
        spacing: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        align_items: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        justify_content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_long_press: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
        children: Vec<ViewNode>,
    },
    #[serde(rename = "button")]
    Button {
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_click: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_long_press: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "toggle")]
    Toggle {
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
        checked: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_change: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_long_press: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "checkbox")]
    Checkbox {
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
        checked: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_change: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "slider")]
    Slider {
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bind: Option<String>,
        value: f32,
        min: f32,
        max: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        step: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_change: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "progress")]
    Progress {
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        value: f32,
        max: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "meter")]
    Meter {
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        value: f32,
        min: f32,
        max: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "badge")]
    Badge {
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        tone: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "divider")]
    Divider {
        axis: StackAxis,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "spacer")]
    Spacer {
        #[serde(skip_serializing_if = "Option::is_none")]
        size: Option<f32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "dropzone")]
    Dropzone {
        label: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        accept: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_drop: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
        children: Vec<ViewNode>,
    },
    #[serde(rename = "filePicker")]
    FilePicker {
        label: String,
        #[serde(default)]
        accept: Vec<String>,
        #[serde(default)]
        multiple: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_pick: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "image")]
    Image {
        src: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        alt: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        placeholder: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_long_press: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "link")]
    Link {
        href: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        rel: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
        children: Vec<ViewNode>,
    },
    #[serde(rename = "webView")]
    WebView {
        src: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "scroll")]
    Scroll {
        axis: StackAxis,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
        children: Vec<ViewNode>,
    },
    #[serde(rename = "list")]
    List {
        ordered: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
        children: Vec<ViewNode>,
    },
    #[serde(rename = "listItem")]
    ListItem {
        #[serde(skip_serializing_if = "Option::is_none")]
        on_long_press: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
        children: Vec<ViewNode>,
    },
    #[serde(rename = "slotRotate")]
    SlotRotate {
        phrases: Vec<String>,
        interval_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    /// Single-line or multi-line text entry; maps to SwiftUI `TextField` / `TextEditor`.
    #[serde(rename = "input")]
    Input {
        placeholder: String,
        /// Context field name for the bound string (Swift codegen uses `context.<bind>`).
        bind: String,
        #[serde(default)]
        multiline: bool,
        #[serde(default)]
        secure: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_change: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    /// Segmented / picker control bound to a context string field.
    #[serde(rename = "picker")]
    Picker {
        /// Context field holding the selected option value.
        bind: String,
        options: Vec<PickerOption>,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_change: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
    #[serde(rename = "tabs")]
    Tabs {
        bind: String,
        tabs: Vec<TabItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        on_change: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ViewStyle>,
    },
}

impl ViewNode {
    /// Mutable access to the node's style slot, whatever the variant.
    pub fn style_mut(&mut self) -> &mut Option<ViewStyle> {
        match self {
            ViewNode::Text { style, .. }
            | ViewNode::If { style, .. }
            | ViewNode::ForEach { style, .. }
            | ViewNode::Stack { style, .. }
            | ViewNode::Button { style, .. }
            | ViewNode::Toggle { style, .. }
            | ViewNode::Checkbox { style, .. }
            | ViewNode::Slider { style, .. }
            | ViewNode::Progress { style, .. }
            | ViewNode::Meter { style, .. }
            | ViewNode::Badge { style, .. }
            | ViewNode::Divider { style, .. }
            | ViewNode::Spacer { style, .. }
            | ViewNode::Dropzone { style, .. }
            | ViewNode::FilePicker { style, .. }
            | ViewNode::Image { style, .. }
            | ViewNode::Link { style, .. }
            | ViewNode::WebView { style, .. }
            | ViewNode::Scroll { style, .. }
            | ViewNode::List { style, .. }
            | ViewNode::ListItem { style, .. }
            | ViewNode::SlotRotate { style, .. }
            | ViewNode::Input { style, .. }
            | ViewNode::Picker { style, .. }
            | ViewNode::Tabs { style, .. } => style,
        }
    }

    /// Attach a source `#id` to the node, creating the style slot if needed.
    pub fn set_id(&mut self, id: impl Into<String>) {
        self.style_mut().get_or_insert_with(ViewStyle::default).id = Some(id.into());
    }
}

/// One selectable option in a [`ViewNode::Picker`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct PickerOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct TabItem {
    pub value: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub children: Vec<ViewNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum StackAxis {
    Row,
    Column,
}
