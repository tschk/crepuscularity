# Bolt performance notes

## Embedded PPM export

`Rgb888Buffer::write_ppm` must reuse `as_rgb888_bytes()` for its P6 payload.
Writing each three-byte pixel directly to `File` caused one write per pixel.
A 640x480 debug-build host probe (seven existing-file exports, sorted median)
measured 441532 us before and 385 us after bulk writing, with identical bytes.
These timings exclude durability guarantees; no `sync_all` is requested.
Keep exact header/channel-order and empty-buffer coverage in `ppm_bytes.rs`.

## Compiler-owned SSR fragments

Generated JSX fragments passed through `h(Component)`, allocating component scopes
for transparent grouping. Bypass only the fresh compiler fragment marker; authored
components must retain context, readiness, and cleanup boundaries. Keep DOM output
unchanged and remove the host import when every reference was an optimized fragment
so fragment-only modules still pass TypeScript `--noUnusedLocals`.
A local two-component/eight-fragment fixture (21 alternating samples of 3000 fresh
SSR renders, after warmup) measured median 16.69 ms before and 7.20 ms after, with
identical HTML and lifecycle traces. This is a fixture-specific timing, not a general
SSR or bundle-size claim; generated TypeScript grew by 58 bytes from annotations.


## GPUI target defaults

Ordinary HotReload contexts omit target defaults. Calling the immutable renderer
there deep-cloned every unused List/Scope payload on each redraw. Temporarily
insert only missing defaults into the owned context and restore keys on unwind;
return the GPUI entity lease before resuming a renderer panic. Keep immutable
public render calls unchanged. An actual TestAppContext HotReloadView window
fixture (five warmups, twenty forced draws) measured 100/400 unused rows at
566/2066 allocations and 107724/379824 bytes per draw before, versus 57 allocations
and 12643 bytes for both after. These are allocated bytes, not retained memory or
frame latency. Pending structural refresh and external captured slot contexts can
still clone independently. Evidence: /tmp/crepus-runtime-target-defaults-fix/.


## Embedded panel byte extraction

`Ui::rgb565_for_panel` must borrow the framebuffer and reusable scratch as disjoint
fields. Calling `self.rgb565().to_vec()` first cloned the whole source frame on
every Bgr extraction even after scratch was warm; `flush()` already avoided this.
The public-accessor regression at 240x320 measured ten allocations / 1536000 bytes
for ten warmed Bgr extractions before, versus zero allocations / bytes after.
It verifies RGB borrowing, exact swapped bytes, updated red/blue pixels, scratch
pointer reuse and empty buffers. These are allocation counts, not panel timings.
Keep `tests/panel_bytes.rs`; no hardware driver behavior changed.

## Reactive disposal queue membership

Unqueued nodes must not retain-scan pending FIFOs: membership sets already prove
absence. Disposing N unqueued effects while N others are pending performed N²
comparisons. A public-API release probe (three warmups, fifteen sorted samples,
setup/flush/cleanup excluded) measured N=1000/4000/8000 medians of
862291/12703666/47731709 ns before versus 120375/582792/1578333 ns after.
Guard both FIFO scans with their membership-set removal result. Actual queued
disposals still scan; this is not a frame-latency bound. Keep mixed memo/effect
batch and queued-disposal coverage. Evidence: /tmp/crepus-reactive-disposal-fix/.

## Rejected GPUI include negative caching

Do not memoize IncludeCache resolution/read/parse errors merely because its maps
store Result. `failed_named_parses_do_not_poison_the_render_cache` deliberately
requires a missing file, then an invalid file, then a repaired component to retry
within the same cache. Success-only source publication also preserves frontend
partitioning. A debug probe of 1000 repeated missing lookups measured median
1.634 ms before versus 1.314–1.400 ms with negative caching (three warmups, fifteen
samples), but the full runtime gate exposed the recovery regression. Rejected
and reverted the optimization; keep the recovery contract when measuring future
error-path work. Evidence: `/tmp/crepus-gpui-error-cache-fix/`. These are lookup
fixture timings, not physical-window frame latency.

## Reactive text binding mutations

`bind_text` must inspect child structure before skipping equal text: aggregate
`textContent` equality alone would retain markup/comments instead of flattening
them. Reuse only one Text child; clear all children for an empty value. An actual
Chromium/WASM effect probe measured 1000 child-list replacements for 1000 unchanged
text reruns before, versus zero DOM mutation records after. A changed nonempty
value now emits one character-data record while retaining the Text node. These
are mutation counts, not frame-latency or elapsed-time measurements; observer
mutation types and node identity intentionally change. Keep structural cleanup,
empty-value and disposal checks in `tests/dom_bind.rs`. Evidence:
`/tmp/crepus-reactive-text-work/`.

## Queued reactive teardown

Cancel queued observers by removing FIFO membership, then skip canceled numeric IDs
when draining. Retaining the whole queue on each observer disposal makes bulk
teardown quadratic even with constant-time enqueue deduplication. Keep membership
removal before callbacks and drain past tombstones so surviving work retains order.
A release fixture including disposal plus final flush measured 4000 effects at
8.51 ms before / 1.20 ms after and 8000 at 28.50 ms / 3.02 ms (three warmups,
seven samples). Unflushed batches retain canceled IDs, not observer payloads.
Evidence: `/tmp/crepus-queued-disposal/`; these are local scheduler timings, not
browser/UI frame measurements.

## Flutter structural comparison pairs

Do not eagerly push every collection value pair before scalar comparison. A stack
of lazy child iterators bounds pending pair storage by nesting depth and permits
value-read short-circuiting. Actual rendered unequal 10000-entry list/map fixtures
read 10000 values per side before this change and one per side after it. Maps still
validate every key before visiting values; this is not constant-time map equality.
Retain identity-pair cycle detection and iterative depth handling. Map values now
visit insertion order; arbitrary side-effecting custom maps are not covered by
JSON structural-equivalence checks. Evidence: `/tmp/crepus-flutter-lazy-comparison/`.

## Reactive attribute idempotence

Compare safe attributes against live DOM values before writing. Repeating an
identical srcdoc/src write can navigate an iframe and lose its document state;
attribute setters are not behaviorally free. Chromium WASM tests measured 1000
mutation records for 1000 unchanged title, xlink:href and srcdoc effect reruns
before the guard. Afterward Chromium and WebKit both observe zero. Check xlink by
namespace/local name, retain unsafe removal, and repair externally edited values
rather than caching authored values. Changed values still produce one mutation.
Evidence: `/tmp/crepus-reactive-stable-attr/`; direct Chromium iframe reset probe
is in `/tmp/crepus-reactive-attr-repeat-audit/`. Not a frame-latency benchmark.

## Reactive class presence

DOMTokenList add/remove calls can emit class-attribute mutation records even when
presence is unchanged. Check live contains() before either mutation, retaining
external-edit recovery. Actual Chromium baseline: 1000 mutations for 1000 stable
true reruns and another 1000 for stable false; Chromium and WebKit both observe
zero after the guard. Unrelated classes and disposal remain covered. Evidence:
`/tmp/crepus-reactive-stable-class/`; mutation counts are not frame timings.

## Large root HTML commits

Spreading parsed top-level nodes into replaceChildren can exceed Chromium's
JavaScript argument limit (150000 nodes reproduced RangeError). Transfer children
into one DocumentFragment instead. Avoid Range.extractContents here: WebKit
measurements were 623ms at 10000 nodes and 2758ms at 20000, while direct transfer
completed a 150000-node commit in 161ms. These are single fixture observations,
not frame-latency guarantees. Also reject blanket identical-innerHTML skips:
live input value/checked state can differ from serialization, and rerendering
currently resets it. Evidence: /tmp/crepus-root-arity-audit/.

## Native result storage JSON reuse

The native Rust action template's store_result_json already owns a parsed Value.
Extract error metadata from that value instead of reparsing the raw result; keep
the raw-string parser wrapper for dispatch callers and preserve exact raw fallback
errors. A warmed release fixture measured 36,030 bytes at 155.775→86.308 µs and
1,800,030 bytes at 7235.917→6493.300 µs per full storage call (median 11×5).
This includes parsing, metadata and model commits; it is not device/frame timing.
Evidence: /tmp/crepus-result-parse-audit/.
