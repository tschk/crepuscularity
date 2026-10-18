const std = @import("std");
const zpui = @import("zpui");
const bridge = @import("crepuscularity_zpui");
const Form = @import("form").Form;
const testing = std.testing;
const TestWindow = zpui.core.test_platform.TestWindow;

const Fixture = struct {
    app: *zpui.App,
    handle: zpui.WindowHandle(Form),
    window: *zpui.Window,
    platform: *TestWindow,
    image: *zpui.RenderImage,

    fn init(source: []const u8) !Fixture {
        const app = try zpui.App.initTest(testing.allocator);
        errdefer app.deinit();
        const image = try previewImage();
        errdefer image.release();
        const handle = try app.openWindow(.{ .bounds = .{ .origin = .zero, .size = .{ .width = 400, .height = 1200 } } }, Form, build, .{ source, image });
        const window = handle.window(app).?;
        const platform = TestWindow.of(window.platform_window);
        platform.simulateA11yActivation(true);
        var fixture = Fixture{ .app = app, .handle = handle, .window = window, .platform = platform, .image = image };
        fixture.settle();
        return fixture;
    }

    fn build(source: []const u8, image: *zpui.RenderImage, window: *zpui.Window, cx: *zpui.Context(Form)) Form {
        window.setDesktopTheme(.{ .style = .adwaita });
        window.setDesktopControls(true);
        var form = Form.init(cx.gpa(), source) catch @panic("invalid form fixture");
        form.preview = image;
        return form;
    }

    fn deinit(self: *Fixture) void {
        self.app.deinit(); // Retire frames/atlases before releasing the borrowed image.
        self.image.release();
    }

    fn view(self: *Fixture) *Form {
        var lease = self.handle.rootView(self.app).?.lease(self.app);
        defer lease.end();
        return lease.value;
    }

    fn settle(self: *Fixture) void {
        self.app.runUntilParked();
        self.window.drawAndPresent();
    }

    fn findNode(self: *Fixture, role: zpui.Role, label: []const u8) !zpui.a11y.Node {
        const tree = self.platform.a11yTree() orelse return error.MissingAccessibilityTree;
        for (tree.nodes.items) |node| {
            if (node.role == role and std.mem.eql(u8, tree.str(node.label) orelse "", label)) return node;
        }
        return error.MissingNode;
    }

    fn click(self: *Fixture, role: zpui.Role, label: []const u8) !void {
        const node = try self.findNode(role, label);
        self.platform.click(node.bounds.origin.x + node.bounds.size.width / 2, node.bounds.origin.y + node.bounds.size.height / 2);
        self.settle();
    }

    fn key(self: *Fixture, value: []const u8) void {
        self.platform.typeKey(value);
        self.settle();
    }
};

fn previewImage() !*zpui.RenderImage {
    const a = testing.allocator;
    const frames = try a.alloc(zpui.image.Frame, 1);
    errdefer a.free(frames);
    const pixels = try a.alloc(u8, 8 * 4 * 4);
    errdefer a.free(pixels);
    @memset(pixels, 255);
    frames[0] = .{ .width = 8, .height = 4, .pixels = pixels };
    return zpui.RenderImage.create(a, .{ .frames = frames });
}

test "crepus-zpui controlled toggle and checkbox update host state by mouse and keyboard" {
    var f = try Fixture.init(@embedFile("fixtures/form.json"));
    defer f.deinit();
    try f.click(.@"switch", "Enabled");
    try testing.expect(f.view().enabled);
    try testing.expectEqualStrings("enabled_changed", f.view().last_handler.?);
    f.key("space");
    try testing.expect(!f.view().enabled);
    try f.click(.check_box, "Remember");
    try testing.expect(f.view().remember);
    f.key("enter");
    try testing.expect(!f.view().remember);
    try testing.expectEqual(@as(usize, 4), f.view().changes);
}

test "crepus-zpui slider honors range and step through pointer and keyboard events" {
    var f = try Fixture.init(@embedFile("fixtures/form.json"));
    defer f.deinit();
    try f.click(.slider, "Volume");
    try testing.expect(f.view().volume >= 0 and f.view().volume <= 100);
    try testing.expectApproxEqAbs(@as(f64, 0), @mod(f.view().volume, 10), 0.001);
    f.key("home");
    try testing.expectEqual(@as(f64, 0), f.view().volume);
    f.key("right");
    try testing.expectEqual(@as(f64, 10), f.view().volume);
    f.key("end");
    try testing.expectEqual(@as(f64, 100), f.view().volume);
    const node = try f.findNode(.slider, "Volume");
    try testing.expectEqual(@as(?f64, 100), node.numeric_value);
    try testing.expectEqualStrings("volume_changed", f.view().last_handler.?);
}

test "crepus-zpui picker selection and tab panel follow the bound option value" {
    var f = try Fixture.init(@embedFile("fixtures/form.json"));
    defer f.deinit();
    try f.click(.combo_box, "mode");
    f.key("down");
    f.key("enter");
    try testing.expect(f.view().expanded);
    try testing.expectEqualStrings("mode_changed", f.view().last_handler.?);
    _ = try f.findNode(.tab_panel, "Overview");
    try f.click(.radio_button, "Details");
    try testing.expect(f.view().details);
    _ = try f.findNode(.tab_panel, "Details");
    try testing.expectError(error.MissingNode, f.findNode(.tab_panel, "Overview"));
    f.key("left");
    try testing.expect(!f.view().details);
    _ = try f.findNode(.tab_panel, "Overview");
}

test "crepus-zpui link click and keyboard activation preserve href target and rel" {
    var f = try Fixture.init(@embedFile("fixtures/form.json"));
    defer f.deinit();
    try f.click(.link, "Help");
    try testing.expectEqualStrings("https://example.com/help", f.view().last_link.?.href);
    try testing.expectEqualStrings("_blank", f.view().last_link.?.target.?);
    try testing.expectEqualStrings("noopener", f.view().last_link.?.rel.?);
    f.view().last_link = null;
    f.key("enter");
    try testing.expect(f.view().last_link != null);
}

test "crepus-zpui image produces an atlas sprite and new nodes expose accessibility" {
    var f = try Fixture.init(@embedFile("fixtures/form.json"));
    defer f.deinit();
    try testing.expect(f.window.rendered_frame.scene.polychrome_sprites.items.len > 0);
    const image = try f.findNode(.image, "Preview");
    try testing.expectEqual(@as(f32, 64), image.bounds.size.width);
    try testing.expectEqual(@as(f32, 32), image.bounds.size.height);
    _ = try f.findNode(.status, "Online");
    const sync = try f.findNode(.progress_indicator, "Sync");
    try testing.expectEqual(@as(?f64, 60), sync.numeric_value);
    const storage = try f.findNode(.progress_indicator, "Storage");
    try testing.expectEqual(@as(?f64, 10), storage.min_numeric_value);
    try testing.expectEqual(@as(?f64, 70), storage.max_numeric_value);
    var roles = std.EnumSet(zpui.Role).empty;
    for (f.platform.a11yTree().?.nodes.items) |node| roles.insert(node.role);
    try testing.expect(roles.contains(.list) and roles.contains(.list_item) and roles.contains(.separator) and roles.contains(.scroll_view));
}

test "crepus-zpui alignment sizing limits margins and shadows reach the native layout" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"stack","axis":"column","alignItems":"center","justifyContent":"between","style":{"width":240,"height":100,"padding":10},"children":[{"kind":"button","label":"First","onClick":"apply","style":{"width":40,"minWidth":60,"height":20,"marginHorizontal":8,"shadowRadius":2,"shadowOffsetX":1}},{"kind":"button","label":"Second","onClick":"apply","style":{"width":120,"maxWidth":80,"height":20,"alignSelf":"start"}}]}]}
    );
    defer f.deinit();
    const first = try f.findNode(.button, "First");
    const second = try f.findNode(.button, "Second");
    try testing.expectEqual(@as(f32, 60), first.bounds.size.width);
    try testing.expectEqual(@as(f32, 80), second.bounds.size.width);
    try testing.expectEqual(@as(f32, 90), first.bounds.origin.x);
    try testing.expectEqual(@as(f32, 10), second.bounds.origin.x);
    try testing.expectEqual(@as(f32, 10), first.bounds.origin.y);
    try testing.expectEqual(@as(f32, 70), second.bounds.origin.y);
    try testing.expect(f.window.rendered_frame.scene.shadows.items.len > 0);
}

test "crepus-zpui host validation rejects missing methods unknown bindings and wrong values" {
    var text = try bridge.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"text","content":"Snapshot","bind":"status"}]}
    );
    defer text.deinit();
    var empty: struct {} = .{};
    try testing.expectError(error.MissingBindingReader, bridge.validateHost(@TypeOf(empty), &empty, &text));
    var form = try Form.init(testing.allocator, @embedFile("fixtures/form.json"));
    defer form.document.deinit();
    try bridge.validateHost(Form, &form, &text);
    var unknown = try bridge.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"text","content":"Snapshot","bind":"missing"}]}
    );
    defer unknown.deinit();
    try testing.expectError(error.UnknownBinding, bridge.validateHost(Form, &form, &unknown));
    var wrong = try bridge.Document.parse(testing.allocator,
        \\{"version":7,"root":[{"kind":"slider","bind":"enabled","min":0,"max":100,"value":10}]}
    );
    defer wrong.deinit();
    try testing.expectError(error.BindingTypeMismatch, bridge.validateHost(Form, &form, &wrong));
    form.volume = 101;
    try testing.expectError(error.InvalidBoundRange, bridge.validateHost(Form, &form, &form.document));
}

test "crepus-zpui bound text redraws after a binding-only control change" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"stack","axis":"column","children":[{"kind":"toggle","label":"Switch","bind":"enabled","checked":false},{"kind":"text","content":"Snapshot","bind":"status"}]}]}
    );
    defer f.deinit();
    _ = try f.findNode(.label, "Disabled");
    try f.click(.@"switch", "Switch");
    try testing.expectEqual(@as(usize, 1), f.view().changes);
    try testing.expect(f.view().last_handler == null);
    _ = try f.findNode(.label, "Enabled");
    try testing.expectError(error.MissingNode, f.findNode(.label, "Disabled"));
}

test "crepus-zpui text decoration false overrides inherited decoration" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"stack","axis":"column","style":{"underline":true,"strikethrough":true,"fontSize":24,"lineHeight":1.5},"children":[{"kind":"text","content":"Plain child","style":{"underline":false,"strikethrough":false,"italic":false}}]}]}
    );
    defer f.deinit();
    const text = try f.findNode(.label, "Plain child");
    try testing.expect(text.bounds.size.height >= 24);
    for (f.window.rendered_frame.scene.underlines.items) |line| try testing.expectEqual(@as(f32, 0), line.bounds.size.height);
}

test "crepus-zpui missing control backend reports an error instead of disappearing" {
    const Diagnostic = struct {
        document: bridge.Document,
        failure: ?anyerror = null,

        pub fn render(self: *@This(), window: *zpui.Window, cx: *zpui.Context(@This())) zpui.AnyElement {
            return bridge.render(@This(), self, window, cx, &self.document, "diagnostic") catch |err| {
                self.failure = err;
                return zpui.intoAnyElement(zpui.div());
            };
        }

        pub fn deinit(self: *@This(), _: *zpui.App) void {
            self.document.deinit();
        }

        fn build(_: *zpui.Window, cx: *zpui.Context(@This())) @This() {
            return .{ .document = bridge.Document.parse(cx.gpa(), "{\"version\":7,\"root\":[{\"kind\":\"checkbox\",\"label\":\"Read only\",\"checked\":false}]}") catch @panic("invalid fixture") };
        }
    };
    const app = try zpui.App.initTest(testing.allocator);
    defer app.deinit();
    const handle = try app.openWindow(.{ .bounds = .{ .origin = .zero, .size = .{ .width = 200, .height = 100 } } }, Diagnostic, Diagnostic.build, .{});
    handle.window(app).?.drawAndPresent();
    var lease = handle.rootView(app).?.lease(app);
    defer lease.end();
    try testing.expectEqual(@as(?anyerror, error.MissingControlBackend), lease.value.failure);
}

test "crepus-zpui meter ratio stays finite across extreme valid f32 ranges" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"meter","value":0,"min":-3e38,"max":3e38}]}
    );
    defer f.deinit();
    var found_fill = false;
    for (f.window.rendered_frame.scene.quads.items) |quad| {
        if (std.meta.eql(quad.background, zpui.Background.fromRgba(zpui.rgb(0x2563eb)))) {
            try testing.expectEqual(@as(f32, 80), quad.bounds.size.width);
            found_fill = true;
        }
    }
    try testing.expect(found_fill);
}

test "crepus-zpui translation moves paint hitboxes and accessibility without moving siblings" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"stack","axis":"row","children":[{"kind":"button","label":"Moved","onClick":"apply","style":{"width":60,"height":30,"backgroundColor":"#123456","translateX":20,"translateY":40}},{"kind":"button","label":"Neighbor","style":{"width":60,"height":30}}]}]}
    );
    defer f.deinit();
    const moved = try f.findNode(.button, "Moved");
    const neighbor = try f.findNode(.button, "Neighbor");
    try testing.expectEqual(@as(f32, 20), moved.bounds.origin.x);
    try testing.expectEqual(@as(f32, 40), moved.bounds.origin.y);
    try testing.expectEqual(@as(f32, 60), neighbor.bounds.origin.x);
    try testing.expectEqual(@as(f32, 0), neighbor.bounds.origin.y);
    try f.click(.button, "Moved");
    try testing.expectEqual(@as(usize, 1), f.view().applied);
    var found = false;
    for (f.window.rendered_frame.scene.quads.items) |quad| if (quad.background.solid.eql(zpui.rgb(0x123456).toHsla())) {
        found = true;
        try testing.expectEqual(@as(f32, 20), quad.bounds.origin.x);
        try testing.expectEqual(@as(f32, 40), quad.bounds.origin.y);
    };
    try testing.expect(found);
}

test "crepus-zpui all image positions crop actual atlas sprites at the selected edge" {
    const cases = .{
        .{ "left-top", @as(i32, 0), @as(i32, 0) },
        .{ "left-bottom", @as(i32, 0), @as(i32, 2) },
        .{ "right-top", @as(i32, 4), @as(i32, 0) },
        .{ "right-bottom", @as(i32, 4), @as(i32, 2) },
        .{ "left", @as(i32, 0), @as(i32, 1) },
        .{ "right", @as(i32, 4), @as(i32, 1) },
        .{ "top", @as(i32, 2), @as(i32, 0) },
        .{ "bottom", @as(i32, 2), @as(i32, 2) },
        .{ "center", @as(i32, 2), @as(i32, 1) },
    };
    var base_x: i32 = 0;
    var base_y: i32 = 0;
    inline for (.{ "32", "8" }) |height| {
        inline for (cases, 0..) |case, index| {
            var f = try Fixture.init("{\"version\":7,\"root\":[{\"kind\":\"image\",\"src\":\"preview\",\"alt\":\"Crop\",\"style\":{\"width\":32,\"height\":" ++ height ++ ",\"objectFit\":\"cover\",\"objectPosition\":\"" ++ case[0] ++ "\"}}]}");
            defer f.deinit();
            try testing.expect(f.window.rendered_frame.scene.polychrome_sprites.items.len > 0);
            const sprite = f.window.rendered_frame.scene.polychrome_sprites.items[0];
            if (index == 0) {
                base_x = sprite.tile.bounds.origin.x;
                base_y = sprite.tile.bounds.origin.y;
            }
            if (comptime std.mem.eql(u8, height, "32")) {
                try testing.expectEqual(base_x + case[1], sprite.tile.bounds.origin.x);
                try testing.expectEqual(@as(i32, 4), sprite.tile.bounds.size.width);
            } else {
                try testing.expectEqual(base_y + case[2], sprite.tile.bounds.origin.y);
                try testing.expectEqual(@as(i32, 2), sprite.tile.bounds.size.height);
            }
        }
    }
}

test "crepus-zpui static position ignores insets while relative positioning keeps them" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"stack","axis":"column","children":[{"kind":"button","label":"Static","style":{"position":"static","left":50,"top":20,"width":60,"height":30}},{"kind":"button","label":"Relative","style":{"position":"relative","left":50,"top":20,"width":60,"height":30}}]}]}
    );
    defer f.deinit();
    const first = try f.findNode(.button, "Static");
    const second = try f.findNode(.button, "Relative");
    try testing.expectEqual(@as(f32, 0), first.bounds.origin.x);
    try testing.expectEqual(@as(f32, 0), first.bounds.origin.y);
    try testing.expectEqual(@as(f32, 50), second.bounds.origin.x);
    try testing.expectEqual(@as(f32, 50), second.bounds.origin.y);
}

test "crepus-zpui icon tabs paint decoded icons and keep mouse keyboard selection and panels" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"tabs","bind":"tab","onChange":"tab_changed","tabs":[{"value":"overview","label":"Overview","icon":"preview","children":[]},{"value":"details","label":"Details","icon":"preview","children":[]}]}]}
    );
    defer f.deinit();
    try testing.expect(f.window.rendered_frame.scene.polychrome_sprites.items.len >= 2);
    try f.click(.radio_button, "Details");
    try testing.expect(f.view().details);
    _ = try f.findNode(.tab_panel, "Details");
    f.key("left");
    try testing.expect(!f.view().details);
    _ = try f.findNode(.tab_panel, "Overview");
    f.key("end");
    try testing.expect(f.view().details);
}

test "crepus-zpui Unicode text casing inherits and normal resets the inherited mode" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"stack","axis":"column","style":{"textTransform":"uppercase"},"children":[{"kind":"text","content":"Straße"},{"kind":"text","content":"Keep Case","style":{"textTransform":"normal"}},{"kind":"text","content":"ΟΣ","style":{"textTransform":"lowercase"}}]}]}
    );
    defer f.deinit();
    _ = try f.findNode(.label, "STRASSE");
    _ = try f.findNode(.label, "Keep Case");
    _ = try f.findNode(.label, "ος");
}

test "crepus-zpui transformed choice labels preserve raw bound option values" {
    var f = try Fixture.init(
        \\{"version":7,"root":[{"kind":"tabs","bind":"tab","style":{"textTransform":"uppercase"},"tabs":[{"value":"overview","label":"Overview","children":[]},{"value":"details","label":"Details","children":[]}]}]}
    );
    defer f.deinit();
    try f.click(.radio_button, "DETAILS");
    try testing.expect(f.view().details);
}
