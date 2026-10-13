//! Construct a frame from owned IR with explicit host binding and event contracts.
const std = @import("std");
const zpui = @import("zpui");
pub const ir = @import("view_ir");
pub const Document = ir.Document;
pub const host = @import("host.zig");
pub const Value = host.Value;
pub const Change = host.Change;
pub const Link = host.Link;
pub const validateHost = host.validate;
const styles = @import("style.zig");
const controls = @import("controls.zig");

/// Validate host methods/values and construct elements. Namespace distinguishes instances.
/// Document and host-returned strings/images must outlive the rendered frame.
pub fn render(comptime View: type, self: *View, window: *zpui.Window, cx: *zpui.Context(View), document: *const Document, namespace: []const u8) !zpui.AnyElement {
    try validateHost(View, self, document);
    var root = zpui.div().id(namespace);
    for (document.root, 0..) |*node, index| {
        const path = zpui.fmt("{s}/{d}", .{ namespace, index });
        root = root.child(try renderNode(View, self, window, cx, node, path));
    }
    return zpui.intoAnyElement(root);
}

fn renderNode(comptime View: type, self: *View, window: *zpui.Window, cx: *zpui.Context(View), node: *const ir.Node, path: []const u8) anyerror!zpui.AnyElement {
    const identity = if (node.style.id) |id| zpui.fmt("{s}/{s}", .{ path, id }) else path;
    var element = zpui.div().id(identity);
    switch (node.kind) {
        .text => {
            const value = try host.text(View, self, node);
            element = element.role(.label).ariaLabel(value).child(value);
        },
        .button => {
            element = element.role(.button).ariaLabel(node.text).child(node.text);
            if (node.on_click) |handler| {
                const Callback = struct {
                    fn click(view: *View, name: []const u8, event: *const zpui.ClickEvent, w: *zpui.Window, context: *zpui.Context(View)) void {
                        if (comptime @hasDecl(View, "onCrepusClick")) view.onCrepusClick(name, event, w, context);
                    }
                };
                element = element.cursorPointer().focusable().onClick(cx.listenerWith(handler, Callback.click));
            }
        },
        .stack, .scroll => {
            element = element.flex();
            element = if (node.axis == .row) element.flexRow() else element.flexCol();
            if (node.spacing) |v| element = element.gap(zpui.px(v));
            if (node.align_items) |v| element = styles.alignItems(element, v);
            if (node.justify_content) |v| element = styles.justify(element, v);
            if (node.kind == .scroll) {
                element = element.role(.scroll_view);
                element = if (node.axis == .row) element.overflowXScroll() else element.overflowYScroll();
            }
            element = try appendChildren(View, self, window, cx, element, node.children, identity);
        },
        .toggle, .checkbox, .slider, .picker, .tabs => {
            element = element.flex().flexCol().itemsStart().child(try controls.render(View, self, window, cx, node, zpui.fmt("{s}/control", .{identity})));
            if (node.kind == .toggle and node.text.len > 0) element = element.child(node.text);
            if (node.kind == .tabs) {
                const selected = try host.selected(View, self, node);
                const panel = zpui.div().id(zpui.fmt("{s}/panel", .{identity})).role(.tab_panel).ariaLabel(node.options[selected].label);
                element = element.child(try appendChildren(View, self, window, cx, panel, node.options[selected].children, zpui.fmt("{s}/tab/{d}", .{ identity, selected })));
            }
        },
        .progress, .meter => {
            const fraction: f32 = @floatCast((@as(f64, node.value) - node.min) / (@as(f64, node.max) - node.min));
            const fill = zpui.div().h(zpui.px(8)).w(styles.length(-1)).bg(zpui.rgb(0x2563eb));
            var track = zpui.div().w(zpui.px(160)).h(zpui.px(8)).bg(zpui.rgb(0xe2e8f0)).overflowHidden();
            if (node.style.width != null) track = track.wFull();
            track = track.child(fill.w(zpui.relative(fraction)));
            element = element.role(.progress_indicator).ariaLabel(node.text).ariaNumericValue(node.value).ariaMinNumericValue(node.min).ariaMaxNumericValue(node.max).child(track);
        },
        .badge => {
            const color: u32 = switch (node.tone) {
                .neutral => 0x64748b,
                .info => 0x2563eb,
                .success => 0x16a34a,
                .warning => 0xd97706,
                .@"error" => 0xdc2626,
            };
            element = element.role(.status).ariaLabel(node.text).px(zpui.px(6)).py(zpui.px(2)).rounded(zpui.px(4)).bg(zpui.rgb(color)).textColor(zpui.rgb(0xffffff).toHsla()).child(node.text);
        },
        .divider => {
            element = element.role(.separator).ariaOrientation(if (node.axis == .row) .horizontal else .vertical).bg(zpui.rgb(0x94a3b8)).flexNone();
            element = if (node.axis == .row) element.wFull().h(zpui.px(1)) else element.hFull().w(zpui.px(1));
        },
        .spacer => element = if (node.size) |v| element.size(zpui.px(v)).flexNone() else element.flexGrow(1),
        .image => {
            const source: zpui.ImageSource = if (comptime @hasDecl(View, "crepusImage"))
                self.crepusImage(node.src) orelse return error.UnknownImageSource
            else
                zpui.ImageSource.fromString(node.src);
            if (source == .uri) return error.UnresolvedImageUri;
            if ((source == .embedded or source == .path) and zpui.images.services(window.app).svg.assets == null) return error.MissingAssetSource;
            element = element.role(.image).ariaLabel(node.text);
            if (zpui.images.loadImage(window.app, source)) |ready| {
                var image = zpui.img(ready).objectFit(switch (node.style.objectFit orelse .contain) {
                    .contain => .contain,
                    .cover => .cover,
                    .fill => .fill,
                    .none => .none,
                    .@"scale-down" => .scale_down,
                });
                if (node.style.width) |v| image = image.w(styles.length(v));
                if (node.style.height) |v| image = image.h(styles.length(v));
                element = element.child(image);
            } else if (node.placeholder) |label| element = element.child(label);
        },
        .link => {
            const Callback = struct {
                fn open(view: *View, source: *const ir.Node, event: *const zpui.ClickEvent, w: *zpui.Window, context: *zpui.Context(View)) void {
                    if (comptime @hasDecl(View, "onCrepusLink")) view.onCrepusLink(Link{ .href = source.href, .target = source.target, .rel = source.rel }, event, w, context);
                }
            };
            element = element.role(.link).ariaLabel(try accessibleName(View, self, node)).cursorPointer().focusable().onClick(cx.listenerWith(node, Callback.open));
            element = try appendChildren(View, self, window, cx, element, node.children, identity);
        },
        .list => {
            element = element.role(.list).flex().flexCol();
            for (node.children, 0..) |*child, index| {
                const child_path = zpui.fmt("{s}/{d}", .{ identity, index });
                const marker = if (node.ordered) zpui.fmt("{d}.", .{index + 1}) else "•";
                element = element.child(zpui.div().flex().flexRow().gap(zpui.px(6)).child(marker).child(try renderNode(View, self, window, cx, child, child_path)));
            }
        },
        .listItem => element = try appendChildren(View, self, window, cx, element.role(.list_item), node.children, identity),
    }
    return zpui.intoAnyElement(styles.apply(element, node.style));
}

fn appendChildren(comptime View: type, self: *View, window: *zpui.Window, cx: *zpui.Context(View), initial: zpui.StatefulDiv, children: []const ir.Node, path: []const u8) !zpui.StatefulDiv {
    var element = initial;
    for (children, 0..) |*child, index| {
        element = element.child(try renderNode(View, self, window, cx, child, zpui.fmt("{s}/{d}", .{ path, index })));
    }
    return element;
}

fn accessibleName(comptime View: type, self: *View, node: *const ir.Node) ![]const u8 {
    var name: std.ArrayList(u8) = .empty;
    try appendName(View, self, node.children, &name);
    return if (name.items.len > 0) name.items else node.href;
}

fn appendName(comptime View: type, self: *View, children: []const ir.Node, name: *std.ArrayList(u8)) anyerror!void {
    for (children) |*child| {
        if (child.style.hidden == true) continue;
        const label: []const u8 = switch (child.kind) {
            .text => try host.text(View, self, child),
            .button, .badge, .image, .toggle, .checkbox => child.text,
            else => "",
        };
        if (label.len > 0) {
            if (name.items.len > 0) try name.append(zpui.frameAllocator(), ' ');
            try name.appendSlice(zpui.frameAllocator(), label);
        }
        try appendName(View, self, child.children, name);
    }
}
