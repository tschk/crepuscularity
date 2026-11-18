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
//! var session = try crepus.Session.init(allocator, io);
//! defer session.deinit();
//!
//! try session.setTemplate(
//!    \\input bind=count
//!    \\span "Count {count}"
//! , null);
//! try session.setContext(.{ "count": "1" });
//! const ir = try session.renderIr();
//! // ir.root: []ViewNode — the parsed UI tree
//! ```

const c = @cImport({
    @cDefine("CREPUSCULARITY_ABI_H", {});
    @cInclude("crepuscularity_abi.h");
});

const std = @import("std");

pub const Error = error{
    AbiCallFailed,
    NoSession,
    ParseError,
};

/// A parsed UI node from the IR tree.
pub const ViewNode = struct {
    kind: []const u8,
    content: ?[]const u8 = null,
    axis: ?[]const u8 = null,
    label: ?[]const u8 = null,
    src: ?[]const u8 = null,
    children: ?[]ViewNode = null,

    pub fn deinit(self: *ViewNode, allocator: std.mem.Allocator) void {
        if (self.content) |s| allocator.free(s);
        if (self.axis) |s| allocator.free(s);
        if (self.label) |s| allocator.free(s);
        if (self.src) |s| allocator.free(s);
        if (self.children) |kids| {
            for (kids) |*k| k.deinit(allocator);
            allocator.free(kids);
        }
    }
};

/// Rendered IR output: version + root node tree.
pub const ViewIr = struct {
    version: u32,
    root: ?ViewNode = null,

    pub fn deinit(self: *ViewIr, allocator: std.mem.Allocator) void {
        if (self.root) |*r| r.deinit(allocator);
    }

    pub fn toHtml(self: *const ViewIr, allocator: std.mem.Allocator) ![]const u8 {
        var buf = std.ArrayList(u8).init(allocator);
        defer buf.deinit();
        if (self.root) |*r| try renderNodeToHtml(r, &buf);
        return try allocator.dupe(u8, buf.items);
    }
};

/// Event from the UI runtime (bind changes, button clicks, etc.)
pub const UiEvent = struct {
    handler: []const u8,
    payload: ?std.json.Value = null,
};

/// CrepusSession: renders .crepus templates, manages state, dispatches events.
///
/// Not thread-safe — use one session per thread.
pub const Session = struct {
    allocator: std.mem.Allocator,
    ptr: ?*c.CrepusSession,
    callback: ?*const fn (event: UiEvent) void,

    pub fn init(allocator: std.mem.Allocator) !Session {
        const ptr = c.crepus_session_new() orelse return error.AbiCallFailed;
        return .{ .allocator = allocator, .ptr = ptr, .callback = null };
    }

    pub fn deinit(self: *Session) void {
        if (self.ptr) |p| {
            c.crepus_session_free(p);
            self.ptr = null;
        }
    }

    pub fn setTemplate(self: *Session, template: []const u8, base_dir: ?[]const u8) !void {
        const rc = c.crepus_session_set_template_string(
            self.ptr,
            @ptrCast(template.ptr),
            if (base_dir) |d| @ptrCast(d.ptr) else null,
        );
        if (rc != 0) return error.AbiCallFailed;
    }

    pub fn setContext(self: *Session, ctx: std.json.Value) !void {
        var buf: std.Io.Writer.Allocating = .init(self.allocator);
        defer buf.deinit();
        try std.json.Stringify.value(ctx, .{}, &buf.writer);
        const rc = c.crepus_session_set_context_json(
            self.ptr,
            @ptrCast(buf.written().ptr),
        );
        if (rc != 0) return error.AbiCallFailed;
    }

    pub fn patchContext(self: *Session, patch: std.json.Value) !void {
        var buf: std.Io.Writer.Allocating = .init(self.allocator);
        defer buf.deinit();
        try std.json.Stringify.value(patch, .{}, &buf.writer);
        const rc = c.crepus_session_apply_context_patch_json(
            self.ptr,
            @ptrCast(buf.written().ptr),
        );
        if (rc != 0) return error.AbiCallFailed;
    }

    pub fn onEvent(self: *Session, callback: *const fn (event: UiEvent) void) void {
        _ = self;
        _ = callback;
        // ponytail: Zig callbacks via C ABI need c.CrepusEventCallback trampoline.
        // For now, event handling is done by polling renderIr after dispatchEvent.
    }

    pub fn renderIr(self: *Session) !ViewIr {
        const json_ptr = c.crepus_session_render_ir_json(self.ptr) orelse return error.AbiCallFailed;
        defer c.crepus_string_free(json_ptr);
        const json_bytes = std.mem.sliceTo(@as([*:0]u8, @ptrCast(json_ptr)), 0);
        const parsed = try std.json.parseFromSlice(std.json.Value, self.allocator, json_bytes, .{
            .ignore_unknown_fields = true,
            .allocate = .alloc_always,
        });
        defer parsed.deinit();
        const version = @as(u32, @intCast(parsed.value.object.get("version").?.integer));
        return .{ .version = version, .root = try parseViewNode(self.allocator, parsed.value.object.get("root").?) };
    }

    pub fn dispatchEvent(self: *Session, event: []const u8) !ViewIr {
        const json_ptr = c.crepus_session_dispatch_event_json(self.ptr, @ptrCast(event.ptr)) orelse return error.AbiCallFailed;
        defer c.crepus_string_free(json_ptr);
        const json_bytes = std.mem.sliceTo(@as([*:0]u8, @ptrCast(json_ptr)), 0);
        const parsed = try std.json.parseFromSlice(std.json.Value, self.allocator, json_bytes, .{
            .ignore_unknown_fields = true,
            .allocate = .alloc_always,
        });
        defer parsed.deinit();
        const version = @as(u32, @intCast(parsed.value.object.get("version").?.integer));
        return .{ .version = version, .root = try parseViewNode(self.allocator, parsed.value.object.get("root").?) };
    }
};

fn parseViewNode(allocator: std.mem.Allocator, val: std.json.Value) !ViewNode {
    var node = ViewNode{ .kind = "" };
    if (val.object.get("kind")) |k| {
        if (k == .string) node.kind = try allocator.dupe(u8, k.string);
    }
    if (val.object.get("content")) |c| {
        if (c == .string) node.content = try allocator.dupe(u8, c.string);
    }
    if (val.object.get("axis")) |a| {
        if (a == .string) node.axis = try allocator.dupe(u8, a.string);
    }
    if (val.object.get("label")) |l| {
        if (l == .string) node.label = try allocator.dupe(u8, l.string);
    }
    if (val.object.get("src")) |s| {
        if (s == .string) node.src = try allocator.dupe(u8, s.string);
    }
    if (val.object.get("children")) |kids| {
        if (kids == .array) {
            var children = std.ArrayList(ViewNode).init(allocator);
            for (kids.array.items) |child| {
                try children.append(try parseViewNode(allocator, child));
            }
            node.children = try children.toOwnedSlice(allocator);
        }
    }
    return node;
}

fn renderNodeToHtml(node: *const ViewNode, buf: *std.ArrayList(u8)) !void {
    if (std.mem.eql(u8, node.kind, "text")) {
        if (node.content) |c| try buf.appendSlice(c);
    } else if (std.mem.eql(u8, node.kind, "stack")) {
        const axis = node.axis orelse "column";
        try buf.writer().print("<div data-crepus-kind=\"stack\" data-axis=\"{s}\">", .{axis});
        if (node.children) |kids| { for (kids) |*k| try renderNodeToHtml(k, buf); }
        try buf.appendSlice("</div>");
    } else if (std.mem.eql(u8, node.kind, "button")) {
        const label = node.label orelse "";
        try buf.writer().print("<button>{s}</button>", .{label});
    } else if (std.mem.eql(u8, node.kind, "scroll")) {
        const axis = node.axis orelse "column";
        try buf.writer().print("<div data-crepus-kind=\"scroll\" data-axis=\"{s}\">", .{axis});
        if (node.children) |kids| { for (kids) |*k| try renderNodeToHtml(k, buf); }
        try buf.appendSlice("</div>");
    } else if (std.mem.eql(u8, node.kind, "image")) {
        const src = node.src orelse "";
        try buf.writer().print("<img src=\"{s}\">", .{src});
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
    try std.testing.expect(ir.version >= 4);
}

test "session dispatch event rerenders" {
    const gpa = std.testing.allocator;
    var session = try Session.init(gpa);
    defer session.deinit();
    try session.setTemplate(
        \\input bind=count
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
