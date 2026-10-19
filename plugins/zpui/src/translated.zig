//! Translation preserves layout while moving paint, hit-testing and accessibility together.
const zpui = @import("zpui");
const element = zpui.window.element;
const Point = zpui.Point(zpui.Pixels);
const Bounds = zpui.Bounds(zpui.Pixels);
const Translated = struct {
    child: zpui.AnyElement,
    offset: Point,
    pub fn requestLayout(self: *@This(), _: ?element.GlobalElementId, _: *void, window: *zpui.Window, app: *zpui.App) element.LayoutId {
        return self.child.requestLayout(window, app);
    }
    pub fn prepaint(self: *@This(), _: ?element.GlobalElementId, _: Bounds, _: *void, _: *void, window: *zpui.Window, app: *zpui.App) void {
        window.pushElementOffset(self.offset);
        defer window.popElementOffset();
        self.child.prepaint(window, app);
    }
    pub fn paint(self: *@This(), _: ?element.GlobalElementId, _: Bounds, _: *void, _: *void, window: *zpui.Window, app: *zpui.App) void {
        self.child.paint(window, app);
    }
};
pub fn translate(child: zpui.AnyElement, x: f32, y: f32) zpui.AnyElement {
    return zpui.intoAnyElement(Translated{ .child = child, .offset = .{ .x = x, .y = y } });
}
