//! Owned, validated Crepuscularity View IR v7 nodes.
//! The compiler resolves templates; dynamic values come from explicit host bindings.
const std = @import("std");
const json = @import("json.zig");
pub const styles = @import("style_ir.zig");
pub const Style = styles.Style;
pub const Axis = styles.Axis;
const string = json.string;
const number = json.number;
const oneOf = json.oneOf;

pub const ir_version: u32 = 7;
pub const zpui_revision = "cca22983e7e8124bb8e650616e3803f217e52b95";
pub const Kind = enum { stack, text, button, scroll, toggle, checkbox, slider, picker, tabs, progress, meter, badge, divider, spacer, image, link, list, listItem, @"if", forEach, input, slotRotate, webView, dropzone, filePicker };
pub const Tone = enum { neutral, info, success, warning, @"error" };
pub const Option = struct { value: []const u8, label: []const u8, children: []const Node = &.{}, icon: ?[]const u8 = null };

pub const Node = struct {
    kind: Kind,
    text: []const u8 = "",
    axis: Axis = .column,
    spacing: ?f32 = null,
    align_items: ?styles.Align = null,
    justify_content: ?styles.Justify = null,
    on_click: ?[]const u8 = null,
    on_change: ?[]const u8 = null,
    on_long_press: ?[]const u8 = null,
    on_drop: ?[]const u8 = null,
    on_pick: ?[]const u8 = null,
    condition: []const u8 = "",
    item_name: []const u8 = "",
    else_children: []const Node = &.{},
    phrases: []const []const u8 = &.{},
    interval_ms: u64 = 0,
    accept: []const []const u8 = &.{},
    multiline: bool = false,
    secure: bool = false,
    multiple: bool = false,
    bind: ?[]const u8 = null,
    checked: bool = false,
    value: f32 = 0,
    min: f32 = 0,
    max: f32 = 1,
    step: ?f32 = null,
    size: ?f32 = null,
    tone: Tone = .neutral,
    src: []const u8 = "",
    placeholder: ?[]const u8 = null,
    href: []const u8 = "",
    target: ?[]const u8 = null,
    rel: ?[]const u8 = null,
    ordered: bool = false,
    options: []const Option = &.{},
    style: Style = .{},
    children: []const Node = &.{},
};

pub const Document = struct {
    arena: std.heap.ArenaAllocator,
    root: []const Node,

    /// Strings and nodes belong to this document, independent of the JSON input.
    pub fn parse(parent: std.mem.Allocator, source: []const u8) !Document {
        if (source.len > 1024 * 1024) return error.DocumentTooLarge;
        const parsed = try std.json.parseFromSlice(std.json.Value, parent, source, .{});
        defer parsed.deinit();
        const value = parsed.value;
        if (value != .object) return error.InvalidDocument;
        const version = value.object.get("version") orelse return error.MissingVersion;
        if (version != .integer or version.integer != ir_version) return error.UnsupportedVersion;
        var fields = value.object.iterator();
        while (fields.next()) |field| {
            if (!oneOf(field.key_ptr.*, &.{ "version", "root" })) return error.UnsupportedDocumentField;
        }
        var arena = std.heap.ArenaAllocator.init(parent);
        errdefer arena.deinit();
        const root = try decodeNodes(arena.allocator(), value.object.get("root") orelse return error.MissingRoot, 1);
        return .{ .arena = arena, .root = root };
    }

    pub fn deinit(self: *Document) void {
        self.arena.deinit();
        self.* = undefined;
    }
};

fn decodeNodes(a: std.mem.Allocator, value: std.json.Value, depth: usize) anyerror![]const Node {
    if (value != .array) return error.InvalidChildren;
    const nodes = try a.alloc(Node, value.array.items.len);
    for (value.array.items, nodes) |item, *node| node.* = try decodeNode(a, item, depth);
    return nodes;
}

fn allowedFields(kind: Kind) []const []const u8 {
    return switch (kind) {
        .stack => &.{ "axis", "children", "spacing", "alignItems", "justifyContent", "onLongPress" },
        .scroll => &.{ "axis", "children" },
        .text => &.{ "content", "bind" },
        .button => &.{ "label", "onClick", "onLongPress" },
        .toggle => &.{ "label", "bind", "checked", "onChange", "onLongPress" },
        .checkbox => &.{ "label", "bind", "checked", "onChange" },
        .slider => &.{ "label", "bind", "value", "min", "max", "step", "onChange" },
        .picker => &.{ "bind", "options", "onChange" },
        .tabs => &.{ "bind", "tabs", "onChange" },
        .progress => &.{ "label", "value", "max" },
        .meter => &.{ "label", "value", "min", "max" },
        .badge => &.{ "label", "tone" },
        .divider => &.{"axis"},
        .spacer => &.{"size"},
        .image => &.{ "src", "alt", "placeholder", "onLongPress" },
        .link => &.{ "href", "target", "rel", "children" },
        .list => &.{ "ordered", "children" },
        .listItem => &.{ "children", "onLongPress" },
        .@"if" => &.{ "condition", "thenChildren", "elseChildren" },
        .forEach => &.{ "bind", "itemName", "itemBody" },
        .input => &.{ "placeholder", "bind", "multiline", "secure", "onChange" },
        .slotRotate => &.{ "phrases", "intervalMs" },
        .webView => &.{"src"},
        .dropzone => &.{ "label", "accept", "onDrop", "children" },
        .filePicker => &.{ "label", "accept", "multiple", "onPick" },
    };
}

fn decodeNode(a: std.mem.Allocator, value: std.json.Value, depth: usize) !Node {
    if (depth > 128) return error.TreeTooDeep;
    if (value != .object) return error.InvalidNode;
    const tag = try string(value.object.get("kind") orelse return error.MissingKind);
    const kind = std.meta.stringToEnum(Kind, tag) orelse return error.UnsupportedNode;
    var node: Node = .{ .kind = kind };
    var fields = value.object.iterator();
    while (fields.next()) |field| {
        const key = field.key_ptr.*;
        if (oneOf(key, &.{ "kind", "style" }) or oneOf(key, allowedFields(kind))) continue;
        if (field.value_ptr.* != .null) return error.UnsupportedNodeField;
    }
    switch (kind) {
        .text => node.text = try json.requiredString(a, value, "content"),
        .button, .toggle, .checkbox, .badge, .dropzone, .filePicker => node.text = try json.requiredString(a, value, "label"),
        .slider, .progress, .meter => node.text = try json.ownedString(a, value, "label") orelse "",
        .image, .webView => {
            node.src = try json.requiredString(a, value, "src");
            if (node.src.len == 0) return error.EmptyImageSource;
            node.text = try json.ownedString(a, value, "alt") orelse "";
            node.placeholder = try json.ownedString(a, value, "placeholder");
        },
        .link => {
            node.href = try json.requiredString(a, value, "href");
            if (node.href.len == 0) return error.EmptyLink;
            node.target = try json.ownedString(a, value, "target");
            node.rel = try json.ownedString(a, value, "rel");
        },
        else => {},
    }
    if (oneOf(tag, &.{ "stack", "scroll", "divider" })) {
        node.axis = try json.enumValue(Axis, value.object.get("axis") orelse return error.MissingAxis);
    }
    if (oneOf(tag, &.{ "stack", "scroll", "link", "list", "listItem", "dropzone" })) {
        node.children = try decodeNodes(a, value.object.get("children") orelse return error.MissingChildren, depth + 1);
    }
    if (kind == .stack) {
        if (json.optional(value, "spacing")) |v| {
            node.spacing = try number(v);
            if (node.spacing.? < 0) return error.InvalidSpacing;
        }
        if (json.optional(value, "alignItems")) |v| node.align_items = try json.enumValue(styles.Align, v);
        if (json.optional(value, "justifyContent")) |v| node.justify_content = try json.enumValue(styles.Justify, v);
    }
    node.on_click = try handler(a, value, "onClick");
    node.on_change = try handler(a, value, "onChange");
    node.on_long_press = try handler(a, value, "onLongPress");
    node.on_drop = try handler(a, value, "onDrop");
    node.on_pick = try handler(a, value, "onPick");
    node.bind = try handler(a, value, "bind");
    if ((kind == .picker or kind == .tabs or kind == .input or kind == .forEach) and node.bind == null) return error.MissingBinding;
    if (kind == .input) {
        node.placeholder = try json.requiredString(a, value, "placeholder");
        if (json.optional(value, "multiline")) |v| node.multiline = try json.boolean(v);
        if (json.optional(value, "secure")) |v| node.secure = try json.boolean(v);
    }
    if (kind == .@"if") {
        node.condition = try json.requiredString(a, value, "condition");
        if (node.condition.len == 0) return error.EmptyCondition;
        node.children = try decodeNodes(a, value.object.get("thenChildren") orelse return error.MissingChildren, depth + 1);
        if (json.optional(value, "elseChildren")) |v| node.else_children = try decodeNodes(a, v, depth + 1);
    }
    if (kind == .forEach) {
        node.item_name = try json.requiredString(a, value, "itemName");
        if (node.item_name.len == 0) return error.EmptyItemName;
        node.children = try decodeNodes(a, value.object.get("itemBody") orelse return error.MissingChildren, depth + 1);
    }
    if (kind == .slotRotate) {
        node.phrases = try strings(a, value.object.get("phrases") orelse return error.MissingPhrases);
        if (node.phrases.len == 0) return error.EmptyPhrases;
        const interval = value.object.get("intervalMs") orelse return error.MissingInterval;
        if (interval != .integer or interval.integer <= 0 or interval.integer > std.math.maxInt(u64) / std.time.ns_per_ms) return error.InvalidInterval;
        node.interval_ms = @intCast(interval.integer);
    }
    if (kind == .dropzone) {
        if (try json.ownedString(a, value, "accept")) |filter| {
            var filters: std.ArrayList([]const u8) = .empty;
            var parts = std.mem.splitScalar(u8, filter, ',');
            while (parts.next()) |part| {
                const trimmed = std.mem.trim(u8, part, " \t\r\n");
                if (trimmed.len > 0) try filters.append(a, trimmed);
            }
            if (filter.len > 0 and filters.items.len == 0) return error.InvalidAccept;
            node.accept = filters.items;
        }
    }
    if (kind == .filePicker) {
        if (json.optional(value, "accept")) |v| node.accept = try strings(a, v);
        if (json.optional(value, "multiple")) |v| node.multiple = try json.boolean(v);
    }
    if (kind == .toggle or kind == .checkbox) node.checked = try json.boolean(value.object.get("checked") orelse return error.MissingChecked);
    if (kind == .slider or kind == .progress or kind == .meter) {
        node.value = try number(value.object.get("value") orelse return error.MissingValue);
        node.max = try number(value.object.get("max") orelse return error.MissingMaximum);
        if (kind != .progress) node.min = try number(value.object.get("min") orelse return error.MissingMinimum);
        if (node.max <= node.min or node.value < node.min or node.value > node.max) return error.InvalidRange;
        if (json.optional(value, "step")) |v| {
            node.step = try number(v);
            if (node.step.? <= 0) return error.InvalidStep;
        }
    }
    if (kind == .badge) {
        if (json.optional(value, "tone")) |v| node.tone = try json.enumValue(Tone, v);
    }
    if (kind == .spacer) {
        if (json.optional(value, "size")) |v| {
            node.size = try number(v);
            if (node.size.? < 0) return error.InvalidSize;
        }
    }
    if (kind == .list) node.ordered = try json.boolean(value.object.get("ordered") orelse return error.MissingOrdered);
    if (kind == .picker or kind == .tabs) {
        const key: []const u8 = if (kind == .picker) "options" else "tabs";
        node.options = try decodeOptions(a, value.object.get(key) orelse return error.MissingOptions, kind == .tabs, depth);
    }
    if (json.optional(value, "style")) |v| node.style = try styles.decode(a, v);
    if ((node.style.objectFit != null or node.style.objectPosition != null) and kind != .image) return error.ObjectFitRequiresImage;
    return node;
}

fn strings(a: std.mem.Allocator, value: std.json.Value) ![]const []const u8 {
    if (value != .array) return error.InvalidStringArray;
    const values = try a.alloc([]const u8, value.array.items.len);
    for (value.array.items, values) |item, *owned| owned.* = try a.dupe(u8, try string(item));
    return values;
}

fn handler(a: std.mem.Allocator, object: std.json.Value, key: []const u8) !?[]const u8 {
    const name = try json.ownedString(a, object, key) orelse return null;
    if (name.len == 0) return error.EmptyHandler;
    return name;
}

fn decodeOptions(a: std.mem.Allocator, value: std.json.Value, tabs: bool, depth: usize) ![]const Option {
    if (value != .array or value.array.items.len == 0 or value.array.items.len > std.math.maxInt(u32)) return error.InvalidOptions;
    const options = try a.alloc(Option, value.array.items.len);
    for (value.array.items, options, 0..) |item, *option, index| {
        if (item != .object) return error.InvalidOption;
        var fields = item.object.iterator();
        while (fields.next()) |field| {
            if (oneOf(field.key_ptr.*, &.{ "value", "label" })) continue;
            if (tabs and oneOf(field.key_ptr.*, &.{ "children", "icon" })) continue;
            if (field.value_ptr.* != .null) return error.UnsupportedOptionField;
        }
        option.* = .{ .value = try json.requiredString(a, item, "value"), .label = try json.requiredString(a, item, "label") };
        for (options[0..index]) |previous| if (std.mem.eql(u8, previous.value, option.value)) return error.DuplicateOption;
        if (tabs) option.icon = try json.ownedString(a, item, "icon");
        if (tabs) option.children = try decodeNodes(a, item.object.get("children") orelse return error.MissingChildren, depth + 1);
    }
    return options;
}

test "canonical Rust golden fixture has an array of roots and resolved style hints" {
    var document = try Document.parse(std.testing.allocator, @embedFile("hello.json"));
    defer document.deinit();
    try std.testing.expectEqual(@as(usize, 1), document.root.len);
    const stack = document.root[0];
    try std.testing.expectEqual(Kind.stack, stack.kind);
    try std.testing.expectEqual(Axis.column, stack.axis);
    try std.testing.expectEqual(@as(?f32, 8), stack.spacing);
    try std.testing.expectEqual(@as(?f32, 14), stack.children[0].style.fontSize);
    try std.testing.expectEqualStrings("Hi", stack.children[0].text);
}

test "multiple roots and escaped Unicode text own their storage" {
    const source = try std.testing.allocator.dupe(u8, "{\"version\":7,\"root\":[{\"kind\":\"text\",\"content\":\"Ada \\uD83E\\uDD80\"},{\"kind\":\"button\",\"label\":\"Save\",\"onClick\":\"save\"}]}");
    var document = Document.parse(std.testing.allocator, source) catch |err| {
        std.testing.allocator.free(source);
        return err;
    };
    std.testing.allocator.free(source);
    defer document.deinit();
    try std.testing.expectEqual(@as(usize, 2), document.root.len);
    try std.testing.expectEqualStrings("Ada 🦀", document.root[0].text);
    try std.testing.expectEqualStrings("save", document.root[1].on_click.?);
}

test "reject version mismatch and non-array roots" {
    // Real output from installed crepus 0.11.0; do not relabel old IR as v7.
    try std.testing.expectError(error.UnsupportedVersion, Document.parse(std.testing.allocator, @embedFile("legacy-v5.json")));
    try std.testing.expectError(error.UnsupportedVersion, Document.parse(std.testing.allocator, "{\"version\":8,\"root\":[]}"));
    try std.testing.expectError(error.InvalidChildren, Document.parse(std.testing.allocator, "{\"version\":7,\"root\":{}}"));
}

test "reject unsupported controls and behavior instead of dropping them" {
    try std.testing.expectError(error.MissingBinding, Document.parse(std.testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"input\"}]}"));
    try std.testing.expectError(error.UnsupportedNodeField, Document.parse(std.testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"button\",\"label\":\"Save\",\"onHover\":\"save\"}]}"));
    try std.testing.expectError(error.UnsupportedStyle, Document.parse(std.testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"text\",\"content\":\"Hi\",\"style\":{\"rotate\":45}}]}"));
}

test "scroll axis and fill/fit sizing are preserved" {
    var document = try Document.parse(std.testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"scroll\",\"axis\":\"row\",\"style\":{\"width\":-1,\"height\":-2},\"children\":[]}]}");
    defer document.deinit();
    try std.testing.expectEqual(Axis.row, document.root[0].axis);
    try std.testing.expectEqual(@as(?f32, -1), document.root[0].style.width);
    try std.testing.expectEqual(@as(?f32, -2), document.root[0].style.height);
}

test "invalid sizes, colors, opacity and handler types fail with cleanup" {
    try std.testing.expectError(error.InvalidSize, Document.parse(std.testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"text\",\"content\":\"Hi\",\"style\":{\"width\":-3}}]}"));
    try std.testing.expectError(error.UnsupportedColor, Document.parse(std.testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"text\",\"content\":\"Hi\",\"style\":{\"backgroundColor\":\"red\"}}]}"));
    try std.testing.expectError(error.InvalidOpacity, Document.parse(std.testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"text\",\"content\":\"Hi\",\"style\":{\"opacity\":2}}]}"));
    try std.testing.expectError(error.InvalidString, Document.parse(std.testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"button\",\"label\":\"Save\",\"onClick\":1}]}"));
}

test "document size limit is checked before JSON allocation" {
    const source = try std.testing.allocator.alloc(u8, 1024 * 1024 + 1);
    defer std.testing.allocator.free(source);
    @memset(source, ' ');
    try std.testing.expectError(error.DocumentTooLarge, Document.parse(std.testing.allocator, source));
}

test "deep tree rejection frees partially decoded nodes" {
    const a = std.testing.allocator;
    var source: std.ArrayList(u8) = .empty;
    defer source.deinit(a);
    try source.appendSlice(a, "{\"version\":7,\"root\":[");
    for (0..130) |_| try source.appendSlice(a, "{\"kind\":\"stack\",\"axis\":\"column\",\"children\":[");
    try source.appendSlice(a, "{\"kind\":\"text\",\"content\":\"Leaf\"}");
    for (0..130) |_| try source.appendSlice(a, "]}");
    try source.appendSlice(a, "]}");
    try std.testing.expectError(error.TreeTooDeep, Document.parse(a, source.items));
}

test "depth limit accepts 128 node levels and rejects 129" {
    const a = std.testing.allocator;
    for ([_]usize{ 128, 129 }) |levels| {
        for ([_][]const u8{
            "{\"kind\":\"text\",\"content\":\"Leaf\"}",
            "{\"kind\":\"stack\",\"axis\":\"column\",\"children\":[]}",
        }) |leaf| {
            var source: std.ArrayList(u8) = .empty;
            defer source.deinit(a);
            try source.appendSlice(a, "{\"version\":7,\"root\":[");
            for (1..levels) |_| try source.appendSlice(a, "{\"kind\":\"stack\",\"axis\":\"column\",\"children\":[");
            try source.appendSlice(a, leaf);
            for (1..levels) |_| try source.appendSlice(a, "]}");
            try source.appendSlice(a, "]}");
            if (levels == 128) {
                var document = try Document.parse(a, source.items);
                document.deinit();
            } else {
                try std.testing.expectError(error.TreeTooDeep, Document.parse(a, source.items));
            }
        }
    }
}
