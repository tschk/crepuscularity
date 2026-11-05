# Target capability review, 2026-11-05

This review covers the embedded renderer, GPUI runtime events, and spacing
utilities. It is not a complete audit of the repository.

## Embedded templates without an on-device parser

The embedded renderer supports layout and painting without `std`, but the
runtime parser requires `std`. A compile-time template path can instead let
firmware update variables, lay out, and paint without an on-device parser.

PR [#856](https://github.com/tschk/crepuscularity/pull/856) restores the
`no_std` build and adds a Thumb firmware check to CI. PR
[#853](https://github.com/tschk/crepuscularity/pull/853) proposes compiling a
bounded template into data that firmware can render with `alloc`. The embedded
implementation rejects animations and unsupported events, which its original
compiler silently ignored. Tests compare representative pixels and hit targets with
the existing `std` renderer, but utility class parity is incomplete. Hardware
behavior still needs validation on a physical panel.

## GPUI events after template reload

The runtime renderer does not attach template event handlers. It displays an
error in place of an element that declares an event instead of silently
rendering an inert control. The compile-time macro and runtime preview have
different interaction contracts.
PR [#853](https://github.com/tschk/crepuscularity/pull/853) proposes a named
click-handler registry for hot-reloaded templates. Its scope is click events,
not general event parity. Without a registry, it leaves clicks inert. Any
replacement must diagnose unsupported events as well as unknown handlers. A
useful acceptance case changes an included component on disk and then
dispatches a click with values from its current scope.

## Spacing differs between targets

The core point parser accepts decimal multipliers, while the integer pixel
parser uses a smaller table. A class such as `p-0.25` can therefore be
interpreted on one target and ignored on another. PR
[#857](https://github.com/tschk/crepuscularity/pull/857) proposes a common
Tailwind v4 quarter-step rule, with a separate copy for `no_std` builds and a
parity test. The change also rejects numeric spellings accepted today, so its
compatibility impact deserves review before merge.

## Portability diagnostics

`crepus inspect` can print the AST, HTML, or View IR, but it does not compare
target capabilities. A target-aware inspection mode could identify unsupported
events, embedded template constructs, and inconsistent utility classes before
an app is deployed. Such a mode needs diagnostics tied to source locations and
must distinguish a hard error from an intentionally degraded preview. The
existing target renderers and their tests should define the initial rules.
