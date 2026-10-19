//! Icon-bearing tabs use drawn radio buttons while retaining controlled selection.
const std = @import("std");
const zpui = @import("zpui");
const ir = @import("view_ir");
const host = @import("host.zig");
const images = @import("images.zig");

pub fn render(comptime View: type, self: *View, window: *zpui.Window, cx: *zpui.Context(View), node: *const ir.Node, identity: []const u8, scope: ?*const host.Scope, callbacks: std.mem.Allocator) !zpui.AnyElement {
    const selected = try host.selectedAt(View, self, node, scope);
    var row = zpui.div().id(identity).role(.group).ariaLabel(node.bind.?).flex().flexRow().gap(zpui.px(4));
    const Payload = struct { node: *const ir.Node, scope: ?*const host.Scope, index: usize };
    const Callback = struct {
        fn apply(view: *View, data: *const Payload, index: usize, w: *zpui.Window, context: *zpui.Context(View)) void {
            if (comptime @hasDecl(View, "onCrepusChange")) view.onCrepusChange(host.Change{ .bind = data.node.bind, .handler = data.node.on_change, .value = .{ .text = data.node.options[index].value }, .scope = data.scope }, w, context);
        }
        fn click(view: *View, data: *const Payload, _: *const zpui.ClickEvent, w: *zpui.Window, context: *zpui.Context(View)) void {
            apply(view, data, data.index, w, context);
        }
        fn key(view: *View, data: *const Payload, event: *const zpui.input.KeyDownEvent, w: *zpui.Window, context: *zpui.Context(View)) void {
            const current = host.selectedAt(View, view, data.node, data.scope) catch return;
            const count = data.node.options.len;
            const index = if (std.mem.eql(u8, event.keystroke.key, "left")) (if (current == 0) count - 1 else current - 1) else if (std.mem.eql(u8, event.keystroke.key, "right")) (current + 1) % count else if (std.mem.eql(u8, event.keystroke.key, "home")) 0 else if (std.mem.eql(u8, event.keystroke.key, "end")) count - 1 else return;
            apply(view, data, index, w, context);
            context.stopPropagation();
        }
    };
    for (node.options, 0..) |option, index| {
        const payload = try callbacks.create(Payload);
        payload.* = .{ .node = node, .scope = scope, .index = index };
        var button = zpui.div().id(zpui.fmt("{s}/option/{d}", .{ identity, index })).role(.radio_button).ariaLabel(option.label).ariaToggled(index == selected)
            .focusable().tabIndex(if (index == selected) 0 else -1).cursorPointer().flex().flexRow().itemsCenter().gap(zpui.px(4)).px(zpui.px(8)).py(zpui.px(4)).rounded(zpui.px(4))
            .bg(zpui.rgb(if (index == selected) @as(u32, 0xbdd7ff) else 0xe2e8f0)).onClick(cx.listenerWith(payload, Callback.click)).onKeyDown(cx.listenerWith(payload, Callback.key));
        if (option.icon) |src| {
            const source: zpui.ImageSource = if (comptime @hasDecl(View, "crepusImage")) self.crepusImage(src) orelse return error.UnknownImageSource else zpui.ImageSource.fromString(src);
            if (source == .uri) return error.UnresolvedImageUri;
            if ((source == .embedded or source == .path) and zpui.images.services(window.app).svg.assets == null) return error.MissingAssetSource;
            if (zpui.images.loadImage(window.app, source)) |ready| button = button.child(try images.render(ready, .{ .width = 16, .height = 16, .objectFit = .contain }));
        }
        row = row.child(button.child(option.label));
    }
    return zpui.intoAnyElement(row);
}
