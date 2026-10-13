//! Hold recognition with copied context, movement cancellation, and entity-owned timers.
const std = @import("std");
const zpui = @import("zpui");
const host = @import("host.zig");
const input = zpui.input;

pub fn State(comptime View: type) type {
    return struct {
        const Self = @This();
        arena: std.heap.ArenaAllocator,
        view: ?zpui.WeakEntity(View) = null,
        window: zpui.WindowId = @fromBackingInt(@intCast(0)),
        handler: []const u8 = "",
        scope: ?*const host.Scope = null,
        down: input.MouseDownEvent = .{ .button = .left, .position = .zero },
        fired: bool = false,
        task: zpui.Task(void) = .none,

        fn init(app: *zpui.App) Self {
            return .{ .arena = std.heap.ArenaAllocator.init(app.gpa) };
        }
        pub fn deinit(self: *Self, _: *zpui.App) void {
            self.task.cancel();
            self.arena.deinit();
        }
        fn fire(self: *Self, cx: *zpui.Context(Self)) void {
            self.task.detach();
            self.fired = true;
            const Callback = struct {
                fn call(view: *View, state: *Self, w: *zpui.Window, context: *zpui.Context(View)) void {
                    if (comptime @hasDecl(View, "onCrepusLongPress")) view.onCrepusLongPress(state.handler, state.scope, &state.down, w, context);
                }
                fn inWindow(state: *Self, w: *zpui.Window, app: *zpui.App) void {
                    _ = state.view.?.update(app, call, .{ state, w });
                }
            };
            _ = cx.app.updateWindow(self.window, self, Callback.inWindow);
        }
        fn up(self: *Self, _: *const input.MouseUpEvent, _: *zpui.Window, _: *zpui.Context(Self)) void {
            self.task.cancel();
        }
        fn move(self: *Self, event: *const input.MouseMoveEvent, _: *zpui.Window, _: *zpui.Context(Self)) void {
            const dx = event.position.x - self.down.position.x;
            const dy = event.position.y - self.down.position.y;
            if (dx * dx + dy * dy > 64) self.task.cancel();
        }
        fn outside(self: *Self, _: *const input.MouseDownEvent, _: *zpui.Window, _: *zpui.Context(Self)) void {
            self.task.cancel();
        }
    };
}

pub fn attach(comptime View: type, window: *zpui.Window, cx: *zpui.Context(View), initial: zpui.StatefulDiv, identity: []const u8, handler: []const u8, scope: ?*const host.Scope) !zpui.StatefulDiv {
    const S = State(View);
    const gid = window.pushElementId(.from(zpui.fmt("{s}/hold", .{identity})));
    defer window.popElementId();
    const entity = window.useKeyedState(S, gid, S.init);
    const Payload = struct { entity: zpui.Entity(S), view: zpui.WeakEntity(View), handler: []const u8, scope: ?*const host.Scope };
    const payload = try zpui.frameAllocator().create(Payload);
    payload.* = .{ .entity = entity, .view = cx.weakEntity(), .handler = handler, .scope = scope };
    const Callback = struct {
        fn down(_: *View, data: *const Payload, event: *const input.MouseDownEvent, w: *zpui.Window, _: *zpui.Context(View)) void {
            var lease = data.entity.lease(w.app);
            defer lease.end();
            const state = lease.value;
            state.task.cancel();
            _ = state.arena.reset(.retain_capacity);
            const a = state.arena.allocator();
            state.handler = a.dupe(u8, data.handler) catch @panic("OOM");
            state.scope = host.cloneScope(a, data.scope) catch @panic("invalid async scope");
            state.view = data.view;
            state.window = w.id;
            state.down = event.*;
            state.fired = false;
            state.task = lease.cx.timer(500 * std.time.ns_per_ms, S.fire) catch @panic("OOM");
        }
    };
    var lease = entity.lease(window.app);
    defer lease.end();
    return initial.onMouseDown(.left, cx.listenerWith(payload, Callback.down))
        .onMouseUp(.left, lease.cx.listener(S.up)).onMouseUpOut(.left, lease.cx.listener(S.up))
        .onMouseMove(lease.cx.listener(S.move)).onMouseDownOut(lease.cx.listener(S.outside));
}
