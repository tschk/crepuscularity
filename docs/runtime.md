# Runtime And Reactivity

**Also:** [Documentation home](README.md) · [Production readiness](production.md) · [Web implementation spec](CREPUS_WEB_IMPLEMENTATION_SPEC.md)

Crepuscularity has two runtime layers:

- `crepuscularity-runtime` parses and renders `.crepus` templates for GPUI and hot-reload desktop apps.
- `crepuscularity-reactive` provides `Signal`, `Memo`, and `Effect` for WASM hydration and client-side updates.

## State model

Use `Signal<T>` for mutable state, `Memo<T>` for cached derived values, and `Effect` for DOM or app-side side effects.

```rust
use crepuscularity_reactive::{Effect, Memo, Signal};

let count = Signal::new(0);
let doubled = {
    let count = count.clone();
    Memo::new(move || count.get() * 2)
};

let effect = {
    let doubled = doubled.clone();
    Effect::new(move || {
        let _label = format!("doubled={}", doubled.get());
    })
};

count.set(2);
effect.dispose();
doubled.dispose();
```

Reads inside a `Memo` or `Effect` create subscriptions automatically. Setting a signal compares the old and new values with `PartialEq`; unchanged values do not flush subscribers.

Signal reads clone a stable value snapshot. User `Clone`, `PartialEq` and displaced
value destructors run without borrowing the signal's storage. If comparison
performs a nested write, the outer setter commits its requested value afterward;
a destructor running after that commit can make a later write. `update` clones
its previous value without subscribing the active observer.

Memo computations and cache validation retain invalidations raised by these
callbacks. An obsolete computation cannot overwrite a fresher nested cache, and
memo reads retry until the cache is current. Callbacks that continually invalidate
their own dependencies can therefore keep recomputing; they must eventually settle.

Validation of already recorded memo dependencies uses an explicit stack, so deep
warm chains do not consume one call frame per dependency. Computation closures
still run synchronously: deep cold chains and recursive user computations can
exhaust the call stack. This does not move UI work to a worker thread.

When a computation callback unwinds and the caller catches its panic, its previous
subscriptions are retained alongside any partial new reads, allowing a later
source change to retry it. The next successful run replaces that conservative
subscription set. Disposed sources and observers are not restored. This recovery
requires panic unwinding; it cannot recover from aborts or WASM traps.

## Update lifecycle

1. `Signal::set` marks direct memo/effect subscribers as `Dirty` and downstream memo caches as `Check`.
2. Dirty memos recompute before their dependents are scheduled.
3. If a memo value is unchanged, dependent effects are skipped.
4. Effects flush immediately unless a batch is active.
5. `batch_begin` and `batch_end` coalesce multiple signal writes into one effect flush.

Nested observers are restored after each memo/effect run, so reading a memo inside an effect does not prevent the outer effect from subscribing to later signal reads.

Pending memo and effect jobs use FIFO queues with hash-set membership, so repeated
invalidations coalesce without scanning the pending queue. Membership is cleared
before execution, allowing a running observer to enqueue itself again.

## Disposal

Dropping the last `Signal` clone removes its graph node and subscriptions. Signals
captured by a memo or effect remain alive until that observer releases them.

Keep returned handles for long-lived `Effect` and `Memo` values. Call `dispose` when a component unmounts, a hydration island is replaced, or a test needs to tear down the graph.

If an effect's initial run unwinds, its unreturned node, subscriptions, queued
work and retained callback are removed before the panic continues. Successfully
created effects still require explicit disposal; dropping their handles does not
dispose them. Panic-abort configurations cannot run unwind cleanup.

On WASM, `crepuscularity-web` with the `reactive` feature re-exports signals,
memos, effects, DOM bindings and `hydrate_root` through
`crepuscularity_web::reactive`. Reactive handles stay on their creating thread.

DOM helpers return effect handles:

```rust
let text_effect = crepuscularity_reactive::bind_text("count", move || count.get().to_string());
text_effect.dispose();
```

`bind_text(id, closure)` keeps a sole text node when updating nonempty text and
skips DOM mutations when that text is unchanged. Other child structures are
flattened to plain text; empty values remove all children. Mutation observers
therefore see character-data changes for reused nodes rather than child removal
and insertion. Dispose the returned effect when its binding is no longer needed.

`bind_value(id, closure)` binds the live `value` property of inputs, textareas,
and selects, including controls already edited by the user. It avoids assigning
an unchanged value and returns an `Effect` handle for disposal. Use `bind_attr`
for attributes. File inputs can be cleared with an empty string, but nonempty
file values cannot be assigned. Selects with no selection still apply the bound
value, allowing an empty-valued option to be selected after an unmatched value.

`bind_checked(id, closure)` binds a checkbox or radio's live `checked` property
from a boolean closure, including edited controls. Dispose its returned effect
when the control unmounts. These property bindings do not dispatch input/change
events; wire user events to your signals separately.

`bind_bool_attr(id, attribute, closure)` adds or removes HTML boolean attributes
such as `disabled`, `required`, and `hidden` using a boolean closure. It avoids
rewriting unchanged presence and returns a disposable effect. ARIA attributes
use string values through `bind_attr`; live checked state uses `bind_checked`.
`bind_attr` handles SVG `xlink:href` in the XLink namespace, including removal
when a reactive URL is rejected.

## Hydration payloads

`crepuscularity-web` emits hydration context as base64 JSON inside a non-executable `application/json` script:

```html
<script id="__crepus_hydration__" type="application/json" data-crepus-encoding="base64">...</script>
```

The canonical SSR payload is now `__crepus_hydration__`, which wraps `{ "v": 1, "ctx": ..., "bind": ... }` and uses `data-crepus-encoding="base64"`. `hydrate_root` reads that payload first and keeps `__crepus_ctx__` plus raw JSON compatibility for older output. Only a version-1 envelope with a `ctx` field is unwrapped; legacy application objects retain their own `ctx` keys and sibling values.

## Web and Axum rendering

The in-memory template cache retains up to 64 recently used ASTs per thread.
Entries compare both source and optional path; source and path keys exceeding
256 KiB combined are parsed without caching. Eviction leaves AST snapshots held
by callers valid. File parsing shares this cache and reads current source on every
call, so equal-length edits with restored timestamps are observed. Logical file
paths retain their frontend selection. These limits bound cache entries and key
sizes, not exact AST heap usage.

Plain HTML and SSR ordinary includes share this content/path cache. Include
sources are still read for every occurrence, so filesystem I/O remains part of
rendering.

Ordinary and named includes borrow authored slot nodes and their caller context
for the duration of synchronous rendering, rather than cloning the caller for
each include. Forwarded slots retain caller scope across nested includes and
textarea content. The public owned `TemplateContext.slot` API is unchanged.

Plain HTML and SSR loops reuse one scratch context, restoring overwritten values
between rows instead of copying the whole parent variable map each time. Row
fields, aliases and nested declarations retain their existing scope behavior.
Loop bodies containing declarations can still clone their scope; this is not a
bound on the cost of arbitrary templates.

Axum request handlers and streaming helpers offload template parsing,
default-context cloning, document-head construction and body rendering to Tokio
blocking tasks. Constructors that preparse templates remain synchronous. Streaming sends the
head before rendering the body and skips body work if that first send fails.
Started blocking work cannot be forcibly cancelled, and expensive templates can
still occupy the blocking pool.

## Template file watching

The shared GPUI/TUI watcher subscribes to both the logical entry directory and the
resolved entry target directory. Includes and `context.toml` stay relative to the
logical filename; parser frontend selection is unchanged.

A background monitor checks root paths every 200 ms and reconciles subscriptions
when entry or directory symlinks are retargeted. It also re-arms replaced watch
directories. Native file events still deliver ordinary edits; this does not scan
or hash every file. Each watcher owns one extra monitor thread. Construction and
explicit watch/unwatch calls remain synchronous. Dropping its handle signals
cleanup on that worker without joining from the UI thread. Filesystem probes
cannot be forcibly cancelled. Same-path replacement detection uses device/inode identity on
Unix and filesystem creation time elsewhere; unavailable or preserved creation
times can hide replacements on non-Unix platforms.

## GPUI and Metal

GPUI hot reload sets up its watcher on the background executor, then reads the
entry source there for each reload and uses the content/path cache above.
Construction returns before watcher setup; ordinary reloads reuse the watcher,
while changing the entry path replaces it. Failed setup is reported and retried
on a later explicit reload. Equal-length edits and atomic replacements
are reflected even when modification times are preserved. Unchanged source can
reuse an AST on the same worker; rendering and view commits remain on the UI thread.
`HotReloadView` also prepares ordinary and named include sources, path resolutions
and parse errors on that worker. It publishes one shared include snapshot with
the root and context; render-time include access uses that snapshot without
filesystem resolution, reads or parsing. Include errors remain visible only when
their branch is rendered. Props, defaults, slots and variable expressions still
evaluate separately for each occurrence on the UI thread.

Ordinary and named GPUI includes borrow supplied slot nodes and caller scopes during
synchronous element construction, avoiding a full parent-context copy per include.
Forwarded slots retain caller variables, directories and virtual bundles. Explicitly
owned `TemplateContext.slot` values remain supported; local declarations can still
copy their scope. Element construction and view commits remain on the UI thread.

Hot-reload rendering temporarily inserts missing GPUI target defaults into its owned
context and removes them afterward, preserving caller overrides and avoiding a
full variable clone. Direct immutable rendering and the pending structural-refresh
fallback can still clone context; captured external slots keep their own contexts.

Caller-managed variable edits use the prepared graph immediately. Changes to the
public root AST, base directory, virtual bundle or captured slot structure queue
preparation on the next poll without rereading the entry or calling its context
loader. Until preparation completes, the previous root/include environment stays
visible with current caller variables. In-place slot AST edits are compared by
value; captured slot contexts keep their own scope. File changes refresh through
the existing watched entry subtree or an explicit `reload()`; files outside that
watch coverage still require explicit reloads. Synchronous filesystem work already
running on the worker cannot be interrupted.

Direct public `render_nodes` and `render_node` calls retain their synchronous
contract: includes share parsed sources within one call and the next call reads
fresh source. Use `HotReloadView` for background include preparation.

`crepus preview` also reloads sibling `context.toml` data on that worker. Each load
replaces the file-owned context, so removed keys and deleted files clear old
values. Root, context and prepared include snapshots publish together, discarding
loads invalidated by newer edits or structural context changes. The existing
`HotReloadState::new` constructor preserves caller-managed context;
`new_with_context_loader` explicitly opts into full context replacement on reload.

GPUI-backed crates need the Xcode SDK and Metal compiler in the cargo environment. Use the helper when cargo cannot route `xcrun` to the installed Metal toolchain:

```bash
eval "$(scripts/metal-env.sh)"
cargo clippy -p crepuscularity-runtime --all-targets -- -D warnings
scripts/metal-env.sh -- cargo check -p crepuscularity-gpui
scripts/metal-env.sh --check
```

On macOS with a Metal device, run the headless paint and interaction fixtures
explicitly. Default test runs skip these device-dependent checks:

```bash
cargo test -p crepuscularity-runtime --lib -- --ignored --test-threads=1
```

They check rendered pixels after style, focus, scroll and context changes;
they do not measure an interactive frame-time budget.

For a persistent shell setup:

```bash
export SDKROOT="$(xcrun --show-sdk-path)"
export DEVELOPER_DIR="$(xcode-select -p)"
export TOOLCHAINS="Metal"
```

## SSR region markers

The `crepuscularity-web` SSR marker mode preserves ordinary element and
`display:contents` span markers. Under table, thead, tbody, tfoot, tr, colgroup,
select, optgroup and option parents, dynamic text/raw/if/for/match/include regions
use paired comments instead. Their binding descriptor adds `"marker": "range"`;
manifest version 1, binding IDs and kinds are unchanged.

```html
<!--crepus:if:c2:start--><tr><td>Cell</td></tr><!--crepus:if:c2:end-->
```

Pair comments by kind and ID after HTML parsing. A DOM Range starts immediately
after the start comment and ends immediately before the end comment. The browser
can insert implicit tbody or colgroup elements between these endpoints, so a
consumer must support different parent nodes and retain static neighbors when
replacing content. Contextual insertion may create additional table groups;
do not assume the parsed tree is byte-for-byte the source markup. Includes,
conditions and slots inherit their actual enclosing element's insertion context.
Ordinary elements inside those regions, such as td content, retain normal spans.

This is emitted metadata. Current reactive hydration restores context and invokes
host setup; it does not automatically reconcile structural regions. DOM binding
helpers still target ordinary element IDs. The bundled app's SSR-presence check
continues to use the enclosing dynamic element marker.
