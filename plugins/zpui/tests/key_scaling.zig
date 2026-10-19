//! Explicit ReleaseFast key-validation benchmark; includes host lookup and validation.
const std = @import("std");
const host = @import("host");
extern "c" fn clock() c_long;
const View = struct {
    items: []const host.Item,
    pub fn crepusValue(self: *@This(), _: []const u8) ?host.Value {
        return .{ .list = self.items };
    }
};

test "manual collection key scaling" {
    const a = std.testing.allocator;
    for ([_]usize{ 1000, 2000, 4000 }) |count| {
        const items = try a.alloc(host.Item, count);
        defer a.free(items);
        for (items, 0..) |*item, index| item.* = .{
            .key = try std.fmt.allocPrint(a, "row-{d:0>8}", .{index}),
            .value = .{ .number = @floatFromInt(index) },
        };
        defer for (items) |item| a.free(item.key);
        var view: View = .{ .items = items };
        for ([_]bool{ false, true }) |duplicate| {
            const last = items[count - 1].key;
            if (duplicate) items[count - 1].key = items[0].key;
            defer items[count - 1].key = last;
            const start = clock();
            for (0..25) |_| {
                if (duplicate) {
                    try std.testing.expectError(error.DuplicateCollectionKey, host.collection(View, &view, "rows", null));
                } else {
                    const found = try host.collection(View, &view, "rows", null);
                    try std.testing.expect(found.ptr == items.ptr);
                    try std.testing.expectEqual(@as(f64, @floatFromInt(count - 1)), found[count - 1].value.number);
                    std.mem.doNotOptimizeAway(found.ptr);
                }
            }
            const elapsed = @as(f64, @floatFromInt(clock() - start)) / 1000.0;
            std.debug.print("keys={d} duplicate={} iterations=25 cpu_ms={d:.3}\n", .{ count, duplicate, elapsed });
        }
    }
}
