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
const CallbackStorage = @import("callback_storage.zig").Storage;
const rotation = @import("rotation.zig");
const editor = @import("editor.zig");
const images = @import("images.zig");
const translated = @import("translated.zig");
const mac_web = @import("mac_web.zig");
const typography = @import("typography.zig");
pub const MacBrowser = mac_web.Browser;
const files = @import("files.zig");
const long_press = @import("long_press.zig");
pub const Scope = host.Scope;
pub const Item = host.Item;
pub const Field = host.Field;
pub const Files = host.Files;
pub const Input = host.Input;
pub const WebView = host.WebView;

/// Validate host methods/values and construct elements. Namespace distinguishes instances.
/// Document and host-returned strings/images must outlive the rendered frame.
pub fn render(comptime View: type, self: *View, window: *zpui.Window, cx: *zpui.Context(View), document: *const Document, namespace: []const u8) !zpui.AnyElement {
    try validateHost(View, self, document);
    const callback_id = window.pushElementId(.from(zpui.fmt("{s}/crepus-private-callbacks", .{namespace})));
    const storage = window.useKeyedState(CallbackStorage, callback_id, CallbackStorage.init);
    window.popElementId();
    var lease = storage.lease(window.app);
    defer lease.end();
    var arena = std.heap.ArenaAllocator.init(window.app.gpa);
    errdefer arena.deinit();
    const callbacks = arena.allocator();
    var root = zpui.div().id(namespace);
    for (document.root, 0..) |*node, index| {
        const path = zpui.fmt("{s}/{d}", .{ namespace, index });
        root = root.child(try renderNode(View, self, window, cx, node, path, null, .normal, callbacks));
    }
    lease.value.commit(arena);
    return zpui.intoAnyElement(root);
}

fn renderNode(comptime View: type, self: *View, window: *zpui.Window, cx: *zpui.Context(View), node: *const ir.Node, path: []const u8, scope: ?*const host.Scope, inherited_case: typography.Case, callbacks: std.mem.Allocator) anyerror!zpui.AnyElement {
    const identity = if (node.style.id) |id| zpui.fmt("{s}/{s}", .{ path, id }) else path;
    const mode = node.style.textTransform orelse inherited_case;
    var element = zpui.div().id(identity);
    switch (node.kind) {
        .text => {
            const value = try typography.text(try host.textAt(View, self, node, scope), mode);
            element = element.role(.label).ariaLabel(value).child(value);
        },
        .button => {
            const label = try typography.text(node.text, mode);
            element = element.role(.button).ariaLabel(label).child(label);
            if (node.on_click) |handler| {
                const Payload = struct { handler: []const u8, scope: ?*const host.Scope };
                const payload = try callbacks.create(Payload);
                payload.* = .{ .handler = handler, .scope = scope };
                const Callback = struct {
                    fn click(view: *View, data: *const Payload, event: *const zpui.ClickEvent, w: *zpui.Window, context: *zpui.Context(View)) void {
                        if (comptime @hasDecl(View, "onCrepusScopedClick")) view.onCrepusScopedClick(data.handler, data.scope, event, w, context) else if (comptime @hasDecl(View, "onCrepusClick")) view.onCrepusClick(data.handler, event, w, context);
                    }
                };
                element = element.cursorPointer().focusable().onClick(cx.listenerWith(payload, Callback.click));
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
            element = try appendChildren(View, self, window, cx, element, node.children, identity, scope, mode, callbacks);
        },
        .toggle, .checkbox, .slider, .picker, .tabs => {
            element = element.flex().flexCol().itemsStart().child(try controls.render(View, self, window, cx, try typography.control(callbacks, node, mode), zpui.fmt("{s}/control", .{identity}), scope, callbacks));
            if (node.kind == .toggle and node.text.len > 0) element = element.child(try typography.text(node.text, mode));
            if (node.kind == .tabs) {
                const selected = try host.selectedAt(View, self, node, scope);
                const panel = zpui.div().id(zpui.fmt("{s}/panel", .{identity})).role(.tab_panel).ariaLabel(node.options[selected].label);
                element = element.child(try appendChildren(View, self, window, cx, panel, node.options[selected].children, zpui.fmt("{s}/tab/{d}", .{ identity, selected }), scope, mode, callbacks));
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
            element = element.role(.status).ariaLabel(try typography.text(node.text, mode)).px(zpui.px(6)).py(zpui.px(2)).rounded(zpui.px(4)).bg(zpui.rgb(color)).textColor(zpui.rgb(0xffffff).toHsla()).child(try typography.text(node.text, mode));
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
                element = element.child(try images.render(ready, node.style));
            } else if (node.placeholder) |label| element = element.child(try typography.text(label, mode));
        },
        .link => {
            const Callback = struct {
                fn open(view: *View, source: *const ir.Node, event: *const zpui.ClickEvent, w: *zpui.Window, context: *zpui.Context(View)) void {
                    if (comptime @hasDecl(View, "onCrepusLink")) view.onCrepusLink(Link{ .href = source.href, .target = source.target, .rel = source.rel }, event, w, context);
                }
            };
            element = element.role(.link).ariaLabel(try accessibleName(View, self, node, scope, mode)).cursorPointer().focusable().onClick(cx.listenerWith(node, Callback.open));
            element = try appendChildren(View, self, window, cx, element, node.children, identity, scope, mode, callbacks);
        },
        .list => {
            element = element.role(.list).flex().flexCol();
            for (node.children, 0..) |*child, index| {
                const child_path = zpui.fmt("{s}/{d}", .{ identity, index });
                const marker = if (node.ordered) zpui.fmt("{d}.", .{index + 1}) else "•";
                element = element.child(zpui.div().flex().flexRow().gap(zpui.px(6)).child(marker).child(try renderNode(View, self, window, cx, child, child_path, scope, mode, callbacks)));
            }
        },
        .@"if" => {
            const active = try host.condition(View, self, node.condition, scope);
            const children = if (active) node.children else node.else_children;
            const branch = zpui.fmt("{s}/{s}", .{ identity, if (active) "then" else "else" });
            element = try appendChildren(View, self, window, cx, element, children, branch, scope, mode, callbacks);
        },
        .forEach => {
            const items = try host.collection(View, self, node.bind.?, scope);
            for (items, 0..) |item, index| {
                const child_scope = try host.cloneScope(callbacks, &host.Scope{ .parent = scope, .name = node.item_name, .key = item.key, .index = index, .value = item.value });
                const item_path = zpui.fmt("{s}/key/{d}:{s}", .{ identity, item.key.len, item.key });
                const row = try appendChildren(View, self, window, cx, zpui.div().id(item_path), node.children, item_path, child_scope, mode, callbacks);
                element = element.child(row);
            }
        },
        .slotRotate => {
            const text = try typography.text(rotation.text(window, node, identity), mode);
            element = element.role(.label).ariaLabel(text).child(text);
        },
        .input => {
            if (mode != .normal) return error.InputTextTransformUnsupported;
            const request: host.Input = .{ .identity = identity, .bind = node.bind.?, .handler = node.on_change, .value = try host.textAt(View, self, node, scope), .placeholder = node.placeholder.?, .multiline = node.multiline, .secure = node.secure, .scope = scope };
            const input_element = if (comptime @hasDecl(View, "crepusInput")) try self.crepusInput(request, window, cx) else try editor.render(View, request, window, cx);
            element = element.child(input_element);
        },
        .webView => {
            if (!window.platform_window.supportsNativeViews()) return error.MissingNativeViewBackend;
            const request: host.WebView = .{ .identity = identity, .src = node.src, .scope = scope };
            const native = if (comptime @hasDecl(View, "crepusWebView")) try self.crepusWebView(request, window, cx) else try mac_web.render(request, window);
            var view = zpui.nativeViewWith(native, .{ .corner_radius = node.style.cornerRadius orelse 0 });
            view = view.wFull().hFull();
            element = element.child(view);
        },
        .dropzone => {
            element = try files.drop(View, self, cx, element.role(.group).ariaLabel(node.text), node, scope, callbacks);
            element = try appendChildren(View, self, window, cx, element, node.children, identity, scope, mode, callbacks);
        },
        .filePicker => element = try files.picker(View, self, window, cx, element, try typography.control(callbacks, node, mode), scope, callbacks),
        .listItem => element = try appendChildren(View, self, window, cx, element.role(.list_item), node.children, identity, scope, mode, callbacks),
    }
    if (node.on_long_press) |handler| element = try long_press.attach(View, window, cx, element, identity, handler, scope, callbacks);
    const styled = zpui.intoAnyElement(styles.apply(element, node.style));
    return if (node.style.translateX != null or node.style.translateY != null) translated.translate(styled, node.style.translateX orelse 0, node.style.translateY orelse 0) else styled;
}

fn appendChildren(comptime View: type, self: *View, window: *zpui.Window, cx: *zpui.Context(View), initial: zpui.StatefulDiv, children: []const ir.Node, path: []const u8, scope: ?*const host.Scope, mode: typography.Case, callbacks: std.mem.Allocator) !zpui.StatefulDiv {
    var element = initial;
    for (children, 0..) |*child, index| {
        element = element.child(try renderNode(View, self, window, cx, child, zpui.fmt("{s}/{d}", .{ path, index }), scope, mode, callbacks));
    }
    return element;
}

fn accessibleName(comptime View: type, self: *View, node: *const ir.Node, scope: ?*const host.Scope, mode: typography.Case) ![]const u8 {
    var name: std.ArrayList(u8) = .empty;
    try appendName(View, self, node.children, &name, scope, mode);
    return if (name.items.len > 0) name.items else node.href;
}

fn appendName(comptime View: type, self: *View, children: []const ir.Node, name: *std.ArrayList(u8), scope: ?*const host.Scope, inherited_case: typography.Case) anyerror!void {
    for (children) |*child| {
        if (child.style.hidden == true) continue;
        const mode = child.style.textTransform orelse inherited_case;
        if (child.kind == .@"if") {
            const active = if (try host.condition(View, self, child.condition, scope)) child.children else child.else_children;
            try appendName(View, self, active, name, scope, mode);
            continue;
        }
        if (child.kind == .forEach) {
            const items = try host.collection(View, self, child.bind.?, scope);
            for (items, 0..) |item, index| {
                const nested: host.Scope = .{ .parent = scope, .name = child.item_name, .key = item.key, .index = index, .value = item.value };
                try appendName(View, self, child.children, name, &nested, mode);
            }
            continue;
        }
        const raw: []const u8 = switch (child.kind) {
            .text => try host.textAt(View, self, child, scope),
            .button, .badge, .image, .toggle, .checkbox => child.text,
            else => "",
        };
        const label = try typography.text(raw, mode);
        if (label.len > 0) {
            if (name.items.len > 0) try name.append(zpui.frameAllocator(), ' ');
            try name.appendSlice(zpui.frameAllocator(), label);
        }
        try appendName(View, self, child.children, name, scope, mode);
    }
}
