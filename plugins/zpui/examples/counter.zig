//! Import this view from a host configured with the pinned ZPUI build module.
const std = @import("std");
const zpui = @import("zpui");
const crepus = @import("crepuscularity_zpui");

pub const Counter = struct {
    document: crepus.Document,
    count: usize = 0,

    // Generate this snapshot with `crepus native ir counter.crepus`.
    pub fn init(allocator: std.mem.Allocator, json: []const u8) !Counter {
        return .{ .document = try crepus.Document.parse(allocator, json) };
    }

    pub fn deinit(self: *Counter, _: *zpui.App) void {
        self.document.deinit();
    }

    pub fn render(self: *Counter, window: *zpui.Window, cx: *zpui.Context(Counter)) zpui.Div {
        return zpui.div().flex().flexCol()
            .child(crepus.render(Counter, self, window, cx, &self.document, "counter"))
            .child(zpui.fmt("Count: {d}", .{self.count}));
    }

    pub fn onCrepusClick(self: *Counter, handler: []const u8, _: *const zpui.ClickEvent, _: *zpui.Window, cx: *zpui.Context(Counter)) void {
        if (std.mem.eql(u8, handler, "increment")) {
            self.count += 1;
            cx.notify();
        }
    }
};
