//! Acceptance of the built-in editor through the real ZPUI input-handler bridge.
const std = @import("std");
const zpui = @import("zpui");
const bridge = @import("crepuscularity_zpui");
const testing = std.testing;
const TestWindow = zpui.core.test_platform.TestWindow;
const View = struct {
    document: bridge.Document,
    value: std.ArrayList(u8) = .empty,
    changes: usize = 0,
    fn init(source: []const u8, value: []const u8, _: *zpui.Window, cx: *zpui.Context(View)) View {
        var view = View{ .document = bridge.Document.parse(cx.gpa(), source) catch @panic("invalid editor fixture") };
        view.value.appendSlice(cx.gpa(), value) catch @panic("OOM");
        return view;
    }
    pub fn deinit(self: *View, app: *zpui.App) void {
        self.document.deinit();
        self.value.deinit(app.gpa);
    }
    pub fn render(self: *View, window: *zpui.Window, cx: *zpui.Context(View)) zpui.AnyElement {
        return bridge.render(View, self, window, cx, &self.document, "edit") catch @panic("editor rejected");
    }
    pub fn crepusValue(self: *View, _: []const u8) ?bridge.Value {
        return .{ .text = self.value.items };
    }
    pub fn onCrepusChange(self: *View, change: bridge.Change, _: *zpui.Window, cx: *zpui.Context(View)) void {
        self.value.clearRetainingCapacity();
        self.value.appendSlice(cx.gpa(), change.value.text) catch @panic("OOM");
        self.changes += 1;
        cx.notify();
    }
    pub fn onCrepusClick(_: *View, _: []const u8, _: *const zpui.ClickEvent, _: *zpui.Window, _: *zpui.Context(View)) void {}
};
const Fixture = struct {
    app: *zpui.App,
    handle: zpui.WindowHandle(View),
    window: *zpui.Window,
    platform: *TestWindow,
    fn init(source: []const u8, value: []const u8) !Fixture {
        const app = try zpui.App.initTest(testing.allocator);
        errdefer app.deinit();
        const handle = try app.openWindow(.{ .bounds = .{ .origin = .zero, .size = .{ .width = 400, .height = 400 } } }, View, View.init, .{ source, value });
        const window = handle.window(app).?;
        const platform = TestWindow.of(window.platform_window);
        platform.simulateA11yActivation(true);
        var f: Fixture = .{ .app = app, .handle = handle, .window = window, .platform = platform };
        f.settle();
        return f;
    }
    fn settle(self: *Fixture) void {
        self.app.runUntilParked();
        self.window.drawAndPresent();
    }
    fn deinit(self: *Fixture) void {
        self.app.deinit();
    }
    fn model(self: *Fixture) *View {
        var lease = self.handle.rootView(self.app).?.lease(self.app);
        defer lease.end();
        return lease.value;
    }
    fn focus(self: *Fixture) !void {
        const tree = self.platform.a11yTree() orelse return error.MissingTree;
        for (tree.nodes.items) |n| if (n.role == .text_input or n.role == .password_input or n.role == .multiline_text_input) {
            self.platform.click(n.bounds.origin.x + 1, n.bounds.origin.y + 1);
            self.settle();
            return;
        };
        return error.MissingInput;
    }
    fn input(self: *Fixture) zpui.platform.InputHandler {
        return self.platform.input_handler orelse @panic("not focused");
    }
    fn key(self: *Fixture, name: []const u8, modifiers: zpui.input.Modifiers) void {
        _ = self.platform.simulateInput(.{ .key_down = .{ .keystroke = .{ .key = name, .modifiers = modifiers } } });
        self.settle();
    }
    fn typeText(self: *Fixture, value: []const u8) void {
        const handler = self.input();
        handler.vtable.replaceTextInRange(handler.ptr, null, value);
        self.settle();
    }
};
const simple =
    \\{"version":7,"root":[{"kind":"input","bind":"name","placeholder":"Name","onChange":"edit","style":{"width":200,"height":40}}]}
;

test "crepus-zpui built-in input commits typed Unicode and survives redraw with selection" {
    var f = try Fixture.init(simple, "A");
    defer f.deinit();
    try f.focus();
    f.key("end", .{});
    f.typeText("😀");
    try testing.expectEqualStrings("A😀", f.model().value.items);
    const handler = f.input();
    const selection = handler.vtable.selectedTextRange(handler.ptr).?;
    try testing.expectEqual(@as(usize, 3), selection.range.start); // UTF-16, not UTF-8 bytes.
    try testing.expectEqual(@as(usize, 3), selection.range.end);
    f.settle();
    f.key("backspace", .{});
    try testing.expectEqualStrings("A", f.model().value.items);
    try testing.expectEqual(@as(usize, 2), f.model().changes);
}

test "crepus-zpui deleting combining emoji family flags and Indic clusters is grapheme safe" {
    inline for (.{ "e\u{301}", "👩‍👩‍👧‍👦", "🇦🇺", "👍🏽", "क्‍ष" }) |cluster| {
        var f = try Fixture.init(simple, "A" ++ cluster);
        defer f.deinit();
        try f.focus();
        f.key("end", .{});
        f.key("backspace", .{});
        try testing.expectEqualStrings("A", f.model().value.items);
        f.typeText(cluster);
        f.key("home", .{});
        f.key("right", .{});
        f.key("delete", .{});
        try testing.expectEqualStrings("A", f.model().value.items);
    }
}

test "crepus-zpui IME preedit remains local and commit uses outward rounded UTF16 ranges" {
    var f = try Fixture.init(simple, "a😀b");
    defer f.deinit();
    try f.focus();
    var handler = f.input();
    handler.vtable.replaceAndMarkTextInRange(handler.ptr, .{ .start = 2, .end = 2 }, "e\u{301}", .{ .start = 2, .end = 2 });
    f.settle();
    try testing.expectEqualStrings("a😀b", f.model().value.items);
    try testing.expectEqual(@as(usize, 0), f.model().changes);
    handler = f.input();
    const marked = handler.vtable.markedTextRange(handler.ptr).?;
    try testing.expectEqual(@as(usize, 1), marked.start);
    try testing.expectEqual(@as(usize, 3), marked.end);
    handler.vtable.unmarkText(handler.ptr);
    f.settle();
    try testing.expectEqualStrings("ae\u{301}b", f.model().value.items);
    try testing.expectEqual(@as(usize, 1), f.model().changes);
    handler = f.input();
    try testing.expect(handler.vtable.markedTextRange(handler.ptr) == null);
    var output: std.ArrayList(u8) = .empty;
    defer output.deinit(testing.allocator);
    _ = handler.vtable.textForRange(handler.ptr, .{ .start = 1, .end = 3 }, &output, testing.allocator);
    try testing.expectEqualStrings("e\u{301}", output.items);
}

test "crepus-zpui selection clipboard and single line newline filtering update the host" {
    var f = try Fixture.init(simple, "Alpha");
    defer f.deinit();
    try f.focus();
    const shortcut: zpui.input.Modifiers = if (@import("builtin").os.tag == .macos) .{ .platform = true } else .{ .control = true };
    f.key("a", shortcut);
    f.key("c", shortcut);
    const copied = f.app.platform.vtable.readClipboard(f.app.platform.ptr, testing.allocator).?;
    defer testing.allocator.free(copied);
    try testing.expectEqualStrings("Alpha", copied);
    f.app.platform.vtable.writeClipboard(f.app.platform.ptr, "B\r\neta");
    f.key("v", shortcut);
    try testing.expectEqualStrings("Beta", f.model().value.items);
    f.key("a", shortcut);
    f.key("x", shortcut);
    try testing.expectEqualStrings("", f.model().value.items);
}

test "crepus-zpui secure multiline input masks accessibility and blocks plaintext extraction" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"input","bind":"name","placeholder":"Password","secure":true,"multiline":true,"style":{"width":200,"height":120}}]}
    , "s😀e\u{301}");
    defer f.deinit();
    try f.focus();
    const tree = f.platform.a11yTree().?;
    var found = false;
    for (tree.nodes.items) |node| if (node.role == .password_input) {
        found = true;
        try testing.expectEqualStrings("•••", tree.str(node.value).?);
    };
    try testing.expect(found);
    const handler = f.input();
    var output: std.ArrayList(u8) = .empty;
    defer output.deinit(testing.allocator);
    try testing.expect(handler.vtable.textForRange(handler.ptr, .{ .start = 0, .end = 4 }, &output, testing.allocator) == null);
    const shortcut: zpui.input.Modifiers = if (@import("builtin").os.tag == .macos) .{ .platform = true } else .{ .control = true };
    f.app.platform.vtable.writeClipboard(f.app.platform.ptr, "keep");
    f.key("a", shortcut);
    f.key("c", shortcut);
    const copied = f.app.platform.vtable.readClipboard(f.app.platform.ptr, testing.allocator).?;
    defer testing.allocator.free(copied);
    try testing.expectEqualStrings("keep", copied);
    f.key("end", .{});
    f.key("enter", .{});
    f.typeText("second");
    try testing.expectEqualStrings("s😀e\u{301}\nsecond", f.model().value.items);
    f.key("home", .{});
    f.key("down", .{});
    try testing.expectEqual(@as(usize, 6), f.input().vtable.selectedTextRange(f.input().ptr).?.range.start);
    for (f.platform.a11yTree().?.nodes.items) |node| if (node.role == .password_input) {
        try testing.expectEqualStrings("•••\n••••••", f.platform.a11yTree().?.str(node.value).?);
    };
}

test "crepus-zpui external host update cancels composition and resets stale geometry safely" {
    var f = try Fixture.init(simple, "abc");
    defer f.deinit();
    try f.focus();
    const handler = f.input();
    handler.vtable.replaceAndMarkTextInRange(handler.ptr, null, "é", null);
    f.settle();
    f.model().value.clearRetainingCapacity();
    try f.model().value.appendSlice(testing.allocator, "X");
    f.settle();
    const current = f.input();
    try testing.expect(current.vtable.markedTextRange(current.ptr) == null);
    f.key("end", .{});
    f.typeText("!");
    try testing.expectEqualStrings("X!", f.model().value.items);
}

test "crepus-zpui multiline vertical navigation and drag outside the editor preserve selection" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"input","bind":"name","placeholder":"Body","multiline":true,"style":{"width":200,"height":120}}]}
    , "abc\ndef");
    defer f.deinit();
    try f.focus();
    f.key("home", .{});
    f.key("down", .{});
    var handler = f.input();
    try testing.expectEqual(@as(usize, 4), handler.vtable.selectedTextRange(handler.ptr).?.range.start);
    f.key("up", .{});
    handler = f.input();
    try testing.expectEqual(@as(usize, 0), handler.vtable.selectedTextRange(handler.ptr).?.range.start);
    _ = f.platform.simulateInput(.{ .mouse_down = .{ .button = .left, .position = .{ .x = 1, .y = 1 } } });
    _ = f.platform.simulateInput(.{ .mouse_move = .{ .position = .{ .x = 300, .y = 300 }, .pressed_button = .left } });
    _ = f.platform.simulateInput(.{ .mouse_up = .{ .button = .left, .position = .{ .x = 300, .y = 300 } } });
    f.settle();
    handler = f.input();
    const selected = handler.vtable.selectedTextRange(handler.ptr).?;
    try testing.expectEqual(@as(usize, 0), selected.range.start);
    try testing.expectEqual(@as(usize, 7), selected.range.end);
}

test "crepus-zpui empty edits do not dispatch host changes" {
    var f = try Fixture.init(simple, "ABC");
    defer f.deinit();
    try f.focus();
    f.key("home", .{});
    f.key("backspace", .{});
    f.key("end", .{});
    f.key("delete", .{});
    f.typeText("");
    const shortcut: zpui.input.Modifiers = if (@import("builtin").os.tag == .macos) .{ .platform = true } else .{ .control = true };
    f.key("x", shortcut);
    f.app.platform.vtable.writeClipboard(f.app.platform.ptr, "");
    f.key("v", shortcut);
    f.app.platform.vtable.writeClipboard(f.app.platform.ptr, "\r\n");
    f.key("v", shortcut);
    try testing.expectEqualStrings("ABC", f.model().value.items);
    try testing.expectEqual(@as(usize, 0), f.model().changes);
    f.typeText("D");
    try testing.expectEqualStrings("ABCD", f.model().value.items);
    try testing.expectEqual(@as(usize, 1), f.model().changes);
}
