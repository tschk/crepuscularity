const std = @import("std");
const ir = @import("view_ir");
const testing = std.testing;

fn visit(nodes: []const ir.Node, seen: *std.EnumSet(ir.Kind)) void {
    for (nodes) |node| {
        seen.insert(node.kind);
        visit(node.children, seen);
        visit(node.else_children, seen);
        for (node.options) |option| visit(option.children, seen);
    }
}

test "compiled form covers all eighteen original node kinds" {
    var document = try ir.Document.parse(testing.allocator, @embedFile("fixtures/form.json"));
    defer document.deinit();
    var seen = std.EnumSet(ir.Kind).empty;
    visit(document.root, &seen);
    try testing.expectEqual(@as(usize, 18), seen.count());
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

test "reject duplicate options and missing bindings while preserving tab icons" {
    try testing.expectError(error.DuplicateOption, ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"picker","bind":"mode","options":[{"value":"x","label":"One"},{"value":"x","label":"Two"}]}]}
    ));
    var icons = try ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"tabs","bind":"tab","tabs":[{"value":"x","label":"X","icon":"x.svg","children":[]}]}]}
    );
    defer icons.deinit();
    try testing.expectEqualStrings("x.svg", icons.root[0].options[0].icon.?);
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
        .{ error.UnsupportedEnumValue, "\"objectPosition\":\"midway\"" },
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

test "runtime decoder owns conditions collections input timing web and file metadata" {
    var doc = try ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"if","condition":"ready","thenChildren":[],"elseChildren":[]},{"kind":"forEach","bind":"rows","itemName":"row","itemBody":[]},{"kind":"input","bind":"name","placeholder":"Name","secure":true,"multiline":true,"onChange":"edit"},{"kind":"slotRotate","phrases":["One","Two"],"intervalMs":250},{"kind":"webView","src":"https://example.com"},{"kind":"dropzone","label":"Files","accept":".png, .jpg","onDrop":"drop","children":[]},{"kind":"filePicker","label":"Choose","accept":[".txt"],"multiple":true,"onPick":"pick"}]}
    );
    defer doc.deinit();
    try testing.expectEqualStrings("row", doc.root[1].item_name);
    try testing.expect(doc.root[2].secure and doc.root[2].multiline);
    try testing.expectEqual(@as(u64, 250), doc.root[3].interval_ms);
    try testing.expectEqualStrings("Two", doc.root[3].phrases[1]);
    try testing.expectEqualStrings(".jpg", doc.root[5].accept[1]);
    try testing.expect(doc.root[6].multiple);
    var seen = std.EnumSet(ir.Kind).empty;
    visit(doc.root, &seen);
    var form = try ir.Document.parse(testing.allocator, @embedFile("fixtures/form.json"));
    defer form.deinit();
    visit(form.root, &seen);
    try testing.expectEqual(std.EnumSet(ir.Kind).full.bits, seen.bits);
}

test "runtime malformed payloads fail with arena cleanup" {
    const cases = .{
        .{ error.EmptyCondition, "{\"kind\":\"if\",\"condition\":\"\",\"thenChildren\":[]}" },
        .{ error.EmptyItemName, "{\"kind\":\"forEach\",\"bind\":\"rows\",\"itemName\":\"\",\"itemBody\":[]}" },
        .{ error.MissingBinding, "{\"kind\":\"input\",\"placeholder\":\"Name\"}" },
        .{ error.EmptyPhrases, "{\"kind\":\"slotRotate\",\"phrases\":[],\"intervalMs\":1}" },
        .{ error.InvalidInterval, "{\"kind\":\"slotRotate\",\"phrases\":[\"A\"],\"intervalMs\":0}" },
        .{ error.InvalidInterval, "{\"kind\":\"slotRotate\",\"phrases\":[\"A\"],\"intervalMs\":18446744073710}" },
        .{ error.InvalidStringArray, "{\"kind\":\"filePicker\",\"label\":\"Choose\",\"accept\":\".txt\"}" },
    };
    inline for (cases) |case| try testing.expectError(case[0], ir.Document.parse(testing.allocator, "{\"version\":7,\"root\":[" ++ case[1] ++ "]}"));
}

test "all gradient directions and complete endpoints are validated" {
    inline for (.{ "to-t", "to-tr", "to-r", "to-br", "to-b", "to-bl", "to-l", "to-tl" }) |direction| {
        var doc = try ir.Document.parse(testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"text\",\"content\":\"Gradient\",\"style\":{\"backgroundGradientDirection\":\"" ++ direction ++ "\",\"backgroundGradientFrom\":\"#000000\",\"backgroundGradientTo\":\"#FFFFFF\"}}]}");
        doc.deinit();
    }
    try testing.expectError(error.IncompleteGradient, ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"text","content":"Gradient","style":{"backgroundGradientDirection":"to-r","backgroundGradientFrom":"#000000"}}]}
    ));
}

test "source classes are retained in owned document storage" {
    const source = try testing.allocator.dupe(u8,
        \\{"version":7,"root":[{"kind":"text","content":"X","style":{"classes":["text-lg","theme-蓝"]}}]}
    );
    var doc = ir.Document.parse(testing.allocator, source) catch |err| {
        testing.allocator.free(source);
        return err;
    };
    testing.allocator.free(source);
    defer doc.deinit();
    try testing.expectEqualStrings("theme-蓝", doc.root[0].style.classes.?[1]);
}

test "nonempty malformed drop filters never become unrestricted" {
    inline for (.{ ",", " ", " , , " }) |accept| {
        try testing.expectError(error.InvalidAccept, ir.Document.parse(testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"dropzone\",\"label\":\"Files\",\"accept\":\"" ++ accept ++ "\",\"children\":[]}]}"));
    }
    var doc = try ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"dropzone","label":"Files","accept":"","children":[]},{"kind":"dropzone","label":"Files","children":[]}]}
    );
    defer doc.deinit();
    try testing.expectEqual(@as(usize, 0), doc.root[0].accept.len);
    try testing.expectEqual(@as(usize, 0), doc.root[1].accept.len);
}

test "all image positions and supported text transforms decode typed values" {
    inline for (.{ "center", "top", "bottom", "left", "right", "left-top", "left-bottom", "right-top", "right-bottom" }) |position| {
        var doc = try ir.Document.parse(testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"image\",\"src\":\"preview\",\"style\":{\"objectPosition\":\"" ++ position ++ "\"}}]}");
        doc.deinit();
    }
    inline for (.{ "normal", "uppercase", "lowercase" }) |mode| {
        var doc = try ir.Document.parse(testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"text\",\"content\":\"X\",\"style\":{\"textTransform\":\"" ++ mode ++ "\"}}]}");
        doc.deinit();
    }
    try testing.expectError(error.UnsupportedEnumValue, ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"text","content":"X","style":{"textTransform":"capitalize"}}]}
    ));
}

test "HTML display contents metadata is validated and ignored by native layout" {
    inline for (.{ "true", "false", "null" }) |hint| {
        var document = try ir.Document.parse(testing.allocator, "{\"version\":7,\"root\":[{\"kind\":\"stack\",\"axis\":\"row\",\"style\":{\"htmlDisplayContents\":" ++ hint ++ ",\"padding\":6},\"children\":[{\"kind\":\"text\",\"content\":\"Child\"}]}]}");
        defer document.deinit();
        try testing.expectEqual(ir.styles.Axis.row, document.root[0].axis);
        try testing.expectEqual(@as(f32, 6), document.root[0].style.padding.?);
        try testing.expectEqualStrings("Child", document.root[0].children[0].text);
    }
    try testing.expectError(error.InvalidBoolean, ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"stack","axis":"column","style":{"htmlDisplayContents":"true"},"children":[]}]}
    ));
    try testing.expectError(error.UnsupportedStyle, ir.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"stack","axis":"column","style":{"htmlDisplayContent":true},"children":[]}]}
    ));
}
