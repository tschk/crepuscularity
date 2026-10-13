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
    const decoder_tests = b.createModule(.{
        .root_source_file = b.path("tests/decoder.zig"),
        .target = target,
        .optimize = optimize,
        .imports = &.{.{ .name = "view_ir", .module = ir }},
    });
    const tests = b.addTest(.{ .root_module = decoder_tests });
    const run = b.addRunArtifact(tests);
    const original_tests = b.addTest(.{ .root_module = ir });
    const original_run = b.addRunArtifact(original_tests);
    const step = b.step("test", "Test the View IR boundary without a ZPUI backend");
    step.dependOn(&original_run.step);
    step.dependOn(&run.step);
}
