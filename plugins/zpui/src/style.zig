//! Exact mappings for the supported ViewStyle hints.
const zpui = @import("zpui");
const ir = @import("view_ir");

pub fn alignItems(initial: zpui.StatefulDiv, value: ir.styles.Align) zpui.StatefulDiv {
    return switch (value) {
        .start => initial.itemsStart(),
        .end => initial.itemsEnd(),
        .center => initial.itemsCenter(),
        .stretch => initial.itemsStretch(),
        .baseline => initial.itemsBaseline(),
    };
}

pub fn justify(initial: zpui.StatefulDiv, value: ir.styles.Justify) zpui.StatefulDiv {
    return switch (value) {
        .start => initial.justifyStart(),
        .end => initial.justifyEnd(),
        .center => initial.justifyCenter(),
        .between => initial.justifyBetween(),
        .around => initial.justifyAround(),
        .evenly => initial.justifyEvenly(),
    };
}

pub fn length(value: f32) zpui.Length {
    return if (value == -1) .{ .definite = .{ .fraction = 1 } } else if (value == -2) .auto else .{ .definite = .{ .absolute = .{ .pixels = zpui.px(value) } } };
}

pub fn apply(initial: zpui.StatefulDiv, style: ir.Style) zpui.StatefulDiv {
    var element = initial;
    inline for (.{ .{ "padding", "p" }, .{ "paddingHorizontal", "px" }, .{ "paddingVertical", "py" }, .{ "paddingTop", "pt" }, .{ "paddingRight", "pr" }, .{ "paddingBottom", "pb" }, .{ "paddingLeft", "pl" }, .{ "margin", "m" }, .{ "marginHorizontal", "mx" }, .{ "marginVertical", "my" }, .{ "marginTop", "mt" }, .{ "marginRight", "mr" }, .{ "marginBottom", "mb" }, .{ "marginLeft", "ml" }, .{ "fontSize", "textSize" }, .{ "cornerRadius", "rounded" }, .{ "borderWidth", "border" }, .{ "top", "top" }, .{ "right", "right" }, .{ "bottom", "bottom" }, .{ "left", "left" } }) |mapping| {
        if (@field(style, mapping[0])) |v| element = @field(zpui.StatefulDiv, mapping[1])(element, zpui.px(v));
    }
    inline for (.{ .{ "width", "w" }, .{ "height", "h" }, .{ "minWidth", "minW" }, .{ "minHeight", "minH" }, .{ "maxWidth", "maxW" }, .{ "maxHeight", "maxH" } }) |mapping| {
        if (@field(style, mapping[0])) |v| element = @field(zpui.StatefulDiv, mapping[1])(element, length(v));
    }
    if (style.aspectRatio) |v| element = element.aspectRatio(v);
    if (style.fontWeight) |v| element = element.fontWeight(v);
    if (style.opacity) |v| element = element.opacity(v);
    if (style.foregroundColor) |v| element = element.textColor(zpui.rgb(v).toHsla());
    if (style.backgroundColor) |v| element = element.bg(zpui.rgb(v));
    if (style.borderColor) |v| element = element.borderColor(zpui.rgb(v).toHsla());
    if (style.hidden == true) element = element.hidden();
    if (style.overflowHidden == true) element = element.overflowHidden();
    if (style.flexDirection) |axis| {
        element = element.flex();
        element = if (axis == .row) element.flexRow() else element.flexCol();
    }
    if (style.flexGrow) |v| element = element.flexGrow(v);
    if (style.flexShrink) |v| element = element.flexShrink(v);
    if (style.flexWrap) |v| element = if (v) element.flexWrap() else element.flexNowrap();
    if (style.alignSelf) |v| element = switch (v) {
        .start => element.selfStart(),
        .end => element.selfEnd(),
        .center => element.selfCenter(),
        .stretch => element.selfStretch(),
        .baseline => element.selfBaseline(),
    };
    if (style.position) |v| element = if (v == .absolute) element.absolute() else element.relative();
    if (style.textAlign) |v| element = element.textAlign(switch (v) {
        .left => .left,
        .center => .center,
        .right => .right,
    });
    if (style.lineHeight) |v| element = element.lineHeight(zpui.relative(v));
    if (style.fontFamily) |v| element = element.fontFamily(switch (v) {
        .sans => ".SystemUIFont",
        .serif => "serif",
        .mono => "monospace",
    });
    if (style.italic) |v| element = if (v) element.italic() else element.notItalic();
    if (style.underline) |v| {
        if (v) element = element.underline() else element.style().text.underline = .{ .thickness = 0 };
    }
    if (style.strikethrough) |v| {
        if (v) element = element.lineThrough() else element.style().text.strikethrough = .{ .thickness = 0 };
    }
    if (style.whiteSpace) |v| element = if (v == .nowrap) element.whitespaceNowrap() else element.whitespaceNormal();
    if (style.textOverflow) |v| {
        if (v == .ellipsis) element = element.textEllipsis() else element = element.overflowHidden();
    }
    if (style.lineClamp) |v| element = element.lineClamp(v);
    if (style.cursor) |v| element = element.cursor(switch (v) {
        .auto, .default => .arrow,
        .pointer => .pointing_hand,
        .text => .ibeam,
        .move => .open_hand,
    });
    if (style.shadowRadius != null or style.shadowColor != null or style.shadowOffsetX != null or style.shadowOffsetY != null) {
        const shadows = zpui.frameAllocator().alloc(zpui.style.BoxShadow, 1) catch @panic("OOM");
        shadows[0] = .{ .color = zpui.rgb(style.shadowColor orelse 0).toHsla(), .offset = .{ .x = style.shadowOffsetX orelse 0, .y = style.shadowOffsetY orelse 0 }, .blur_radius = style.shadowRadius orelse 0 };
        element = element.shadow(shadows);
    }
    return element;
}
