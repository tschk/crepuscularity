//! Validated layout and typography hints, independent of a graphics backend.
const std = @import("std");
const json = @import("json.zig");
pub const Axis = enum { row, column };
pub const Align = enum { start, end, center, stretch, baseline };
pub const Justify = enum { start, end, center, between, around, evenly };
pub const TextAlign = enum { left, center, right };
pub const TextTransform = enum { normal, uppercase, lowercase };
pub const FontFamily = enum { sans, serif, mono };
pub const Position = enum { static, relative, absolute };
pub const ObjectPosition = enum { center, top, bottom, left, right, @"left-top", @"left-bottom", @"right-top", @"right-bottom" };
pub const Gradient = enum { @"to-t", @"to-tr", @"to-r", @"to-br", @"to-b", @"to-bl", @"to-l", @"to-tl" };
pub const WhiteSpace = enum { normal, nowrap };
pub const TextOverflow = enum { clip, ellipsis };
pub const Cursor = enum { auto, default, pointer, text, move };
pub const ObjectFit = enum { contain, cover, fill, none, @"scale-down" };

pub const Style = struct {
    id: ?[]const u8 = null,
    classes: ?[]const []const u8 = null,
    padding: ?f32 = null,
    paddingHorizontal: ?f32 = null,
    paddingVertical: ?f32 = null,
    paddingTop: ?f32 = null,
    paddingRight: ?f32 = null,
    paddingBottom: ?f32 = null,
    paddingLeft: ?f32 = null,
    margin: ?f32 = null,
    marginHorizontal: ?f32 = null,
    marginVertical: ?f32 = null,
    marginTop: ?f32 = null,
    marginRight: ?f32 = null,
    marginBottom: ?f32 = null,
    marginLeft: ?f32 = null,
    width: ?f32 = null,
    height: ?f32 = null,
    minWidth: ?f32 = null,
    minHeight: ?f32 = null,
    maxWidth: ?f32 = null,
    maxHeight: ?f32 = null,
    aspectRatio: ?f32 = null,
    fontSize: ?f32 = null,
    fontWeight: ?f32 = null,
    textAlign: ?TextAlign = null,
    lineHeight: ?f32 = null,
    fontFamily: ?FontFamily = null,
    textTransform: ?TextTransform = null,
    italic: ?bool = null,
    underline: ?bool = null,
    strikethrough: ?bool = null,
    cornerRadius: ?f32 = null,
    borderWidth: ?f32 = null,
    opacity: ?f32 = null,
    foregroundColor: ?u32 = null,
    backgroundColor: ?u32 = null,
    borderColor: ?u32 = null,
    hidden: ?bool = null,
    overflowHidden: ?bool = null,
    flexDirection: ?Axis = null,
    flexGrow: ?f32 = null,
    flexShrink: ?f32 = null,
    flexWrap: ?bool = null,
    alignSelf: ?Align = null,
    position: ?Position = null,
    top: ?f32 = null,
    right: ?f32 = null,
    bottom: ?f32 = null,
    left: ?f32 = null,
    shadowColor: ?u32 = null,
    shadowRadius: ?f32 = null,
    shadowOffsetX: ?f32 = null,
    shadowOffsetY: ?f32 = null,
    textOverflow: ?TextOverflow = null,
    whiteSpace: ?WhiteSpace = null,
    lineClamp: ?u32 = null,
    cursor: ?Cursor = null,
    objectFit: ?ObjectFit = null,
    objectPosition: ?ObjectPosition = null,
    translateX: ?f32 = null,
    translateY: ?f32 = null,
    backgroundGradientDirection: ?Gradient = null,
    backgroundGradientFrom: ?u32 = null,
    backgroundGradientTo: ?u32 = null,
};

pub fn decode(a: std.mem.Allocator, value: std.json.Value) !Style {
    if (value != .object) return error.InvalidStyle;
    var result: Style = .{};
    var fields = value.object.iterator();
    while (fields.next()) |field| {
        const key = field.key_ptr.*;
        const val = field.value_ptr.*;
        if (val == .null) continue;
        // Web-only layout metadata does not change the native content stack.
        if (std.mem.eql(u8, key, "htmlDisplayContents")) {
            _ = try json.boolean(val);
            continue;
        }
        if (std.mem.eql(u8, key, "classes")) {
            if (val != .array) return error.InvalidClasses;
            const owned = try a.alloc([]const u8, val.array.items.len);
            for (val.array.items, owned) |item, *out| out.* = try a.dupe(u8, try json.string(item));
            result.classes = owned;
            continue;
        }
        var found = false;
        inline for (comptime std.meta.fieldNames(Style)) |name| {
            if (std.mem.eql(u8, key, name)) {
                const T = @typeInfo(@TypeOf(@field(result, name))).optional.child;
                @field(result, name) = if (comptime T == []const u8)
                    try a.dupe(u8, try json.string(val))
                else if (comptime T == []const []const u8)
                    unreachable // classes are handled above
                else if (comptime T == bool)
                    try json.boolean(val)
                else if (comptime T == f32)
                    try styleNumber(name, val)
                else if (comptime T == u32 and std.mem.eql(u8, name, "lineClamp"))
                    try positiveInteger(val)
                else if (comptime T == u32)
                    try color(val)
                else
                    try json.enumValue(T, val);
                found = true;
            }
        }
        if (!found) return error.UnsupportedStyle;
    }
    const gradient = result.backgroundGradientDirection != null or result.backgroundGradientFrom != null or result.backgroundGradientTo != null;
    if (gradient and (result.backgroundGradientDirection == null or result.backgroundGradientFrom == null or result.backgroundGradientTo == null)) return error.IncompleteGradient;
    return result;
}

fn styleNumber(comptime key: []const u8, value: std.json.Value) !f32 {
    const n = try json.number(value);
    if (comptime json.oneOf(key, &.{ "width", "height", "maxWidth", "maxHeight" })) {
        if (n < 0 and n != -1 and n != -2) return error.InvalidSize;
    } else if (comptime std.mem.eql(u8, key, "opacity")) {
        if (n < 0 or n > 1) return error.InvalidOpacity;
    } else if (comptime std.mem.eql(u8, key, "fontWeight")) {
        if (n < 1 or n > 1000 or @floor(n) != n) return error.InvalidFontWeight;
    } else if (comptime std.mem.eql(u8, key, "aspectRatio") or std.mem.eql(u8, key, "lineHeight")) {
        if (n <= 0) return error.InvalidStyleNumber;
    } else if (comptime !std.mem.startsWith(u8, key, "margin") and !json.oneOf(key, &.{ "top", "right", "bottom", "left", "shadowOffsetX", "shadowOffsetY", "translateX", "translateY" })) {
        if (n < 0) return error.InvalidStyleNumber;
    }
    return n;
}

fn positiveInteger(value: std.json.Value) !u32 {
    if (value != .integer or value.integer <= 0 or value.integer > std.math.maxInt(u32)) return error.InvalidLineClamp;
    return @intCast(value.integer);
}

fn color(value: std.json.Value) !u32 {
    const text = try json.string(value);
    if (text.len != 7 or text[0] != '#') return error.UnsupportedColor;
    for (text[1..]) |digit| if (!std.ascii.isHex(digit)) return error.UnsupportedColor;
    return std.fmt.parseInt(u32, text[1..], 16) catch error.UnsupportedColor;
}
