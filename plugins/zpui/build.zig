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
    const renderer = b.addModule("crepuscularity_zpui", .{
        .root_source_file = b.path("src/renderer.zig"),
        .target = target,
        .optimize = optimize,
        .imports = &.{.{ .name = "view_ir", .module = ir }},
    });
    // This command is lazy: decoding/tests do not build or download the native helper.
    // Native builds use the host's Rust target; cross builds must specify an installed
    // rust target via -Drust-target (the Zig and Rust ABIs must agree).
    const rust_target = b.option([]const u8, "rust-target", "Installed Rust target matching the Zig target") orelse rustTriple(target.result) orelse "unsupported-target-requires-rust-target-option";
    const unicode = b.addSystemCommand(&.{ "cargo", "build", "--locked", "-j1", "--release", "--manifest-path" });
    unicode.addFileArg(b.path("native-text/Cargo.toml"));
    unicode.addFileInput(b.path("native-text/Cargo.lock"));
    unicode.addFileInput(b.path("native-text/src/lib.rs"));
    unicode.addArgs(&.{ "--target", rust_target, "--target-dir" });
    const output = unicode.addOutputDirectoryArg("native-text");
    const library = if (target.result.os.tag == .windows and target.result.abi == .msvc) "crepus_zpui_text.lib" else "libcrepus_zpui_text.a";
    if (target.result.os.tag == .macos) renderer.linkFramework("WebKit", .{ .needed = true });
    renderer.addObjectFile(output.path(b, b.fmt("{s}/release/{s}", .{ rust_target, library })));
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

fn rustTriple(target: std.Target) ?[]const u8 {
    if (target.cpu.arch != .aarch64 and target.cpu.arch != .x86_64) return null;
    return switch (target.os.tag) {
        .macos => if (target.cpu.arch == .aarch64) "aarch64-apple-darwin" else "x86_64-apple-darwin",
        .linux => if (target.abi == .gnu) (if (target.cpu.arch == .aarch64) "aarch64-unknown-linux-gnu" else "x86_64-unknown-linux-gnu") else null,
        .windows => if (target.cpu.arch != .x86_64) null else switch (target.abi) {
            .msvc => "x86_64-pc-windows-msvc",
            .gnu => "x86_64-pc-windows-gnu",
            else => null,
        },
        else => null,
    };
}
