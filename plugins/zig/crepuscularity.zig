const std = @import("std");
const builtin = @import("builtin");

pub const ViewIr = struct {
    version: u32,
    json: []u8,
};

pub const UiDocument = struct {
    html: []const u8,
};

fn runCli(allocator: std.mem.Allocator, io: std.Io, argv: []const []const u8) ![]u8 {
    // Use argv, not a shell; process.run drains stdout and stderr together.
    const result = try std.process.run(allocator, io, .{ .argv = argv });
    defer allocator.free(result.stderr);
    errdefer allocator.free(result.stdout);
    switch (result.term) {
        .exited => |code| if (code != 0) return error.CrepusFailed,
        else => return error.CrepusFailed,
    }
    return result.stdout;
}

pub fn renderIr(allocator: std.mem.Allocator, io: std.Io, path: []const u8) !ViewIr {
    const output = try runCli(allocator, io, &.{ "crepus", "native", "ir", path });
    errdefer allocator.free(output);
    return .{ .version = try irVersion(allocator, output), .json = output };
}

fn irVersion(allocator: std.mem.Allocator, output: []const u8) !u32 {
    const parsed = try std.json.parseFromSlice(std.json.Value, allocator, output, .{});
    defer parsed.deinit();
    if (parsed.value != .object) return error.InvalidIr;
    const version = parsed.value.object.get("version") orelse return error.InvalidIr;
    const root = parsed.value.object.get("root") orelse return error.InvalidIr;
    if (version != .integer or version.integer < 0 or version.integer > std.math.maxInt(u32) or root != .array)
        return error.InvalidIr;
    return @intCast(version.integer);
}

pub fn renderHtml(allocator: std.mem.Allocator, io: std.Io, path: []const u8) !UiDocument {
    // Explicit empty context preserves the IR route's no-autoload behavior.
    const empty_context = if (builtin.os.tag == .windows) "NUL" else "/dev/null";
    return .{ .html = try runCli(allocator, io, &.{ "crepus", "render", path, "--ctx", empty_context }) };
}

test "renderIr decodes current root-array ViewIr" {
    const allocator = std.testing.allocator;
    const ir = try renderIr(allocator, std.testing.io, "../fixtures/hello.crepus");
    defer allocator.free(ir.json);
    try std.testing.expectEqual(@as(u32, 7), ir.version);
}

test "renderHtml outputs complete escaped HTML" {
    const allocator = std.testing.allocator;
    const doc = try renderHtml(allocator, std.testing.io, "../fixtures/html-multiple.crepus");
    defer allocator.free(doc.html);
    try std.testing.expectEqualStrings("<span>First &lt;&amp;&quot;雪</span><span>Second</span><button data-onclick=\"save\">Action</button>", std.mem.trimEnd(u8, doc.html, "\r\n"));
}

test "renderHtml handles interactive template" {
    const allocator = std.testing.allocator;
    const doc = try renderHtml(allocator, std.testing.io, "../fixtures/interactive.crepus");
    defer allocator.free(doc.html);
    try std.testing.expect(doc.html.len > 0);
    try std.testing.expect(std.mem.indexOf(u8, doc.html, "button") != null);
}

test "renderIr and renderHtml report CLI errors without leaking" {
    const allocator = std.testing.allocator;
    try std.testing.expectError(error.CrepusFailed, renderIr(allocator, std.testing.io, "../fixtures/missing-zig-template.crepus"));
    try std.testing.expectError(error.CrepusFailed, renderHtml(allocator, std.testing.io, "../fixtures/missing-zig-template.crepus"));
}

test "malformed IR shapes fail without leaked parse allocations" {
    for ([_][]const u8{
        "[]",                                   "{}",                              "{\"version\":-1,\"root\":[]}",
        "{\"version\":4294967296,\"root\":[]}", "{\"version\":\"7\",\"root\":[]}", "{\"version\":7,\"root\":{}}",
    }) |json| {
        try std.testing.expectError(error.InvalidIr, irVersion(std.testing.allocator, json));
    }
}
