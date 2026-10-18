//! Native file dialogs and external-path drops. Async contexts own their storage.
const std = @import("std");
const zpui = @import("zpui");
const ir = @import("view_ir");
const host = @import("host.zig");

pub fn extensions(comptime View: type, self: *View, accept: []const []const u8) ![]const []const u8 {
    if (comptime @hasDecl(View, "crepusAcceptExtensions")) {
        const resolved = try self.crepusAcceptExtensions(accept);
        for (resolved) |entry| if (entry.len == 0 or entry[0] == '.' or std.mem.indexOfAny(u8, entry, "/\\*;, \t\r\n") != null) return error.InvalidAcceptExtension;
        return resolved;
    }
    const result = try zpui.frameAllocator().alloc([]const u8, accept.len);
    for (accept, result) |entry, *out| {
        if (entry.len < 2 or entry[0] != '.' or std.mem.indexOfAny(u8, entry[1..], "/\\*;, \t\r\n") != null) return error.MissingMimeResolver;
        out.* = entry[1..];
    }
    return result;
}

pub fn matches(path: []const u8, filter: []const []const u8) bool {
    if (filter.len == 0) return true;
    for (filter) |extension| {
        if (path.len > extension.len and path[path.len - extension.len - 1] == '.' and std.ascii.eqlIgnoreCase(path[path.len - extension.len ..], extension)) return true;
    }
    return false;
}

pub fn drop(comptime View: type, self: *View, cx: *zpui.Context(View), element: zpui.StatefulDiv, node: *const ir.Node, scope: ?*const host.Scope, callbacks: std.mem.Allocator) !zpui.StatefulDiv {
    const Payload = struct { node: *const ir.Node, scope: ?*const host.Scope, extensions: []const []const u8 };
    const payload = try callbacks.create(Payload);
    payload.* = .{ .node = node, .scope = scope, .extensions = try ownExtensions(callbacks, try extensions(View, self, node.accept)) };
    if (node.on_drop == null) return element;
    const Callback = struct {
        fn run(view: *View, data: *const Payload, event: *const zpui.ExternalPaths, window: *zpui.Window, context: *zpui.Context(View)) void {
            const accepted = zpui.frameAllocator().alloc([]const u8, event.paths.len) catch @panic("OOM");
            var count: usize = 0;
            for (event.paths) |path| if (matches(path, data.extensions)) {
                accepted[count] = path;
                count += 1;
            };
            if (count > 0) {
                if (comptime @hasDecl(View, "onCrepusDrop")) view.onCrepusDrop(host.Files{ .handler = data.node.on_drop, .paths = accepted[0..count], .scope = data.scope }, window, context);
            }
        }
    };
    return element.onDrop(zpui.ExternalPaths, cx.listenerWith(payload, Callback.run));
}

// A callback may arrive after its window or App was destroyed. The token survives
// the App while a platform callback is outstanding and never dereferences a dead App.
const Lifetime = struct {
    app: ?*zpui.App,
    allocator: std.mem.Allocator,
    references: usize = 1,
    fn release(self: *Lifetime) void {
        self.references -= 1;
        if (self.references == 0) self.allocator.destroy(self);
    }
};
const Dialogs = struct {
    lifetime: *Lifetime,
    pub fn deinit(self: *Dialogs, _: *zpui.App) void {
        self.lifetime.app = null;
        self.lifetime.release();
    }
};
fn lifetime(app: *zpui.App) *Lifetime {
    if (app.tryGlobal(Dialogs)) |dialogs| return dialogs.lifetime;
    const token = app.gpa.create(Lifetime) catch @panic("OOM");
    token.* = .{ .app = app, .allocator = app.gpa };
    app.setGlobal(Dialogs{ .lifetime = token }) catch @panic("OOM");
    return token;
}

pub fn picker(comptime View: type, self: *View, window: *zpui.Window, cx: *zpui.Context(View), element: zpui.StatefulDiv, node: *const ir.Node, scope: ?*const host.Scope, callbacks: std.mem.Allocator) !zpui.StatefulDiv {
    const filter = try extensions(View, self, node.accept);
    if (node.on_pick == null) return element.role(.button).ariaLabel(node.text).child(node.text);
    if (window.app.platform.vtable.promptForPaths == null) return error.MissingFileDialogBackend;
    const Payload = struct { node: *const ir.Node, scope: ?*const host.Scope, filter: []const []const u8 };
    const payload = try callbacks.create(Payload);
    payload.* = .{ .node = node, .scope = scope, .filter = try ownExtensions(callbacks, filter) };
    const Job = struct {
        arena: std.heap.ArenaAllocator,
        token: *Lifetime,
        window: zpui.WindowId,
        view: zpui.WeakEntity(View),
        handler: []const u8,
        scope: ?*const host.Scope,
        filter: []const []const u8,
        multiple: bool,

        fn done(raw: ?*anyopaque, paths: ?[]const []const u8) void {
            const job: *@This() = @ptrCast(@alignCast(raw.?));
            const a = job.token.allocator;
            defer {
                job.arena.deinit();
                job.token.release();
                a.destroy(job);
            }
            const app = job.token.app orelse return;
            if (paths) |selected| {
                if (selected.len == 0) return;
                const J = @This();
                const Result = struct { job: *J, paths: []const []const u8 };
                const Callback = struct {
                    fn apply(view: *View, result: Result, w: *zpui.Window, context: *zpui.Context(View)) void {
                        if (comptime @hasDecl(View, "onCrepusPick")) view.onCrepusPick(host.Files{ .handler = result.job.handler, .paths = result.paths, .scope = result.job.scope }, w, context);
                    }
                    fn inWindow(result: Result, w: *zpui.Window, context_app: *zpui.App) void {
                        _ = result.job.view.update(context_app, apply, .{ result, w });
                    }
                };
                for (selected) |path| if (!matches(path, job.filter)) return;
                if (!job.multiple and selected.len > 1) return;
                _ = app.updateWindow(job.window, Result{ .job = job, .paths = selected }, Callback.inWindow);
            }
        }
    };
    const Callback = struct {
        fn click(_: *View, data: *const Payload, _: *const zpui.ClickEvent, w: *zpui.Window, context: *zpui.Context(View)) void {
            const a = context.gpa();
            const job = a.create(Job) catch @panic("OOM");
            job.arena = std.heap.ArenaAllocator.init(a);
            const owned = job.arena.allocator();
            const filter_copy = owned.alloc([]const u8, data.filter.len) catch @panic("OOM");
            for (data.filter, filter_copy) |extension, *out| out.* = owned.dupe(u8, extension) catch @panic("OOM");
            const token = lifetime(w.app);
            token.references += 1;
            job.* = .{ .arena = job.arena, .token = token, .window = w.id, .view = context.weakEntity(), .handler = owned.dupe(u8, data.node.on_pick.?) catch @panic("OOM"), .scope = host.cloneScope(owned, data.scope) catch @panic("invalid async scope"), .filter = filter_copy, .multiple = data.node.multiple };
            w.app.platform.promptForPaths(.{ .multiple = data.node.multiple, .extensions = filter_copy }, .{ .ctx = job, .func = Job.done });
        }
    };
    return element.role(.button).ariaLabel(node.text).child(node.text).cursorPointer().focusable().onClick(cx.listenerWith(payload, Callback.click));
}

fn ownExtensions(a: std.mem.Allocator, filter: []const []const u8) ![]const []const u8 {
    const owned = try a.alloc([]const u8, filter.len);
    for (filter, owned) |value, *out| out.* = try a.dupe(u8, value);
    return owned;
}
