//! Retained, host-controlled text entry with UTF-16 IME and grapheme-safe editing.
const std = @import("std");
const zpui = @import("zpui");
const host = @import("host.zig");
const unicode = @import("text_support.zig");
const input = zpui.input;
const input_handler = zpui.window.input_handler;
const Range = input_handler.Range;
const Selection = input_handler.Selection;
const TextLayout = zpui.elements.text.TextLayout;
const Bounds = zpui.Bounds(zpui.Pixels);
const Point = zpui.Point(zpui.Pixels);
const text_limit = 1024 * 1024;

pub fn State(comptime View: type) type {
    return struct {
        const Self = @This();
        text: std.ArrayList(u8) = .empty,
        last_host: std.ArrayList(u8) = .empty,
        arena: std.heap.ArenaAllocator,
        focus: zpui.FocusHandle,
        parent: ?zpui.WeakEntity(View) = null,
        bind: []const u8 = "",
        handler: ?[]const u8 = null,
        placeholder: []const u8 = "",
        scope: ?*const host.Scope = null,
        multiline: bool = false,
        secure: bool = false,
        anchor: usize = 0,
        caret: usize = 0,
        marked: ?Range = null, // UTF-8 byte offsets internally; platform ranges are UTF-16.
        layout: ?*TextLayout = null, // Owned geometry; platform callbacks occur between frames.
        dragging: bool = false,
        blur_subscription: ?zpui.Subscription = null,

        fn init(app: *zpui.App) Self {
            const layout = app.gpa.create(TextLayout) catch @panic("OOM");
            layout.* = .{ .gpa = app.gpa };
            return .{ .arena = std.heap.ArenaAllocator.init(app.gpa), .focus = app.focusHandle(), .layout = layout };
        }
        pub fn deinit(self: *Self, app: *zpui.App) void {
            if (self.layout) |layout| {
                layout.deinit();
                app.gpa.destroy(layout);
            }
            self.text.deinit(app.gpa);
            self.last_host.deinit(app.gpa);
            self.arena.deinit();
            if (self.blur_subscription) |*subscription| subscription.deinit();
            self.focus.release(app);
        }
        fn prepare(self: *Self, request: host.Input, parent: zpui.WeakEntity(View), a: std.mem.Allocator) !void {
            if (!unicode.valid(request.value)) return error.InvalidUtf8;
            if (request.value.len > text_limit) return error.InputTooLarge;
            const changed = !std.mem.eql(u8, self.last_host.items, request.value);
            if (self.marked == null or changed) {
                if (!std.mem.eql(u8, self.text.items, request.value)) {
                    self.text.clearRetainingCapacity();
                    try self.text.appendSlice(a, request.value);
                    self.anchor = @min(self.anchor, self.text.items.len);
                    self.caret = @min(self.caret, self.text.items.len);
                    // External changes can shorten into a multibyte character.
                    while (self.caret > 0 and !std.unicode.utf8ValidateSlice(self.text.items[0..self.caret])) self.caret -= 1;
                    while (self.anchor > 0 and !std.unicode.utf8ValidateSlice(self.text.items[0..self.anchor])) self.anchor -= 1;
                    self.marked = null;
                }
            }
            self.last_host.clearRetainingCapacity();
            try self.last_host.appendSlice(a, request.value);
            _ = self.arena.reset(.retain_capacity);
            const owned = self.arena.allocator();
            self.bind = try owned.dupe(u8, request.bind);
            self.handler = if (request.handler) |handler| try owned.dupe(u8, handler) else null;
            self.placeholder = try owned.dupe(u8, request.placeholder);
            self.scope = try host.cloneScope(owned, request.scope);
            self.parent = parent;
            self.multiline = request.multiline;
            self.secure = request.secure;
        }
        fn selection(self: *const Self) Range {
            return .{ .start = @min(self.anchor, self.caret), .end = @max(self.anchor, self.caret) };
        }
        fn rangeBytes(self: *const Self, range: Range) Range {
            return .{ .start = unicode.byteOffset(self.text.items, range.start, false), .end = unicode.byteOffset(self.text.items, range.end, true) };
        }
        fn emit(self: *Self, window: *zpui.Window, cx: *zpui.Context(Self)) void {
            const Callback = struct {
                fn change(view: *View, editor: *Self, w: *zpui.Window, context: *zpui.Context(View)) void {
                    if (comptime @hasDecl(View, "onCrepusChange")) view.onCrepusChange(host.Change{ .bind = editor.bind, .handler = editor.handler, .value = .{ .text = editor.text.items }, .scope = editor.scope }, w, context);
                }
            };
            _ = self.parent.?.update(cx.app, Callback.change, .{ self, window });
            cx.notify();
        }
        fn replace(self: *Self, range: Range, value: []const u8, a: std.mem.Allocator) bool {
            if (range.start > range.end or range.end > self.text.items.len or value.len > text_limit or !unicode.valid(value)) return false;
            var filtered: std.ArrayList(u8) = .empty;
            defer filtered.deinit(a);
            if (self.multiline) filtered.appendSlice(a, value) catch @panic("OOM") else {
                for (value) |byte| if (byte != '\r' and byte != '\n') filtered.append(a, byte) catch @panic("OOM");
            }
            if (range.start == range.end and filtered.items.len == 0) return false;
            const new_len = self.text.items.len - (range.end - range.start) + filtered.items.len;
            if (new_len > text_limit) return false;
            // Allocate separately: inserting can otherwise invalidate slices into this text.
            var next: std.ArrayList(u8) = .empty;
            next.appendSlice(a, self.text.items[0..range.start]) catch @panic("OOM");
            next.appendSlice(a, filtered.items) catch @panic("OOM");
            next.appendSlice(a, self.text.items[range.end..]) catch @panic("OOM");
            self.text.deinit(a);
            self.text = next;
            self.caret = range.start + filtered.items.len;
            self.anchor = self.caret;
            return true;
        }
        pub fn selectedTextRange(self: *Self, _: *zpui.Window, _: *zpui.Context(Self)) ?Selection {
            const r = self.selection();
            return .{ .range = .{ .start = unicode.utf16Offset(self.text.items, r.start), .end = unicode.utf16Offset(self.text.items, r.end) }, .reversed = self.caret < self.anchor };
        }
        pub fn markedTextRange(self: *Self, _: *zpui.Window, _: *zpui.Context(Self)) ?Range {
            const r = self.marked orelse return null;
            return .{ .start = unicode.utf16Offset(self.text.items, r.start), .end = unicode.utf16Offset(self.text.items, r.end) };
        }
        pub fn textForRange(self: *Self, range: Range, out: *std.ArrayList(u8), _: *zpui.Window, cx: *zpui.Context(Self)) ?Range {
            if (self.secure or range.start > range.end) return null;
            const r = self.rangeBytes(range);
            out.appendSlice(cx.gpa(), self.text.items[r.start..r.end]) catch return null;
            return .{ .start = unicode.utf16Offset(self.text.items, r.start), .end = unicode.utf16Offset(self.text.items, r.end) };
        }
        pub fn replaceTextInRange(self: *Self, range: ?Range, value: []const u8, window: *zpui.Window, cx: *zpui.Context(Self)) void {
            const r = if (range) |explicit| self.rangeBytes(explicit) else self.marked orelse self.selection();
            if (!self.replace(r, value, cx.gpa())) return;
            self.marked = null;
            self.emit(window, cx);
        }
        pub fn replaceAndMarkTextInRange(self: *Self, range: ?Range, value: []const u8, selected: ?Range, _: *zpui.Window, cx: *zpui.Context(Self)) void {
            const r = if (range) |explicit| self.rangeBytes(explicit) else self.marked orelse self.selection();
            if (!self.replace(r, value, cx.gpa())) return;
            self.marked = .{ .start = r.start, .end = self.caret };
            if (selected) |s| {
                const bytes = self.text.items[r.start..self.caret];
                self.anchor = r.start + unicode.byteOffset(bytes, s.start, false);
                self.caret = r.start + unicode.byteOffset(bytes, s.end, true);
            }
            cx.notify(); // Preedit stays local until committed.
        }
        pub fn unmarkText(self: *Self, window: *zpui.Window, cx: *zpui.Context(Self)) void {
            if (self.marked == null) return;
            self.marked = null;
            self.emit(window, cx);
        }
        pub fn acceptsTextInput(_: *Self, _: *zpui.Window, _: *zpui.Context(Self)) bool {
            return true;
        }
        fn displayIndex(self: *const Self, byte_index: usize) usize {
            if (!self.secure) return byte_index;
            return unicode.maskOffset(self.text.items, byte_index, self.multiline, false);
        }
        pub fn boundsForRange(self: *Self, range: Range, bounds: Bounds, _: *zpui.Window, _: *zpui.Context(Self)) ?Bounds {
            const layout = self.layout orelse return null;
            const r = self.rangeBytes(range);
            const p = layout.positionForIndex(self.displayIndex(r.start)) orelse bounds.origin;
            const end = layout.positionForIndex(self.displayIndex(r.end)) orelse p;
            return .{ .origin = p, .size = .{ .width = @max(1, if (p.y == end.y) end.x - p.x else 1), .height = layout.lineHeight() } };
        }
        fn blur(self: *Self, window: *zpui.Window, cx: *zpui.Context(Self)) void {
            self.dragging = false;
            self.unmarkText(window, cx);
        }
        fn key(self: *Self, event: *const input.KeyDownEvent, window: *zpui.Window, cx: *zpui.Context(Self)) void {
            const k = event.keystroke;
            const secondary = k.modifiers.secondary();
            const r = self.selection();
            if (secondary and std.mem.eql(u8, k.key, "a")) {
                self.anchor = 0;
                self.caret = self.text.items.len;
            } else if (secondary and (std.mem.eql(u8, k.key, "c") or std.mem.eql(u8, k.key, "x"))) {
                if (!self.secure) window.app.platform.vtable.writeClipboard(window.app.platform.ptr, self.text.items[r.start..r.end]);
                if (std.mem.eql(u8, k.key, "x") and self.replace(r, "", cx.gpa())) self.emit(window, cx);
            } else if (secondary and std.mem.eql(u8, k.key, "v")) {
                const value = window.app.platform.vtable.readClipboard(window.app.platform.ptr, cx.gpa()) orelse return;
                defer cx.gpa().free(value);
                if (self.replace(r, value, cx.gpa())) self.emit(window, cx);
            } else if (std.mem.eql(u8, k.key, "backspace") or std.mem.eql(u8, k.key, "delete")) {
                const deleted = if (r.start != r.end) r else if (std.mem.eql(u8, k.key, "backspace")) Range{ .start = unicode.boundary(self.text.items, self.caret, false), .end = self.caret } else Range{ .start = self.caret, .end = unicode.boundary(self.text.items, self.caret, true) };
                if (self.replace(deleted, "", cx.gpa())) self.emit(window, cx);
            } else if (std.mem.eql(u8, k.key, "left") or std.mem.eql(u8, k.key, "right")) {
                const forward = std.mem.eql(u8, k.key, "right");
                self.caret = if (!k.modifiers.shift and r.start != r.end) (if (forward) r.end else r.start) else unicode.boundary(self.text.items, self.caret, forward);
                if (!k.modifiers.shift) self.anchor = self.caret;
            } else if (std.mem.eql(u8, k.key, "up") or std.mem.eql(u8, k.key, "down")) {
                if (!self.multiline) return;
                if (self.layout) |layout| if (layout.positionForIndex(self.displayIndex(self.caret))) |position| {
                    self.caret = self.hit(.{ .x = position.x, .y = position.y + (if (std.mem.eql(u8, k.key, "down")) layout.lineHeight() else -layout.lineHeight()) + layout.lineHeight() / 2 });
                };
                if (!k.modifiers.shift) self.anchor = self.caret;
            } else if (std.mem.eql(u8, k.key, "home") or std.mem.eql(u8, k.key, "end")) {
                self.caret = if (std.mem.eql(u8, k.key, "home")) 0 else self.text.items.len;
                if (!k.modifiers.shift) self.anchor = self.caret;
            } else if (std.mem.eql(u8, k.key, "enter")) {
                if (self.multiline and self.replace(r, "\n", cx.gpa())) self.emit(window, cx);
            } else return; // Printable keys flow through ZPUI's IME handler.
            self.marked = null;
            cx.stopPropagation();
            cx.notify();
        }
        fn hit(self: *Self, position: Point) usize {
            const layout = self.layout orelse return 0;
            const display = switch (layout.indexForPosition(position)) {
                .inside => |i| i,
                .outside => |i| i,
            };
            if (self.secure) return unicode.maskOffset(self.text.items, display, self.multiline, true);
            const bounded = @min(display, self.text.items.len);
            const previous = unicode.boundary(self.text.items, bounded, false);
            return if (unicode.boundary(self.text.items, previous, true) == bounded) bounded else previous;
        }
        fn down(self: *Self, event: *const input.MouseDownEvent, window: *zpui.Window, cx: *zpui.Context(Self)) void {
            self.focus.focus(window);
            self.caret = self.hit(event.position);
            if (!event.modifiers.shift) self.anchor = self.caret;
            self.dragging = true;
            cx.notify();
        }
        fn move(self: *Self, event: *const input.MouseMoveEvent, _: *zpui.Window, cx: *zpui.Context(Self)) void {
            if (!self.dragging or event.pressed_button != .left) return;
            self.caret = self.hit(event.position);
            cx.notify();
        }
        fn up(self: *Self, _: *const input.MouseUpEvent, _: *zpui.Window, _: *zpui.Context(Self)) void {
            self.dragging = false;
        }
        pub fn render(self: *Self, window: *zpui.Window, cx: *zpui.Context(Self)) zpui.StatefulDiv {
            if (self.blur_subscription == null) self.blur_subscription = cx.onBlur(self.focus, window, Self.blur) catch @panic("OOM");
            const display = if (self.secure) unicode.mask(zpui.frameAllocator(), self.text.items, self.multiline) catch @panic("OOM") else self.text.items;
            const owned = zpui.frameAllocator().dupe(u8, if (display.len == 0) self.placeholder else display) catch @panic("OOM");
            var text = zpui.styledText(owned);
            if (self.anchor != self.caret and display.len > 0) {
                const r = self.selection();
                text = text.withHighlights(&.{.{ .start = self.displayIndex(r.start), .end = self.displayIndex(r.end), .style = .{ .background_color = zpui.rgb(0xbdd7ff).toHsla() } }});
            }
            text.layout_ptr = self.layout.?;
            text.owns_layout = false;
            const Paint = struct {
                fn globalMove(weak: *zpui.WeakEntity(Self), event: *const input.MouseMoveEvent, phase: zpui.window.DispatchPhase, w: *zpui.Window, app: *zpui.App) void {
                    if (phase == .capture) _ = weak.update(app, Self.move, .{ event, w });
                }
                fn globalUp(weak: *zpui.WeakEntity(Self), event: *const input.MouseUpEvent, phase: zpui.window.DispatchPhase, w: *zpui.Window, app: *zpui.App) void {
                    if (phase == .capture) _ = weak.update(app, Self.up, .{ event, w });
                }
                fn run(entity: zpui.Entity(Self), bounds: Bounds, w: *zpui.Window, app: *zpui.App) void {
                    const state = entity.read(app);
                    w.handleInput(state.focus, .init(entity, bounds));
                    w.onMouseEvent(input.MouseMoveEvent, entity.downgrade(), globalMove);
                    w.onMouseEvent(input.MouseUpEvent, entity.downgrade(), globalUp);
                    if (!state.focus.isFocused(w)) return;
                    const layout = state.layout orelse return;
                    const p = layout.positionForIndex(if (state.text.items.len == 0) 0 else state.displayIndex(state.caret)) orelse bounds.origin;
                    w.paintQuad(zpui.fill(.{ .origin = p, .size = .{ .width = 1, .height = layout.lineHeight() } }, zpui.rgb(0x111827).toHsla()));
                }
            };
            var result = zpui.div().id("editor").relative().trackFocus(self.focus).tabIndex(0).cursorText().wFull().minH(zpui.px(28))
                .role(if (self.secure) .password_input else if (self.multiline) .multiline_text_input else .text_input)
                .ariaLabel(self.placeholder).ariaPlaceholder(self.placeholder).ariaValue(zpui.frameAllocator().dupe(u8, display) catch @panic("OOM"))
                .ariaTextSelection(self.displayIndex(self.anchor), self.displayIndex(self.caret))
                .onMouseDown(.left, cx.listener(Self.down)).onMouseMove(cx.listener(Self.move)).onMouseUp(.left, cx.listener(Self.up)).onMouseUpOut(.left, cx.listener(Self.up))
                .onKeyDown(cx.listener(Self.key)).child(text).child(zpui.canvas(cx.entity(), Paint.run).absolute().inset0());
            result = if (self.multiline) result.minH(zpui.px(96)).hFull().whitespaceNormal().overflowYScroll() else result.hFull().whitespaceNowrap().overflowHidden();
            return result;
        }
    };
}

/// Hosts can override this factory; otherwise it is the built-in retained editor.
pub fn render(comptime View: type, request: host.Input, window: *zpui.Window, cx: *zpui.Context(View)) !zpui.AnyElement {
    const S = State(View);
    const gid = window.pushElementId(.from(zpui.fmt("{s}/editor-state", .{request.identity})));
    defer window.popElementId();
    const entity = window.useKeyedState(S, gid, S.init);
    var lease = entity.lease(window.app);
    defer lease.end();
    try lease.value.prepare(request, cx.weakEntity(), cx.gpa());
    return zpui.intoAnyElement(entity);
}
