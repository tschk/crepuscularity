const std = @import("std");

pub fn build(b: *std.Build) void {
    const test_step = b.step("test", "Run tests");
    const smoke = b.addSystemCommand(&.{ "/bin/sh", "-c", "\"${CREPUS_BIN:-crepus}\" native ir plugins/fixtures/hello.crepus | grep '\"version\":7'" });
    test_step.dependOn(&smoke.step);
}
