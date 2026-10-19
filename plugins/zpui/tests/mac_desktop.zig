//! Real AppKit/Metal/WebKit acceptance. Opens only this test's own temporary window.
const std = @import("std");
const zpui = @import("zpui");
const bridge = @import("crepuscularity_zpui");
extern var kCFRunLoopDefaultMode: *anyopaque;
extern fn CFRunLoopRunInMode(*anyopaque, f64, bool) c_int;
extern fn crepus_take_snapshot(*anyopaque, [*:0]const u8, ?*anyopaque, *const fn (?*anyopaque, bool) callconv(.c) void) void;
const source =
    \\{"version":7,"root":[{"kind":"stack","axis":"column","children":[{"kind":"input","bind":"name","placeholder":"Name","style":{"width":200,"height":40}},{"kind":"webView","src":"data:text/html,%3Ctitle%3ECrepuscularity%20ZPUI%3C/title%3E%3Cbody%20style=%22margin:0;background:%23123456;width:200px;height:100px%22%3E%3C/body%3E","style":{"width":200,"height":100}}]}]}
;
const Result = struct { success: bool = false, failure: ?anyerror = null };
var result: Result = .{};
var running_app: ?*zpui.App = null;

const View = struct {
    document: bridge.Document,
    browser: bridge.MacBrowser = .{},
    value: std.ArrayList(u8) = .empty,
    window_id: zpui.WindowId,
    attempts: usize = 0,
    edited: bool = false,
    edit_queued: bool = false,
    snapshot_requested: bool = false,
    fn init(window: *zpui.Window, cx: *zpui.Context(View)) View {
        var view = View{ .document = bridge.Document.parse(cx.gpa(), source) catch @panic("bad desktop source"), .window_id = window.id };
        view.value.appendSlice(cx.gpa(), "A") catch @panic("OOM");
        var timer = cx.timer(100 * std.time.ns_per_ms, View.probe) catch @panic("OOM");
        timer.detach();
        return view;
    }
    pub fn deinit(self: *View, app: *zpui.App) void {
        self.browser.deinit(app);
        self.document.deinit();
        self.value.deinit(app.gpa);
    }
    pub fn crepusValue(self: *View, _: []const u8) ?bridge.Value {
        return .{ .text = self.value.items };
    }
    pub fn onCrepusChange(self: *View, change: bridge.Change, _: *zpui.Window, cx: *zpui.Context(View)) void {
        self.value.clearRetainingCapacity();
        self.value.appendSlice(cx.gpa(), change.value.text) catch @panic("OOM");
        self.edited = std.mem.eql(u8, self.value.items, "é😀");
        cx.notify();
    }
    pub fn crepusWebView(self: *View, request: bridge.WebView, window: *zpui.Window, _: *zpui.Context(View)) !zpui.NativeViewId {
        return self.browser.open(request.src, window);
    }
    pub fn render(self: *View, window: *zpui.Window, cx: *zpui.Context(View)) zpui.AnyElement {
        return bridge.render(View, self, window, cx, &self.document, "desktop") catch @panic("desktop renderer rejected");
    }
    fn probe(self: *View, cx: *zpui.Context(View)) void {
        if (cx.app.windowById(self.window_id) == null) return;
        self.attempts += 1;
        if (self.attempts > 100) {
            result.failure = error.DesktopAcceptanceTimeout;
            cx.app.platform.quit();
            return;
        }
        if (!self.edited and !self.edit_queued) {
            self.edit_queued = true;
            const context = cx.gpa().create(zpui.WindowId) catch @panic("OOM");
            context.* = self.window_id;
            cx.app.deferFn(context, editNative);
        }
        if (self.browser.title()) |title| {
            if (self.edited and std.mem.eql(u8, title, "Crepuscularity ZPUI") and !self.snapshot_requested) {
                if (!std.mem.eql(u8, self.value.items, "é😀")) {
                    result.failure = error.NativeInputMismatch;
                    cx.app.platform.quit();
                    return;
                }
                self.snapshot_requested = true;
                crepus_take_snapshot(self.browser.view.?, "/tmp/crepus-zpui-browser-acceptance.png", null, snapshotDone);
            }
        }
        var timer = cx.timer(100 * std.time.ns_per_ms, View.probe) catch @panic("OOM");
        timer.detach();
    }
};
fn editNative(context: *zpui.WindowId, app: *zpui.App) void {
    defer app.gpa.destroy(context);
    const window = app.windowById(context.*) orelse return;
    window.setA11yActive(true);
    window.drawAndPresent();
    for (window.rendered_frame.a11y.nodes.items) |node| if (node.role == .text_input) {
        const p = zpui.input.Point{ .x = node.bounds.origin.x + 1, .y = node.bounds.origin.y + 1 };
        _ = window.dispatchEvent(.{ .mouse_down = .{ .button = .left, .position = p } });
        _ = window.dispatchEvent(.{ .mouse_up = .{ .button = .left, .position = p } });
        window.drawAndPresent();
        const handler = window.input_handler orelse break;
        handler.replaceTextInRange(window, .{ .start = 0, .end = 1 }, "é😀");
        window.drawAndPresent();
        return;
    };
    result.failure = error.MissingNativeInput;
    app.platform.quit();
}
fn snapshotDone(_: ?*anyopaque, valid: bool) callconv(.c) void {
    result.success = valid;
    if (!valid) result.failure = error.BrowserPixelMismatch;
    running_app.?.platform.quit();
}
fn launch(_: void, app: *zpui.App) void {
    _ = app.openWindow(.{ .bounds = .{ .origin = .{ .x = 80, .y = 80 }, .size = .{ .width = 300, .height = 220 } } }, View, View.init, .{}) catch |err| {
        result.failure = err;
        app.platform.quit();
    };
}
pub fn main(init: std.process.Init) !void {
    const platform = try zpui.mac_platform.create(init.gpa);
    const app = try zpui.App.init(init.gpa, platform);
    defer {
        app.deinit();
        running_app = null;
        // Drain AppKit close callbacks and the pinned executor's canceled
        // 2-second arena-trim runnable; cancellation frees it when dispatched.
        _ = CFRunLoopRunInMode(kCFRunLoopDefaultMode, 2.2, false);
    }
    running_app = app;
    app.run({}, launch);
    if (result.failure) |err| return err;
    if (!result.success) return error.DesktopAcceptanceIncomplete;
    std.debug.print("Native AppKit/Metal input and WKWebView snapshot passed: /tmp/crepus-zpui-browser-acceptance.png\n", .{});
}
