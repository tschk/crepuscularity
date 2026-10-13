//! Owned, validated subset of Crepuscularity View IR v7.
//! The compiler resolves templates/context; this decoder executes no expressions.
const std = @import("std");

pub const ir_version: u32 = 7;
pub const zpui_revision = "cca22983e7e8124bb8e650616e3803f217e52b95";
pub const Axis = enum { row, column };
pub const Kind = enum { stack, text, button, scroll };

pub const Style = struct {
    id: ?[]const u8 = null,
    padding: ?f32 = null,
    paddingHorizontal: ?f32 = null,
    paddingVertical: ?f32 = null,
    paddingTop: ?f32 = null,
    paddingRight: ?f32 = null,
    paddingBottom: ?f32 = null,
    paddingLeft: ?f32 = null,
    margin: ?f32 = null,
    width: ?f32 = null,
    height: ?f32 = null,
    fontSize: ?f32 = null,
    fontWeight: ?f32 = null,
    cornerRadius: ?f32 = null,
    borderWidth: ?f32 = null,
    opacity: ?f32 = null,
    foregroundColor: ?u32 = null,
    backgroundColor: ?u32 = null,
    borderColor: ?u32 = null,
    hidden: ?bool = null,
    overflowHidden: ?bool = null,
    flexDirection: ?Axis = null,
};

pub const Node = struct {
    kind: Kind,
    text: []const u8 = "",
    axis: Axis = .column,
    spacing: ?f32 = null,
    on_click: ?[]const u8 = null,
    style: Style = .{},
    children: []const Node = &.{},
};

pub const Document = struct {
    arena: std.heap.ArenaAllocator,
    root: []const Node,

    /// Strings and nodes belong to this document, independent of the JSON input.
    pub fn parse(parent: std.mem.Allocator, json: []const u8) !Document {
        if (json.len > 1024 * 1024) return error.DocumentTooLarge;
        const parsed = try std.json.parseFromSlice(std.json.Value, parent, json, .{});
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
        const root = try decodeNodes(arena.allocator(), value.object.get("root") orelse return error.MissingRoot, 0);
        return .{ .arena = arena, .root = root };
    }

    pub fn deinit(self: *Document) void {
        self.arena.deinit();
        self.* = undefined;
    }
};

fn decodeNodes(a: std.mem.Allocator, value: std.json.Value, depth: usize) anyerror![]const Node {
    if (depth > 128) return error.TreeTooDeep;
    if (value != .array) return error.InvalidChildren;
    const nodes = try a.alloc(Node, value.array.items.len);
    for (value.array.items, nodes) |item, *node| node.* = try decodeNode(a, item, depth);
    return nodes;
}

fn decodeNode(a: std.mem.Allocator, value: std.json.Value, depth: usize) !Node {
    if (value != .object) return error.InvalidNode;
    const tag = try string(value.object.get("kind") orelse return error.MissingKind);
    const kind = std.meta.stringToEnum(Kind, tag) orelse return error.UnsupportedNode;
    var node: Node = .{ .kind = kind };
    var fields = value.object.iterator();
    while (fields.next()) |field| {
        const key = field.key_ptr.*;
        if (oneOf(key, &.{ "kind", "style" })) continue;
        if ((kind == .stack or kind == .scroll) and oneOf(key, &.{ "axis", "children" })) continue;
        if (kind == .stack and std.mem.eql(u8, key, "spacing")) continue;
        if (kind == .text and std.mem.eql(u8, key, "content")) continue;
        if (kind == .button and oneOf(key, &.{ "label", "onClick" })) continue;
        // Nullable optional metadata has no active behavior in a snapshot.
        if (field.value_ptr.* != .null) return error.UnsupportedNodeField;
    }
    switch (kind) {
        .text, .button => {
            const key: []const u8 = if (kind == .text) "content" else "label";
            node.text = try a.dupe(u8, try string(value.object.get(key) orelse return error.MissingText));
            if (value.object.get("onClick")) |event| {
                if (event != .null) {
                    const handler = try string(event);
                    if (handler.len == 0) return error.EmptyHandler;
                    node.on_click = try a.dupe(u8, handler);
                }
            }
        },
        .stack, .scroll => {
            const axis = try string(value.object.get("axis") orelse return error.MissingAxis);
            node.axis = std.meta.stringToEnum(Axis, axis) orelse return error.UnsupportedAxis;
            node.children = try decodeNodes(a, value.object.get("children") orelse return error.MissingChildren, depth + 1);
            if (value.object.get("spacing")) |spacing| {
                if (spacing != .null) {
                    node.spacing = try number(spacing);
                    if (node.spacing.? < 0) return error.InvalidSpacing;
                }
            }
        },
    }
    if (value.object.get("style")) |style| {
        if (style != .null) node.style = try decodeStyle(a, style);
    }
    return node;
}

fn decodeStyle(a: std.mem.Allocator, value: std.json.Value) !Style {
    if (value != .object) return error.InvalidStyle;
    var result: Style = .{};
    var fields = value.object.iterator();
    while (fields.next()) |field| {
        const key = field.key_ptr.*;
        const val = field.value_ptr.*;
        if (val == .null) continue;
        if (std.mem.eql(u8, key, "classes")) {
            // Original class tokens are metadata; resolved ViewStyle hints drive layout.
            if (val != .array) return error.InvalidClasses;
            for (val.array.items) |item| _ = try string(item);
            continue;
        }
        if (std.mem.eql(u8, key, "id")) {
            result.id = try a.dupe(u8, try string(val));
            continue;
        }
        if (std.mem.eql(u8, key, "flexDirection")) {
            result.flexDirection = std.meta.stringToEnum(Axis, try string(val)) orelse return error.UnsupportedAxis;
            continue;
        }
        var found = false;
        inline for (.{ "hidden", "overflowHidden" }) |name| {
            if (std.mem.eql(u8, key, name)) {
                if (val != .bool) return error.InvalidBoolean;
                @field(result, name) = val.bool;
                found = true;
            }
        }
        inline for (.{ "foregroundColor", "backgroundColor", "borderColor" }) |name| {
            if (std.mem.eql(u8, key, name)) {
                const color = try string(val);
                if (color.len != 7 or color[0] != '#') return error.UnsupportedColor;
                @field(result, name) = std.fmt.parseInt(u32, color[1..], 16) catch return error.UnsupportedColor;
                found = true;
            }
        }
        inline for (.{ "padding", "paddingHorizontal", "paddingVertical", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft", "margin", "width", "height", "fontSize", "fontWeight", "cornerRadius", "borderWidth", "opacity" }) |name| {
            if (std.mem.eql(u8, key, name)) {
                const n = try number(val);
                if (comptime oneOf(name, &.{ "width", "height" })) {
                    if (n < 0 and n != -1 and n != -2) return error.InvalidSize;
                } else if (comptime std.mem.eql(u8, name, "opacity")) {
                    if (n < 0 or n > 1) return error.InvalidOpacity;
                } else if (comptime std.mem.eql(u8, name, "fontWeight")) {
                    if (n < 1 or n > 1000) return error.InvalidFontWeight;
                } else if (comptime !std.mem.eql(u8, name, "margin")) {
                    if (n < 0) return error.InvalidStyleNumber;
                }
                @field(result, name) = n;
                found = true;
            }
        }
        if (!found) return error.UnsupportedStyle;
    }
    return result;
}

fn string(value: std.json.Value) ![]const u8 {
    if (value != .string) return error.InvalidString;
    return value.string;
}

fn number(value: std.json.Value) !f32 {
    const n: f32 = switch (value) {
        .integer => |v| @floatFromInt(v),
        .float => |v| @floatCast(v),
        else => return error.InvalidNumber,
    };
    if (!std.math.isFinite(n)) return error.InvalidNumber;
    return n;
}

fn oneOf(value: []const u8, choices: []const []const u8) bool {
    for (choices) |choice| if (std.mem.eql(u8, value, choice)) return true;
    return false;
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
    try std.testing.expectError(error.UnsupportedNode, Document.parse(std.testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"input\"}]}"));
    try std.testing.expectError(error.UnsupportedNodeField, Document.parse(std.testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"button\",\"label\":\"Save\",\"onLongPress\":\"save\"}]}"));
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
