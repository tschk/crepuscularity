//! Checked Unicode helpers, linked from the separately locked native-text library.
const std = @import("std");
extern fn crepus_text_grapheme([*]const u8, usize, usize, bool) usize;
extern fn crepus_text_byte_offset([*]const u8, usize, usize, bool) usize;
extern fn crepus_text_utf16_offset([*]const u8, usize, usize) usize;
extern fn crepus_text_case([*]const u8, usize, u8, ?[*]u8, usize) usize;

pub fn valid(text: []const u8) bool {
    return std.unicode.utf8ValidateSlice(text);
}
pub fn boundary(text: []const u8, index: usize, forward: bool) usize {
    const result = crepus_text_grapheme(text.ptr, text.len, index, forward);
    std.debug.assert(result != std.math.maxInt(usize));
    return result;
}
pub fn byteOffset(text: []const u8, index: usize, round_up: bool) usize {
    const result = crepus_text_byte_offset(text.ptr, text.len, index, round_up);
    std.debug.assert(result != std.math.maxInt(usize));
    return result;
}
pub fn utf16Offset(text: []const u8, index: usize) usize {
    const result = crepus_text_utf16_offset(text.ptr, text.len, index);
    std.debug.assert(result != std.math.maxInt(usize));
    return result;
}
pub fn mapCase(a: std.mem.Allocator, text: []const u8, upper: bool) ![]const u8 {
    const mode: u8 = if (upper) 0 else 1;
    const len = crepus_text_case(text.ptr, text.len, mode, null, 0);
    if (len == std.math.maxInt(usize)) return error.InvalidUtf8;
    const output = try a.alloc(u8, len);
    errdefer a.free(output);
    if (crepus_text_case(text.ptr, text.len, mode, output.ptr, len) != len) return error.InvalidUtf8;
    return output;
}

extern fn crepus_text_mask([*]const u8, usize, bool, ?[*]u8, usize) usize;
extern fn crepus_text_mask_offset([*]const u8, usize, usize, bool, bool) usize;
pub fn mask(a: std.mem.Allocator, text: []const u8, multiline: bool) ![]const u8 {
    const len = crepus_text_mask(text.ptr, text.len, multiline, null, 0);
    if (len == std.math.maxInt(usize)) return error.InvalidUtf8;
    const output = try a.alloc(u8, len);
    errdefer a.free(output);
    if (crepus_text_mask(text.ptr, text.len, multiline, output.ptr, len) != len) return error.InvalidUtf8;
    return output;
}
pub fn maskOffset(text: []const u8, index: usize, multiline: bool, reverse: bool) usize {
    const result = crepus_text_mask_offset(text.ptr, text.len, index, multiline, reverse);
    std.debug.assert(result != std.math.maxInt(usize));
    return result;
}
