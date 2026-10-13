//! Call inside a ZPUI view's render method, with a document retained by the host.
const zpui = @import("zpui");
pub const ir = @import("view_ir");
pub const Document = ir.Document;

/// Host must implement onCrepusClick(handler, event, window, context).
/// Namespace must distinguish multiple adapter instances in the same parent.
pub fn render(comptime View: type, self: *View, window: *zpui.Window, cx: *zpui.Context(View), document: *const Document, namespace: []const u8) zpui.AnyElement {
    _ = self;
    _ = window;
    var root = zpui.div().id(namespace);
    for (document.root, 0..) |node, index| {
        const path = zpui.fmt("{s}/{d}", .{ namespace, index });
        root = root.child(renderNode(View, cx, node, path));
    }
    return zpui.intoAnyElement(root);
}

fn renderNode(comptime View: type, cx: *zpui.Context(View), node: ir.Node, path: []const u8) zpui.AnyElement {
    const identity = if (node.style.id) |id| zpui.fmt("{s}/{s}", .{ path, id }) else path;
    var element = zpui.div().id(identity);
    switch (node.kind) {
        .text => element = element.child(node.text),
        .button => {
            element = element.role(.button).ariaLabel(node.text).child(node.text);
            if (node.on_click) |handler| {
                const Callback = struct {
                    fn click(view: *View, name: []const u8, event: *const zpui.ClickEvent, window: *zpui.Window, context: *zpui.Context(View)) void {
                        view.onCrepusClick(name, event, window, context);
                    }
                };
                element = element.cursorPointer().focusable().onClick(cx.listenerWith(handler, Callback.click));
            }
        },
        .stack, .scroll => {
            element = element.flex();
            element = if (node.axis == .row) element.flexRow() else element.flexCol();
            if (node.spacing) |spacing| element = element.gap(zpui.px(spacing));
            if (node.kind == .scroll) {
                element = if (node.axis == .row) element.overflowXScroll() else element.overflowYScroll();
            }
            for (node.children, 0..) |child, index| {
                const child_path = zpui.fmt("{s}/{d}", .{ identity, index });
                element = element.child(renderNode(View, cx, child, child_path));
            }
        },
    }
    return zpui.intoAnyElement(applyStyle(element, node.style));
}

fn applyStyle(initial: zpui.StatefulDiv, style: ir.Style) zpui.StatefulDiv {
    var element = initial;
    inline for (.{
        .{ "padding", "p" },
        .{ "paddingHorizontal", "px" },
        .{ "paddingVertical", "py" },
        .{ "paddingTop", "pt" },
        .{ "paddingRight", "pr" },
        .{ "paddingBottom", "pb" },
        .{ "paddingLeft", "pl" },
        .{ "margin", "m" },
        .{ "fontSize", "textSize" },
        .{ "cornerRadius", "rounded" },
        .{ "borderWidth", "border" },
    }) |mapping| {
        if (@field(style, mapping[0])) |value| element = @field(zpui.StatefulDiv, mapping[1])(element, zpui.px(value));
    }
    if (style.fontWeight) |value| element = element.fontWeight(value);
    if (style.opacity) |value| element = element.opacity(value);
    if (style.foregroundColor) |color| element = element.textColor(zpui.rgb(color).toHsla());
    if (style.backgroundColor) |color| element = element.bg(zpui.rgb(color));
    if (style.borderColor) |color| element = element.borderColor(zpui.rgb(color).toHsla());
    if (style.hidden == true) element = element.hidden();
    if (style.overflowHidden == true) element = element.overflowHidden();
    if (style.flexDirection) |axis| {
        element = element.flex();
        element = if (axis == .row) element.flexRow() else element.flexCol();
    }
    inline for (.{ .{ "width", "w" }, .{ "height", "h" } }) |mapping| {
        if (@field(style, mapping[0])) |value| {
            const length: zpui.Length = if (value == -1)
                .{ .definite = .{ .fraction = 1 } }
            else if (value == -2)
                .auto
            else
                .{ .definite = .{ .absolute = .{ .pixels = zpui.px(value) } } };
            element = @field(zpui.StatefulDiv, mapping[1])(element, length);
        }
    }
    return element;
}
