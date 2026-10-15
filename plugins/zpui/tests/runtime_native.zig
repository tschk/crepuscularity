//! Runtime contracts exercised against the pinned backend, with a host-controlled model.
const std = @import("std");
const zpui = @import("zpui");
const bridge = @import("crepuscularity_zpui");
const testing = std.testing;
const TestWindow = zpui.core.test_platform.TestWindow;

const rows = [_]bridge.Item{
    .{ .key = "first", .value = .{ .record = &.{ .{ .name = "title", .value = .{ .text = "First" } }, .{ .name = "active", .value = .{ .boolean = true } } } } },
    .{ .key = "second", .value = .{ .record = &.{ .{ .name = "title", .value = .{ .text = "Second" } }, .{ .name = "active", .value = .{ .boolean = false } } } } },
};
const View = struct {
    document: bridge.Document,
    ready: bool = true,
    reverse: bool = false,
    reversed: [2]bridge.Item = .{ rows[1], rows[0] },
    clicks: usize = 0,
    holds: usize = 0,
    dropped: usize = 0,
    picked: usize = 0,
    last_key: ?[]u8 = null,
    native: ?zpui.NativeViewId = null,
    dummy_native: u8 = 0,
    input_flags: bool = false,
    web_src_seen: bool = false,

    fn init(source: []const u8, _: *zpui.Window, cx: *zpui.Context(View)) View {
        return .{ .document = bridge.Document.parse(cx.gpa(), source) catch @panic("invalid runtime fixture") };
    }
    pub fn deinit(self: *View, app: *zpui.App) void {
        self.document.deinit();
        if (self.last_key) |key| app.gpa.free(key);
    }
    pub fn render(self: *View, window: *zpui.Window, cx: *zpui.Context(View)) zpui.AnyElement {
        return bridge.render(View, self, window, cx, &self.document, "runtime") catch @panic("runtime host rejected");
    }
    pub fn crepusValue(self: *View, name: []const u8) ?bridge.Value {
        if (std.mem.eql(u8, name, "ready")) return .{ .boolean = self.ready };
        if (std.mem.eql(u8, name, "rows")) return .{ .list = if (self.reverse) &self.reversed else &rows };
        if (std.mem.eql(u8, name, "name")) return .{ .text = "Ada" };
        return null;
    }
    pub fn onCrepusChange(_: *View, _: bridge.Change, _: *zpui.Window, _: *zpui.Context(View)) void {}
    pub fn onCrepusScopedClick(self: *View, _: []const u8, scope: ?*const bridge.Scope, _: *const zpui.ClickEvent, _: *zpui.Window, cx: *zpui.Context(View)) void {
        self.clicks += 1;
        if (scope) |s| self.recordKey(s.key, cx.gpa());
        cx.notify();
    }
    pub fn onCrepusLongPress(self: *View, _: []const u8, scope: ?*const bridge.Scope, _: *const zpui.input.MouseDownEvent, _: *zpui.Window, cx: *zpui.Context(View)) void {
        self.holds += 1;
        if (scope) |s| self.recordKey(s.key, cx.gpa());
        cx.notify();
    }
    pub fn onCrepusDrop(self: *View, event: bridge.Files, _: *zpui.Window, cx: *zpui.Context(View)) void {
        self.dropped += event.paths.len;
        if (event.scope) |s| self.recordKey(s.key, cx.gpa());
        cx.notify();
    }
    pub fn onCrepusPick(self: *View, event: bridge.Files, _: *zpui.Window, cx: *zpui.Context(View)) void {
        self.picked += event.paths.len;
        if (event.scope) |s| self.recordKey(s.key, cx.gpa());
        cx.notify();
    }
    fn recordKey(self: *View, key: []const u8, a: std.mem.Allocator) void {
        if (self.last_key) |old| a.free(old);
        self.last_key = a.dupe(u8, key) catch @panic("OOM");
    }
    pub fn onCrepusLink(_: *View, _: bridge.Link, _: *const zpui.ClickEvent, _: *zpui.Window, _: *zpui.Context(View)) void {}
    // Contract witnesses only. These do not claim a native editor/browser implementation.
    pub fn crepusInput(self: *View, request: bridge.Input, _: *zpui.Window, _: *zpui.Context(View)) !zpui.AnyElement {
        self.input_flags = request.multiline and request.secure;
        return zpui.intoAnyElement(zpui.div().id(request.identity).role(.text_input).ariaValue(request.value).ariaPlaceholder(request.placeholder).child(request.value));
    }
    pub fn crepusWebView(self: *View, request: bridge.WebView, window: *zpui.Window, _: *zpui.Context(View)) !zpui.NativeViewId {
        self.web_src_seen = std.mem.eql(u8, request.src, "https://example.com");
        if (self.native == null) self.native = try window.attachNativeView(@ptrCast(&self.dummy_native), .{});
        return self.native.?;
    }
};

const Fixture = struct {
    app: *zpui.App,
    handle: zpui.WindowHandle(View),
    window: *zpui.Window,
    platform: *TestWindow,
    fn init(source: []const u8) !Fixture {
        return initWithDialogs(source, false);
    }
    fn initWithDialogs(source: []const u8, dialogs: bool) !Fixture {
        const app = try zpui.App.initTest(testing.allocator);
        errdefer app.deinit();
        if (dialogs) {
            dialog_vtable = app.platform.vtable.*;
            dialog_vtable.promptForPaths = recordPrompt;
            app.platform.vtable = &dialog_vtable;
        }
        const handle = try app.openWindow(.{ .bounds = .{ .origin = .zero, .size = .{ .width = 400, .height = 400 } } }, View, View.init, .{source});
        const window = handle.window(app).?;
        const platform = TestWindow.of(window.platform_window);
        platform.simulateA11yActivation(true);
        var result: Fixture = .{ .app = app, .handle = handle, .window = window, .platform = platform };
        result.settle();
        return result;
    }
    fn deinit(self: *Fixture) void {
        self.app.deinit();
    }
    fn settle(self: *Fixture) void {
        self.app.runUntilParked();
        self.window.drawAndPresent();
    }
    fn model(self: *Fixture) *View {
        var lease = self.handle.rootView(self.app).?.lease(self.app);
        defer lease.end();
        return lease.value;
    }
    fn node(self: *Fixture, role: zpui.Role, label: []const u8) !zpui.a11y.Node {
        const tree = self.platform.a11yTree() orelse return error.MissingTree;
        for (tree.nodes.items) |n| if (n.role == role and std.mem.eql(u8, tree.str(n.label) orelse "", label)) return n;
        return error.MissingNode;
    }
    fn click(self: *Fixture, label: []const u8) !void {
        const n = try self.node(.button, label);
        self.platform.click(n.bounds.origin.x + n.bounds.size.width / 2, n.bounds.origin.y + n.bounds.size.height / 2);
        self.settle();
    }
};

test "crepus-zpui conditions skip inactive invalid bindings and switch branches" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"if","condition":"ready","thenChildren":[{"kind":"text","content":"Ready"}],"elseChildren":[{"kind":"text","content":"Wait"}]}]}
    );
    defer f.deinit();
    _ = try f.node(.label, "Ready");
    f.model().ready = false;
    f.settle();
    _ = try f.node(.label, "Wait");
    try testing.expectError(error.MissingNode, f.node(.label, "Ready"));
    var doc = try bridge.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"if","condition":"ready","thenChildren":[{"kind":"text","content":"X","bind":"unknown"}],"elseChildren":[]}]}
    );
    defer doc.deinit();
    try bridge.validateHost(View, f.model(), &doc);
    f.model().ready = true;
    try testing.expectError(error.UnknownBinding, bridge.validateHost(View, f.model(), &doc));
}

test "crepus-zpui collections resolve item fields scoped handlers and stable keys on reorder" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"forEach","bind":"rows","itemName":"row","itemBody":[{"kind":"if","condition":"row.active","thenChildren":[{"kind":"text","content":"","bind":"row.title"}],"elseChildren":[]},{"kind":"button","label":"Choose","onClick":"choose","style":{"height":30,"width":80}}]}]}
    );
    defer f.deinit();
    _ = try f.node(.label, "First");
    try testing.expectError(error.MissingNode, f.node(.label, "Second"));
    try f.click("Choose");
    try testing.expectEqualStrings("first", f.model().last_key.?);
    f.model().reverse = true;
    f.settle();
    try f.click("Choose");
    try testing.expectEqualStrings("second", f.model().last_key.?);
}

test "crepus-zpui collection duplicate and empty keys fail before rendering" {
    const Bad = struct {
        pub fn crepusValue(_: *@This(), _: []const u8) ?bridge.Value {
            return .{ .list = &.{ .{ .key = "same", .value = .{ .text = "A" } }, .{ .key = "same", .value = .{ .text = "B" } } } };
        }
    };
    var bad: Bad = .{};
    var doc = try bridge.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"forEach","bind":"rows","itemName":"row","itemBody":[]}]}
    );
    defer doc.deinit();
    try testing.expectError(error.DuplicateCollectionKey, bridge.validateHost(Bad, &bad, &doc));
}

test "crepus-zpui timed text advances once per interval and redraw does not restart it" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"slotRotate","phrases":["One","Two"],"intervalMs":100}]}
    );
    defer f.deinit();
    _ = try f.node(.label, "One");
    f.app.advanceClock(99 * std.time.ns_per_ms);
    f.settle();
    _ = try f.node(.label, "One");
    f.app.advanceClock(std.time.ns_per_ms);
    f.settle();
    _ = try f.node(.label, "Two");
    f.app.advanceClock(100 * std.time.ns_per_ms);
    f.settle();
    _ = try f.node(.label, "One");
    var baseline = try Fixture.init("{\"version\":7,\"root\":[]}");
    defer baseline.deinit();
    baseline.app.advanceClock(200 * std.time.ns_per_ms);
    baseline.settle();
    try testing.expectEqual(baseline.app.test_platform.?.test_dispatcher.pendingTimers() + 1, f.app.test_platform.?.test_dispatcher.pendingTimers());
}

test "crepus-zpui timer cancels when conditional content unmounts" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"if","condition":"ready","thenChildren":[{"kind":"slotRotate","phrases":["One","Two"],"intervalMs":100}],"elseChildren":[]}]}
    );
    defer f.deinit();
    f.model().ready = false;
    f.settle();
    f.settle();
    f.app.advanceClock(100 * std.time.ns_per_ms);
    f.settle();
    try testing.expectError(error.MissingNode, f.node(.label, "Two"));
    var baseline = try Fixture.init("{\"version\":7,\"root\":[]}");
    defer baseline.deinit();
    baseline.app.advanceClock(100 * std.time.ns_per_ms);
    baseline.settle();
    try testing.expectEqual(baseline.app.test_platform.?.test_dispatcher.pendingTimers(), f.app.test_platform.?.test_dispatcher.pendingTimers());
}

test "crepus-zpui external drops filter extensions and preserve scoped item context" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"forEach","bind":"rows","itemName":"row","itemBody":[{"kind":"dropzone","label":"Drop","accept":".png","onDrop":"drop","style":{"width":100,"height":60},"children":[]}]}]}
    );
    defer f.deinit();
    const paths = [_][]const u8{ "/tmp/yes.PNG", "/tmp/no.txt" };
    _ = f.platform.simulateInput(.{ .file_drop = .{ .entered = .{ .position = .{ .x = 10, .y = 10 }, .paths = &paths } } });
    _ = f.platform.simulateInput(.{ .file_drop = .{ .submit = .{ .position = .{ .x = 10, .y = 10 } } } });
    f.settle();
    try testing.expectEqual(@as(usize, 1), f.model().dropped);
    try testing.expectEqualStrings("first", f.model().last_key.?);
}

test "crepus-zpui long press fires after threshold and cancels on release or movement" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"button","label":"Hold","onLongPress":"hold","style":{"width":100,"height":60}}]}
    );
    defer f.deinit();
    _ = f.platform.simulateInput(.{ .mouse_down = .{ .button = .left, .position = .{ .x = 10, .y = 10 } } });
    f.app.advanceClock(499 * std.time.ns_per_ms);
    f.settle();
    try testing.expectEqual(@as(usize, 0), f.model().holds);
    f.app.advanceClock(std.time.ns_per_ms);
    f.settle();
    try testing.expectEqual(@as(usize, 1), f.model().holds);
    _ = f.platform.simulateInput(.{ .mouse_up = .{ .button = .left, .position = .{ .x = 10, .y = 10 } } });
    _ = f.platform.simulateInput(.{ .mouse_down = .{ .button = .left, .position = .{ .x = 10, .y = 10 } } });
    _ = f.platform.simulateInput(.{ .mouse_up = .{ .button = .left, .position = .{ .x = 10, .y = 10 } } });
    f.app.advanceClock(500 * std.time.ns_per_ms);
    f.settle();
    try testing.expectEqual(@as(usize, 1), f.model().holds);
    _ = f.platform.simulateInput(.{ .mouse_down = .{ .button = .left, .position = .{ .x = 10, .y = 10 } } });
    _ = f.platform.simulateInput(.{ .mouse_move = .{ .position = .{ .x = 30, .y = 10 }, .pressed_button = .left } });
    f.app.advanceClock(500 * std.time.ns_per_ms);
    f.settle();
    try testing.expectEqual(@as(usize, 1), f.model().holds);
}

test "crepus-zpui long press consumes the release so the click handler stays silent" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"button","label":"Hold","onLongPress":"hold","onClick":"tap","style":{"width":100,"height":60}}]}
    );
    defer f.deinit();
    _ = f.platform.simulateInput(.{ .mouse_down = .{ .button = .left, .position = .{ .x = 10, .y = 10 } } });
    f.app.advanceClock(500 * std.time.ns_per_ms);
    f.settle();
    try testing.expectEqual(@as(usize, 1), f.model().holds);
    try testing.expectEqual(@as(usize, 0), f.model().clicks);
    // Releasing after the hold dispatched must not also fire the click handler.
    _ = f.platform.simulateInput(.{ .mouse_up = .{ .button = .left, .position = .{ .x = 10, .y = 10 } } });
    f.settle();
    try testing.expectEqual(@as(usize, 0), f.model().clicks);
    try testing.expectEqual(@as(usize, 1), f.model().holds);
    // A later quick tap still clicks exactly once.
    _ = f.platform.simulateInput(.{ .mouse_down = .{ .button = .left, .position = .{ .x = 10, .y = 10 } } });
    _ = f.platform.simulateInput(.{ .mouse_up = .{ .button = .left, .position = .{ .x = 10, .y = 10 } } });
    f.settle();
    try testing.expectEqual(@as(usize, 1), f.model().clicks);
    try testing.expectEqual(@as(usize, 1), f.model().holds);
}

test "crepus-zpui built-in input and absent browser factory validation" {
    const Missing = struct {
        pub fn crepusValue(_: *@This(), _: []const u8) ?bridge.Value {
            return .{ .text = "" };
        }
        pub fn onCrepusChange(_: *@This(), _: bridge.Change, _: *zpui.Window, _: *zpui.Context(@This())) void {}
    };
    var missing: Missing = .{};
    var input_doc = try bridge.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"input","bind":"name","placeholder":"Name"}]}
    );
    defer input_doc.deinit();
    try bridge.validateHost(Missing, &missing, &input_doc); // Built-in editor now supplies this contract.
    var web_doc = try bridge.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"webView","src":"https://example.com"}]}
    );
    defer web_doc.deinit();
    if (@import("builtin").os.tag == .macos) try bridge.validateHost(Missing, &missing, &web_doc) else try testing.expectError(error.MissingWebViewFactory, bridge.validateHost(Missing, &missing, &web_doc));
}

test "crepus-zpui gradient paints native angle and both stops" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"text","content":"Gradient","style":{"width":100,"height":60,"backgroundGradientDirection":"to-br","backgroundGradientFrom":"#123456","backgroundGradientTo":"#ABCDEF"}}]}
    );
    defer f.deinit();
    var found = false;
    for (f.window.rendered_frame.scene.quads.items) |quad| if (quad.background.tag == .linear_gradient) {
        found = true;
        try testing.expectEqual(@as(f32, 135), quad.background.gradient_angle_or_pattern_height);
        try testing.expect(std.meta.eql(zpui.rgb(0x123456).toHsla(), quad.background.colors[0].color));
        try testing.expect(std.meta.eql(zpui.rgb(0xabcdef).toHsla(), quad.background.colors[1].color));
    };
    try testing.expect(found);
}

var dialog_vtable: zpui.platform.Platform.VTable = undefined;
var pending_dialog: ?zpui.platform.PathsCallback = null;
var prompt_multiple: bool = false;
var prompt_extensions: []const []const u8 = &.{};
fn recordPrompt(_: *anyopaque, options: zpui.platform.PathPromptOptions, done: zpui.platform.PathsCallback) void {
    std.debug.assert(pending_dialog == null);
    pending_dialog = done;
    prompt_multiple = options.multiple;
    prompt_extensions = options.extensions;
}
fn finishDialog(paths: ?[]const []const u8) void {
    const done = pending_dialog.?;
    pending_dialog = null;
    done.func(done.ctx, paths);
    prompt_extensions = &.{};
}

test "crepus-zpui picker copies scoped context and handles delayed selection cancellation and filtering" {
    var f = try Fixture.initWithDialogs(
        \\{"version":7,"root":[{"kind":"forEach","bind":"rows","itemName":"row","itemBody":[{"kind":"filePicker","label":"Pick","accept":[".txt"],"multiple":true,"onPick":"pick","style":{"width":80,"height":40}}]}]}
    , true);
    defer f.deinit();
    try f.click("Pick");
    try testing.expect(prompt_multiple);
    try testing.expectEqualStrings("txt", prompt_extensions[0]);
    f.model().reverse = true;
    f.settle();
    finishDialog(&.{ "/tmp/a.txt", "/tmp/b.TXT" });
    f.settle();
    try testing.expectEqual(@as(usize, 2), f.model().picked);
    try testing.expectEqualStrings("first", f.model().last_key.?);
    try f.click("Pick");
    finishDialog(null);
    try testing.expectEqual(@as(usize, 2), f.model().picked);
    try f.click("Pick");
    finishDialog(&.{"/tmp/wrong.png"});
    try testing.expectEqual(@as(usize, 2), f.model().picked);
}

test "crepus-zpui picker callback safely retires after window and App destruction" {
    var f = try Fixture.initWithDialogs(
        \\{"version":7,"root":[{"kind":"filePicker","label":"Pick","accept":[],"onPick":"pick","style":{"width":80,"height":40}}]}
    , true);
    try f.click("Pick");
    f.platform.simulateClose();
    f.app.runUntilParked();
    finishDialog(&.{"/tmp/a.txt"});
    f.deinit();
    var next = try Fixture.initWithDialogs(
        \\{"version":7,"root":[{"kind":"filePicker","label":"Pick","accept":[],"onPick":"pick","style":{"width":80,"height":40}}]}
    , true);
    try next.click("Pick");
    next.deinit();
    finishDialog(&.{"/tmp/a.txt"});
}

test "crepus-zpui host factory request preserves input flags and native view placement" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"input","bind":"name","placeholder":"Name","multiline":true,"secure":true},{"kind":"webView","src":"https://example.com","style":{"width":120,"height":80}}]}
    );
    defer f.deinit();
    try testing.expect(f.model().input_flags and f.model().web_src_seen);
    const native = f.model().native.?;
    const placement = f.platform.native_placement[@backingInt(native)].?;
    try testing.expectEqual(@as(f32, 120), placement.bounds.size.width);
    try testing.expectEqual(@as(f32, 80), placement.bounds.size.height);
    // This proves request/placement plumbing only, never browser rendering or editing.
}

test "crepus-zpui long press cancels when pointer leaves and never fires in inactive window" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"button","label":"Hold","onLongPress":"hold","style":{"width":100,"height":60}}]}
    );
    defer f.deinit();
    _ = f.platform.simulateInput(.{ .mouse_down = .{ .button = .left, .position = .{ .x = 97, .y = 10 } } });
    _ = f.platform.simulateInput(.{ .mouse_move = .{ .position = .{ .x = 102, .y = 10 }, .pressed_button = .left } });
    f.app.advanceClock(500 * std.time.ns_per_ms);
    f.settle();
    try testing.expectEqual(@as(usize, 0), f.model().holds);
    _ = f.platform.simulateInput(.{ .mouse_down = .{ .button = .left, .position = .{ .x = 10, .y = 10 } } });
    f.platform.simulateActive(false);
    f.app.advanceClock(500 * std.time.ns_per_ms);
    f.settle();
    try testing.expectEqual(@as(usize, 0), f.model().holds);
}

test "crepus-zpui link accessible names follow active conditions and scoped collections" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"link","href":"https://example.com","children":[{"kind":"if","condition":"ready","thenChildren":[{"kind":"forEach","bind":"rows","itemName":"row","itemBody":[{"kind":"text","content":"","bind":"row.title"}]}],"elseChildren":[{"kind":"text","content":"Wait"}]}]}]}
    );
    defer f.deinit();
    _ = try f.node(.link, "First Second");
    f.model().ready = false;
    f.settle();
    _ = try f.node(.link, "Wait");
    try testing.expectError(error.MissingNode, f.node(.link, "First Second"));
}
