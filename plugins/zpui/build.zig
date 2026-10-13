const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    const ir = b.addModule("view_ir", .{
        .root_source_file = b.path("src/view_ir.zig"),
        .target = target,
        .optimize = optimize,
    });
    // The host adds its already configured, version-pinned zpui module here.
    // Decoding and its tests do not require a graphics backend or download.
    _ = b.addModule("crepuscularity_zpui", .{
        .root_source_file = b.path("src/renderer.zig"),
        .target = target,
        .optimize = optimize,
        .imports = &.{.{ .name = "view_ir", .module = ir }},
    });
    const tests = b.addTest(.{ .root_module = ir });
    const run = b.addRunArtifact(tests);
    b.step("test", "Test the View IR boundary without a ZPUI backend").dependOn(&run.step);
}
