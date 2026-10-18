//! Unicode case transformations inherit through the portable view tree.
const std = @import("std");
const zpui = @import("zpui");
const ir = @import("view_ir");
const unicode = @import("text_support.zig");
pub const Case = ir.styles.TextTransform;
pub fn text(value: []const u8, mode: Case) ![]const u8 {
    return switch (mode) {
        .normal => value,
        .uppercase => try unicode.mapCase(zpui.frameAllocator(), value, true),
        .lowercase => try unicode.mapCase(zpui.frameAllocator(), value, false),
    };
}
/// Copy presentation metadata without changing bound values or callback names.
pub fn control(a: std.mem.Allocator, node: *const ir.Node, mode: Case) !*const ir.Node {
    if (mode == .normal) return node;
    const copy = try a.create(ir.Node);
    copy.* = node.*;
    copy.text = try a.dupe(u8, try text(node.text, mode));
    const options = try a.alloc(ir.Option, node.options.len);
    for (node.options, options) |option, *out| {
        out.* = option;
        out.label = try a.dupe(u8, try text(option.label, mode));
    }
    copy.options = options;
    return copy;
}
