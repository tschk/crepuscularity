//! Host-controlled ZPUI native/drawn form controls.
const zpui = @import("zpui");
const ir = @import("view_ir");
const host = @import("host.zig");

pub fn render(comptime View: type, self: *View, window: *zpui.Window, cx: *zpui.Context(View), node: *const ir.Node, identity: []const u8) !zpui.AnyElement {
    const kind: zpui.elements.native_control.Kind = switch (node.kind) {
        .toggle => .switch_,
        .checkbox => .checkbox,
        .slider => .slider,
        .picker => .popup,
        .tabs => .segmented,
        else => unreachable,
    };
    if (window.measureNativeControl(.{ .kind = kind }) == null and zpui.elements.desktop_controls.controlLook(window, null) == null) return error.MissingControlBackend;
    const Callback = struct {
        fn change(view: *View, source: *const ir.Node, event: *const zpui.NativeControlEvent, w: *zpui.Window, context: *zpui.Context(View)) void {
            if (comptime @hasDecl(View, "onCrepusChange")) {
                const value: host.Value = switch (source.kind) {
                    .toggle, .checkbox => .{ .boolean = event.on },
                    .slider => .{ .number = event.value },
                    .picker, .tabs => if (event.index < source.options.len) .{ .text = source.options[event.index].value } else return,
                    else => unreachable,
                };
                view.onCrepusChange(host.Change{ .bind = source.bind, .handler = source.on_change, .value = value }, w, context);
            }
        }
    };
    const active = node.bind != null or node.on_change != null;
    const listener = cx.listenerWith(node, Callback.change);
    const width: ?f32 = if (node.style.width) |v| (if (v >= 0) v else null) else null;
    const element = switch (node.kind) {
        .toggle => zpui.nativeSwitch(identity, .{ .on = try host.checked(View, self, node), .enabled = active, .label = node.text, .width = width }, listener, null),
        .checkbox => zpui.nativeCheckbox(identity, .{ .on = try host.checked(View, self, node), .enabled = active, .title = node.text, .width = width }, listener, null),
        .slider => zpui.nativeSlider(identity, .{ .value = try host.numeric(View, self, node), .min = node.min, .max = node.max, .step = node.step orelse 0, .enabled = active, .label = node.text, .width = width }, listener, null),
        .picker, .tabs => blk: {
            const labels = try zpui.frameAllocator().alloc([]const u8, node.options.len);
            for (node.options, labels) |option, *label| label.* = option.label;
            const options: zpui.elements.native_control.ChoiceOptions = .{ .items = labels, .selected = try host.selected(View, self, node), .enabled = active, .label = node.bind.?, .width = width };
            break :blk if (node.kind == .picker) zpui.nativePopup(identity, options, listener, null) else zpui.nativeSegmented(identity, options, listener, null);
        },
        else => unreachable,
    };
    return zpui.intoAnyElement(element);
}
