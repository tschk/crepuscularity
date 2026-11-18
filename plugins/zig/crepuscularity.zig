const std = @import("std");

pub const ViewIr = struct {
    version: u32,
    json: []u8,
};

pub const UiDocument = struct {
    html: []const u8,
};

pub fn renderIr(allocator: std.mem.Allocator, path: []const u8) !ViewIr {
    // ponytail: argv exec, no shell
    const result = try std.process.Child.run(.{
        .allocator = allocator,
        .argv = &.{ "crepus", "native", "ir", path },
    });
    defer allocator.free(result.stderr);
    if (result.term.Exited != 0) return error.CrepusFailed;
    const parsed = try std.json.parseFromSlice(std.json.Value, allocator, result.stdout, .{});
    defer parsed.deinit();
    const version = @as(u32, @intCast(parsed.value.object.get("version").?.integer));
    return .{ .version = version, .json = result.stdout };
}

pub fn renderHtml(allocator: std.mem.Allocator, path: []const u8) !UiDocument {
    var ir = try renderIr(allocator, path);
    defer allocator.free(ir.json);

    const parsed = try std.json.parseFromSlice(std.json.Value, allocator, ir.json, .{});
    defer parsed.deinit();

    const root = parsed.value.object.get("root") orelse return error.MissingRoot;
    var html_buf = std.ArrayList(u8).init(allocator);
    defer html_buf.deinit();
    try renderNodeToHtml(root.*, &html_buf);

    return .{ .html = try html_buf.toOwnedSlice(allocator) };
}

fn renderNodeToHtml(node: std.json.Value, buf: *std.ArrayList(u8)) !void {
    if (node != .object) return;
    const kind = if (node.object.get("kind")) |k| switch (k) { .string => |s| s, else => return } else return;

    if (std.mem.eql(u8, kind, "text")) {
        const content = if (node.object.get("content")) |c| switch (c) { .string => |s| s, else => "" } else "";
        try buf.appendSlice(content);
    } else if (std.mem.eql(u8, kind, "stack")) {
        const axis = if (node.object.get("axis")) |a| switch (a) { .string => |s| s, else => "column" } else "column";
        try buf.writer().print("<div data-crepus-kind=\"stack\" data-axis=\"{s}\">", .{axis});
        try renderChildren(node, buf);
        try buf.appendSlice("</div>");
    } else if (std.mem.eql(u8, kind, "button")) {
        const label = if (node.object.get("label")) |l| switch (l) { .string => |s| s, else => "" } else "";
        try buf.writer().print("<button>{s}</button>", .{label});
    } else if (std.mem.eql(u8, kind, "scroll")) {
        const axis = if (node.object.get("axis")) |a| switch (a) { .string => |s| s, else => "column" } else "column";
        try buf.writer().print("<div data-crepus-kind=\"scroll\" data-axis=\"{s}\">", .{axis});
        try renderChildren(node, buf);
        try buf.appendSlice("</div>");
    } else if (std.mem.eql(u8, kind, "image")) {
        const src = if (node.object.get("src")) |s| switch (s) { .string => |v| v, else => "" } else "";
        try buf.writer().print("<img src=\"{s}\">", .{src});
    }
}

fn renderChildren(node: std.json.Value, buf: *std.ArrayList(u8)) !void {
    const children = if (node.object.get("children")) |c| switch (c) {
        .array => |a| a.items,
        else => return,
    } else return;
    for (children) |child| {
        try renderNodeToHtml(child, buf);
    }
}

test "renderIr decodes ViewIr" {
    const allocator = std.testing.allocator;
    const ir = try renderIr(allocator, "../fixtures/hello.crepus");
    defer allocator.free(ir.json);
    try std.testing.expectEqual(@as(u32, 7), ir.version);
}

test "renderHtml outputs valid HTML" {
    const allocator = std.testing.allocator;
    const doc = try renderHtml(allocator, "../fixtures/hello.crepus");
    defer allocator.free(doc.html);
    try std.testing.expect(doc.html.len > 0);
    try std.testing.expect(std.mem.indexOf(u8, doc.html, "Hello") != null);
}

test "renderHtml handles interactive template" {
    const allocator = std.testing.allocator;
    const doc = try renderHtml(allocator, "../fixtures/interactive.crepus");
    defer allocator.free(doc.html);
    try std.testing.expect(doc.html.len > 0);
    try std.testing.expect(std.mem.indexOf(u8, doc.html, "button") != null);
}
