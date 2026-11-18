const std = @import("std");

pub fn build(b: *std.Build) void {
    const test_step = b.step("test", "Run Zig plugin tests");
    const smoke = b.addSystemCommand(&.{ "/bin/sh", "-c", "if [ -f plugins/fixtures/hello.crepus ]; then fixture=plugins/fixtures/hello.crepus; else fixture=fixtures/hello.crepus; fi; \"${CREPUS_BIN:-crepus}\" native ir \"$fixture\" | grep '\"version\":7'" });
    test_step.dependOn(&smoke.step);
}
