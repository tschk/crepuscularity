# Web runtime threads

New `crepus web new` sites run their WASM runtime in a module worker. The generated
`crepus_worker_runtime` export opts the runtime in. Bundle serialization, Rust
rendering, event handlers and runtime state stay in that worker; DOM updates,
event cancellation, focus/selection and island cleanup stay on the browser thread.
This reduces rendering stalls but cannot guarantee a lag-free page: large DOM
updates and application code can still block the browser.

Existing runtimes without the marker retain their main-thread behavior. To opt an
existing pure runtime in, add this export and rebuild:

```rust
#[wasm_bindgen]
pub fn crepus_worker_runtime() {}
```

Worker runtimes must not depend on `window`, `document`, or other DOM-only imports.
JavaScript module islands currently receive synchronous runtime exports and require
main-thread mode. Select it in the site's HTML head, including for a marked runtime:

```html
<meta name="crepus-runtime-thread" content="main">
```

`content="worker"` explicitly opts an unmarked runtime in; the same restrictions
apply. Rust islands can render asynchronously in worker mode. Worker availability
and CSP must permit same-origin module workers and WASM, and deployments must keep
`app.js`, `runtime-client.js`, `runtime-worker.js` and `pkg/` together.

WASM site builds emit `crepus-islands.json` even when no islands are used, so
startup can load an empty manifest without a missing-resource response. Island
bundle and manifest destinations must stay within the selected output directory,
including when existing filesystem links are present.

Dev asset responses use the decoded requested filename to select their MIME type,
so a contained `module.js` symlink can point to a differently named file and still
load as JavaScript. Canonical paths still determine containment and file reads.

The dev server keeps supported frontend sources in its virtual file map, including
new, renamed and removed files. `.crepus` edits use bundle hot updates; other
frontend sources and authored island modules request a full page reload.
Directory events reconcile descendant template keys, including populated folders
moved into the site and folders renamed or removed, and request a full reload.
Island discovery parses `.crepus` files, the selected entry and their literal
template includes using each source filename's frontend. Authored island modules
remain opaque to discovery; their host compiler owns their syntax. Discovery scans
inactive branches too, skips absent lazy includes, and limits include traversal
to depth 64. More than 4096 accepted discovery states produces an explicit error.

The worker executes event handlers and their selected renders in FIFO transactions,
awaiting asynchronous handlers. Rejected handlers skip their render, report an
error and leave the queue usable. A render error does not roll back a completed
handler. Stale HTML never replaces a newer bundle or detached scope; a scope
replaced by an earlier root commit refreshes the root without replaying its handler.
Transport failure rejects pending work. The app does not replay mutations or switch
to a new main-thread runtime after failure, since that could duplicate side effects
or discard worker state. Reload after fixing a failed runtime.

Run the manual browser transport regression with
`python3 crates/crepuscularity-cli/tests/web_worker_browser.py` (requires installed
Python Playwright and Chromium; add `--browser webkit` for WebKit). It loads the
production worker/client assets with a JavaScript fixture runtime and checks response clone failures, queue recovery
without handler replay, cached initialization failure, and disposal settlement of
in-flight, queued and future requests when the worker is blocked. It does not test
WASM compilation or execution.

Run `python3 crates/crepuscularity-cli/tests/web_root_browser.py` (or add
`--browser webkit`) to check large root replacement, live form reset, focus and
selection, mixed nodes, inert parsed scripts, and empty output. It exercises the
production reconciliation functions without compiling or running WASM.

Run `python3 crates/crepuscularity-cli/tests/web_worker_app_browser.py` (or add
`--browser webkit`) for production app/client/worker integration. A labelled
JavaScript runtime fixture rejects initialization, rendering or handlers on the
main thread and checks ordered event mutations after empty-root and SSR startup.
This does not compile or execute Rust/WASM.
