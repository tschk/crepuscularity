//! Entity-owned timers survive redraws and are canceled on unmount/window close.
const std = @import("std");
const zpui = @import("zpui");
const ir = @import("view_ir");
const State = struct {
    index: usize = 0,
    count: usize = 0,
    delay_ns: u64 = 0,
    started: bool = false,

    fn init(_: *zpui.App) State {
        return .{};
    }
    fn tick(self: *State, cx: *zpui.Context(State)) void {
        self.index = (self.index + 1) % self.count;
        cx.notify();
        var task = cx.timer(self.delay_ns, tick) catch @panic("OOM");
        task.detach();
    }
};
pub fn text(window: *zpui.Window, node: *const ir.Node, identity: []const u8) []const u8 {
    var hash = std.hash.Wyhash.init(node.interval_ms);
    for (node.phrases) |phrase| {
        hash.update(std.mem.asBytes(&phrase.len));
        hash.update(phrase);
    }
    const key = zpui.fmt("{s}/rotation/{x}", .{ identity, hash.final() });
    const gid = window.pushElementId(.from(key));
    defer window.popElementId();
    const entity = window.useKeyedState(State, gid, State.init);
    const index = entity.read(window.app).index; // Subscribe this window to timer notifications.
    var lease = entity.lease(window.app);
    defer lease.end();
    if (!lease.value.started) {
        lease.value.count = node.phrases.len;
        lease.value.delay_ns = node.interval_ms * std.time.ns_per_ms;
        lease.value.started = true;
        var task = lease.cx.timer(lease.value.delay_ns, State.tick) catch @panic("OOM");
        task.detach();
    }
    return node.phrases[index];
}
