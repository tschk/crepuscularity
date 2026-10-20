const std = @import("std");

pub fn build(b: *std.Build) void {
    @import("zig/build.zig").buildTests(b, "zig/", "../");
}
