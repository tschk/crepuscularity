//! Shared strict JSON readers for the owned IR decoder.
const std = @import("std");

pub fn string(value: std.json.Value) ![]const u8 {
    if (value != .string) return error.InvalidString;
    return value.string;
}

pub fn number(value: std.json.Value) !f32 {
    const n: f32 = switch (value) {
        .integer => |v| @floatFromInt(v),
        .float => |v| @floatCast(v),
        else => return error.InvalidNumber,
    };
    if (!std.math.isFinite(n)) return error.InvalidNumber;
    return n;
}

pub fn boolean(value: std.json.Value) !bool {
    if (value != .bool) return error.InvalidBoolean;
    return value.bool;
}

pub fn optional(object: std.json.Value, key: []const u8) ?std.json.Value {
    const value = object.object.get(key) orelse return null;
    return if (value == .null) null else value;
}

pub fn ownedString(a: std.mem.Allocator, object: std.json.Value, key: []const u8) !?[]const u8 {
    const value = optional(object, key) orelse return null;
    return try a.dupe(u8, try string(value));
}

pub fn requiredString(a: std.mem.Allocator, object: std.json.Value, key: []const u8) ![]const u8 {
    return try ownedString(a, object, key) orelse error.MissingText;
}

pub fn oneOf(value: []const u8, choices: []const []const u8) bool {
    for (choices) |choice| if (std.mem.eql(u8, value, choice)) return true;
    return false;
}

pub fn enumValue(comptime E: type, value: std.json.Value) !E {
    return std.meta.stringToEnum(E, try string(value)) orelse error.UnsupportedEnumValue;
}
