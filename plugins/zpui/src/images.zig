//! Object-fit geometry with all View IR object positions and actual atlas cropping.
const std = @import("std");
const zpui = @import("zpui");
const ir = @import("view_ir");
const styles = @import("style.zig");
const Bounds = zpui.Bounds(zpui.Pixels);
const Context = struct { image: *zpui.RenderImage, fit: zpui.image.ObjectFit, position: ir.styles.ObjectPosition, radius: f32 };

pub fn fittedBounds(bounds: Bounds, natural: zpui.Size(zpui.DevicePixels), fit: zpui.image.ObjectFit, position: ir.styles.ObjectPosition) Bounds {
    var fitted = fit.getBounds(bounds, natural);
    const fractions: [2]f32 = switch (position) {
        .center => .{ 0.5, 0.5 },
        .top => .{ 0.5, 0 },
        .bottom => .{ 0.5, 1 },
        .left => .{ 0, 0.5 },
        .right => .{ 1, 0.5 },
        .@"left-top" => .{ 0, 0 },
        .@"left-bottom" => .{ 0, 1 },
        .@"right-top" => .{ 1, 0 },
        .@"right-bottom" => .{ 1, 1 },
    };
    fitted.origin = .{ .x = bounds.origin.x + (bounds.size.width - fitted.size.width) * fractions[0], .y = bounds.origin.y + (bounds.size.height - fitted.size.height) * fractions[1] };
    return fitted;
}
fn paint(context: Context, bounds: Bounds, window: *zpui.Window, _: *zpui.App) void {
    const fitted = fittedBounds(bounds, context.image.size(0), context.fit, context.position);
    const visible = bounds.intersect(fitted);
    if (visible.isEmpty()) return;
    window.paintImageFitted(visible, fitted, .{ .top_left = context.radius, .top_right = context.radius, .bottom_right = context.radius, .bottom_left = context.radius }, context.image, 0, false, null);
}
pub fn render(image: *zpui.RenderImage, style: ir.Style) !zpui.AnyElement {
    if (image.frameCount() == 0 or image.size(0).width == 0 or image.size(0).height == 0) return error.InvalidImage;
    const natural = image.renderSize(0);
    if (!std.math.isFinite(natural.width) or !std.math.isFinite(natural.height) or natural.width <= 0 or natural.height <= 0) return error.InvalidImage;
    const fit: zpui.image.ObjectFit = switch (style.objectFit orelse .contain) {
        .contain => .contain,
        .cover => .cover,
        .fill => .fill,
        .none => .none,
        .@"scale-down" => .scale_down,
    };
    var element = zpui.canvas(Context{ .image = image, .fit = fit, .position = style.objectPosition orelse .center, .radius = style.cornerRadius orelse 0 }, paint);
    const width = style.width;
    const height = style.height;
    // Preserve natural aspect when only one absolute dimension is supplied.
    const w64: f64 = if (width) |v| v else if (height) |h| (if (h >= 0) @as(f64, h) * natural.width / natural.height else natural.width) else natural.width;
    const h64: f64 = if (height) |v| v else if (width) |v| (if (v >= 0) @as(f64, v) * natural.height / natural.width else natural.height) else natural.height;
    if (!std.math.isFinite(w64) or !std.math.isFinite(h64) or w64 > std.math.floatMax(f32) or h64 > std.math.floatMax(f32)) return error.InvalidImageSize;
    element = element.w(styles.length(@floatCast(w64))).h(styles.length(@floatCast(h64)));
    return zpui.intoAnyElement(element);
}
