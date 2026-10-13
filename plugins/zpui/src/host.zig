//! Typed host bindings and events. The host owns mutation and notification.
const std = @import("std");
const ir = @import("view_ir");

pub const Value = union(enum) { text: []const u8, boolean: bool, number: f64 };
pub const Change = struct { bind: ?[]const u8, handler: ?[]const u8, value: Value };
pub const Link = struct { href: []const u8, target: ?[]const u8, rel: ?[]const u8 };

pub fn read(comptime View: type, self: *View, bind: []const u8) !Value {
    if (comptime @hasDecl(View, "crepusValue")) {
        return self.crepusValue(bind) orelse error.UnknownBinding;
    } else return error.MissingBindingReader;
}

pub fn checked(comptime View: type, self: *View, node: *const ir.Node) !bool {
    const bind = node.bind orelse return node.checked;
    const value = try read(View, self, bind);
    return if (value == .boolean) value.boolean else error.BindingTypeMismatch;
}

pub fn numeric(comptime View: type, self: *View, node: *const ir.Node) !f64 {
    const bind = node.bind orelse return node.value;
    const value = try read(View, self, bind);
    if (value != .number) return error.BindingTypeMismatch;
    if (!std.math.isFinite(value.number) or value.number < node.min or value.number > node.max) return error.InvalidBoundRange;
    return value.number;
}

pub fn text(comptime View: type, self: *View, node: *const ir.Node) ![]const u8 {
    const bind = node.bind orelse return node.text;
    const value = try read(View, self, bind);
    return if (value == .text) value.text else error.BindingTypeMismatch;
}

pub fn selected(comptime View: type, self: *View, node: *const ir.Node) !u32 {
    const value = try text(View, self, node);
    for (node.options, 0..) |option, index| if (std.mem.eql(u8, option.value, value)) return @intCast(index);
    return error.UnknownSelection;
}

/// Check required methods and binding types before constructing a frame.
pub fn validate(comptime View: type, self: *View, document: *const ir.Document) !void {
    for (document.root) |*node| try validateNode(View, self, node);
}

fn validateNode(comptime View: type, self: *View, node: *const ir.Node) anyerror!void {
    if (node.on_click != null and !@hasDecl(View, "onCrepusClick")) return error.MissingClickHandler;
    if ((node.on_change != null or (node.bind != null and node.kind != .text)) and !@hasDecl(View, "onCrepusChange")) return error.MissingChangeHandler;
    switch (node.kind) {
        .text => _ = try text(View, self, node),
        .toggle, .checkbox => _ = try checked(View, self, node),
        .slider => _ = try numeric(View, self, node),
        .picker, .tabs => _ = try selected(View, self, node),
        .link => if (!@hasDecl(View, "onCrepusLink")) return error.MissingLinkHandler,
        .image => if (std.mem.indexOf(u8, node.src, "://") != null and !@hasDecl(View, "crepusImage")) return error.MissingImageResolver,
        else => {},
    }
    for (node.children) |*child| try validateNode(View, self, child);
    for (node.options) |option| for (option.children) |*child| try validateNode(View, self, child);
}
