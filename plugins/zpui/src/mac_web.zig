//! macOS WKWebView reference. The host may override it on any platform.
const std = @import("std");
const zpui = @import("zpui");
const host = @import("host.zig");
const Obj = *anyopaque;
extern fn objc_getClass([*:0]const u8) ?Obj;
extern fn sel_registerName([*:0]const u8) Obj;
extern fn objc_msgSend() void;
const Rect = extern struct { x: f64 = 0, y: f64 = 0, width: f64 = 0, height: f64 = 0 };
fn call0(comptime R: type, object: Obj, name: [*:0]const u8) R {
    const call: *const fn (Obj, Obj) callconv(.c) R = @ptrCast(&objc_msgSend);
    return call(object, sel_registerName(name));
}
fn call1(comptime R: type, object: Obj, name: [*:0]const u8, argument: anytype) R {
    const call: *const fn (Obj, Obj, @TypeOf(argument)) callconv(.c) R = @ptrCast(&objc_msgSend);
    return call(object, sel_registerName(name), argument);
}
fn string(value: []const u8) !Obj {
    const class = objc_getClass("NSString") orelse return error.MissingFoundation;
    const allocated = call0(?Obj, class, "alloc") orelse return error.OutOfMemory;
    const call: *const fn (Obj, Obj, [*]const u8, usize, usize) callconv(.c) ?Obj = @ptrCast(&objc_msgSend);
    return call(allocated, sel_registerName("initWithBytes:length:encoding:"), value.ptr, value.len, 4) orelse error.InvalidUtf8;
}
pub const Browser = struct {
    view: ?Obj = null,
    native: ?zpui.NativeViewId = null,
    window_id: zpui.WindowId = @fromBackingInt(@intCast(0)),
    source: std.ArrayList(u8) = .empty,
    fn init(_: *zpui.App) Browser {
        return .{};
    }
    pub fn deinit(self: *Browser, app: *zpui.App) void {
        if (self.native) |id| if (app.windowById(self.window_id)) |w| w.detachNativeView(id);
        if (self.view) |view| {
            call0(void, view, "stopLoading");
            call0(void, view, "release");
        }
        self.view = null;
        self.native = null;
        self.source.deinit(app.gpa);
        self.source = .empty;
    }
    pub fn title(self: *const Browser) ?[]const u8 {
        const view = self.view orelse return null;
        const label = call0(?Obj, view, "title") orelse return null;
        const bytes = call0(?[*:0]const u8, label, "UTF8String") orelse return null;
        return std.mem.span(bytes);
    }
    pub fn open(self: *Browser, src: []const u8, window: *zpui.Window) !zpui.NativeViewId {
        if (@import("builtin").os.tag != .macos) return error.UnsupportedPlatform;
        if (self.native != null and self.window_id != window.id) return error.BrowserWindowMismatch;
        if (self.native != null and std.mem.eql(u8, self.source.items, src)) return self.native.?;
        const text = try string(src);
        defer call0(void, text, "release");
        const url = call1(?Obj, objc_getClass("NSURL") orelse return error.MissingFoundation, "URLWithString:", text) orelse return error.InvalidBrowserUrl;
        const request = call1(?Obj, objc_getClass("NSURLRequest") orelse return error.MissingFoundation, "requestWithURL:", url) orelse return error.InvalidBrowserUrl;
        if (self.view == null) {
            const configuration_class = objc_getClass("WKWebViewConfiguration") orelse return error.MissingWebKit;
            const configuration = call0(?Obj, configuration_class, "new") orelse return error.OutOfMemory;
            defer call0(void, configuration, "release");
            const store = call0(?Obj, objc_getClass("WKWebsiteDataStore") orelse return error.MissingWebKit, "nonPersistentDataStore") orelse return error.MissingWebKit;
            call1(void, configuration, "setWebsiteDataStore:", store);
            const allocated = call0(?Obj, objc_getClass("WKWebView") orelse return error.MissingWebKit, "alloc") orelse return error.OutOfMemory;
            const create: *const fn (Obj, Obj, Rect, Obj) callconv(.c) ?Obj = @ptrCast(&objc_msgSend);
            const view = create(allocated, sel_registerName("initWithFrame:configuration:"), .{}, configuration) orelse return error.MissingWebKit;
            errdefer call0(void, view, "release");
            const native = try window.attachNativeView(view, .{});
            self.view = view;
            self.native = native;
            self.window_id = window.id;
        }
        // Copy before initiating navigation so allocation failure does not commit a new URL.
        var next: std.ArrayList(u8) = .empty;
        errdefer next.deinit(window.app.gpa);
        try next.appendSlice(window.app.gpa, src);
        _ = call1(?Obj, self.view.?, "loadRequest:", request) orelse return error.BrowserNavigationRejected;
        self.source.deinit(window.app.gpa);
        self.source = next;
        return self.native.?;
    }
};
pub fn render(request: host.WebView, window: *zpui.Window) !zpui.NativeViewId {
    if (@import("builtin").os.tag != .macos) return error.MissingWebViewFactory;
    if (window.app.test_platform != null) return error.HeadlessBrowserRequiresFactory;
    if (!window.platform_window.supportsNativeViews()) return error.MissingNativeViewBackend;
    const gid = window.pushElementId(.from(zpui.fmt("{s}/web-state", .{request.identity})));
    defer window.popElementId();
    const entity = window.useKeyedState(Browser, gid, Browser.init);
    var lease = entity.lease(window.app);
    defer lease.end();
    return lease.value.open(request.src, window);
}
