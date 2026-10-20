//! Zig bindings for Crepuscularity ABI — embed .crepus UIs in any Zig project.
//!
//! Links to libcrepuscularity_abi (build via `cargo build -p crepuscularity-abi`).
//! The ABI library provides a CrepusSession that renders .crepus templates to
//! an IR JSON tree, manages state, and dispatches UI events.
//!
//! Usage:
//! ```zig
//! const crepus = @import("crepuscularity_abi.zig");
//!
//! var session = try crepus.Session.init(allocator);
//! defer session.deinit();
//!
//! try session.setTemplate(
//!    \\input bind=count
//!    \\span "Count {count}"
//! , null);
//! try session.setContext(.{ .count = "1" });
//! var ir = try session.renderIr();
//! defer ir.deinit(allocator);
//! // ir.root: []ViewNode — the parsed UI tree
//! ```

const c = @cImport({
    @cInclude("crepuscularity_abi.h");
});
const std = @import("std");

pub const Error = error{
    AbiCallFailed,
    NoSession,
    ParseError,
    EmbeddedNul,
    UnsupportedNode,
};

/// Parsed nodes own their strings and children. Release them through ViewIr.deinit.
pub const ViewNode = struct {
    kind: []const u8,
    content: ?[]const u8 = null,
    axis: ?[]const u8 = null,
    label: ?[]const u8 = null,
    src: ?[]const u8 = null,
    children: ?[]ViewNode = null,

    pub fn deinit(self: *ViewNode, allocator: std.mem.Allocator) void {
        allocator.free(self.kind);
        if (self.content) |value| allocator.free(value);
        if (self.axis) |value| allocator.free(value);
        if (self.label) |value| allocator.free(value);
        if (self.src) |value| allocator.free(value);
        if (self.children) |children| freeNodes(allocator, children);
        self.* = undefined;
    }
};

/// Owned IR snapshot, independent of the session and subsequent ABI calls.
pub const ViewIr = struct {
    version: u32,
    root: []ViewNode,

    pub fn deinit(self: *ViewIr, allocator: std.mem.Allocator) void {
        freeNodes(allocator, self.root);
        self.* = undefined;
    }

    /// Render text, stack, scroll, button and image nodes. Other kinds fail explicitly.
    /// The returned HTML is owned by the caller and must be freed with allocator.
    pub fn toHtml(self: *const ViewIr, allocator: std.mem.Allocator) ![]const u8 {
        var buffer: std.ArrayList(u8) = .empty;
        defer buffer.deinit(allocator);
        for (self.root) |*node| try renderNodeToHtml(node, allocator, &buffer);
        return try buffer.toOwnedSlice(allocator);
    }
};

pub const UiEvent = struct {
    handler: []const u8,
    payload: ?std.json.Value = null,
};

/// Session handles are owning values: do not copy them. Not thread-safe.
pub const Session = struct {
    allocator: std.mem.Allocator,
    ptr: ?*c.CrepusSession,
    callback: ?*const fn (event: UiEvent) void,

    pub fn init(allocator: std.mem.Allocator) !Session {
        const ptr = c.crepus_session_new() orelse return error.AbiCallFailed;
        return .{ .allocator = allocator, .ptr = ptr, .callback = null };
    }

    pub fn deinit(self: *Session) void {
        if (self.ptr) |ptr| c.crepus_session_free(ptr);
        self.ptr = null;
    }

    fn requireSession(self: *const Session) Error!*c.CrepusSession {
        return self.ptr orelse error.NoSession;
    }

    pub fn setTemplate(self: *Session, template: []const u8, base_dir: ?[]const u8) !void {
        const ptr = try self.requireSession();
        const source = try cString(self.allocator, template);
        defer self.allocator.free(source);
        const directory = if (base_dir) |value| try cString(self.allocator, value) else null;
        defer if (directory) |value| self.allocator.free(value);
        if (c.crepus_session_set_template_string(ptr, source.ptr, if (directory) |value| value.ptr else null) != 0)
            return error.AbiCallFailed;
    }

    /// Accept a JSON value or a Zig value supported by std.json.Stringify.
    pub fn setContext(self: *Session, context: anytype) !void {
        const ptr = try self.requireSession();
        const json = try std.json.Stringify.valueAlloc(self.allocator, context, .{});
        defer self.allocator.free(json);
        const terminated = try cString(self.allocator, json);
        defer self.allocator.free(terminated);
        if (c.crepus_session_set_context_json(ptr, terminated.ptr) != 0) return error.AbiCallFailed;
    }

    pub fn patchContext(self: *Session, patch: anytype) !void {
        const ptr = try self.requireSession();
        const json = try std.json.Stringify.valueAlloc(self.allocator, patch, .{});
        defer self.allocator.free(json);
        const terminated = try cString(self.allocator, json);
        defer self.allocator.free(terminated);
        if (c.crepus_session_apply_context_patch_json(ptr, terminated.ptr) != 0) return error.AbiCallFailed;
    }

    /// Callback registration remains unsupported; poll the returned IR after dispatchEvent.
    pub fn onEvent(self: *Session, callback: *const fn (event: UiEvent) void) void {
        _ = self;
        _ = callback;
        // shortcut: a C callback trampoline and explicit userdata lifetime are not implemented.
    }

    pub fn renderIr(self: *Session) !ViewIr {
        const ptr = try self.requireSession();
        const json = c.crepus_session_render_ir_json(ptr) orelse return error.AbiCallFailed;
        defer c.crepus_string_free(json);
        return parseViewIr(self.allocator, std.mem.span(json));
    }

    pub fn dispatchEvent(self: *Session, event: []const u8) !ViewIr {
        const ptr = try self.requireSession();
        const terminated = try cString(self.allocator, event);
        defer self.allocator.free(terminated);
        const json = c.crepus_session_dispatch_event_json(ptr, terminated.ptr) orelse return error.AbiCallFailed;
        defer c.crepus_string_free(json);
        const parsed = try std.json.parseFromSlice(std.json.Value, self.allocator, std.mem.span(json), .{});
        defer parsed.deinit();
        if (parsed.value != .object) return error.ParseError;
        const ir = parsed.value.object.get("ir") orelse return error.ParseError;
        return copyViewIr(self.allocator, ir);
    }
};

fn cString(allocator: std.mem.Allocator, value: []const u8) ![:0]u8 {
    if (std.mem.indexOfScalar(u8, value, 0) != null) return error.EmbeddedNul;
    return allocator.dupeZ(u8, value);
}

fn freeNodes(allocator: std.mem.Allocator, nodes: []ViewNode) void {
    for (nodes) |*node| node.deinit(allocator);
    allocator.free(nodes);
}

fn parseViewIr(allocator: std.mem.Allocator, source: []const u8) !ViewIr {
    const parsed = try std.json.parseFromSlice(std.json.Value, allocator, source, .{});
    defer parsed.deinit();
    return copyViewIr(allocator, parsed.value);
}

fn copyViewIr(allocator: std.mem.Allocator, value: std.json.Value) !ViewIr {
    if (value != .object) return error.ParseError;
    const version = value.object.get("version") orelse return error.ParseError;
    if (version != .integer or version.integer < 0 or version.integer > std.math.maxInt(u32)) return error.ParseError;
    const root = value.object.get("root") orelse return error.ParseError;
    return .{ .version = @intCast(version.integer), .root = try parseNodes(allocator, root) };
}

fn parseNodes(allocator: std.mem.Allocator, value: std.json.Value) anyerror![]ViewNode {
    if (value != .array) return error.ParseError;
    const nodes = try allocator.alloc(ViewNode, value.array.items.len);
    var initialized: usize = 0;
    errdefer {
        for (nodes[0..initialized]) |*node| node.deinit(allocator);
        allocator.free(nodes);
    }
    for (value.array.items, nodes) |child, *node| {
        node.* = try parseViewNode(allocator, child);
        initialized += 1;
    }
    return nodes;
}

fn optionalString(allocator: std.mem.Allocator, value: std.json.Value, key: []const u8) !?[]const u8 {
    const field = value.object.get(key) orelse return null;
    if (field == .null) return null;
    if (field != .string) return error.ParseError;
    return try allocator.dupe(u8, field.string);
}

fn parseViewNode(allocator: std.mem.Allocator, value: std.json.Value) anyerror!ViewNode {
    if (value != .object) return error.ParseError;
    const kind = value.object.get("kind") orelse return error.ParseError;
    if (kind != .string) return error.ParseError;
    var node = ViewNode{ .kind = try allocator.dupe(u8, kind.string) };
    errdefer node.deinit(allocator);
    node.content = try optionalString(allocator, value, "content");
    node.axis = try optionalString(allocator, value, "axis");
    node.label = try optionalString(allocator, value, "label");
    node.src = try optionalString(allocator, value, "src");
    if (value.object.get("children")) |children| {
        if (children != .null) node.children = try parseNodes(allocator, children);
    }
    return node;
}

fn escapeHtml(allocator: std.mem.Allocator, buffer: *std.ArrayList(u8), value: []const u8, attribute: bool) !void {
    for (value) |byte| {
        const escaped: ?[]const u8 = switch (byte) {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            '\r' => if (attribute) "&#13;" else null,
            else => null,
        };
        if (escaped) |text| try buffer.appendSlice(allocator, text) else try buffer.append(allocator, byte);
    }
}

fn renderNodeToHtml(node: *const ViewNode, allocator: std.mem.Allocator, buffer: *std.ArrayList(u8)) anyerror!void {
    if (std.mem.eql(u8, node.kind, "text")) {
        try escapeHtml(allocator, buffer, node.content orelse "", false);
    } else if (std.mem.eql(u8, node.kind, "stack") or std.mem.eql(u8, node.kind, "scroll")) {
        try buffer.appendSlice(allocator, "<div data-crepus-kind=\"");
        try escapeHtml(allocator, buffer, node.kind, true);
        try buffer.appendSlice(allocator, "\" data-axis=\"");
        try escapeHtml(allocator, buffer, node.axis orelse "column", true);
        try buffer.appendSlice(allocator, "\">");
        if (node.children) |children| {
            for (children) |*child| try renderNodeToHtml(child, allocator, buffer);
        }
        try buffer.appendSlice(allocator, "</div>");
    } else if (std.mem.eql(u8, node.kind, "button")) {
        try buffer.appendSlice(allocator, "<button>");
        try escapeHtml(allocator, buffer, node.label orelse "", false);
        try buffer.appendSlice(allocator, "</button>");
    } else if (std.mem.eql(u8, node.kind, "image")) {
        try buffer.appendSlice(allocator, "<img src=\"");
        try escapeHtml(allocator, buffer, node.src orelse "", true);
        try buffer.appendSlice(allocator, "\">");
    } else {
        return error.UnsupportedNode;
    }
}

test "session init and free roundtrips" {
    const gpa = std.testing.allocator;
    var session = try Session.init(gpa);
    defer session.deinit();
    try std.testing.expect(session.ptr != null);
}

test "session renders template to IR" {
    const gpa = std.testing.allocator;
    var session = try Session.init(gpa);
    defer session.deinit();
    try session.setTemplate(
        \\input bind=count
        \\span
        \\  "Count {count}"
    , null);
    try session.setContext(.{ .count = "1" });
    var ir = try session.renderIr();
    defer ir.deinit(gpa);
    try std.testing.expect(ir.version == 7);
}

test "session dispatch event rerenders" {
    const gpa = std.testing.allocator;
    var session = try Session.init(gpa);
    defer session.deinit();
    try session.setTemplate(
        \\span
        \\  "Count {count}"
    , null);
    try session.setContext(.{ .count = "1" });
    var before = try session.renderIr();
    defer before.deinit(gpa);
    const before_html = try before.toHtml(gpa);
    defer gpa.free(before_html);
    try std.testing.expect(std.mem.indexOf(u8, before_html, "Count 1") != null);

    var after = try session.dispatchEvent("{\"handler\":\"bind:count:2\"}");
    defer after.deinit(gpa);
    const after_html = try after.toHtml(gpa);
    defer gpa.free(after_html);
    try std.testing.expect(std.mem.indexOf(u8, after_html, "Count 2") != null);
}

test "renderHtml outputs valid HTML" {
    const gpa = std.testing.allocator;
    var session = try Session.init(gpa);
    defer session.deinit();
    try session.setTemplate(
        \\stack axis=column
        \\  "hello zig"
    , null);
    var ir = try session.renderIr();
    defer ir.deinit(gpa);
    const html = try ir.toHtml(gpa);
    defer gpa.free(html);
    try std.testing.expect(std.mem.indexOf(u8, html, "hello zig") != null);
}

test "owned root arrays survive JSON input release and escape HTML attributes" {
    const allocator = std.testing.allocator;
    var ir = blk: {
        const source = try allocator.dupe(u8, "{\"version\":7,\"root\":[{\"kind\":\"scroll\",\"axis\":\"a\\r\\n&#13;\\\"雪\",\"children\":[{\"kind\":\"text\",\"content\":\"<&>\"}]},{\"kind\":\"image\",\"src\":\"a\\r\\\"&\"}]}");
        defer allocator.free(source);
        break :blk try parseViewIr(allocator, source);
    };
    defer ir.deinit(allocator);
    try std.testing.expectEqual(@as(usize, 2), ir.root.len);
    const html = try ir.toHtml(allocator);
    defer allocator.free(html);
    try std.testing.expectEqualStrings("<div data-crepus-kind=\"scroll\" data-axis=\"a&#13;\n&amp;#13;&quot;雪\">&lt;&amp;&gt;</div><img src=\"a&#13;&quot;&amp;\">", html);
}

test "malformed IR and unsupported HTML fail without leaked partial trees" {
    const allocator = std.testing.allocator;
    for ([_][]const u8{
        "{}",                          "{\"version\":-1,\"root\":[]}",
        "{\"version\":7,\"root\":{}}", "{\"version\":7,\"root\":[{\"kind\":\"stack\",\"axis\":\"column\",\"children\":[{\"kind\":\"text\",\"content\":false}]}]}",
    }) |source| {
        try std.testing.expectError(error.ParseError, parseViewIr(allocator, source));
    }
    var ir = try parseViewIr(allocator, "{\"version\":7,\"root\":[{\"kind\":\"input\"}]}");
    defer ir.deinit(allocator);
    try std.testing.expectError(error.UnsupportedNode, ir.toHtml(allocator));
}

test "session rejects embedded NUL and closed handles" {
    const allocator = std.testing.allocator;
    var session = try Session.init(allocator);
    defer session.deinit();
    try std.testing.expectError(error.EmbeddedNul, session.setTemplate("span\x00hidden", null));
    try std.testing.expectError(error.EmbeddedNul, session.setTemplate("span", "dir\x00hidden"));
    try std.testing.expectError(error.EmbeddedNul, session.dispatchEvent("{}\x00hidden"));
    session.deinit();
    try std.testing.expectError(error.NoSession, session.renderIr());
    try std.testing.expectError(error.NoSession, session.setContext(.{}));
}

test "owned parsing and rendering clean up every allocation failure" {
    const Exercise = struct {
        fn run(allocator: std.mem.Allocator) !void {
            var ir = try parseViewIr(allocator, "{\"version\":7,\"root\":[{\"kind\":\"stack\",\"axis\":\"column\",\"children\":[{\"kind\":\"text\",\"content\":\"Owned雪\"}]},{\"kind\":\"button\",\"label\":\"Action\"}]}");
            defer ir.deinit(allocator);
            const html = try ir.toHtml(allocator);
            defer allocator.free(html);
        }
    };
    try std.testing.checkAllAllocationFailures(std.testing.allocator, Exercise.run, .{});
}

test "snapshots outlive session teardown and later context patches" {
    const allocator = std.testing.allocator;
    var session = try Session.init(allocator);
    defer session.deinit();
    try session.setTemplate("span \"Count {count}\"\nspan \"Second\"", null);
    const context = try std.json.parseFromSlice(std.json.Value, allocator, "{\"count\":\"1\"}", .{});
    defer context.deinit();
    try session.setContext(context.value);
    var before = try session.renderIr();
    defer before.deinit(allocator);
    try session.patchContext(.{ .count = "2" });
    var after = try session.renderIr();
    defer after.deinit(allocator);
    session.deinit();
    try std.testing.expectEqual(@as(usize, 2), before.root.len);
    const before_html = try before.toHtml(allocator);
    defer allocator.free(before_html);
    const after_html = try after.toHtml(allocator);
    defer allocator.free(after_html);
    try std.testing.expectEqualStrings("Count 1Second", before_html);
    try std.testing.expectEqualStrings("Count 2Second", after_html);
}
