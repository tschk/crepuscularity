//! Render-owned callback memory survives presentation and frame-arena trimming.
const std = @import("std");
const zpui = @import("zpui");

pub const Storage = struct {
    current: ?std.heap.ArenaAllocator = null,
    previous: ?std.heap.ArenaAllocator = null,

    pub fn init(_: *zpui.App) Storage {
        return .{};
    }
    pub fn deinit(self: *Storage, _: *zpui.App) void {
        if (self.current) |*arena| arena.deinit();
        if (self.previous) |*arena| arena.deinit();
    }
    pub fn commit(self: *Storage, arena: std.heap.ArenaAllocator) void {
        // The preceding presented frame can still dispatch while the next is built.
        // Event callbacks lease their view, preventing synchronous render reentry.
        if (self.previous) |*old| old.deinit();
        self.previous = self.current;
        self.current = arena;
    }
};
