const std = @import("std");

pub fn build(b: *std.Build) void {
    buildTests(b, "", "../../");
}

pub fn buildTests(b: *std.Build, comptime source_prefix: []const u8, comptime repo_prefix: []const u8) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    const test_step = b.step("test", "Compile and run Zig CLI and ABI adapter tests");
    inline for (.{ "crepuscularity.zig", "crepuscularity_abi.zig" }) |filename| {
        const module = b.createModule(.{
            .root_source_file = b.path(source_prefix ++ filename),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
        });
        if (comptime std.mem.eql(u8, filename, "crepuscularity_abi.zig")) {
            module.addIncludePath(b.path(repo_prefix ++ "crates/crepuscularity-abi/include"));
            module.addLibraryPath(b.path(repo_prefix ++ "target/debug"));
            module.addRPath(.{ .cwd_relative = b.pathResolve(&.{ b.graph.cache.cwd, b.build_root.path orelse ".", repo_prefix ++ "target/debug" }) });
            module.linkSystemLibrary("crepuscularity_abi", .{});
        }
        const tests = b.addTest(.{ .root_module = module });
        const run = b.addRunArtifact(tests);
        run.setCwd(b.path(source_prefix));
        test_step.dependOn(&run.step);
    }
}
