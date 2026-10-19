//! Controlled form example. Image storage is supplied and retained by the host.
const std = @import("std");
const zpui = @import("zpui");
const crepus = @import("crepuscularity_zpui");

pub const Form = struct {
    document: crepus.Document,
    enabled: bool = false,
    remember: bool = false,
    volume: f64 = 30,
    expanded: bool = false,
    details: bool = false,
    changes: usize = 0,
    applied: usize = 0,
    last_handler: ?[]const u8 = null,
    last_link: ?crepus.Link = null,
    preview: ?*zpui.RenderImage = null,

    pub fn init(a: std.mem.Allocator, source: []const u8) !Form {
        var form = Form{ .document = try crepus.Document.parse(a, source) };
        errdefer form.document.deinit();
        try crepus.validateHost(Form, &form, &form.document);
        return form;
    }

    pub fn deinit(self: *Form, _: *zpui.App) void {
        self.document.deinit();
    }

    pub fn render(self: *Form, window: *zpui.Window, cx: *zpui.Context(Form)) zpui.AnyElement {
        return crepus.render(Form, self, window, cx, &self.document, "form") catch @panic("invalid form host");
    }

    /// Read current values. Returned strings remain valid for the whole frame.
    pub fn crepusValue(self: *Form, bind: []const u8) ?crepus.Value {
        if (std.mem.eql(u8, bind, "enabled")) return .{ .boolean = self.enabled };
        if (std.mem.eql(u8, bind, "remember")) return .{ .boolean = self.remember };
        if (std.mem.eql(u8, bind, "volume")) return .{ .number = self.volume };
        if (std.mem.eql(u8, bind, "mode")) return .{ .text = if (self.expanded) "expanded" else "compact" };
        if (std.mem.eql(u8, bind, "tab")) return .{ .text = if (self.details) "details" else "overview" };
        if (std.mem.eql(u8, bind, "status")) return .{ .text = if (self.enabled) "Enabled" else "Disabled" };
        return null;
    }

    /// Commit a typed event to host state and schedule a fresh frame.
    pub fn onCrepusChange(self: *Form, change: crepus.Change, _: *zpui.Window, cx: *zpui.Context(Form)) void {
        const bind = change.bind orelse return;
        if (std.mem.eql(u8, bind, "enabled")) self.enabled = change.value.boolean;
        if (std.mem.eql(u8, bind, "remember")) self.remember = change.value.boolean;
        if (std.mem.eql(u8, bind, "volume")) self.volume = change.value.number;
        if (std.mem.eql(u8, bind, "mode")) self.expanded = std.mem.eql(u8, change.value.text, "expanded");
        if (std.mem.eql(u8, bind, "tab")) self.details = std.mem.eql(u8, change.value.text, "details");
        self.last_handler = change.handler;
        self.changes += 1;
        cx.notify();
    }

    pub fn onCrepusClick(self: *Form, handler: []const u8, _: *const zpui.ClickEvent, _: *zpui.Window, cx: *zpui.Context(Form)) void {
        if (std.mem.eql(u8, handler, "apply")) self.applied += 1;
        cx.notify();
    }

    /// The host decides how to navigate and interpret target/rel.
    pub fn onCrepusLink(self: *Form, link: crepus.Link, _: *const zpui.ClickEvent, _: *zpui.Window, cx: *zpui.Context(Form)) void {
        self.last_link = link;
        cx.notify();
    }

    pub fn crepusImage(self: *Form, source: []const u8) ?zpui.ImageSource {
        if (std.mem.eql(u8, source, "preview")) {
            const image = self.preview orelse return null;
            return .{ .render = image };
        }
        return null;
    }
};
