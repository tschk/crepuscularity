# ZPUI integration (experimental)

[`plugins/zpui`](../plugins/zpui/README.md) provides an optional Zig renderer for
Crepuscularity's [View IR v7](view-ir-contract.md). It builds stacks, text,
buttons, scrolling, native form controls, links, images, lists and status/layout
elements through a host-supplied ZPUI module. All 25 node shapes decode; 24 have
direct implementations, including a built-in retained editor. Browser hosting has a
macOS WKWebView reference; other platforms require a host factory.
Scoped conditions/collections, entity-owned rotation timers, long press and files
are also implemented. Typed binding readers and change events connect controls to host-owned
Zig state; unsupported behavior and invalid values produce errors.

The adapter targets
[`plyght/zpui@cca22983e7e8124bb8e650616e3803f217e52b95`](https://github.com/plyght/zpui/tree/cca22983e7e8124bb8e650616e3803f217e52b95),
which declares Zig 0.17.0 as its minimum version. Twenty-one decoder tests and
forty-four native headless tests cover compiler-generated counter and form
fixtures, mouse/keyboard activation, selection and bindings, layout/color,
scrolling, image atlas output, accessibility, and document replacement against
that revision. [The completion goal](../plugins/zpui/COVERAGE.md) tracks all 25 kinds,
71 styles and five event contracts, including the remaining browser platforms,
style/backend work and acceptance gates. Real GPU output and other platforms
are outside this acceptance scope.

See the [adapter guide](../plugins/zpui/README.md) for module wiring, ownership,
coverage, validation commands, and ISC/MIT licensing. This does not
change the Rust GPUI CE or Kit adapters.
