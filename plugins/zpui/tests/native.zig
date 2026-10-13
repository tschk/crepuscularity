const std = @import("std");
const zpui = @import("zpui");
const bridge = @import("crepuscularity_zpui");
const Counter = @import("counter").Counter;
const TestWindow = zpui.core.test_platform.TestWindow;
const testing = std.testing;

const Fixture = struct {
    app: *zpui.App,
    handle: zpui.WindowHandle(Counter),
    window: *zpui.Window,
    platform: *TestWindow,

    fn init(json: []const u8) !Fixture {
        const app = try zpui.App.initTest(testing.allocator);
        errdefer app.deinit();
        const handle = try app.openWindow(.{
            .bounds = .{ .origin = .zero, .size = .{ .width = 400, .height = 300 } },
        }, Counter, build, .{json});
        const window = handle.window(app).?;
        const platform = TestWindow.of(window.platform_window);
        platform.simulateA11yActivation(true);
        var fixture = Fixture{ .app = app, .handle = handle, .window = window, .platform = platform };
        fixture.settle();
        return fixture;
    }

    fn build(json: []const u8, _: *zpui.Window, cx: *zpui.Context(Counter)) Counter {
        return Counter.init(cx.gpa(), json) catch @panic("invalid counter fixture");
    }

    fn settle(self: *Fixture) void {
        self.app.runUntilParked();
        self.window.drawAndPresent();
    }

    fn deinit(self: *Fixture) void {
        self.app.deinit();
    }

    fn button(self: *Fixture, label: []const u8) !zpui.a11y.Node {
        const tree = self.platform.a11yTree() orelse return error.MissingAccessibilityTree;
        for (tree.nodes.items) |node| {
            if (node.role == .button and std.mem.eql(u8, tree.str(node.label) orelse "", label)) return node;
        }
        return error.MissingButton;
    }

    fn click(self: *Fixture, label: []const u8) !void {
        const node = try self.button(label);
        self.platform.click(node.bounds.origin.x + node.bounds.size.width / 2, node.bounds.origin.y + node.bounds.size.height / 2);
        self.settle();
    }

    fn count(self: *Fixture) usize {
        var lease = self.handle.rootView(self.app).?.lease(self.app);
        defer lease.end();
        return lease.value.count;
    }
};

test "crepus-zpui compiled counter dispatches real mouse and keyboard clicks" {
    var fixture = try Fixture.init(@embedFile("fixtures/counter.json"));
    defer fixture.deinit();
    try testing.expectEqual(@as(usize, 0), fixture.count());
    try fixture.click("Increment");
    try testing.expectEqual(@as(usize, 1), fixture.count());
    const tree = fixture.platform.a11yTree().?;
    try testing.expect(tree.focus_node != null);
    fixture.platform.typeKey("space");
    fixture.settle();
    try testing.expectEqual(@as(usize, 2), fixture.count());
    fixture.platform.typeKey("enter");
    fixture.settle();
    try testing.expectEqual(@as(usize, 3), fixture.count());
}

test "crepus-zpui applies sizing and color to actual element layout" {
    var fixture = try Fixture.init(
        \\{"version":7,"root":[{"kind":"button","label":"Sized","onClick":"increment","style":{"width":120,"height":40,"backgroundColor":"#123456","foregroundColor":"#FFFFFF","borderColor":"#000000","borderWidth":1}}]}
    );
    defer fixture.deinit();
    const button = try fixture.button("Sized");
    try testing.expectEqual(@as(f32, 120), button.bounds.size.width);
    try testing.expectEqual(@as(f32, 40), button.bounds.size.height);
    var found_background = false;
    var found_border = false;
    for (fixture.window.rendered_frame.scene.quads.items) |quad| {
        if (std.meta.eql(quad.background, zpui.Background.fromRgba(zpui.rgb(0x123456)))) {
            found_background = true;
            try testing.expectEqual(@as(f32, 120), quad.bounds.size.width);
            try testing.expectEqual(@as(f32, 40), quad.bounds.size.height);
        }
        // ZPUI emits the background and border in separate paint passes.
        if (quad.bounds.size.width == 120 and quad.bounds.size.height == 40 and quad.border_widths.top == 1) {
            try testing.expect(std.meta.eql(quad.border_color, zpui.rgb(0).toHsla()));
            found_border = true;
        }
    }
    try testing.expect(found_background);
    try testing.expect(found_border);
    try fixture.click("Sized");
    try testing.expectEqual(@as(usize, 1), fixture.count());
}

test "crepus-zpui scroll wheel moves children on both supported axes" {
    const snapshots = [_][]const u8{
        \\{"version":7,"root":[{"kind":"scroll","axis":"column","style":{"width":200,"height":60},"children":[{"kind":"button","label":"One","style":{"height":30}},{"kind":"button","label":"Two","style":{"height":30}},{"kind":"button","label":"Three","style":{"height":30}},{"kind":"button","label":"Four","style":{"height":30}}]}]}
        ,
        \\{"version":7,"root":[{"kind":"scroll","axis":"row","style":{"width":60,"height":40},"children":[{"kind":"button","label":"One","style":{"width":30}},{"kind":"button","label":"Two","style":{"width":30}},{"kind":"button","label":"Three","style":{"width":30}},{"kind":"button","label":"Four","style":{"width":30}}]}]}
        ,
    };
    for (snapshots, 0..) |snapshot, axis| {
        var fixture = try Fixture.init(snapshot);
        defer fixture.deinit();
        const before = try fixture.button("One");
        _ = fixture.platform.simulateInput(.{ .scroll_wheel = .{
            .position = .{ .x = 10, .y = 10 },
            .delta = .{ .pixels = if (axis == 0) .{ .x = 0, .y = -20 } else .{ .x = -20, .y = 0 } },
        } });
        fixture.settle();
        const after = try fixture.button("One");
        try testing.expect(if (axis == 0) after.bounds.origin.y < before.bounds.origin.y else after.bounds.origin.x < before.bounds.origin.x);
    }
}

test "crepus-zpui replaces a document and retires it after the old frame" {
    var fixture = try Fixture.init(@embedFile("fixtures/counter.json"));
    defer fixture.deinit();
    var replacement = try bridge.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"button","label":"Replacement","onClick":"increment"}]}
    );
    var retired: bridge.Document = undefined;
    {
        var lease = fixture.handle.rootView(fixture.app).?.lease(fixture.app);
        defer lease.end();
        retired = lease.value.document;
        lease.value.document = replacement;
        replacement = undefined;
    }
    fixture.window.refresh();
    fixture.settle();
    retired.deinit();
    try fixture.click("Replacement");
    try testing.expectEqual(@as(usize, 1), fixture.count());
    fixture.platform.typeKey("space");
    fixture.settle();
    try testing.expectEqual(@as(usize, 2), fixture.count());
}

comptime {
    _ = @import("coverage_native.zig");
}
