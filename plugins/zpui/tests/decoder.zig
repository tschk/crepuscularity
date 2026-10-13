const std = @import("std");
const ir = @import("view_ir");
const testing = std.testing;

fn visit(nodes: []const ir.Node, seen: *std.EnumSet(ir.Kind)) void {
    for (nodes) |node| {
        seen.insert(node.kind);
        visit(node.children, seen);
        for (node.options) |option| visit(option.children, seen);
    }
}

test "compiled form covers every supported node kind" {
    var document = try ir.Document.parse(testing.allocator, @embedFile("fixtures/form.json"));
    defer document.deinit();
    var seen = std.EnumSet(ir.Kind).empty;
    visit(document.root, &seen);
    try testing.expectEqual(std.EnumSet(ir.Kind).full.bits, seen.bits);
}

test "control and option strings survive the source buffer" {
    const input = try testing.allocator.dupe(u8, @embedFile("fixtures/form.json"));
    var document = ir.Document.parse(testing.allocator, input) catch |err| {
        testing.allocator.free(input);
        return err;
    };
    testing.allocator.free(input);
    defer document.deinit();
    const children = document.root[0].children;
    try testing.expectEqualStrings("enabled", children[1].bind.?);
    try testing.expectEqualStrings("Compact", children[4].options[0].label);
    try testing.expectEqualStrings("overview", children[5].options[0].value);
    try testing.expect(children[5].options[0].children.len > 0);
}

test "reject malformed control ranges and checked values with cleanup" {
    const cases = .{
        .{ error.InvalidRange, "{\"kind\":\"slider\",\"value\":5,\"min\":10,\"max\":0}" },
        .{ error.InvalidRange, "{\"kind\":\"progress\",\"value\":101,\"max\":100}" },
        .{ error.InvalidRange, "{\"kind\":\"meter\",\"value\":10,\"min\":10,\"max\":10}" },
        .{ error.InvalidStep, "{\"kind\":\"slider\",\"value\":5,\"min\":0,\"max\":10,\"step\":0}" },
        .{ error.InvalidBoolean, "{\"kind\":\"checkbox\",\"label\":\"Check\",\"checked\":1}" },
    };
    inline for (cases) |case| {
        const source = "{\"version\":7,\"root\":[" ++ case[1] ++ "]}";
        try testing.expectError(case[0], ir.Document.parse(testing.allocator, source));
    }
}

test "reject duplicate options unsupported tab icons and missing bindings" {
    try testing.expectError(error.DuplicateOption, ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"picker","bind":"mode","options":[{"value":"x","label":"One"},{"value":"x","label":"Two"}]}]}
    ));
    try testing.expectError(error.UnsupportedOptionField, ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"tabs","bind":"tab","tabs":[{"value":"x","label":"X","icon":"x.svg","children":[]}]}]}
    ));
    try testing.expectError(error.MissingBinding, ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"picker","options":[]}]}
    ));
}

test "layout and typography accept exact enum and numeric contracts" {
    var document = try ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"stack","axis":"row","alignItems":"center","justifyContent":"between","children":[],"style":{"marginLeft":-2,"marginHorizontal":4,"minWidth":30,"maxWidth":-1,"aspectRatio":2,"italic":true,"underline":false,"strikethrough":true,"fontFamily":"mono","textAlign":"right","lineHeight":1.5,"flexGrow":1,"flexShrink":0,"flexWrap":true,"alignSelf":"end","position":"absolute","top":-4,"shadowOffsetX":-1,"shadowRadius":2,"lineClamp":2,"whiteSpace":"nowrap","textOverflow":"ellipsis","cursor":"pointer"}}]}
    );
    defer document.deinit();
    const node = document.root[0];
    try testing.expectEqual(ir.styles.Align.center, node.align_items.?);
    try testing.expectEqual(ir.styles.Justify.between, node.justify_content.?);
    try testing.expectEqual(@as(f32, -2), node.style.marginLeft.?);
    try testing.expectEqual(ir.styles.FontFamily.mono, node.style.fontFamily.?);
    try testing.expectEqual(@as(u32, 2), node.style.lineClamp.?);
}

test "invalid style values fail instead of mapping to an approximation" {
    const cases = .{
        .{ error.UnsupportedEnumValue, "\"position\":\"fixed\"" },
        .{ error.UnsupportedEnumValue, "\"cursor\":\"wait\"" },
        .{ error.UnsupportedObjectPosition, "\"objectPosition\":\"top\"" },
        .{ error.InvalidStyleNumber, "\"aspectRatio\":0" },
        .{ error.InvalidStyleNumber, "\"flexGrow\":-1" },
        .{ error.InvalidFontWeight, "\"fontWeight\":400.5" },
        .{ error.InvalidLineClamp, "\"lineClamp\":1.5" },
        .{ error.InvalidNumber, "\"width\":1e39" },
        .{ error.UnsupportedColor, "\"backgroundColor\":\"#+12345\"" },
        .{ error.UnsupportedColor, "\"foregroundColor\":\"#12_345\"" },
        .{ error.UnsupportedColor, "\"borderColor\":\"#12345G\"" },
    };
    inline for (cases) |case| {
        const source = "{\"version\":7,\"root\":[{\"kind\":\"text\",\"content\":\"X\",\"style\":{" ++ case[1] ++ "}}]}";
        try testing.expectError(case[0], ir.Document.parse(testing.allocator, source));
    }
}

test "remaining seven node kinds stay explicit unsupported errors" {
    inline for (.{ "if", "forEach", "input", "slotRotate", "webView", "dropzone", "filePicker" }) |kind| {
        try testing.expectError(error.UnsupportedNode, ir.Document.parse(testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"" ++ kind ++ "\"}]}"));
    }
}
