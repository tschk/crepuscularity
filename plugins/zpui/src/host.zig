//! Explicit host bindings. Expressions remain host-owned; the bridge scopes loop values.
const std = @import("std");
const ir = @import("view_ir");

pub const Value = union(enum) { text: []const u8, boolean: bool, number: f64, record: []const Field, list: []const Item };
pub const Field = struct { name: []const u8, value: Value };
pub const Item = struct { key: []const u8, value: Value };
pub const Scope = struct { parent: ?*const Scope = null, name: []const u8, key: []const u8, index: usize, value: Value };
pub const Change = struct { bind: ?[]const u8, handler: ?[]const u8, value: Value, scope: ?*const Scope = null };
pub const Link = struct { href: []const u8, target: ?[]const u8, rel: ?[]const u8 };
pub const Files = struct { handler: ?[]const u8, paths: []const []const u8, scope: ?*const Scope = null };
pub const Input = struct { identity: []const u8, bind: []const u8, handler: ?[]const u8, value: []const u8, placeholder: []const u8, multiline: bool, secure: bool, scope: ?*const Scope };
pub const WebView = struct { identity: []const u8, src: []const u8, scope: ?*const Scope };

pub fn read(comptime View: type, self: *View, bind: []const u8) !Value {
    return readAt(View, self, bind, null);
}

pub fn readAt(comptime View: type, self: *View, bind: []const u8, scope: ?*const Scope) !Value {
    if (comptime @hasDecl(View, "crepusResolve")) {
        if (self.crepusResolve(bind, scope)) |value| return value;
    }
    var current = scope;
    while (current) |s| : (current = s.parent) {
        if (std.mem.eql(u8, bind, s.name)) return s.value;
        if (std.mem.startsWith(u8, bind, s.name) and bind.len > s.name.len and bind[s.name.len] == '.') {
            var fields = std.mem.splitScalar(u8, bind[s.name.len + 1 ..], '.');
            var value = s.value;
            while (fields.next()) |name| {
                if (value != .record) return error.BindingTypeMismatch;
                var found = false;
                for (value.record) |field| {
                    if (std.mem.eql(u8, field.name, name)) {
                        value = field.value;
                        found = true;
                        break;
                    }
                }
                if (!found) return error.UnknownBinding;
            }
            return value;
        }
    }
    if (comptime @hasDecl(View, "crepusValue")) return self.crepusValue(bind) orelse error.UnknownBinding;
    return error.MissingBindingReader;
}

pub fn condition(comptime View: type, self: *View, expression: []const u8, scope: ?*const Scope) !bool {
    if (comptime @hasDecl(View, "crepusCondition")) return self.crepusCondition(expression, scope) orelse error.UnknownCondition;
    const value = try readAt(View, self, expression, scope);
    return if (value == .boolean) value.boolean else error.BindingTypeMismatch;
}

pub fn collection(comptime View: type, self: *View, bind: []const u8, scope: ?*const Scope) ![]const Item {
    const value = try readAt(View, self, bind, scope);
    if (value != .list) return error.BindingTypeMismatch;
    for (value.list, 0..) |item, index| {
        if (item.key.len == 0) return error.EmptyCollectionKey;
        for (value.list[0..index]) |previous| if (std.mem.eql(u8, previous.key, item.key)) return error.DuplicateCollectionKey;
    }
    return value.list;
}

pub fn checked(comptime View: type, self: *View, node: *const ir.Node) !bool {
    return checkedAt(View, self, node, null);
}
pub fn checkedAt(comptime View: type, self: *View, node: *const ir.Node, scope: ?*const Scope) !bool {
    const bind = node.bind orelse return node.checked;
    const value = try readAt(View, self, bind, scope);
    return if (value == .boolean) value.boolean else error.BindingTypeMismatch;
}
pub fn numeric(comptime View: type, self: *View, node: *const ir.Node) !f64 {
    return numericAt(View, self, node, null);
}
pub fn numericAt(comptime View: type, self: *View, node: *const ir.Node, scope: ?*const Scope) !f64 {
    const bind = node.bind orelse return node.value;
    const value = try readAt(View, self, bind, scope);
    if (value != .number) return error.BindingTypeMismatch;
    if (!std.math.isFinite(value.number) or value.number < node.min or value.number > node.max) return error.InvalidBoundRange;
    return value.number;
}
pub fn text(comptime View: type, self: *View, node: *const ir.Node) ![]const u8 {
    return textAt(View, self, node, null);
}
pub fn textAt(comptime View: type, self: *View, node: *const ir.Node, scope: ?*const Scope) ![]const u8 {
    const bind = node.bind orelse return node.text;
    const value = try readAt(View, self, bind, scope);
    return if (value == .text) value.text else error.BindingTypeMismatch;
}
pub fn selected(comptime View: type, self: *View, node: *const ir.Node) !u32 {
    return selectedAt(View, self, node, null);
}
pub fn selectedAt(comptime View: type, self: *View, node: *const ir.Node, scope: ?*const Scope) !u32 {
    const value = try textAt(View, self, node, scope);
    for (node.options, 0..) |option, index| if (std.mem.eql(u8, option.value, value)) return @intCast(index);
    return error.UnknownSelection;
}

/// Validate only active branches and instantiated collection items.
pub fn validate(comptime View: type, self: *View, document: *const ir.Document) !void {
    for (document.root) |*node| try validateNode(View, self, node, null);
}
fn validateNode(comptime View: type, self: *View, node: *const ir.Node, scope: ?*const Scope) anyerror!void {
    if (node.on_click != null) {
        if (scope != null and !@hasDecl(View, "onCrepusScopedClick")) return error.MissingScopedClickHandler;
        if (scope == null and !@hasDecl(View, "onCrepusClick") and !@hasDecl(View, "onCrepusScopedClick")) return error.MissingClickHandler;
    }
    if (node.on_long_press != null and !@hasDecl(View, "onCrepusLongPress")) return error.MissingLongPressHandler;
    if ((node.on_change != null or (node.bind != null and node.kind != .text and node.kind != .forEach)) and !@hasDecl(View, "onCrepusChange")) return error.MissingChangeHandler;
    switch (node.kind) {
        .text => _ = try textAt(View, self, node, scope),
        .toggle, .checkbox => _ = try checkedAt(View, self, node, scope),
        .slider => _ = try numericAt(View, self, node, scope),
        .picker, .tabs => {
            _ = try selectedAt(View, self, node, scope);
            for (node.options) |option| if (option.icon) |icon| {
                if (icon.len == 0) return error.EmptyImageSource;
                if (std.mem.indexOf(u8, icon, "://") != null and !@hasDecl(View, "crepusImage")) return error.MissingImageResolver;
            };
        },
        .@"if" => {
            const children = if (try condition(View, self, node.condition, scope)) node.children else node.else_children;
            for (children) |*child| try validateNode(View, self, child, scope);
            return;
        },
        .forEach => {
            const items = try collection(View, self, node.bind.?, scope);
            for (items, 0..) |item, index| {
                const child_scope: Scope = .{ .parent = scope, .name = node.item_name, .key = item.key, .index = index, .value = item.value };
                for (node.children) |*child| try validateNode(View, self, child, &child_scope);
            }
            return;
        },
        .input => {
            const value = try textAt(View, self, node, scope);
            if (!std.unicode.utf8ValidateSlice(value)) return error.InvalidUtf8;
            if (value.len > 1024 * 1024) return error.InputTooLarge;
        },
        .webView => if (@import("builtin").os.tag != .macos and !@hasDecl(View, "crepusWebView")) return error.MissingWebViewFactory,
        .dropzone => if (node.on_drop != null and !@hasDecl(View, "onCrepusDrop")) return error.MissingDropHandler,
        .filePicker => if (node.on_pick != null and !@hasDecl(View, "onCrepusPick")) return error.MissingPickHandler,
        .link => if (!@hasDecl(View, "onCrepusLink")) return error.MissingLinkHandler,
        .image => if (std.mem.indexOf(u8, node.src, "://") != null and !@hasDecl(View, "crepusImage")) return error.MissingImageResolver,
        else => {},
    }
    for (node.children) |*child| try validateNode(View, self, child, scope);
    // Unselected tab panels are inactive (like inactive conditional branches).
    if (node.kind == .tabs) {
        const index = try selectedAt(View, self, node, scope);
        for (node.options[index].children) |*child| try validateNode(View, self, child, scope);
    }
}

/// Copy async event context; dialogs/timers must never retain a frame arena pointer.
pub fn cloneScope(a: std.mem.Allocator, source: ?*const Scope) anyerror!?*const Scope {
    const s = source orelse return null;
    const owned = try a.create(Scope);
    owned.* = .{ .parent = try cloneScope(a, s.parent), .name = try a.dupe(u8, s.name), .key = try a.dupe(u8, s.key), .index = s.index, .value = try cloneValue(a, s.value, 0) };
    return owned;
}
fn cloneValue(a: std.mem.Allocator, source: Value, depth: usize) anyerror!Value {
    if (depth >= 128) return error.BindingTooDeep;
    return switch (source) {
        .text => |s| .{ .text = try a.dupe(u8, s) },
        .boolean, .number => source,
        .record => |fields| blk: {
            const owned = try a.alloc(Field, fields.len);
            for (fields, owned) |field, *out| out.* = .{ .name = try a.dupe(u8, field.name), .value = try cloneValue(a, field.value, depth + 1) };
            break :blk .{ .record = owned };
        },
        .list => |items| blk: {
            const owned = try a.alloc(Item, items.len);
            for (items, owned) |item, *out| out.* = .{ .key = try a.dupe(u8, item.key), .value = try cloneValue(a, item.value, depth + 1) };
            break :blk .{ .list = owned };
        },
    };
}
