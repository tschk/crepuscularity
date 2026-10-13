# ZPUI integration (experimental)

[`plugins/zpui`](../plugins/zpui/README.md) provides an optional Zig renderer for
Crepuscularity's [View IR v7](view-ir-contract.md). It builds stacks, text,
buttons, and scrolling containers through a host-supplied ZPUI module. Named
click handlers reach host-owned Zig state; unsupported controls and style hints
produce errors.

The adapter targets
[`plyght/zpui@cca22983e7e8124bb8e650616e3803f217e52b95`](https://github.com/plyght/zpui/tree/cca22983e7e8124bb8e650616e3803f217e52b95),
which declares Zig 0.17.0 as its minimum version. Eight decoder tests pass on
Zig 0.16.0 and 0.17.0. Four native headless tests compile the renderer and counter
view, then exercise mouse/keyboard clicks, focus, layout/color, scrolling, and
document replacement against that revision. Real GPU output and other platforms
are outside this acceptance scope.

See the [adapter guide](../plugins/zpui/README.md) for module wiring, ownership,
coverage, validation commands, and ISC/MIT licensing. This does not
change the Rust GPUI CE or Kit adapters.
