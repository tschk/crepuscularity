#!/usr/bin/env python3
"""Run adapter tests with the real pinned ZPUI module; never download dependencies."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    """Validate the pinned inputs and run native acceptance in a temporary build."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--zig", default="zig")
    parser.add_argument("--zpui-source", required=True, type=Path)
    parser.add_argument("--desktop", action="store_true", help="Open a real macOS acceptance window and verify browser pixels")
    args = parser.parse_args()
    plugin = Path(__file__).resolve().parents[1]
    upstream = args.zpui_source.resolve()
    revision = subprocess.check_output(["git", "-C", str(upstream), "rev-parse", "HEAD"], text=True).strip()
    if revision != "cca22983e7e8124bb8e650616e3803f217e52b95":
        parser.error(f"expected pinned ZPUI revision, got {revision}")
    dirty = subprocess.check_output(["git", "-C", str(upstream), "status", "--porcelain", "--untracked-files=no"], text=True).splitlines()
    # On case-insensitive macOS filesystems these provenance files collide with
    # each library's source/ directory. They are not compiler inputs. All other
    # tracked changes, including source/header/build changes, are rejected.
    metadata_collisions = {" D vendor/lunasvg/SOURCE", " D vendor/plutovg/SOURCE"}
    for change in dirty:
        if change not in metadata_collisions or not (upstream / change[3:]).is_dir():
            parser.error("native acceptance requires unchanged upstream compiler inputs")
    if subprocess.check_output([args.zig, "version"], text=True).strip() != "0.17.0":
        parser.error("native acceptance requires Zig 0.17.0")
    with tempfile.TemporaryDirectory(prefix="crepus-zpui-native-") as temporary:
        root = Path(temporary).resolve()
        (root / "build.zig.zon").write_text(
            '.{ .name = .crepus_zpui_native_validation, .version = "0.1.0", .fingerprint = 0xe52b485812e49944, '
            '.minimum_zig_version = "0.17.0", .paths = .{"build.zig", "build.zig.zon"}, '
            '.dependencies = .{ .bridge = .{ .path = ' + json.dumps(os.path.relpath(plugin, root)) +
            ' }, .zpui = .{ .path = ' + json.dumps(os.path.relpath(upstream, root)) + ' } } }\n'
        )
        desktop = '''
    const swift = b.addSystemCommand(&.{"xcrun", "swiftc", "-emit-library", "-parse-as-library"});
    swift.addFileArg(bridge_dep.path("tools/mac-snapshot.swift"));
    swift.addArg("-o");
    const snapshot = swift.addOutputFileArg("libmac_snapshot.dylib");
    const desktop_root = b.createModule(.{
        .root_source_file = bridge_dep.path("tests/mac_desktop.zig"), .target = target, .optimize = optimize,
        .imports = &.{ .{ .name = "zpui", .module = zpui }, .{ .name = "crepuscularity_zpui", .module = bridge } },
    });
    desktop_root.addObjectFile(snapshot);
    const desktop = b.addExecutable(.{ .name = "crepus-zpui-desktop-acceptance", .root_module = desktop_root });
    b.step("desktop", "Verify real AppKit/Metal input and WKWebView pixels").dependOn(&b.addRunArtifact(desktop).step);
''' if args.desktop else ""
        (root / "build.zig").write_text('''const std = @import("std");
pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    const upstream = b.dependency("zpui", .{ .target = target, .optimize = optimize });
    const zpui = upstream.module("zpui");
    const bridge_dep = b.dependency("bridge", .{ .target = target, .optimize = optimize });
    const bridge = bridge_dep.module("crepuscularity_zpui");
    bridge.addImport("zpui", zpui);
    const counter = b.createModule(.{
        .root_source_file = bridge_dep.path("examples/counter.zig"),
        .target = target, .optimize = optimize,
        .imports = &.{ .{ .name = "zpui", .module = zpui }, .{ .name = "crepuscularity_zpui", .module = bridge } },
    });
    const form = b.createModule(.{
        .root_source_file = bridge_dep.path("examples/form.zig"),
        .target = target, .optimize = optimize,
        .imports = &.{ .{ .name = "zpui", .module = zpui }, .{ .name = "crepuscularity_zpui", .module = bridge } },
    });
    const root = b.createModule(.{
        .root_source_file = bridge_dep.path("tests/native.zig"),
        .target = target, .optimize = optimize,
        .imports = &.{ .{ .name = "zpui", .module = zpui }, .{ .name = "crepuscularity_zpui", .module = bridge }, .{ .name = "counter", .module = counter }, .{ .name = "form", .module = form } },
    });
    const tests = b.addTest(.{ .root_module = root, .filters = &.{"crepus-zpui"} });
    b.step("test", "Run adapter acceptance on the real headless ZPUI platform")
        .dependOn(&b.addRunArtifact(tests).step);
}
'''.replace("\n}\n", desktop + "\n}\n"))
        completed = subprocess.run(
            [args.zig, "build", "desktop" if args.desktop else "test", "-j1", "--summary", "all"],
            cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
        print(completed.stdout.decode("utf-8", errors="replace"), end="")
        completed.check_returncode()
        if b"error(SafeAllocator)" in completed.stdout:
            raise SystemExit("Native acceptance failed allocator teardown")


if __name__ == "__main__":
    main()
