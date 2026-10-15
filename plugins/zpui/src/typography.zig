//! Unicode case transformations inherit through the portable view tree.
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
pub fn control(node: *const ir.Node, mode: Case) !*const ir.Node {
    if (mode == .normal) return node;
    const copy = try zpui.frameAllocator().create(ir.Node);
    copy.* = node.*;
    copy.text = try text(node.text, mode);
    const options = try zpui.frameAllocator().alloc(ir.Option, node.options.len);
    for (node.options, options) |option, *out| {
        out.* = option;
        out.label = try text(option.label, mode);
    }
    copy.options = options;
    return copy;
}
