# Target audit — 2026-10-17

Six parallel reviewers inspected target families; the primary reviewed reported source paths, reproduction logs, coverage, and priorities. This is a bounded review of the current, heavily modified local checkout, not an exhaustive audit of every file or platform. Previously fixed findings were excluded. The initial review made no implementation fixes, commits, pushes, or deployments; verified local follow-up fixes are recorded below.

## Findings

P1 means a substantial responsiveness risk in an ordinary supported flow. P2 means a concrete correctness defect or avoidable cost whose impact depends on the application.

| Priority | Module | Finding and trigger | Evidence |
| --- | --- | --- | --- |
| P2, fixed locally | HTML child declarations | Ordinary element children now use the existing ordered lexical declaration walker instead of skipping lets. | Three actual Cargo regressions fail before; combined web/LVGL 219 tests, plain web 97 tests and fresh Chromium scope/range checks pass. |
| P2, fixed locally | LVGL hidden values | Typed false/null emit false, while empty/string-false presence emits true; existing attribute precedence remains. | Three actual Cargo regressions fail before; six-frontend typed matrix and full package suite pass. |
| P2, fixed locally | SSR restricted-parent regions | Tables/selects emit DOM-safe paired comment ranges instead of parser-moved/discarded span wrappers. | All five new regressions fail before; 167 all-feature and 94 plain web tests, strict Clippy and 16 Chromium cases pass. |
| P2, fixed locally | Java HTML output | HTML uses the canonical Rust renderer instead of extracting the first IR text field. | Original Java acceptance test fails; strict javac, actual Java/CLI tests, diagnostic/cleanup probes and Chromium DOM/click-metadata checks pass. |
| P2, fixed locally | HTML CLI JSON context | Reuses native IR context validation; unsupported shapes and malformed .json files fail before output. | Actual CLI regression fails before; 155 scoped Rust tests pass, including native context compatibility and supported TOML. |
| P2, fixed locally | TUI invisible content | Invisible subtrees preserve layout but do not paint or participate in focus discovery. | Three actual Cargo regressions failed before; all 275 TUI tests and inspected TestBackend geometry pass. |
| P2, fixed locally | Public GPUI transitions | Width/height/opacity restart when duration or endpoints change instead of reusing completed state. | Actual Metal regression failed before; all 78 wrapper tests including formerly ignored Metal tests pass. |
| P2, fixed locally | TUI hidden content | Hidden classes and HTML hidden attributes omit subtrees before painting, layout gaps and focus discovery. | All five final regressions fail on the original renderer; 272 package tests, strict Clippy and inspected public TestBackend output pass. |
| P1, fixed locally | GPUI animation reload | Edited animation sequences get fresh private state, preventing a retained stage index from crashing a shorter sequence. | Actual runtime/Metal shrink regression failed before; all 73 runtime tests including seven Metal tests and strict Clippy pass. |
| P2, fixed locally | Flutter dependency helper | A comment or unrelated value mentioning the package no longer makes `flutter add` silently skip installation. | Actual filesystem regression failed before; 123 slim CLI unit tests pass, including byte-preserving idempotency and unsupported-form rejection. |
| P2, fixed locally | View IR checked defaults | defaultChecked/default-checked now populate checked snapshots, including bare indentation syntax. | Three cohorts failed before; 353 selected native tests, 442 core tests, strict Clippy and Solid Chromium default/edit/reset/SSR probes pass. |
| P2, contained locally | Embedded disabled buttons | Unsupported true/present disabled state returns an explicit render error instead of an enabled handler/hit target. | Three regressions failed before; all 125 default embedded tests and strict default-feature Clippy pass; false/null preserve exact pixels and hit behavior. |
| P2, fixed locally | View IR input defaults | defaultValue/default-value now populate the existing initial-value field instead of disappearing. | Three original cohorts failed before; 356 selected native tests, strict Clippy and equivalent-output Vue browser probes pass. |
| P1, fixed locally | Firefox extension background | Firefox background scripts now declare module type, matching the shipped static-import asset. | Manifest regression failed before; all 46 extension Rust tests and 15 Bun tests pass; actual Firefox loading pending. |
| P2, fixed locally | Extension capability watcher | An invalid existing manifest produces a load error during source-triggered capability rescans, instead of false missing-capability advice. | Actual event-handler regression failed before; all 46 extension Rust tests pass. |
| P2, contained locally | View IR picker defaults | Parent value/defaultValue/default-value state fails explicitly with bind guidance instead of disappearing. | Both regression cohorts failed before; 349 selected native tests pass. |
| P2, fixed locally | Vue SSR textarea defaults | Leading LF in an unbound textarea default survives server HTML parsing while preserving the client default. | Failing-before regression; 353 selected native tests, strict Clippy/TypeScript and Chromium DOM/SSR/edit/reset/hydration probes pass. |
| P2, fixed locally | Web hydration feature boundary | Two private hydration-only wrappers now compile only with that feature, removing warnings that blocked strict non-hydration builds. | Callers verified; strict Clippy passes with and without hydration; 29 selected web tests pass. |
| P2, contained locally | GPUI runtime islands | Active `embed` nodes and invalid `slot-rotate` children now draw inline red diagnostics. | Failing-before Metal regression, inspected asset-naming diagnostic and active/inactive target guards; full 71-test runtime suite including GPU fixtures passes. |
| P1, fixed locally | GPUI macOS text | Platform font support was missing, selecting `NoopTextSystem` even though GPUI font support was enabled. Runtime/workspace dependencies now enable the platform feature, and the GPUI wrapper forwards its font feature. | Dependency source and compiled feature fingerprints identify the no-op path; actual Metal text pixels appear after the feature change. |
| P2, fixed locally | Solid SSR textarea defaults | Unbound defaults retain the DOM property and supply escaped server-only body content; bound values now serialize as body text too. | Failing-before regressions; 357 native tests, strict Clippy and exact-source Chromium DOM/SSR editing/reset/value probes pass. |
| P1, fixed locally | Native Android/iOS hosts | Document-picker metadata and copies now run on serial workers, with main-thread delivery and stale-result cleanup. | Executable gated-copy probes; Swift build/iOS typecheck; 29 scaffold tests; device/provider validation pending |
| P2, fixed locally | LVGL form bindings | Canonical paths resolve context state and emit typed text/checked/selected/value snapshots instead of literal bind attributes. | Three original regressions failed before; full 42-test LVGL suite and strict Clippy pass |
| P2, fixed locally | Android result ordering | Accepted main-thread deliveries remain serialized through worker interruption; interrupt status is restored afterward. | Baseline ordering failure; four gated JVM cases plus primary native queue probes and Rust gates pass |
| P2, fixed locally | LVGL | Text input `value` and canonical form `bind` properties map to the XML `text` API, preserving numeric widget values. | Three regressions failed before; all 38 LVGL tests and strict Clippy pass |
| P2, fixed locally | Android action queue | Recoverable dispatcher exceptions become captured main-thread error results before subsequent jobs run. | Actual-source JVM missing-callback regression before; full native queue probes, 339 native units and 29 scaffold tests pass |
| P2, fixed locally | Runtime | Frontend bridge sends and receives used opposite queues. Receive methods now consume the queue for their named destination. | Both new regressions failed before; full runtime suite passes after |
| P2, contained locally | TUI islands | Active unsupported islands draw inline red diagnostics instead of disappearing. | Two regressions failed before; all 267 TUI tests and an inspected 80-column context-toggle preview pass |
| P2, fixed locally | LVGL ranges | Slider/bar min/max aliases emit target range names and validate integer bounds; unsupported steps fail explicitly. | Four regressions failed before; all 46 LVGL tests and strict Clippy pass |
| P2, fixed locally | TUI | Ordinary, named and nested includes now inherit eight target/platform variables. Guarded rendering and focus discovery agree with the root. | Three regression tests failed before; full TUI suite passes after |
| P2, fixed locally | Embedded | Active unsupported `embed` islands now fail explicitly before framebuffer painting, including button children. | Failing-before regression; full package and focused follow-up tests pass |
| P2, fixed locally | Embedded buttons | Primary labels use evaluated children, retaining control-flow/include/slot scopes instead of rereading the source AST. | Two regressions failed before; full 121-test package suite, pixel/hit-bound checks and rendered preview pass |
| P2, contained locally | GPUI macros | Authored include slots now fail explicitly instead of retaining fallback text and appending caller content after the callee. | Actual wrong-output probe; real Cargo regression failed before; macro tests and strict Clippy pass |
| P2, fixed locally | Template macros | Entry/transitive reads emit compiler dependencies for `view_file!`, `crepus_refs!` and `template_refs!`. | Actual Cargo consumer rebuilds after template-only edits; symlink retargeting remains a Cargo limitation |
| P2, contained locally | GPUI macros | Unsupported explicit include props and selected named-component defaults now fail compilation instead of silently using a conflicting caller binding. | Both regressions failed before; actual Cargo consumer exercises direct/nested/named errors and positive accessor compatibility |
| P2, fixed locally | Flutter | ForEach applies the shared reserved-null root policy before direct collection lookup; host keys cannot make null expressions iterate. | Widget regression failed before; full 535-test Flutter suite passes after |
| P2, fixed locally | Native Android host | Activity identity is weak; identity-guarded detach clears the picker launcher/pending action and invalidates old delivery. | Actual-helper lifecycle/old-generation tests; no device heap measurement |
| P2, fixed locally | Tauri macros | Generated bridge inputs and argument locals use isolated identifiers; commands and parameters named `payload` or `app` compile and dispatch correctly. | Original compilation failed; package and runtime regressions pass |
| P2, fixed locally | VS Code | Automatic discovery checks native executables on PATH before an existing workspace fallback; asynchronous lifecycle work is guarded and shutdown failures remain recoverable. | 18 resolver/lifecycle tests and TypeScript compilation pass; editor host launch pending |
| P2, fixed locally | Authored Vue compiler | Component listeners unwrap imported ref-valued callbacks inside live prop getters, including fused arrays and once listeners. | Before-fix DOM/SSR TypeError and strict TS failure; 110 Vue tests, strict Clippy, fresh normal/minified runtime probes pass |
| P2, fixed locally | Authored Svelte compiler | Projected each callbacks/tags use a typed identity helper around the full callee, preserving casts and unbound receiver semantics without TS2695. | Strict DOM/SSR, normal/minified runtime probes; 1,280 native tests and strict Clippy pass |
| P2, fixed locally | Lite | Foreground redraws acquire a shared immutable tree instead of cloning the full host snapshot and sorting storage keys. | 100-acquisition probe: 810,900 allocations /119,714,200 bytes to zero; default and V8-enabled tests pass |

## Fix locations and acceptance checks

1. **Picker I/O:** Android `crates/crepuscularity-cli/templates/native/android/app/src/main/java/dev/crepuscularity/nativeshell/CrepusRustActions.kt:160` calls the copy path at lines260–293. iOS `crates/crepuscularity-cli/templates/native/ios/Sources/NativeShell/CrepusRustActions.swift:428` calls the copy path at lines489–513. Move provider reads/copies to a bounded worker; return publication/bookkeeping to main. Preserve security-scoped access, cancellation/generation checks and partial-file cleanup. Verify a gated provider keeps the main heartbeat running and publishes exactly one result.
2. **Bridge queues:** `crates/crepuscularity-runtime/src/frontend_bridge.rs:33`. Pair destination-labelled send/receive methods consistently. Check both directions, FIFO, clones and absence of cross-delivery.
3. **TUI include environment:** `crates/crepuscularity-tui/src/render.rs:1355`. Carry target/platform keys into the child context without leaking ordinary parent vars. Check ordinary/named/nested includes, focus discovery, defaults/overrides and caller-owned slots.
4. **Embedded islands:** `crates/crepuscularity-embedded/src/render.rs:227`. Reject an active unsupported island explicitly, as LVGL does. Check direct/include/slot cases, guarded fallback success and unchanged framebuffer on failure.
5. **Macro dependencies:** `crates/crepuscularity_macros/src/lib.rs:116,210`. Register every read template using generated `include_str!` sentinels or an appropriate tracked dependency mechanism. Check entry-only and transitive-only edits rebuild a consumer; cover ID-collection macros that read the same files.
6. **Macro include props:** `crates/crepuscularity_macros/src/lib.rs:200`. Implement scoped prop bindings, or explicitly reject unsupported props instead of discarding them. A regression must contain a conflicting caller variable; retain caller scope for slots.
7. **Flutter null loops:** `plugins/crepuscularity-flutter/lib/src/renderer.dart:833`. Apply the shared reserved-root policy before the direct-key fast path. Check all four null aliases, dotted aliases, and valid nested `object.None` members.
8. **Android detach:** `CrepusRustActions.kt:117,156,168` and adjacent `MainActivity.kt:38`. Remove the unused strong Activity field and detach old launchers/pending state with an identity/generation guard. Check finish, rotation/reinstall and late picker delivery; use a device/emulator retention check.
9. **Tauri hygiene:** `crates/crepuscularity-tauri-macros/src/lib.rs:32–83`. Use hygienic generated identifiers for inputs/locals; preserve authored names as JSON keys. Compile commands with `payload` followed by another argument and `app` followed by injected state/handles.
10. **Editor resolution:** `editors/vscode/src/extension.ts:12`. Follow configured override, PATH discovery, then an existing workspace fallback. Check PATH-only, local-only, explicit override and missing-server cases, including Windows executable handling.
11. **Svelte callback lowering:** `crates/crepuscularity-native/src/svelte_each.rs:538–591`. Preserve authored casts and unbound callback receivers through the existing typed identity-helper pattern. Strictly compile `{#each rows as {callback}}<p>{(callback as ()=>number)()}</p>{/each}`; retain member-method receiver semantics.
12. **Lite redraw snapshots:** `crates/crepuscularity-lite/src/main.rs:722` and `src/host.rs:724`. Reuse an immutable render-only tree snapshot without changing the explicit full guest snapshot API. Measure unchanged large-tree redraws and verify concurrent updates publish the latest tree.

## Coverage

| Reviewer family | Modules/flows inspected | Limits |
| --- | --- | --- |
| Desktop | `gpui`, `runtime`, `lite`, `tauri`; host/macro integration | No new rendered desktop/frame-time gate |
| Web | `web`, `reactive`, `webext`, WASM bridge, bundled web runtime; authored Svelte callback lowering and adjacent helpers; bounded other emitter inspection | React/Svelte/Vue/Solid/Imba/Moonshine cohorts not exhaustively exercised |
| Native | `native`, `abi`, Swift/Kotlin templates, Flutter parser/decoder/render/timer paths | No picker-provider, emulator/device, heap-profiler or concurrency stress gate |
| Terminal/embedded | `tui`, `embedded`, `lvgl`, `colors` | Buffer/framebuffer probes; no physical display or new latency measurements |
| Core | Parser/eval/context/preprocess/watch/cache flows, `wasm`, `crepuscularity_macros` | No full parser matrix or fresh WASM build |
| Tooling/plugins | `cli`, `lsp`, `components`, `plugin-bindgen`, `tauri-macros`, selected language plugins, VS Code/Zed | Selective plugin/compiler coverage; no full polyglot matrix or editor host launch |
| Primary integration | `crepuscularity` facade manifest/build/context/output paths; finding source/log cross-checks | No additional facade defect established; existing package tests not rerun |

All 22 framework crate directories received bounded flow coverage across these families. This does not mean every module, function, scaffold or generated adapter was independently tested. No new finding was established for a target is not a claim of complete parity or freedom from bugs.

## Evidence and verification limits

Detailed reviewer reports and reproducible probes are saved locally under `/tmp/crepus-target-review-20261017/`: `desktop.md`, `web.md`, `native.md`, `terminal-embedded.md`, `core.md`, and `tooling.md`. These temporary artifacts may be cleaned later; the triggers and acceptance requirements above remain in this report.

During the initial review, the bridge probe compiled the exact current source. Other Rust probes use existing compiled local libraries/proc macros; their corresponding current source paths were cross-checked, but a fresh Cargo build was not run. Flutter ran against current plugin source. Web emission used the existing local CLI and strict TypeScript tooling. These are scoped reproductions, not full-suite or cross-platform certification. The initial review did not run a Cargo test suite, browser cohort or hardware performance gate. Follow-up verification is recorded below; no external action was performed.

Configured OMP useful-review attempts had already failed; this pass used six available native review agents. No provider retries, credential changes, installations, or purchases were needed.

### Follow-up: frontend bridge direction

The receive-queue correction preserves sends, locking, shared clones and FIFO ordering. Two regression tests cover exclusive destination delivery and interleaved bidirectional FIFO delivery through clones. Both failed against the original implementation. After correction, `cargo test --locked -p crepuscularity-runtime` passed 65 tests; five existing Metal-dependent tests remain ignored. External consumers that compensated for the inverted behavior must remove that workaround. No in-repository callers were found. All-target/all-feature package Clippy with `-D warnings`, formatting and diff checks also passed.

### Follow-up: TUI include environment

The shared include-context helper now carries only the eight target/platform keys before explicit props. Named defaults yield to inherited flags; explicit props still override them. Ordinary parent variables remain private, and slots retain their captured caller scope. Four integration tests cover default/custom environments across ordinary/named/nested includes, guarded focus discovery, explicit props, named defaults and slot isolation. Three failed before the fix; the fourth protects existing scope behavior. `cargo test --locked -p crepuscularity-tui` passed the full package suite and five documentation tests. All-target/all-feature strict package Clippy, formatting and diff checks also passed. Rendering was exercised using Ratatui terminal buffers, not a physical terminal session.

### Follow-up: embedded unsupported islands

Active `embed` nodes now produce an error naming the source and embedded target. Button children pass through the shared renderer for validation before primary-label extraction, closing its prior bypass after an early label. Existing label selection remains unchanged. This additionally evaluates active button descendants that were previously ignored, with extra temporary lowering work for those children; no hardware timing claim is made. Four integration tests cover root/include/named/slot/button rejection, exact framebuffer preservation, successful template recovery, web-guarded fallback pixels, inactive button branches and unused slot fallbacks. Two regressions failed before the fix. The full default-feature embedded suite passed, then the expanded four-test target passed; default all-target strict Clippy, formatting and diff checks passed. No physical panel or MCU run was performed.

### Follow-up: Tauri command identifiers

Generated bridge inputs and positional argument locals now use mixed-site identifiers. Authored names remain JSON keys, with raw parameter identifiers normalized (`r#type` reads `type`). Regressions exercise commands sharing parameter names, `app`/`payload` commands, names resembling internal locals, state/handle/window injection, and async borrowed/owned string inputs. The original regression produced six compiler errors; both package suites now pass (16 runtime/integration tests, including the external dependency-alias consumer). All-target/all-feature strict Clippy for both packages, formatting and diff checks also passed. No command-dispatch threading change was made. Raw command registration names remain a separate unchanged limitation; clients supplying raw-prefixed JSON parameter keys must use the ordinary key.

### Follow-up: Flutter null loop roots

ForEach and ordinary lookup now share the null-root predicate. All four aliases (`null`, `none`, `None`, `Null`) and their dotted roots are rejected before direct host-key collection lookup. Binding trimming, direct dotted-key precedence, real nested null-named members, and missing/non-list behavior remain covered. The new widget regression failed before the fix; `flutter test --no-pub` passed all 535 tests including rendering/source fuzz tests. Targeted Flutter analysis found no issues, and formatting/diff checks passed. No physical device or release-mode performance gate was run.

### Follow-up: document-picker I/O and Android teardown

Both generated mobile hosts now prepare picker results away from the UI thread. iOS uses one serial import slot; the main actor owns admission/delegate identity and publication. Metadata and copy share one balanced security-scope lifetime. Canceling releases delegate retention and emits once while keeping admission busy until the current noninterruptible copy finishes; stale successful copies are removed on the worker. Android captures URI order, application context, sink and generation, uses one daemon worker with four waiting slots, and checks generation before I/O and main-thread delivery. Rejected posts and stale delivery clean copies on the worker. A throwing Android sink counts as one delivery attempt; FutureTask captures its exception and later jobs proceed.

Android's singleton now holds a weak Activity identity and clears launcher/pending references on guarded detach. Old Activity destruction cannot detach or shut down a replacement installation. The main Activity invokes detach before native shutdown.

Verification: actual extracted Swift delegate/retention/JSON/copy helpers passed with a blocked-provider heartbeat, duplicate/canceled callbacks, stale copy cleanup, real local-file byte comparisons, selection ordering and partial failures. Actual extracted Android install/detach/worker/copy helpers passed `kotlinc -Werror`/JVM checks for blocked I/O/main progress, captured hooks/context, exact bytes, permission/copy failures, stale callbacks/completions, queue bounds, throwing sinks and rejected main posting. Platform/JSON collaborators are labelled stubs in the Android probe; no full Android build or grant validation is claimed. The existing full generated/snapshot action-queue gate passed, as did the added Android picker gate separately, 29 `native_ir_cli` tests, Swift package build with warnings as errors, full iOS simulator source typechecking with warnings as errors, Python syntax and diff checks. The picker probes are included in the native action-queue script for future runs.

Remaining acceptance: real iOS/Android document-provider grants (including cloud documents), on-device frame/heartbeat measurements, and Android heap retention checks. Cancellation/teardown suppresses stale publication but cannot interrupt an active provider copy. Other host action queues and main-thread result application remain outside this fix.

### Follow-up: Lite render snapshots

The stored host tree is now an Arc-owned immutable wrapper with iterative destruction. `HostState::render_snapshot` and `Bridge::host_render_tree` acquire just that tree; the foreground surface renderer consumes its root, preserving the same node data and empty-state behavior. Repeated reads retain the same tree identity until replacement; old readers remain stable afterward. Normalization still occurs before publication. Replaced ownership is released after unlocking. The public owned/serialized `HostSnapshot` contract remains unchanged.

An optimized exact-source host probe compared 100 acquisitions for a 1,000-node tree with 100 storage keys. The prior foreground path (`snapshot`) requested 810,900 allocations totaling 119,714,200 bytes; shared render acquisition requested zero allocations/bytes. This is cumulative allocation for snapshot acquisition, not peak memory, frame time or all redraw work. The probe uses production host code with a labelled unused BridgeError collaborator and an existing Core library; its source and command are retained in `/tmp/crepus-lite-render-snapshot/`.

Tests cover unchanged Arc identity, normalized styles, owned full-snapshot isolation, worker replacement, retained-old-version stability and final-owner destruction of a 20,000-node tree on a 64 KiB stack. Default library tests passed 97/97; all-feature library and foreground binary tests passed 103/103 and 35/35, including real V8 tests. All-target/all-feature strict Clippy, affected-file formatting and diff checks also passed. Foreground element construction, host-mutex contention behind other full-snapshot operations, and final shared-tree destruction on whichever thread owns the last Arc remain possible costs. No rendered frame-time improvement is claimed.

### Follow-up: authored Svelte each callbacks

Projected callback calls and tags now use the existing typed identity-helper strategy around the complete transparent callee expression. Identifier projection remains an independent edit, preserving authored casts, satisfies/non-null expressions, optional calls, generic instantiation, nested projections and decoded helper-name hygiene. Ordinary member calls keep their receiver. Server snapshot lowering remains ordinary local values and does not need the DOM helper. No source syntax restrictions were relaxed.

The original public generated DOM graph failed strict TypeScript with TS2695 before rebuilding. Fresh normal CLI emission produced DOM/SSR graphs accepted by TypeScript7.0.2 strict checking. Installed official Svelte5.46.4 accepted the richer authored client/server fixture. Actual normal and Bun-minified DOM/SSR output preserved unbound direct/nested callbacks and tags, generic/optional/nested expressions, member receivers and escaped-name hygiene; DOM unmount emptied the target. An intentionally wrong satisfies constraint still failed TS1360 in both targets, proving authored type validation was retained. DOM execution used installed Happy DOM, not a real browser.

The proper web-modules focused cohort passed15 tests. All-feature native testing passed1,280 tests across149 successful test targets with no ignores; strict all-target/all-feature package Clippy, Rust formatting and diff checks passed. The initial no-feature invocation ran zero tests and was not counted. Integration fixtures, commands, graph hashes and logs live in `/tmp/crepus-each-unbound-fix/`. No full browser/frame-time or source-framework parity claim is made.

With existing disk-cleanup authorization, 250 verified inactive project-owned Cargo test executables were removed, reclaiming2,732,654,000 bytes. Cargo test fingerprints, source dep-info, Mach-O headers and inactive-open-file checks constrained deletion; the completed Native suite used its terminal success log as additional evidence. Source files, dependency libraries, real CLI binaries and installed tools were retained.

### Follow-up: template macro dependencies

The file-view and both accessor macros emit `include_str!` dependency sentinels for their entry and visited include files. Dependency collection preserves authored paths separately from canonical cycle detection without changing include resolution or generated accessors. An actual offline Cargo consumer edits only entry/direct/nested template files and an included named component; rebuilt output/accessor layouts change without editing Rust. Unix coverage also edits through an existing symlink. The 26 macro unit tests and real consumer regression pass; four existing documentation examples remain ignored. All-target/all-feature strict package Clippy and both TUI macro-consumer tests pass. Formatting and scoped diff checks pass.

Retargeting a symlink alone still requires a forced rebuild. Independent tiny consumers using only Rust built-ins reproduced this with both `include_str!` and `include_bytes!`: rustc dep-info records the alias, but Cargo canonicalizes its dependency to the previous target. No project configuration or compiler cache was changed to conceal this limit. Unsupported macro include props/defaults are addressed by the follow-up below.

### Follow-up: VS Code language-server discovery and lifecycle

Explicit configuration stays authoritative. Automatic discovery asynchronously checks native files on PATH before an existing workspace debug executable, handles Windows PATHEXT/native suffixes, and explains the setting when no server exists. It invokes no shell. Async lookup/startup is generation-guarded, so disposal and newer restarts cancel stale launches/errors. Shutdown waits for startup, serializes stops, and retains failed handles for retry before replacement. Overlapping restart/deactivation regressions exercise failed stops; persistent failure rejects async deactivation rather than claiming success.

All 18 Bun tests (45 assertions), TypeScript compilation, generated output inspection and scoped diff checks pass. Tests execute the actual extension with mocked VS Code/LanguageClient boundaries and real temporary filesystem discovery; Windows discovery is simulated. No actual VS Code host or Windows process launch was performed. Existing locked development dependencies were installed for verification; manifests/lockfiles and external systems were unchanged.

### Follow-up: macro include props and named defaults

`view_file!` expands includes textually and has no component-local prop scope. Explicit include props now fail with the include and source path, and selected named-component defaults fail with the component name. This prevents a conflicting Rust caller binding from silently supplying the value. The restriction applies only to this compile-time view route; the accessor macros still collect IDs and preserve source for runtime evaluation. Unselected component defaults do not block prop-free selection. Implementing actual scoped macro props remains future work; authored-slot expansion is now explicitly rejected by the follow-up below.

The actual Cargo regression first compiled unsupported explicit props successfully against the old implementation; the named-default unit regression also failed before its guard. Afterward, 27 unit tests and the actual Cargo consumer pass. The consumer covers direct/nested/named props, direct/nested defaults, a positive unselected-default case retaining caller scope, and positive web/TUI accessor generation with props/defaults. Four existing doc examples remain ignored. Strict all-target/all-feature package Clippy, formatting and scoped diff checks pass. No rendered GPUI process was launched.

### Follow-up: Vue component listener refs

Component event listener expressions now use the same template rewrite as intrinsic events before callback fusion. Imported ref-valued callbacks unwrap inside the existing live prop getters, so replacements are observed on later emits. Ordinary imported and local functions, callback source order, DOM once keys and SSR listener fusion retain their contracts. Frontend documentation now records the supported component-event subset instead of incorrectly listing every component event as unsupported.

The new regression failed against the original lowering. Old CLI output reproduced the actual “Component event listeners must be functions” error in DOM and SSR, and strict TypeScript rejected ref objects in listener descriptors. All 110 Vue tests across 20 all-feature test targets pass; all-target/all-feature strict native Clippy, formatting and scoped diff checks pass. A fresh CLI build passed with two existing unrelated web dead-code warnings and an existing future-compat warning. Its emitted DOM/SSR graphs pass strict TypeScript 7.0.2 and normal/minified Bun runtime probes. Probes exercise imported ref replacement between emissions, ordinary callbacks, fused listeners, DOM once handling, repeated clicks and unmount cleanup. DOM uses Happy DOM rather than a real browser; no broader Vue parity or frame-time claim is made. Evidence: `/tmp/crepus-vue-listener-ref-fix/verification.json` and `integration/`.

### Follow-up: authored macro include slots

Each template body is parsed before textual include expansion. An iterative AST check rejects authored include slots through element, conditional, loop and match bodies, with include/source diagnostics. This prevents the reproduced `before;fallback;after;provided` output. Inline/multiline quoted text remains literal, prop-free includes keep their behavior, and runtime accessor macros still accept props/defaults/slots. Full scoped macro slot support remains future work; previously accepted authored slots now produce compilation errors.

The actual Cargo regression failed before the guard. All 28 unit tests and the real Cargo consumer pass, with four existing documentation examples ignored. Positive accessor generation and direct/nested/named slot diagnostics pass; a unit matrix covers nested control bodies. All-target/all-feature strict macro Clippy, formatting and scoped diff checks pass. The preflight AST is dropped before recursive text expansion; its added parsing happens at compile time. No full GPUI host was launched.

### Follow-up: embedded evaluated button labels

Button primary-label extraction now reuses the child tree already evaluated for rendering, eliminating the second source walk that ignored control flow, includes and local declarations. First visible text/span content wins through the renderer's transparent control/slot wrappers; explicit empty content still wins, and arbitrary layout containers remain excluded. Active unsupported islands still fail before painting.

Two regressions failed before the fix. The full default-feature embedded suite passes 121 tests across 28 test/doc targets, including exact RGB565 comparison, bounds, hit edges, includes/defaults/props/slots, declarations, visibility, empty precedence and framebuffer preservation on error. Strict default-feature all-target Clippy, formatting and scoped diff checks pass. An actual freshly built public-library RGB888 probe was rendered and visually inspected: conditional, local and included labels appear in their buttons, and hit tests agree. Evidence: `/tmp/crepus-embedded-button-control-fix/`. No physical MCU/panel or optional hardware-feature matrix was exercised.

### Follow-up: LVGL text input values

Widget-aware attribute mapping aliases `value` and the shared Svelte form `bind` property to `text` for `input` and `textarea`, matching the [official LVGL textarea XML API](https://lvgl.io/docs/pro/built_in_widgets/lv_textarea). The shared parser remains unchanged. Alias collisions use the existing last-binding-wins deduplication; extracted single-label body text retains its existing precedence. Slider, progress and meter `value` attributes remain unchanged. This addresses emitted text snapshots, not full source-framework form binding compatibility or live LVGL two-way state.

All three new regression cohorts failed against the original emitter. The full package suite passes 38 tests across 14 test/doc targets, including all six template frontends, static/expression/form values, XML escaping, text/value precedence, named include props and numeric control preservation. A previous input test was corrected to expect the actual target API. All-target/all-feature strict Clippy, formatting, independent source review and scoped diff checks pass. No LVGL Pro editor/runtime or physical widget was executed. Evidence: `/tmp/crepus-lvgl-text-value-fix/`.

### Follow-up: Android dispatcher exception results

The generated and checked-in Android action objects catch recoverable `Exception`s only around the dispatcher and reuse the existing error envelope. Captured hooks, serial worker, main-thread delivery and latch ordering are unchanged; fatal JVM errors, custom sink failures and rejected main posting retain their existing behavior. A throwing dispatcher now reports an error before the next queued action starts instead of dropping its callback and reaching the worker uncaught handler.

The updated actual-source JVM regression fails against the preserved baseline and passes against both generated/snapshot objects, checking captured hooks, main delivery, exact callback count and error-before-next-job ordering. The primary reran the full native action-queue gate, including existing Swift routing, state/FFI transport, picker and Kotlin async-edit probes. All 339 default native unit tests and 29 CLI native scaffold tests pass. All-target/all-feature strict native Clippy, formatting, independent review and scoped diff checks pass. JVM collaborators are explicitly labelled platform/transport stubs; no Android process, JNI/device or full Gradle build was run. Evidence: `/tmp/crepus-native-dispatch-exception-fix/`.

### Follow-up: canonical LVGL form snapshots

The canonical shared `bind` attribute now decodes bare/quoted context paths and evaluates expression forms without looking up returned strings again. Text controls emit text; explicit checkbox/switch tags require booleans and emit checked state; select/dropdown require nonnegative integer indices and emit selected; sliders require integer values. Integers use a signed 32-bit snapshot range, following the publicly available [LVGL 9.3 XML converter](https://raw.githubusercontent.com/lvgl/lvgl/v9.3.0/src/others/xml/lv_xml_utils.c), with current property names checked against the official XML API. Wrong types, overflow, and canonical bind on unsupported tags fail explicitly. Existing alias ordering and explicit nonbind attributes retain their behavior; this creates snapshots, not LVGL subjects or write-back.

The first three regression cohorts failed before implementation. All 42 LVGL tests across 15 test/doc targets pass, including bare/quoted/expression forms, no second string lookup, dotted paths, declarations, named props/defaults, Svelte select bindings, collision precedence, accepted integer boundaries and invalid types/ranges. Strict all-target/all-feature Clippy, formatting, independent review and scoped diff checks pass. The shared AST treats string-literal bind values as paths regardless of authored braces; this limit is documented. HTML input type variants still do not select other LVGL widget kinds—use explicit control tags. No LVGL runtime, newer private XML-engine integer implementation or physical panel was exercised. Evidence: `/tmp/crepus-lvgl-control-bind-fix/`.

### Follow-up: Android interrupted result delivery

Both generated and snapshot action paths now share a private accepted-delivery wait. It remembers InterruptedException while waiting on the original latch, finishes only after main-thread delivery/cleanup, then restores interrupt status. This prevents the reproduced second-dispatch-before-first-sink race. Rejected posts still fail before waiting; prior dispatcher error conversion, fatal errors and main-sink cleanup remain unchanged.

The updated actual-source JVM harness fails against the preserved baseline and passes both generated/snapshot variants. Four gated cases cover action/bound-change jobs with self-interruption and interruption while main delivery is held; a direct invocation of the actual helper verifies interrupt status at return rather than after the executor may clear it. The primary full native action-queue gate passes, including Swift routing/state/picker and Kotlin edit/picker probes. All 339 default native units and 29 CLI scaffold tests pass. Strict all-target/all-feature native Clippy, formatting, independent review and scoped diff checks pass. Platform/FFI collaborators remain labelled stubs; no Android device/JNI/full Gradle run was performed. Evidence: `/tmp/crepus-native-interrupt-fix/`.

### Follow-up: visible TUI island diagnostics

The active embed branch now flushes surrounding text and emits the existing inline red error widget, naming the asset and suggesting an is_web guard. This matches include-error rendering: draw still succeeds, and the frame can contain surrounding content. It is not an error-return or transactional/no-partial-paint guarantee. Inactive branches and unused slot fallbacks remain unevaluated.

Two new regressions failed before the change. All 267 TUI tests across 28 unit/integration/doc targets pass. The new 80-column buffer cases cover root/container/button/if/match/loop/include/named/caller-slot paths, red styling, repeated diagnostics, preserved neighbors, inactive fallback and context-update recovery. The primary compiled a current public-library TestBackend probe and inspected its 80-column before/after output: disabling the branch removes the diagnostic while retaining neighbor text. Strict all-target/all-feature package Clippy, formatting, independent source review and scoped diff checks pass. No physical terminal session was exercised. Evidence: `/tmp/crepus-tui-island-diagnostic-fix/`.

### Follow-up: LVGL range aliases and step capability

Slider, progress and meter source min/max properties map to min_value/max_value in the current official LVGL Pro XML contract. Aliases evaluate once, normalize exact i32 numeric snapshots, preserve reversed bounds and use existing post-mapping collision precedence. Fractional/nonfinite/overflow values and incompatible types fail explicitly; quoted precision is checked without float rounding. Unit steps use the same exact integer policy and are omitted; nonunit steps fail because no quantization implementation exists. Raw target attributes and other widgets retain their prior behavior. Older XML dialects can use different range names, so this does not certify every historical LVGL XML engine.

All four new regression cohorts failed against the original range emitter. The full LVGL suite passes 46 tests across 16 test/doc targets, including static/bound values, exact endpoints, quoted forms, alias collisions, reversed bounds, non-target attributes, unsupported steps and near-one string precision. Strict all-target/all-feature Clippy, formatting, independent review and scoped diff checks pass. No LVGL editor/runtime or physical display was executed. Evidence: `/tmp/crepus-lvgl-range-fix/`.

### Follow-up: GPUI platform fonts and visible island diagnostics

The split GPUI platform dependency did not enable its font-kit feature. Pinned macOS platform code therefore selected NoopTextSystem; enabling fonts on gpui alone did not enable the platform renderer. Runtime/workspace platform dependencies now enable font-kit, and the GPUI wrapper forwards its font feature. Cargo.lock adds only the dependency edge to the already locked zed-font-kit package.

Active web islands now use the existing inline red diagnostic convention, naming their asset and suggesting an is_web guard. Invalid slot-rotate child-validation errors also surface instead of becoming an empty label. This does not add a web-island adapter or change the render API to return errors.

The new Metal fixture first proves that ordinary text paints, then covers direct/nested/target-guarded islands, invalid rotating text, and an inactive web branch. Its island case failed before the diagnostic change; the final full runtime suite passes all 71 tests, including all six normally ignored Metal fixtures. The primary inspected the actual rendered asset-naming diagnostic. GPUI wrapper tests pass 69 cases with seven preexisting ignored cases, and strict all-target runtime/wrapper Clippy passes. The first wrapper build exhausted disk; its inactive failed-build object files were removed and a retry passed after free capacity recovered. No physical desktop window or other OS was exercised. Evidence: `/tmp/crepus-gpui-font-feature-fix/`.

### Follow-up: Solid server textarea defaults

Unbound textarea defaults retain their client defaultValue property and emit escaped server-only body content through the existing Solid web runtime. The emitter compensates the HTML parser's initial-newline rule, preserves carriage returns as character references and escapes literal ampersands/angle brackets before passing raw server text. Generated aliases are reserved against component-name collisions. A narrow JSX type assertion bridges Solid's server raw-text result without globally changing JSX child types. Bound textarea SSR values still use the old property path and remain unfinished; this fix covers unbound defaults.

The new unbound regression fails against the saved baseline and passes after the change. The primary passes 339 default native units and 17 focused integration tests, plus strict all-target/all-feature native Clippy. Fresh exact-source TSX passes strict TypeScript 5.7.3 and installed Solid DOM/SSR compilation. Primary Chromium probes verify ordinary/empty/LF/CR/CRLF/hostile-text defaults, dirty edits across updates, node identity, form reset, bound client updates and disposal without browser errors. Independent scoped review found no blocker. No hydration or full application deployment was exercised. Evidence: `/tmp/crepus-solid-textarea-fix/`.

### Follow-up: Solid bound textarea server values

The bound-value limitation noted above is resolved locally. Bound textarea server output now writes escaped raw-text body content; its value attribute is client-only so server getters run once. Client rendering retains the live property path. A demand-emitted helper uses DOM string conversion, including nullish empty text and Symbol rejection, followed by the existing HTML/newline escaping policy. Loop-local bindings keep their generated local access, and the helper name is reserved against component-name collisions.

Both new bound regressions fail against the preserved baseline. The primary passes 339 default native units and 18 focused integration tests, strict all-target/all-feature Clippy, strict TypeScript 5.7.3 and Chromium probes. Eleven value cases compare actual DOM and parsed server output, verify one getter read, and include booleans/numbers/nullish/arrays/objects/hostile text/LF/CR/Symbol errors. Existing unbound edit/reset/identity and bound update/disposal probes still pass. A fresh public-package emitter produces byte-identical TSX to the browser-tested source. Independent review found no blocker. No hydration or full deployed application was exercised. Evidence: `/tmp/crepus-solid-bound-textarea-fix/`.

### Follow-up: extension manifest diagnostics and Firefox module mode

Capability rescans now propagate an existing manifest's load failure, matching the one-shot check. The actual event-handler regression proves invalid TOML yields Error rather than misleading MissingCapabilities advice. Firefox background manifests now declare type module; the shipped background asset uses static imports, while the [Mozilla background contract](https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/manifest.json/background) defaults omitted type to classic. The existing Firefox manifest test failed before the one-line correction.

All 46 all-feature extension Rust tests and 15 Bun tests pass. Strict all-target/all-feature package Clippy with --no-deps passes. The broader Clippy invocation is blocked by preexisting dead-code warnings for Walker::node and textarea_content in crepuscularity-web; those unrelated sources were not changed. Independent review and scoped formatting/diff checks pass. No actual Firefox extension was loaded. Evidence: `/tmp/crepus-webext-manifest-watch-fix/` and `/tmp/crepus-webext-firefox-module-fix/`.

### Follow-up: explicit View IR picker-default capability

Parent value/defaultValue/default-value attributes on select/picker now fail with bind guidance, just as selected-option defaults already do. The guard checks authored presence, so empty/null/malformed expressions and attributes combined with bind cannot disappear silently. This contains unsupported initial-selection behavior; it does not implement initial picker defaults across native/web renderers.

Both new regression cohorts failed against the preserved baseline. The primary passes 339 default native units and 10 focused integration tests, covering both tags, all three aliases, markup/DSL, malformed values and preserved bound options/name/required state. Scoped source review and formatting/diff checks pass. Evidence: `/tmp/crepus-native-picker-default-fix/`.

The picker containment also passes strict all-target/all-feature native Clippy. The Vue leading-newline SSR defect above remains open and has not been patched.

### Follow-up: Vue initial textarea newlines

Leading-LF unbound textarea defaults now use a local Vue directive. Its client beforeMount sets the original defaultValue; its getSSRProps adds one server-only LF so HTML parsing consumes the extra character. The generated textarea stays bodyless so Vue's SSR textarea transform handles and escapes the returned value. Ordinary defaults, inputs and bound textareas retain their prior paths. This does not change existing HTML carriage-return normalization or claim bound-textarea hydration parity.

The new regression fails against the saved baseline. All 339 default native units and 14 focused integration tests pass, along with strict all-target/all-feature native Clippy and strict TypeScript 5.7.3. Exact emitted source compiled with installed Vue 3.5.13 passes Chromium client/parsed-server newline and hostile-text checks, dirty edits across updates, identity, reset and disposal. An isolated leading-LF fixture hydrates both pristine and pre-edited server HTML without warnings/errors. The fresh public-package emitter produces byte-identical SFC output to the browser-tested fixture. Independent review and scoped diff/format checks pass. Evidence: `/tmp/crepus-vue-textarea-newline-fix/`.

### Follow-up: hydration-only helper gates

Walker::node and the textarea_content wrapper are private and have callers only in hydration-gated lib.rs functions. They now carry the same hydration feature gate; plain HTML and SSR continue through the ungated nodes and textarea_content_with_slot paths. This resolves the earlier extension Clippy blocker without suppressing warnings. Strict extension all-target/all-feature Clippy passes with hydration absent; strict web all-target Clippy passes with no defaults and hydration enabled. Eight web units and 21 hydration integration tests pass. Independent caller review found no missed export or call site. Evidence: `/tmp/crepus-web-hydration-feature-fix/`.

### Follow-up: input default-value aliases

View IR input/textfield/textinput/textarea and checkbox submission-value snapshots now accept defaultValue/default-value alongside the existing value spelling. A shared helper resolves the single authored spelling, then reuses existing duplicate validation and evaluation. Distinct spellings fail explicitly because this IR has only one initial-value field. Checkbox checked state and existing bind precedence remain separate. HTML file inputs reject the aliases by presence, and textarea child/default ambiguity still fails.

The three original cohorts fail against the saved baseline. All 339 default native units and 17 focused integration tests pass, including canonical-IR equality across six frontends for static and dynamic property forms, empty/null/numeric strings, malformed expressions, duplicates/conflicts, file values and textarea-body boundaries. Strict all-target/all-feature native Clippy and scoped formatting/diff checks pass. Fresh public lowering from defaultValue source emits byte-identical Vue output to the previously tested canonical fixture; primary compiler/Chromium edit/reset/identity/SSR/default-hydration probes pass again. Two initial test assumptions were corrected against current canonical behavior: shared JSX attribute entities retain their spelling, and React bound inputs use the existing default-plus-ref path. Independent source review found no blocker. This adds initial snapshot aliases, not separate live/reset fields or full source-framework execution. Evidence: `/tmp/crepus-native-input-default-alias-fix/`.

### Bounded review: Vue bound textarea hydration

Sixteen actual Chromium runs compare emitted Vue SFCs with minimal Vue 3.5.13 reference components across ordinary/LF/hostile/nullish values and pristine/pre-edited hydration. Results match in every case. Pre-edited values survive with Vue mismatch warnings in both versions; leading LF is consumed in server parsing in both and remains absent through hydration. Reactive updates, node identity, reset and disposal agree. No Crepus-specific regression was promoted or warning suppression added. Evidence: `/tmp/crepus-vue-bound-hydration-audit/`.

A suspected duplicate picker-name finding was disproved by fresh public emitter output and tracing implicit_form_name: it omits the fallback when an explicit name exists. No production change was made for that suspicion.

### Follow-up: checked-default aliases

View IR checkbox/toggle/switch lowering selects a sole authored checked/defaultChecked/default-checked spelling and retains existing duplicate validation and expression evaluation. Distinct spellings fail because this IR stores one initial checked snapshot. HTML checkbox presence and custom-control boolean conversion retain their separate policies; live bind precedence and checkbox submission values are unchanged. The indentation parser admits both new aliases in its existing bare-boolean token list.

All three regression cohorts failed against exact saved native/parser baselines. The primary passes 339 default native units and 14 focused integration tests, plus 423 core units and 19 indentation-binding tests. Six-frontend static/dynamic canonical-IR equality, bare syntax, false/null/string cases, malformed expressions, duplicates and conflicts are covered. Strict all-target/all-feature core/native Clippy passes. A fresh public emitter feeds installed Solid compilation and actual Chromium checks for initial/default checked state, retained edits across updates, reset, identity and parsed SSR state. The harness uses the public Rust emitter directly, not a rebuilt CLI. Evidence: `/tmp/crepus-native-checked-alias-fix/`.

### Follow-up: embedded disabled-state containment

Embedded buttons cannot represent disabled interaction metadata. They now evaluate authored disabled state and return an explicit unsupported-state error before rendering children whenever it is present under the target's HTML boolean policy. False/null retain normal rendering; expression errors propagate. No public tree fields, click dispatch, styling or hit-test API were added. This contains unsupported state rather than implementing disabled buttons. Hosts must handle errors; existing last-good-document/frame behavior is unchanged.

Three new regressions failed before the guard, with the positive enabled-parity case already passing. All 125 default embedded tests across 29 unit/integration/doc targets and strict default-feature all-target Clippy pass. Tests cover presence/dynamic/uppercase/string values, false/null pixel-handler-hit parity, active branches/includes/slots and framebuffer preservation on errors. A test that incorrectly treated division by zero as an evaluator error was changed to a genuine syntax error; the existing evaluator returns null for that arithmetic case. Physical hardware and actual firmware event dispatch were not exercised. Evidence: `/tmp/crepus-embedded-disabled-fix/`.

### Follow-up: Flutter dependency detection

`crepus flutter add` previously searched the entire pubspec for `crepuscularity_flutter:`, so even a TODO comment could produce a successful no-op with the package absent. It now inspects direct package keys in the supported top-level block mapping. Comments, scalar values, nested package options, similarly named packages and dev dependencies do not count. Existing plain or quoted dependency keys leave the file byte-for-byte unchanged. Unfamiliar mapping forms fail before writing; this helper does not implement a general YAML parser. Independent review caught escaped quoted keys that could alias the renderer name; these now reject before writing, with an actual filesystem preservation regression.

The actual filesystem regression failed with the old detection and passes with the fix. All 123 no-default-feature CLI binary unit tests pass. The regression also checks repeated insertion, quoted existing dependency keys with CRLF, and an unsupported inline mapping remaining unchanged on error. Evidence: `/tmp/crepus-flutter-dependency-fix/`. Strict production-binary Clippy and scoped formatting/diff checks pass. Strict test-target Clippy is blocked by the existing `items_after_test_module` error in `src/preview.rs:89`; no unrelated source was changed. A rebuilt CLI also adds the package on the comment-only fixture and leaves the second invocation unchanged. No Flutter package download or application build was performed.

### New bounded review: TUI hidden content

A fresh public `Template`/Ratatui `TestBackend` probe paints `SECRET` inside a `hidden` span, both at the root and inside a visible layout. Current style handling encodes hidden/invisible as zero dimensions, while layout minimums and root allocation can still paint the text. This remains unfixed. A repair must explicitly suppress painting, distinguish layout visibility semantics, and exclude hidden ancestry from focus discovery. Source and output evidence: `/tmp/crepus-tui-hidden-review/`.

### New bounded review: GPUI animation sequence shrink

Runtime animation identities currently depend on element path or explicit ID, not animation configuration (`renderer.rs:411–423`). The pinned GPUI retains its current sequence index under that identity. An actual headless Metal GPUI probe advances from stage zero to stage one, replaces a two-stage sequence with one stage under the same identity, and panics at `elements/animation.rs:173` (length 1, index 1). The runtime callback's property-index guard runs after this upstream indexing and cannot prevent it. Evidence: `/tmp/crepus-gpui-animation-shrink-review/`. This is an actual pinned GPUI API reproduction plus inspected runtime call flow, not yet a full runtime hot-reload regression. A repair should reset animation state when its configuration changes while retaining the element's interaction identity. Unfixed.

### Follow-up: GPUI animation reload state

Animation wrapper IDs now combine the existing path/explicit identity with a fingerprint of the ordered stage count, property, effective duration, easing and repeat flag. Unchanged configurations retain progress. Edited configurations restart private animation state, while authored interaction IDs are unchanged. No GPUI dependency or public AST type changed.

The actual runtime renderer regression advances a two-stage animation into its second stage, reloads a one-stage animation under the same explicit element ID, and draws again. With the original ID call, it panics in the pinned GPUI at length 1/index 1. With the fix, the stage-one pixel check and the final opaque pixel check pass. All 73 runtime tests pass with ignored Metal fixtures explicitly enabled and serialized, including seven Metal fixtures. Strict all-target runtime Clippy and scoped checks pass. Evidence: `/tmp/crepus-gpui-animation-shrink-fix/`. This validates a headless Metal renderer, not an interactive desktop application or other OS.

### Follow-up: TUI hidden subtrees

Hidden suppression now runs before inserting layout children, flushing adjacent text or dispatching pseudo-elements. It uses active resolved classes plus the HTML hidden attribute, with false/null visible and all other values present. Contextual focus traversal applies the same predicate before recursing; syntactic focus discovery checks only static classes/literal attribute values and leaves unresolved expressions alone. Classes are resolved once per built element. No public style or widget-tree fields changed.

The first class-only candidate passed the dynamic class/include case but failed three actual Cargo tests: bare indentation `hidden` is parsed as a boolean attribute, not a class. The final helper handles both forms. All five final regression cohorts fail against the original renderer under Cargo and pass with the fix. All 272 TUI tests across 29 unit/integration/doc targets pass; the focused suite also passes after adding an explicit focused-ID hide/reveal assertion. Public TestBackend output now contains only VISIBLE for the original nested reproduction and blank cells for hidden roots, in either dimensional-class order. Tests also cover exact layout/gap equivalence, conditional/target classes, false/null/string-false attributes, focus recovery, includes, caller slots and hidden slot/rotation diagnostics. Strict all-target Clippy, formatting, independent review and scoped diff checks pass. Evidence: `/tmp/crepus-tui-hidden-fix/`. Physical terminal interaction was not run. At that stage, `invisible` remained on the prior size-hint path; it is addressed in the next follow-up.

### Follow-up: TUI invisible geometry and painting

Invisible elements now retain their ordinary layout hints and built content measurements. A private flag in the existing block metadata suppresses all paint before borders, background, text or descendants are drawn. Adding default block metadata contributes no layout overhead and preserves existing auto-height provenance; no public widget variant or style fields changed. Contextual and syntactic focus discovery prune invisible ancestry. Unlike hidden, invisible descendants still build/evaluate to preserve dimensions.

All three actual Cargo regression cohorts fail on the baseline and pass after the fix. All 275 TUI tests across 30 unit/integration/doc targets pass. Tests compare the following marker's exact position against visible counterparts in flex, fit, gap and scroll layouts, and cover borders/background, roots, slots, dynamic include/caller-slot visibility and focus. An inspected public TestBackend preview shows the following TAIL row at the same coordinates after the preceding box's text, border and background vanish. Strict all-target Clippy, formatting, independent review and scoped diff checks pass. Evidence: `/tmp/crepus-tui-invisible-fix/`. No physical terminal interaction was run.

### Follow-up: public GPUI transition restart

The public width/height/opacity helpers now namespace their private animation state beneath the caller's typed ElementId using the helper property, duration and endpoint bit patterns. Identical arguments retain progress; edits restart from the supplied starting value, matching the width helper's documented contract. This is separate from template runtime animation reload identity. Public signatures and interpolation remain unchanged.

The actual package Metal regression fails against the original helpers after a completed same-ID transition changes endpoints. With the fix, width, height and opacity all retain unchanged completion, restart from the new starting point and reach the new endpoint. The complete GPUI wrapper suite passes 78 tests across five unit/integration/doc targets with ignored Metal tests explicitly enabled and serialized. Strict all-target Clippy, formatting, independent review and scoped diff checks pass. Evidence: `/tmp/crepus-gpui-transition-restart-fix/`. Duration changes are covered by the identity unit test, not a separate pixel timeline. No physical desktop or other OS was exercised.

### New bounded review: Java HTML output

Java `renderHtml` currently extracts only the first quoted content field from IR JSON, dropping later text and controls and mishandling JSON escapes. An actual CLI/JVM probe with First and Second text nodes returns only First. Evidence: `/tmp/crepus-java-html-review/`. This was subsequently fixed below. The existing canonical `crepus render` CLI is a promising complete route that preserves text, escaping, handlers, filename dispatch and include origins; it must receive an explicit context file even for an empty map to prevent adjacent context.toml autoload. Context-shape validation also needs preservation because the current web CLI context converter silently skips shapes rejected by native IR. No Java source changes were made in this iteration.

### Follow-up: Java canonical HTML and shared JSON validation

Java HTML now calls `crepus render` with the original absolute template path and an explicit owned temporary JSON context, even for an empty map. It uses source HTML/web-renderer semantics, which is documented separately from the unchanged IR API. Both APIs reuse the existing concurrent-stderr subprocess pattern; nonzero diagnostics propagate and the owned context file is removed in finally. No host JSON parser or new dependency was added. The smoke script now compiles and runs the dedicated Java HTML acceptance test.

To preserve validation across the new route, the HTML CLI now reuses `native::ir::merge_json_ctx` instead of its duplicate converter that silently omitted unsupported values. Malformed `.json` context files fail instead of falling back to TOML. All 123 slim CLI binary unit tests, 29 native IR CLI tests and three HTML context CLI tests pass (155 total); supported scalar/list data and ordinary TOML are retained. Strict production-binary/new-integration Clippy passes. Broader test-target Clippy was not rerun; the earlier preview.rs lint is outside this change.

The original Java implementation fails the new acceptance test on multiple nodes/escaping. The fixed actual CLI/JVM checks pass multi-node/button/Unicode/escaping/newline output, relative includes, Svelte filename dispatch, session context and empty-context isolation. Existing Java IR smoke passes. Strict `javac -Xlint:all -Werror` passes. The existing Java/Kotlin/PHP noisy-diagnostic stress fixture passes after the shared runner refactor, and the fixture now also verifies HTML nonzero diagnostics plus temporary-context cleanup. Chromium parses actual Java-generated HTML, retains literal hostile text without executing a script, and exposes the expected button handler metadata through a clicked DOM target. This tests host wiring with a test listener, not a shipped Java browser event runtime. Full polyglot smoke and platform deployment were not run. Evidence: `/tmp/crepus-java-html-fix/` and `/tmp/crepus-render-json-context-fix/`.

### New bounded review: SSR table-region markers

The current web SSR Walker wraps dynamic regions in span elements regardless of their HTML parent. Actual public renderer output plus Chromium parsing shows a conditional table row outside its region marker: the parser moves an empty marker before the table while leaving the row inside tbody. This contradicts manifest-to-DOM ownership. Evidence: `/tmp/crepus-web-table-markers-review/`. The producer is subsequently fixed below; no automatic hydration update failure is claimed. A complete fix needs valid range anchors and matching hydration consumers in restricted insertion contexts, rather than substituting another universal wrapper element.

### Follow-up: valid SSR region ranges in table/select contexts

The shared Walker now tracks the actual HTML parent's insertion context through transparent conditions, loops, matches, includes and forwarded slots. Under table/thead/tbody/tfoot/tr/colgroup/select/optgroup/option, dynamic text/raw/if/for/match/include regions emit paired fixed-kind/numeric-ID comments and add `marker: "range"` to their own binding descriptor. Actual element children reset and restore that context before propagating errors. Ordinary span/element markers, allocation order, root markers, manifest version 1 and plain rendering retain their contracts. No dependency, public entrypoint or automatic region reconciler was added.

All five actual Cargo regression cohorts fail against the original producer. The fixed all-feature web suite passes 167 tests across 22 targets; the no-default-feature suite passes 94 across 20 targets. Strict all-target Clippy passes both configurations. SSR also checks successfully for wasm32-unknown-unknown using the installed rustup compiler selected via command-local RUSTC. The default Homebrew compiler could not find that rustup target; verbose diagnostics showed sccache resolving bare rustc, so the successful retry used the explicit compiler path without changing configuration. A freshly linked public renderer exporter passes 16 Chromium cases covering explicit/implicit table bodies, headers/footers/cells/columns, select/optgroup/option text and raw output, loops, matches, nested/empty regions and forwarded include slots. Every binding resolves to its expected span/element or comment pair, and existing SSR startup detection remains satisfied by the enclosing dynamic element marker. One browser fixture was corrected from row.value to row because its loop rows use the existing scalar-alias contract; production behavior was unchanged.

Twelve test-host contextual DOM Range replacements preserve anchors, static neighbor identities and unaffected selection. Implicit tbody/colgroup parsing can put endpoints in different parents; the documented contract uses endpoints immediately after/before the comments rather than sibling traversal. Contextual replacement can create additional table groups, so this does not promise byte-identical wrapper structure. These are renderer/DOM contract checks, not an application hydration runtime. Current reactive hydration restores context and calls host setup without automatically reconciling structural regions. Range selection, consumer cleanup/reconciliation and physical browser integration remain host responsibilities. Evidence: `/tmp/crepus-web-region-ranges-fix/` and independent cross-parent probes in `/tmp/crepus-range-dom-review/`.

### Follow-up: HTML child declaration scope

Ordinary element children now go through the existing nested node-list walker, which applies declarations in order with a lazily cloned local overlay. This replaces the per-child node path that discarded LetDecl nodes. Root/range marker restoration and textarea handling remain unchanged; no public API changed. The original renderer fails all three actual Cargo regression cohorts: ordered/default/nested scope, condition/loop/include/slot propagation, and evaluated expression errors. A fixture was corrected to pass label explicitly into an isolated include; production include isolation was preserved.

The final combined web/LVGL all-feature run passes 219 tests (170 web and 49 LVGL). Plain web passes 97; strict all-target Clippy passes the all-feature packages and plain web. The SSR web feature also checks for wasm32-unknown-unknown with the installed rustup compiler selected per command. Fresh public-renderer Chromium output shows outer CALLER, local LOCAL, nested INNER and component COMPONENT values while supplied slots retain LOCAL and leaving the element restores CALLER. A test host delegates the button's handler through its closest annotated ancestor, exercising nested SSR spans. The existing 16-case region-range browser matrix and 12 contextual replacements also pass with the newly linked renderer. Evidence: `/tmp/crepus-web-element-declarations-fix/`. This verifies generated DOM and a test host listener, not automatic structural hydration.

### Follow-up: LVGL typed hidden attributes

Hidden binding values now evaluate exactly once before generic string conversion. Typed false/null produce explicit false; every other present value produces true. Existing class hints, explicit-attribute priority and source-order last-wins behavior are unchanged. This aligns source boolean presence with the [public LVGL 9.3 conversion contract](https://raw.githubusercontent.com/lvgl/lvgl/v9.3.0/src/others/xml/lv_xml_utils.c) and its [hidden object-flag setter](https://raw.githubusercontent.com/lvgl/lvgl/v9.3.0/src/others/xml/parsers/lv_xml_obj_parser.c).

All three actual Cargo regression cohorts fail on the baseline and pass after normalization. Tests cover all six parser frontends, bare/static/dynamic values, strings/null/booleans/zero, class/repeated precedence, expression errors, inactive guards, and ordinary/named include props. All 49 LVGL tests pass in the combined gate and strict Clippy passes. Inspected fresh public API XML shows null/boolean-false as hidden=false and bare/string-false as hidden=true. No LVGL runtime/editor or physical panel was launched. Evidence: `/tmp/crepus-lvgl-hidden-fix/`.

### New bounded review: Lite guest watcher symlink targets

The guest watcher resolves a canonical target but watches only the logical path's parent and retains the initial target in its callback. An actual OS probe observes a regular-file edit, then misses an external symlink target edit despite the configured alias reading updated content. Evidence: `/tmp/crepus-lite-guest-watch-review/`. Unfixed. A complete repair needs target-parent subscriptions and reconciliation after symlink/parent-directory retargeting. Core's managed watcher already reconciles root identities, but its public factory also watches neighboring templates/context files; a target-only policy and adapter to Lite's existing debouncer would avoid duplicating that engine. No watcher implementation or public watcher API was changed in this iteration.

### Follow-up: managed single-file guest watching

Core now exposes a notify-feature `SingleFileWatcher` adapter over its existing managed root engine. It retains both logical and canonical target parents for nonrecursive subscriptions, reconciles symlink/root changes in the background, filters to the configured file, and normalizes events to one logical path for Lite's existing debouncer. Its one-active-file contract supports unwatch/rewatch and suppresses late events from the old generation. Existing template watchers retain recursive template/context relevance. Lite keeps its callback thread, debounce configuration and process-lifetime ownership.

The original Lite implementation fails all three new symlink regression tests under actual macOS Cargo execution: external target edits, directory retargets and nested target parents. The fixed Core all-feature suite passes 672 tests; default Lite passes 104, including five guest-watch tests covering those cases, atomic save/delete/recreate, neighboring-file filtering and callback execution off the caller thread. A first test incorrectly required exactly one burst callback; native-event tracing showed no rearm and only a 12ms event burst, while the installed mini debouncer intentionally emits continuous and final notifications. The corrected test permits that bounded pair without changing production debounce semantics. Retarget tests drain across reconciliation intervals before checking subsequent edits. Strict all-target Clippy and formatting pass for Core all features and Lite defaults. Evidence: `/tmp/crepus-single-file-watch-fix/` and `/tmp/crepus-lite-guest-watch-fix/`. No V8-feature build, physical desktop interaction, or other OS was exercised for this watcher change.

### Parallel review queue

Read-only agents identified three further bounded issues, not repaired in this watcher iteration: Tauri command macros ignore explicit rename_all options (current live macro/runtime probe in `/tmp/crepus-tauri-command-case-review/`); Signal::set runs user equality while holding a mutable RefCell borrow, so equality reading the same signal panics (current-source probe in `/tmp/crepus-reactive-signal-equality-review/`); and ABI file-session rendering ignores set_component, unlike template sessions (source inspection plus existing dylib probe in `/tmp/crepus-abi-session-review/`, requiring a fresh package regression before repair). Changing Tauri's default argument casing would need compatibility consideration; explicit option handling can be addressed independently.

### Follow-up: ABI file-session component selection

File sessions now apply the session's explicit component selector to an effective qualified entry before calling the existing virtual-file renderer. The original stored entry remains unchanged: an explicit selector overrides its fragment, and NULL restores that entry. Template selection, source/context ownership, relative virtual includes and event rerendering continue through their existing paths. Header and embedder documentation describe precedence and deferred unknown-component errors.

Both new public ABI Rust regression cohorts fail against the original source and pass after the fix. All 11 ABI tests and strict all-target/all-feature Clippy pass. Coverage includes template/file parity, context updates during event dispatch, unknown selectors and recovery, fragment override/clear, unqualified entry restoration, and selected components with relative virtual includes. The production abi-profile dylib builds successfully. The expanded C11 smoke passes with Wall/Wextra/Werror against that library, the generated Rust smoke exercises all 13 exported functions successfully, and the macOS temporary relocation smoke passes. Independent review, formatting and scoped diff checks pass. Evidence: `/tmp/crepus-abi-component-fix/`. Full polyglot smoke and non-macOS execution were not run.

Parallel follow-up investigation found two considerations for the remaining queue: Tauri's command audit scanner also needs to recognize option-bearing command attributes, and fixing signal equality must also account for user Clone/Drop reentrancy and nested-write semantics. No implementation change for either finding was included in this ABI repair.

### Follow-up: signal value reentrancy and memo cache invalidation

Signal storage now holds shared value snapshots so user Clone, PartialEq and displaced-value Drop code runs outside RefCell borrows. Equality suppresses a write only while the compared snapshot remains current. Comparison-triggered nested writes complete before the outer setter commits; destructors run after commit and notification invalidation. Update keeps its internal read untracked. Unique payload storage is reused, allocating a new payload only when an outstanding read snapshot must retain the previous value. Creating a signal uses one extra shared payload allocation; public APIs and graph-handle ownership are unchanged.

Five initial actual Cargo regressions reproduce borrow panics against the original implementation. The signal-only candidate then exposed a stale memo cache when cloning writes a source during computation: a separate actual Cargo regression observed 1 instead of 7. Memo nodes now carry invalidation revisions; computation and Check validation commit only against the captured revision, preserve fresher nested commits, and retry after user destructors. A private RAII activity counter limits full revision propagation to reentrant Signal writes. Ordinary memo commits and writes outside active computation retain pruning. A first candidate forced repeated descendant scans; refinement restores linear counted traversal for tested lazy chains: 18/38/78 visits for 20/40/80 nodes. This is scoped graph evidence, not a universal latency guarantee.

The final all-feature reactive package passes 76 tests, including nine signal and seven memo regression cases. They cover equality/clone/drop callbacks, snapshot-equal nested writes, comparison/destructor panic recovery, pending effects, untracked update, nested fresh-cache preservation, indirect dirty chains, Check validation, equality suppression and deep batched reads. Strict all-target/all-feature Clippy, formatting, independent review and wasm32-unknown-unknown checking pass. An actual compiled WASM probe in Chromium passes reentrant Clone/memo assertions, text updates, user-edited checkbox updates and disposal stopping DOM changes, without page errors. Optimized actual-source probes assert zero allocations for one million ordinary scalar set/get pairs after setup; a 100-node chain with 100 writes per batch retains the prior 700000 allocations over 1000 batches. Timings collected alongside compilation are exploratory and are not used as a performance claim. Evidence: `/tmp/crepus-signal-reentrancy-fix/`.

Callbacks that continually invalidate their own dependencies still require an application convergence policy; no finite-latency guarantee is claimed. Other browsers, physical devices and a whole-workspace gate were not exercised for this change. Tauri option handling remains queued.

### Follow-up: explicit Tauri command argument naming

The command macro now parses and validates optional rename_all="camelCase" or rename_all="snake_case" rather than ignoring attribute tokens. It converts only serialized payload argument keys after State/AppHandle/Window injection, retaining borrowed-string decoding, async function invocation and existing error propagation. Omitting the option preserves exact unraw Rust parameter names for adapter compatibility; the README explicitly distinguishes this from upstream Tauri's camelCase default. Unknown/duplicate options, unsupported modes, missing equals and nonstring values produce compile errors, including the previously ignored bare async option. Async fn behavior is unchanged.

The implementation reuses heck 0.5 already present in the lockfile, matching the [upstream macro's case-conversion approach](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-macros/src/command/wrapper.rs). Its cached manifest declares MIT OR Apache-2.0; the no_std case-conversion crate has no dependency or telemetry path. The lockfile delta is one macro dependency entry, with no new package/version. The project audit's lightweight source scanner now recognizes option-bearing commands and normal whitespace while requiring complete command attribute names. Existing lexical limitations for comments/strings remain; this does not claim Rust parsing.

Both new command-naming Cargo tests fail on the original macro; the public audit regression fails on the original scanner while its longer-name negative passes. The final all-feature Tauri/macro run passes 21 tests. A real downstream package aliases the adapter as tauri, invokes a camelCase command without a direct serde_json dependency, and checks seven invalid option forms through the compiler. Runtime cohorts cover sync/async, borrowed/raw/acronym/Unicode names, injected parameters, optional values, renamed diagnostics and existing exact-key compatibility. The scanner covers seven bare/options/whitespace forms plus longer-name/other-qualifier negatives. Strict all-target/all-feature Clippy, formatting, independent review and scoped diff checks pass; a new test-loop lint was corrected and the affected four tests rerun. Evidence: `/tmp/crepus-tauri-command-naming-fix/` and `/tmp/crepus-tauri-command-audit-fix/`. No physical desktop integration or whole-workspace gate was run.

### Next bounded performance review: GPUI include preparation

Read-only review confirms HotReloadView::render calls render_nodes synchronously with a fresh per-render include cache. Ordinary includes reach ast_cache::parse_file and read_to_string; named includes read and parse directly. Include resolution also uses filesystem canonicalize/exists. The existing background entry loader prepares the entry AST/context, leaving included files on the render path. A complete repair must prepare the resolved include graph as well as its contents; merely populating virtual_files would leave path-resolution filesystem work on the UI thread. Investigation is ongoing, with no latency measurement or implementation claim yet.

Evidence and a scoped worker-preparation proposal are saved in `/tmp/crepus-runtime-include-worker-review/`. An actual cached-core FIFO probe confirms parse_file waits synchronously for source; it is not a GPUI latency measurement. Core's thread-local AST cache cannot be prewarmed on the worker for use on the UI thread. Preparation must retain lazy inactive-branch errors and resolve includes with the correct base directory/virtual bundle, while props, slots and scope remain per-occurrence evaluation. Public render_nodes/node callers should retain fresh-render semantics.

### Follow-up: GPUI hot-reload include snapshots

HotReloadView now renders a shared owned include graph prepared on the existing background executor. Preparation resolves paths and loads ordinary/named component sources across branches, loops, matches, authored slots and captured external slot contexts without evaluating runtime scope. Its per-bundle maps retain path/parse results, including lazy errors, and prepared rendering has no filesystem or parser fallback. Depth-aware visitation permits a shallower later traversal of a previously depth-limited file; cycles retain the existing render-depth diagnostic. The existing stack-growth helper protects preparation recursion. Shared backing maps are not cloned per frame.

Completed root, include graph and owned-loader context publish together after checking the current entry path, root identity, structural context and changed flag. Caller-managed variables remain live. Changes to the public AST/base directory/virtual bundle/slot structure trigger preparation of the current AST on the next poll without rereading the entry or invoking the owned loader. The prior root/include environment remains visible with current variables until publication. Worker capture excludes ordinary root variables; slot AST comparison and the pending-structural-refresh context clone remain UI CPU work. Public template errors still render existing diagnostics.

Direct public rendering keeps its synchronous fresh-per-call contract. The first candidate cached dynamic errors and failed the existing failed_named_parses_do_not_poison_render_cache regression; it was corrected so only preparation snapshots errors, while direct rendering caches successes. New HotReload tests initially inspected AnyElement's erased wrapper ID, which GPUI intentionally reports as None; they now inspect the actual rendered Stateful Div, matching existing renderer tests.

Final combined runtime/GPUI verification passes 165 tests (87 runtime with all eight Metal cases enabled, plus 78 GPUI wrapper tests), using the runtime text-style feature and serialized Metal execution. Default runtime separately passes 79 with eight hardware tests ignored. Strict runtime all-target/all-feature Clippy, formatting, independent review and scoped diff checks pass. New tests cover deferred include loading, rendering after deletion and symlink escape replacement using the old snapshot, explicit reload refresh, variable edits without re-preparation, manual root/VFS changes without context-loader calls, base and in-place slot changes, Err-to-manual-Ok recovery, actual include watcher delivery, edits during worker loading, inactive errors, deep/cyclic include traversal, and a 70-context captured-slot chain. A real Metal fixture prepares nested ordinary/named includes on a distinct worker thread, edits red source to blue, deletes both include files, and successfully paints the original red and refreshed blue snapshots. Evidence: `/tmp/crepus-runtime-include-worker-fix/`.

This removes include source I/O and path resolution from HotReloadView rendering; it does not move layout, scope evaluation or painting off the UI thread. Existing watch coverage remains the entry subtree; external include roots need explicit reload. In-progress synchronous worker I/O is not cancellable. No frame-time benchmark, physical desktop session, other OS or whole-workspace gate was run.

### Next bounded performance review: target-default context cloning

An isolated actual-source with_gpui_target probe confirms default HotReload contexts lack the eight target keys and therefore clone the full variable context during rendering. A List/Scope fixture at 100/200/400 rows allocates 319/619/1219 times and 91701/179001/353601 bytes per helper call; contexts already containing every default return borrowed with zero allocations. Evidence and a possible temporary-default restoration-guard proposal: `/tmp/crepus-runtime-target-clone-review/`. This is allocation evidence only; no implementation or frame-latency claim yet. Public immutable rendering and caller target overrides must remain unchanged.


### Parallel follow-up review findings

Read-only module reviews identified four bounded follow-ups, not fixes or runtime measurements:
- Kotlin `renderHtml` still extracts only the first IR content string, losing additional nodes and truncating escaped quotes; mirror the canonical CLI route used by Java.
- LVGL loops reread and reparse each included source per occurrence; add a per-public-render source cache while preserving occurrence scope and next-render freshness.
- Legacy web hydration unwraps object-valued application `ctx` keys; classify canonical versioned envelopes before unwrapping and preserve raw application objects.
- Flutter comparisons stringify null as `"null"`, while core uses the empty string; align scalar comparison coercion and test null/empty/literal-null branches.
The bounded parser/View IR review found no new concrete defect. These findings remain source-derived and unimplemented.


### GPUI target-default clone removal verified

HotReloadView now borrows its owned context with scoped missing target defaults;
existing overrides remain untouched and added keys are removed on normal return
or unwind. A renderer panic is caught inside the state update and resumed after
GPUI returns the entity lease. Public immutable rendering retains its prior
contract. Map capacity/order can change after temporary insertion; keys and values
are restored. Pending structural-refresh fallback and external captured slot
contexts can still clone.

The actual HotReloadView TestAppContext window probe forces twenty fresh draws
after five warmups. At 100/400 unused List/Scope rows, the baseline allocates
566/2066 times and 107724/379824 bytes per draw; the candidate allocates 57 times
and 12643 bytes in both cases. Thread-local counters exclude background/setup
work. This is allocation evidence, not a frame-latency or physical desktop claim.
Temporary probe source and before/after logs live at
`/tmp/crepus-runtime-target-defaults-fix/`; it is not permanent project tooling.
Combined all-feature runtime/GPUI checks pass 168 tests including serialized
Metal cases; strict runtime Clippy passes. Scoped tests also verify target
branches, caller overrides, payload identity and restoration on panic. No commits,
pushes or deployment were performed.


### Hydration, Kotlin HTML and Flutter comparison follow-ups verified

Legacy hydration application objects with object-valued `ctx` keys no longer
lose sibling state. Only recognized version-1 envelopes unwrap their context;
malformed canonical contexts still produce empty state. The new regression fails
against the prior decoder and passes with the fix. Reactive verification passes
77 native tests, strict all-target/all-feature Clippy, and an all-target WASM
check. An actual WASM Chromium DOM probe verifies legacy raw/base64 state,
canonical precedence, nested application contexts, invalid canonical context,
and absent payload behavior. The first browser assertion compared JSON key order;
it was corrected to compare fields because Rust HashMap iteration is unordered.
Evidence: `/tmp/crepus-hydration-envelope-fix/`.

Kotlin renderHtml now uses the canonical CLI renderer, preserving all nodes,
controls, escaping, source frontends and includes. Its explicit temporary context
also prevents implicit context.toml loading for an empty supplied context. The
shared process helper closes stdin, drains stderr concurrently, propagates errors
and destroys the child; temporary context cleanup runs in finally. Strict Kotlin
compilation, actual CLI IR/HTML tests and noisy Java/Kotlin/PHP diagnostics pass.
The new Kotlin HTML acceptance and failure-cleanup tests are wired into the
existing smoke runner. A Chromium check confirms exact Unicode/escaped text,
the second button and its handler metadata, with a labelled host listener click;
it does not prove automatic framework event dispatch. The baseline fails the new
HTML test. Evidence: `/tmp/crepus-kotlin-html-fix/`.

Flutter condition equality now coerces null to an empty string, matching core,
while leaving the numeric comparison branch unchanged. New widget tests exercise
null aliases, missing paths, null data, quoted empty/literal-null strings in both
operand positions, scalar controls, source rendering and live loop updates. The
baseline fails and the fixed full renderer suite passes 539 tests; scoped analyzer
and formatting pass. Other preexisting mixed float/string and collection coercion
differences are outside this bounded fix. Evidence:
`/tmp/crepus-flutter-null-comparison-review/`.

Primary diff inspection and independent hydration/Kotlin reviews found no
blockers. LVGL per-occurrence include parsing remains the next performance
follow-up. No whole-workspace gate, device deployment, commits or pushes.


### LVGL per-render include caching verified

LVGL now shares resolved paths, source strings and lazy ordinary/named ASTs within
one public render. The cache retains virtual bundle Arcs and partitions by pointer
identity; resolution keys include the caller base directory. Named component ASTs
are moved into shared nodes once rather than cloned per occurrence. Props,
frontmatter/body defaults, caller slots, target context, active-branch errors and
include depth still evaluate per occurrence. The next public render creates a
fresh cache, so changed/deleted files and path validation are rechecked. This is
a per-source first-lookup snapshot, not an atomic filesystem-wide snapshot.

Seven new public/private tests cover repeated loop props/defaults/slots, separate
captured bundles with the same path, next-render virtual/disk edits, lazy errors,
finite cached recursion and depth limits, pointer-identical AST reuse after source
deletion, and all named definitions already parsed before a second component is
requested. Combined LVGL and umbrella manifest tests pass 59 cases. Strict
all-target/all-feature Clippy for both crates, LVGL all-target/all-feature WASM
check, formatting, independent review and scoped diff inspection pass.

A real public-render allocation probe compares ordinary/named includes at
100/200/400 rows against an inline-render XML oracle. All six baseline/candidate
XML files are byte-identical. At 400 rows, ordinary includes drop from 30449 to
22875 allocations and 3815640 to 2817773 allocated bytes; named includes drop from
33649 to 22879 allocations and 4298840 to 2815288 bytes. The fixture uses virtual
sources and a debug host build; timing samples are retained as diagnostics only,
not a frame-latency or physical-panel claim. Evidence and temporary probe source:
`/tmp/crepus-lvgl-include-cache-fix/`. The probe was removed from project examples
after use. XML generation, per-occurrence evaluation and authored-slot context
clones remain synchronous. No whole-workspace gate or physical LVGL panel run;
no commits or pushes.


### Effect initialization, Tabs admission and embedded extraction verified

A failed first Effect::new run now removes its unreturned graph node using a
constructor-only unwind guard. Existing remove_node cleanup releases subscriptions,
queued jobs and the callback outside graph borrows; ordinary Effect handle-drop
semantics remain unchanged. Native regressions prove capture reclamation, queued
self-work removal, original panic payload, restored parent observers and normal
later signal writes. The baseline leaks the captured closure. Reactive verification
passes 79 tests and strict all-target/all-feature Clippy. Rebuilding and exercising
the WASM browser probe also preserves text, edited checkbox and explicit-disposal
behavior with no page errors. Browser panic recovery is not claimed; panic-abort
cannot execute unwind cleanup. Evidence: `/tmp/crepus-effect-constructor-unwind/`.

View IR Tabs now reject unsupported direct content instead of accepting and
silently discarding it. Direct tab/page elements and literal whitespace remain
supported; panel declarations/control flow still lower normally. Existing specific
metadata-handler/include/control-flow diagnostics remain. New tests span all six
frontends, all three Tabs aliases, unknown elements/text, whitespace, real panel
content and invalid declarations/expression text. The baseline fails the new tests.
The full native all-feature suite passes 1473 tests and strict all-target/all-feature
Clippy passes. Evidence: `/tmp/crepus-tabs-children-fix/`.

The embedded Bgr rgb565_for_panel accessor now borrows framebuffer and scratch as
disjoint fields, removing the source-frame Vec clone. The public-accessor test at
240x320 measures ten warmed extractions at ten allocations / 1536000 bytes before,
versus zero allocations / bytes after. RGB output remains borrowed. Exact byte
swapping, red-to-blue render updates, scratch pointer reuse and empty frames pass.
Embedded verification passes 126 tests, strict Clippy and a no-default-feature
check. A new constant-chunk Clippy lint in the test was fixed with as_chunks and
the focused check rerun. Main flush behavior was already copy-free and is unchanged.
Evidence: `/tmp/crepus-panel-bytes-fix/`.

The initial full native link ran out of disk. Under the standing project-space
cleanup request, cargo clean for crepuscularity-native removed 5.0 GiB of regenerable
package build artifacts; source and installed runtimes were preserved. The full
suite then passed with test debug info disabled and two build jobs. Primary actual
diff inspection, independent review, formatting and scoped diff checks pass.
No whole-workspace gate, physical panel/device validation, commits or pushes.

Read-only follow-ups remain: Webext dev ignores Remove events and rebuilds overlay
stale outputs (deleted pages/assets remain, custom generated CSS can accumulate),
so a trigger-only fix is incomplete. See `/tmp/crepus-webext-deletion-review/report.md`.
Flutter mixed float/string spelling and collection equality still differ from core;
these require dedicated coercion/structural-comparison work beyond the null fix.


### Webext staged output and live watcher recovery verified

Browser builds now generate a fresh sibling stage, validate literal manifest asset
paths, and replace only the selected browser leaf. Removed pages/assets disappear
and generated CSS does not accumulate. Normal generation/validation failures retain
the complete prior output; invalid authored popup rendering now fails explicitly.
Output symlinks are rejected. Rename rollback preserves the prior directory on
publication failure; abrupt process termination can leave hidden stages, and the
two-rename replacement has a brief missing-path window. Safari packaging remains
after browser publication and is not part of that rollback transaction.

The dev watcher admits removals and watches parent directories for recreation and
atomic config saves. Failed builds keep watching without advancing the reload ID.
An actual-process idle assertion caught repeated rebuilds on macOS: refreshing
unchanged watches restarted FSEvents, and fresh APFS clone copies emitted source
file events. Watches now retain unchanged directory identities; Apple asset copies
precreate their destination to retain ordinary fs::copy metadata semantics without
cloning. The final actual-process test passes deletion, recreation, failed page and
popup recovery, monotonic reload IDs and stable idle behavior.

Verification: 137 CLI unit tests, five staged-output integration tests and one
actual dev-process test pass. Default-feature CLI strict all-target Clippy passes.
The slim all-target lint has an existing items_after_test_module failure in the
unrelated dirty preview.rs; its scoped check permits only that lint. Independent
output and watcher reviews found no blockers. Evidence and before/after patches:
`/tmp/crepus-webext-output-fix/`. No physical extension installation, real Safari
packaging, Windows watcher run or whole-workspace gate is claimed. No commits or
pushes were performed.


### Flutter structural condition equality

A baseline widget regression confirms collection conditions compared printed Dart
values: an empty list incorrectly equaled the string "[]". Equality now compares
ordered list elements and map keys/values, using the existing scalar coercion for
nested fields. Mixed collection/scalar values never compare equal. An iterative
walk avoids recursive descent; identity-pair tracking terminates shared/cyclic host
collections, which exceed the JSON/Core data domain. Ordinary scalar comparisons
skip collection traversal allocations. Integral doubles now omit a trailing .0
when compared with strings, including negative zero; exponent spelling still follows
Dart and is documented rather than claimed identical to Rust.

Focused widget regressions cover collection/scalar ambiguity, list order/length,
map key sets/order, nested values, scalar coercion and cyclic/shared host data.
The baseline fails; eleven focused tests and the final 541-test full Flutter suite
pass. Scoped Dart analysis, formatting and diff checks pass. Independent review
found no blockers.
Package-wide analyzer reports the existing publishable-package path dependency
warning at pubspec.yaml:25; the manifest was not changed in this iteration.
Evidence: `/tmp/crepus-flutter-collection-equality-review/`.

A separate read-only manifest audit reproduced invalid Chromium WAR match paths
when deriving resources from scoped content-script/host patterns. Source, emitted
JSON and external contract are recorded at `/tmp/crepus-webext-war-match-review/`;
normalization plus origin-level deduplication remains the next scoped fix.


### Webext resource origin matches corrected

The prior read-only manifest finding is now fixed. Two new tests fail on the
baseline: derived host/content patterns and explicit resource patterns retained
page paths, which Chromium rejects for web_accessible_resources.matches. The
shared MV3 generator now normalizes supported URL patterns to origin + /* and
deduplicates after normalization, retaining first-origin order. Host permissions
and content-script URL paths are unchanged; explicit resource matches still replace
the derived fallback. <all_urls> remains unchanged, file paths normalize to file:///*,
and malformed authorities/unsupported schemes remain browser validation concerns.
Documentation now states resource exposure is origin-wide, including when an
explicit resource match was authored with a path.

The complete all-feature Webext suite passes 48 tests and strict all-target,
all-feature Clippy passes. Fifteen Bun extension-asset tests pass. A freshly built
CLI generated Chromium, Firefox and Safari manifests from the scoped-path fixture;
JSON assertions verify resource origin normalization and unchanged host/content
paths in all three. Five existing CLI Webext integration cases also pass.
The wasm32 check cannot run with the active toolchain: its core target library is
not installed (E0463), so WASM-target compilation remains unverified.
Independent review, scoped rustfmt and diff checks pass.
Evidence: `/tmp/crepus-webext-war-normalization-fix/`. Actual browser loading and
whole-workspace gates are not claimed. No commits or pushes.

A read-only TUI audit found eval_value converts evaluation errors into Null, so
active malformed interpolation/declarations can silently disappear. The existing
BufferRenderer build_error channel offers a bounded error-before-paint fix while
preserving lazy inactive branches; a public-API failing test is still required.


### TUI active text and declaration errors propagate before painting

A public Buffer rendering regression reproduced successful blank output for
malformed active text/raw expressions and declarations. Text-part evaluation and
declaration application now return the original evaluator error in both mixed
children and pure-text elements. The existing renderer build_error channel retains
the first failure and returns before painting. Root component defaults also fail
before paint when absent, and satisfied defaults skip evaluation. Focus discovery
remains best effort. Hidden subtrees and inactive branches are not evaluated;
missing variables retain normal null/empty text semantics.

Eight final regression cases pass, covering prior-buffer preservation, renderer
reuse, first-error identity, root/pure-text declarations and raw expressions,
provided/missing component defaults, includes/caller slots and cached Template
retries/recovery through a Ratatui TestBackend. Two initial cases fail on baseline.
The full TUI suite passed 279 tests before the final four added coverage cases;
the final eight-case target passes separately, yielding 283 distinct passing tests.
Strict all-target/all-feature Clippy, scoped formatting, actual diff inspection and
independent review pass. Evidence: `/tmp/crepus-tui-expression-errors-fix/`.
No physical terminal, whole-workspace gate, commits or pushes are claimed.

Remaining expression consumers still use legacy fallbacks: conditions, dynamic
classes, loop/match selection, include props, IDs, titles, scroll bindings and
rotation intervals. Those need their own lazy, fallible-path checks before claiming
uniform TUI expression error propagation. Rotator interval fallback is documented
cross-target behavior and should be assessed separately from malformed expressions.


### TUI control expressions now fail before painting

Two baseline public regression cases confirm malformed active if/for/match
expressions returned success instead of their evaluator errors. Branch conditions,
loop iterators and match selectors now retain the original first error through the
existing build_error channel, so the shared Buffer/Frame path returns before paint.
Valid missing/null/non-list iterators retain empty-loop compatibility. Inactive
branches, unselected match arms and empty-loop bodies stay lazy. The shared loop
visitor is fallible and can stop after a body error, restoring row overlays first;
focus discovery explicitly retains best-effort behavior.

Four new public cohorts cover original error identity, exact previous-buffer
preservation, laziness, non-list compatibility and renderer recovery. One internal
visitor test verifies remaining rows are not visited after stopping. The full TUI
suite passes 288 tests; strict all-target/all-feature Clippy, scoped formatting and
diff checks pass. Independent actual-source review found no blockers. Evidence:
`/tmp/crepus-tui-control-errors-fix/`. No physical terminal or whole-workspace gate,
commits or pushes. Dynamic classes, include props and visual bindings still retain
legacy expression fallbacks and are not covered by this control-flow fix.

The parallel framework-emitter audit identified loop-dependent image/link URL
expressions being snapshotted against parent context before React/Solid/Imba emit,
so they can become empty literal URLs. This is source evidence only; a real emitted
module/host regression is required. Retaining dynamic URL expressions across View
IR and relevant hosts is preferable to claiming support through a rejection-only
change. Evidence: `/tmp/crepus-framework-row-url-review/`.


### Live image/link URLs retained across framework emitters

The actual CLI baseline IR confirms authored root and loop src/href expressions
were lost as empty or initial snapshot strings. Image now carries optional srcBind,
Link hrefBind, while preserving existing snapshot fields. Lowering retains validated
nonliteral expression-origin bindings; static literal output and old JSON remain
unchanged. React, Solid, Svelte, Vue, Imba and Moonshine DOM/SSR use existing scoped
expression/text helpers, including lexical loop variables. Generated SwiftUI and
Compose links evaluate current scope through their existing model helpers and avoid
activating blank destinations; native images remain existing text placeholders.
Flutter View IR decodes the optional fields and resolves its supported constrained
operands/mixed text against live root/loop data. Unsupported Flutter expressions
retain snapshots; its tolerant source parser remains unchanged.

Image mutation diffing now compares srcBind, preserving expression-only changes;
Link uses existing structural replacement. Independent review caught lexical capture
compatibility: declarations/includes/named defaults have values unavailable to host
root scope. Three new regressions fail before a conservative existing-depth guard
retains those URL snapshots. This does not claim live captured lexical environments.
Rust callers constructing Image/Link variants must supply the additional optional
field; the additive JSON fields default absent without an IR version change.

Actual untouched CLI emitted React mounts/rerenders in React + happy-dom; Svelte,
Vue, Solid and Imba compile with installed compilers and pass Chromium checks;
Moonshine DOM + SSR execute with the installed local runtime. Checks cover root and
multiple lexical rows, scope/list updates, shrinking/empty lists, null/missing
values, exact quote/ampersand/less-than URL characters and disposal. Chromium reports
no JavaScript errors. React intentionally omits a blank image src and logs its
platform warning; SSR escaping is checked for Moonshine. Image resource loading is
not an acceptance claim. Rebuilding the final CLI after the capture guard yields
byte-identical source for all seven verified web targets.

The all-feature native suite passes 1489 tests, and the final nine-case URL target
adds the six-frontend coverage case, for 1490 distinct native tests. All 544 Flutter
tests and nine existing CLI emission integration cases pass. Strict native
all-target/all-feature and default CLI all-target Clippy pass; scoped Dart analysis,
formatting and diff checks pass. Primary integration and actual scoped diffs plus
independent shared-IR review found no blockers. Evidence, snapshots, hashes, runtime
scopes and logs: `/tmp/crepus-dynamic-url-fix/`,
`/tmp/crepus-dynamic-url-review/`, `/tmp/crepus-flutter-dynamic-url-review/`.
No physical native device/build validation, full-workspace gate, commits or pushes.
Remaining: captured lexical URLs, general Flutter expression parity, dynamic other
media attributes and legacy static demo hosts.


### TUI include property errors and Flutter source URL bindings

Two public baseline TUI cases reproduce malformed include-property expressions
silently becoming Null. prepare_include_context now returns Result and uses the
original evaluator error. Build records the first failure and returns before any
paint; best-effort focus discovery skips an invalid include. Existing path/read/
parse/depth inline diagnostics, isolated child scope, target flags, virtual files
and caller-context slot capture are preserved. Four new test cohorts cover exact
error/previous pixels, inactive/hidden laziness, missing prop Null compatibility,
caller/child/slot scopes and recovery with later caller data. Full TUI verification
passes 292 tests and strict all-target/all-feature Clippy. Independent scoped diff
review found no blockers. Evidence: `/tmp/crepus-tui-include-errors-fix/`.

The Flutter source parser previously stripped src/href braces without retaining
origin, causing link taps to dispatch the variable name and images to keep a
placeholder. The complete balanced unquoted outer expression now populates srcBind/
hrefBind for the existing live scope resolver. Quoted URL braces remain literal;
partial brace wrappers and other tolerant parsing retain prior behavior. Repeated
attributes clear stale metadata before applying the final value. Initial three
widget regressions fail on baseline; 51 focused tests and the full 547-test Flutter
suite pass; the final four-case source target adds a passing duplicate-attribute
regression, yielding 548 distinct Flutter tests. Scoped Dart analysis
and formatting pass. Independent parser review found no blockers. Evidence:
`/tmp/crepus-flutter-source-url-review/`.
No new dependencies or evaluator, physical terminal/device verification,
whole-workspace gates, commits or pushes. General Flutter expressions remain
constrained; remaining TUI visual binding/class expression fallbacks are unchanged.


### TUI class interpolation and condition errors propagate

Three public regression cases fail on baseline because malformed class interpolation
or conditional class expressions render successfully after losing the evaluator
error. active_classes now returns Result; rendering records the original first
error before constructing or painting that element. Conditional class values are
interpolated only after a true condition. Both focus-discovery callers explicitly
retain best-effort fallback. slot-rotate now receives the already resolved class
slice, eliminating its redundant second evaluation without changing interval or
phrase behavior. Existing class-before-own-hidden and interpolation-before-target
filter ordering is retained; inactive branches and hidden-parent descendants stay
lazy.

Four new public cohorts pass: original evaluator error/previous pixels for ordinary
and rotating elements, conditional value laziness, hidden/inactive descendants,
renderer recovery and live red/blue/missing classes. Full TUI verification passes
296 tests and strict all-target/all-feature Clippy. Primary actual diff inspection,
independent review, scoped formatting and diff checks pass. Evidence:
`/tmp/crepus-tui-class-errors-fix/`. No physical terminal or whole-workspace gate,
commits or pushes. Titles/hidden/scroll bindings and rotation interval fallbacks
remain separate unfinished expression consumers.

Separate source-confirmed parser follow-up: indent element.rs:217 splits quoted
class values with split_whitespace before interpolation. Thus class="w-{1 + 2}"
becomes fragments and reports an unclosed placeholder instead of producing w-3.
The equivalent unquoted class={1 + 2} route keeps its expression intact. Preserve
interpolation spans while splitting quoted class tokens, after auditing shared
consumers and static token conventions; no parser fix is claimed in this batch.


### Quoted indentation class interpolation spans preserved

Five of six new core regression cases fail on baseline: class="w-{1 + 2} flex"
and quoted when classes split expression whitespace into separate AST tokens.
The class-list and when-list paths now share a small splitter that preserves
complete interpolation spans using the existing delimiter matcher. Unicode
whitespace outside expressions keeps normal static token behavior; adjacent
prefixes/suffixes stay with their expression. An unmatched opener retains the
remainder so evaluator diagnostics see the original malformed span. Static class
AST tokens and alias lookup conventions remain unchanged. Independent review
confirms UTF-8 byte boundaries, linear scanning and shared caller coverage.

Six core cases cover both quote types, Unicode separators, quoted/nested braces,
malformed spans, unchanged static lists and when classes. Two TUI TestBackend
cases verify actual red/blue cells after context changes and false-when laziness;
two native lowering cases verify exact evaluated style token arrays. Verification
passes 690 core all-feature tests, 298 TUI tests, 352 native unit/focused integration
tests and 312 tests across web/embedded/LVGL, totaling 1652 scoped passing tests.
Strict all-target/all-feature Clippy for core/TUI/native passes. The old real CLI
fails the fixture with an unclosed-placeholder error; rebuilt CLI emits the same
fixture as <div class="w-3 flex">Example</div>, checked with an HTML parser.
Formatting, actual diff inspection and diff checks pass. Evidence:
`/tmp/crepus-quoted-class-fix/`, `/tmp/crepus-quoted-class-target-tests/`.
No full native web-module suite, GPUI runtime, physical device/terminal or
whole-workspace gate is claimed. No new dependencies, commits or pushes.
The GPUI runtime's separate bounded class interpolation behavior and remaining TUI
visual-binding fallbacks still require focused work.


### GPUI embedded dimension expressions now style and repaint

Runtime apply_context_class previously handled hex colors and opacity only, then
passed other embedded expressions through as literal utility names. Three new
styler cases fail on baseline for width/height/spacing and matching state variants.
The fallback now interpolates the class once and applies its resolved tokens with
no context, preserving the existing nonrecursive class-value contract. Successful
hex/alpha/opacity handlers retain precedence; missing/malformed expressions retain
the prior no-op behavior. No renderer production code or public API changed.

Three new style regressions compare complete refinements against static utilities
across context updates, including hover/target prefixes, original color/opacity
semantics, multi-token values, error no-ops and brace-containing nonrecursive data.
An ignored macOS Metal regression parses actual quoted dimension expressions,
renders a white 100x100 window with a red child, and checks every logical sample.
Baseline fails immediately with white at the expected red origin. The candidate
paints 30x20, then updates the same view's context, notifies and repaints 60x40;
all inside/outside samples pass. The final all-feature Metal run also passes.

Full all-feature runtime/GPUI tests pass 155 cases (other GPU fixtures remain
ignored); the explicitly exercised new Metal case yields 156 distinct checks.
Strict all-target/all-feature Clippy, scoped formatting, primary actual diff
inspection and independent production review pass. Evidence and snapshots:
`/tmp/crepus-runtime-dynamic-class-fix/`,
`/tmp/crepus-gpui-dynamic-dimensions-fix/`. The renderer test author did not retain
a before snapshot; primary reviewed its exact new test block, without treating the
large preexisting renderer git diff as this task's changes.
No other-platform or whole-workspace gate, commits or pushes. GPUI styling still
uses legacy error no-ops and existing unsupported utilities; this adds no claim
of universal utility coverage or impossible-to-lag rendering.


### GPUI dynamic state shadow composition

Dynamic hover ring/shadow utilities were classified before interpolation, losing
base shadow composition for whole and partial expressions. A failing regression
reproduced it. State styling now passes the base refinement through the existing
resolution path and seeds shadows only when applying a resolved ring/shadow
utility. Explicit empty resets stay empty; subsequent utilities retain order.
Public styling APIs and specialized color/opacity resolution remain unchanged.

Three unit regressions cover context updates, whole/partial expressions, nested
target prefixes, nonmatching/no-op/nonrecursive values and reset ordering. A new
Metal test dispatches actual hover events and checks ring and ordinary-shadow
pixels independently. Full all-feature runtime/GPUI suites pass 158 tests; both
explicitly exercised Metal state-shadow fixtures pass, totaling 160 distinct
checks. Strict all-target/all-feature Clippy passes after restoring an existing
lint annotation displaced during helper extraction. Scoped formatting, primary
actual diff inspection and independent production review pass. Evidence:
`/tmp/crepus-runtime-state-shadow-fix/`,
`/tmp/crepus-gpui-state-shadow-expression-fix/`,
`/tmp/crepus-runtime-dynamic-state-shadow-fix/`.

With 311 MiB free initially, authorized cleanup of regenerable native-crate Cargo
artifacts removed 3349 files / 2.6 GiB before verification. No source or installed
runtime was removed. No whole-workspace or other-platform gates, commits or pushes.
This fixes runtime state composition, not a universal no-lag guarantee; generated
macro target-prefix classification remains a separate audit candidate.


### Generated GPUI state lists preserve target-prefixed shadow composition

Independent caller review confirmed the generated-view shadow classifier stripped
GPUI target prefixes only before the state, although the utility applicator also
accepts them after it. Thus evaluated class lists such as `hover:gpui:ring-0`
reset the ring on an unseeded collection and lose ordinary base shadows. The
shared classifier now strips repeated `gpui:`/`gui:` prefixes after a matching
state too. Foreign targets and other states remain excluded. No public API or
macro production code changes.

A new unit regression fails on baseline and covers all three states, both reset
kinds, nested prefixes and negative cases. A generated-view Metal hover test also
fails on baseline with an incorrect ring pixel, then passes after the fix. The
fixture reuses the existing hover helper and retains its static-first-box case.
Full all-feature runtime/GPUI suites pass 159 tests; two explicitly exercised
Metal generated-view cases pass (161 distinct checks). Strict all-target/all-feature
Clippy, scoped rustfmt, diff checks, primary actual scoped diff inspection and
independent review pass. Two new-test lint issues were corrected before final
verification. Evidence: `/tmp/crepus-macro-shadow-prefix-fix/` (baseline failures,
final logs, before snapshots and scoped patch).
No commits, pushes or new dependencies. Whole-workspace/other-platform gates and
static macro literal nested-prefix support are outside this verified fix.


### Static GPUI state target prefixes and TUI hidden errors

Parallel reviews found two concrete inconsistencies. Static GPUI state utilities
with inner target prefixes silently dropped; conditional equivalents failed with
Unknown class. Seven production lines normalize state utilities before mapping
and shadow classification and normalize direct style mapping too. The baseline
conditional Metal fixture fails to compile; final static/conditional actual hover
pixels pass alongside both prior generated-view fixtures. Unit coverage compares
normalized mappings and generated state bodies across all three states, reset and
spacing utilities, and nested target prefixes. Foreign targets generate no state
call or shadow seeding. Test expectations were corrected for existing lexical
helper emission and nondeterministic generated IDs using an explicit fixture ID.

Malformed TUI hidden bindings previously became null, making content visible.
The hidden predicate now returns the original evaluation error; tree building
records it before painting. Static hidden classes, earlier true hidden bindings,
hidden ancestors and inactive branches remain lazy. Three new buffer regressions
cover original errors, unchanged sentinel cells, lazy paths, existing valid-value
semantics and reuse after failure. Two fail on the unmodified production baseline.
Primary corrected overescaped fixture quotes before red/green verification.
The public focus-discovery list remains best effort, with its explicit fallback.

Final checks: 30 macro tests (including template dependency rebuild integration),
159 runtime/GPUI tests, three explicit Metal hover tests and 301 TUI tests: 493
passing scoped checks. Strict all-target/all-feature Clippy passes separately for
macros, runtime/GPUI and TUI. Scoped rustfmt, diff checks and primary actual diff
inspection pass. The first combined package test invocation exhausted build space;
CLI Cargo cleanup removed 2945 generated files / 1.1 GiB, and separate package
verification avoided the unnecessary combined feature-unification rebuild.
Evidence: `/tmp/crepus-static-state-prefix-fix/` and
`/tmp/crepus-tui-hidden-errors-fix/` (snapshots, red/green logs and scoped patches).
No dependencies, commits or pushes. Whole-workspace, other-platform and physical
terminal checks remain unrun; title/scroll/interval evaluation fallbacks remain
separate TUI audit candidates.


### TUI title and consumed scroll errors return before painting

Parallel source review identified remaining eval-error-to-null fallbacks in title,
scroll-bottom and offset bindings. Two regression tests fail on the exact baseline
because malformed expressions return Ok and paint. Title and consumed scroll
properties now evaluate fallibly through the existing first-build-error path.
Scroll configuration is computed once and passed privately to container building,
removing duplicate resolution for vertical pure-text scroll elements. Existing
public types stay unchanged. Parent properties resolve before child construction.

Four buffer regressions cover all offset aliases, scroll-bottom and title errors,
unchanged sentinel cells, hidden/inactive and unconsumed-property laziness,
renderer recovery, explicit invalid/missing offset vs global offset defaults,
actual scroll row positions, and title updates/removal. Empty boxes, horizontal
pure text and non-scroll elements retain their prior lazy scroll behavior; valid
coercion/parse fallbacks remain unchanged. The full all-feature TUI suite passes
304 tests; a subsequently added title regression passes in the final four-test
property suite, yielding 305 distinct scoped checks. Final strict all-target/
all-feature Clippy, scoped rustfmt, diff checks, independent production review and
primary actual diff inspection pass. An initial mid-edit compile exposed a closure
error-type inference issue, corrected before the controlled baseline and green runs.
Evidence: `/tmp/crepus-tui-properties-errors-fix/`.
No dependencies, commits or pushes. Physical terminal, other-platform and whole-
workspace gates remain unrun. Slot rotation interval's explicitly documented
fallback-on-failure contract remains unchanged and needs separate consideration.


### Generated script bindings use valid, non-output banners

Parallel plugin review reproduced a shared banner defect: Ruby output starts with
C-style comments and fails ruby -c; PHP's banner precedes its opening tag and is
printed (including its header path) on import. The generator now uses hash comments
for Python/Ruby/Nim, keeps Ruby's existing frozen-string directive first, and places
PHP banners inside the opening tag. A missing PHP opening tag fails explicitly.
Checked-in Ruby/PHP outputs were updated only at their banners; ABI bodies remain
unchanged. Nim is reachable through a custom manifest, although absent from the
default plugin manifest. Other language markers remain unchanged.

A freshly generated real-header fixture fails on baseline Ruby syntax and passes
final Ruby syntax, PHP syntax, silent PHP require and Nim marker assertions.
Checked-in Ruby/PHP syntax and silent PHP import also pass. All ten bindgen tests
pass, including generated Rust and C# compiler/signature checks. Strict all-target
Clippy, scoped rustfmt, diff checks, primary actual scoped diff inspection and
independent production review pass. Evidence: `/tmp/crepus-bindgen-banner-fix/`
and `/tmp/crepus-bindgen-comment-review/`.

An exploratory full Nim check exposed existing generated pointer-type syntax/type
errors beyond the banner (log final-script.log); this is a recorded separate backend
failure, not a passing Nim acceptance claim. Final tests verify only its marker.
Ruby/PHP checks require their installed runtimes; ABI loading/calls were not exercised
in this change. Runtime hot-reload scheduling review found no new concrete bug.
No dependencies, commits or pushes. Whole-workspace and other-platform gates remain
unrun. The prior TUI interval fallback contract remains unchanged.


### Typed Nim ABI generation replaces malformed optional backend output

The previous Nim failure was more than comments: equilibrium's imports route
omitted constructor/callback functions, mapped int32_t statuses to pointers,
retained return stars in function names, emitted unescaped keyword parameters,
and did not export declarations. All three initial Nim regressions fail against
the exact previous generator. The Nim route now reuses bounded shared header
preparation and equilibrium's public typed Rust binding generator, then lowers
its syntax tree with the existing workspace syn parser. This avoids a second C
parser or fragile substitutions. syn is now a direct bindgen dependency; its
already locked MIT/Apache-2.0 license was checked locally. No version change or
new lockfile package was needed; the scoped lock change adds only that dependency.

The adapter exports typed opaque session pointers, nullable cdecl callbacks,
correct status/string/pointer/scalar declarations and escaped procedure/parameter
names from the selected header. Unknown declarations/types, arrays, variadics and
opaque sessions by value fail explicitly before that file is written. Shared
session/callback typedef validation and header admission limits remain intact.
Raw Nim importc prototypes avoid C nested-const callback incompatibilities found
by an actual Clang compile; no caller cast is needed and C ABI pointer identity
and widths are preserved. No hand-coded function-name list drives production.

All fourteen bindgen tests pass. Four Nim cases check all thirteen current ABI
signatures from a separate importing module, compile full signature assignments
and direct calls to C, preserve an extra scalar/pointer function from an overridden
header, reject unsupported declarations without output, and execute callbacks
against an independent C stub that includes the authoritative header. That runtime
check verifies payload, userdata identity, callback count, nil clearing, string
free and session cleanup using freshly generated bindings. Strict all-target
Clippy, scoped formatting, diff checks, independent review and primary actual
scoped diff inspection pass. Evidence: `/tmp/crepus-nim-bindgen-fix/` and earlier
source diagnosis `/tmp/crepus-nim-bindgen-review/`.
The C-stub runtime is not a run of the Rust ABI implementation. Nim remains a
custom-manifest generator backend, not a new default plugin package. Whole-
workspace and other-platform gates remain unrun. No commits or pushes.


### Generated Nim consumer passes against the actual current Rust ABI

A new opt-in ignored integration test closes the previous real-runtime gap.
Primary freshly built crepuscularity-abi locally, then generated Nim bindings,
compiled an importing executable linked to that exact dylib with its parent rpath,
and executed it successfully on macOS. It exercises all thirteen current exports:
context replacement/patch and bind-event rerenders, typed callbacks and userdata,
nil callback clearing, file bundles, named component selection/clear/reset,
invalid context/template and nil-session signed statuses, session/global error
consumption, owned string cleanup and session destruction. Borrowed callback
payloads are copied without freeing or reentering the session. A reserved Nim
identifier in the initial authored consumer fixture was renamed before the passing
run; no adapter production change was required.

The standard bindgen suite passes fourteen tests with this new runtime gate
ignored; its explicit run passes separately (fifteen distinct checks). Strict
all-target Clippy, scoped rustfmt, diff checks, independent review and primary
actual scoped diff inspection pass. The README records fresh-build and explicit
ignored-test commands. Evidence: `/tmp/crepus-nim-bindgen-fix/abi-build.log`,
`real-abi.log`, `real-abi-full-tests.log`, `real-abi-clippy.log`,
`real-abi-scoped.patch`, `real-abi-verification.json` (tested dylib/test hashes).
This proves the current Native IR ABI on macOS, not OS host rendering or another
platform. No commits, pushes or deployments.

Parallel optional-backend review reproduced D/Odin/Hare omissions of constructors
and callbacks, pointer-valued statuses and mangled pointer-return declarations
through the locked public equilibrium imports API. D syntax alone passes, but a
signed-status consumer fails compilation; Odin rejects its missing package header.
Hare compiler is unavailable. These remain concrete open backend repairs, not
passing targets. Evidence: `/tmp/crepus-extra-bindgen-review/`. A future D/Odin
adapter can reuse Nim's typed Rust/syn front end without inventing another C parser.


### D bindings preserve the full typed ABI

Four new D regressions fail against the old imports generator: missing constructor
and callback, pointer-valued statuses, incorrect pointer return types and silent
unsupported output. A dedicated D emitter now consumes the same authoritative
Rust/syn declarations as Nim. Only that existing frontend was extracted into
header::typed_declarations; no new generic IR, parser or dependency was added.
The D module exports opaque sessions and extern(C) callback/function declarations,
correct 32-bit statuses and const/mutable pointer types. Parameter names are
omitted because they are not ABI data and may be D keywords. Unsupported types,
arrays, variadics and opaque values fail before the binding file is written.
Rust and C# generation remain unchanged. No default D plugin package was added.

The native tests compile exact importing-module types for all thirteen exports,
a fourteenth scalar/pointer function from an overridden header and rejected-header
cases. An independently compiled authoritative-header C stub exercises callbacks,
payload/userdata identity, null clearing and cleanup using generated D imports.
A new explicit ignored gate links the current Rust ABI dylib and exercises all
thirteen exports, state changes, files/components, signed errors/error consumption,
callback clearing and owned string cleanup. It passes on macOS. Nim's actual Rust
ABI gate also still passes after the frontend extraction.

Final standard bindgen suite passes eighteen tests (two real-library gates ignored);
explicit D and Nim gates add two distinct passing checks, totaling twenty. The ABI
build is current, strict all-target Clippy passes, and scoped formatting, diff
checks, primary actual scoped diff inspection and independent source/test review
pass. README records custom-manifest D behavior and rerunnable compiler/runtime
gates. Evidence: `/tmp/crepus-d-bindgen-fix/` (exact baseline failures, final tests,
real-abi/nim-real-abi logs, snapshots, scoped patch and verification hashes).
No commits or pushes. This verifies Native IR ABI behavior on macOS, not native
GUI/device rendering or other platforms. Odin/Hare generator defects remain open.


### Odin bindings use complete typed C declarations

All four new Odin regressions fail against the exact old generator. A dedicated
Odin emitter now reuses the bounded typed Rust/syn frontend, writes a valid package
and system ABI-library import, and emits all current typed C procedures/callbacks,
session pointers, cstrings/raw pointers and signed i32 statuses. Generated parameter
names avoid source-language keywords. Unsupported declarations/types, arrays,
variadics and opaque values fail before file output. Generated comments retain
string ownership, borrowed/read-only callback lifetime, synchronization and
no-same-session-reentry restrictions. No new dependency or default Odin package.

Final twenty-two standard bindgen tests pass, including exact Odin importing-module
signatures for all thirteen exports, a selected-header extra function, no-output
rejection cases and a callback runtime across an independently compiled real-header
C stub. The explicit real Rust ABI gate also passes on macOS, exercising all
thirteen exports, state changes, callback payload/userdata/nil clearing, errors,
file bundles/components and owned-string cleanup (twenty-three distinct checks).
The fixture reads borrowed string views before freeing allocations. Its real
library gate checks the conventional library basename used by the generated
system import. Strict all-target Clippy, scoped formatting, diff checks, primary
actual diff inspection and independent production/test review pass.

Native linking exposed two installed-tool issues: Odin's unquoted Xcode Beta SDK
path and a PATH archive tool producing misaligned Mach-O archives. Tests now select
the installed Command Line Tools SDK only in the Odin compiler child and use Apple's
xcrun libtool for the temporary macOS C-stub archive. No global developer setting,
installed runtime or tool was modified. Initial diagnostic logs remain in evidence.
Authorized Cargo cleanup removed 2312 regenerable web build files / 674.6 MiB before
verification. Source and installed runtimes were preserved. Evidence and hashes:
`/tmp/crepus-odin-bindgen-fix/` (red/green/full-tests/real-abi/clippy logs, snapshots,
primary scoped patch and verification.json). README records rerunnable gates and
cstring lifetime constraints. No commits or pushes.
This verifies Native IR ABI behavior on macOS; GUI/device, other-platform and
whole-workspace gates remain unrun. Hare's optional generator defects remain open.

### 2026-10-18: Shared ABI header declaration integrity

Parallel production/test review found multiline prototypes silently omitted, split
return types emitted as void, and unnamed parameters dropped or misread. The shared
Rust/C#/Nim/D/Odin path now frames declarations after C line splicing and comment
removal, validates syntax before invoking the dependency, and checks generated
function names and arities. Ambiguous unnamed parameters, inline callback parameters,
incomplete declarations and omitted functions fail before binding output. Existing
header admission limits and session/callback validation remain. No new dependency.

Both new tests failed against the exact previous production files. Final 24 standard
tests and three explicit real Rust ABI gates (Nim, D, Odin; all thirteen exports)
pass, along with strict all-target Clippy and scoped formatting. Regressions compare
all five backends, compile generated Rust signatures, exercise UTF-8 comments and
LF/CRLF continuations, and require no output on rejected input. Primary scoped diff
and independent review completed. Evidence: `/tmp/crepus-header-integrity-fix/`.
This is bounded header framing and declaration integrity, not full preprocessing or
general C type equivalence. Whole-workspace and other-platform gates remain unrun;
Hare's optional generator defects remain open. No commits or pushes.

### 2026-10-18: Reject computed class-object keys across shared frontends

Parallel parser review found Vue, Angular and Astro interpreting `{ [name]: enabled }`
as a literal `[name]` class. A new public parser regression reproduced that AST
against the previous helper. Five lines in the shared class-object parser now reject
computed keys explicitly before quoted-key decoding; quoted `'[name]'` stays static.
Two integration tests cover three computed key forms across all three frontends and
quoted static bracket keys, including Angular's alternate class binding.

Full core suite: 669 passed; final targeted run: both tests passed after adding the
positive case (670 distinct checks combined). Strict all-target core Clippy, scoped
formatting, diff checks and independent read-only review pass. Primary inspected
the actual five-line production diff and new tests. Frontend contract updated.
Evidence: `/tmp/crepus-computed-class-key-fix/`. No commits or pushes.

Parallel source reviews initially flagged native binding_f32 fallback behavior
(withdrawn after contract revalidation below);
embedded grow-0/flex-none does not reset preceding growth. Hare's generic generator
probe still omits two ABI functions and emits incorrect types/symbols; no installed
Hare compiler is available. These findings are not fixed or runtime-verified here.
Whole-workspace, browser and other-platform checks were not run for this parser fix.

### 2026-10-18: Embedded growth reset follows class order

Both new regressions failed against the previous implementation: grow-0/flex-none
were accepted no-ops, so a reset row child retained an 80px growth allocation rather
than the plain child's 32px. The shared utility handler now resets only current
Flex1 dimensions to Auto on both axes, preserving current Fixed, Fill and Fraction
hints. Reverse class order still enables growth. No public model or dependencies
changed. The existing model cannot restore dimensions overwritten before growth;
this is a reset within that model, not full CSS flex-basis support.

All 128 embedded standard tests pass. Final enhanced targeted tests pass for
row/column bounds, painted framebuffer pixel equality, hit-test equality, reverse
order, mixed fixed/growing axes, fractions and fill. The non-default-features
style test passes; strict all-target Clippy, scoped formatting and diff checks pass.
Primary reviewed the actual production diff and tests; independent design review
found no blocker. Evidence: `/tmp/crepus-embedded-growth-reset-fix/`. No hardware
panel, MCU-target build or whole-workspace checks; no commits or pushes. Native
numeric behavior required contract revalidation (resolved below); Hare remains pending.

### 2026-10-18: Numeric fallback finding withdrawn after contract audit

Current view-ir-contract.md explicitly preserves defaults for successfully
evaluated nonnumeric/nonfinite slider, progress and meter attributes. The earlier
source-review candidate therefore was intentional compatibility, not a defect.
Production remains unchanged. Renamed the misleading reject-nonfinite test and
expanded it from a slider-only example to 126 cases: nine property slots, seven
invalid strings (including whitespace/overflow), literal and context-expression
origins. Complete generated nodes equal omitted-property baselines; the existing
expression-error propagation regression also passes.

Both numeric tests pass. Strict all-target/all-feature native Clippy, scoped
formatting, diff checks, actual primary diff review and independent test review
pass. Evidence: `/tmp/crepus-native-numeric-contract-audit/`. No full native test
suite, browser/device checks, commits or pushes. Next source-review candidate:
slot-rotate interval duplicates silently use the last binding despite the consumed
scalar duplicate contract; preserve invalid interval fallback when investigating.

### 2026-10-18: Rotating-text interval duplicates follow scalar contract

A baseline public-render regression reproduced slot-rotate interval=100
interval=200 silently selecting200, contrary to the consumed-scalar duplicate
contract. Lowering now uses the existing single-binding/string helper before
unchanged trimming/u64 parsing/default selection. Absent, valid, zero, context,
invalid, negative and overflow interval snapshots retain their behavior; expression
errors still propagate. Static/static and both static/dynamic duplicate orders
fail explicitly. No dependencies or public IR changes.

All 352 native library tests and strict all-target/all-feature Clippy pass, as do
scoped formatting, diff checks, primary actual diff review and independent review.
Initial fallback-test JSON-field assumption was corrected to inspect the public
SlotRotate enum directly; the corrected fallback test passes the baseline and only
the duplicate regression fails before the fix. Evidence and scoped patch:
`/tmp/crepus-slot-rotate-interval-fix/`. No integration/browser/device or full
workspace gate was run. No commits or pushes. Hare generator defects remain open.

### 2026-10-18: Dynamic TUI IDs propagate rendering errors

Parallel review found element_id discarding evaluator errors. The regression
reproduced an active id={1 +} rendering successfully. The helper now returns the
original error; widget construction records it through the existing before-paint
error path. Contextual focus discovery remains explicitly best-effort, while literal
ID priority, first-binding behavior and empty/missing ID handling stay unchanged.
Expanded property regressions check exact errors and unchanged sentinel buffers on
pure-text/structural elements, hidden/inactive laziness and renderer recovery.

All 305 TUI tests passed; final four enhanced property tests and strict
all-target/all-feature Clippy pass. Scoped formatting, diff checks, primary actual
diff inspection and independent source review pass. Evidence:
`/tmp/crepus-tui-id-errors-fix/`. No interactive terminal or whole-workspace gate.

### 2026-10-18: Ruby/PHP secure input HTML preserves masking

Parallel review and installed-runtime probes found both adapters dropping the IR
secure flag. Scoped single-line input branches now add type=password when secure;
plain and multiline HTML remain unchanged. Both public adapter regressions fail
against saved before files and pass after the fix; the primary reran Ruby, PHP
and syntax checks and inspected actual scoped production/test diffs. Tests use
labelled subprocess CLI stubs to isolate adapter behavior, not Rust source-parser
integration. Commands: `ruby plugins/ruby/tests/test_secure_input.rb` and
`php plugins/php/secure_input_smoke.php`. Evidence:
`/tmp/crepus-plugin-secure-input-fix/`. No browser/real-CLI integration gate.
No commits or pushes for either change. Remaining source-review candidates include
Astro raw-text script/style handling; Hare's binding generator remains open.

### 2026-10-18: Astro raw-text elements fail explicitly

The baseline parser converted `{secret}` inside an authored JavaScript string into
a context expression. CSS/JavaScript bodies shared ordinary markup parsing, and
the shared AST has no literal raw-text representation: text escapes HTML; RawHtml
evaluates and sanitizes. Rather than silently corrupt these constructs, Astro now
rejects script/style tags case-insensitively before attribute/body parsing, including
empty, self-closing and external-script forms. This intentionally tightens admission
for previously accepted unsupported tags; literal script/style support remains
unimplemented. Stylesheet links and ordinary interpolation remain supported.

The original regression fails against the saved baseline; the positive companion
passes it. All 684 current core tests and strict all-target Clippy pass. Regressions
cover JS string interpolation, JS comparison/braces, CSS, external/empty/self-closing
and mixed-case tags, plus preserved link/interpolation AST behavior. Scoped
formatting, diff checks, primary actual diff inspection and independent source/test
review pass. Frontend documentation records the exact boundary. Evidence:
`/tmp/crepus-astro-raw-text-fix/`. No browser, downstream target or whole-workspace
gate was run. No commits, pushes or dependencies added.

### 2026-10-18: Polyglot HTML adapters preserve links

Python/Ruby/PHP previously returned empty HTML for Link IR, silently erasing
destinations and child content. Scoped branches now emit escaped href/target/rel
and recursively render children. Optional null attributes are omitted; explicit
empty strings remain. Each adapter matches canonical web href screening: leading
ASCII control/space trimming, tab/CR/LF removal and case folding before rejecting
javascript/vbscript/data destinations by omitting href while preserving contents.
Existing hrefBind values remain evaluated snapshots; no live reactive claim.

All three public adapter regressions pass and fail against preserved baselines.
Primary reran Python, Ruby, PHP, both prior secure-input checks, language syntax
checks and diff checks; independently reviewed production/test scoped patches.
An independent source review found no blocker for valid View IR. Twelve native
link-filtered unit tests also pass for source lowering and native code generation.
Tests cover nested children/images, escaped Unicode/attributes, optional values,
mixed-case/control-obfuscated unsafe schemes and relative/HTTPS/mailto URLs.
Evidence: `/tmp/crepus-plugin-link-fix/` including task.patch and red/green logs.
Adapter tests use labelled CLI stubs or an IR collaborator; no actual CLI pipeline,
browser interaction or whole-plugin suite is claimed. No dependencies, commits
or pushes.

Next embedded source-review candidate: nonempty control-flow wrappers allocate
a fixed Auto main size even when three children exceed it, allowing the following
sibling to overlap child pixels. Requires bounds/framebuffer reproduction and an
intrinsic-measurement fix preserving existing one-child wrapper sizing.

### 2026-10-18: Embedded auto containers reserve descendant space

Three baseline regressions reproduce control-flow children overlapping a following
sibling: if/for/match contents, nested wrappers and wide content in a row parent.
Auto non-text containers now use intrinsic descendant extents, summed along their
layout direction and maximized across it, plus visible gaps/insets and the existing
32×16 content minimum. A single bottom-up pass temporarily stores measurements in
existing bounds until allocation consumes them; no public fields/dependencies.
This avoids repeated recursive measurement during layout. Explicit size branches,
root allocation and cross-axis stretching remain unchanged. Relative/flexible
children contribute zero intrinsic basis and resolve within actual allocation.

Full embedded suite: 132 passed. Two additional focused cases bring distinct
standard checks to134; final six integration cases cover CF/nested/cross-axis
bounds, complete framebuffer equality, hit ownership, old minima, insets, visible
gaps and relative/flex allocation. Fifty-one default and thirty no-default-feature
unit tests pass after adding repeated-layout invalidation checks to an existing
unit. Strict all-target Clippy, formatting, diff checks, primary actual diff
inspection and independent architecture/source/test review pass. Docs record the
auto sizing policy. Evidence: `/tmp/crepus-embedded-intrinsic-fix/` and worker test
snapshot `/tmp/crepus-embedded-control-flow-intrinsic-fix/`. No hardware panel, MCU
compile, timing benchmark or whole-workspace gate. Existing explicit-size overflow
and cross-axis stretching policies remain; this is not full CSS layout. No commits
or pushes. Hare bindings and broader target audits remain open.

### 2026-10-18: Reactive disposal avoids unrelated queue scans

Scheduler review found remove_node scanning both pending FIFOs even when queue
membership proved the disposed node absent. Guard each retain by the membership
set removal result; queued disposal semantics and ordering stay unchanged. The
release public-API probe excludes graph setup, batch flush and cleanup, uses three
warmups/fifteen samples, and measures unrelated disposal against pending queues.
N=1000/4000/8000 medians: 862291/12703666/47731709 ns before and
120375/582792/1578333 ns after. This removes N² comparisons for this workload;
actual queued-many disposal can still scan, and frame latency was not measured.

All 80 reactive native checks and strict all-target/all-feature Clippy pass. New
public coverage preserves batched outputs while disposing unqueued effects/memos;
existing queued-disposal cancellation coverage stays green. Formatting, actual
primary diff inspection and independent invariant review pass. WASM DOM check
passes with explicit installed rustup rustc. PATH Homebrew rustc and rustup-run
cargo both lacked the resolved target libraries; logs retain those initial failures.
No installed tool or global setting changed. Evidence:
`/tmp/crepus-reactive-disposal-fix/`; measured lesson added to .jules/bolt.md.

### 2026-10-18: Rust island scope rerenders retain authored props

Initial mount passed authored JSON props while delegated/explicit scope rerenders
passed an empty object, losing visible content. One runtime line now reuses the
existing parseIslandProps helper on rerender. The new actual-source subsystem
regression fails on baseline and covers initial mount, delegated update, explicit
editor rerender, Unicode props and ordinary scopes retaining empty props. Two
extracted-function fixtures include their helper dependency. All 98 Bun tests /
178 assertions pass in both worker and primary runs. Primary actual scoped diff
and syntax/diff checks pass. Evidence: `/tmp/crepus-web-island-props-fix/`.
The test uses happy-dom and runtime export collaborators; no real browser/WASM
application gate or universal lag guarantee. Neither change adds dependencies,
commits or pushes. Next embedded candidate: fixed-container visible overflow can
paint a child outside ancestor bounds that hit testing currently excludes.

### 2026-10-18: Embedded visible overflow remains hittable

Four of five new integration regressions and an expanded no-std-compatible unit
fail against the baseline: painting includes fixed-container overflowing children,
but hit testing pruned them at ancestor bounds. deepest_hit now prunes only hidden
or invisible subtrees and uses the node's own bounds solely for its initial hit.
Descendant traversal, unnamed-ID bubbling and later-child/root precedence retain
the existing policy; the document screen guard still clips coordinates. Opacity
zero remains hittable as before. No paint/layout changes or new dependencies.

All 139 embedded checks, thirty no-default-feature unit tests and strict all-target
Clippy pass. Tests assert actual blue/overlapping pixels, overflow bounds, hit IDs,
visibility suppression, unnamed bubbling, overlap order, half-open edges and screen
clipping. Primary inspected the actual scoped diff and tests; independent review,
formatting and diff checks pass. Docs record the visible-overflow contract.
Evidence: `/tmp/crepus-embedded-overflow-hit-fix/`. Hit testing now visits visible
subtrees outside ancestor bounds; no spatial index or latency benchmark is claimed.
No hardware/MCU or whole-workspace gate; no commits or pushes. Broader target audits
and Hare binding defects remain open.

### 2026-10-18: Pending initial Rust island renders cannot overwrite newer scopes

Actual-source happy-dom reproduction kept the island attached, rendered NEW via
rerenderScope, then resolved its older initial result and observed OLD overwrite
NEW. Private WeakMap generations now identify each element's latest render.
Initial queued dispatch and publication check their captured generation, and
explicit scope rerenders invalidate it before invoking the export. Connectivity
guards remain. Weak keys avoid retaining detached elements; other islands remain
independent. Explicit scope exports retain their synchronous-string contract.

Both new regressions fail against preserved baseline source: late attached
publication and supersession before queued export execution. Worker and primary
full Bun gates pass100 tests/185 assertions. Tests execute actual source subsystem
and verify visible DOM, skipped stale calls and independent mounts. Primary
actual scoped diff inspection, independent lifecycle review and diff checks pass.
DSL contract updated. Evidence: `/tmp/crepus-web-island-generation-fix/`. No real
browser/WASM application or CLI build was run; the asynchronous initial-export
path is tested with runtime collaborators, not claimed as a generalized async
scope API. No dependencies, commits or pushes. Broader lifecycle/target audits
and Hare's generator remain open.

### 2026-10-18: Module error replacement tears down child islands

A delayed parent-module rejection replaced its contents with diagnostics while
leaving a mounted child disposer uncalled. One call to existing disposeIslands(el)
now initiates descendant cleanup before clearing error contents. Parent identity
stays intact; existing helper removes registrations before calling them, isolates
synchronous throws/asynchronous rejections and does not await async completion.
Late-finishing child mounts retain their existing detached-element cleanup path.

All three new actual-source regressions fail baseline: normal, throwing and
rejecting child disposers. Worker and primary full Bun gates pass103 tests /
206 assertions, including existing delayed-mount cleanup checks. Tests verify
attached-at-cleanup timing, exactly-once invocation after later root replacement,
error UI and diagnostics. Primary actual production/test diff inspection, independent
lifecycle review and diff checks pass. DSL cleanup contract updated. Evidence:
`/tmp/crepus-web-island-error-cleanup-fix/`. Tests use happy-dom/module-loader
collaborators; no real browser/module-server or CLI build gate. No dependencies,
commits or pushes. Broader lifecycle/target audits and Hare bindings remain open.

### 2026-10-18: ABI event aliases share binding updates

The accepted event-name alias reported bind:count:2 through callback/result routing
but retained Count1 because binding updates inspected only handler. A public FFI
regression reproduces that mismatch. Dispatch now resolves the effective name once
(handler takes precedence over event) before binding updates and reuses it for
callback/result routing. The private helper accepts the resolved str; redundant
clones are removed. Context merge, callback/render order and pointer lifetimes stay
unchanged. README documents accepted spellings and precedence.

All ten ABI unit tests and strict all-target Clippy pass. The regression compares
raw name, JSON string, handler object, event-alias object and conflicting fields,
checking rendered state and callback/result handlers using per-session userdata.
Primary actual diff inspection, formatting, diff checks and independent source/test
review pass. Initial test type qualification was corrected before the behavioral
red reproduction; retained evidence: `/tmp/crepus-abi-event-alias-fix/`. No external
host/dynamic-library loading or whole-workspace gate. No dependencies, commits
or pushes. Remaining target audits and Hare binding defects are open.

### 2026-10-18: Shared class expressions preserve quoted operands

Vue/Angular/Astro class lowering mistook any expression starting/ending with
quotes for one string literal: 'a' + 'b' became literal class tokens rather than
ab. Complete eval_literal Str recognition now controls scalar/array literal
handling, while the original unescape helper preserves prior escape semantics.
Compound operands remain evaluator expressions, including quote-start variable
concatenations/nested arrays. Quote-start malformed array expressions still fail
explicitly with closing-quote diagnostic guidance. No dependency/public AST changes.

The new integration regression fails baseline and checks six forms across all
three frontends through actual context interpolation: scalar/array concatenation,
variable suffix, nesting, escaped quote/Unicode and carriage-return literals. All
685 core checks and strict all-target Clippy pass. Primary scoped diff inspection,
independent review, formatting and diff checks pass. Frontend contract updated.
Evidence: `/tmp/crepus-class-literal-expression-fix/`. No browser/downstream target
gate; no commits or pushes.

### 2026-10-18: Web scaffold display names serialize as TOML strings

Raw display names were substituted inside quoted manifest values, so Acme "Docs"
produced invalid TOML in both name and SEO title. Both placeholders now receive
complete TOML string literals from the existing serializer. Public relative paths
and slug/runtime crate naming remain unchanged. A private parent-directory seam
lets the existing filesystem scaffolder run in isolated temporary directories.

Primary baseline test retains that seam/test but restores old substitution; the
actual written manifest fails TOML parsing for quotes. Fixed filesystem tests
roundtrip ordinary, quoted, backslash and newline names in both fields, parse
runtime manifest and check generated files. All 138 CLI slim binary unit tests
and strict no-default-feature binary Clippy pass. Primary actual scoped diff
inspection, independent review, formatting and diff checks pass. Evidence:
`/tmp/crepus-web-scaffold-name-fix/`. No default desktop build, generated-project
WASM build or whole-workspace gate. No dependencies, commits or pushes.

### 2026-10-18: Duplicate class-object keys use their final condition

Vue/Angular/Astro lowered duplicate object keys as independent conditional classes,
so { active:true, active:false } incorrectly kept active. A per-object key index
now updates the decoded whole key's condition while preserving its first insertion
position, then splits class tokens. Distinct overlapping whole keys and separate
array objects stay independent; no global token deduplication or dependencies.

The new regression fails baseline. All 686 core checks and strict all-target Clippy
pass. Six expression forms across all three frontends evaluate actual conditional
classes, checking lastfalse/lasttrue, quoted aliases, overlap, key order and nested
array independence. Primary actual diff inspection, independent review, formatting
and diff checks pass. Frontend docs updated. Evidence:
`/tmp/crepus-class-duplicate-key-fix/`. No downstream/browser gate.

### 2026-10-18: Explicit iOS specs are honored and select their own project

Both ios generate/build previously discarded --spec and always rewrote the built-in
project. Explicit files now route through installed XcodeGen, using dump JSON to
resolve a validated project basename and generate --project . under the app root.
Generation returns that exact project path to the build; arbitrary first-project
discovery is removed, avoiding an existing-project decoy. Missing/dump/generation
failures stop rather than falling back or invoking the builder. Omitted --spec
retains built-in generation. Module/CLI docs distinguish those paths.

All three original explicit-spec tests fail against preserved baseline source.
Final five iOS CLI integration tests and138 slim CLI binary units pass, as does
strict no-default-feature binary Clippy. Unix executable stubs verify argument/CWD
routing, relative/absolute spaced paths, selectedproject versus decoy, dump errors,
malformed output/bad names, generation failure and no unintended build invocation.
Primary also ran the freshly built CLI against installed XcodeGen with a real
custom YAML/spaced path: generated Actual Selected.xcodeproj contains ActualTarget,
preserves a decoy project, and a missing-spec retry leaves project bytes unchanged.
Project path/hash and logs: `/tmp/crepus-ios-spec-fix/actual-xcodegen.json`.
Primary actual diff inspection, independent architecture/test review, formatting
and diff checks pass. No actual xcodebuild/device build, default desktop build or
whole-workspace gate. Generator failure tests assert no fallback overwrites; they
do not certify XcodeGen's own transactional-write behavior. No dependencies,
commits or pushes for either fix. Hare bindings and remaining target audits stay open.


### 2026-10-18: Tauri conversion preserves bundled main sources

Conversion copied the graph and then overwrote root main.crepus with an include
wrapper, losing an authored main entry or child and producing an include cycle.
The CLI now preserves a main entry directly. A different entry whose graph owns
root main.crepus relocates the entire graph under __crepus_source, retaining
relative paths, before writing the scaffold wrapper. Both mobile and desktop view
roots follow the same rule; wrapper paths use supported quoted-string escapes.

Primary reproduced the circular include using the existing CLI. The new regression
fails against preserved baseline source; all nine Tauri CLI integration tests and
138 slim binary units pass with the fix. Three fixture graphs check original source
bytes and actual native rendering in both view roots, including nested relative
includes and spaced paths. Strict slim binary/integration Clippy, scoped formatting,
diff checks, independent review and primary actual diff inspection pass. A rebuilt
CLI converts the original failing fixture and native ir successfully renders its
main entry. Artifacts: /tmp/crepus-tauri-main-fix/. No platform project build,
default desktop build or whole-workspace gate. No commits or pushes.

Parallel review withdrew the proposed TOML row flattening: documented loop aliases
already work and an authored value key makes flattening unsafe. JSON object rows
with an authored value key retain a separate representation ambiguity; no context
changes were made. LVGL attribute serialization has a source-confirmed candidate:
XML-forbidden characters such as NUL pass through unescaped. No executable
regression or fix yet. Native host dispatch review found no new worker routing bug;
large result parsing/commits remain a UI-thread workload limit. GPUI performance
docs now distinguish prepared HotReloadView includes from direct render snapshots.


### 2026-10-18: LVGL rejects XML-forbidden characters

A real manifest build with label A\u0000B reported success but emitted XML rejected
by Python ElementTree. The shared serializer now returns an explicit render error
for characters excluded by XML 1.0 Char, propagating through recursive nodes and
the public render APIs. Text, attributes and export names share validation;
existing escaping and permitted Unicode, including high-plane noncharacters,
remain unchanged. No dependencies or lossy replacement.

Both invalid-character regressions fail against the preserved baseline; all 59
LVGL checks and strict all-target Clippy pass. Tests cover dynamic text, input,
id, export name, forbidden C0/BMP characters, valid boundary scalars, whitespace
and markup escaping. Independent review, primary scoped diff inspection and
formatting/diff checks pass. Rebuilt slim CLI exits1 for the original invalid
manifest without overwriting an existing artifact. A valid manifest roundtrips
TAB/LF/CR, markup punctuation, CJK and supplementary Unicode through ElementTree.
Proof/logs: /tmp/crepus-lvgl-xml-char-fix/. Documentation cites the XML character
contract. No LVGL hardware/Pro import or whole-workspace gate; XML name grammar
is a separate existing boundary and is not certified by character validation.
No commits or pushes. Parallel bounded WASM/LSP review found no new verified defect;
real browser/editor sessions remain unrun.


### 2026-10-18: LVGL validates serialized attribute names

Accepted DSL div 1foo="bar" previously exported successfully to XML that
ElementTree rejects. Attribute serialization now validates full XML 1.0 Fifth
Edition NameStartChar/NameChar grammar. Valid unknown attributes, colons, Unicode
and existing hyphen-to-underscore mappings remain supported. Export options.name
remains an attribute value accepting free text. Static mapped tag names remain
unchanged. No dependencies or lossy renaming.

The new invalid-name regression fails preserved baseline; all 62 LVGL checks and
strict all-target Clippy pass. Tests cover digit/punctuation/combining starts,
Unicode ranges and supplementary cap, allowed subsequent characters, custom keys,
unknown-widget mapping and free-text export names. Independent review, primary
actual scoped diff inspection, formatting and diff checks pass. Rebuilt slim CLI
rejects the original malformed key without overwriting existing output; valid
custom/CJK keys roundtrip through ElementTree. Evidence:
/tmp/crepus-lvgl-xml-name-fix/. Docs cite the XML Name contract. Namespace binding,
LVGL schema/Pro import, hardware and whole-workspace validation remain unrun.
No commits or pushes.

Parallel review confirmed literal class braces can retain provenance using the
existing quoted expression representation across one-pass consumers; no frontend
edits yet. Python/Ruby/PHP actual adapter probes also confirmed style.formValue
input/textarea defaults are dropped despite correct fresh CLI IR. Evidence:
/tmp/crepus-plugin-input-value-review/ir.json. Those adapter fixes remain pending.


### 2026-10-18: Python/Ruby/PHP retain unbound input defaults

The native CLI produced correct style.formValue snapshots while all three HTML
adapters dropped input/textarea defaults. Each now emits escaped initial values
for unbound controls, retaining explicit-empty versus absent input attributes and
nonempty binding precedence (including binding key "0"). Textareas compensate
HTML initial-LF removal and encode CR after escaping, matching the existing native
emitter policy. Secure single-line controls remain password inputs. No dependencies
or new binding runtime.

All three public adapter regressions fail baseline and pass candidate. Primary ran
eight Python checks (six against actual CLI), three Ruby public adapter checks,
and three PHP smoke checks, plus syntax/diff checks and independent review. A shared
12-control IR fixture exercises defaults, empty/missing/bound controls, Unicode,
markup and LF/CR edge cases. Primary Chromium verifies defaultValue/editable value,
masking, absence of injected scripts and editing for each adapter. Separate real
CLI/context probes through public renderer AND session APIs in all three languages
also pass Chromium checks with closing-textarea/script text. Primary inspected the
rendered screenshot and scoped actual diff. Evidence, browser scripts and screenshot:
/tmp/crepus-plugin-input-value-fix/. Plugin surface docs updated. No Rust changes or
Cargo gate needed; other browser engines and full polyglot suite remain unrun.
No commits or pushes. Literal class-brace frontend preservation remains next.


### 2026-10-18: Frontend literal class braces retain provenance

Vue/Angular/Astro/JSX static class tokens and shared host class-list literal
entries/object keys previously reached renderer interpolation as raw braces,
reading context or losing authored text. A small shared parser helper now quotes
only known literal tokens containing an opening brace using the existing single
expression representation. Plain class ASTs remain unchanged. JSX and Svelte share
an explicit builder mode so Svelte authored quoted interpolation retains its
behavior. Angular double-brace interpolation, explicit expressions, indentation
interpolation and existing Crepus extension when/class directives are unchanged.
No public AST schema or dependency changes.

Both literal regressions fail preserved six-file baseline; all 689 core and 132
web checks pass. Three core tests evaluate actual interpolation/conditions across
static classes, scalar/array/object/nested class lists, absent/present context,
unclosed braces and quotes/backslashes. Two web integration tests verify actual
HTML classes and supported dynamic paths. One initial web fixture incorrectly
mixed Angular interpolation and literal text (explicitly unsupported); corrected
the fixture without altering that boundary, then reran the full web gate.
Strict all-target core/web Clippy, scoped formatting/diff checks, independent
consumer review and primary actual scoped diff inspection pass. Rebuilt slim CLI
native ir on a Vue fixture with name=CHANGED retains classes ["{name}", "plain"].
Docs describe literal versus dynamic source semantics. Evidence:
/tmp/crepus-class-brace-literal-fix/. GPUI/terminal/embedded runtime sessions,
WASM/browser and full-workspace gates remain unrun; shared consumers were traced
for one-pass interpolation. No commits or pushes.


### 2026-10-18: LVGL disabled preserves typed presence

Generic string conversion emitted disabled="" for null/missing and disabled="false"
for the present string "false". LVGL 9.3 lv_xml_obj_apply consumes disabled through
lv_xml_to_bool, whose only false spelling is "false", reversing both cases. The
existing hidden normalization now also handles disabled: Bool(false)/Null false,
all other present values true, emitted as canonical boolean strings. Canonical
lowercase keys retain last-binding ordering through existing attribute dedup.

Both regressions fail baseline; all 64 LVGL checks and strict all-target Clippy
pass. New tests cover all six frontends, bare/dynamic presence, missing/null/false,
string false/empty/zero/true, mixed-case ordering, original errors and inactive
branch laziness. Primary inspected the actual scoped diff; independent review,
formatting and diff checks pass. Rebuilt slim CLI manifest probe parses XML for
seven cases and confirms false,false,false,true,true,true,true. Official versioned
LVGL converter/object parser sources verified; docs link the contract. Evidence:
/tmp/crepus-lvgl-disabled-fix/. No hardware/Pro import or full-workspace gate.
No commits or pushes.

Parallel picker review found no selected-value snapshot defect: IR carries a bind
path and explicitly rejects authored selection defaults. Actual CLI static-ID
brace checks across Vue/Astro/JSX/Angular preserve literal IDs already; no ID edits.
Evidence: /tmp/crepus-id-brace-audit/results.json.


### 2026-10-18: TypeScript/Bun input parity

Public TS adapter probes using the actual CLI received secure/formValue IR but
rendered empty unmasked inputs. Its input branch now preserves unbound initial
values, explicit-empty versus missing attributes, binding precedence (including
"0"), single-line password masking and multiline textarea LF/CR parsing semantics.
The shared 12-control fixture remains unchanged; the new test additionally marks
a multiline fixture secure to assert textarea precedence. No dependencies or
process-execution changes.

The public regression fails baseline; all 11 scoped Bun plugin tests with 19
assertions pass in worker and primary. Primary Chromium checks both fixture output
and real CLI/context output through renderHtml and session APIs: defaultValue,
normalized editable values, secure masking, escaped closing-textarea/script text,
no script nodes and editing all pass. Primary inspected the rendered screenshot,
actual scoped diff and independent review; diff checks pass. Evidence:
/tmp/crepus-typescript-input-fix/. An initial broader Bun path filter also included
editors/zed/grammars/crepus/plugins/typescript-bun/crepuscularity.test.ts, whose old
IR-version4 expectation fails against current version7; left that unrelated copy
unchanged. Scoped ./plugins/typescript-bun passes. Other engines, dedicated typecheck
and full-polyglot gate remain unrun. No Rust changes or Cargo gate needed.
No commits or pushes.

Primary separately reproduced event-loop blocking in renderIr: its async function
uses spawnSync; a100ms timer remains unfired after an intentionally delayed fakeCLI
returns (~606ms observed). Evidence /tmp/crepus-typescript-render-yield/proof.json.
That confirmed scheduling fix is next, distinct from input serialization. Embedded
visibility/hit review found no new contract-backed defect; firmware click dispatch
is explicitly outside the existing target contract.


### 2026-10-18: Bun adapter CLI rendering yields to the event loop

Async renderIr awaited file loading but then used spawnSync, blocking host timers
through CLI execution. It now uses Bun.spawn with owned Blob stdin and direct
argv, concurrently reading stdout/stderr and awaiting child exit. Filename,
baseDir, context and no-shell command boundaries stay unchanged. Nonzero exits
retain stderr errors, missing executables retain ENOENT, and successful malformed
JSON retains SyntaxError. No new dependencies or cancellation API.

The gated-child regression fails baseline: only a parent event-loop callback can
release the child, which times out with synchronous execution. All 15 scoped Bun
plugin tests with 26 assertions pass in worker and primary, including actual CLI
render/session and prior input tests. New tests additionally cover large UTF-8
stdin and exact envelope/argv, concurrent large stdout/stderr, nonzero diagnostic
with output pressure and malformed JSON. Primary reran the original timer probe:
100ms timer unfired before render completed on baseline (~606ms); fired before
completion with the fix (~365ms). These observations prove yielding, not a latency
benchmark. Independent review, primary actual scoped diff inspection and diff
checks pass. Docs distinguish asynchronous compilation from buffered results and
main-thread JSON/HTML work. Evidence: /tmp/crepus-typescript-render-yield-fix/.
No Cargo gate needed; no Rust changes. Cancellation, output streaming to callers,
large-result JSON/HTML scheduling, other OS/runtime versions and whole-workspace
checks remain outside this fix. No commits or pushes.


### 2026-10-18: TypeScript/Bun renders Link nodes

The actual CLI emitted a Link tree while public TS renderHtml returned an empty
string. Added the missing switch arm using existing recursive children and escaping,
with the same safe-href policy as Python/Ruby/PHP: leading C0-space normalization,
global TAB/LF/CR removal for scheme checks, ASCII scheme casefold, and omission of
javascript/vbscript/data hrefs. Safe original values and optional empty target/rel
attributes remain intact; hrefBind remains an evaluated snapshot. No dependencies,
schema changes or invented browser binding runtime.

Both new public tests fail baseline and pass candidate, covering adversarial IR
and actual CLI output through renderHtml and session APIs. All 17 scoped Bun tests
with31 assertions pass in primary. Chromium checks escaped attribute bytes,
recursive text/button children, no href on unsafe links, literal entity spellings,
no script elements and actual CLI hash-link click navigation. Primary inspected
rendered screenshot and actual scoped diff; independent review and diff checks pass.
README documents the added support. Evidence: /tmp/crepus-typescript-link-fix/.
No Rust changes or Cargo gate needed; other browser engines/typecheck/full-polyglot
checks remain unrun. No commits or pushes.

Parallel async-session review found no internal commit-order bug: APIs return IR
without committing a stored view; concurrent callers own result application order.
LVGL direct checked still has a candidate Boolean-coercion mismatch: null/missing
emit empty strings, numeric0 emits "0", which LVGL interprets true. Existing bind
Bool snapshots are correct. Direct-state coercion policy needs comparison with
custom control semantics before editing; no checked-state changes made this turn.
Evidence: /tmp/crepus-lvgl-checked-audit/result.xml.


### 2026-10-18: LVGL custom checked states match native coercion

Direct checked snapshots on custom checkbox/switch previously passed through
string conversion: null/missing became empty and zero became "0", both true in
LVGL. A scoped branch now evaluates once and follows native custom-control
coercion: Null false; trimmed text empty/true/1/yes/on true; other spellings false.
It emits explicit Boolean strings under canonical checked keys, retaining the
existing ordered last-attribute rule. HTML presence semantics and other tags are
unchanged; canonical bind remains strict Boolean. Docs explain the distinction.

All three regressions fail baseline; all 67 LVGL checks and strict all-target
Clippy pass. Tests cover both controls, scalar/coercion matrix, all six frontend
dynamic paths, bare/missing state, binding/repeated-attribute order, original
expression errors, inactive-branch laziness, strict bind rejection and generic
button attribute scope. Independent review, primary actual scoped diff inspection,
formatting and diff checks pass. Rebuilt slim CLI manifest XML and native IR match
across 28 authored cases (13 values per control plus two bare controls). Evidence:
/tmp/crepus-lvgl-checked-fix/actual/proof.json and adjacent logs/snapshots.
Primary added a why comment after gates; no behavior changed afterward. No physical
LVGL/Pro import or full-workspace gate. No dependencies, commits or pushes.


### 2026-10-18: Web void children fail explicitly

JSX paired void tags retained children in AST, but HTML returned before rendering
them, silently losing content. A shared check now rejects all nonempty child trees
on HTML void elements before evaluation in the main plain/SSR walker and legacy
hydration renderer. Void tag classification is case-insensitive. Empty paired and
self-closing voids, ordinary elements and inactive outer branches retain support.

All three new regressions fail baseline. Final web hydration-enabled gate passes
159 checks; strict all-target hydration Clippy, formatting/diff checks and
independent review pass. Tests cover indent/JSX, direct AST malformed expressions,
declaration/control children, all public modes and mixed-case tags. The broader
gate exposed a prior class-literal fixture expecting quoted JSX interpolation;
confirmed its failure on pre-void source, then changed only that fixture to explicit
className={dynamic}, retaining its filtering assertions. Primary inspected scoped
diffs. Rebuilt CLI now rejects the original img/span silent-drop reproduction.
DSL docs updated. Evidence: /tmp/crepus-web-void-child-fix/. No browser/WASM or
full-workspace gate; no commits or pushes.

### 2026-10-18: Web bundle output checks actual bytes

The historical DriverCache fingerprint described desired source, not current output.
A file existing at crepus-bundle.json could therefore remain stale after replacement
or when using another output directory. The private writer now compares actual
destination bytes before skipping. Removed this unnecessary cache path and renamed
its private helper/callers; existing cache files remain untouched. Identical output
still avoids writes.

Two artifact-repair tests fail baseline; the unchanged-mtime case passes. All 141
slim CLI binary tests and strict binary Clippy pass. Tests cover replaced output,
independent output directories and exact timestamp preservation without sleeps.
Primary actual scoped diff inspection, independent review, formatting/diff checks
pass. Rebuilt CLI controlled probe warms output, replaces it, repairs it, then
preserves a historical timestamp on identical bytes. An intentional app.js-directory
error stops every probe before WASM compilation, with CARGO=false as an additional
guard; this is not a full site-build success claim. CLI docs updated. Evidence:
/tmp/crepus-web-output-cache-fix/actual/proof.json and adjacent logs/snapshots.
No atomic-publication or multi-target transaction claim; no commits or pushes.

Parallel Go public RenderHTML/current CLI IRv7 probe confirms dropped password
masking and style.formValue; implementation remains pending. Evidence:
/tmp/crepus-go-input-review/current-ir.json and current-go-html.txt. Current web
--emit help also still calls generated source output a stub; documentation wording
can be corrected separately without changing emit behavior.


### 2026-10-18: Go input defaults and password masking

Actual current CLI IRv7 contained secure/formValue snapshots while public Go
RenderHTML emitted empty unmasked controls. Its input branch now preserves unbound
initial values, explicit-empty versus missing, binding precedence (including "0"),
single-line password masking and textarea LF/CR parsing compensation. Optional
bind/placeholder fields no longer stringify nil as <nil>. No API, dependency,
manifest or unrelated generated-file changes.

Both new regressions fail baseline. Full Go module tests pass in worker; primary
runs go vet and go test -race -count=1 ./... successfully against current absolute
CLI. Tests use shared12-control fixture with only Goquoteentity spelling adjusted,
extra missing/nil metadata and securemultiline cases, and real public CLI/session
output with Unicode, escaping and bindings. Primary Chromium checks real CLI/public
renderer+session output defaultValue and normalized editable value, password type,
no injected script, and editing; rendered screenshot inspected. Independent review,
primary actual scoped diff inspection, gofmt and diff checks pass. golangci-lint
is unavailable (commandnotfound), so that check remains missing. Evidence:
/tmp/crepus-go-input-fix/. Other browser engines and full polyglot/workspace gates
remain unrun; no Rust changes/Cargo gate needed. Docs updated. No commits or pushes.


### 2026-10-18: Framework emits honor manifest target selection

web build accepted --manifest/--target but dropped both before framework emission,
compiling caller index.crepus instead. Framework resolution now delegates the shared
HTML argument resolver. An explicit manifest selects the target; explicit site,
entry and output override selected fields. Site alone considers only its own local
manifest. Invalid/missing explicit manifests, unknown selectors and ambiguity error
before output. Already-resolved root target metadata bypasses local reselection.
Framework routes remain independent of runtime/Cargo.toml. Malformed autodiscovered
local manifests still retain the existing optional-loader fallback; strict error
claims concern explicit manifests/selectors.

All four new CLI regressions fail baseline. Final 142 slim CLI binary units and
92 integration checks pass: four new manifest cases plus existing web, module,
Svelte module, SFC script, output failure and source-collision suites. Initial two
WASM integration failures were PATH rustc missing wasm32 core; reran the built
integration executable with scoped Rustup RUSTC and one test thread. Both actual
docs-site/example-site builds then compiled WASM and generated bindgen packages;
related ambiguity test also passes. No global toolchain modifications. Strict slim
binary/new-integration Clippy, formatting/diff checks, independent review and primary
actual scoped diff inspection pass. Original React wrong-caller reproduction now
selects Expected chosen source. Help no longer calls framework source outputs stubs.
CLI docs explain precedence. Evidence: /tmp/crepus-framework-manifest-fix/.
No deployment/host-framework execution or full-workspace gate. No commits or pushes.

### 2026-10-18: Go renders Link nodes

Actual CLI Link IR and visible child text were dropped by Go HTML rendering. Added
the missing branch with existing recursive child rendering and stdlib HTML escaping.
Optional target/rel preserve empty values without nil text; hrefBind remains a
snapshot. Safe-href normalization matches the other adapters exactly: leading
ASCII C0-space trim, TAB/LF/CR removal, ASCII scheme casefold; javascript/vbscript/
data omit href without dropping child content. No dependencies or API changes.

Three regressions fail baseline; full Go tests pass in worker. Primary go vet and
go test -race -count=1 pass against current CLI. Tests cover nested/escaped content,
optional metadata, obfuscated unsafe schemes, safe relative/HTTPS/mailto and entity
spellings, plus actual CLI/public renderer+session routes. Primary Chromium checks
inert unsafe hrefs, escaped fields and actual CLI/session hash-link navigation;
rendered screenshot and actual scoped diff inspected. Independent review, gofmt
and diff checks pass. README support matrix updated. Evidence:
/tmp/crepus-go-link-fix/. golangci-lint remains unavailable; other browser engines
and full-polyglot checks unrun. No Rust changes for this adapter; no commits/pushes.


### 2026-10-18: Explicit-site local manifest errors stop builds

A malformed local crepus.toml under explicit --site was swallowed by the optional
loader, causing successful output from fallback index.crepus instead of configured
source. Added a fallible loader reusing existing read/parse/web-target conversion;
only the explicit-site local branch opts into errors. The Option wrapper preserves
legacy consumers and autodiscovery policy. Genuinely absent files and valid configs
without web targets retain defaults. A NotFound read checks symlink metadata so a
dangling authored config is not treated as absence. Resolved metadata bypass remains.

The new regression fails baseline. All 143 slim CLI units and six manifest CLI
integration tests pass, plus strict binary/integration Clippy. Failure matrix covers
syntax, directory configs, invalid UTF-8 and dangling symlinks against absent,
existing sentinel and regular-file output states. Positive missing/non-web configs,
selectors and already-resolved metadata stay covered. Rebuilt CLI original malformed
site repro now exits1 before output in both React and HTML routes. Independent
review, primary actual scoped diff inspection, formatting/diff checks pass. CLI docs
updated. Evidence: /tmp/crepus-local-manifest-fix/. No full WASM/site or workspace
gate rerun for this loader-only change. No dependencies, commits or pushes.

Parallel Angular audit confirmed attr.* property conflation: false Boolean attr
values are dropped, and scalar null values remain empty attributes. Angular uses
attribute string conversion and null removal (angular.dev/guide/templates/binding).
A nullable-string-only fix is incomplete: evaluator lacks ternary and provenance
also affects scalar omission and control-name routing. Full fix needs preserved
attribute provenance across consumers, not newly rejecting the admitted feature.
No Angular behavior changes yet. Evidence:
/tmp/crepus-angular-attr-bool-audit/Entry.ng. C# input branch also remains a likely
parity gap from source inspection; no executable C# proof or edits this turn.


### 2026-10-18: Parallel module review pass

Seven bounded review lanes covered CLI/Tauri/webext, shared core/evaluation/reactive/
WASM/macros/LSP, native/ABI, TUI/embedded/LVGL/GPUI, plugins/Flutter, shared parser
frontends, and authored web-module compilation. These are scoped reviews, not an
exhaustive all-target certification. No production code changed in this pass.

Findings queued for focused implementation and regression checks:

- Tauri Android metadata inserts the display title into XML without attribute
  escaping. Actual CLI conversion of `AT&T "Docs"` succeeds with invalid XML.
  Primary inspected the substitution; evidence: /tmp/crepus-tauri-metadata-review/.
- Shared Svelte quoted classes split interpolation expressions on whitespace.
  Primary reproduced `class="w-{1 + 2}"` failing public CLI rendering with
  `unclosed interpolation placeholder`. Reuse the existing interpolation-aware
  class splitter for this path; evidence: /tmp/crepus-svelte-class-space-audit/.
- Authored Svelte rune/prop/snippet callback receiver walkers omit the TypeScript
  instantiation wrapper. Worker compilation of `(fn<number>)()` yields a cell
  method call; strict TypeScript alone passes. Runtime regression still required.
  Evidence: /tmp/crepus-instantiation-receiver-review/.
- Python HTML rendering drops inputType/formName/required, reproduced through the
  public renderer with mocked IR transport. TypeScript has the same source-level
  omission; Go/Swift searches suggest broader parity work. Actual CLI-to-browser
  form validation/submission verification remains required.
- Previously confirmed Angular attr.* provenance remains open. A shared AST origin
  distinction must reach HTML null/Boolean serialization, textarea handling, and
  native control routing; a Boolean-only patch would be incomplete.

Core/native/terminal review lanes reported no additional confirmed defect. No Cargo,
Gradle/device, browser, fuzzing, or full workspace gates ran for this review-only
pass. Existing target limitations and dirty concurrent changes were preserved.
No commits or pushes.


### 2026-10-18: Tauri XML labels and Svelte class-expression whitespace

Fixed two findings from the parallel pass. Android label metadata now uses the
existing escaping helper before XML substitution, retaining window-title/product
precedence and default labels. Actual rebuilt CLI conversion of `AT&T "Docs"` now
roundtrips through ElementTree with the exact label. Regression cases cover all
five reserved characters, Unicode, title precedence, plain names and defaults.
This addresses reserved-character escaping; XML-forbidden controls and attribute
whitespace normalization are not covered by the HTML escape helper.

Svelte quoted class lists now reuse the existing interpolation-aware splitter via
a crate-private reexport. JSX/Vue/Angular/Astro literal-class handling is unchanged.
New core regressions cover arithmetic whitespace, quoted braces, Unicode class
separators and dynamic/static compatibility; web coverage proves final HTML.
Actual rebuilt CLI renders the original failing file as `<div class="w-3">OK</div>`.
Frontend documentation updated.

Primary guarded production-file baseline swaps make both new regressions fail;
candidate restoration is unconditional. Candidate passes all 691 core, 160 web
(with hydration), 143 slim CLI unit and 10 Tauri integration checks. Strict Clippy
passes core/web all-targets and slim CLI binary/Tauri integration targets. Formatting,
scoped diff inspection and independent reviews pass. Evidence:
/tmp/crepus-tauri-xml-fix/ and /tmp/crepus-svelte-class-space-fix/. No dependencies,
commits or pushes. Full workspace, WASM/browser and Android device/build checks
were not rerun for these changes. Angular provenance, generic callback receivers
and plugin form metadata remain queued from the parallel review.


### 2026-10-18: Generic Svelte callback receivers

Authored TypeScript instantiation expressions around rune, prop and snippet
callbacks were not traversed by their unbound-call walkers, so `(fn<number>)()`
called a synthetic cell method and changed `this`. Added the missing transparent
wrapper to those walkers. The same root affected scalar template-const callbacks;
that path now wraps the full callee with a hygienic typed identity helper, replacing
comma/tag wrappers and preserving initializer edit folding, optional calls, type
arguments and actual member receivers. Distinct each-binding/effect-statement
walkers are unchanged. Frontend docs updated. No dependencies added.

Primary production-only guarded baseline swaps fail all four new regressions.
Candidate passes 768 native units and 20 selected native integration checks, plus
28 CLI Svelte/module integration checks. Two first-run failures were stale test
expectations: SSR prop snapshot cells also need unbound calls, and one old const
assertion still expected the comma wrapper. Expectations corrected with existing
aggregate-member assertions retained. Selected native/CLI strict Clippy and
formatting/diff checks pass; independent scoped diff review found no blockers.

Actual current CLI compiled a typed fixture spanning all four callback categories,
plain/optional/tagged calls, authored casts and method controls. DOM and SSR runtime
checks pass both before and after Bun production minification. Strict TypeScript
passes with explicit prop callback types and installed Bun platform declarations.
Chromium executes 14 receiver assertions with no page errors; rendered screenshot
inspected. Evidence: /tmp/crepus-instantiation-receiver-fix/ and
/tmp/crepus-instantiation-receiver-review/. No full workspace, other-browser or
source-framework-wide parity claim; no commits or pushes.

Parallel form-metadata investigation narrows the next adapter fix to existing
Input renderers in Python, Bun/TypeScript, Go, Ruby and PHP. Their shared native
contract carries inputType (14 admitted types), formName (including empty), and
Boolean required; secure overrides type and multiline omits type. Python public
mocked-transport rendering reproduces the omission; the other four remain
source-confirmed pending executable regression checks. C# has broader existing
input-default/secure gaps; Swift/Rust lack Input support, while Java/Kotlin delegate
to CLI rendering and are outside this omission. No adapter edits this turn.


### 2026-10-18: Input metadata in five HTML language adapters

Python, Ruby, PHP, Go and TypeScript/Bun Input rendering now honors the native
14-type allowlist, escaped formName (including explicit empty strings) and strictly
Boolean required. Secure single-line inputs override the type to password;
multiline retains textarea semantics. Bound/default-value precedence and LF/CR
textarea compensation remain unchanged. No dependencies or public API changes.

Added shared26-case input-metadata IR fixture plus focused adapter regressions.
Isolated exact-before renderer copies fail each new regression; candidate passes
all five. Primary independently runs nine Python checks, 18 Bun checks/33
assertions, ten Ruby checks/14 assertions, full Go race suite and vet, and all
seven PHP smoke scripts plus PHP syntax checks. Ruby's first broad run used the
plugin cwd; its preexisting path test requires repository cwd. Corrected invocation
passes without source changes. golangci-lint remains unavailable.

Real current CLI input IR and both public renderer/session APIs are verified in
all five languages. This exposed a separate PHP empty-context bug: default []
encoded as a JSON array and current CLI rejected it. Casting only the top-level
context to an object now emits {}, preserves associative fields and nested lists;
new stub-envelope regression fails baseline and passes candidate. Primary real
CLI default session now succeeds.

Chromium checks all26 rendered fixture controls across every adapter, including
14types, null/invalid fields, escaping and textarea newline fidelity. Real CLI
forms reject invalid email/missing required data; editing then submission preserves
all five named fields, optional empties and secure/textarea values. Rendered
screenshot inspected. Independent cross-adapter scoped reviews and primary
actual diffs approve; formatting/syntax/diff checks pass. README updated. Evidence:
/tmp/crepus-input-metadata-fix/. No Rust edits/builds, full workspace or other
browser checks; C# broader input gaps and Swift/Rust missing Input support remain
separate. Angular attribute provenance remains open. No commits or pushes.


### 2026-10-18: C# Input renderer parity

C# Input rendering now preserves the same defaults and metadata as the five
previously repaired HTML adapters: native14 input types, secure password
override, multiline textarea LF/CR compensation, bound/default precedence,
escaped names including empty strings and strictly Boolean required. Missing
optional fields/style are safe; unsupported input types omit type. No ABI,
dependency or public API changes.

Added --input-html mode to the existing C# smoke executable, exercising all38
shared defaults/metadata cases through actual public HTML/session APIs and a
labelled Unix/Python CLI transport stub. The exact-before production renderer
fails those new checks; candidate passes. Primary no-restore warnings-as-errors
smoke-project and solution builds succeed with zero warnings, and existing CLI
smoke (context, frontend filename, includes and large stderr draining) passes.
Independent source review and primary actual scoped diff inspection approve.

Primary separately checks the current CLI through public C# HTML/session calls.
Chromium verifies all26 metadata controls, password masking, newline fidelity,
invalid-email/required blocking, editing and successful submission of all five
named fields. Rendered screenshot inspected. Evidence:
/tmp/crepus-csharp-input-fix/. README updated. No Rust builds, full workspace,
Windows or other browser gates; no commits or pushes.

Angular design trace confirms provenance must also survive macros' private
Binding conversion. Planned shared evaluation returns absent for Attribute null
and strings for other Attribute values while preserving original expressions.
Consumer changes must cover textarea value vs attribute, class literal braces,
HTML presence state and native control-name exclusion, with duplicate policy
explicit. Map includes web/native/WASM, terminal/embedded/LVGL/GPUI/runtime and
macro paths. No Angular production edits this turn.


### 2026-10-18: Angular attribute provenance across targets

Added public BindingOrigin::Attribute and shared eval_binding. Angular attr.*
retains its original expression and provenance: null omits, other values use
shared evaluator string formatting. Expression/Literal behavior is unchanged.
Parser rejects conflicting writers for the same attribute, including mixed
property/attribute bindings and attribute class with nonempty class lists.
The enum addition requires downstream exhaustive matches to handle Attribute;
no in-tree exhaustive match was found. Frontend contract docs updated.

Shared web plain/SSR/legacy hydration serialization now preserves false-valued
Boolean attributes and omits null scalar attributes. Textarea attribute value
stays an attribute rather than replacing body content. URL filtering remains.
Slot rotation ignores attribute interval for configuration, serializes it as an
ordinary attribute, and combines literal attribute classes with its mandatory
class once without duplicate HTML attributes or interpolation.

Native IR scalar/presence helpers preserve nullable attributes; raw bind and
interval remain controls only for ordinary origins. Literal attribute classes
and hidden presence project into style, URL attributes stay snapshots, and
textarea/select/file attribute values do not become control state. Null
disabled/readonly disappear before validation (including flattened metadata);
present unsupported state still errors. TUI/embedded/LVGL support corresponding
nullable/presence/class semantics while excluding attribute control-name routing.
Runtime GPUI IDs retain null omission. Macros preserve origin internally and
explicitly reject unsupported HTML attributes before GPUI control interpretation;
no authored macro frontend route was expanded. Native/GPUI targets retain their
existing bounded HTML projection, not arbitrary DOM attribute support.

Primary old Angular-lowering baseline fails both initial web regressions; focused
regressions now cover parser origin/conflicts, all three web render modes, native
control/metadata boundaries, terminal/embedded/LVGL state and macro/runtime seams.
Passing gates: 715 core checks with two baseline failures excluded; 163 web checks
with hydration; 213 selected nonweb checks; 775 native checks with web-modules;
30 macro and 90 runtime units; 143 slim CLI units (2,129 total). Strict all-targets
Clippy passes all eight affected packages; wasm32-unknown-unknown WASM package
check passes using the installed stable Rust toolchain. Formatting, independent
reviews and primary scoped diff inspection pass.

Full core run remains red on preexisting svelte_modules tests
quoted_attributes_interpolate_while_encoded_braces_remain_literal and
unsupported_constructs_and_malformed_markup_are_errors (accepted &copy; mixed
attribute and class:active). Both fail with the original Angular builder restored;
no unrelated Svelte repair was folded into this patch. Full workspace, hardware
and actual WASM JavaScript/browser execution were not verified.

Rebuilt CLI verifies HTML, native IR, React emit and conflicting-writer errors.
Chromium checks attribute-vs-property false, null attributes, hidden state, literal
classes, textarea editing, safe links, required validation and form submission.
Rendered screenshot inspected. Evidence: /tmp/crepus-angular-attribute-fix/.
Using the user's earlier disk-cleanup authorization, removed 285 older duplicate
rebuildable Rust test executables (1,688 MiB logical size), retaining newest per
name and library/source caches; exact manifest saved with evidence. No commits,
pushes, deployments or remote changes.


### 2026-10-18: Svelte module contract checks restored

The two previously recorded core failures were obsolete negative expectations.
Authored class directives already have parser/native implementation and positive
unit coverage; mixed quoted attributes now use the shared Svelte character-reference
decoder, including copy and valid semicolonless amp. Replaced those negatives with
public parser positive assertions for retained class-helper metadata, one className
writer and exact mixed decoding. Unknown references and single-pass amp-copy decoding
are also asserted. Malformed empty/unclosed interpolation and all other unsupported
style/event/spread/module cases remain negative. No production implementation changed.
Updated only the stale mixed-attribute entity documentation paragraph; concurrent
namespace documentation edits were preserved and excluded from the scoped patch.

All 724 current core checks pass without exclusions; strict all-targets core Clippy,
formatting/diff checks and independent review pass. Initial full run hit ENOSPC
before tests executed; a retry after free space returned passes. No additional cache
deletion was performed this turn. Public authored DOM/SSR compilation and runtime
checks confirm class toggles plus copy/amp/unknown/single-pass attribute behavior.
Minified Chromium output toggles class and attribute values twice; rendered active
screenshot inspected. Evidence: /tmp/crepus-svelte-contract-checks/. Full workspace
and other-browser checks were not rerun for this test/documentation change.
No dependencies, commits or pushes.


### 2026-10-18: C# Links and inert native attribute default aliases

C# HTML rendering now handles Link nodes with existing recursive child rendering
and HTML escaping. Optional target/rel preserve present empty strings; hrefBind
remains a snapshot. Scheme filtering matches the other five adapters: trim leading
ASCII C0-space, remove TAB/LF/CR for scheme recognition, fold ASCII uppercase only
and omit href for javascript/vbscript/data. Named entity spellings and non-ASCII
lookalikes remain escaped literals. No dependencies, Input or ABI behavior changes.

Public CLI-stub/session Link regression fails baseline and passes candidate, covering
recursive content, escaped/empty/absent attributes and four blocked/five allowed
URLs. Primary warnings-as-errors smoke-project/solution builds pass with zero
warnings; Input fixtures and existing real CLI smoke pass. Current CLI/public
C# HTML/session calls and Chromium verify safe hash navigation, inert unsafe links
and exact escaping/filtering. Nested-anchor fixture recursion is asserted as raw
HTML; browser structure is not assumed because HTML repairs nested anchors.
Rendered screenshot inspected. Evidence: /tmp/crepus-csharp-link-fix/.

Native Angular Attribute defaultValue/default-value/defaultChecked/default-checked
were incorrectly used as initial control state, unlike their inert HTML attribute
meaning. Tightened existing alias filters to retain only actual Attribute value
and checked (value still excludes textarea); normal-origin aliases remain unchanged.
Inert aliases no longer create false conflicts alongside actual value/checked.
Fresh CLI IR now matches browser-empty/unchecked controls. One initial test fixture
used paired input closing markup; corrected it before baseline proof. New regression
then fails baseline for the actual formValue defect and passes candidate. All802
selected native checks, strict all-targets Clippy, fresh slim CLI build, formatting
and diff checks pass. Independent native/C# reviews and primary scoped diff inspection
approve. Evidence: /tmp/crepus-angular-default-alias-fix/. README/frontend docs updated.
No full workspace, Windows or hardware gates, commits or pushes.

Parallel CLI audit confirms another queued defect: GPUI/TUI shared project-name
validation accepts Unicode alphanumerics outside Rust/Cargo XID (app² scaffolds
invalid Rust; offline no-deps Cargo metadata also rejects it). Valid Unicode 雪
works and must remain supported; blanket ASCII rejection would regress it. Shared
validator has only GPUI/TUI callers. Existing locked/cached unicode-ident is suitable
but requires a direct CLI dependency declaration; no scaffold edits yet. Evidence:
/tmp/crepus-scaffold-identifier-review/.


### 2026-10-18: Parallel module review follow-up

Six independent lanes reviewed CLI/scaffolds; core/reactive/runtime;
web/WASM/webext; native/ABI/Tauri/iOS; Flutter/language plugins; and
TUI/embedded/LVGL/GPUI. Review is bounded, not exhaustive target certification.
Primary inspected the affected source paths. Findings queued for focused repair:

- Reactive runtime `track_read` deduplicates each source with Vec::contains;
  distinct N-source observer runs perform N(N-1)/2 comparisons. Source-proven
  complexity, no timing measurement yet. Existing subscriber sets may supply
  deduplication without a second collection; lifecycle semantics need tests.
- Web app.js rerender/rerenderScope synchronously serialize and call WASM rendering
  on the browser main thread before DOM replacement. Promises do not offload CPU
  work. No latency measurement yet; worker ownership must preserve state, event
  ordering, errors and stale-result handling. DOM commits still require main-thread
  work; no impossible-to-lag guarantee is justified.
- C# ABI SetTemplate forwards embedded NUL in template/baseDir to a NUL-terminated
  native string boundary. Native probe succeeds while truncating template content;
  actual managed-call regression remains pending. Python already rejects NUL.
- Tauri metadata() reads v2 root keys only. Existing CLI conversion of valid v1
  nested productName/version/identifier succeeds with incorrect defaults.
  Reproduction: /tmp/crepus-tauri-v1-metadata-review/. Preserve v2 behavior.
- TUI virtual content height saturates at u16::MAX; source tracing suggests the
  final row of a 65,536-row one-line list cannot be reached. Runtime reproduction
  pending; do not describe this candidate as a verified rendered failure.

No review-lane builds, commits or pushes. No claim that unmentioned modules are
bug-free, or that all platforms/hardware were exercised.


Scaffold follow-up implemented: shared GPUI/TUI validation uses the already-locked
unicode-ident XID tables with existing underscore/hyphen rules, rejecting app²
before filesystem writes while preserving 雪 and combining identifiers. Pascal
conversion prefixes underscore when stripping separators exposes a combining mark.
Direct dependency declaration reuses the existing locked version; no new package
resolution. Primary inspected source/tests/manifest/lock changes. CLI 143 unit plus
six public scaffold integration checks pass. Generated projects were not built
against their remote dependencies; no full workspace/platform gate claimed.

Strict production CLI Clippy passes. All-targets Clippy is blocked by unrelated
preview.rs items_after_test_module at line89 (production items at131/136), outside
this scaffold change. No unrelated repair made. Concurrent manifest version changes
were preserved. No commits or pushes.


### 2026-10-18: Reactive tracking, Tauri v1 metadata, C# ABI strings

Replaced reactive track_read's per-read linear Vec::contains with the existing
reciprocal subscriber HashSet insertion result, retaining first-read source order
without another collection/dependency. Live observer/source checks avoid dangling
edges. Unit checks cover reciprocal order/deduplication, cleanup, source disposal
and restored dead observers. Public wide-observer test covers repeated reads,
dependency switching, equality suppression and changed values. Existing nested,
batched, panic and reentrancy suites remain green.

Seven-sample local debug median for one signal write plus wide memo read:
1,000 sources 4.05ms -> 1.24ms; 4,000 46.11ms -> 4.99ms;
8,000 180.12ms -> 9.04ms (~19.9x). This measures dependency tracking on this host,
not browser frame latency or a release-build guarantee. Manual ignored benchmark
has correctness assertions, no flaky timing threshold. Evidence:
/tmp/crepus-tracking-fix/{runtime.patch,timing.json}; full reactive all-features
suite, strict all-targets Clippy and wasm32-unknown-unknown all-features check pass.
Browser DOM tests remain target-gated and were not executed by host cargo test.

Tauri metadata uses detected version-specific paths: v1 package productName/version
and tauri.bundle.identifier; v2 root keys unchanged. Existing window-title priority
is retained. New unit test distinguishes conflicting v1/v2 locations; public CLI
conversions cover product fallback/title override, Android label/version/applicationId
and iOS bundle ID. Full Tauri crate tests (21), 11 CLI integration tests and strict
all-targets Clippy pass. Prior existing-CLI baseline reproduction retained at
/tmp/crepus-tauri-v1-metadata-review/; scoped patch at
/tmp/crepus-tauri-v1-metadata-fix/. Generated native apps were not hardware-built.

C# SetTemplate rejects embedded NULs in template/baseDir before native transport,
after the existing disposal guard. Actual managed wrapper/native ABI regression
fails without the checks (NUL template accepted), passes candidate and proves
Unicode include paths/astral text, unchanged previous IR after rejection, and
continued session usability. Smoke test optional CREPUS_ABI_LIB resolver allows an
explicit absolute native library. The first DYLD-only run could not resolve the
library; explicit resolver then passed. Both smoke project and full C# solution
warnings-as-errors builds pass with zero warnings. README documents command.
Evidence /tmp/crepus-csharp-abi-nul-fix/. Existing native dylib used; no native ABI
change in this patch. Windows not checked.

Primary inspected scoped production/tests/config/documentation diffs; concurrent
work retained. No new dependencies, commits, pushes, deployment or remote mutations.
Web main-thread rendering and TUI large-scroll candidate remain outstanding; this
tracking improvement does not move DOM/rendering to a worker or guarantee no lag.

Existing real C# ABI lifetime fixture also passes (valid lifecycle, idempotent
disposal, five disposed-operation guards before serialization). Focused Rust
formatting and scoped whitespace checks pass.


### 2026-10-18: Large TUI sibling scrolling and web worker boundary

Public BufferRenderer baseline reproduces the TUI aggregate-height defect:
65,536 one-line rows at scroll-bottom show ROW-65534 instead of ROW-65535.
Scrollable now sums sibling heights/gaps and clamps offsets in usize, then paints
intersecting children in child-local Ratatui coordinates and copies their visible
cells into a viewport-sized buffer. It no longer allocates a globally positioned
u16 virtual rectangle or shrinks away trailing siblings. Short-content flex layout
and horizontal paths are retained. An individual subtree's recursive height
measurement still has the existing u16 limit; docs state this explicitly.

All three new public regressions pass: 65,536 rows with bottom/explicit offset;
aggregate gaps beyond u16 plus i64::MAX offset clamp and partial/nonzero viewport;
and a 65,535-row individual child. All26 focused scroll tests and the full TUI crate
suite including doc tests pass. Existing clipping snapshot proves small-layout,
style, border, Unicode and nested-scroll compatibility. Primary inspected source,
regressions and scoped diff; independent reviewer approved coordinate bounds and
remaining limit. Evidence /tmp/crepus-tui-large-scroll-fix/{render.before.rs,render.patch,large_scroll.rs}.
No claim of arbitrary-height individual nested subtrees or live terminal-device QA.

Parallel web review identified a compatibility constraint before worker work:
JavaScript islands receive synchronous {runtime, rerender}; converting those exports
to an async proxy silently changes that API. New pure scaffold runtimes are the
first sound worker-default integration target, with explicit main mode for custom
DOM-dependent/island runtimes. Worker must own initialization/state and serialize
handler-plus-render transactions, carry revision/generation guards, recover its
queue after errors and never replay a mutation on another runtime. DOM commits,
focus/composition/event cancellation and island disposal stay on main. Current
app.js remains main-thread; no worker implementation or latency improvement is
claimed in this TUI change. Worker transport/browser heartbeat checks remain next.
No dependencies, commits, pushes, deployments or remote mutations.

Strict all-targets TUI Clippy, focused Rust formatting and scoped diff whitespace checks pass.


### 2026-10-18: Worker-owned web runtime for new pure scaffolds

New web scaffold runtime exports crepus_worker_runtime, selecting a module worker
by default. Legacy/unmarked runtimes retain main execution; explicit HTML head
meta crepus-runtime-thread=main opts a marked runtime out (worker opts unmarked in).
Worker owns one initialized WASM instance, state, bundle serialization and FIFO
handler-plus-render transactions, awaiting asynchronous handlers. Rejected calls
report errors and release the queue; transport failure rejects pending/future work
without mutation replay or runtime fallback. DOM commits, event cancellation,
focus/selection/composition and island cleanup remain on main. JS module islands
require explicit main mode to preserve their synchronous runtime-export contract;
worker mode reports an actionable island error before importing the module.

Scoped/root generation and bundle revision guards discard obsolete HTML. Independent
review found a root/scope race: a root commit could detach a queued scope whose
handler subsequently mutated state. A focused regression failed before repair;
detached scoped handler responses now request a current root render without
replaying the handler. Existing legacy event/island tests remain green. Worker
assets publish in both dist and .crepus-dev; exact dev routes use existing safe
static-file resolution and reject external symlinks.

Verification: 127 Bun tests/304 assertions (103 existing event/island plus24
worker/client/app), 144 slim CLI unit tests, all29 web CLI integrations, focused
Rust formatting and diff checks pass. Original web integration run hit the known
default-compiler missing wasm32 target; rerun with installed stable RUSTC passes,
including both real WASM example/docs builds. Strict production CLI Clippy passes;
all-targets Clippy remains blocked solely by unrelated preview.rs:89
items_after_test_module (production items at131/136). No unrelated repair made.

Actual CLI-built WASM stress runtime in Chromium: two rapid events plus rendering
occupied ~728ms; a 10ms main-thread heartbeat fired73times in worker mode versus2
in explicit main mode (~725ms). Verified one worker vszero, final counter, scoped
updates, expected handler failure followed by successful mutation, preserved focus
and selection, and root/scope race recovery. Delayed actual WASM response proves
SSR reveals before initialization and an early click executes exactlyonce afterward.
Rendered stress screenshot inspected. Separately CLI-scaffolded a default site,
built its generated runtime against the current local web backend (temporary
manifest path override), and verified SSR initial content, exactlyone worker and
actual backend bundle update in Chromium; screenshot inspected. This does not
claim a newly published crate or browser-wide/hardware performance certification.
Evidence: /tmp/crepus-web-worker-fix/ (browser-proof.json, default-proof.json,
browser.py, default-browser.py, screenshots, app.patch, bun-final.log, cli-unit.log,
clippy.log); Rust asset patch /tmp/crepus-worker-assets-fix/scoped.patch.

Primary inspected production/test/doc diffs; independent final source review found
no blockers. docs/web-runtime.md documents threading and compatibility limits,
linked from README/docs index. No dependencies, commits, pushes, deployment,
remote mutations or installed tool changes. Existing applications must explicitly
opt in; other framework/native targets remain separate work. DOM work can still
block; this is not a no-lag guarantee. Full workspace, Safari/Firefox and device
checks were not run for this feature. Goal remains active.


### 2026-10-18: Worker boot rejection/cleanup and CLI Clippy gate

Chromium reproduced a readiness rejection leak: fast runtime initialization failure
became unhandled while main awaited bundle/manifest fetches. main now attaches an
immediate rejection observer while retaining the original promise for later awaits,
so the existing visible error path still receives the error. Startup failure also
disposes the worker; successful initialization followed by failed bundle fetching
previously left it alive. No state mutation replay or runtime fallback introduced.

New bootstrap tests execute actual main/error-handler source in isolated Bun
subprocesses. Explicit deferred bundle/manifest/readiness gates and event-loop ticks
replace timing assumptions. Baseline fails two regressions; candidate passes all3
(19assertions), preserving pending SSR delegation and original promise identity.
Full browser/runtime Bun gate passes130tests/323assertions. Actual module-worker
Chromium fixture with deliberately failed JS initialization proves baseline has an
unhandled readiness error and zero terminations, candidate unhandled[] and exactly
one termination; bundle-fetch abort also terminates once. No claim that the failure
fixture executes WASM. Existing actual CLI-built WASM success fixture with current
app rerun passes event/state/error/scoped/root-race/focus checks and SSR revealbefore
WASM plus exactlyone earlyevent. Worker heartbeat73 vsmain4 across~729/750ms in
this local run; no universal latency claim. Error screenshot inspected.
Evidence /tmp/crepus-web-bootstrap-fix/ (app.before.js,app.patch,baseline.log,bun.log,
browser.py,browser-proof.json,browser.png,cli.log). First browser assertion expected
a single Error prefix; actual worker transport wraps the string error, so corrected
the test expectation to its existing double prefix before final green run.

Resolved the repeatedly reported CLI all-targets lint by moving the unchanged
preview.rs test module after CargoBuildOutcome/cargo_build_fallback; declaration
bodies/comments preserved byte-for-byte. All144 CLI unit tests, strict all-targets
slim CLI Clippy, formatting and scoped whitespace checks now pass. This removes the
previous preview.rs blocker; does not certify the full workspace/default GPUI build.
Scoped before/final diff /tmp/crepus-preview-test-module-fix/.

Independent target follow-up found remaining full caller-context slot captures at
LVGL lib.rs522/585, embedded include_expand.rs155/213 and GPUI renderer.rs861/929.
Primary confirmed LVGL captures clone ctx containing the entire loop list per used
slot include. Source-derived N-by-N copied row contexts are quadratic, not a timing
measurement. Web already uses a private borrowed SlotFrame; a compatible LVGL fix
can thread that frame through three private rendering helpers while preserving
owned public TemplateContext.slot, caller context/base_dir/bundle identity,
forwarding/defaults and consumption depth. No target fix or target performance
measurement claimed yet; queued next. No dependencies, commits, pushes, deployments,
remote data changes or installed-tool changes. Concurrent work preserved.


### 2026-10-18: Borrowed supplied slots in LVGL and embedded

Both target include expanders cloned the supplied slot AST and full caller context
per occurrence, including an entire input list inside row loops. Reused Web's
private synchronous SlotFrame pattern: borrowed nodes/context plus caller-slot
chain threaded through private recursion. Removed owned captures from ordinary
and named include expanders. Public entrypoints and TemplateContext.slot remain
unchanged; owned external slots still render from their captured caller. Empty
supplied slots do not inherit parent content. Props/defaults, caller local/loop
scope, target flags, slot-wrapper attributes, directory/VFS/cache identity, lazy
errors and include-depth guards retain their prior paths.

Primary verified exact production-before snapshots for both targets before baseline
measurement, inspected scoped production/tests diffs, and independently reviewed
candidate lifetimes/compatibility. Repeated supplied-slot public fixture (ordinary
includes, five-sample local debug medians, no timing threshold):
- Embedded100/400/1000 rows:5.63/57.56/325.87ms ->1.48/4.92/12.60ms.
- LVGL100/400/1000 rows:5.13/54.18/310.61ms ->1.62/4.80/13.70ms.
Approximately26x/23x at1000 rows. Embedded timing includes document debug-output
formatting; LVGL includes source parsing/XML output. These are scoped host render
measurements, not allocation-byte counts, browser latency, or firmware proof.
Manual ignored benchmark tests consume output and assert the final row exists.
Other functional tests assert per-occurrence content. Mutating local declarations
can still clone their scope; no blanket linearity claim for all slot templates.

Full embedded+LVGL suites with embedded macros pass216checks including doc tests;
manual benchmarks run separately. Six focused new semantic tests cover named/
ordinary props/defaults, loop locals, reused/nested forwarding, owned external slot
bundle/base_dir, no accidental inheritance, lazy error and cycle depth behavior.
First embedded fixture used `let` as markup rather than `$: let`; corrected it,
then all tests pass. Added an explicit embedded owned-slot-through-borrowed-frame
case after review identified that coverage gap. Strict all-targets Clippy for both
targets/macros, host embedded no-default-features check, formatting and diff checks
pass. Two embedded public CLI check/snapshot integrations and three LVGL manifest
CLI integrations pass. Render/layout/paint golden coverage remains unchanged; no
hardware, ESP/STM32, full workspace or GPUI build claim.

Docs/lvgl.md and embedded.md explain the scoped improvement and remaining local
scope-copy cost. Before/final production diffs, focused tests and timing logs:
/tmp/crepus-lvgl-borrowed-slots-fix/ and /tmp/crepus-embedded-used-slot-fix/;
shared evidence /tmp/crepus-slot-scaling-baseline.log, /tmp/crepus-slot-tests.log,
/tmp/crepus-slot-cli.log, /tmp/crepus-slot-lvgl-cli.log, /tmp/crepus-slot-timing.json.
GPUI's corresponding owned supplied-slot capture remains queued; event closures
have separate intentional owned lifetimes and must not be changed blindly.
No dependencies, commits, pushes, deployments, remote mutations or cleanup.
Concurrent changes retained; goal remains active.


### 2026-10-18: GPUI supplied-slot context copying

Located the remaining GPUI path in crepuscularity-runtime/src/renderer.rs, not
crepuscularity-gpui/src. Runtime no-run compilation and a public render_nodes
construction benchmark established a usable baseline before production changes.
Borrowed private SlotFrame now carries authored nodes, caller context and the
caller-slot chain through synchronous rendering; ordinary/named includes no longer
clone the full caller context and slot AST. Public render_nodes/render_node and
prepared entrypoint signatures remain unchanged. Target-default injection, path
strings for generated state/animation IDs, stack growth/IncludeGuard, red include
diagnostics, source snapshots and public owned TemplateContext.slot fallback remain.
Owned GPUI text/styles/IDs/animation parameters stay owned; no borrowed frame escapes.
Prepared-include walker and publication paths were reviewed without changes: authored
slots already prepare at caller context and external owned slots retain their own
base directories/bundles. Independent review found no blockers.

Five-sample local debug median GPUI element construction, 100/400/1000 rows with
ordinary includes and supplied slot IDs:5.21/58.72/309.53ms ->2.53/9.38/23.85ms.
Approximately13x at1000 rows. This excludes actual layout/painting/complete frame
latency and is not a no-lag guarantee. Manual ignored public benchmark constructs
and consumes output and validates the root identity, without a timing assertion.
Ordinary source parsing is outside this benchmark; include lookup/construction is
inside. Local declarations and immutable target-default fallback can still clone
scope; element construction/view commits remain on UI thread.

All93 runtime tests pass (11 default-ignored), plus70 downstream GPUI tests
(10 default-ignored). Three new non-GPU semantic tests cover ordinary/named nested
forwarding, caller declarations/style vs component props, prepared-render parity,
external owned slots with distinct source directory/bundle and missing targetflags,
loop aliases and exact generated IDs. Explicit new ignored Metal test runs/passes
on this host: actual headless window draw/capture verifies forwarded caller red
pixels override the component's blue prop in ordinary and named components.
This is headless Metal proof, not physical desktop/cross-platform interaction QA.
Primary fixed test-only shared Metal lock visibility and restored the public render
function doc comment after the new private struct, before final compilation.
Strict all-targets runtime Clippy, focused formatting and scoped diff checks pass.
Existing third-party block future-incompatibility notice remains; no dependency
change. Other ignored Metal tests and full workspace/cross-platform gates not run.

Evidence /tmp/crepus-gpui-borrowed-slots-fix/{renderer.before.rs,final.patch,
baseline.log,timing.log,timing.json,tests.log,metal.log,clippy.log,gpui-tests.log};
readiness compile /tmp/crepus-gpui-build-readiness.log. docs/runtime.md updated.
Primary inspected actual scoped diff and acceptance results; concurrent prepared
include work retained unchanged. No dependencies, commits, pushes, branches,
deployment, remote mutation, cleanup or installed-tool changes. Goal remains active.


### 2026-10-18: Parser boundary probes and authored input test contract

Independent bounded LSP, native-adapter and ABI/parser source reviews found no new
confirmed defect. They checked UTF16/version/diagnostic lifecycle/URI paths; native
draft delivery and Flutter callback contracts; and ABI reentry/lifetime/context
validation. Apparent Flutter action-only payload and ABI raw event behaviors are
documented contracts, not findings. No runtime LSP/native-device certification.

Built current core and ran a separate deterministic malformed/Unicode probe through
all6 filename-selected frontends:50000 short token sources,300000 parse+diagnostic
cases, no panic. Error formatting also executed. Probe sources include multibyte
characters, astral Unicode, NUL/noncharacters, comments/script/style delimiters,
quotes/escapes and incomplete structures. Retained a smaller2000-source×6
frontend smoke as tests/parser_robustness.rs (12000 cases), fixedseed and bounded
inputs, standard library only. This tests panic resistance over this corpus, not
semantic correctness of all inputs, termination/abort resistance or exhaustive
fuzzing. No production parser or native adapter behavior changed.

Full core gate exposed an unrelated stale authored Svelte negative expectation:
input date/color bind:value are now admitted by authored.rs's14-type allowlist and
current docs/frontends.md374. Moved those expectations into the existing positive
matrix, covering all14 supported types with shorthand+explicit target marker and
retained type assertions. File/checkbox/dynamic/select and conflicting/duplicate/
malformed bindings and textarea-body negatives remain. One intermediate edit
extended positives but had not removed the old negative rows; corrected those
before final gate. Independent review checked actual contract diff and corpus scope.
All732 core checks, strict all-targets core Clippy, formatting and scoped diff
checks pass. No host framework/browser execution claimed for this test-only change.

Evidence /tmp/crepus-parser-boundary-probe/ (main.rs,probe,result.log,before/,
test-contract.patch,tests.log,final-tests.log,clippy.log) and core build log
/tmp/crepus-parser-probe-build.log. No dependencies, commits, pushes, branches,
deployment, remote changes, cleanup or installed-tool modifications. Concurrent
production edits retained. Remaining local-declaration scope copying and broader
framework/runtime coverage remain open; goal active.


### 2026-10-18: Worker transport and extension audit refresh

Reviewed worker request settlement, fatal transport teardown, cached initialization
failure, and FIFO recovery with an independent read-only reviewer. No new concrete
defect found. Native queue/picker review likewise found no supported-lifecycle
regression; a source-only interrupted-picker hypothesis remains unvalidated and
was not changed. Primary reviewed extension capability scanning and watcher paths.

Current verification: `cargo test --locked -p crepuscularity-webext` passes all 44
unit tests; Bun worker queue/client suites pass 15 tests with 63 assertions. These
exercise queue/client behavior separately, not a real browser worker's response
cloning adapter. No browser-extension installation or native device run performed.
No production changes, commits, pushes, or remote mutations in this audit refresh.


### 2026-10-18: Real browser worker response adapter regression

Closed the preceding response-cloning coverage gap with the manual
`crates/crepuscularity-cli/tests/web_worker_browser.py` regression, using unchanged
production runtime-client.js/runtime-worker.js and a fixture JavaScript runtime on
a temporary loopback server. Chromium verifies uncloneable call and event-render
responses reject, queued/later reads succeed, completed mutations are not replayed,
and three queued initialization requests receive the same failure after exactly
one initialization attempt. Client/browser/server/temp cleanup and bounded waits
are included. Both the initial run and strengthened final run passed; independent
read-only review found no blockers. Added the command and scope to web-runtime.md.
No production change or WASM execution is claimed. Safari/Firefox and native-device
checks remain outstanding. No commits, pushes, deployments or remote mutations.


### 2026-10-18: Capability watcher populated-directory imports

Confirmed a public CapabilityWatcher defect: moving a pre-populated directory
containing nested .crepus browser API usage into a watched project emitted no
missing-capability warning. Both deterministic Create(Folder)/rename dispatch and
an actual macOS same-filesystem directory move failed before the change. Directory
paths now request the existing capability scan, once per notify event. No invented
FileChanged events; manifest notifications and invalid-manifest error handling
remain. Unrelated ordinary files still do not trigger scans. The CLI uses its own
watcher and one-shot scanner, so no CLI-specific repair is claimed.

All 47 extension unit tests and strict all-targets Clippy pass; rustfmt/scoped diff
checks pass. Independent review approved the actual task patch. Corrected the
capability scanner docs example (Path argument and Vec<CapabilityUsage> result)
and documented public watcher behavior. Before/patch evidence lives under
/tmp/crepus-webext-directory-watch-fix/. Linux/Windows native watcher delivery and
large-project scan throughput were not tested; directory events may add scans on
the notify callback. No dependencies, commits, pushes, branches, deployments,
remote mutations or cleanup. Concurrent changes preserved; audit goal remains active.


### 2026-10-18: TUI horizontal clipping preserves logical geometry

Confirmed partial host buffers reflowed text at the clipped width and restarted
left-clipped content at its first character; borders/titles were likewise redrawn
at the crop edge. Four new public BufferRenderer integration regressions failed
before the fix, including requested area (2,3,16,1), visible buffer (5,3,5,1):
ABCDE instead of the full-render crop DEFGH. Coverage compares every cell against
full-render crops for wrapping plus vertical clipping, alignment, borders/titles,
nonzero origins, Unicode wide/combining characters, and preexisting host styles.

The shared paint_node_buf path now renders horizontally clipped nodes into logical
width/visible-height scratch cells, seeded from the host, then copies the crop.
Normal full-width rendering skips scratch allocation. This fixes content and
container geometry together without changing Paragraph semantics. Independent
review checked recursion termination, style preservation, nested scroll paths and
allocation bounds. All four regressions, the full TUI suite including doctests,
strict all-targets Clippy, scoped rustfmt and diff checks pass. Added docs/tui.md
behavior note. Before/source task diff at
/tmp/crepus-tui-logical-text-clipping-fix/{render.before.rs,render.patch}.

No interactive terminal or platform-specific device validation performed. An
individual subtree's existing u16 height limit remains. Separate widget helper
review identified inline raw script closing-tag handling as a source-backed
candidate needing browser reproduction and a semantics/CSP-preserving design;
no widget helper production change made. No commits, pushes, branches, dependency
changes, deployments, remote mutations or cleanup. Goal remains active.


### 2026-10-18: Tauri listener cleanup reentrancy

Confirmed Listener::drop destroyed the last handler Arc while holding the event
registry mutex. A valid handler capturing another Listener from the same app
therefore deadlocked when the captured subscription was destroyed. Added a bounded
thread/channel regression; baseline failed its two-second timeout and exited.
Scoped handler extraction now releases the guard before destroying captured user
resources. Empty bucket cleanup remains under the lock. Existing in-flight emit
snapshots retain their semantics; independent review checked same-name nested
subscriptions as well. The test also verifies an empty registry and successful
new listen/emit after cleanup.

All 23 all-feature Tauri checks pass (unit/integration including package-alias
compile checks); strict all-feature/all-target Clippy, rustfmt and scoped diff
checks pass. Actual task diff inspected against saved before.rs under
/tmp/crepus-tauri-listener-drop-fix/ (task.patch). No desktop/mobile host run or
concurrency stress benchmark performed. No dependencies, commits, pushes, branches,
deployment, remote mutation or cleanup; concurrent changes preserved.

Embedded reviewer source-traced a separate candidate: ST7789 preset byte swapping
in display.rs feeds little-endian ImageRawLE decoding in panel/mipidsi_blit.rs via
the embassy_stm32 adapter. Color preservation needs a composed host regression and
an explicit driver-versus-wire byte contract before changing presets; no hardware
result or embedded fix claimed. ABI argument/transaction review found no new
supported-flow defect. Goal remains active.


### 2026-10-18: Native RGB565 display input avoids double encoding

Confirmed the ST7789 preset's wire-byte swap fed little-endian mipidsi image
decoding, corrupting RGB565 values. Baseline native-display regression failed on
the Bgr config. Added backward-compatible Rgb565Display::uses_native_rgb565_bytes
(default false); flush_framebuffer skips only byte swapping for opt-in semantic
adapters. Both STM32 mipidsi adapters opt in. Offsets, length checks, existing raw
and ESP drivers, rgb565_for_panel and direct LE blit APIs preserve their contracts.
Corrected STM32 BGR channel-order wording and documented adapter input semantics.

Host tests capture both byte-order modes/offsets/scratch behavior and actual
mipidsi ST7789 U16BEIter output through production blit_rect, preserving primary
words f800/07e0/001f for ST7789 and default configs. Direct blit_full remains LE.
Full embedded suite with macros+mipidsi passed; strict all-target Clippy passed
after correcting two new test iterator lint errors. Affected display tests reran
green after that correction. no-default-features+mipidsi host check, scoped
rustfmt/diff checks pass. Independent review approved actual display/STM32 changes.
Before and final task patches: /tmp/crepus-embedded-native-display-fix/.
STM32 feature cross-compilation and physical panel/SPI/color calibration were not
performed; host driver conversion is the verified boundary. No new dependencies,
commits, pushes, branches, deployments, remote changes or cleanup. Goal active.


### 2026-10-18: STM32 adapter cross-target gate and frame export guidance

Closed the preceding STM32 adapter compile gap with the installed stable ARM
compiler:
`RUSTC=/Users/undivisible/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rustc cargo check --locked -p crepuscularity-embedded --no-default-features --features embassy-stm32,embassy-stm32/stm32f411re --target thumbv7em-none-eabihf`.
It passes and compiles the actual Embassy adapters, including native-byte hooks.
The initial default compiler lacked target core despite the installed stable
rustup target; switching the compiler resolved that configuration mismatch.
Two existing private config-field dead-code warnings remain; no strict target
Clippy pass claimed. No new manifest dependencies or toolchain installs.

Corrected examples/advanced/embedded-stm32/README.md advice to byte-swap exported
frame.bin for color issues. The firmware calls direct blit_full, which always
decodes LE RGB565, unlike the native-byte-aware Ui::flush adapter route. Host
exports should retain default LE input; RGB/BGR controller channel order is a
separate physical calibration. Actual diff/scoped whitespace checks reviewed.
Before doc snapshot: /tmp/crepus-stm32-target-check/README.before.md.
This is an adapter library check, not a linked/flashed standalone firmware build
or physical SPI/panel proof. No commits, pushes, branches, remote mutations,
deployments or cleanup. Goal active.


### 2026-10-18: Scoped slot-rotation timer ownership

Confirmed a main-thread retention leak: initSlotRotate(root) registered descendant
intervals under the outer root; rerenderScope only stopped intervals keyed to the
scope, leaving removed elements retained until their next tick (up to 2147483647ms).
The deterministic scoped-replacement regression failed before the change. Added
per-element stop ownership alongside the existing root cleanup sets. Shared
preserveFocus replacement stops rotations in the removed subtree, overlapping
initialization replaces a previous element timer, and module-island error-content
replacement also stops descendant rotations. Unchanged sibling timers stay active;
pending transitions remain guarded by the existing active flag.

Actual Chromium module app with fixture runtime: baseline retained the removed
interval (3 active timers), candidate cancels it immediately (2 active), sibling
active in both. Exercised the update button and checked rendered replacement text.
Proof/browser script and scoped production patch under
/tmp/crepus-slot-rotation-teardown-fix/. Two retained regressions cover scope
replacement/reinit and failed-module replacement. The source-fragment slot tests
were updated to include the shared ownership helper; an intermediate suite failed
16 fragment tests before that harness adjustment. Final primary Bun check: all132
checks/333assertions across event, worker app/queue/client and boot suites pass.
Independent review approved production diff; scoped whitespace check passes.

No WASM/browser performance or heap-size measurement, Safari/Firefox, or device
validation claimed. Arbitrary external DOM removals still use the next-tick guard;
framework-owned replacement now cleans immediately. No Cargo rebuild, dependencies,
commits, pushes, branches, deployments, remote mutations or cleanup. Goal active.


### 2026-10-18: Reactive memo/effect thread affinity

Confirmed Memo<i32> and Effect auto-implemented Send/Sync despite IDs resolving
against thread-local NODES/RUNTIME. A fresh-library two-thread public API probe
returned destination memo value 2 instead of source value 1; moving/disposal of an
effect removed a destination subscription (one run instead of two). Four new
compile_fail doctests also failed baseline because cross-thread movement/shared
references compiled successfully. Signal already carries Rc and was unaffected.

Added private zero-sized Rc phantom markers to Memo/Effect; no allocations,
graph/lifecycle logic or same-thread behavior changed. Cross-thread transfers and
sharing now fail compilation. This deliberately rejects previously compiling
invalid use; callers must construct/use/dispose these handles on their owning
thread. Preserved concurrent constructor/lifetime edits. Independent review found
no blockers, including WASM/DOM callback compatibility by inspection.

All 87 all-feature reactive checks pass, including four compile-fail examples;
strict all-feature/all-target Clippy, wasm32 all-feature library check using the
installed stable compiler, scoped rustfmt and diff checks pass. Actual saved task
diff inspected. Evidence /tmp/crepus-reactive-thread-affinity-fix/ includes before
files, scoped.patch, probe.rs/probe, and final tests.log. External consumers and
browser WASM execution not run. No commits, pushes, branches, dependencies,
deployment, remote mutation or cleanup. Goal remains active.


### 2026-10-18: Web reactive facade completeness and downstream check

Checked in-repo downstream uses after the thread-affinity correction: the reactive
counter example executes successfully. Found the WASM web reactive facade omitted
bind_bool_attr/bind_checked/bind_value/hydrate_root, though its existing feature
already enables the underlying DOM implementation. Added only those four existing
helper reexports under the unchanged wasm32+reactive gate and documented that
entry point plus owning-thread usage. No host API, runtime logic or dependency
change. Independent review approved actual saved task diff.

A separate temp WASM consumer with only crepuscularity-web as a direct dependency
failed baseline E0432 for all four imports, then passed locked/offline checking
after the change, including construction of Signal/Memo and form Effect handles.
Evidence /tmp/crepus-web-reactive-facade-fix/ contains before source, task.patch,
consumer source/lock and baseline/final logs. Normal default+reactive all-target
strict Clippy passes; scoped rustfmt/diff checks pass. The no-default-features
all-target variant exposes an unrelated existing angular_attributes.rs test import
of render_template_to_html_with_ssr while ssr is disabled; recorded without
changing that test. The no-default-feature consumer library itself passes on WASM.
Browser execution/external production consumers not run. No commits, pushes,
branches, deployments, remote changes or cleanup. Goal active.


### 2026-10-18: Optional SSR test coverage respects enabled features

Repaired the previously recorded no-default web all-target failure in
angular_attributes.rs. Its variants helper imported/called the SSR-only renderer
unconditionally. Plain HTML remains always checked; SSR is appended only when
ssr is enabled, matching the existing independent hydration branch. Assertions and
source fixtures remain unchanged; no production renderer behavior changed.

Full no-default web suite: 106 checks pass. Focused angular attribute tests also
pass with hydration-only (no SSR) and default SSR+hydration (three checks each),
covering the one/two/three renderer combinations. The formerly failing strict
no-default+reactive all-target Clippy gate now passes. Parallel static audit found
no other unguarded SSR/hydration imports across web integration tests. Scoped
rustfmt/diff checks and actual task diff inspection passed. Evidence under
/tmp/crepus-web-test-feature-gates-fix/ (before.rs/task.patch and three test logs).
No browser runtime validation needed or claimed for this test-only change. No
commits, pushes, branches, dependencies, deployments, remote mutations or cleanup.
Goal remains active.

### 2026-10-18: Make the VS Code verification command runnable

Editor integration review found no additional confirmed resolver/lifecycle bug.
The package had pretest but no test script: bun run test fell through to /bin/test
and exited1. Added test="bun test tests" so the standard package command invokes
the existing compile prehook and both resolver/lifecycle suites. Verified command
output includes TypeScript compilation followed by18 passing tests/45 assertions.
Compile refreshed out/extension.js and emitted the existing source resolver's
out/server-executable.js; no lifecycle/resolver source was rewritten this pass.

Primary inspected the manifest diff and generated output; independent source
review covered discovery, restart/stop ordering and advertised .crepus registration.
Initial direct test invocation used a repository-relative filter from the package
cwd and matched zero files; the corrected root invocation and final package script
ran all18 tests. Evidence /tmp/crepus-editor-integration-audit/ contains initial,
corrected and package-command logs plus package.before.json. No VS Code Extension
Host, VSIX install or Zed execution claimed. No dependencies, commits, pushes or
remote mutations. Goal remains active.

### 2026-10-18: Refresh distributed WASM and close URL-binding package gap

Free space recovered above3GiB externally, allowing the deferred package build.
Ran the existing build.sh with process-local RUSTC resolved to the installed
rustup compiler; installed tools/default environment were not modified. Matching
wasm-bindgen0.2.126 passed the script's version gate. Release WASM compilation,
frontend-presence gate, generation/copy and all66 parser tests pass (256 assertions).
Added one regression for resolved image/link URLs plus their binding provenance.

The identical public example recorded previously without srcBind/hrefBind now
contains both, matching native serialization and the updated public types.
Strict typed-consumer and exhaustive kind/style checks pass. npm/wasm and pkg
artifacts match byte-for-byte; only their WASM bytes changed from this turn's
prebuild hashes, while generated glue stayed identical. Primary inspected the
actual test/generated diff. This supersedes the earlier stale-package blocker.

Evidence /tmp/crepus-wasm-package-refresh/ includes build.log, before/after hashes,
refreshed.json and typecheck.log; the old public output remains under the earlier
URL-binding evidence directory. The initially named regression-baseline.log was
captured after build completion and is a passing postbuild run, not a red baseline;
regression-after-build.log records that same result unambiguously. No browser,
published-package or full CLI integration claim. No commits, pushes, dependencies,
installed-tool changes or remote mutations. Goal remains active.

### 2026-10-18: Close deferred CLI WASM integration acceptance

Retested the previously failing real web-build integrations with process-local
RUSTC selected by rustup which rustc. Both docs-site and reference-site builds
compile WASM, run wasm-bindgen and emit the asserted package/HTML/bundle outputs.
Then rebuilt the slim web_cli test executable against current source with normal
host settings and ran its entire29-test suite under that child compiler selection:
29 passed, zero failed/ignored/excluded. This includes framework emits, scaffold
contracts, extension builds and the two actual WASM builds. The initial focused
run matched three tests (including multi-target selection), not just two.

Evidence /tmp/crepus-cli-wasm-acceptance/ includes selection.txt, focused tests.log,
current-build.log and complete current-tests.log. The CLI binary was refreshed
before final acceptance; no existing green result was used to infer source parity.
This supersedes the two missing-target failures as acceptance blockers under the
selected compiler. Default PATH still selects the separate non-WASM Homebrew
compiler; that environment/readiness mismatch was not silently changed or skipped.
No browser/live server, published package, full workspace or all CLI integration
suite claim. No production edits, commits, pushes or remote mutations this pass.
Goal remains active.

### 2026-10-18: Nested WASM builds honor configured Cargo home

Reproduced custom CARGO_HOME being ignored: an isolated actual CLI build selected
the fake PATH Cargo instead of the configured-home executable. The shared bin
helper now checks CARGO_HOME first and treats it as authoritative; both executable
selection and nested PATH setup reuse that helper. Explicit CARGO remains first.
The executable filename now uses cargo.exe consistently on Windows, including
HOME-based discovery; this Windows behavior was inspected, not executed.

The Unix regression uses child-process-only environment settings and fake marker
executables, covers a Cargo-home path containing spaces and explicit CARGO
precedence, and fails before the fix/passes afterward. All147 slim CLI units and
all30 web_cli integrations pass; real WASM tests use the previously validated
process-local rustup compiler selection. Strict all-target slim Clippy, scoped
format/diff checks and primary/independent actual-diff review pass. Evidence:
/tmp/crepus-cargo-home-fix/ (before files, baseline, regression/units/web tests,
Clippy and task patches). No Windows or real custom-home compiler installation
test, dependencies, installed-tool changes, commits, pushes or remote mutations.
Default mixed-toolchain environment remains unchanged. Goal remains active.

### 2026-10-18: Anchor relative Cargo home before nested directory changes

Follow-up edge case to configured-home support: relative executable/PATH entries
were interpreted again after the nested process changed into runtime/. Resolve
configured home with std::path::absolute against the caller, use its absolute bin
for discovery and pass the same absolute CARGO_HOME to both metadata/build child
commands. Do not canonicalize or change global environment/tool overrides.

The isolated source-exact pre-change helpers fail ENOENT; updated helpers execute
the intended marker and report the anchored home. The first full Cargo comparison
finished compiling after the edit, so its passing baseline.log is not red evidence.
The retained before/after helper sources and probe logs establish the regression
separately. The CLI regression now covers relative/absolute home × implicit/explicit
Cargo, including spaces and child-reported home identity. All147 slim units and30
web integrations pass; strict all-target Clippy, formatting/diff checks and primary/
independent actual-diff review pass. Evidence /tmp/crepus-relative-cargo-home-fix/.
Real WASM acceptance uses process-local rustup compiler selection as previously.
No Windows run, installed-tool change, dependencies, commits, pushes or remote
mutations. Goal remains active.

### 2026-10-18: Verify comment suppression through actual LSP stdio

Added two protocol assertions for the earlier indentation-comment fix: after a
versioned full document update containing an alias-named comment, completion and
hover must both return null. Built the current crepus-lsp binary and ran the full
existing JSON-RPC script with stdin left open in both graceful-shutdown and
exit-without-shutdown modes. Both pass, including lifecycle versions, diagnostics,
closed-document behavior, JSX context and expected process exit statuses0/1.

Independent bounded cursor review found no new UTF-16/surrogate/CRLF boundary or
diagnostic conversion defect; invalid-position clamping was not misreported as a
bug. Primary inspected the actual13-line protocol diff; scoped diff checks pass.
Evidence /tmp/crepus-lsp-stdio-comment-check/ includes before script, current build
and both protocol logs. No production changes, actual editor-host/VSIX test,
commits, pushes, dependencies or remote mutations this pass. Goal remains active.

### 2026-10-18: Reactive graph cleanup and reentrancy follow-up

Primary reviewed signal ownership, snapshot replacement and memo dependency
reconciliation; independent review covered effect constructor unwind, queued
disposal, self-disposal and destruction outside graph borrows. No additional
verified defect found. Current tests exercise equal-valued dynamic dependency
switches, reentrant comparison/clone/destructor callbacks, explicit disposal,
thread-local teardown and thread-affinity compile failures.

All87 all-feature reactive checks pass with one existing ignored test;
strict all-target/all-feature Clippy passes,
and all-feature wasm32-unknown-unknown cargo check passes with process-local
rustup compiler selection. Evidence: /tmp/crepus-reactive-cleanup-audit/ contains
complete test, Clippy and target-check logs. These establish graph behavior and
browser-target compilation, not actual DOM/hydration execution, heap profiling or
a no-lag guarantee. No production edits, commits, pushes, dependencies or remote
mutations this pass. Goal remains active.

### 2026-10-18: Execute reactive WASM DOM and hydration tests in Chromium

Closed the browser-execution gap from the preceding graph review. Built current
reactive dom_bind tests for wasm32-unknown-unknown with all features. Matching
wasm-bindgen-test-runner0.2.126 served its interactive test page on an ephemeral
loopback port (NO_HEADLESS=1, WASM_BINDGEN_TEST_ADDRESS=127.0.0.1:0); installed
Playwright Chromium ran the actual WASM module and reported9 passed, zero failed/
ignored/filtered. Runner server was terminated after inspecting the complete DOM
test result. No WebDriver installation or browser collaborator mock was needed.

Coverage includes live text/attributes/classes, boolean attributes, edited input/
textarea values, checked properties, stopping updates after disposal, canonical
and legacy hydration payloads and unsafe-URL removal/recovery. Separately built
and ran the actual dom WASM tests under Node:3 passed for missing-DOM behavior.
Evidence /tmp/crepus-reactive-browser-audit/ contains target builds, full browser
DOM output and Node runner output. No production edits; tests were already present.
This establishes these DOM helpers in Chromium, not application-wide hydration,
heap profiling, other browsers or a no-lag guarantee. No dependencies, commits,
pushes, installed-tool changes or remote mutations. Goal remains active.

### 2026-10-18: Cover select and file-input reactive value branches in-browser

Parallel worker added two focused dom_bind regressions; primary reviewed all new
code and exercised the actual WASM artifact in installed Playwright Chromium.
Select coverage includes overriding an edited initial selection, unmatched values
clearing selection, recovery and stopping after disposal. File coverage uses a
test-only DataTransfer/File fixture with a genuine FileList: forbidden nonempty
values leave selection intact, empty values clear it, and disposed bindings leave
reseeded files unchanged. No production code or dependency/features changed.

All11 actual WASM browser tests pass, zero failed/ignored/filtered. Strict Clippy
for this WASM test target passes with Cargo/Rustc/Clippy selected from the same
installed rustup toolchain; an initial mixed-distribution invocation failed E0514
on cached host dependencies, resolved by coherent process-local tool selection
without cleaning or modifying installed tools. Scoped rustfmt/diff checks pass.
Evidence /tmp/crepus-reactive-controls-browser/ includes build, complete browser
output, initial/coherent Clippy logs. No physical file picker, framework app,
Safari/Firefox or heap-profile claim. No commits, pushes or remote mutations.
Goal remains active.

### 2026-10-18: Verify the WASM integration prerequisite mismatch

Follow-up to the two WASM integration failures: a dependency-free no_std Rust
metadata probe fails with E0463 using PATH rustc, but succeeds using the compiler
returned by rustup which rustc. PATH selects Homebrew Rust; rustup's installed
target inventory describes a separate toolchain. ~/.cargo/bin has neither cargo
nor rustc proxies, so the existing PATH-prepend policy cannot select them.
Evidence: /tmp/crepus-wasm-prereq-audit/{probe.rs,selected.log,rustup.log,
environment.log}. Independent source review confirmed the mismatch between
tests/web_cli.rs prerequisite inventory and wasm_bundle.rs effective selection.

No compiler override, installed-tool change or test exclusion was introduced.
The readiness helper still needs an effective-compiler probe rather than a
rustup inventory check; this is a verified remaining test-infrastructure issue.
Large WASM acceptance builds were not rerun with under1GiB free disk space.
No production fix or full integration-suite pass claimed. Goal remains active.


### 2026-10-18: JSX normalized EOF diagnostics stay on UTF-8 boundaries

Confirmed public diagnostic panic for incomplete `<div>｛x｝界`: full-width braces
shrink during JSX normalization, but its byte-offset map lacked an EOF entry.
Normalized EOF11 fell back unchanged into the original full-width closing brace;
diagnostics sliced at a non-character boundary. Baseline public regression
reproduced that exact panic. Added original source EOF as the final map entry,
with capacity and length invariant adjusted. Existing ordinary offsets stay
unchanged; no parser admission or AST behavior changes. Independent review checked
sole map consumer, empty input and BOM handling.

Retained public-path matrix covers CJK/astral suffixes, BOM, CRLF and combined cases,
asserting parse errors and exact original EOF UTF-16 diagnostic ranges. Full core
run exposed an unrelated stale svelte_modules negative `<select bind:value/>`;
current authored.rs and docs/frontends admit select binding. Preserved concurrent
Svelte work instead of modifying that contract test. The remaining full core
suite passes733checks with only authored_value_binding_boundaries_are_explicit
explicitly skipped; strict all-target core Clippy and scoped rustfmt/diff checks
pass. Actual task patches inspected. Evidence /tmp/crepus-jsx-eof-offset-fix/
contains beforefiles, baseline.log, tests.log (original full-run failure),
remaining-tests.log, clippy.log and scoped patches. LSP request/config review found
no additional confirmed issue. No live LSP/editor/browser execution claimed.
No commits, pushes, branches, dependencies, deployments, remote mutations or
cleanup. Goal active.


### 2026-10-18: Restore full core gate for supported Svelte select bindings

Revalidated the prior full-core blocker: authored_value_binding_boundaries_are_explicit
still rejected `<select bind:value/>`, while authored.rs and docs/frontends now
explicitly admit static select binding. Moved that one stale negative contract to
the existing positive marker test, covering shorthand/explicit targets across
single select, bare multiple and literal-empty multiple. Assert emitted binding
marker+target, retained select tag and multiple presence. All other negatives
remain unchanged; existing production tests still reject dynamic multiple forms.
No compiler/runtime admission changed. An initial assertion assumed select was
the first emitted token; corrected it to preserve the existing fragment wrapper.

Full core suite now passes734checks without exclusions; strict all-target core
Clippy, scoped rustfmt/diff checks pass. Independent review approved actual task
diff and confirmed retained rejection coverage. Evidence under
/tmp/crepus-svelte-select-contract-fix/ (before.rs, baseline.log, tests.log with
intermediate assertion failure, final-tests.log, clippy.log, task.patch).
This validates parser normalization/markers, not host Svelte/Moonshine DOM/SSR or
published-runtime availability. No commits, pushes, branches, dependencies,
deployment, remote mutation or cleanup. Concurrent production work preserved;
goal remains active.


### 2026-10-18: Generated docs mount stripping preserves nested names

Confirmed serve_docs_path repeatedly trimmed /docs, causing normal generated
/docs/docsmith.html and /docs/docs/guide.html to resolve flattened wrong files.
The socket regression baseline returned HTTP200 with a decoy body, demonstrating
why status-only checks miss this. Replaced repeated trim with single strip_prefix;
root/index fallback and canonical static resolver checks remain unchanged. Existing
pkg/islands mounts include a trailing slash and do not have this ordinary nested
path defect; no extra production changes made there.

Both docs cases now verify exact intended body over a bounded loopback socket
with distinct decoys. All145 slim CLI unit checks and strict all-target slim CLI
Clippy pass; scoped rustfmt/diff checks pass. Actual task diff inspected and
independently approved. Evidence /tmp/crepus-dev-route-prefix-fix/ contains before
source, baseline.log, tests.log, clippy.log and task.patch. No launched browser or
installed/global CLI verification claimed. A separate include symlink/frontend
identity candidate remains source-traced only and requires composed runtime
reproduction before changing cross-target path/cache semantics. No commits,
pushes, branches, dependencies, deployment, remote mutation or cleanup. Goal active.

### 2026-10-18: Preserve authored include frontend identity across targets

Reproduced a symlinked child.vue targeting payload.crepus selecting JSX instead
of Vue and rendering a false conditional branch. Ordinary includes now retain
the authored full filename for frontend selection while canonical paths still
govern filesystem reads, admission and nested include directories. A shared core
helper supplies parser identity; web, native, TUI, embedded, LVGL, GPUI and CLI
validation consume it. AST and prepared caches distinguish physical source and
frontend identity, including aliases in either order and compound Angular suffixes.
Named component parsing and virtual-source lifetime behavior remain unchanged.

Parallel target reviews and primary integration/diff review completed. All2,219
checks across the seven affected library crates pass, with14 existing ignored;
all146 slim CLI unit checks pass. Strict all-target Clippy for those libraries
and slim CLI, scoped rustfmt and diff checks pass. During verification, corrected
a missing include prop in a new TUI fixture and restored the existing GPUI
virtual-source Arc lifetime contract; no existing assertion was weakened.

Evidence: /tmp/crepus-include-frontend-identity-fix/ includes the exact failing
baseline, per-target regressions, full test and Clippy logs, and scoped before
snapshots. Unix symlink coverage does not establish Windows symlink behavior,
live devices, WASM or live GPU rendering. No performance or lag guarantee claimed.
No commits, pushes, branches, dependencies, deployment, remote mutation or cleanup.
Concurrent work preserved; goal remains active.

### 2026-10-18: LSP indentation comments suppress editor suggestions

Reproduced class completions and alias hovers inside full-line indentation
comments, which the parser discards. The shared cursor context now returns no
context after a leading comment marker in indentation mode. Markup handling,
tag IDs and cursor positions before the marker retain their existing behavior.
One regression covers whitespace/BOM comments, braces, aliases, following lines
and those retained boundaries. Baseline failed before the context fix.

All37 LSP library/binary tests and strict all-target/all-feature Clippy pass;
scoped formatting and actual task diff checks pass. Independent review checked
the scope and prompted explicit pre-marker coverage. Parallel plugin-bindgen
review produced no new verified finding; generic adapters were not regenerated.
Evidence: /tmp/crepus-lsp-comment-context-fix/ with before source, failing baseline,
final tests, Clippy log and task patch. No live editor verification, commits,
pushes, branches, remote mutations or cleanup. Goal remains active.

### 2026-10-18: Decode static dev-server asset URLs exactly once

Socket regression reproduced HTTP404 for an existing space-containing filename.
The shared static resolver now decodes percent escapes once before its existing
parent-segment and canonical containment checks. Spaces, Unicode and literal
percent filenames resolve correctly; plus remains literal. Invalid escapes,
non-UTF-8, NUL, encoded parent segments and encoded symlink escapes are rejected.
Static, docs, pkg and islands handlers share this resolver; routing prefixes and
template preview routing remain unchanged. No dependency added.

All147 slim CLI unit tests pass. The broader slim CLI run completed329 passing
checks but stopped at two existing-environment WASM build failures: selected
rustc cannot find core for wasm32-unknown-unknown (E0463). This does not establish
a full integration-suite pass. Strict all-target slim CLI Clippy and scoped
format/diff checks pass; primary and independent reviewer inspected task.patch.
Parallel embedded API review found no new concrete arithmetic/bounds defect.
Evidence: /tmp/crepus-dev-encoded-assets-fix/ contains before source, failing
socket baseline, broader test log, Clippy log and actual patch. No launched
browser/Windows verification, commits, pushes, cleanup or remote mutations.
Goal remains active.

### 2026-10-18: Browser worker failure and teardown follow-up

Reviewed current runtime-client pending-request rejection, listener removal and
one-shot termination; runtime-worker FIFO recovery and initialization caching;
app event capture/stop behavior and island/slot teardown. Independent event
review found no new concrete defect. A direct Chromium native-listener probe
confirmed target capture stopPropagation suppresses the target bubble invocation,
so that suspected delegation difference was rejected rather than patched.

All129 tests across web-runtime-client, web-runtime-worker, web-worker-app and
web-events pass (314 assertions). The actual Chromium Worker regression also
passes: uncloneable results reject, later FIFO calls recover without handler
replay, and initialization failure reaches later calls. Browser evidence at
/tmp/crepus-worker-cleanup-audit/browser.log. This uses a JavaScript runtime
fixture, not WASM; no heap-retention profiling, Safari or Firefox result claimed.
No production edits, dependencies, commits, pushes or cleanup this pass.
Goal remains active.

### 2026-10-18: Preserve special JSON keys in extension module output

Reproduced JavaScript object-literal __proto__ semantics dropping the own field
and selecting a prototype instead. BrowserProgram's module emitter now uses
computed keys for storage defaults/sets and message fields. Literal JSON objects
and arrays render recursively with computed object keys at every depth; scalar
emission is unchanged. No shadowable JSON.parse dependency is introduced.
The data interpreter already constructs storage/message objects safely and was
not changed. Its separate binding-name dictionary and expression sentinel
contracts remain outside this fix.

The new Rust regression failed before the fix; all48 webext tests and strict
all-target Clippy pass afterward, along with scoped formatting/diff checks.
A tiny linked Rust driver emitted the actual module, which Bun executed against
mock storage/runtime APIs: own __proto__ keys, normal object prototypes, nested
JSON arrays/objects and a binding named JSON all pass. One intermediate driver
attempt linked an incompatible serde_json artifact; the final driver infers the
public Value type and executes successfully. Primary and independent reviewer
inspected the actual diff. Evidence /tmp/crepus-webext-object-key-fix/ contains
before source, baseline, final test/Clippy logs, generator, emitted module,
execution check/log and task.patch. No real extension-host, WASM or browser
permission test claimed. No commits, pushes, dependencies or remote mutations.
Goal remains active.

### 2026-10-18: Extension data-program bindings use a prototype-free table

Reproduced the adjacent data-interpreter defect through the complete content
loader: binding __proto__ to42 returned Object.prototype instead, and an unbound
toString reference returned its inherited function. Changed the private vars
table to Object.create(null); indexed reads/writes and all other interpreter
semantics remain unchanged. This table has no prototype-method consumers.

The new loader regression covers storage/runtime bindings, constructor, missing
names and emitted statement values. It failed against the previous table and
passes after the one-line fix. All16 extension JavaScript tests pass with65
assertions. Primary/independent review and scoped diff checks pass. Evidence at
/tmp/crepus-webext-binding-table-fix/ includes source/test before snapshots and
baseline/final test logs. Host APIs and runtime imports are test doubles; no real
extension-host or WASM claim. No Rust rebuild needed for this JavaScript-only
change. No commits, pushes, dependencies or remote mutations. Goal remains active.

### 2026-10-18: Extension member expressions retain literal property identity

Confirmed JsExpr::Member serialized as a dotted variable name, causing the data
interpreter to read an unrelated literal binding instead of the object's field.
The module route also sanitized property punctuation, reading a_b for a-b.
Member serialization now uses {$member:[object,property]}, consumed by the bundled
content interpreter as indexed property access; module output uses a quoted
computed property. Plain $var dotted names remain exact bindings. Null/missing
objects retain ordinary JavaScript member failure semantics; no eval added.

Rust and full-loader JavaScript regressions failed before their respective fixes.
All49 webext Rust tests,17 JavaScript tests (67 assertions), strict all-target
Clippy, scoped formatting and diff checks pass. A linked public-API Rust driver
emitted actual module/data output; Bun execution confirmed literal a-b reads and
preserved member operands. Primary and independent reviewer inspected actual
diffs; emitter and bundled content asset were updated together. Evidence at
/tmp/crepus-webext-member-fix/. No real extension-host, WASM or cross-version
asset compatibility test claimed; rebuild generated extensions with matching
runtime/content assets. No commits, pushes, dependencies or remote mutations.

Separate source-confirmed candidate: module binding browserApi collides with its
generated parameter, and dollar/underscore names can sanitize to the same name.
Resolving hygiene while preserving raw-JavaScript references needs a separate
bounded design/regression pass. No speculative identifier rewrite included.
Goal remains active.

### 2026-10-18: Extension module API parameter avoids binding collisions

Fixed the confirmed browserApi parameter collision without renaming authored
bindings or raw expressions. The emitter chooses browserApi plus enough trailing
underscores to avoid every emitted binding name, then uses that parameter for
all four generated API call paths. No-conflict output retains its previous name.
The regression covers browserApi and browserApi_ bindings plus a raw reference.

Baseline regression failed; all50 webext Rust tests and strict all-target Clippy
pass after the fix. A linked public-API driver emitted the actual module; Bun
import/execution confirms parsing, storage values, runtime replies and preserved
raw references. Primary/independent actual-diff review, formatting and diff checks
pass. Evidence: /tmp/crepus-webext-parameter-fix/ with before source, baseline,
tests/Clippy logs, generator, emitted module, execution check/log and task.patch.
General sanitized-name collisions (such as dollar/underscore), reserved names and
raw-expression validation remain separate. No extension-host or WASM verification,
commits, pushes, dependencies or remote mutations. Goal remains active.

### 2026-10-18: Preserve dollar signs in extension JavaScript identifiers

Fixed the source-confirmed $value/_value collision by allowing the valid dollar
identifier character in the existing shared sanitizer. Binding declarations,
variable/member-object references and API-parameter collision checks remain
consistent; raw expressions are unchanged. No general identifier remapping or
new dependency introduced. Invalid leading characters/reserved words and other
lossy-name collisions remain separate, not claimed fixed.

The new regression fails before the one-line fix and covers distinct declarations,
variable values and a raw dollar reference. All51 webext Rust tests and strict
all-target Clippy pass; linked public-API emitted-module execution returns distinct
dollar7/underscore9/raw7 values. Primary/independent review and scoped format/diff
checks pass. Evidence /tmp/crepus-webext-dollar-ident-fix/ contains before source,
baseline, tests/Clippy logs, generator, emitted module, execution check/log and
task.patch. No real extension-host/WASM verification, commits, pushes or remote
mutations. Goal remains active.

### 2026-10-18: WASM public types include image/link URL bindings

Parallel WASM review found the public ViewNode union omitted optional srcBind and
hrefBind fields present in current native IR. Added those two optional string
properties without runtime/schema changes. A strict TypeScript consumer narrowed
image/link nodes and failed with TS2339 before the fix; it passes afterward.
A tiny linked current-native driver confirms actual serialized srcBind:photo and
hrefBind:destination, matching the source lowering/serde contract. All65 packaged
parser tests pass (255 assertions); scoped actual diff checks pass.

The packaged WASM binary is stale for this feature: the same bound URL example
resolves its URLs but omits binding metadata. The optional declarations remain
compatible with that binary; the parser tests do not establish binding parity.
WASM rebuild deferred with under400MiB free disk; no generated artifacts updated.
ABI source review found no additional confirmed ownership/atomicity defect.
Evidence: /tmp/crepus-wasm-url-binding-types-fix/ contains before types, failing
consumer/typecheck logs, packaged test log and packaged/current-native JSON.
No full workspace, browser/device or rebuilt-WASM verification; no commits,
pushes, dependencies or remote mutations. Goal remains active.

### 2026-10-18: Complete public IR type-surface comparison

Primary and independent comparison found no further omissions after the URL
binding-field fix. All82 current Rust ViewStyle fields exist under their serde
camelCase names in TypeScript; only the two explicitly deprecated fraction
compatibility fields are extra. All25 ViewNode kinds and node payloads, LabelPart,
PickerOption and TabItem were reviewed for field names, types and optionality.
Optional TS representations of defaulted native booleans/collections remain
compatible with input/default behavior and were not tightened needlessly.

Generated temporary strict TypeScript consumers from the actual Rust kind/style
names. Exhaustive Record<ViewNodeKind,true> and reverse kind assignments pass,
as does the complete style-key union check. Evidence under
/tmp/crepus-wasm-ir-type-audit/ includes consumers, summary.json and typecheck.log.
This is source/type-surface evidence, not rebuilt-WASM serialization parity;
the packaged binary staleness recorded above remains. No production changes,
dependencies, commits, pushes, compiler changes or remote mutations this pass.
Goal remains active.

### 2026-10-18: Native action-queue and host lifetime acceptance follow-up

Ran scripts/test-native-action-queue.py against current generated and snapshot
Swift/Kotlin source, including the installed routing helpers, async editing,
scope JSON and picker probes. Entire gate passes: FIFO/exactly-once dispatch,
single active worker, preceding result committed before next action, main-thread
sinks, blocked-worker main heartbeat, captured hooks, interrupted deliveries,
rejected posts, reentrant edits, transformed acknowledgments and picker partial
failure/cleanup. Logs: /tmp/crepus-native-dispatch-audit/checks.log.

These probes compile actual extracted queue/state/helper code with labelled
Android/Compose/FFI collaborators; they do not establish real device UI, JNI,
SwiftUI lifecycle or end-to-end Rust FFI operation. Independent read-only Tauri
review found no additional confirmed event/listener/state/invoke lifetime defect;
documented cancellation and synchronous-runtime limits remain. Bounded scalar
review found existing finite checks on Android float conversion. No production
edits, compiler/runtime changes, commits, pushes or remote mutations this pass.
Goal remains active.

### 2026-10-18: C# fixed-width scalar ABI correction

Parallel plugin review reproduced generated uint32_t and float signatures using
IntPtr for both arguments and returns. A typed C# consumer rejects saved old
output with CS0123. Replaced the permissive C# renderer with the existing shared
typed-header admission path and explicit fixed-width integer/floating mappings;
pointers and callbacks remain IntPtr and imports retain Cdecl. Ambiguous by-value
platform types such as C long now fail before output rather than silently mapping
to pointers. Library names use escaped C# verbatim literals and method identifiers
are escaped. No dependency added.

Added fixed-width signature compilation and platform-long rejection coverage.
All plugin-bindgen tests pass (25 passed, three existing real-Rust-ABI tests
ignored); all four C# tests pass, including compilation of the real 13-export
header. Strict all-target Clippy and scoped rustfmt pass. Fresh bindings generated
through the public Rust API also execute against an independent C shared library:
uint32/high-bit values, float, double, int64 and uint64 round trips pass under
.NET 8. Evidence: /tmp/crepus-csharp-scalar-fix/. Reviewed own before/current
diffs; preserved concurrent changes. No whole-workspace, Windows, or full real
Rust ABI C# execution claim. No commits, pushes or remote mutations.
Goal remains active.

### 2026-10-18: C# NUL regression acceptance and smoke-gate integration

Revalidated pending C# SetTemplate NUL guards against a freshly rebuilt current
Rust ABI dylib. Existing regression rejects leading/middle/trailing template NUL
and base-directory NUL without replacing previous IR, verifies Unicode include
paths/text, and confirms subsequent session usability. Managed build passes with
zero warnings/errors; real native smoke passes. Added --abi-nul to the existing
plugin-smoke.sh gate, reusing its exported CREPUS_ABI_LIB and the fixture's
concurrent environment-based native-library resolver. Removed overlapping local
resolver changes after a compile failure exposed concurrent edits; final build
and execution are green. Shell syntax and affected diff whitespace checks pass;
primary inspected before/current wrapper and dispatch diffs and the fixture.
Evidence: /tmp/crepus-csharp-abi-nul-fix/{abi-build,final-build,final-test}.log.
Initial shell-based DYLD_LIBRARY_PATH invocation could not resolve the library;
explicit fixture resolver succeeds without global environment changes. The
entire multi-language smoke script and Windows were not run. No commits, pushes
or remote mutations. Goal remains active.

### 2026-10-18: Fresh typed C# backend against the complete real Rust ABI

Current ABI dylib passes existing managed lifecycle/disposed-before-serialization
and checked-in generated-binding 13-export acceptance fixtures. To avoid treating
stale checked-in bindings as proof of the replacement C# renderer, generated a
fresh C# file through the public bindgen API from a byte copy of the current real
header. Compiled and ran the existing full 13-export fixture against that new
file with warnings as errors. Only fixture resolver library name changes to match
the temporary header stem; all native calls target the freshly rebuilt actual
Rust dylib. PASS includes UTF-8 ownership, context/patch/files/component state,
nullable rooted Cdecl callback and userdata, signed errors and consumed errors.
Evidence: /tmp/crepus-csharp-fresh-abi-acceptance/check.log and generated source;
managed lifecycle logs under /tmp/crepus-csharp-abi-nul-fix/. This extends the
earlier scalar C-stub check with real Rust session behavior. No production edits
or generated-package refresh; concurrent tracked bindings preserved. Independent
LVGL XML re-review confirms prior character/name fixes, with no new defect found.
Windows, full multi-language smoke and workspace checks remain unrun. No commits,
pushes or remote mutations. Goal remains active.

### 2026-10-18: D/Nim/Odin admit ordinary C floating-point declarations

Independent review reproduced explicit generation failures for C float in all
three typed adapters. The shared binder produces c_float/c_double, while their
existing floating mappings only admitted f32/f64. Added those two aliases in
each adapter, retaining existing native float/double representations and error
handling. Extended existing selected-header tests with float/double arguments
and returns; original integer/pointer signatures remain checked. No dependencies
or compatibility fallback added. Baseline CLI probe inputs are preserved under
/tmp/crepus-typed-float-review/.

All 25 bindgen tests pass, with three existing real-Rust-ABI cases ignored.
D ldc2, Nim check/C compilation and Odin type checking validate the generated
float/double signatures. Strict all-target Clippy, scoped formatting and diff
checks pass. Primary inspected worker before/current patch across all six files.
Evidence: /tmp/crepus-typed-float-fix/{tests,clippy}.log and task.patch. This is
compiler/signature evidence, not floating runtime round-trip proof for these
languages or Windows validation.

Separately, all 735 current default-feature core tests pass, including the
2,000-case malformed Unicode stress test across six frontend filenames. A
bounded Angular UTF-8/EOF source review found no new concrete failure. Evidence:
/tmp/crepus-core-current-audit.log and /tmp/crepus-parser-robustness-current.log.
No full-feature/workspace claim, commits, pushes or remote mutations.
Goal remains active.

### 2026-10-18: D/Nim/Odin floating runtime and real ABI acceptance

Generated fresh D, Nim and Odin imports through the current public bindgen API
from the real ABI header plus float/double exports. Built an independent C
dylib implementing distinct nonidentity float and double arithmetic. Compiled
and executed callers using ldc2, Nim/C and Odin against that exact library; all
three return expected results for both argument/return widths. Generated source
was used unchanged. Initial D harness link flags used unsupported macOS
-rpath= syntax; corrected to separate linker arguments, then every runtime
passes. Evidence and commands: /tmp/crepus-typed-float-runtime/checks.log.

Also explicitly enabled all three previously ignored bindgen real-Rust-ABI
acceptance tests using CREPUS_ABI_LIB pointing to the freshly built current
dylib. D, Nim and Odin each pass their complete 13-export session lifecycle,
callback and error contract checks; no remaining ignored cases in this explicit
run. Evidence: /tmp/crepus-typed-float-runtime/real-abi.log. This supplements
the default suite rather than changing ignore policy or tracked generated
packages. Independent TUI review found no cursor-index bug: this crate forwards
key/paste events and hosts own text editing. No production edits, dependencies,
commits, pushes or remote mutations. Windows and full workspace remain unrun.
Goal remains active.

### 2026-10-18: Web textarea CR and leading-newline preservation

Independent SSR review reproduced textarea data loss for bound source
\r\nfirst\rsecond: actual Chromium value became first\nsecond, losing
the leading logical newline as HTML normalizes raw CRLF before applying its
initial-textarea-newline rule. Shared textarea content serialization now encodes
CR as &#13; after escaping and before existing leading-LF compensation. Bound
value and interpolated child paths share the fix, including hydration output.
No dependencies, global escape changes or literal-entity double decoding.

Regression covers five CR/CRLF/LF/entity/markup cases across value/child and
ordinary/SSR/hydration renderers. All 182 all-feature web tests/docs pass; strict
all-feature/all-target Clippy, formatting and scoped diff checks pass. Primary
inspected actual before/current two-line renderer change and focused tests.
Actual Chromium checks on 20 fresh Rust-generated HTML outputs prove textContent
and defaultValue preserve source, while textarea.value follows normal browser
CR-to-LF normalization. Baseline browser failure evidence:
/tmp/crepus-web-textarea-cr-review/browser-before.log. Fix/checks/browser source:
/tmp/crepus-web-textarea-cr-fix/. No claim that textarea.value retains raw CR.

Also revalidated current runtime: 94 ordinary tests pass, then all 11 normally
ignored Metal tests pass serially (105 total). Hot-reload request publication,
watcher lifecycle and recovery source review found no new concrete issue. Logs:
/tmp/crepus-runtime-current-audit.log and /tmp/crepus-runtime-current-metal-audit.log.
These are host/headless Metal checks, not interactive desktop or Windows proof.
No commits, pushes or remote mutations. Goal remains active.

### 2026-10-18: Preserve CR in web binding attributes and form submissions

Independent review and Chromium FormData reproduce hidden input and selected
option values changing row\rkey to row\nkey through raw HTML newline
preprocessing. Shared push_bindings now encodes CR after HTML escaping. URL
admission, boolean omission and text serialization retain their existing paths;
the previous textarea fix is preserved. Tests cover CR, CRLF and literal entity
text plus HTML punctuation across hidden/option, ordinary/SSR/hydration output.

All 183 all-feature web checks and strict all-target/all-feature Clippy pass;
scoped formatting/diff checks and primary before/current patch inspection pass.
Six actual Rust-rendered HTML forms also pass Chromium FormData checks, retaining
exact CR/CRLF/entity values for both controls. Initial standalone probe selected
incompatible cached core/web rlibs; corrected probe infers the context through
the web API and links only that crate. No production toolchain change. Evidence:
/tmp/crepus-web-attribute-cr-review/browser-before.log and
/tmp/crepus-web-attribute-cr-fix/{tests,clippy,browser-after}.log with driver and
scoped.patch. This covers generic binding attributes, not every custom metadata
serializer. Independent native numeric-control review found no new validated
defect. No Windows/other browser or full workspace claim; no commits, pushes or
remote mutations. Goal remains active.

### 2026-10-18: Browser worker lifecycle and numeric evaluator re-review

Independent bounded worker review traced fatal pending-request rejection,
listener removal/termination, bootstrap failure disposal, FIFO event/render
transactions, and stale root/scope/bundle generation guards. No new concrete
cleanup or stale-response failure found. Primary inspected current client/queue
implementations and ran all three worker client, queue and app Bun test files;
results recorded in /tmp/crepus-worker-current-tests.log. These exercise actual
JS modules with labelled transport/DOM collaborators; they are not new real
browser/WASM performance proof or a no-lag guarantee.

Independent numeric evaluator review checked finite/range validation, checked
integer arithmetic, MIN remainder/negation boundaries, mixed comparison
precision, conversion bounds and short-circuiting against current regression
coverage. No new confirmed panic or wrong numeric output found; the preceding
735-test core gate remains the applicable runtime evidence. Native lite worker
source inspection confirms synchronous host requests and joined shutdown; no
unbounded-script cancellation guarantee is inferred. No production edits,
commits, pushes or remote mutations. Goal remains active.

### 2026-10-18: Imba literal compiler/browser acceptance and LSP state review

Independent Imba escaping review found no concrete new defect. Closed its
unverified Unicode-separator boundary by emitting a component through the native
public emitter API, compiling with installed Imba 2.0.0-alpha.253, bundling with
Bun, and mounting in actual Chromium. Exact textContent and aria-label match
source containing U+2028/U+2029, NUL/C0, CRLF, braces resembling interpolation,
quotes, backslash, CJK and supplementary Unicode. No browser errors. Artifacts
and logs: /tmp/crepus-imba-literal-acceptance/. Initial shared temporary
node_modules link lacked the runtime package; the isolated fixture now links
the already installed cached Imba package directly. No installation or changes
to installed runtimes. This is one literal-heavy component/compiler/browser
case, not full Imba source compatibility or other browser proof.

Independent bounded LSP state review checked unopened/stale/equal-version
changes, last full replacement, close-time empty diagnostics publication under
lifecycle ordering locks, and file-URI decoding. No supported-flow defect found;
existing FULL synchronization contract excludes incremental client updates.
No new protocol session or editor-host check this pass. No production edits,
commits, pushes or remote mutations. Goal remains active.

### 2026-10-18: LVGL shares the current framework color palette

Independent review found LVGL still hardcoded an older tiny palette: actual
public render output used blue-500 #3b82f6 and red-500 #ef4444 while other
updated targets use shared #2b7fff/#fb2c36. Replaced named-color hardcodes with
the existing first-party crepuscularity-colors lookup, adding its path/version
dependency. Black/white six-digit spelling, transparent fallback, arbitrary hex
and unknown-name behavior remain unchanged. Existing color assertions now
expect corrected shared values. New tests cover 26 families × 11 shades across
background/text/border, plus special and unknown colors.

All 76 LVGL checks pass with one existing ignored case; strict locked all-target
Clippy, scoped formatting and diff checks pass. Primary inspected the worker
patch and lockfile delta (only the LVGL colors dependency). A newly linked
public renderer probe emits corrected colors and its XML parses with independent
ElementTree. Baseline old-output artifact:
/tmp/crepus-lvgl-palette-baseline/output.xml; fix/checks/new output:
/tmp/crepus-lvgl-shared-palette-fix/. Shared colors and component registry tests
also pass (eight checks total); bounded registry review found no new defect.
No physical LVGL/Pro import or full workspace validation. Existing LVGL color
users will see the intentional palette correction. No commits, pushes or remote
mutations. Goal remains active.

### 2026-10-18: Embedded color and no-std Cortex-M acceptance

Follow-up scan found no remaining old-palette tables among the reviewed Rust
renderers; CLI accent/error-display literals are product defaults rather than
Tailwind named-color mappings. TUI intentionally maps classes to terminal ANSI
families instead of shared truecolor values. Independent embedded color review
confirmed ASCII/length checks precede hex byte slicing, iterative arbitrary
bracket stripping, and shared named-palette delegation; no new concrete defect.

Ran current embedded --no-default-features --lib tests: 32 pass, including
malformed Unicode hex, deep bracket nesting and exhaustive 65,536 RGB565
round trips. Shared colors and embedded both pass locked no-std cross checks for
thumbv7em-none-eabihf using the existing coherent stable compiler, selected only
for those processes. Logs: /tmp/crepus-embedded-nostd-current-tests.log,
/tmp/crepus-colors-thumb-audit.log and /tmp/crepus-embedded-thumb-current-check.log.
No hardware execution, display timing, STM32 HAL build or full workspace claim.
No production edits, commits, pushes or remote mutations. Goal remains active.

### 2026-10-18: Cached-file invalid UTF-8 and repair regression

Bounded core file-loading review found no new BOM/encoding/cache defect:
read_to_string runs before successful content-cache admission and errors retain
the source path. Added the missing invalid-UTF8 recovery assertion to the
existing in-place/atomic replacement cache regression. After a successful cached
parse and deletion, invalid file bytes must produce Io(InvalidData), then a
repaired template must parse as new content. This verifies observable freshness
and recovery without changing production behavior or cache policy.

All five file_cache_bounds tests, strict focused Clippy, scoped formatting and
diff checks pass. Primary inspected the exact 11-line test-only diff against
before snapshot. Evidence: /tmp/crepus-cache-utf8-recovery/{tests,clippy}.log and
before.rs. Existing full-core gate remains separate earlier evidence; no new
whole-workspace or filesystem-watcher runtime claim. No commits, pushes or
remote mutations. Goal remains active.

### 2026-10-18: ABI rejects unrepresentable JSON events before dispatch

Independent native ctypes probe reproduced valid event JSON containing 1e400
returning success and calling the event callback with the entire JSON as its
action name. parse_event treated every serde Value parse error as a legacy raw
name. It now distinguishes syntactically valid JSON through RawValue on that
error path: unrepresentable values return an event JSON error before context
updates, bindings or callbacks. Syntactically non-JSON raw names retain existing
behavior, including historical malformed-looking raw strings. README documents
the numerical rejection contract. No new dependencies or generic prefix checks.

New FFI regression checks context and payload positive/negative overflow, no
callbacks, unchanged IR and subsequent raw/quoted dispatch. All 13 ABI tests
pass; strict all-target Clippy, scoped formatting and diff checks pass. Primary
inspected before/current parser/test patch and documentation edit. Rebuilt
actual dylib repeats the same ctypes probe: null result, no callbacks and
number-out-of-range error, instead of baseline success. Evidence:
/tmp/crepus-abi-event-overflow-fix/{baseline-ctypes,final-ctypes,tests,build,clippy}.log
with probe.py and scoped.patch.

Separately current webext acceptance passes 51 Rust checks and 17 JS checks
(67 assertions); manifest source review found no new verified defect. Logs:
/tmp/crepus-webext-current-acceptance.log and /tmp/crepus-webext-current-js.log.
No real installed-extension/Windows/full-workspace claim. No commits, pushes
or remote mutations. Goal remains active.

### 2026-10-18: Python dictionary events reject nonfinite JSON numbers

Actual managed-wrapper/native probe found Python dispatch_event dictionaries
serialize Infinity/NaN using json.dumps' permissive default. Those non-JSON
objects reach the intentionally retained raw-handler fallback and dispatch an
unrelated action containing the entire serialized object. Dictionary event
serialization now sets allow_nan=False; raw string inputs retain their contract.
No native parser heuristic or dependency added.

New real-ABI regression fails before the one-line fix (seven failures including
wrong accepted events) and passes after it. Tests cover NaN and both infinities
nested in payload/context, no callbacks, exact unchanged IR and subsequent finite
event dispatch. All eight Python ABI wrapper tests pass against the current
real dylib. Primary inspected exact before/current production and test diffs;
scoped whitespace check passes. Evidence:
/tmp/crepus-python-event-finite-fix/{baseline,tests}.log and before snapshots.
Independent virtual-file/session transition review found no new atomicity issue.
No full multi-language smoke, Windows, commits, pushes or remote mutations.
Goal remains active.

### 2026-10-18: GPUI macro diagnoses malformed dynamic color expressions

Independent macro review found bg-{1 +} accepted with its color silently
omitted: optional style mapping swallowed syn expression errors. Added a
fallible color-expression preflight in element generation across ordinary and
conditional classes, respecting existing target/state admission and alpha forms.
Unknown literal utilities and foreign-target classes retain existing behavior;
no broad fallible-mapper refactor or runtime work added.

Tests-first regression fails preserved baseline with an accepted empty-style
token stream. Tests cover bg/text/border, plain/alpha/bracketed alpha, state and
conditional forms, valid authored expressions and ignored literal/foreign cases.
Full package passes 32 unit tests plus the actual downstream template-dependency
rebuild test; four existing doc examples remain ignored. Strict all-feature/
all-target Clippy, scoped formatting/diff checks and primary actual patch review
pass. A separate rustc consumer using the built proc macro emits the intended
Invalid color expression diagnostic. Evidence:
/tmp/crepus-macro-dynamic-color-fix/{baseline,tests,clippy,consumer}.log and
scoped.patch. No live GPUI application or other OS claim.

Separate queued finding: current crepus ios new my.app succeeds but emits
struct My.appApp: App, an invalid Swift identifier. Repro preserved under
/tmp/crepus-ios-identifier-review/. No scaffold fix attempted in this pass;
project-name compatibility requires a separate bounded decision/change.
No commits, pushes or remote mutations. Goal remains active.

### 2026-10-18: iOS project names produce valid Swift identifiers

Fixed queued scaffold bug: crepus ios new my.app preserved the directory but
emitted invalid struct My.appApp. Swift frontend parsing confirms baseline
failure. iOS now derives a local Swift stem separately from authored directory
and display text, replacing punctuation with separators before existing Pascal
conversion and using Crepus for an empty derived stem. Existing identifier-safe
Unicode, separators and leading-digit prefix behavior remain. Other scaffold
name policies/global helper are unchanged.

Added helper cases and actual dotted-name CLI regression. All 148 slim CLI unit
tests and six iOS integration tests pass; strict slim all-target Clippy, scoped
formatting/diff checks and primary patch inspection pass. Ten actual rebuilt
CLI scaffolds (dot/quote/newline/digits/Unicode/combining/separators/emoji) preserve
authored directories, parse TOML schemes matching emitted types, and pass Swift
frontend parsing. A separate exact-source helper harness also passes Swift
parsing. This is syntax/project-generation evidence, not an iOS SDK build or
simulator run. Artifacts: /tmp/crepus-ios-identifier-fix/; baseline Swift error:
/tmp/crepus-ios-identifier-baseline/swift.log.

Initial Cargo baseline did not complete as disk dropped below 200 MiB; no test
red is claimed (its filter also mismatched). After disk recovered above 2 GiB,
full relevant gates completed. Standalone harness initially selected mixed
compiler cached unicode-ident, then used its matching installed stable rustc;
no cleaning or global compiler changes. No commits, pushes or remote mutations.
Goal remains active.

### 2026-10-18: Facade web output matches manifest directory contract

Independent manifest review found documented shared web out semantics diverged:
CLI treats out as a directory, but Rust facade wrote HTML directly to that path,
creating a file that blocks later directory builds or erroring for an existing
directory. Facade build_web now maps configured out to out/index.html; absent
out remains a returned-only artifact. Native/LVGL output-file semantics retain
their existing paths. No extension heuristic or asset/package build added.

Tests-first public write_manifest_file_target regression fails baseline on the
missing index.html path. After the fix all four facade tests pass, including
absent/existing output directories, old index replacement, unrelated asset and
source retention, manifest-relative resolution from outside cwd, and absent out.
Strict all-target Clippy, scoped formatting/diff checks and primary actual
source/docs patch inspection pass. Docs select a supported site target from the
mixed manifest and explain render-only HTML, output paths and no WASM/assets.
Evidence: /tmp/crepus-web-manifest-output-fix/{baseline,tests,clippy}.log and
scoped patches. Existing facade callers that treated web out as a file must use
the documented directory semantics; this is an intentional correction. No full
WASM build or workspace gate.

Independent Android scaffold-name review found no iOS-like interpolation bug:
name only selects a destination directory; generated Kotlin/config are static.
No Android compiler/device proof claimed. No commits, pushes or remote mutations.
Goal remains active.

### 2026-10-18: Named target selection rejects duplicate matching IDs

Review found inconsistent ambiguous selection: CLI chose first matching ID,
facade built every match, and positional CLI selectors swallowed selection
errors before type fallback. Explicit named selection now rejects a second
matching ID in CLI general/web selectors and facade before building/writing.
The positional caller routes any matching ID through the fallible ID selector,
retaining type fallback only when no ID matches. No-selector build-all order
and unique/missing behavior remain unchanged; no output-collision policy added.
Docs state the named-selector requirement.

All three tests-first regressions fail baseline. Final five facade and 150 slim
CLI unit tests pass. Rebuilt actual CLI positional, --target and web selection
all report ambiguity before missing-template reads or output modification;
sentinel output stays intact. Strict combined facade/slim CLI all-target Clippy,
scoped formatting/whitespace and primary actual patch review pass. Two trailing
blank lines from worker formatting were removed before final checks. Evidence:
/tmp/crepus-ambiguous-target-fix/ includes baseline/final logs, caller probe
output and scoped patches.

Separate observed boundary remains: explicitly building all targets that share
an output FILE can overwrite in manifest order; rejecting shared directories
would break valid framework emission and requires a separate output policy.
No full WASM build, Windows or workspace claim. No commits, pushes or remote
mutations. Goal remains active.

### 2026-10-18: Reproduce build-all file collision and audit path identity

Confirmed remaining collision through both public facade write_manifest_file
and rebuilt CLI build --all: two native targets with distinct IDs and the same
out=dist/shared.json both report success; final output contains only the second
template. Fixture uses temporary local files and actual public render/write
paths. Logs/driver: /tmp/crepus-output-collision-audit/reproduction.log and probe.rs.
No collision guard is claimed this pass.

Independent path-identity investigation found no reusable Core output resolver.
Closest existing CLI absolute_resolved helper resolves existing ancestors and
symlinks but rejects missing suffix ..; moving it unchanged into shared code
would newly reject currently valid output paths. Lexical collapsing before
resolving existing symlinks can instead identify the wrong destination. Any
shared guard must handle these semantics and distinguish known output FILES
from framework output directories; broad directory uniqueness would reject
valid shared-directory workflows. This investigation narrows the next concrete
implementation to a common output-path identity/checking helper, with alias,
missing-parent, symlink-parent and untouched-sentinel regressions before writes.
No production edits, cleanup, commits, pushes or remote mutations.
Goal remains active.

### 2026-10-18: Reject primary output-file collisions before writes

Fixed the reproduced build-all overwrite through a shared Core output-path
preflight used by facade write_artifacts and CLI manifest builds. Path identity
resolves existing symlinks before later .. components, normalizes missing
suffixes, rejects non-directory ancestors/loops, and compares destinations
without writing. CLI preflights known primary files (web index.html and native/
IR/LVGL/embedded explicit/default files) before starting any selected target;
render writers reuse the same path calculation. Distinct files in a shared
directory remain valid. No new dependency or framework abstraction.

Facade tests-first collision regression fails old writer with Ok after writes.
Original public facade/CLI overwrite reproduction remains under
/tmp/crepus-output-collision-audit/. New actual CLI tests cover identical paths,
missing/../ aliases, symlinked parents, untouched sentinel and absent earlier
output, plus distinct native/LVGL files sharing a directory. Final review caught
a compatibility issue with the first candidate: a direct output symlink may
legitimately point to a missing file. Resolver now follows relative/absolute
read_link targets with a shared 40-link limit; actual CLI test proves missing
file creation succeeds and preserves the link. Initial candidate's dangling-link
rejection is not the shipped local behavior.

Final combined tests pass 655 checks: 492 Core units, six facade tests, 153 slim
CLI units and four real CLI integrations. Strict combined all-target Clippy,
scoped formatting/diff checks and primary actual source/integration/docs diff
review pass. Independent final resolver review found no blockers. Artifacts:
/tmp/crepus-output-collision-fix/ includes final-tests.log, final-clippy.log,
baseline/first-pass logs and scoped snapshots/patches.

Scope is known primary path collisions, not hard links, webext multi-asset
collisions, all batch-I/O failures or concurrent filesystem mutation protection.
Windows/WASM/full workspace and live browser/device builds were not rerun.
No commits, pushes, cleanup or remote mutations. Goal remains active.

### 2026-10-18: Extension page clients cannot overwrite runtime hosts

Actual successful CLI build with pages/background.crepus overwrote required
src/background.js with a DOM page client. Builtin options/content hosts were
also vulnerable to corresponding page names. Generated page clients now use
src/__crepus_pages/{entry}.js through the shared output/HTML path helper; JS
vendor imports derive depth from that new script location. HTML page locations
and built-in/custom host paths remain unchanged. Documentation describes the
generated namespace. Production change is two lines, with existing nested-path
expectations updated and a root/options/content host-preservation regression.

Tests-first write_runtime_assets regression fails baseline on overwritten
background bytes. Baseline public build evidence is preserved under
/tmp/crepus-webext-page-collision-baseline/. Final 154 slim CLI unit tests and
five actual extension-output integration tests pass, as do strict all-target
Clippy, scoped formatting/diff checks and primary actual patch inspection.
Actual CLI build also preserves custom background bytes and generates all four
root/nested client files. Chromium loads actual generated root and nested pages
and executes clients with a labelled runtime collaborator, proving relative
HTML/script/vendor paths; no WASM or installed-extension runtime claim. Logs:
/tmp/crepus-webext-page-namespace-fix/{baseline,tests,clippy,cli-build,browser}.log
and task.patch. Generic authored-asset collision detection remains separate.

Also closed the previous shared output_paths module's WASM compile boundary:
current Core passes locked wasm32-unknown-unknown check with the existing
stable compiler. Log /tmp/crepus-output-paths-wasm-check.log. No global compiler
changes, dependencies, commits, pushes or remote mutations. Goal remains active.

### 2026-10-18: Extension page stems and runtime export literals survive generation

Actual CLI accepted my page.crepus but emitted runtime.my page_main, rejected
by Bun's JS parser. Dotted pages options.v1/options.v2 also both generated
options.html because rendering stripped .crepus before replacing another
extension; repeated .crepus suffixes were over-trimmed in manifest conversion.
Fixed output generation to replace only the final extension, manifest mapping
to strip only the final suffix, and runtime main/render lookups to use serialized
quoted property names. Legacy hyphen-to-underscore export naming is preserved.
Docs show final-extension replacement examples.

Both tests-first regressions fail baseline. Final 155 slim CLI units, five
actual extension-output integrations and 52 webext tests pass (212 total), with
strict combined all-target Clippy, formatting and primary actual diff review.
Actual CLI generates six distinct dotted/repeated/spaced/hyphenated page outputs;
all generated clients pass Bun parsing. Chromium exercises quoted main exports
and renderer fallback on those actual generated pages with a labelled runtime
collaborator. No errors; no real WASM/installed-extension claim. Baseline compiler
and flattening logs under /tmp/crepus-webext-page-filename-review/; new checks
and patches under /tmp/crepus-webext-page-filename-fix/. General HTML filename
escaping/URL-special-character handling was outside this scoped change.
No commits, pushes or remote mutations. Goal remains active.

### 2026-10-18 — extension page URL and title escaping

Parallel CLI review reproduced successful builds with quoted and query-character
page filenames whose generated script attributes were broken or addressed the
wrong URL. The shared page script URL helper now percent-encodes UTF-8 bytes,
preserving structural separators and unreserved characters. Existing HTML escaping
protects the final attribute and composed title. Physical filenames stay unchanged;
`#` component-selector rejection is unchanged.

Tests-first regression failed baseline. Final 156 slim CLI unit tests and five
actual-output integration tests pass, as does strict all-target slim CLI Clippy.
Primary inspected the scoped patch and formatting. Five actual CLI-generated pages
(quote, query, literal percent escape, space and Unicode) load their clients in
Chromium and preserve literal title entities using a labelled runtime stub.
No real WASM or installed-extension proof. Evidence: /tmp/crepus-webext-page-url-fix/.
Parallel bounded Core/LSP and native lifecycle source reviews found no new confirmed
defect; TUI registered-file include provenance remains a candidate awaiting a test.
No commits, pushes or remote mutations. Goal remains active.

### 2026-10-18 — browser-program inline literal safety and TUI provenance

Parallel review identified a public composition bug: BrowserProgram emitted literal
`</script>` directly into widget build_frame_doc's module script. Tests-first
regression fails baseline; Chromium confirms premature script termination, syntax
error and injected following markup execution. A local JSON-string serializer now
escapes `<` as `\u003c` across recursive literal values/keys, storage keys, message
keys and member properties. Runtime string values are unchanged. Raw JavaScript,
raw CSS and arbitrary widget HTML remain caller-authored; their boundary is documented.

All 55 webext tests and strict all-target Clippy pass. Chromium on an actual
Rust-generated frame document now logs the exact mixed-case closing-tag/comment/
Unicode string, without errors or injected markup execution. Primary inspected the
scoped diff and formatting. Evidence: /tmp/crepus-webext-script-literals-fix/.
No installed-extension/CSP acceptance claim; CLI external page scripts are unaffected.

TUI file-registration include provenance is independently reproduced: registered
component siblings fail under an empty caller base or select a caller's different
sibling. Existing 147 TUI library tests pass. Added a passing in-memory caller-base
control and an explicitly ignored known-defect regression (baseline run failed).
Fixing automatic provenance requires retained state, but ComponentRegistry and
Component currently permit public struct literals. Compatibility decision requested;
no production TUI change made. Evidence: /tmp/crepus-tui-registry-include-fix/.
No commits, pushes or remote mutations. Goal remains active.

### 2026-10-18 — LVGL binding expression ownership

Parallel LVGL review found a second brace-removal pass after shared parsing. Actual
baseline `input value={{name}}` renders VISIBLE instead of preserving the shared
evaluator's error for `{name}`. Removed the private unwrapping helper and evaluate
stored AST expressions unchanged in generic values, hidden/disabled, checked,
integer bounds/step and form binding. Quoted/bare bind-path decoding is retained.
Docs clarify the single parser-owned delimiter removal.

Tests-first regression fails baseline. Eight invalid binding paths now return the
exact core evaluator error; inactive branches remain lazy, and ordinary bindings
and literal bind paths retain output. All 78 LVGL tests pass (one existing manual
scaling test ignored); final 11 library tests rerun after simplification. Strict
all-target Clippy, formatting and primary scoped diff inspection pass. Evidence:
/tmp/crepus-lvgl-binding-expression-fix/. No LVGL hardware/firmware validation.

Current reactive host suite independently passes 87 tests (one existing ignored
check), log /tmp/crepus-reactive-current-host-audit.log. Read-only extension asset
validation review found no new confirmed defect. Binding-generator review retained
an actual CLI reproduction: C `const char **` generates Rust `*const *mut c_char`
rather than `*mut *const c_char`; evidence /tmp/crepus-pointer-const-review/.
That qualifier-lowering defect is not fixed in this change. TUI provenance API
compatibility decision remains pending. No commits, pushes or remote mutations.
Goal remains active.

### 2026-10-18 — nested C pointer qualifiers in typed bindings

Parallel bindgen review reproduced successful wrong Rust generation for C
`const char **`: outer const was emitted instead of const on the character pointee.
Other valid pointer-level qualifiers could fail generated Rust parsing. Retain the
original C prototypes, normalize const away only for equilibrium's scalar/depth
mapping, then restore each pointer's qualifiers from the base outward in emitted
function signatures. Reparse that corrected code as the shared typed AST, keeping
Rust and typed consumers consistent. Existing signature name/arity admission checks
run before reconstruction. No dependency or external runtime edits.

Tests-first baseline fails. Regression covers 18 pointer shapes as parameters and
returns, base/postfix const, top-level const, compact spellings, triple/mixed levels,
callback/session pointers, similar parameter names, and unchanged rejection of
volatile/restrict/array forms. Full bindgen suite: 26 passed, three existing real-ABI
checks ignored; final four library tests rerun after compact-case additions. Strict
all-target Clippy, formatting, primary scoped diff review and independent source
review pass. Fresh public-generator Rust and D outputs compile/link/run against an
independent C fixture: pointer replacement and mutable pointee writes both pass,
with Rust function-pointer signature assertions. Evidence and patches:
/tmp/crepus-pointer-qualifier-fix/; original CLI reproduction:
/tmp/crepus-pointer-const-review/. No new all-language ABI or real-session claim.
TUI provenance compatibility question remains pending. No commits, pushes or remote
mutations. Goal remains active.

### 2026-10-18 — remaining typed pointer and real-session acceptance

Closed the follow-up native compiler gap from the pointer-qualifier fix. Freshly
generated Nim and Odin bindings compile/link/run against the same independent C
pointer fixture used for Rust/D: replacement of a pointer to const characters and
mutation through a const pointer both pass. Their existing models omit const but
retain pointer depth. Logs: /tmp/crepus-pointer-qualifier-fix/{nim,odin}-acceptance.log.
No production changes were needed.

Rebuilt the current Rust ABI library, then explicitly enabled the three existing
D/Nim/Odin generated-binding real-ABI tests against that absolute library path.
All three pass, exercising all thirteen exports, owned strings, callbacks/userdata,
context, event dispatch and session lifecycle. Logs in the same evidence directory:
real-abi-build.log and real-abi-tests.log. This supplements the earlier independent
C pointer checks; it does not establish every language/OS combination.

Independent read-only Nim/Odin consumer and embedded text/layout arithmetic reviews
found no new confirmed defect. Existing TUI file-origin issue remains pending its
public struct-literal compatibility decision. Primary inspected this append and
check output. No commits, pushes, remote mutations or destructive cleanup.
Goal remains active.

### 2026-10-18 — Vue awaited-loop setup diagnostics

Parallel frontend review suggested top-level for-await could escape async-setup
validation. Public compiler probes corrected that hypothesis: plain root loops
already fail source parsing, while import-bearing setup loops fail later when the
synchronous generated component is reparsed. No successful invalid output was
established. The concrete defect is late generated-source diagnostics instead of
the existing authored async-setup diagnostic for those admitted source modules.

Added a six-line ForOfStatement visitor guard at setup function depth zero, using
the existing async-setup error and preserving traversal/nested async functions.
Tests-first import-bearing cases fail baseline with `module parse failed` and a
generated offset. Final 27 focused Vue tests pass across DOM and SSR; root/block
and import-bearing awaited loops reject, nested async declarations/arrows retain
for-await, and their actual output reparses without Oxc errors. Strict all-target
native Clippy with web-modules, formatting and primary scoped diff review pass.
Docs clarify awaited-loop behavior. Evidence: /tmp/crepus-vue-for-await-fix/.
No live browser/runtime claim or new async-setup support. Independent bounded TUI
control-flow/include source review found no new validated defect. Existing TUI
registry provenance compatibility question remains pending. No commits, pushes,
remote mutations or destructive cleanup. Goal remains active.

### 2026-10-18 — compute-worker JSON data preservation

Parallel Lite review found host post_message serialized JSON directly into a JS
object expression. Actual public WorkerRuntime tests against real V8 fail baseline:
root and nested own __proto__ keys disappear from delivered event.data. Double
serialization now passes JSON text to the worker bootstrap, which captures the
original JSON.parse before guest entry and parses data before dispatch. Public
payload/event APIs and callback error behavior remain unchanged; only the private
dispatcher's internal wire argument changes. Updated method docs and Lite guide.

Both new real-V8 tests pass, covering objects/nested arrays/scalars, Unicode,
quotes/LF/CR, guest replacement of JSON.parse, and subsequent delivery after a
throwing handler. Existing eleven V8 async lifecycle/FIFO/abort/reset tests pass
(13 total). Primary scoped diff and formatting checks pass; independent source
review approves. Evidence: /tmp/crepus-lite-worker-json-fix/. No transport stubs.
Strict Clippy was attempted for lib and the two relevant test targets but failed
in gpui_media's dependency build-script link with ENOSPC (errno 28), before crate
lint completion. It remains unverified; no broad cleanup or unchanged retry.

Current LSP suite also passes 37 tests, log /tmp/crepus-lsp-current-module-audit.log.
Bounded web serialization and runtime bridge/include-cache reviews found no new
confirmed defect in their examined paths. Full workspace/UI/hardware acceptance
is not established. Existing TUI registry provenance compatibility decision remains
pending. No commits, pushes or remote mutations. Goal remains active.

### 2026-10-18 — worker JSON lint gate closed

Revalidated the previous ENOSPC condition: free capacity had recovered, then rose
to about 1 GB during this pass. Retried the same relevant Lite Clippy scope with
CARGO_BUILD_JOBS=1 to serialize dependency build work. Strict Clippy now passes for
the v8-enabled library, worker_messages and v8_async targets. Log:
/tmp/crepus-lite-worker-json-fix/clippy-serial.log. This closes the prior missing
lint gate; no source/test changes or repeated runtime checks were necessary.
Primary diff whitespace check remains clean. Previous thirteen real-V8 tests
remain the runtime evidence for the unchanged worker fix.

Independent sibling invoke-envelope source review found no equivalent loss:
callbacks return serialized JSON strings as V8String, rather than evaluating JSON
as object expressions. No new runtime/desktop-UI or cross-platform evidence is
claimed for that read-only review. No cleanup, commits, pushes or remote mutations.
Existing TUI provenance compatibility decision remains pending. Goal remains active.

### 2026-10-18 — typed download attribute bindings

Primary web attribute audit found overloaded download values used ordinary string
coercion: typed false emits download="false" and typed true suggests filename true.
The HTML hyperlink contract makes attribute presence enable downloading and its
value suggest a filename (https://html.spec.whatwg.org/multipage/links.html#attr-hyperlink-download).
Actual Chromium on a Rust-rendered baseline link downloads false.txt for typedfalse.

Shared push_bindings now omits typed false/null download values and serializes typed
true as an empty attribute. Strings (including "false"/"true") remain filename data;
explicit Angular attr.download remains literal attribute presence. No change to
generic reactive DOM setter APIs or other attribute handling. Docs clarify rules.
Tests cover DSL, virtual JSX and Angular, plain/SSR markers on/off/hydration output.
Corrected tests-first baseline is one real failure plus passing explicit-attribute
control; opening-tag assertions account for hydration include wrappers.

All 169 web checks with hydration pass, plus strict all-target Clippy, formatting,
primary scoped patch review and independent source review. Fresh Rust-rendered
output exercised in Chromium across 24 mode/value combinations: false/null navigate
to the text resource; true downloads its default filename; string values retain
filename behavior. Evidence/patch: /tmp/crepus-web-download-fix/. Browser proof is
Chromium initial generated markup, not live WASM updates or every browser/OS.
No commits, pushes, remote mutations or destructive cleanup. Goal remains active.

### 2026-10-18 — Rust 2024 generated foreign blocks

Primary binding-output audit found generated extern C blocks lacked explicit
unsafety, so importing them from an edition-2024 consumer fails. Tests-first
existing full ABI signature consumer now compiles in both 2021 and 2024 editions
with warnings denied; baseline fails on the 2024 foreign-block diagnostic.
Rust output adapter now marks line-start foreign blocks unsafe. Shared C type
parsing, other language outputs, callback aliases and calling conventions stay
unchanged. Locked generator formatting and adapter anchoring independently reviewed.

All 26 bindgen tests pass (three existing real-ABI tests ignored in this run),
including both-edition signature checks for actual and overridden headers. Strict
all-target Clippy, formatting and primary actual scoped patch inspection pass.
Fresh public-generator Rust 2024 output additionally compiles/links/runs against
the retained independent C pointer fixture, checking both pointer replacement and
pointee mutation. Evidence: /tmp/crepus-rust-2024-bindgen-fix/.

The Rust edition guide confirms unsafe extern syntax is available from Rust 1.82
in all editions and required in 2024; docs explicitly record that emitted-syntax
minimum (https://doc.rust-lang.org/edition-guide/rust-2024/unsafe-extern.html).
Checks used the installed compiler; Rust 1.82 itself was not exercised. Independent
bounded web form and shared arithmetic reviews found no new confirmed defect.
No commits, pushes, remote mutations or destructive cleanup. Goal remains active.

### 2026-10-18 — Python UTF-8 transport and TypeScript context keys

Parallel Python review reproduced render_ir failing on valid UTF-8 CLI output
under LC_ALL=C with Python UTF-8/coercion modes disabled. Context-mode template
reads also used the locale default. Two scoped changes make template reads and
subprocess text streams explicitly UTF-8, retaining strict decoding, path checks,
argument execution and payloads. Tests-first isolated ASCII-locale children fail
baseline on both output and source reads; invalid UTF-8 controls remain errors.
All 20 Python wrapper checks pass. An existing built CLI also passes three actual
ASCII-locale context modes (None, empty dict and Unicode context). Evidence:
/tmp/crepus-python-encoding-fix/; original /tmp/crepus-python-encoding-review/.
Permanent transport tests use a labelled executable stub, not a replacement parser.
Their Unix shebang path is explicitly skipped on Windows.

Independent TypeScript review reproduced public dispatch of bind:__proto__:value
silently losing an initially absent key through the inherited object setter.
The binding path now defines an own enumerable/writable/configurable data property
for that key; other assignments and the baseDir/_ blocklist stay intact. Tests-first
case fails baseline; absent/existing keys, prototype identity, colon values,
callback observations and serialization now pass. All 20 local Bun checks pass
when invoked with explicit local test paths. Actual dispatch through the existing
built CLI produces the Unicode colon-containing value in View IR. Evidence:
/tmp/crepus-ts-context-proto-fix/. New focused dispatch tests label their render stub.

An initial broad Bun path filter also discovered editors/zed/grammars/crepus's
mirrored plugin tests: its stale expected IR version 4 fails against current 7.
That unrelated mirror is unchanged and recorded in tests.log; scoped-tests.log
covers the owned plugin suite. Both production patches passed independent review,
primary scoped diff inspection and whitespace checks. Docs describe the boundaries.
No Windows transport acceptance, commits, pushes, remote mutations or destructive
cleanup. Goal remains active.

### 2026-10-18 — strict UTF-8 template bytes in Go and Bun adapters

Primary sibling boundary audit reproduced TypeScript accepting a template with
byte FF inside quoted text and rendering U+FFFD, while the same built CLI invoked
directly rejects invalid UTF-8. Go's byte-to-string plus json.Marshal path likewise
replaces invalid bytes; tests-first public RenderIR cases confirm success instead
of rejection. Both adapters now validate before the CLI: Go uses standard utf8.Valid
before string/JSON conversion, and Bun uses a fatal UTF-8 TextDecoder over its async
file buffer. Valid bytes/transport/path rules stay unchanged; measured Bun BOM
handling matches its prior file.text decoding.

All 20 top-level Go tests and 21 local Bun tests pass, plus go vet, gofmt checks,
primary scoped patch/whitespace review and independent source reviews. New labelled
transport stubs assert invalid input cannot start the CLI and valid Unicode reaches
the payload. Actual public Go and Bun APIs against the existing built CLI reject
the invalid fixture and render valid CJK/astral text. Evidence:
/tmp/crepus-wrapper-utf8-validation/. No new dependencies or compiler changes.
Windows transport fixtures were not exercised (Go's shell stub skips Windows).
Docs describe the stricter source-byte boundary.

Confirmed the earlier Zed mirror is an ignored cached git checkout used for grammar
sources; the plugin smoke command roots Bun execution under plugins and does not
select that cache. It remains untouched. No commits, pushes, remote mutations or
destructive cleanup. Goal remains active.

### 2026-10-18 — Ruby strict UTF-8 CLI transport

- Reproduced locale-dependent Unicode failures in template reads and captured CLI stdout/stderr under `LC_ALL=C`. Ruby now reads contextual templates as UTF-8, captures pipe bytes in binary mode, and validates both output streams as UTF-8 before parsing or raising diagnostics. Invalid bytes are rejected without replacement; binary-name/path restrictions and argv execution remain unchanged.
- Added four isolated ASCII-locale transport regressions using a labelled CLI stub: valid Unicode with/without context, malformed source rejected before spawn, malformed quoted JSON output rejected, and Unicode error diagnostics preserved. Corrected the stub itself to read stdin as binary bytes, matching the native CLI transport.
- Primary verification: all Ruby wrapper tests passed (14 runs, 20 assertions); both changed Ruby files passed syntax checks; actual existing built CLI rendered Unicode in ASCII-locale subprocesses with nil, empty, and Unicode-bearing contexts. Independent read-only review found no blocker; actual scoped diff and `git diff --check` passed.
- Evidence: `/tmp/crepus-ruby-encoding-fix/` contains baseline, final suite, before snapshot, and real-CLI logs. Unix fixture coverage does not establish Windows transport behavior. No commit or push performed.

### 2026-10-18 — Java/Kotlin malformed UTF-16 context transport

- Confirmed actual Java IR rendering silently changed an unpaired high surrogate to `?`: the shared JSON quoting emitted raw UTF-16 code units, then the UTF-8 stdin writer replaced malformed input. Both new Java and Kotlin acceptance tests failed against before snapshots on the first malformed IR value.
- Minimal production fix: both existing JSON quote functions now escape surrogate code units alongside control characters. Valid surrogate pairs remain standard JSON pairs; strict native decoding rejects lone/reversed surrogates rather than receiving substituted characters. This shared path covers context values, map keys, and both IR/stdin and HTML/context-file APIs.
- Parallel worker authored two real-CLI acceptance entrypoints: CJK/astral preservation in each API plus four malformed forms × keys/values × IR/HTML per language (32 rejection checks total). Primary inspected tests/diffs and integrated both into the existing plugin smoke script. Independent production review found no blocker.
- Primary verification on javac 17.0.20 and Kotlin 2.4.10/JRE17: all six Java/Kotlin IR, HTML, and Unicode acceptance entrypoints passed against the existing built native CLI. Existing noisy diagnostic/HTML temporary-file cleanup fixture passed for Java/Kotlin/PHP. Shell syntax and `git diff --check` passed. Logs/before snapshots: `/tmp/crepus-jvm-unicode-fix/`.
- Full polyglot smoke/build and other operating systems were not rerun; these checks establish the changed JVM wrappers on this macOS host. No commit or push performed.
- Independent bounded embedded source review found no new validated defect; no embedded edits or runtime claims from that pass.

### 2026-10-18 — Ruby regression coverage in plugin smoke

- Found the main plugin smoke script invoked only `test_path_traversal.rb`, omitting the other five current Ruby test files including Unicode transport regressions. Replaced that single-file invocation with sorted `test_*.rb` discovery using the existing Ruby/minitest runtime. Existing direct real-CLI Ruby smoke remains intact.
- Primary ran the exact discovery command with the existing built CLI: 14 runs, 20 assertions, zero failures/errors/skips. Shell syntax, actual before/current script diff, and `git diff --check` passed. Evidence: `/tmp/crepus-ruby-smoke-gate/`.
- Full multi-language smoke and hosted CI were not run. Independent parallel read-only LSP and native View IR passes found no new confirmed defects; their bounded source reviews do not establish runtime acceptance.

### 2026-10-18 — Current native, web-extension, and macro library gates

- Primary revalidated the current checkout with serialized native Cargo execution: `cargo test --locked -p crepuscularity-native --features web-modules --lib` (818 passed), `cargo test --locked -p crepuscularity-webext --lib` (55 passed), and `cargo test --locked -p crepuscularity_macros --lib` (32 passed). All 905 tests passed without failures or ignored tests.
- Independent parallel source reviews of current web-extension manifest/content-script/resource/browser assembly and macro diagnostics/scopes/escaping/expressions found no new confirmed defects. These passes excluded already recorded fixed defects rather than reporting them again.
- No production changes from this pass. Logs: `/tmp/crepus-native-current-module-audit.log`, `/tmp/crepus-webext-current-module-audit.log`, `/tmp/crepus-macros-current-module-audit.log`. The checks cover library unit tests; integration suites, real browsers/devices, cross-compilation, and deployments were not revalidated here. Actual audit-ledger addition and `git diff --check` inspected/passed. No commit or push performed.

### 2026-10-18 — Current framework form integration contracts

- Independent bounded reactive DOM review found no new confirmed defect in live input/textarea/select updates, checked properties, file-input restrictions, missing-element dependency tracking, disposal or hydration/URL policies. Independent framework-emitter review inspected Solid/Vue/Svelte inputs; primary additionally inspected React's shared InputRef runtime and emitted input/textarea defaults, updates, and native change routing. No production edits resulted.
- Primary ran current native integration suites `framework_checked_bindings`, `framework_form_fields`, `framework_input_bindings`, `framework_input_events`, and `textarea_children` with locked dependencies and serialized Cargo. All passed; complete per-suite counts and output are in `/tmp/crepus-framework-forms-current-audit.log`.
- These tests verify frontend-to-IR and generated-source contracts, including six frontend input events and multiple output frameworks. They do not execute host framework compilers or prove rerender/event behavior in a browser. Prior actual WASM DOM evidence remains separately recorded; no new browser, no-lag, cross-platform or deployment claim. No commit or push performed.

### 2026-10-18 — Fresh React input lifecycle browser acceptance

- Closed the current React runtime gap identified in the preceding generated-source form check. Primary linked a tiny public Rust consumer against the freshly built native crate, emitted input/textarea/unbound-input TSX, passed strict TypeScript checks, and bundled it with Bun against existing installed React/ReactDOM 19.1.0. No dependencies installed or production files edited.
- Actual headless Playwright Chromium under React StrictMode passed native per-keystroke input versus blur change timing; native Event and handler receiver preservation; latest handler after three rerenders without duplicate dispatch; bound host updates; unbound edit retention; same-value caret retention; and unmount cleanup (held detached nodes have null onchange and dispatch no callbacks). No page or console errors occurred. Independent read-only InputRef lifecycle review found no blocker.
- Evidence and runnable local fixture: `/tmp/crepus-react-input-lifecycle/` (public emitter source/binary, fresh Fields.tsx, host fixture, build/browser scripts and logs). Local HTTP server and browser were closed. This is React19.1/Chromium input lifecycle evidence, not every framework/browser, application performance, or a no-lag guarantee. No commits, pushes, deployments or remote mutations.

### 2026-10-18 — Current GPUI host responsiveness gates

- Primary ran `CARGO_BUILD_JOBS=1 cargo test --locked -p crepuscularity-lite --features v8,oxc --bin crepuscularity-lite` against the current checkout: all 35 tests passed, no ignored tests. This includes startup configuration I/O yielding foreground work, async startup error/disconnection handling, guest file reads and TypeScript compilation yielding UI, V8 reply waits allowing foreground heartbeats, and host pointer/keyboard dispatch.
- Current source inspection confirms LiteRoot teardown aborts V8 and observes join on a separate named thread. Independent bounded audits of Lite worker FIFO/reset/abort and runtime hot-reload publication/cancellation found no new confirmed defect. A parallel Flutter form review found documented unsupported free controls and intentional action-only callbacks, not a new validated bug.
- Evidence: `/tmp/crepus-lite-host-responsiveness-current.log`. No production changes. GPUI test contexts and real V8 workers establish these tested yield paths, not physical-window latency, every OS/application, interruptibility of native callbacks, or an impossible-to-lag guarantee. Actual ledger addition and `git diff --check` passed. No commits, pushes or remote mutations.

### 2026-10-18 — Preserve CR in reference-adapter HTML attributes

- Confirmed raw CR/CRLF loss in quoted HTML attributes: the TypeScript/Bun before snapshot's hidden value and form name parsed with LF substitutions in Chromium. Expanded shared defaults fixture failed before fixes in Bun and PHP; the same existing escaping pattern affected Python/Ruby/Go.
- Five adapters now encode CR as `&#13;` after ordinary attribute escaping. Small shared per-language attribute helpers cover input values/names/placeholders/bind keys, action names, links, image metadata, and picker values. Ordinary text escaping and textarea initial-LF compensation remain intact; URL admission policy is unchanged. Parallel workers owned Go and Python/Ruby, primary owned Bun/PHP/fixture/integration; independent five-adapter review found no blocker.
- Shared fixture adds hidden CR/CRLF defaults and action/link/image/picker attributes with Unicode, ampersand and quote coverage. Main plugin smoke now discovers all seven existing PHP smoke entrypoints, so this regression is included there. Primary inspected actual before/current diffs, Go formatting/vet, Ruby/PHP syntax, shell syntax, and `git diff --check`.
- Verification: all 20 Python tests, 14 Ruby tests, 20 Go top-level tests, 21 Bun tests and seven PHP smoke entrypoints passed. Initial Bun full-suite invocation used a cwd-relative incorrect CLI path; rerunning with the correct absolute existing binary passed all21, no code changes needed. Labelled IR/CLI fixtures isolate adapter serialization in shared cases. Chromium checked each public adapter's fixture output, preserved parsed CR attributes and exact hidden-input FormData keys/values, picker values and ordinary editable input interaction.
- Separately, all five public APIs rendered a real template through the existing built CLI with CR-bearing context; their resulting HTML also preserved exact names/placeholders/hidden values and FormData in Chromium. Evidence, before snapshots, fixture outputs and runnable probes: `/tmp/crepus-wrapper-attribute-cr-fix/`, worker patches `/tmp/crepus-attribute-cr-fix/` and `/tmp/crepus-go-attribute-cr-fix/`.
- Full multi-language smoke, other browser engines and OSes were not rerun. Scope is these five reference adapters; no new live binding runtime, commits, pushes or remote mutations.

### 2026-10-18 — Rust session callback registration after errors

- Found `ViewSession::dispatch` removed a callback to borrow it mutably, then propagated its error before reinsertion. A second dispatch therefore lost the callback and proceeded to rendering instead of calling it again. Worker made the minimal shared fix: retain the callback Result, restore registration, then propagate that Result. Binding updates still precede callbacks and ordinary errors still stop rendering; successful replacement semantics remain unchanged.
- New two-dispatch regression checks the same original error, preserved callback state/count, and binding visibility. It fails against a preserved before implementation (second error becomes missing-template I/O) and passes with the fix. Primary inspected the actual diff; independent reviewer approved.
- All five Rust reference-adapter tests passed with the actual built CLI, strict all-target Clippy, formatting and `git diff --check` passed. Baseline and current crates sharing a target directory initially yielded a stale baseline executable; a fresh isolated current build proved the fix, then refreshing the edited source timestamp and rerunning the normal target gate also passed. No source-content workaround or dependency changes. Evidence: `/tmp/crepus-rust-handler-retention-fix/`.
- Follow-up finding reproduced separately without production changes: current C and C++ `render_html` extract only the first quoted JSON content fragment, dropping later nodes and mishandling escaped quotes/HTML. Both actual wrappers on a three-node real-CLI template output only a truncated first fragment. Public-consumer probes/evidence: `/tmp/crepus-c-html-followup/`. Repair remains pending; canonical `crepus render` is the existing complete rendering route to evaluate next.
- Source reviews also located remaining CR attribute gaps in the supported Swift/Rust HTML subsets; those were not changed in this callback fix. No commit, push, deployment or remote mutation.

### 2026-10-18 — C/C++ canonical HTML rendering

- Fixed the confirmed first-JSON-string extraction bug in both POSIX reference helpers. HTML now executes the existing `crepus render` argv route directly and returns its complete output; IR keeps its separate native route. Explicit `--ctx /dev/null` supplies empty TOML and prevents sibling `context.toml` autoload, preserving prior no-context semantics. No JSON parser or dependency added.
- Reused each subprocess transport. C retains caller-capacity checks, null termination, draining after overflow and child status handling. C++ now retries interrupted reads/waits and reports read/wait failures after reaping. Smoke entrypoints no longer require invented stack markers absent from canonical renderer output. Worker owned C++; primary owned C, fixtures/gates/integration; independent review approved.
- New `scripts/test-c-html.py`, wired into plugin smoke, compiles actual public-helper consumers with `-Wall -Wextra -Werror` and verifies byte-for-byte canonical output for multiple nodes, Unicode/quotes/HTML escaping, relative includes, Svelte frontend, empty source, missing files, and disabled context autoload. Labelled fragmented-output collaborator verifies HTML argv, zero/8191/8192/131072-byte outputs, C overflow/cap0/cap1 behavior, and nonzero child exit with buffered output. Existing quoted-path/fragmented IR+HTML pipe gate was updated for the real two-route protocol and passes.
- The new canonical-output gate fails against preserved original C/C++ sources on the initial multi-node/escaped-string case. Both real-CLI smoke binaries and both final regression scripts pass; Python/shell syntax and `git diff --check` pass. Chromium on freshly compiled actual-helper HTML preserves both text nodes exactly and the button's action/click without page errors. Primary inspected actual scoped diffs.
- Evidence and preserved baselines: `/tmp/crepus-c-html-fix/`, `/tmp/crepus-cpp-html-fix/`, earlier reproducer `/tmp/crepus-c-html-followup/`. Full polyglot smoke, other OSes, and physical UI performance were not rerun. These adapters remain POSIX-specific; C output must fit its caller's buffer. No commits, pushes, deployments or remote mutations.

### 2026-10-18 — Swift/Rust supported HTML attribute CR preservation

- Closed the remaining confirmed CR gaps in these reference adapters' supported HTML subsets. Small attribute helpers perform ordinary HTML escaping first, then encode CR as `&#13;`; stack/scroll axis and button action attributes use them. Swift also routes its kind attribute through the helper. Text rendering and unsupported-node boundaries remain unchanged; Rust callback-retention fix is preserved.
- Rust worker added a focused matrix covering CR/CRLF, literal `&#13;`, quotes, ampersands/angle brackets, Unicode, button actions and axis attributes. It fails against the preserved before implementation. All six current Rust adapter tests, strict all-target Clippy and formatting pass.
- Primary added a real-CLI Swift XCTest using JSX action strings with raw CR/CRLF and literal entities. It fails before production changes; both Swift tests pass with compiler warnings treated as errors, including the existing large-pipe regression. Independent scoped review approved escaping order and test coverage; primary inspected actual Rust/Swift diffs and `git diff --check` passed.
- Actual public Swift and Rust consumers were compiled against current sources/artifacts and invoked the existing built CLI. Chromium reproduces the Swift before snapshot's CRLF→LF action loss, then verifies exact action attributes, unchanged literal entity text and button interaction for both repaired outputs. Evidence: `/tmp/crepus-swift-attribute-cr-fix/`, `/tmp/crepus-rust-attribute-cr-fix/` (before snapshots, red/green gates, public probes and browser log).
- Other OSes/browser engines and full polyglot smoke were not rerun. No new controls, dependencies, commits, pushes or remote mutations. The separately observed Rust scroll marker issue remains outside this CR fix.

### 2026-10-18 — Rust scroll HTML marker

- Fixed the recorded Rust reference-renderer mismatch: shared Stack/Scroll rendering hardcoded `data-crepus-kind="stack"`. It now selects the marker from the actual enum variant, preserving shared axis escaping and recursive children.
- Focused exact-output regression covers a scroll nested in a stack plus an empty root scroll, including child text escaping. It fails before the production edit. All seven Rust adapter tests, strict all-target Clippy, formatting and `git diff --check` pass. Independent scoped review approved; primary inspected the actual diff.
- Primary compiled a public consumer and exercised a real template with `overflow-y-auto`; inspected actual CLI IR to confirm a Scroll node before validating HTML. Chromium confirms one scroll marker, the two distinct stack wrappers, viewport axis and exact child text. A preliminary bare `scroll` tag did not produce Scroll IR and was replaced with the existing supported overflow-class source rather than treating that probe as evidence.
- Evidence: `/tmp/crepus-rust-scroll-marker-fix/`. No claim of CSS scrolling behavior in this minimal reference adapter. Other platforms/full polyglot smoke were not run; no commits, pushes or remote mutations.
- Independent next-pass candidate: Zig wrappers appear to retain old single-root assumptions while current IR uses arrays; existing Zig smoke does not compile those wrapper sources. This is source-only evidence pending compiler/runtime verification, not a validated runtime fix. C# uses recursive root-array rendering rather than the repaired C/C++ extraction approach.

### 2026-10-18 — Compile and repair actual Zig 0.16 reference adapters

- Revalidated the source-only candidate with installed Zig0.16 and local standard-library sources. Both original wrappers fail compilation (removed Child.run/ArrayList APIs, shadowed C import, and an incorrectly predefined C header guard). The former root/nested build targets only spawned a CLI version grep and did not compile these sources. Baseline logs are retained; no successful old-runtime behavior is claimed.
- Standalone CLI adapter now accepts `(allocator, io, path)`, uses `std.process.run` to drain both streams, frees buffers on process/parse/shape failures, validates version/root-array shape, and uses canonical CLI HTML rendering with explicit empty context. It resolves `crepus` through parent PATH; this and the required Io argument are documented. Removed the obsolete single-object HTML walker instead of adding another renderer.
- ABI worker repaired header import, current collection APIs, root arrays, deep string/tree ownership and partial-error cleanup, sentinel-terminated C inputs with explicit embedded-NUL rejection, and closed-session checks. Context methods accept JSON-compatible Zig values and std.json.Value. HTML rendering remains an explicit text/stack/scroll/button/image subset, safely escaped with CR-preserving attributes; unsupported kinds return an error. Existing callback registration remains unsupported and documented.
- Primary integration caught and fixed two real runtime issues before acceptance: dispatch returns an event envelope containing `ir`, so it now extracts and deep-copies that value; relative runtime-library search paths failed after the test CWD changed, so test linkage now embeds an absolute native-debug ABI path. No ABI library/source mutation was required.
- Root and nested build entrypoints now share actual compile/run tests, linked to the existing real Rust ABI. Both pass all14 tests (five CLI, nine ABI), including current v7 roots, complete Unicode/escaped multi-node HTML, malformed IR, process failures, real dispatch, context patches, snapshots surviving JSON release/session teardown, std.json.Value contexts, NUL/closed-handle rejection, unsupported HTML, and exhaustive allocation-failure cleanup. Formatting/diff checks and independent ownership/protocol/build reviews pass. Primary inspected actual diffs and preserved concurrent edits.
- Separate public-consumer binary compiles both imports with std.process.Init, renders real CLI HTML, dispatches a real ABI binding event, frees the session, and renders the retained owned snapshot. Chromium verifies complete parsed CLI nodes/text/button action/click and exact escaped ABI snapshot text. Evidence and baseline snapshots/logs/probes: `/tmp/crepus-zig-wrapper-audit/`.
- Compatibility: Zig0.16 CLI callers must pass std.Io; ABI ViewIr.root is now []ViewNode and must be released with deinit. Session handles must not be copied. Full polyglot smoke, other platforms/browser engines, and callback implementation were not exercised/provided in this repair. No new dependencies, installed-tool edits, commits, pushes or remote mutations.

### 2026-10-18 — V canonical HTML and C# attribute CR preservation

- Confirmed V `render_html` ignored its source after an IR-version check and always returned a hardcoded Hello/name container. New multi-node actual-CLI regression fails against that implementation. Primary replaced it with the existing canonical HTML command, sharing the adapter's existing quoted-path transport and private per-call context-file lifecycle with the unchanged IR API. Explicit context maps prevent sibling autoload, including empty maps; no renderer/dependency added.
- V0.5.1 full test file passes all five test functions, including existing real ABI callback/string-release and concurrent context isolation checks. New HTML tests cover complete multiple nodes, Unicode/escaping, button action metadata, relative includes/context, spaces in paths, empty-context no-autoload and missing-template errors. V formatting passes. Chromium on its actual public helper output preserves both text nodes, button action and click. V IR's existing version-only result type is unchanged in this HTML repair.
- Independently confirmed C# had raw CR in quoted attributes while textarea already compensated for CR. Its existing shared-fixture input gate fails before the fix. Worker introduced HtmlEncode-then-CR-entity attribute escaping at input/name/placeholder/binding, picker, action, link, image and layout metadata sites; text/textarea/URL policy remain unchanged. Public/session fixture checks and an added actual-CLI hidden-input context case pass.
- Primary built the .NET solution and smoke project with warnings as errors (zero warnings/errors), ran general IR/frontend/include/diagnostic smoke plus link and input HTML/session checks. Chromium verifies shared C# attribute CR/CRLF retention, ordinary editing, exact hidden FormData names/values, and a separate real-CLI HTML roundtrip. Main plugin smoke now invokes existing C# input/link checks; SDK telemetry is explicitly opted out for its .NET checks.
- Independent scoped review approved both repairs; primary inspected actual before/current diffs and shell/format/diff gates passed. Evidence: `/tmp/crepus-v-html-fix/`, `/tmp/crepus-csharp-attribute-cr-fix/` (red/green logs, before snapshots, actual HTML and browser log).
- Full polyglot smoke and other OS/browser engines were not rerun. No new dependencies, commits, pushes, deployments or remote mutations.

### 2026-10-18 — Integrated polyglot smoke gate

- Full `CARGO_BUILD_JOBS=1 DOTNET_CLI_TELEMETRY_OPTOUT=1 bash scripts/plugin-smoke.sh` passed on macOS against the current CLI and freshly built Rust ABI. This covers Python, Go, Bun/TypeScript, V, Zig, Rust, Swift, C/C++, Ruby, PHP, Java, Kotlin and C#, including canonical HTML/pipe regressions, Unicode/diagnostic isolation, and actual/generated ABI lifecycle checks. C# build finished with zero warnings/errors. Log: `/tmp/crepus-full-polyglot-smoke-current.log`.
- Initial attempt stopped at a concurrent native compiler edit missing `ModuleHints.solid_component_imports`; that caller was corrected concurrently before this rerun. Primary made no compiler change. Independent read-only follow-up found no remaining missed initialization/provenance path or native/WASM schema-version mismatch. This is source review plus successful integrated build, not a rerun of all native compiler tests.
- Corrected two small Zig documentation omissions: README session-wrapper list includes Zig, and the ABI usage example releases its owned IR snapshot. Independent documentation review and final diff checks pass.
- Other OSes, browser engines and physical-window latency were not checked in this gate. No commits, pushes, deployment, destructive cleanup, new dependencies or remote mutations.

### 2026-10-18 — iOS state reader output sizing

- Parallel native-host review found that scaffold `CrepusStateStore.readString` returned empty text for required UTF-8 lengths >=4096; list JSON at the same boundary decoded as an empty list. Current Rust `copy_json_to_output` already reports the full required byte count. Reproduced the failure in the actual-source Swift scope harness before editing production.
- Scaffold and advanced mobile example now release each undersized buffer, resize to the reported byte count plus the terminator, and retry only read-only text/items evaluators. Integer conversion/addition is guarded; successful output decodes exactly the reported byte length, preserving embedded NUL and Unicode. Action/change dispatch is unchanged and never replayed.
- Expanded the existing FFI-stub harness to model actual Rust partial-copy/required-length semantics and exercise both actual Swift reader implementations: empty output, 4095/4096/4097/16384-byte text, Unicode/NUL suffixes, and 1000-item JSON through the public scaffold model and example reader. Existing scope/type/transport checks still pass under warnings-as-errors. Full `python3 scripts/test-native-action-queue.py` passes, including generated/snapshot Swift/Kotlin FIFO, routing, drafts, picker and main-heartbeat gates.
- Independent scoped review approved ownership, arithmetic and call-site boundaries; primary inspected all three actual before/current diffs and final whitespace checks. Evidence: `/tmp/crepus-swift-reader-size-fix/` (baseline/current logs, before snapshots, task patch and full native action-queue log).
- FFI collaborators are explicitly labelled stubs in these acceptance probes; no iOS-device rendering, live Rust/Swift linkage, physical-window latency, Android deployment or cross-platform build claim. Terminal/embedded read-only follow-up found no new validated boundary defect. No dependencies, commits, pushes, deployment or remote mutations.

### 2026-10-18 — Stack-safe Solid component export verification

- Parallel compiler review identified recursive verification of arbitrarily long valid reexport chains. Primary reproduced a process-aborting stack overflow with 2049 real pragma-enabled modules resolved through `verified_imports` on a 2 MiB thread. Baseline exited by SIGABRT; this was a runtime failure, not merely a source concern.
- Replaced the single-successor recursive export walk with a loop. A local hash set detects exact path/export cycles in constant-time lookup; only fully verified chains enter the existing success cache. Both imported tags and default-export verification still share the same helper and fail-closed pragma/export admission. Removed obsolete recursive active-stack state. No syntax support or public API changes.
- New small-stack long-chain regression and all nine resolver tests pass, preserving existing cycle, missing pragma, unsupported export and alias/default cases. Full current native web-modules library suite passes all827 tests; current TUI library suite separately passes all147 tests. Independent read-only scoped review approved cache/error/call-site behavior. Primary inspected the actual before/current patch; rustfmt and whitespace checks pass. Evidence: `/tmp/crepus-solid-chain-audit/`, `/tmp/crepus-tui-current-audit.log`.
- Next native-host source candidate: synchronous/action-bearing Swift edits still have a 4096-byte action-result limit and can report failure after Rust executes. This was not runtime-reproduced or changed in this pass. A correct repair must return the complete exact per-call result without retrying mutation; cached `lastResult` is not a safe general substitute because change dispatch adds binding fields after that cache is populated.
- No browser/device or cross-platform gate was run for this compiler-only repair. No dependencies, commits, pushes, destructive cleanup, deployment or remote mutations.

### 2026-10-18 — Complete iOS action results without replay

- Runtime-reproduced the prior source candidate: an actual-source generated Swift queue successfully dispatched an 18KB edited string once, then rolled the visible draft back because the bridge synthesized an overflow failure. The baseline FFI stub was adjusted to mirror Rust truncation rather than overflowing its test buffer; the regression failed specifically at the rollback assertion.
- Added two app-local Rust owned-result exports, `crepus_mobile_dispatch_json_owned` and `crepus_mobile_dispatch_change_json_owned`, using existing `alloc_c_string`/`crepus_mobile_free_string`. Legacy buffer exports retain their signatures and required-length/truncation behavior; private helpers share original validation and execution once. Owned change results include all post-dispatch binding fields rather than relying on cached metadata.
- Swift default routing, synchronous stored dispatch and action-bearing changes consume/free owned per-call results. Removed the obsolete fixed4096 action buffers and synthetic oversized-result error; allocation failure produces an explicit error. No action is queried/retried. Existing public Swift methods remain available. Advanced mobile example already uses its separate owned-result ABI and needed no dispatch change. `docs/native.md` documents ownership and matching Rust-library rebuild requirement.
- All43 real Rust template tests and strict all-target Clippy pass using the actual template source through its existing standalone manifest dependencies. Added Rust regression checks complete large Unicode/NUL action/change JSON, single sync-count increments, invalid-pointer rejection without extra mutation, and release. The generated/snapshot Swift async regression verifies retained large edit, exactly one call and one free per result; scope/routing fixtures use the new owned signatures. Full `python3 scripts/test-native-action-queue.py` passes, including existing routing, heartbeat, FIFO, drafts, pickers and Kotlin checks.
- Separate warnings-as-errors Swift consumer links the actual freshly built Rust dylib and actual extracted production declarations/dispatch/model methods: large action JSON, action-bearing binding JSON, Unicode/NUL, exact single execution counts, state sink and large text/1000-item list reads all pass. This is real Rust/Swift linkage on macOS, not a device/UI deployment. Evidence: `/tmp/crepus-owned-action-result-fix/` (before snapshots, red/green logs, Rust gate/build/Clippy, linked probe and task patch).
- Independent scoped review approved allocator matching, per-call identity, error paths, legacy exports and all affected callers. Primary inspected actual diffs; rustfmt/whitespace checks pass. Updated Swift hosts must rebuild their app-local Rust library because older libraries lack the new symbols. iOS-device rendering and other-platform builds remain unverified. No dependencies, commits, pushes, deployment, destructive cleanup or remote mutations.

### 2026-10-18 — Literal dependency paths in Moonshine JSON

- Parallel CLI review found raw dependency-path interpolation into package JSON and printed local dependency snippets. Actual CLI regression reproduced invalid scaffold JSON with a valid local checkout path containing quote/backslash/Unicode.
- Both output paths now use existing serde_json string serialization. The shared package helper takes the slug directly, and scaffold/dev write its completed JSON without running a second placeholder substitution; literal `{{slug}}` inside a dependency path is preserved. Both package-helper callers and existing units updated; no public CLI/API changes or dependency additions.
- Actual CLI integration checks all three canonical file-dependency strings roundtrip exactly through scaffold and printed snippet, including quotes, backslashes, Unicode and literal placeholder text. Shared-helper unit additionally covers Windows-style backslashes and control/newline strings. All12 components/Moonshine integration tests and all4 Moonshine units pass. Independent read-only review found no missed caller; primary inspected actual before/current diffs and rustfmt/whitespace checks pass. Evidence: `/tmp/crepus-moonshine-json-path-fix/`.
- Separate current runtime library gate passes94 tests;11 Metal-device tests are ignored by their existing attributes. Log: `/tmp/crepus-runtime-current-audit.log`. Windows filesystem/package installation, Moonshine host compilation, physical GPUI rendering and the Metal tests were not run in this pass.
- Parallel LSP source-only candidate retained for investigation: completion/hover line helpers appear not to recognize lone-CR line endings. No runtime reproduction or LSP repair was performed here. No commits, pushes, deployment, destructive cleanup or remote mutations.

### 2026-10-18 — Consistent CR/LF/CRLF LSP positions

- Revalidated the LSP line-ending candidate against the protocol's document line representation (Microsoft specification explicitly recognizes LF, CRLF and CR: https://github.com/Microsoft/language-server-protocol/blob/gh-pages/_specifications/specification-3-14.md). Actual public-query regression passes LF/CRLF controls and fails lone CR before the change. Parallel review also traced core diagnostic offset mapping to LF-only scans.
- Added one shared original-byte-offset line iterator under core diagnostics, recognizing CR/LF/CRLF and retaining trailing/empty lines. Diagnostic byte positions clamp within EOL sequences to the preceding content boundary; diagnostic endpoints and fallback first-line ranges use the same iterator. LSP context prefix, hover line lookup and local declaration scans reuse it. Parser input is unchanged; this does not silently normalize documents or broaden indentation-parser syntax.
- Core regression verifies mixed endings, original UTF8 offsets, UTF16 columns, CRLF interior boundaries, empty/trailing lines and fallback range. Public LSP regression covers completion, hover, declaration discovery and JSX diagnostic line placement for all three endings with a preceding non-BMP character. An expanded test initially expected source-order declarations instead of their existing alphabetical order; corrected the expectation without changing declaration ordering.
- All517 core library units and full LSP gate (36 library plus2 lifecycle tests) pass. Strict all-target core/LSP Clippy, rustfmt and whitespace checks pass. Actual freshly built `crepus-lsp --stdio` probe initializes, opens a Unicode/lone-CR document, requests completion/hover, changes it with a newer version, receives line1 diagnostics and shuts down cleanly. First temporary probe omitted required --stdio and was corrected after inspecting main; no server change was needed.
- Independent scoped review approved UTF8 boundaries, byte identity, CRLF coalescing, range consistency and linear iterator work. Primary inspected actual before/current diffs. Evidence: `/tmp/crepus-lsp-line-endings-fix/` (baseline, core/LSP gates, Clippy/build, real framed stdio probe and patch). No editor GUI, other-platform or full-workspace check; no dependencies, commits, pushes, deployment or remote mutations.

### 2026-10-18 — Tauri audit directory-link identity

- Primary found Tauri source auditing followed directory links without tracking visited identities. A bounded public TauriProject.audit regression with one self-link, one alias and one external linked source produced65 command reports for2 physical source directories before the production change. This establishes repeated scanning/report duplication, not a measured arbitrary-project hang.
- The walker now records each canonical directory identity before reading it, retaining the first encountered logical path for source attribution. Valid linked external directories remain scanned; existing ignored directory names remain excluded. Canonicalization failure follows the existing best-effort unreadable-directory policy. No public API/classification changes, file-alias/hardlink deduplication or concurrent-filesystem guarantee.
- Regression verifies one report per source directory, external linked-source retention and ignored target content. Full all-feature Tauri/macros gate passes24 tests including package-alias compile/negative diagnostics and async-command compatibility; strict all-target/all-feature Clippy, rustfmt and whitespace checks pass. Independent scoped review approved compatibility and boundaries; primary inspected actual before/current patch. Evidence: `/tmp/crepus-tauri-audit-links-fix/`; initial pre-edit full gate also passed at `/tmp/crepus-tauri-current-audit.log`.
- Parallel web-extension content lifecycle review found no new confirmed issue in startup, streamed codeblock retries, mounting, resize ownership or URL cleanup. This was read-only source review, without extension/browser execution. Windows filesystem/junction and GUI tests were not run. No dependencies, commits, pushes, deployment, destructive cleanup or remote mutations.

### 2026-10-18 — Preflight duplicate built-in docs outputs

- Primary found built-in Markdown docs generation maps README.md (case-insensitively) and index.md to the same index.html, previously overwriting by filesystem enumeration order. Actual generator regression reproduced a successful return despite the collision and overwritten destination.
- Generator now rejects duplicate derived output stems with InvalidInput during its existing first pass, before output directory creation, page writes or search-index writes. Error identifies the colliding output and an input path. Existing web-build caller warning behavior is unchanged: this is docs-generator rejection, not a claim the whole site build now fails or is transactional.
- Regression covers README.md/index.md and Readme.md/index.md collisions, preserving an existing page/search index and preventing writes of unrelated cli.html. A unique-page control preserves normal README-to-index, named pages and search entries. All159 slim CLI binary units and strict slim CLI Clippy pass; rustfmt/whitespace checks pass. Independent review approved first-pass placement and unchanged caller policy; primary inspected actual before/current diff. Evidence: `/tmp/crepus-docs-output-collision-fix/`. No visual design/rendering change, browser or other-platform gate.
- Parallel follow-up found source-backed dev-discovery candidates in web_serve.rs recursive startup validation/VFM collection and moonshine.rs recursive copy. These are not repaired here. Unlike physical-source audit deduplication, virtual-file collection must preserve logical aliases; a correct next fix should guard canonical ancestry cycles without globally dropping alias paths. No runtime hang or latency claim from the source-only review.
- No dependencies, commits, pushes, deployment, destructive cleanup or remote mutations.

### 2026-10-18 — Cycle-safe web dev source discovery preserving aliases

- Revalidated the startup-discovery candidate with actual validation/VFM helpers and a bounded filesystem fixture: one self-link, two independent directory aliases, one external linked source and five generated-directory fixtures. Before the fix startup validation produced99 entries rather than10. This proves repeat traversal, not a browser-latency or arbitrary-project hang measurement.
- Both recursive collectors now use one private walker with canonical ancestry tracking. It removes identities on return (including unreadable-directory paths), stopping only ancestor cycles while retaining independent logical alias paths. Returned paths are not canonicalized. Validation retains its existing all-directory policy; VFM collection retains dist/target/.git/node_modules/.crepus-dev exclusions.
- Regression verifies10 logical validation sources (including the five excluded-from-VFM fixtures) and exact five VFM keys with both aliases, original source, entry and external link. Alias contents match the real source. All160 slim CLI binary units, strict slim CLI Clippy, rustfmt and whitespace checks pass. Independent scoped review approved ancestry ownership/cleanup, policy distinctions and caller coverage; primary inspected the actual before/current diff. Evidence: `/tmp/crepus-web-discovery-cycles-fix/`.
- Full WASM dev-server startup, GUI/browser latency and Windows junction behavior were not exercised. The separate Moonshine recursive-copy cycle candidate remains unmodified and source-only. No dependencies, commits, pushes, deployment, destructive cleanup or remote mutations.

### 2026-10-18 — Moonshine copy-source graph preflight

- Reproduced the remaining Moonshine recursive-copy candidate using the actual private copier and a real self-linked source directory: old code returned success after repeated nested copies and replacing existing output. The regression expected explicit loop rejection and failed before production edits.
- Reused the installed CLI WalkDir dependency with link following to collect/validate the complete source graph before copying. This detects ancestor cycles and rejects broken links. Canonical overlap checks reject destination inside/equal to source and source entries pointing into the output tree. Both current callers already create the destination, so the private helper now explicitly copies into an existing directory; no generic path resolver or new dependency was needed.
- Copy phase preserves logical alias paths, nested regular files and valid external linked sources; nonregular entries remain uncopied. Corrected the inaccurate nonrecursive helper comment. Broken source links now produce a deliberate error rather than the former silent skip. This is source-graph preflight, not atomicity for subsequent disk errors/races or rollback of earlier build writes.
- Real filesystem regressions verify loop error before replacing an existing file, no generated loop subtree, ordinary nested/aliased/external copying, existing unrelated output retention, source/output overlap and broken-link rejection. All163 slim CLI binary units and strict slim CLI Clippy pass. Independent scoped review approved both callers, overlap checks, regular-file filtering and alias behavior. Primary inspected actual before/current patch; rustfmt/whitespace checks pass. Evidence: `/tmp/crepus-moonshine-copy-graph-fix/`.
- Full Rust-to-Vite build, browser output, Windows junctions, hardlink identity and racing filesystem behavior were not exercised. No commits, pushes, deployment, destructive cleanup, tool installs or remote mutations.

### 2026-10-18 — Reject malformed UTF16 at npm WASM parser boundary

- Parallel review found npm source/filename strings pass through TextEncoder in generated wasm-bindgen glue, replacing lone UTF16 surrogate units before Rust can diagnose them. Actual public npm regression reproduced `span "\\uD800"` returning text U+FFFD instead of an error. Existing context strings use the strict JSON path and are separate.
- Added a private Unicode-mode surrogate-range check to both npm parser source entrypoints and the optional parseTemplate filename before encoding. Invalid units throw TypeError; paired astral scalars, the maximum Unicode scalar and literal U+FFFD remain unchanged. README documents this input behavior. Generated glue, parser binaries and Rust are unchanged.
- All68 npm tests pass against the existing actual packaged WASM, including six malformed-unit patterns across both source entrypoints/filename, valid Unicode and existing frontend/IR contracts. Strict ES2022/NodeNext TypeScript typechecking passes using the already installed compiler through Bun. Independent scoped review approved Unicode regex semantics, both callers and compatibility; primary inspected all three actual before/current diffs and whitespace checks pass. Evidence: `/tmp/crepus-wasm-unicode-boundary-fix/` (red/green, snapshots, typecheck and patch).
- Separate current ABI/web library gate passes11 ABI and8 web tests: `/tmp/crepus-abi-web-current-audit.log`. ABI error/reentry/session source inspection found no new confirmed issue in this bounded pass. These are library-level checks, not full SDK/browser/device coverage. No new WASM build, browser-engine matrix, dependencies, commits, pushes, deployment or remote mutations.

### 2026-10-18 — Current full core integration and watcher acceptance

- Revalidated the current worktree after the shared diagnostic-line iterator and npm boundary changes with `CARGO_BUILD_JOBS=1 cargo test -p crepuscularity-core --tests`. All765 tests pass across46 completed library/integration test binaries, with no failures or ignored cases. This includes517 library cases plus248 integration cases covering malformed frontend syntax, declarations, includes, attribute boundaries, cache behavior, comparisons and diagnostics. Log: `/tmp/crepus-core-integration-current-audit.log`.
- Default features compile watcher integration as zero tests. Closed that verification gap explicitly with notify-enabled watch_tests (3 passed) and watch:: library cases (11 passed), including subscription failure/retry, overlapping-root transitions, replaced directory identity and single-file watcher policies. Log: `/tmp/crepus-core-watch-current-audit.log`. These focused feature checks add14 watcher cases rather than claiming the default run exercised them.
- Parallel read-only Angular/Astro structural review found no new confirmed issue in directives, closures, frontmatter byte preservation, fragments, replacement children or explicit unsupported-syntax errors. No speculative parser edits were made. Whitespace checks pass; only this audit evidence was added during the pass.
- Other platforms, actual editor/extension/browser processes, physical UI latency and full-workspace/all-feature acceptance remain outside these gates. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations.

### 2026-10-18 — Execute previously ignored GPUI Metal paint gates

- Closed an earlier explicit verification gap: the current host provides a usable Metal device. First ran the exact ignored text/unsupported-island paint fixture successfully, then `CARGO_BUILD_JOBS=1 cargo test -p crepuscularity-runtime --lib -- --ignored --test-threads=1`. All11 device-dependent tests pass;94 ordinary tests are filtered out in this command. Existing ignore attributes remain appropriate for hosts without a GPU.
- Inspected fixture mechanics: they create headless GPUI windows, draw through the native renderer, inspect captured pixels, and dispatch actual GPUI mouse/focus/scroll input for interaction cases. Passing cases include unsupported-island diagnostics/text, worker-prepared frozen/refreshed include snapshots, color opacity, grid/flex, active/focus colors, scroll movement/clipping, dynamic/runtime shadow resets, context-driven quoted dimensions, animation shrinkage and forwarded-slot colors. This is rendered-output/interaction evidence beyond saving screenshots, but not a physical display latency benchmark or no-lag guarantee.
- Logs: `/tmp/crepus-gpui-metal-current-audit.log` (single capability check) and `/tmp/crepus-gpui-metal-all-current-audit.log` (all11). Cargo emits an existing dependency future-incompatibility warning for block0.1.6; it did not fail these tests and was not changed.
- Added the verified serial Metal test command and its frame-time scope to docs/runtime.md, preserving concurrent documentation edits; primary inspected the actual before/current patch and whitespace checks pass. Doc snapshot/patch: `/tmp/crepus-gpui-metal-audit/`. No production code changes in this pass.
- Parallel read-only reactive cleanup review found no new confirmed defect: bindings retain IDs rather than DOM nodes, explicit disposal remains documented, and graph/captured-resource cleanup occurs outside borrows. This review is not fresh browser leak testing. Other OSes, physical-window interaction latency and real app workloads remain unverified; no commits, pushes, deployment, dependencies or remote mutations.

### 2026-10-18 — Reject GPUI negative-cache optimization after full recovery gate

- Inspected direct synchronous GPUI include lookups while parallel read-only review found no new worker-handoff issue. Hypothesis: cache Result errors within a render to avoid repeated failure-path reads/parses. Used bolt-measured-performance instructions and existing journal; no new dependency or benchmark framework.
- Temporary actual IncludeCache probe measured1000 repeated missing-file lookups, three warmups and fifteen timed samples in the existing debug test profile: baseline median1,634,208ns; negative-cache candidate1,390,959ns, with two repeat medians1,400,041ns and1,314,416ns. Candidate-specific same-cache stability/fresh-cache recovery checks passed after correcting an invalid-input fixture that was actually accepted by the parser. No claim about valid-template workloads or frame latency. Probe and logs retained at `/tmp/crepus-gpui-error-cache-fix/`.
- The full runtime gate exposed an existing intentional contract missed by the initial scoped review: `failed_named_parses_do_not_poison_the_render_cache` requires the same cache to retry missing→invalid→repaired components. Candidate failed that gate (95passed,1failed,11ignored). Did not weaken the existing test or redefine recovery; reverted all production and new test edits. Confirmed renderer.rs exactly matches its pre-attempt snapshot. Archived rejected patch/probe and added a durable rejected-optimization lesson to .jules/bolt.md.
- Restored full runtime gate passes94 tests with11 device fixtures ignored by default; explicit serial Metal gate passes all11. Logs: restored-runtime.log and restored-metal.log in the evidence directory. Primary inspected final journal diff and whitespace checks pass. No production optimization shipped, no Clippy claim for the rejected candidate, no commits/pushes/deployment/remote mutations. Physical-window latency and a behavior-preserving error-path improvement remain unresolved.

### 2026-10-18 — Preflight output links in Moonshine and web static copies

- Reproduced an actual Moonshine destination-file symlink overwrite: the old copier changed an outside sentinel from `keep` to `new`. Parallel CLI review identified the same boundary in web static copying and implemented that caller with exclusive file ownership; primary integrated the shared helper and Moonshine caller.
- Both copiers validate all destination paths before descendant creation/file writes. The shared helper rejects paths escaping the canonical output root through existing links and rejects unresolved/dangling links. Contained links and explicitly selected root aliases remain supported. Web copy also collects the source walk before destination writes, so a source traversal error cannot leave partial descendant copies.
- Actual filesystem regressions cover external file/directory links, dangling links, unchanged existing output on rejection, ordinary nested copies, and contained file/directory links through an explicit root alias. All167 slim CLI binary tests and strict slim CLI Clippy pass. Independent read-only integration review found no concrete blocker in root aliases, contained/dangling links or write ordering. Primary inspected the actual three-file before/current diffs and all caller sites; rustfmt and whitespace checks pass. Evidence: `/tmp/crepus-copy-output-links-fix/` (baseline overwrite, current gate, Clippy, snapshots and task patch).
- This is stable-filesystem preflight, not protection against concurrent link replacement or hardlink aliases, nor whole-build atomicity. Other output writers, Windows junction behavior and full Vite/browser builds were not exercised. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations.

### 2026-10-18 — Preserve popup checkbox edits across delayed settings reads

- Current-state inspection confirmed the Vue awaited-loop diagnostic was already implemented and verified; no duplicate edit. Parallel read-only webext manifest/capability review found no new validated defect. Current webext Rust gate passes55 tests; initial shipped-JS gate passes17 tests.
- Added a tests-first actual popup.js regression: a deferred settings read completed after an enabled checkbox edit and reset the displayed value despite the storage write receiving the new value. Baseline fails with expected false/received true. Per-setting revision snapshots now discard reads predating edits while untouched settings continue loading.
- Independent source review identified the second ordering: read starts during a pending write, then that write finishes before the stale read resolves. Symmetric enabled/autoRender regressions fail the revision-only candidate. Pending-write counts captured at read start close this gap; revision changes cover writes started after the read. Counts clear on resolved/rejected writes. Later reads after settled writes still apply normally, verified by regression.
- Final all21 JavaScript tests pass, including both settings/read orderings and existing storage/clipboard failure tests. Actual shipped popup.js ran in headless Chromium with routed mock storage and fixture markup: real Help/Back clicks, checkbox changes and delayed promise completion pass all4 scenarios. This verifies browser interaction without claiming an installed extension or real browser sync storage test. Current55 Rust tests and strict all-target webext Clippy pass. Primary inspected both scoped before/current diffs; whitespace checks pass. Independent review approved the integrated ordering guards. Evidence: `/tmp/crepus-popup-settings-race-fix/` (two red stages, green tests, browser fixture/log, Rust/Clippy and scoped patch).
- Existing silent persistence-failure policy is unchanged; this does not add durable write guarantees, serialize multiple saves, or establish cross-browser/full-extension acceptance. No dependencies, commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Current runtime thread-boundary and strict-lint audit

- Previous turn made verified progress on popup settings ordering. Returned to the responsiveness requirement by inspecting current HotReloadState load requests, background execution, snapshot publication/invalidation and prepared include lookup. Independent read-only handoff review found no new validated defect against existing edit-discard, caller-variable, root/VFS/slot refresh, recovery and watcher lifecycle contracts. Did not revive rejected negative-result caching.
- Confirmed watcher creation, root/include reads, resolution and parsing execute through the background executor; prepared rendering reads retained graph data. GPUI expression evaluation, props/defaults and element construction remain on the UI thread, as explicitly documented. This audit does not claim every target or arbitrary application work runs off-thread, nor any frame-latency guarantee.
- New current strict gate `CARGO_BUILD_JOBS=1 cargo clippy -p crepuscularity-runtime --all-targets -- -D warnings` passes. Log: `/tmp/crepus-runtime-clippy-current-audit.log`. Existing block0.1.6 future-incompatibility warning remains. Did not repeat unchanged full runtime/Metal tests merely to expand counts.
- Corrected stale hot_reload.rs comments after inspecting current managed watcher implementation: both logical/resolved directories and supported frontend changes are monitored; drop signals background cleanup without joining on the UI thread. Runtime behavior unchanged. Primary inspected scoped before/current diff; rustfmt and whitespace checks pass. Snapshot/patch: `/tmp/crepus-runtime-lifecycle-doc-audit/`.
- Remaining responsiveness evidence gap is physical-window frame/input latency under representative workloads. Existing headless Metal paint/input gates and worker handoff tests do not prove that. No dependencies, commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Framework control and embedded boundary audit

- Reviewed current React/Moonshine JSX, Solid, Vue, Svelte, Imba and Moonshine DOM dropzone/file-picker emission against the shared View IR and documented frontend contracts. Verified drop default cancellation and raw-event forwarding in source; file-picker accept/multiple attributes remain distinct from dropzone metadata. No new reproducible defect found in this bounded pass; no speculative output changes.
- Parallel read-only embedded image/color/layout audit found no new validated conversion or clipping defect. Image decoding/rendering remains outside its documented supported subset, so absence was not reported as a regression or silently added.
- Current default-feature native library gate passes all352 tests without ignored cases: `CARGO_BUILD_JOBS=1 cargo test -p crepuscularity-native --lib`, log `/tmp/crepus-native-default-current-audit.log`. This checks the lightweight default build and shared IR/framework emitter library tests; it is distinct from previously recorded web-modules gates and is not full source-framework or host-runtime compatibility proof.
- Only this evidence entry changed in the pass. Primary inspected the append and whitespace checks pass. Browser/compiler matrices, physical embedded hardware and application frame latency remain unverified here. No dependencies, commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Keep header paths inside generated binding banner comments

- Investigated the remaining popup multiple-save ordering concern without adding a speculative queue: the shim alone does not prove actual browser storage completes writes out of order. Existing popup fix and behavior remain unchanged; that persistence-ordering question still needs authoritative platform/runtime evidence.
- Bindgen source review exposed a separate concrete boundary: shared banner formatting interpolated raw header paths into line comments. A real Unix header directory containing LF/CR/Unicode line separators produced generated Rust which failed syn parsing (`expected !`), despite successful ABI parsing/generation. Baseline regression log retained.
- Shared banner now escapes the displayed diagnostic path with escape_default and escapes PHP closing-tag/Python encoding-cookie delimiters. Original Path values still reach every backend and filesystem operation. No binding signatures, package manifests, checked-in generated outputs or dependencies changed. New Unix actual-path regression checks Rust syntax, silent PHP import, and Python compile from bytes; existing magic-comment/opening-tag behavior remains covered.
- Full plugin-bindgen suite passes27 tests, with3 existing real-ABI integration tests ignored. Strict all-target Clippy, rustfmt and whitespace checks pass. Independent read-only review approved production escaping; its Windows fixture portability concern was resolved with cfg(unix). Primary inspected both actual scoped before/current diffs. Evidence: `/tmp/crepus-bindgen-header-path-fix/` (red/green, Clippy, snapshots and patch).
- Scope is the shared diagnostic banner. Backend-specific include-filename handling and Windows filesystem paths were not newly exercised; ignored D/Nim/Odin real-ABI gates were not run. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Compile Java bindings with unusual header paths

- Follow-up parallel source review found the reachable Java extra backend also embeds raw header basenames in a second line comment. Extended the actual Unix header-path regression to a newline-containing basename and Java output, then ran javac. Baseline reports an uncommented basename fragment plus two illegal Unicode escapes.
- The compiler additionally exposed a regression in the preceding shared-banner fix: Java preprocesses Unicode escapes before comment parsing and rejects Rust-style backslash-u-brace escapes there. Corrected it rather than weakening the fixture. Shared comment_text now changes generated Unicode escapes to inert backslash-x-brace display text and uses backslash-x delimiter escapes; original path/backslash semantics to backends remain unchanged. Both shared banner and Java basename comments use this helper.
- Full plugin-bindgen suite passes27 tests with3 existing D/Nim/Odin real-ABI cases ignored. The actualpath fixture now checks Rust parsing, silent PHP import, byte-based Python compilation and javac compilation together. Strict all-target Clippy, rustfmt and whitespace checks pass. Independent source review approved Java Unicode/backslash eligibility and both callers; primary inspected final scoped production/test diffs. Evidence: `/tmp/crepus-bindgen-java-path-fix/` (actual javac red/green, full suite, Clippy, snapshots and patch).
- Investigated manifest output-base handling but did not impose new path-confinement semantics without an existing contract; manifest/backend filesystem paths remain unchanged. V custom header flag quoting remains a separate unverified candidate requiring V-specific compiler evidence. Windows filesystem/Java execution/JNI linkage and the3 ignored real-ABI tests were not exercised. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — V Darwin rpath supports project directories with spaces

- Followed the remaining V path candidate with installed V0.5.1 compiler probes and parallel source review. Ordinary spaces in -I paths already compile; V quotes those itself, so no speculative include-flag quoting. Actual Darwin rpath with spaces fails: clang treats `with` and `spaces/target/debug` as separate input files. Quoting that specific rpath value succeeds.
- Updated one generator directive and the matching checked-in V binding line. No ABI signatures, include/search-directory behavior or dependencies changed. Added an explicitly ignored macOS V/C-compiler integration regression and ran it locally: copies the actual ABI header to a custom project path with spaces, builds a dummy dylib, generates bindings, then compiles/links V with the emitted directives. Tests-first baseline fails with split-path clang errors; fixed fixture passes. It proves compiler/link-path handling, not behavior of a real ABI library in that fixture.
- Full bindgen default gate passes27 tests with4 ignored (three existing real-ABI cases plus the new explicitly exercised V compiler fixture). Strict all-target Clippy, rustfmt, whitespace and independent source review pass; primary inspected both production lines and actual new fixture diff. Ordinary V plugin gate initially selected installed crepus and failed its IR7 assertion (installed executable returned5). Rerun with CREPUS_BIN set to the checkout target/debug/crepus passes all five V test functions, including real ABI session/callback/string checks and canonical CLI rendering. Installed executable unchanged.
- Evidence: `/tmp/crepus-v-rpath-space-fix/` (red/green generated compiler fixture, full bindgen gate, Clippy, initial/corrected V plugin logs, snapshots and patch); exploratory compiler probes at `/tmp/crepus-v-path-probe/`. General quote/comma/directive-delimiter path handling remains a separate bounded limitation; no claim of arbitrary V path support or other-OS validation. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Reject unrepresentable V ABI directive paths explicitly

- Source review disproved a suspected plugin CLI exit-status bug: ui::error diverges with exit1 and bindgen/test failure paths reach it. No CLI error-status edits or redundant process probes.
- Followed confirmed V compiler/path parser limits from the previous pass. Actual V0.5.1 evidence shows double-quoted directories and directive separators fail compilation or change parsed paths. Before the fix, generate_extra(v, quote-containing-header) still returned success and malformed source; tests-first runtime assertion failed. An initial test mistakenly called Result APIs on the String-returning private helper; corrected the test boundary to existing generate_extra before collecting the actual red result.
- V generator now returns Result and propagates explicit diagnostics through its existing caller. Checks emitted directory/name fields for CR/LF, double quotes and directive delimiters; flag directories also reject separators and V path-macro prefixes, while derived library paths reject commas consumed by Darwin -Wl. Non-UTF8 paths fail instead of lossy substitution. Ordinary spaces, Unicode, apostrophes, dollar signs and comma-containing basenames remain admitted. Checked-in output for supported defaults is byte-identical.
- Independent installed-compiler source review identified that V recognizes bare path-macro prefixes without requiring parentheses; corrected both checks and added bare-prefix regressions. Full28 bindgen tests pass with4 ignored defaults, strict all-target Clippy passes, and the explicitly run generated V/C compiler fixture with spaced include/library directories passes. Three other ignored real-ABI integration gates were not repeated. Primary inspected actual production/tests/docs diffs; rustfmt and whitespace checks pass. Evidence: `/tmp/crepus-v-directive-path-fix/`.
- Updated plugins README with explicit V path limits and actual Unix script-test Python/Java requirements. No arbitrary-path or Windows compiler guarantee; failure can still leave earlier package outputs/created directories, and this adds no multi-package atomicity. No dependencies, commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Valid Cargo runtime names for admitted web site names

- Parallel scaffold review identified web-specific name normalization: directory slugs preserve Unicode alphanumeric characters, but Rust package names require XID characters and cannot start with digits. Actual preexisting checkout CLI successfully scaffolds app²; Cargo rejects its generated app²_runtime manifest. Separate dependency-free Cargo probes also reject123_runtime while accepting accented and Roman-numeral identifiers. Earlier desktop/TUI suspected names were accepted by Cargo metadata and were not changed.
- Web scaffolding now derives only the runtime identifier with XID_continue replacement and a leading underscore when needed. Existing display names, site directory slugs, ordinary runtime names and scaffold dependencies remain unchanged. The runtime suffix ensures nonempty output. No new helper/dependency or stricter site-name admission.
- New regression validates actual generated Cargo manifests for app²,123 site,Éclair and東京 using Cargo read-manifest without dependency resolution. Full168 slim CLI binary tests pass; strict slim CLI Clippy, rustfmt and whitespace checks pass. Independent source review approved naming/compatibility and primary inspected the actual scoped diff. The initial intended unit baseline compiled after the source edit and passed, so it is explicitly not red evidence; the separate actual preexisting CLI/public Cargo failure is the authoritative baseline. Evidence: `/tmp/crepus-web-runtime-name-fix/` (public-baseline.log, initial attempt, current suite, Clippy, snapshot and patch).
- No full generated runtime dependency build/WASM/browser build or Windows filesystem validation. Cargo read-manifest is deliberately used for isolated manifest validation and does not prove compilation. No dependencies, commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Share valid Cargo names across extension and Rust Moonshine scaffolds

- Followed the web scaffold naming fix to every same-pattern Rust-producing caller. Parallel review confirmed Webext; primary confirmed Rust-only Moonshine. Public integration regression runs webext new, init webext, moonshine new and web new with superscript/numeric-leading/accented/ordinary-hyphenated names. Before edits it fails on Webext app²_runtime, rejected by Cargo. Earlier speculative reserved package names self/super/crate/type were accepted by real minimal Cargo checks and were not changed.
- Extracted cargo_package_name into the existing scaffold module and reused it for Webext and Rust Moonshine manifests. Shared normalization retains internal hyphens and XID characters, replaces other characters and prefixes invalid starts. Web retains its prior hyphen-to-underscore policy by normalizing those first. Display labels, directory slugs, ordinary valid package names and other generated assets are unchanged. No new dependency.
- Actual CLI/Cargo integration now passes16 combinations and asserts exact expected names, preserving existing ordinary naming. All168 slim CLI binary units pass. Full web_cli initially reports29 passes/2 failures: docs/reference WASM builds used Homebrew rustc without the installed WASM target. Reused the current integration executable with coherent installed rustup Cargo+rustc and serial web_build_ filter; all4 selected cases pass, including both actual WASM builds and explicit Cargo-selection behavior. Thus every one of the31 web integration cases has passing scoped evidence, with the toolchain distinction retained. No global installation/configuration edits or test weakening.
- Strict slim CLI Clippy including web_cli, rustfmt, whitespace and independent caller/compatibility review pass. Primary inspected actual five-file diff; artifacts: `/tmp/crepus-scaffold-cargo-names-fix/` (red/green, default/full and coherent-toolchain logs, Clippy, snapshots and patch).
- Generated numeric/Unicode scaffold dependency builds and real browser/extension runtime behavior remain unverified; successful WASM builds are existing docs/reference sites. Default Homebrew Rust still lacks its WASM target. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Fresh numeric-name WASM scaffold and actual worker-render acceptance

- Continued generated-identifier review: extension page exports are intentional quoted JS property lookups, not generated Rust identifiers; no speculative rename. Parallel read-only native/iOS app-name review found fixed/native or existing XID-normalized identifiers; no new validated mismatch. Share-extension target naming was outside that bounded app-name review.
- Closed a larger verification gap by scaffolding a fresh `123 site` web project through the current public CLI. It produces directory123-site and valid _123_site_runtime package. Used coherent rustup Cargo/rustc and the scaffold's published dependencies (not sibling path substitutions). Initial offline attempt lacked cached published core; permitted ordinary Cargo dependency retrieval and the actual runtime compiled to WASM.
- Packaging then correctly rejected resolved wasm-bindgen0.2.129 against installed CLI0.2.126, reporting both without modifying tools. Kept repository scaffold/dependency declarations unchanged. Constrained only the disposable fixture runtime dependency to =0.2.126; complete public web build then passes and emits HTML/bundle/JS/WASM. This qualified acceptance must not be described as an unmodified fresh-scaffold full build with the older installed bindgen tool.
- Actual headless Chromium over a temporary local HTTP server renders the expected styled Hello content. Wrapped Worker to record request IDs/kinds and successful correlated replies: default runtime-worker.js receives init+render and replies successfully on initial load and reload, with no page errors. Initially checking visible Hello alone raced prerendered HTML; corrected the probe to wait for the correlated worker render reply before asserting. Screenshot opened/inspected; browser and server closed. Optional absent crepus-islands.json requests return404 without page errors. This is real default worker-render evidence, not main-thread DOM elimination or frame/input latency proof.
- Updated docs/cli.md prerequisites to state exact bindgen crate/executable version matching and coherent explicit CARGO/RUSTC overrides. Primary inspected the actual docs patch and whitespace checks pass. Evidence and runnable browser fixture: `/tmp/crepus-generated-web-acceptance/` (original runtime manifest, schema-mismatch build log, matching build, browser log/script, inspected image and docs patch).
- No production runtime changes or repeated unchanged library tests. Numeric-name web runtime is now compiled and browser-verified with the stated fixture version constraint; arbitrary Unicode, extension/Moonshine dependency builds, other browsers and physical-window latency remain unverified. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Share-extension YAML target names and section-scoped insertion

- Followed the separate share-target path from the native identifier audit. Both public iOS/macOS share commands accept unrestricted names, then previously interpolated them as raw YAML keys. Tests-first actual CLI/XcodeGen probe rejects Share: Docs #1 with a block-mapping parse error. Swift classes and source directories stay fixed and were not changed.
- Shared target-key serialization now JSON-quotes names and escapes raw C1/DEL/NEL/Unicode line separators for YAML preservation. Existing ordinary target-key unit expectations reflect quoted formatting. Actual parser regression covers punctuation, quotes, Unicode and these control/line-break values.
- Independent review caught idempotency collisions: comment #Note and quoted Plain keys could match an unrelated requested literal name, and package NativeShell outside targets could suppress a target of the same name. Added actual collision probes; candidate fails before tightening. Legacy raw-key recognition is limited to safe plain forms with exact two-space headers and empty/comment suffixes; generated quoted keys are always recognized. Legacy ordinary keys with trailing comments remain repeat-safe.
- Scoped matching/insertion to the scaffold-style top-level targets block. Previously appending a target after a subsequent schemes:{} root section breaks YAML; added section regression fails before the fix. New insertion uses UTF8-safe line byte offsets before the next root mapping, leaving other sections intact. Missing supported targets mapping errors explicitly. Code documents the bounded-layout shortcut rather than claiming a general YAML editor; arbitrary authored scalar/key layouts are unsupported.
- Explicitly executed new macOS XcodeGen integration gate passes16 platform/name combinations, each twice, including NativeShell, comment/quoted-name collisions, legacy commented Plain and following schemes section. XcodeGen dump uses an explicit JSON output file: initial probe assumed quiet mode would retain stdout and was corrected after EOF, without changing production behavior for that harness issue. No actual Xcode compilation/signing/device gate is claimed.
- All168 slim CLI binary units and strict slim CLI Clippy including the new integration target pass. New integration is ignored by default on hosts without installed XcodeGen and was run explicitly here. Independent final source/section review approved the repair; primary inspected final actual production/test diff, rustfmt and whitespace checks. Evidence: `/tmp/crepus-share-target-yaml-fix/` (initial YAML red, collision/section red stages, green XcodeGen, unit/Clippy, snapshots and patch).
- Preexisting source-file creation before project validation is unchanged; no transactional multi-file guarantee or support for arbitrary hand-authored YAML styles. No dependencies, commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Settle worker requests whose thrown values cannot stringify

- Returned to worker responsiveness/lifecycle boundaries. Parallel read-only runtime-client review found no new correlation/fatal/disposal defect; existing client registers requests before post, isolates per-request failures and terminates once on fatal transport failures.
- Primary identified worker error reporting itself calls String(error) without a guard. Actual Chromium tests-first export mutates state then throws Object.create(null). The queued read returns3, but the original call never receives a reply and the browser probe reports Transport timeout. This is a lost reply, not a FIFO execution stall or handler replay.
- Worker catch now guards diagnostic string conversion and sends a plain fallback message under the original request ID when coercion throws. Ordinary printable errors and result-clone failures retain their previous behavior. No retry, shared state or additional dependency.
- Extended existing real-browser transport fixture passes: unprintable error rejects before the timeout, queued read sees exactly3 and a subsequent request also returns3. Existing clone-error, selected-render failure, cached initialization failure and no-page-error checks remain green. All24 Bun worker/client/app tests pass (98 assertions). Independent source review approved request-ID correlation, clone-safe fallback and no replay; primary inspected both actual before/current diffs and whitespace checks pass. Evidence: `/tmp/crepus-worker-error-text-fix/` (actual Chromium red/green, Bun gate, snapshots and patch).
- Browser fixture uses actual worker/client source with controlled JS runtime exports; no fresh WASM build or Rust check was needed for this JS-only transport change. Other browsers, arbitrary nonterminating user code and physical-frame/input latency remain outside the evidence. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Worker DOM routing review and browser reload fixture repair

- Primary and independent read-only reviewer checked event/render FIFO ordering, root and scope generation guards, bundle revisions, detached-scope fallback and composition handling. No new concrete production defect found; no speculative runtime change.
- Existing real Chromium reload regression failed waiting for initial content: its mock HTTP routes served app.js but omitted the now-required runtime-client.js import, returning HTML for that module. Added the actual client source route only. Baseline timeout observed; corrected fixture passes module update without navigation, secondary preview navigation and root-relative bundle fetch from nested URLs.
- Real Chromium worker transport regression also passes clone/unprintable failures, FIFO recovery without replay and cached initialization failure. These fixtures use controlled JS runtime exports, not fresh WASM or physical-frame latency measurements. No Rust change or repeated Rust checks.
- Primary inspected the three-line test diff against /tmp/crepus-web-reload-browser.before.py. No commits, pushes, deployment or destructive cleanup. Goal remains active.

### 2026-10-18 — Extension background runtime error replies

- Previous turn made progress by repairing and executing the browser reload fixture. Continued bounded extension background review alongside independent TUI input/event review. TUI reviewer found no new validated defect; paste preservation, focus bounds and cycle guards align with documented host-owned hit-testing/editing.
- Background runtime rejection path called String(error) without guarding coercion. Tests-first injected runtime throws Object.create(null); baseline fails with TypeError: No default value inside error reporting rather than responding. This is distinct from the web worker transport fixed earlier: extension messages use sendResponse and their own handler.
- Guarded only diagnostic conversion and send a plain fallback error through the same response callback. Runtime handlers are not retried, settings paths remain unchanged, and no dependency added. Regression verifies an error reply and continued settings service.
- All22 extension JavaScript tests pass across background, browser shim, popup and content suites (83 assertions). Primary inspected both before/current diffs and scoped whitespace check passes. Artifacts: /tmp/crepus-background-error-fix/ snapshots and red/green logs.
- This is an actual background-source unit fixture with injected extension APIs/runtime, not packaged Chrome/Firefox installation or WASM acceptance. No Rust production change or Rust checks. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Skip detached extension widget render work

- Previous turn made progress fixing background error replies. Continued content lifecycle audit: MutationObserver queues code blocks for a later RAF, but enhanceCodeBlock previously parsed and created shells even when a queued block was removed before that frame. mountWidgetShell only noticed disconnection after shell creation; expensive rendering and possible iframe Blob allocation had already occurred.
- Tests-first regression queues a connected block, removes it before explicitly driven RAF, and expects zero extraction calls. Baseline fails with one extraction. Added the existing DOM isConnected guard before extraction/shell creation; skipped nodes are not marked mounted, so reinsertion still mounts normally.
- All23 extension JS tests pass across4 files (86 assertions), including removal/reinsertion, existing streaming edits, mounted-node deduplication and iframe resize ownership. Primary inspected exact before/current production/test diffs and scoped whitespace check passes. Evidence: /tmp/crepus-content-detached-fix/ snapshots and red/green logs.
- Happy DOM drives MutationObserver and RAF with injected runtime exports; no packaged browser installation, real WASM, quantitative latency or general Blob cleanup guarantee is claimed. No Rust changes/checks, dependencies, commits, pushes, deployment or destructive cleanup. Goal remains active.
- Independent content diff review found no blockers. Parallel native reviewer identified a next regression candidate in native/mobile.rs refresh_mobile_state: historical and current source roots are both rendered against current include files by plan_hot_reload_with_path, potentially treating an include-only edit as Noop and retaining stale last_ir_json. Primary located the planner/cache call sites; runtime regression and repair remain pending. This is source-review evidence, not yet executed reproduction.

### 2026-10-18 — Mobile include reloads diff retained last-good IR

- Previous turn made progress with detached extension rendering and a source-reviewed mobile stale-include candidate. Followed actual refresh_mobile_state/planner/cache/output flow. Planner rerenders historical and current root sources using current dependencies, so include-only edits can compare equal and simultaneous root/dependency edits can omit dependency mutations.
- Initial fixture omitted ctx.base_dir (real dev startup sets it), so its first failure was an include lookup error, not proof of the reported defect. Corrected fixture to use the actual template directory, temporarily removed only the task's retained-IR block and reran baseline. Corrected-red.log proves HotReloadEnvelope sequence1 Noop after child changes Before→After, retaining stale cache. No whole-file restoration over concurrent changes.
- Preserve planner semantic/full-reload gates, but recompute Patch/Noop diffs from deserialized retained last_ir_json against the already rendered new IR. Invalid cached JSON or differing IR versions force full reload; full-reload event payload now uses the same already rendered snapshot stored in cache. No public API/dependency change. Existing source planner remains unchanged for other callers.
- Regression checks include-only refresh, applies emitted mutations to previous IR and compares exact cached output, verifies generated fixture.json includes After, preserves last-good IR when child disappears, and recovers after recreation. All169 slim CLI binary unit tests pass; strict slim CLI Clippy passes. Primary inspected exact two-file before/current diffs, rustfmt and scoped whitespace checks pass. Independent source review approved cache baseline, version fallback and lock scope.
- Evidence: /tmp/crepus-mobile-include-fix/ snapshots, initial invalid-fixture logs, corrected actual red, final green, Clippy and task.patch. No simulator/device live reload gate or concurrent-filesystem snapshot guarantee: code generation still uses its existing separate filesystem reads. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Mobile watcher includes every named frontend

- Previous turn made progress fixing include-only IR refresh. Followed the triggering path: is_template_event admitted only selected root or lowercase .crepus dependencies, despite native include rendering supporting Vue/Svelte/Astro/JSX/Angular files. Independent reviewer confirmed nested Svelte→Vue support in existing native acceptance and parser dispatch contracts.
- Tests-first named dependency regression fails on Part.vue before change. Expanded only path selection to .crepus/.vue/.svelte/.astro/.csx/.jsx/.tsx/.ng and Angular .ng.html/.component.html suffixes, case-insensitively like parser dispatch. Exact selected-root behavior and Create/Modify/Remove event gate remain unchanged. Existing Other.astro rejection assertion was obsolete under dependency support; unrelated notes.txt is still rejected.
- Regression covers10 named file forms ×2 case variants ×3 event kinds (60 combinations), including absent removal/recreation paths without requiring canonicalization. All170 slim CLI binary units and strict slim CLI Clippy pass. Primary inspected the actual one-file before/current diff; rustfmt and scoped whitespace checks pass. Independent diff review approved event-kind/selected-root compatibility. Artifacts: /tmp/crepus-mobile-watch-frontends/ snapshot, actual red/green, Clippy and patch.
- This is watcher classification evidence, not OS-notify timing or live simulator/device acceptance. Arbitrary-extension include dependencies and directory-only events remain concrete dependency-tracking gaps; no general full-graph watcher claim. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Real mobile dev-server watcher acceptance

- Previous turn made progress expanding named-frontend watcher selection. Closed the OS-event/HTTP integration gap with new manual stdlib-only tests/native_mobile_watch.py using the actual current slim CLI binary, a temporary project and loopback server. Rebuilt current CLI successfully; no tool/runtime substitution.
- Probe first verifies initial Before IR, then changes the root until Ready is received to establish watcher registration before the dependency-only test. It then edits Child.vue, requires After in HTTP IR and in the patch event, verifies generated fixture.json, deletes the child and requires an error while IR remains exactly last-good, and recreates the file to require Recovered. Each wait is bounded and checks process liveness; subprocess always terminates/kills and reaps, and temporary files are removed.
- Actual macOS OS watcher test passes. Independent script review identified startup-registration and stale-patch acceptance risks; addressed both with bounded once-per-second root readiness retries and requiring After in the patch event. Re-ran final script successfully. Primary inspected the complete final script and scoped whitespace checks pass. Current CLI build and final OS acceptance log: /tmp/crepus-mobile-watch-frontends/build.log and os-watch.log.
- No additional production code change or repeated Rust unit suite for this acceptance-only addition. This proves current host local dev-server/include behavior, not simulator/device application, other OS watcher backends, arbitrary-extension dependencies or directory-only event handling. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — LVGL boundary review and reproduced deep-memo stack limit

- Previous turn made progress closing real mobile OS watcher acceptance. Switched modules: primary inspected LVGL XML serialization/name/character/whitespace validation, last-attribute precedence, form mapping and included-frontend handling. Existing code/tests cover the reviewed boundaries; no new validated LVGL defect or speculative change.
- Independent reactive reviewer identified recursive lazy memo reads as a bounded probe candidate. Rebuilt current reactive crate, compiled a standalone public-API Rust probe against its actual rlib and ran subprocesses with core dumps disabled. All memo handles stay alive; each memo returns previous.get()+1; computation runs on an explicit2MiB native thread. Depth64 passes; depth4096 aborts with SIGABRT and fatal stack-overflow diagnostic. This is an observed native debug-build limit, not a measured WASM limit or a universal safe-depth threshold.
- Probe/source/logs: /tmp/crepus-reactive-chain-audit/ (probe.rs, binary, build.log, depth-64.log, depth-4096.log). Current source recursively evaluates both lazy user closures and Check-state source validation. A native-only stack-growth dependency would not establish the WASM target contract; no production patch or claim of resolution made. Initial lazy closure recursion remains concrete work for scheduler/stack-contract design.
- Ran current reactive+LVGL package tests:165 pass across37 suites,0 fail;2 manual timing comparisons ignored (repeated_used_slots_scaling and wide_memo_tracking_benchmark). Native package gates do not prove browser DOM/WASM behavior or absence of deep-chain overflow. No commits, pushes, deployment, destructive cleanup, dependencies or remote mutations. Goal remains active.

### 2026-10-18 — Desktop and terminal hot-reload recovery audit

- Previous turn made progress reproducing the native lazy-memo stack limit and verifying reactive/LVGL packages. Continued independent modules rather than introducing an incomplete native-only mitigation.
- Primary inspected desktop request capture/background loading/publication guards, include environment snapshots, live caller variables, watcher retention/retry and renderer error recovery. Current path/template/environment/change guards prevent stale request publication; existing explicit reload semantics cover failed watcher setup. No new confirmed desktop defect.
- Parallel read-only terminal review checked source validation, render/diff snapshot commits, include invalidation, failed-draw retry and flag consumption. Failed draws retain cached output without marking changed inputs successfully rendered; documented reload error behavior remains intact. No new validated terminal defect; registered-file include-origin API issue remains pending as previously recorded.
- Current focused gates pass:16 runtime hot_reload tests,7 TUI hot_template units,3 real include-watch integrations (Vue edits, malformed include recovery, symlink target atomic save). Initial TUI lib hot_reload filter selected0 tests; corrected to actual hot_template names and explicit include_hot_reload integration target, and do not count that empty command as coverage. Logs: /tmp/crepus-hot-reload-review/.
- No production edits or speculative tests added. This is scoped automated/source evidence, not physical desktop-window latency or all backends. Deep lazy memo chains and broader mobile dependency tracking remain concrete unresolved work. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Mobile HTTP routes wait for complete bounded request lines

- Previous turn completed focused desktop/TUI hot-reload source and automated audit. Continued mobile transport boundary: handle_mobile_dev_stream used one TCP read and immediately routed its partial contents. TCP does not preserve request-line packet boundaries; receiving GET /i before r HTTP/1.1 could route /i and return404 prematurely.
- Tests-first real loopback TCP regression sends the prefix, asserts no response within a bounded read timeout, then completes /ir and requires200 plus expected IR. Baseline immediately returns a response byte and fails the partial-line assertion. This is actual socket behavior through the production handler, not a mock stream.
- Replaced one-shot read with standard BufReader/Take/read_until LF, bounded at4097 bytes; lines above4096 bytes (including terminator) or EOF without LF receive400. The reborrow ends before response writing. Existing one-response-per-connection routing/header/method behavior is otherwise unchanged; no new dependency or full HTTP parser.
- Added socket checks for truncated EOF and oversized lines. All172 slim CLI binary unit tests pass and strict slim CLI Clippy passes. Independent source/design review approved bounded accumulation and buffering scope. Primary inspected exact two-file before/current diffs, rustfmt and scoped whitespace checks. Evidence: /tmp/crepus-mobile-http-fix/ snapshots, real red, full green, Clippy and task.patch.
- No device/simulator acceptance or general HTTP protocol conformance claim. Existing blocking thread-per-connection model remains; no connection-deadline/concurrency redesign. No commits, pushes, deployment, destructive cleanup, dependencies or remote mutations. Goal remains active.

### 2026-10-18 — Mobile generated outputs share the authoritative rendered IR

- Previous turn made progress fixing fragmented mobile HTTP routing. Checked Unicode response lengths: existing Rust String byte lengths are correct, no speculative change. Parallel extension settings/message source review found no new concrete issue.
- Found a separate context/snapshot bug: mobile dev startup and refresh rendered HTTP/event IR with --ctx/--var, but sync_outputs invoked separate native Sync/Codegen commands with ctx=None and vars=[], rereading the source/dependencies without those values. Extended existing real-CLI watcher probe with context JSON greeting/title and CLI title override. Actual baseline fixture.json differs from initial /ir despite successful generation logs; recorded real red.
- sync_outputs now consumes the already rendered ViewIr and existing shared default-fixture paths/native source generator. All fixtures and selected Swift/Kotlin output derive from that snapshot, removing separate source reads and discarded-context rendering. Existing Compose package, output paths and trailing-newline policies retained. Content comparison skips unchanged output writes. Startup output failures propagate; reload output failures are logged while the valid live IR/event still advances, preserving live runtime service. No transactional multi-file guarantee added.
- Real current CLI build and manual OS-watcher acceptance pass with --platform all: initial/reloaded fixture JSON equals HTTP IR exactly, context and CLI override values survive, Swift/Kotlin literal child output changes Before→After with package preserved, missing child keeps last-good IR and recreation recovers. Initial source check incorrectly expected resolved context text inside generated dynamic-binding source; corrected to the actual codegen contract and literal child assertions. This harness correction does not invalidate the independent baseline fixture mismatch.
- All172 slim CLI binary units and strict slim CLI Clippy pass. Primary inspected exact production/manual-test diffs, rustfmt and scoped whitespace checks pass. Evidence: /tmp/crepus-mobile-output-snapshot/ snapshots, baseline build/red, current build/unit/Clippy/real-server green logs and task.patch.
- No Swift/Kotlin compilation, simulator/device execution, atomic output commit, or output-write retry redesign claimed. Dynamic bindings retain the existing host model contract. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.
- Independent final production review approved snapshot/output/error scope. Reviewer suggested polling generated sources after fixture writes; existing probe already awaits the After-bearing patch event, which refresh stores only after synchronous output generation, so source assertions are ordered after both writes without extra polling.

### 2026-10-18 — Native sync validates actual destination bytes

- Previous turn made progress sharing mobile rendered IR with generated outputs. Audited separate native sync caching: DriverCache records historical generated JSON hashes by fingerprint, not the contents of any output path. Both default/explicit skip conditions could therefore report successful sync while retaining corrupted files or newly requested stale files.
- Public CLI tests-first regression syncs a minimal native project, corrupts a default fixture with invalid UTF8 bytes and introduces stale extra.json only on the second invocation. Sets an exact sentinel mtime on the still-correct root fixture. Baseline fails because default output retains [255,254].
- Replaced both skip conditions with direct destination-byte equality. Removed this function's redundant cache/fingerprint bookkeeping; other DriverCache callers/API and existing cache files untouched. Existing default-parent filtering, explicit parent creation, no-defaults gating, errors and output counting retained. Correct files now preserve mtimes even without cache metadata; stale/invalid-byte outputs repair normally.
- All30 native_ir_cli integrations and172 slim CLI binary units pass. Regression verifies both repaired files and unchanged sibling mtime without timing sleeps. Strict slim CLI Clippy including native_ir_cli passes; independent final source review approved. Primary inspected exact two-file before/current diffs, rustfmt and scoped whitespace checks. Evidence: /tmp/crepus-native-sync-cache-fix/ snapshots, actual CLI red/green, unit/Clippy logs and task.patch.
- Search found a separate analogous historical-cache skip in Webext page output (webext.rs); review/reproduction remains pending, not silently fixed in this task. No all-target cache or filesystem-race guarantee, device/build proof, commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Webext page output validates actual HTML bytes

- Previous turn made progress repairing stale native fixtures. Followed the analogous Webext page skip: render_crepus_page rendered HTML, then trusted DriverCache historical hash plus output.is_file instead of destination bytes. Tests-first extension page regression renders, overwrites HTML with invalidUTF8 [255,254], then rerenders. Baseline retains corrupted bytes and fails exact expected HTML comparison.
- Replaced skip with actual destination-byte equality, removed this private page path's unused DriverCache/Fingerprint import/open/argument/record and misleading rerender-skip comment. One private caller updated; HTML rendering already occurred before the old check. Shared public cache API and existing on-disk metadata untouched. JS client writing remains unchanged.
- Extended regression repairs HTML then sets/reads exact sentinel mtime and rerenders matching output; HTML mtime stays unchanged without timing sleeps. All172 slim CLI binary units and strict slim CLI Clippy pass. Primary inspected exact one-file before/current diff, rustfmt and scoped whitespace checks; independent proposal review confirmed scope/caller compatibility. Evidence: /tmp/crepus-webext-page-cache-fix/ snapshot, actual red/green, Clippy and task.patch.
- Repository crates search now finds no production is_up_to_date callers beyond the cache API itself (tests retain contract coverage); no public API deletion. No packaged extension/browser/WASM rebuild needed for this filesystem-only change and none claimed. No all-output atomicity/TOCTOU guarantee, commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Hydration payload and DOM binding lifecycle source review

- Previous turn made progress repairing Webext page-cache outputs. Switched to reactive hydration/DOM boundary: primary inspected raw/base64 decode, canonical version/context selection, legacy envelope handling, invalid-data fallback, typed live control writes and URL attribute normalization. Parallel reviewer independently inspected hydration/binding cleanup and current test contracts.
- No new confirmed defect. Rust-owned serde_json/HashMap values preserve ordinary application keys; malformed decode returns empty context and malformed canonical ctx does not leak envelope metadata. Binding effects retain IDs rather than DOM nodes, resolve current elements per update and require explicit disposal; missing/replaced elements and live form behavior align with current documented contract.
- Inspected existing11 actual-WASM DOM tests and recorded prior current controls-browser acceptance. No production edits, duplicate regressions or unchanged browser-test reruns added for this no-finding source audit. Do not present historical acceptance as newly executed; current turn source review is the evidence. Known deep lazy-memo stack limit remains unresolved and was not weakened or hidden.
- No all-application hydration, browser performance or cross-browser claim. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Mobile codegen propagates actual generation errors

- Previous turn completed hydration/DOM source audit without a new defect. Continued mobile wrapper arguments/error handling. run_codegen_full returned Result but discarded execute(NativeCommands::Codegen) failures then returned Ok, so public mobile codegen silently succeeded for missing/invalid input or output write failures. Mobile Build already propagated run_codegen_full with ?, so the ignored inner error could also let it advance beyond failed generation.
- Actual CLI tests-first missing-file case exits successfully on baseline and fails regression (ios missing.crepus incorrectly succeeded). Replaced only the discarded generation result with ?. Android package prepend now runs only after successful generation; output paths, vars and successful source behavior unchanged. Existing mobile-new wrapper policy remains outside this fix.
- Public regression covers missing file, malformed template and blocked output directory for both iOS/Android (six failures with diagnostics), then successful generation for each, checking expected source content and Android package header. All31 native_ir_cli integrations plus172 slim CLI binary units pass; strict slim CLI Clippy including native_ir_cli passes. Independent source/caller review approved propagation and build short-circuit; primary inspected exact production/test diffs, rustfmt and scoped whitespace checks.
- Evidence: /tmp/crepus-mobile-codegen-errors/ snapshots, actual red, integration green, final complete gates, Clippy and task.patch. No Xcode/Gradle/device execution or transactional generation claim. Parallel emitter reviewer identified an unverified CR-normalization candidate in Svelte/Vue static attribute escaping; host-compiler reproduction remains pending.
- No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Preserve carriage returns in Svelte emitted attributes

- Previous turn made progress propagating mobile codegen errors; followed parallel emitter CR-normalization candidate through actual host compilers. Used existing installed MIT Svelte5.57.1 and disposable /tmp fixture with MIT Vue3.5.44 installed via Bun (no repository dependencies/installed runtime modifications). Public Rust emitter probe emits four images with CR, CRLF, LF and tab alt values.
- Actual Svelte compiler→Bun browser bundle→Chromium baseline changes CR and CRLF to LF in DOM getAttribute. Vue compiler-SFC inline-template path preserves all four values in the same browser probe, so Vue remains unchanged. Separately tested proposed numeric &#13; encoding; Svelte still normalizes it, so did not ship that ineffective fix.
- Extended Svelte shared attr helper's existing NUL JSON-expression path to CR. JSON string escaping and literal less-than encoding remain unchanged; ordinary static attributes keep their previous source. Reviewed all shared attribute callers. Public template→IR→component regression checks image alt with mixed CR/CRLF/tab/LF uses the exact expression path without rawCR.
- Rebuilt actual library/probe and host components. Final Chromium DOM attributes exactly match all four inputs for fixed Svelte and unchanged Vue with no page errors; Svelte compiler reports zero warnings. All353 default native library units pass; strict native library Clippy, rustfmt check and scoped whitespace check pass. Independent actual-diff review approved shared escaping/caller scope; primary inspected final one-file before/current diff.
- Evidence: /tmp/crepus-emitter-attribute-cr/ (public probe source/binary, emitted components, compiler/bundler/browser scripts and versions, baseline and failed entity candidate, final green, native test/Clippy logs, snapshot and patch). Actual browser acceptance is DOM-only Chromium with those host versions; SSR, other browsers and full source-framework compatibility are not implied. Svelte skill used for host-compiler validation; actual compiler diagnostics address this byte-preservation change.
- No commits, pushes, deployment, repository dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Preserve CR attributes through Vue static subtree optimization

- Previous turn made progress fixing Svelte attributes; Vue's four-image host fixture passed using individual createElementVNode calls. Expanded that evidence to eight images with the same CR/CRLF/LF/tab values, using the same actual Vue3.5.44 compiler/runtime. This crosses the compiler's static subtree threshold and generates createStaticVNode with serialized HTML.
- Actual Chromium red: optimized Vue attributes change CR and CRLF to LF while fixed Svelte still preserves all eight values. Inspected compiler output containing createStaticVNode and raw escaped CR strings; this explains why the smaller prior Vue probe passed and why its result was not a general optimization-path guarantee.
- Vue shared attr helper now routes CR through its existing NUL nonconstant __crepusText(js_str) v-bind expression path, preventing HTML static stringification for affected values. Ordinary attributes remain static; existing JS and HTML-expression escaping retained. Added public template→IR→component image alt regression with CR/CRLF/tab/LF and checked no rawCR remains in generated source.
- Rebuilt actual native library/public emitter probe and host bundle. Final real Chromium passes exact alt values for all eight Vue and Svelte images without page errors. All354 default native library tests and strict native library Clippy pass; rustfmt and scoped whitespace checks pass. Independent actual-diff review approved shared callers/escaping and preservation of ordinary static output; primary inspected exact one-file before/current diff.
- Evidence: /tmp/crepus-vue-static-attribute-cr/ public probe, compiler/browser scripts and generated sources, actual red/green, build/unit/Clippy logs, original file snapshot and task.patch. Uses the prior disposable host dependency tree, with no repository dependency change. This is DOM Chromium/host-version acceptance, not SSR or other browsers. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Mobile scaffold JNI symbols encode package components

- Previous turn made progress fixing Vue static-subtree attribute preservation. Continued native wrapper review; existing mobile-new codegen failure uses ui::error and native build helpers already fail on generation errors, so no speculative wrapper edit. Parallel Imba emitter source review found no new confirmed supported-subset failure.
- Found JNI renaming defect: mobile new phone-app produces package component phone_app but previously replaced Rust export prefix with literal phone_app instead of JNI phone_1app. Public CLI scaffolding plus real javac -h header probe passes ordinary phone, then baseline fails phone-app because compiler-generated Java_dev_crepuscularity_phone_1app_CrepusRustActions_dispatchAction is absent from scaffold Rust source.
- Added one private package-component UTF16 encoder: ASCII alphanumerics unchanged, underscore escaped, other code units encoded individually with lowercase four-digit hex. Applied only to r_3 JNI export replacement; dev/crepuscularity package separators stay literal, and package/source directories, library names and UI labels are unchanged. Existing shared prefix replacement reaches all13 native methods. No new dependency.
- Unit regression covers ordinary/underscore/BMP/supplementary names; public CLI integration covers phone-app, phone_app and accented names with export and Kotlin-package checks. Actual CLI+javac -h final probe passes five names including U+10400 surrogate pair. All173 slim CLI binary units and32 native_ir_cli integrations pass; strict slim CLI Clippy including integration passes. Independent final source review approved encoding/callers/invariant; primary inspected exact two-file before/current diffs, rustfmt and scoped whitespace checks.
- Evidence: /tmp/crepus-mobile-jni-names/ snapshots, public compiler-header probe, baseline/green, build/unit/integration/Clippy logs and task.patch. This verifies compiler-produced JNI symbol agreement, not Android loadLibrary execution, APK build or device behavior. Arbitrary app-name/path validation across Cargo/Swift/Android identities remains a separate broader concern, not claimed resolved here.
- No commits, pushes, deployment, destructive cleanup, dependencies or remote mutations. Goal remains active.

### 2026-10-18 — Mobile destination paths do not become generated identities

- Previous turn made progress fixing JNI package encoding. Followed remaining destination/name boundary: native scaffold creation admits nested/absolute paths, but mobile wrapper derived snake/Pascal identity from the full path. Tests-first public mobile new nested/phone-app succeeds yet real Cargo read-manifest rejects generated package nested/phone_app_actions due slash.
- Derive both generated identity forms only from Path::file_name; retain full original destination for scaffold creation, root paths and destination messages. Missing final name errors before scaffold mutation. Existing single-component identity policy unchanged; this is a path/identity separation fix, not arbitrary basename sanitization.
- Public CLI integration passes relative nested and absolute destinations. Real Cargo metadata accepts phone_app_actions; generated Swift App filename, Kotlin package/source directory and escaped JNI export prefix are all checked. Cargo read-manifest validates manifest naming without claiming a generated dependency/native build.
- All173 slim CLI binary units and33 native_ir_cli integrations pass; strict slim CLI Clippy including integration passes. Primary inspected exact production/test before/current diffs, rustfmt and scoped whitespace checks. Independent proposal/caller review approved identity-only scope. Evidence: /tmp/crepus-mobile-destination-name/ snapshots, actual CLI/Cargo red/green, Clippy and task.patch.
- Arbitrary basename validation across platform identifiers and full generated Rust/Swift/Kotlin builds remain outside this repair; no simulator/device gate. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Fresh generated mobile Rust and macOS Swift package acceptance

- Previous turn made progress separating mobile destination paths from identities. Closed deeper acceptance gates by building the current public slim CLI and scaffolding nested/phone-app in /tmp/crepus-generated-mobile-rust/. No repository production changes or installed runtime substitution.
- Ran offline cargo test against the actual generated rust/Cargo.toml: package phone_app_actions compiles and all43 generated action/runtime tests pass,0 fail/ignored. This is generated source compilation, beyond prior read-manifest metadata acceptance. Generated dependency lock/build artifacts stay in the disposable project.
- Loaded the resulting actual libphone_app_actions.dylib via ctypes and invoked its owned C ABI with a large Unicode/NUL JSON payload. Decoded320164 output bytes exactly, confirmed syncCount1 (single dispatch), and released the returned allocation with the library's free export. No retry or mock ABI.
- Generated ios/Package.swift explicitly supports macOS14 as well as iOS17 and has no external package dependencies. Actual swift build for the host macOS target succeeds in25.58s without warning/error diagnostics. This compiles the generated NativeShell SwiftUI package; it does not link a signed application or prove iOS/device behavior. Swift skill used for platform/compiler validation; no new protocols/state refactor or unrelated changes.
- Parallel read-only components/catalog+CLI source review found no new confirmed mismatch in loading, title fallbacks, target selection, controlled unknown-ID errors or path-hint behavior. No duplicate test rerun for that bounded no-finding review.
- Evidence: /tmp/crepus-generated-mobile-rust/ (public scaffold/build logs, generated project,43-test output, actual ABI probe/log, Swift build/log, source-hashes.json). iOS SDK/Android APK compilation, linking Rust actions into an app, simulator/device loading, UI behavior and frame latency remain unverified. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Swift pending numeric text reaches numeric controls

- Audited native draft conversion against the shared binding contract. Kotlin and the generated Rust evaluator accept numeric text, while Swift draftNumber accepted only NSNumber, leaving a numeric control on its committed fallback while a text control sharing the binding held a pending numeric string.
- Added finite Double parsing for string drafts in the shared generated Swift state store; existing NSNumber and other typed accessors are unchanged. Async regression covers decimal, negative, exponent, invalid, NaN, Infinity and overflow strings with a discriminating fallback.
- Tests-first async harness fails on the original bridge; fixed generated and checked-in action-object variants pass. Actual generated NativeShell macOS package rebuild passes with warnings treated as errors. A probe linked against its actual Swift objects and the previously generated Rust dylib fails on the original module and passes with the fixed module, reporting numeric string draft 0.75. This is host module/ABI acceptance, not simulator UI or frame-latency proof.
- CLI checks stop before tests on an unrelated concurrent unmatched format brace at crates/crepuscularity-native/src/svelte_bindings.rs:554. Preserved that file and did not claim a passing current Rust gate. Independent read-only Swift review approved scope and regression; primary inspected exact two-file before/current diff and git diff --check passes.
- Evidence: /tmp/crepus-swift-numeric-draft/ snapshots, red/green async logs, actual module probe and build/link logs, CLI failure log and task.patch. No new dependencies, commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Pending native booleans match committed Rust truthiness

- Previous turn made progress fixing Swift numeric string drafts. Parallel read-only Android review found draftBool interpreted string "false" as false while Rust truthy_value treats nonempty strings as true; Swift also used stale fallback for string/collection drafts. Reviewed shared Rust semantics and all generated draftBool callers.
- Both shared Swift/Kotlin stores now apply the existing Rust truthiness rules to admitted JSON values: null false, booleans themselves, numbers nonzero, strings/arrays/objects nonempty. Fallback is evaluated only when no draft exists. Other getters, queue sequencing and committed evaluator unchanged; no new dependency.
- Added 13-case matrices to both existing async harnesses, with opposite fallback values and post-clear checks. Tests-first originals fail; fixed generated and snapshot action-object variants pass on both Swift and Kotlin. Kotlin uses actual extracted production state/queue plus explicitly labelled Android/Compose/JNI stubs; this is not an APK/device acceptance claim.
- Fresh actual NativeShell macOS package build passes with warnings treated as errors. Linked its real Swift objects against the previously generated Rust dylib: all13 pending draft results equal the actual Rust evaluator, including null, numeric, string and collection cases. Independent read-only review approves conversion and fallback behavior; primary inspected exact four-file before/current diffs and git diff --check passes.
- Earlier concurrent svelte_bindings.rs format failure is no longer present in current source. Revalidated current slim CLI: all173 binary unit tests and33 native_ir_cli integrations pass; strict binary Clippy passes. Evidence: /tmp/crepus-native-draft-parity/ snapshots, red/green harness logs, actual package/probe, CLI/Clippy logs and task.patch. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active; simulator/device UI and frame latency unverified.

### 2026-10-18 — Measured remaining native text-draft parity gaps

- Previous turn made progress fixing boolean draft parity. Revalidated current text readers and generated Rust display_value/stringify_value; parallel read-only review independently confirmed missing null/collection handling and Swift non-string fallback. No production edit this turn.
- Actual compiled current Swift NativeShell module linked to generated Rust dylib:17-case probe compares pending draftText to actual scoped Rust evalText and finds14 mismatches. Non-string drafts retain STALE fallback, including booleans/numbers/null/collections; all3 string cases agree. Verified compiled Swift bridge bytes still equal current repository template.
- Compiled/executed actual extracted Kotlin StateStore with installed kotlinx serialization1.7.3. Unused FFI collaborators explicitly trap; probe calls only production snapshot/draft readers, then compares with the Rust-produced expectation table. Finds6 mismatches: null, empty/nonempty arrays, empty/nonempty objects, and exponent spelling (1e20 versus Rust1e+20). This is host JVM evidence, not Kotlin/JNI or Android UI acceptance.
- Architecture review confirms existing text(expr,scopeName,scope) can format a present draft through a fixed ordinary lexical alias, with exact alias lookup before model fallback and no recursion/state mutation/new ABI. Retain string fastpath for normal keystrokes; explicit null must remain present. A full repair needs actual formatter/ABI acceptance rather than duplicating numeric formatting in the mock harness; no partial formatter shipped. Concrete next work is integrating that shared reader while preserving existing harness coverage.
- Evidence: /tmp/crepus-native-text-audit/ actual Swift/Rust probe and JSON rows, Kotlin production extraction/build/JVM logs, cases.json and current source hashes. Primary inspected production reader/caller and scope-resolution flow. Ledger-only diff passes whitespace checks. No commits, pushes, deployment, dependencies, destructive cleanup or remote mutations. Goal remains active; text-draft defect is confirmed and still open.

### 2026-10-18 — Native text drafts reuse committed Rust formatting

- Previous turn made progress measuring17-case text-draft parity and identifying the exact shared evaluator route. Implemented in both Swift/Kotlin draftText: absent draft alone uses lazy fallback; actual strings return locally; other JSON values use the existing text reader with a fixed ordinary scope alias and the present draft as scope. Explicit null survives; no new ABI/formatter/dependency. Removed now-unused Kotlin contentOrNull import.
- Extended existing async regressions to verify non-string draft forwarding, exact scope payload, local string fastpath and post-clear fallback. Swift extractor now includes real text reader/serialization/buffer helpers; its FFI stub returns a sentinel instead of copying Rust formatting. Kotlin exposes a scoped FFI callback in its labelled mock. Both original readers fail tests-first; final generated/snapshot variants pass on both hosts. Existing FIFO/rollback/reentrancy/validation coverage stays green.
- Actual generated NativeShell macOS package rebuild passes with warnings treated as errors. Linked real Swift objects to generated Rust dylib: previous17-case probe now reports zero mismatches, including null, booleans, collections, negative zero, exponent notation and9007199254740993. String edits require no added Rust call; non-string reads use the same existing synchronous evaluator as committed reads. No frame-latency improvement claim.
- All173 slim CLI binary units and33 native_ir_cli integrations pass; strict binary Clippy and git diff --check pass. Independent read-only review approved scope alias isolation, present null transport, read-only ABI reuse and harness boundaries; primary inspected exact eight-file before/current diff. Evidence: /tmp/crepus-native-text-fix/ snapshots, red/green logs, actual Swift/Rust package/probe, CLI/Clippy logs and task.patch.
- Kotlin host tests verify production reader forwarding with labelled Android/Compose/JNI stubs; actual Android JNI loading, simulator/device UI and frame latency remain unverified. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Pending native numeric values follow committed semantics

- Previous turn made progress fixing text-draft formatting. Parallel read-only numeric review confirmed Swift NSNumber accepts JSON true as1 while Rust excludes booleans, and both hosts retain stale numeric fallbacks for present invalid/null/collection drafts. Current contract specifies zero for nonnumeric/nonfinite reads; clarified that present drafts share this rule and only missing drafts use fallback.
- Swift draftNumber now keeps real finite NSNumber values local, excludes CFBoolean explicitly, returns zero for present nonnumeric values, and delegates string parsing through the existing scoped Rust number reader. Kotlin keeps numeric primitives local with finite Float guard and zero overflow behavior, delegates strings through the existing number accessor, and uses fallback only when absent. This supersedes the earlier invalid-string-fallback regression expectation with documented evaluator parity. No new ABI/dependency or extra native call for ordinary slider edits.
- Extended existing async harnesses to extract production Swift number/reader helpers and verify scope/alias/string forwarding without copying Rust parsing. Added bool/null/collection zero cases, absent fallback checks, valid/invalid string forwarding and Compose finite-double-to-Float overflow zero. Both original hosts fail tests-first; both final generated/snapshot variants pass, with Android/Compose/JNI collaborators explicitly mocked.
- Actual old compiled Swift/Rust numeric probe fails. Fresh generated NativeShell macOS package rebuild with warnings treated as errors passes; actual Rust-linked24-case final probe reports zero mismatches, including bool, null, collections, finite large values, negative zero, valid/invalid/nonfinite strings, hex float text, leading plus and whitespace. This checks real formatter/parser ABI, not a mock golden table.
- All173 slim CLI binary units and33 native_ir_cli integrations pass; strict binary Clippy and git diff --check pass. Independent read-only review approved conversions/alias/fastpath/contract; primary inspected exact nine-file before/current diff. Evidence: /tmp/crepus-native-number-fix/ snapshots, actual and harness red/green, generated Swift build/link/probe, CLI/Clippy logs and task.patch.
- Actual Android JNI loading, simulator/device UI and frame latency remain unverified. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Actual Android arm64 action library and Kotlin declaration acceptance

- Previous turn made progress fixing numeric draft semantics. Revalidated Android toolchain/current JNI source before deeper acceptance. Parallel read-only review found no new concrete evalText/evalNumber signature, nullable scope or string transport mismatch; no speculative production edit.
- Verified actual earlier public mobile scaffold Rust source equals current template after only intended JNI-package/state-directory identity replacements. Used coherent installed rustup stable Cargo/Rustc with installed aarch64-linux-android standard library and NDK28.2.13676358 API26 clang. Offline build stopped because cesu8 was not cached; locked build fetched existing locked JNI dependencies and succeeded in9.03s, without manifest/lock/source changes.
- Actual libphone_app_actions.so is ELF64 AArch64 DYN and links Android libc/libdl. NDK llvm-nm confirms its13 JNI export names exactly match all13 current Kotlin external declarations with escaped phone_1app package prefix; no missing/extra JNI export. Compiled those actual declarations with kotlinc and installed Android35 android.jar, then inspected javap native descriptors, including three nullable-reference parameters and Double return on evalNumber.
- Evidence: /tmp/crepus-android-rust-acceptance/ offline/locked build logs, source hashes, exported symbol set, ELF headers/dependencies, extracted production declarations, actual Kotlin compilation and javap descriptors. Generated Android shared library remains under /tmp/crepus-generated-mobile-rust/nested/phone-app/rust/target/aarch64-linux-android/debug/. This closes Android Rust compilation/export and declaration compilation gates; it does not load JNI on Android or compile the full Compose application.
- adb devices reports no connected device; no emulator launch, APK/UI/device execution or frame-latency claim. Ledger-only repository change; inspected recorded results and git diff --check. No commits, pushes, deployment, destructive cleanup, new project dependencies or remote mutations. Goal remains active.

### 2026-10-18 — Terminal/embedded review, origin workaround and completed scratch cleanup

- Previous turn made progress compiling the real Android action library. Continued terminal origin review and delegated independent embedded/LVGL file/output review. Embedded review found no new validated defect in include cache identity, file base/frontend selection, caller-slot scope, deferred parse contract or output error propagation; no speculative implementation change.
- TUI registered-file origin remains unresolved: public Component/ComponentRegistry struct fields discard source paths. New review ruled out an existing safe source-rebase workaround: AST include nodes lack source spans, no template serializer exists, absolute include paths are rejected, and macro expansion cannot preserve props/slots/runtime loading. Preserved public API instead of introducing hidden metadata or a partial rewrite.
- Added four-line public register_file rustdoc explaining current usable workaround: relative includes use the render context base_dir, so supply the component file parent when rendering. Primary inspected exact doc-only diff and whitespace checks; runtime behavior unchanged. Existing known-defect ignored regression remains explicitly ignored, not claimed fixed.
- Current TUI147 library units pass, strict library Clippy passes,3 real OS hot-reload integrations pass and1 in-memory registry include integration passes. The separate file-origin regression is1 ignored. Checks cover current runtime code; subsequent production-file edit is rustdoc only, with no new example/link needing execution. Logs: /tmp/crepus-tui-current-audit.log, /tmp/crepus-tui-current-clippy.log, /tmp/crepus-tui-current-integration.log; doc snapshot/diff: /tmp/crepus-tui-origin-doc/.
- Free disk had fallen to249MiB. Under existing cleanup authorization, removed only four completed scratch swift-build directories created by this audit (number/text/parity/numeric-draft probes), reclaiming about0.5GiB; all source snapshots, generated source projects, probe executables, logs, Android library and original generated Rust host library remain. Completed handles were terminal before cleanup; exact paths recorded in /tmp/crepus-completed-swift-build-cleanup.json. No project source, installed runtime or unrelated build directory removed.
- No commits, pushes, deployment or remote mutations. Goal remains active; file-origin automatic repair, device UI and frame-latency proof remain open.

### 2026-10-18 — Generated extension pages display unprintable runtime failures

- Previous turn made progress documenting terminal origin behavior and validating current TUI/embedded flow. Browser-extension resource/watch review found no new confirmed supported copy/invalidation defect. Runtime Cargo.lock exclusion is explicitly tested to avoid own-build feedback; did not promote a source-only candidate into a speculative watcher change.
- Found generated page catch block could itself reject while reading error.stack or coercing an unprintable thrown value, leaving Loading UI and an unhandled module rejection. Reused the existing background-worker fallback message, preserving ordinary stack text, retrying plain string conversion after broken stack access, and keeping the original rethrow when no root exists. Four extra generated JS lines; no dependency/helper abstraction/new API.
- Added Bun regression executing the actual Rust format-body JavaScript with browser/native-module collaborators mocked, not a rewritten catch algorithm. Baseline has3 real failures (null-prototype object, throwing proxy getters, broken stack getter); final six cases cover those plus ordinary Error stack, string, and no-root original-value identity. All29 webext JavaScript tests across5 files pass. All173 slim CLI binary units and strict binary Clippy pass; public slim CLI build succeeds.
- Actual public CLI builds disposable extension pages. Replaced only its disposable runtime module/WASM with labelled fault-injection fixtures, then loaded real generated pages in Chromium over local HTTP. All3 difficult runtime throws display expected text with zero page errors; exercised each page load, inspected DOM text and opened the rendered screenshot. This is generated page/browser acceptance with mocked native module, not installed-extension or actual WASM execution.
- Independent read-only review approved semantic preservation and production-body extraction; primary inspected exact production and new-test diff, git diff --check passes. Evidence: /tmp/crepus-webext-page-errors/ original source, red/green/Bun/CLI/Clippy logs, public-build output, browser probe/log/screenshot and task.patch. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — AssemblyScript adapter accepts current byte-buffer initialization

- Previous turn made progress fixing generated extension error displays. Audited runtime loader inputs; parallel read-only review confirmed shipped AssemblyScript adapter init fetched its argument as a URL, while current background/content/generated-page loaders pass {module_or_path:wasmBytes}. Following documented adapter installation therefore attempted to fetch an object instead of instantiating supplied bytes.
- Normalize existing loader envelope or direct input, instantiate ArrayBuffer/views directly with the existing imports, and retain URL fetch/instantiateStreaming path. Existing export publication follows successful instantiation; invalid-byte errors propagate. No caller rewrite, new ABI/dependency or unrelated AssemblyScript string helper change.
- Tests-first direct actual module tests fail on original buffer inputs. Added5 Bun tests using a real valid minimal WASM module, including envelope, raw buffer, typed view, legacy URL and invalid-byte CompileError. Final all34 extension JavaScript tests across6 files pass. All55 crepuscularity-webext Rust tests pass, including embedded asset compilation;0 doctests.
- Actual Chromium imports byte-identical current adapter over local HTTP and uses native WebAssembly for all4 valid input modes plus invalid-byte rejection. All succeed/reject as expected with zero page errors. Minimal WASM has no exports, intentionally avoiding claims about AssemblyScript string allocation/runtime export compatibility or full app behavior.
- Independent final read-only review approved current/legacy input paths and error propagation; primary inspected exact production/new-test diff and git diff --check passes. Evidence: /tmp/crepus-as-adapter-input/ before source, real engine red/green, Rust tests, Chromium script/log and task.patch. No commits, pushes, deployment, destructive cleanup, new project dependencies or remote mutations. Goal remains active.

### 2026-10-18 — AssemblyScript strings use the real managed-memory runtime ABI

- Previous turn made progress fixing adapter initialization. Official AssemblyScript runtime contract (https://www.assemblyscript.org/runtime.html) and real compiler acceptance confirm --exportRuntime exports allocation/GC helpers, not the host __getString/__newString conveniences previously assumed by the adapter. Original adapter fails real ASC0.27.31 string export immediately despite --exportRuntime.
- Implemented UTF16 transfer through exported memory: unsigned wasm32 pointers, length header at pointer-4, bounded1024-unit decoding chunks preserving NUL/BOM/astral/lone-surrogate units, and __new(byteLength, stringClassId2) with fresh memory views after allocation. Corrected misleading comments. Two-argument popup action pins the first argument across second-argument allocation and unpins in finally, including serialization/parse failure; other single-argument calls allocate immediately before entering WASM.
- Added an authored AS source fixture and optional compiler-backed Bun gate enabled by CREPUS_ASC_PATH. No committed generated binary or project dependency. Installed only a disposable /tmp compiler toolchain with scripts disabled: assemblyscript0.27.31, binaryen116.0.0-nightly.20240114, long5.3.2; all package manifests declare Apache-2.0. Compiler gate rebuilds fixture in its own temporary directory and cleans its output.
- Actual compiler baseline fails missing raw-export helper. Full final JS suite with compiler gate enabled passes35 tests,142 assertions,0 skips/failures. Fixture covers version/browser-program raw strings, manifest JSON, extraction/render JSON, nested values, one-million-code-unit strings/memory growth,30 twoargument calls, callee collection and cyclic-input failure/recovery.
- Strengthened gate wraps actual compiled __new exports to force real __collect before every host allocation, restoring the global instantiate hook in finally immediately after initialization. Disposable no-pin production variant fails deterministically: actionLength expected13 becomes36, proving first-argument lifetime regression is exercised. Production passes; cyclic-input coverage checks cleanup/recovery but does not independently measure root reclamation.
- Independent unwrapped native Chromium run imports byte-identical current adapter with real compiler WASM: same Unicode/JSON/growth cases and30 action calls pass with zero page errors. All55 webext Rust tests and strict library Clippy pass;0 doctests. Primary inspected exact three-file diff and git diff --check. Two independent read-only reviews approved memory/GC behavior and final test-hook boundaries.
- Evidence: /tmp/crepus-as-string-abi/ before source, compiler package manifests/logs, actual compiled fixture, original red, no-pin red, final green, Chromium probe/log, Rust/Clippy logs and task.patch. Compiler gate is explicitly skipped when CREPUS_ASC_PATH is absent; this run supplied it. Installed-extension execution and other compiler versions/runtime variants remain unverified. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Extension settings failures always format a reply safely

- Previous turn made progress fixing AssemblyScript string ABI. Read-only parallel review and current source inspection confirmed both settings:get/settings:set catches dereferenced error.message directly. Null/undefined/throwing getters reject again before sendResponse; other unusual values return undefined/non-string payloads. Runtime-message formatting is a separate existing contract and remains unchanged.
- Added one private storageFailure formatter shared by both catches: guarded ordinary message conversion, guarded plain error conversion and stable Storage operation failed fallback. Ordinary get denied/set denied messages remain identical. No storage ordering, default write, runtime dispatch, dependency or outbound API change.
- Extended existing production-background harness rejection injection to distinguish omitted arguments from explicit undefined. Seven rejection types across get, set and post-set readback produce21 discriminating baseline failures; final all56 extension JS tests (compiler gate enabled),205 assertions pass. Regressions verify stored value is unchanged on get/set failure and remains written when only subsequent readback fails, without inventing rollback semantics.
- All55 webext Rust tests and strict library Clippy pass. Actual Chromium native module Worker loads byte-identical production background/browser-shim/AS adapter plus previously real-compiled AS fixture. With explicitly injected storage API rejections, all9 null/unprintable/getter cases across three paths reply correctly, preserve expected write state and produce zero page/console errors. This is module-worker/API-fault acceptance, not installed extension/native storage integration.
- Independent read-only review approved formatter, explicit undefined helper behavior and readback-write assertions. Primary inspected exact two-file before/current diff; git diff --check passes. Evidence: /tmp/crepus-webext-storage-errors/ snapshots,21-failure red,56-test green, Rust/Clippy logs, actual worker browser probe/log and task.patch. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Chrome callback exceptions reject instead of leaving pending promises

- Previous turn made progress fixing settings rejection replies. Read current shim and all callback callers; parallel read-only review confirmed exceptions reading Chrome lastError/message or constructing Error escape asynchronous native callbacks. Promise executor catches synchronous calls only, so the async case previously leaves the bridge pending.
- Added try/catch inside the existing native callback, rejecting with the original exception. lastError still reads during callback lifetime; native receiver/arguments, ordinary Error messages, namespace selection, success values and single invocation remain unchanged. No timeout/retry/dependency/API addition.
- Added15 asynchronous regressions (runtime.sendMessage and local/sync get/set across lastError getter, message getter and message conversion). Mock native callback observes escaped exceptions separately; bounded bridge must reject exact original value, with zero escapes and one call. Initial live-source red run overlapped the edit and was discarded; isolated immutable original-source replay establishes3 original passes/15 new failures. Final full JS suite with compiler gate enabled passes71 tests,250 assertions,0 failures/skips.
- All55 webext Rust tests and strict library Clippy pass. Actual Chromium native module Worker imports byte-identical production shim and injected Chrome-shaped callback API. All15 async boundary cases reject with original identity, exactly one native call, zero escapes and zero page errors. This is worker/API-fault acceptance, not installed extension/context-teardown reproduction.
- Independent read-only review approved callback-local catch, error lifetime and test discrimination. Primary inspected exact two-file before/current diff; git diff --check passes. Evidence: /tmp/crepus-webext-callback-errors/ snapshots, isolated snapshot-red, final green, Rust/Clippy logs, actual browser worker/log and task.patch. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Embedded/LVGL current gates and explicit reload contract

- Previous turn made progress fixing Chrome callback Promise settlement. Audited embedded Template cache, reload, retained-document and draw sequence plus framebuffer/display boundaries. Existing render constructs/validates/layouts the document before paint; deferred parsing after reload is intentional. Read failures preserve source/document, successful reload invalidates both parsed cache/document, and parse/layout failure reaches no framebuffer paint. No new validated runtime bug found; no speculative reload policy change.
- Added four-line public reload rustdoc explaining deferred draw parsing and read-success/failure invalidation semantics. Runtime behavior/public shapes unchanged. Primary inspected exact one-file before/current diff and git diff --check; no implementation-mirroring test added for this documentation-only change.
- Independent read-only LVGL supported-node/XML review found no new validated text placement, attribute provenance/control normalization or escaping/name-validation defect. Initial53 embedded library tests pass. Broader current embedded+LVGL unit/integration run completes63 binaries with223 passed,0 failed,2 ignored; the53 initial units are included in223 and not counted twice. Ignored cases are explicit manual repeated-slot scaling comparisons with no timing assertion.
- Host cargo check for embedded --no-default-features passes, verifying no_std feature surface without claiming MCU execution. Strict library Clippy for both crates passes. Tests cover current runtime code; source change is only reload rustdoc. No full-workspace, hardware panel, firmware, SPI calibration or frame-latency claim.
- Evidence: /tmp/crepus-embedded-current-audit/ initial unit log, full targets.log, no-std.log, clippy.log, template.before.rs and task.patch. No commits, pushes, deployment, destructive cleanup, dependencies or remote mutations. Goal remains active.

### 2026-10-18 — Shared colors and embedded libraries generate bare-metal ARM code

- Previous turn made progress validating embedded/LVGL host gates. Primary and independent read-only color review found no new malformed-input or conversion defect: shared palette resolves static ASCII values, embedded parse_hex validates ASCII hex and exact lengths before slicing, and RGB565 conversion already has exhaustive roundtrip coverage. No speculative parser edit or redundant test added.
- All3 shared colors tests pass, including complete palette/numeric agreement;0 doctests. Closed stronger platform gate beyond prior host no_std check: coherent installed rustup stable Cargo/Rustc builds actual colors and embedded libraries for installed thumbv7em-none-eabihf, offline/locked, --no-default-features. Build succeeds in2.78s; no project manifest/lock/dependency change. The on-demand cross-platform host reference is absent at its instructed path/search locations; this check used local installed toolchains only, with no remote-host action.
- Inspected actual .rlib objects, verifying ELF32 little-endian ARM (e_machine40) and ARM v7E-M attributes for both libraries. macOS system ar did not resolve Rust archive long-name members; switched to installed NDK llvm-ar/llvm-readelf rather than treating that inspection-tool limitation as a product defect. Stored archive hashes, object identity and attributes in objects.json.
- Evidence: /tmp/crepus-color-baremetal-audit/ colors-tests.log, baremetal.log, isolated target output, extracted object/attribute logs and objects.json. This verifies target library code generation, not a linked firmware/application, physical display, SPI calibration, boot or frame latency. Repository change is ledger only; results inspected and git diff --check passes.
- No commits, pushes, deployment, destructive cleanup, dependencies or remote mutations. Goal remains active.

### 2026-10-18 — Planned output-file ancestors fail before any build writes

- Previous turn made progress verifying real bare-metal library output. Current manifest/path review found no new confirmed CLI cwd defect; a facade web template-field candidate remains unmodified pending an explicit supported-field contract. Primary found shared output preflight admitted a future file path together with its descendant (new/result plus new/result/view.json), so an otherwise invalid build could write earlier unrelated outputs before failing to create the descendant parent.
- Reviewed every production caller: facade Artifact paths and CLI native/LVGL outputs are files; CLI web reserves out/index.html, not its directory root. Shared validator now compares resolved path identities component-wise with starts_with in both directions, rejecting equality or ancestor relationships before writes. Existing filesystem alias resolution, symlink-loop handling, shared-parent siblings and similarly prefixed names remain supported; updated API doc/error description. No new dependency/public signature.
- Tests-first core cases fail for both orders and directory-symlink aliases. Actual facade red demonstrates unrelated existing sentinel overwritten (replacement versus keep existing), not just an error-text mismatch. Final regression verifies sentinel preservation and no created parent directory in both orders. Added real public CLI manifest integration covering both orders and an earlier target that must remain unwritten.
- Final521 core library tests,7 facade library tests,173 slim CLI binary units,6 framework-manifest integrations and5 output-collision integrations pass (712 distinct scoped tests,0 failures/ignored). Five targeted core validator cases also pass but are included in521, not double-counted. Strict core/facade library Clippy and slim CLI binary/affected-integration Clippy pass; CLI dependency recompilation completed on its original live handle without restart.
- Independent read-only review approved file-only caller contract and actual prefix/order/symlink/sibling behavior. Primary inspected exact three-file before/current diff; rustfmt and git diff --check pass. Existing documented hard-link/concurrent-filesystem limits remain; this is deterministic planned-output preflight, not transactional protection against other I/O failures.
- Evidence: /tmp/crepus-output-ancestor-fix/ snapshots, core-red, facade-preservation-red, core-green, full library/CLI logs, strict Clippy logs and task.patch. No commits, pushes, deployment, destructive cleanup or remote mutations. Goal remains active.

### 2026-10-18 — Generated manifest target selection

- Parallel read-only review identified that the facade returned generated `target-{index}` artifact IDs but selected only explicit IDs. The CLI already selected resolved effective IDs.
- `crepuscularity/src/target.rs` now uses its existing effective-ID helper for both ambiguity validation and selection, preserving original manifest indices. Explicit/generated collisions fail before template reads; unselected builds retain existing behavior.
- Regression first failed with `no target with id "target-1"`; after the fix, all 8 facade library tests pass. Strict library Clippy, rustfmt, actual before/current diff inspection and repository diff whitespace checks pass. Independent reviewer approved the logic.
- Evidence: `/tmp/crepus-generated-target-selection/`. No commits, pushes, branches or deployment. Full workspace and device checks were not run for this selection-only change. Bare-build malformed-manifest fallback remains an investigation candidate, not a verified/fixed issue.

### 2026-10-18 — Bare-build manifest discovery errors

- Confirmed `has_manifest_targets` swallowed discovered manifest read/parse errors, allowing bare `crepus build` to run Cargo and report success. `docs/cli.md` promises fallback for absent targets, not malformed manifests; parallel read-only review confirmed the distinction.
- Made discovery fallible and propagated its error through the sole dispatch caller. Absent manifests and valid targetless manifests still use Cargo; explicit selectors retain their existing execution path. Updated CLI documentation.
- Added Unix public-CLI regressions using a process-local fake Cargo: malformed TOML and invalid target types fail without invoking Cargo; absent and valid iOS-only manifests preserve fallback. Baseline failed because malformed TOML reported build success. No real project build or environment-global mutation occurs in the fixtures.
- Slim CLI checks: 173 unit + 2 discovery + 6 framework manifest + 5 output-collision tests pass (186 total, none failed/ignored). Strict Clippy for the binary and affected integrations, rustfmt and diff whitespace checks pass. Inspected snapshot-to-current production/docs diffs and new test source; independent reviewer found no blockers.
- Evidence: `/tmp/crepus-manifest-discovery/`. Desktop-feature/full-workspace and device checks not run; regression tests are Unix-only. No commits, pushes, branches or deployment.

### 2026-10-18 — Stack-safe warm memo validation

- Target dispatch review found no further confirmed settings defect. Continued the recorded reactive-chain audit with an independent read-only reviewer; current source still recursively validated known memo dependencies.
- Added an isolated public-API regression that incrementally warms 16,384 memos on a 2 MiB native thread stack. The baseline aborted with stack overflow; 4,096 warm nodes passed and were not used as evidence of failure. Both unchanged and changed values are now read inside batches, exercising Check validation and subsequent recomputation. The child process contains aborts without killing the test runner.
- Replaced recursive known-source Check traversal with explicit DFS Visit/Finish frames. Preserved source order, active memo guards, revision checks, disposed-node behavior and post-callback retries. Kept clean and Dirty-only entry paths free of the new traversal allocation. No dependencies or public APIs added. Independent reviewer found no semantic blocker.
- All 88 native reactive tests/doc-tests pass; one existing manual timing benchmark remains ignored. Final native deep regression passes after test extraction. The same 16,384-node regression passes in actual WASM execution through wasm-bindgen-test-runner. Strict all-targets native Clippy, WASM DOM-feature check, rustfmt, repository diff whitespace check and actual scoped source/test/docs inspection pass.
- Evidence: `/tmp/crepus-reactive-warm-chain/`. Documented the remaining synchronous cold/user-closure recursion limit in source and runtime docs. This is not worker-thread rendering, browser visual/latency proof, a cold-chain fix or a guarantee against all stack overflows. No commits, pushes or deployment.

### 2026-10-18 — Retain subscriptions after callback panics

- Parallel review found a concrete gap in existing panic-recovery coverage: memo/effect callbacks cleared previous dependencies before running; a panic before the first tracked read permanently unlinked the observer from later source writes. Two public-API baseline regressions failed with observed values stuck at zero after a subsequent source update.
- Shared `run_observer` now retains the existing cleared-source vector in the observer guard. Unwinding restores the previous observer and unions still-live prior edges with partial new reads through the existing tracking logic. Successful callbacks discard the backup; comparisons/destructors remain outside this recovery scope. Disposed graph nodes are not resurrected, and failed effect constructors retain their existing orphan cleanup.
- Four new native regressions cover effect and memo retry before reads, partial-read retention plus old-edge pruning after success, and disposal during a failed callback. All 92 native reactive tests/doc-tests pass, one existing manual timing benchmark ignored. Strict all-targets Clippy, final WASM DOM-feature check, actual WASM deep-chain normal-path execution, rustfmt and repository diff whitespace checks pass. Independent reviewer approved actual scoped source/test diff; primary inspected source and documentation changes.
- Recovery is conservative: after a nested successful run and outer failure it can retain extra prior edges until the next success. It requires panic unwinding; WASM trap/abort recovery is not claimed. No UI latency or full-workspace/device proof. Evidence: `/tmp/crepus-reactive-panic-subscriptions/`. No commits, pushes, branches or deployment.

### 2026-10-18 — Reentrant validation unwind acceptance

- Independent current-source review of disposal and nested memo commits found no further confirmed bug. Live-node checks and revision-gated cache installation remain intact; conservative subscription recovery is already documented.
- Strengthened the existing comparison-panic regression to traverse 33 warmed dependent memos. Its comparison writes a newer source value and panics while iterative Check frames are pending. After catching the panic, the top memo must immediately return the newer value before batch flushing, remain correct after the flush, and respond to another source write. All chain nodes are explicitly disposed.
- All 7 memo-reentrancy integration tests pass; strict Clippy for the affected integration, rustfmt, repository whitespace check and before/current test diff inspection pass. Evidence: `/tmp/crepus-reactive-validation-unwind/`.
- Verification-only change; no production behavior change or new defect claimed. Panic unwinding was exercised natively, not through WASM traps; no UI latency or full-workspace checks. No commits, pushes, branches or deployment.

### 2026-10-18 — Reactive SVG XLink attributes

- Parallel DOM-binding review identified absent `xlink:href` attributes being created without their required XLink namespace. Existing SSR attributes could mask the issue. Confirmed in actual Chromium/WASM: the new regression failed namespace lookup (`None` instead of `#first`), while the other 11 DOM tests passed.
- `bind_attr` now uses namespace-aware set/remove calls for exact `xlink:href`; unsafe values still remove the legacy qualified attribute as well. Other attribute paths and the existing URL policy are unchanged. No dependencies or manifest changes. DOM namespace contract reference: https://dom.spec.whatwg.org/#dom-element-setattributens ; SVG XLink reference: https://www.w3.org/TR/SVG11/linking.html .
- New browser regression verifies initial creation, updates, unsafe removal, safe recovery and disposal through namespace lookups and actual rendered SVG `use` bounding-box widths 32 → 64 → 0 → 48 → 48. All 12 DOM tests pass in actual Chromium using the current WASM artifact. Independent reviewer approved scoped source/tests; primary inspected snapshot diffs and updated runtime docs.
- Strict WASM DOM library/integration Clippy passes with a coherent installed rustup toolchain in an isolated target directory. Initial attempts mixed Homebrew cargo-clippy with rustup rustc, yielding incompatible metadata/missing target errors; no shared build directories were cleaned or installed runtimes changed. Rustfmt and repository diff whitespace checks pass.
- Evidence: `/tmp/crepus-reactive-xlink/`. Both temporary test servers were stopped after browser runs. Firefox/Safari, broader XML namespace support, UI latency and full-workspace checks were not tested. No commits, pushes, branches or deployment.

### 2026-10-18 — Avoid redundant reactive text DOM work

- Hydration producer/consumer review found no mismatch: canonical IDs, encoding, envelope and legacy compatibility match current docs/tests. Moved to an observed DOM hot path using the measured-performance skill and existing Bolt journal.
- Actual Chromium/WASM baseline: 1000 effect reruns returning unchanged text generated 1000 child-list replacement records. The new regression failed its zero-mutation assertion before production edits; the other 12 DOM tests passed.
- `bind_text` now reuses only a sole Text child for nonempty strings, skips unchanged node values, and preserves plain-text flattening for markup/comments/multiple children. Empty values remove all children and skip already-empty DOM. No new dependencies, public API or off-thread scheduling.
- Final browser probe: unchanged reruns produce zero child-list and zero character-data records; a changed string produces one character-data record and retains the original Text node. Structural cleanup, escaped/Unicode output, empty cleanup and disposal checks pass. All 13 actual Chromium DOM tests pass; strict WASM DOM library/integration Clippy, rustfmt and repository whitespace checks pass. Independent reviewer approved actual source/test diff; primary inspected source/tests/docs and journal additions.
- Updated runtime docs and `.jules/bolt.md` with the bounded measurement and observable mutation/node-identity change. Evidence: `/tmp/crepus-reactive-text-work/`. Both temporary test servers were stopped. Mutation counts establish less DOM replacement work, not elapsed-time, frame-latency, Firefox/Safari or full-workspace proof. No commits, pushes, branches or deployment.

### 2026-10-18 — Empty select value after no selection

- Parallel form-control review and primary inspection identified a concrete single-select ambiguity: the native value getter returns empty both for no selected option and an empty-valued option. Equality-only binding skipped the value setter and left an available empty choice unselected. Duplicate equal-valued options and multi-select APIs were not reclassified as bugs or expanded.
- Actual Chromium/WASM baseline failed selectedIndex -1 versus expected 0 for an initially unselected select bound to an empty string; the other 13 DOM tests passed. The native value getter/setter distinction is specified at https://html.spec.whatwg.org/multipage/form-elements.html#dom-select-value .
- Select bindings now also apply the bound value when selectedIndex is -1. Ordinary unchanged selections and other controls retain their existing behavior. Added browser coverage for initial empty choice selection, unmatched→empty recovery, ordinary nonempty selection and disposal preserving externally cleared selection. Updated runtime docs.
- All 14 actual Chromium DOM tests pass. Strict WASM DOM library/integration Clippy, rustfmt, repository whitespace checks and primary snapshot-to-current source/test/docs diff inspection pass. Independent reviewer approved the scoped change. Both temporary browser test servers were stopped.
- Evidence: `/tmp/crepus-reactive-empty-select/`. Firefox/Safari, UI latency and full-workspace checks not run. No commits, pushes, branches or deployment.

### 2026-10-18 — WebKit DOM acceptance

- Broadened the current DOM acceptance suite to the installed Playwright WebKit 26.5 engine, rather than treating Chromium results as general browser proof. All 14 actual WASM DOM tests pass with no page errors. This includes XLink namespace/rendered geometry, zero redundant text mutations, changed Text-node reuse, empty-option select recovery, file-input clearing, hydration and disposal.
- Independent read-only reviewer found no known modern-WebKit API blocker and identified file-seeding/geometry-settling as potential harness limits; neither limit prevented the actual run. No test expectations were weakened and no production/test source changed.
- Initial client navigation occurred before the test server finished starting; once its live ready message was confirmed, the client reran against the same server and passed. The server was stopped after verification. Evidence and source hashes: `/tmp/crepus-reactive-webkit/`.
- This is installed test-engine proof, not the native Safari app, older WebKit, iOS devices or latency proof. Firefox's configured Playwright executable is absent, so it was not tested or installed. No commits, pushes, branches or deployment.

### 2026-10-18 — Current LSP Unicode/protocol acceptance

- Reviewed current full-document synchronization, UTF-16 cursor conversion, version/lifecycle handling and diagnostic ordering. Independent reviewer found no new concrete version/open-close defect; FULL synchronization intentionally does not apply incremental edit ranges.
- All 38 LSP tests (36 library + 2 server) pass. Built the actual `crepus-lsp` executable and extended the existing stdio protocol script to exercise completion and hover after an emoji with UTF-16 coordinates, plus versioned CRLF error diagnostics.
- Reviewer identified that the first added cursor inside an alias could pass despite an off-by-one offset. Strengthened to completion immediately before the closing quote and hover at the alias start: scalar indexing shifts completion past the quote, while byte indexing leaves hover inside the emoji. Both final real-stdio modes pass, including exit while stdin remains open, with shutdown/status 0 and without shutdown/status 1.
- Strict all-targets LSP Clippy, repository whitespace check and actual before/current script diff inspection pass. Independent reviewer approved final coordinate assertions. No runtime source changes or new defect claimed. Evidence: `/tmp/crepus-lsp-current-audit/`.
- This checks this server's current full-sync protocol behavior, not every editor integration, incremental-sync support or UI latency. No commits, pushes, branches or deployment.

### 2026-10-18 — Reserved foreign function names in D/Odin bindgen

- Parallel generator review found that legal C names reserved by a target lexer were emitted verbatim into invalid bindings. Primary baseline tests confirmed both D `module` and Odin `defer` incorrectly returned successful generation and wrote files. Independent disposable installed-compiler probes rejected the declarations, including LDC2 explicitly identifying `module` as a keyword.
- Added target-specific reserved-name checks before binding output, following the existing bounded/fail-closed selected-header policy. D keywords/special tokens were checked against https://dlang.org/spec/lex.html#keywords ; Odin keyword and legacy `notin` table against https://github.com/odin-lang/Odin/blob/master/src/tokenizer.cpp . Current prefixed ABI function names keep their generated spelling. Automatic reserved-name aliases remain unsupported and are documented.
- Delegated test ownership only for D/Odin integration files; primary owned production checks, integration and final verification. Two regressions cover seven selected-header names with a fresh fixture per case, require target/name-specific errors and verify no binding file was written. Primary inspected actual before/current source/test/docs diffs and applicable callers; reviewer approved guard structure and fail-closed policy. Shared header validation already rejects renamed/raw Rust identifiers before these backends.
- Full bindgen suite: 30 passing tests and four optional gates ignored. Explicitly ran the installed V/spaced-header/library-directory compiler gate: one additional pass, leaving three optional real-Rust-ABI gates unrun. Thus 31 distinct passing tests, including real generated-code compiler checks and independent C-stub callback round trips. Strict all-targets Clippy, rustfmt and repository diff whitespace checks pass.
- Evidence: `/tmp/crepus-bindgen-reserved/` and test snapshots `/tmp/crepus-bindgen-reserved-functions/`. No fresh Rust ABI dynamic-library acceptance run or UI latency claim. No commits, pushes, branches or deployment.

### 2026-10-18 — Fresh Rust ABI acceptance for D/Odin/Nim

- Built the current Rust `crepuscularity-abi` library and verified its native artifact is a Mach-O arm64 dylib with the expected @rpath install name. Recorded the SHA-256 and verified it remained unchanged throughout acceptance.
- Explicitly ran all three previously optional real-Rust-ABI bindgen gates with CREPUS_ABI_LIB pointing to that exact freshly built library. Process-local DYLD_LIBRARY_PATH and fallback overrides were removed for this run; no installed runtimes or global environment were changed. All three generated D, Nim and Odin runtime executables pass.
- Independent read-only review confirms each fixture invokes all 13 exports and exercises rendering/context patches, callback payload/userdata, callback unregistration, freeing owned strings, signed failures, consumed session/global errors, file bundles, named components and session cleanup. D/Nim copy strings before freeing; Odin inspects before freeing. No source or test changes were needed.
- Evidence: `/tmp/crepus-bindgen-real-abi-current/` includes build/test logs, library hash and selected header/generator hashes. This closes the three skipped native runtime gates recorded in the prior bindgen audit; the installed V gate had already passed separately.
- Native macOS lifecycle smoke evidence only: not exhaustive ABI, threading/stress, other operating systems or UI latency proof. Repository whitespace check passes. No commits, pushes, branches or deployment.

### 2026-10-18 — Python wrapper current-runtime acceptance

- Reviewed Python ABI callback ownership, close/error handling and CLI-backed rendering. Independent read-only review found no new concrete lifecycle defect: callbacks remain rooted, exceptions propagate after native dispatch with result freeing, callback reentry is blocked and close releases ownership idempotently.
- Initial Python discovery ran 20 tests: native ABI cases passed, but two CLI-backed checks failed because PATH selected the installed `/Users/undivisible/.cargo/bin/crepus` 0.11.0. It returned older IR version 5 and exposed extracted Svelte script text. This was environment drift, not proof of a current-source regression.
- Built the checkout's slim CLI 0.16.0 and reran with explicit process-local CREPUS_BIN and CREPUS_ABI_LIB paths, as the existing plugin README prescribes. All 20 Python tests pass, including native callbacks/errors, template/event NUL guards, nonfinite event rejection, frontend filename preservation, HTML metadata and UTF-8 transport checks. Test expectations were unchanged; installed CLI/runtime files and global environment were untouched.
- Evidence: `/tmp/crepus-python-current-audit/` contains initial/final test logs, build/version evidence and binary/wrapper hashes. Repository whitespace check passes. No production/test source changes, commits, pushes, branches or deployment.
- Native macOS current-checkout evidence; mock transport cases remain scoped to their assertions. Cross-thread use, other operating systems and UI latency were not validated. Users invoking the older installed CLI can still observe its older behavior.

### 2026-10-18 — Go CLI response UTF-8 boundary

- Audited Go subprocess/context/error paths; independent initial review found no new general subprocess issue. Primary identified a concrete response boundary gap: encoding/json.Unmarshal replaces malformed UTF-8 inside JSON strings instead of returning an error (official behavior: https://pkg.go.dev/encoding/json#Unmarshal).
- Delegated test ownership for `plugins/go/invalid_utf8_test.go`; primary owned production and verification. Labelled CLI transport stubs emit exact response bytes, verify the CLI actually ran, and cover an invalid byte, a malformed multibyte sequence and valid CJK/astral text. Both malformed baseline cases incorrectly returned a nil error; the valid control passed.
- Shared RenderIR now validates successful stdout with existing unicode/utf8.Valid before JSON decoding. Process failures retain their existing errors; RenderHTML and session render/dispatch inherit rejection through the shared caller. No dependencies or generated ABI code changed. Documented the Go transport boundary.
- All 21 top-level Go tests pass with the current checkout CLI, including the three response subcases and existing real CLI integration cases; go vet, gofmt and repository whitespace checks pass. Independent reviewer approved actual guard placement/callers. Primary inspected before/current source/test/docs diffs. Evidence: `/tmp/crepus-go-response-utf8/` and test snapshot `/tmp/crepus-go-response-utf8-fix/`.
- The malformed-response fixture is transport proof, not a simulated compiler. Native macOS Go 1.26.5 was exercised; Unix-script fixtures do not establish Windows behavior, and UI latency was not measured. No commits, pushes, branches or deployment.

### 2026-10-18 — Bun response UTF-8 without blocking pipe draining

- Current TypeScript/Bun plugin exposes asynchronous render methods only. Independent transport review confirmed a response-side gap distinct from existing source-file validation: Response.text replaced malformed UTF-8 before JSON.parse. Primary baseline regressions demonstrated that two successful child responses with invalid bytes incorrectly resolved.
- Kept Promise.all draining stdout, stderr and exit status concurrently; stdout is now collected as bytes. Nonzero exits still report stderr first. Successful output is decoded with fatal UTF-8 TextDecoder before JSON parsing, retaining default BOM stripping. Encoding contract reference: https://encoding.spec.whatwg.org/#interface-textdecoder . Two production lines changed; no dependencies or APIs added.
- Delegated async transport test ownership, primary owned production/integration. Added two invalid-byte response cases and a raw Unicode/BOM control; strengthened the existing nonzero large-output case with an invalid stdout byte to verify diagnostic precedence. All 24 Bun tests (52 assertions) pass with the current checkout CLI, including event-loop yielding, full stdin/large dual-pipe drainage, subprocess failures and malformed JSON. Bun bundle build and repository whitespace checks pass.
- Independent reviewer approved actual drain/decode/exit ordering and BOM compatibility; primary inspected snapshot-to-current source/tests/docs diffs. Updated shared Go/Bun transport documentation. Evidence: `/tmp/crepus-bun-response-utf8/` and test snapshot `/tmp/crepus-ts-response-utf8-fix/`.
- Exercised native macOS Bun 1.4.0; labelled malformed-response stubs prove transport behavior, not compiler semantics. Other operating systems, physical UI/frame latency and cancellation policy were not validated. No commits, pushes, branches or deployment.

### 2026-10-18 — Python context source line-ending parity

- Previous turn made progress rejecting malformed Bun response UTF-8. Parallel read-only Bun lifecycle review found no new concrete issue; Python review followed a source fidelity gap. Default text-file reads translate CRLF and lone CR to LF (https://docs.python.org/3/library/functions.html#open).
- Confirmed with the actual current CLI before editing production: a Svelte text source containing CR and CRLF retains them through direct native ir, while Python render_ir(context={}) changed both to LF. Before/after root evidence is in `/tmp/crepus-python-source-newlines/`.
- Delegated only transport regression ownership; its terminal baseline failed the exact source payload assertion while the other three transport tests passed. Primary replaced read_text with read_bytes().decode("utf-8") in shared render_ir: one production line, strict invalid-byte rejection retained, no dependencies or API changes. HTML and session callers inherit the correction.
- All 21 Python tests pass using explicit current checkout CLI and ABI library, including native ABI sessions, source/response UTF-8 errors and the new exact CRLF/lone-CR envelope check. Actual CLI/Python root parity and render_html CR preservation also pass. Independent reviewer approved callers, strict decoding, file errors and unchanged BOM behavior. Primary inspected before/current source/test/docs diffs; whitespace checks pass.
- Python 3.14.6 on native macOS was exercised. The labelled Unix transport fixture does not prove Windows behavior, and no UI latency measurement was performed. No commits, pushes, branches or deployment.

### 2026-10-18 — Ruby failed-command diagnostic precedence

- Previous turn made progress preserving Python source line endings. Parallel Ruby/PHP reviews identified a concrete Ruby error-ordering issue and a separate PHP no-context diagnostic gap. Ruby validated failed stdout before examining exit status, hiding valid stderr when stdout contained malformed UTF-8.
- Delegated only Ruby transport regression ownership. Labelled Unix CLI stub exits 9 with stdout byte FF and exact Unicode stderr; test covers nil and empty-hash contexts under ASCII locale. Terminal baseline failed with Encoding::InvalidByteSequenceError instead of RuntimeError carrying stderr (first context failed before the second ran).
- Primary changed one production line in shared render_ir: validate both streams on success, only stderr on failure. Strict successful stdout/stderr validation and failed stderr validation remain intact; JSON parsing and all HTML/session callers are unchanged. Independent reviewer approved both transport routes and callers.
- All 15 Ruby tests with 22 assertions pass against the current checkout CLI via explicit PATH, including both new context modes; Ruby syntax and whitespace checks pass. Primary inspected snapshot-to-current source/test/docs diffs. Evidence: `/tmp/crepus-ruby-exit-diagnostic/` and baseline `/tmp/crepus-ruby-error-precedence-fix/`.
- Native macOS Ruby 4.0.6 only; labelled malformed-byte transport fixture is not compiler or UI-latency proof. PHP bare/no-context renderIr still discards detailed stderr while its context route preserves it; retained as concrete next work. No commits, pushes, branches or deployment.

### 2026-10-18 — PHP no-context CLI diagnostics

- Previous turn fixed Ruby failed-stdout diagnostic precedence and retained a concrete PHP gap: context renders captured stderr, while bare file renders used exec and threw a generic error. Revalidated current source and delegated only a new transport smoke regression.
- Terminal baseline: labelled child exits 9 with malformed stdout and exact Unicode stderr; context control passes first, bare call fails with generic "crepus native ir failed". New `plugins/php/transport_smoke.php` is automatically selected by existing `scripts/plugin-smoke.sh` wildcard; no new tooling or dependency.
- Primary reused the existing proc_open path for both modes, selecting argv/payload up front, closing unused bare stdin promptly, and retaining the temporary stderr file, exit-first error check and JSON_THROW_ON_ERROR. Removed the duplicate shell exec branch. Resolved bare path and context filename/baseDir/object shape remain intact; HTML/session callers share the fix.
- All eight PHP smoke scripts pass against the current checkout CLI, including existing 262144-byte context, path boundaries, frontend filename, empty context and session checks. Additional real CLI acceptance passes for Unicode/quote/shell-metacharacter paths in both modes and exact malformed-Svelte diagnostic parity. Actual native ABI dispatch passes with explicit current ABI library. PHP syntax and whitespace checks pass.
- Independent reviewer inspected actual before/current source and regression with no blockers. Primary inspected source/test/docs diffs. Evidence: `/tmp/crepus-php-diagnostics/`; baseline `/tmp/crepus-php-diagnostics-fix/red.log`.
- Native macOS PHP 8.5.8 exercised; CI PHP 8.3, Windows and UI latency were not tested here. Existing context stdin write policy is unchanged. No commits, pushes, branches or deployment.

### 2026-10-18 — Python failed-output diagnostic precedence

- Previous turn made progress preserving PHP bare-route diagnostics. Parallel Python/JVM review confirmed Python eagerly decoded both subprocess streams in text mode, allowing invalid failed stdout to hide valid stderr. Official binary/text and newline behavior: https://docs.python.org/3.14/library/subprocess.html#frequently-used-arguments .
- Delegated only Python transport regression ownership. Terminal baseline: six tests with exactly two failures, one per context None/{} subcase; stdout FF from an exit-9 child caused UnicodeDecodeError instead of the exact normalized Unicode stderr RuntimeError. A successful-child invalid-stderr control passed, establishing existing strictness.
- Primary switched shared render_ir to binary subprocess capture and explicit UTF-8 stdin encoding. Stderr strictly decodes with prior CRLF/lone-CR normalization; nonzero status raises its trimmed diagnostic before stdout is decoded. Successful stdout remains strictly UTF-8 decoded before JSON parsing, and successful stderr remains strict. Existing source-byte fidelity, path checks and HTML/session delegation preserved; no dependencies or new APIs.
- All 23 Python tests pass using explicit current checkout CLI and ABI library, including both new failure modes, strict source/response/stderr checks, exact source line endings and native ABI sessions. Independent reviewer approved the proposed exact implementation and callers; primary inspected actual source/test/docs diffs and whitespace checks pass. Evidence: `/tmp/crepus-python-error-precedence/`.
- Parallel review found a concrete next issue: Java and Kotlin successful stdout use replacement UTF-8 decoding, allowing malformed byte FF in a response to become U+FFFD; their failure-status ordering is already correct. This finding is source-backed and not yet runtime reproduced or fixed.
- Native macOS Python 3.14.6 only; labelled Unix stubs do not prove Windows or compiler behavior. UI latency was not measured. No commits, pushes, branches or deployment.

### 2026-10-18 — Java/Kotlin strict response UTF-8

- Previous turn fixed Python failed-output precedence and retained a source-backed JVM response gap. Revalidated shared runCli callers: both Java and Kotlin decoded successful stdout with replacement behavior, affecting IR and HTML.
- Delegated only test ownership for existing Java/Kotlin Unicode test classes and `scripts/test-plugin-diagnostics.py`. Terminal expanded baseline independently demonstrated all eight malformed responses silently accepted (two languages × IR/HTML × FF/C3-28); eight raw-Unicode/failure-precedence controls and existing noisy diagnostics/HTML cleanup checks passed. Test headers distinguish default actual-CLI acceptance from optional labelled byte fixtures.
- Primary replaced the shared successful-return decoder in each adapter with a fresh UTF-8 CharsetDecoder over captured bytes and added ByteBuffer imports. Defaults report malformed input: https://docs.oracle.com/en/java/javase/25/docs/api/java.base/java/nio/charset/CharsetDecoder.html . Exit checks, stderr decoding policy, concurrent stderr drain, process destruction and HTML context-file cleanup remain unchanged. No dependencies or API changes.
- Java compilation with -Xlint:all -Werror and Kotlin compilation with -Werror pass. Expanded diagnostic script passes all 16 response scenarios plus existing noisy diagnostics and cleanup checks. All six actual-current-CLI acceptance programs (IR/session, HTML, Unicode for each language) pass. Independent reviewer approved the actual production/test diffs and confirmed regression failures cannot be swallowed by their exception handlers. Primary inspected actual source/test/docs diffs and whitespace checks pass. Evidence: `/tmp/crepus-jvm-response-utf8/`; before snapshots and terminal red `/tmp/crepus-jvm-response-encoding-fix/`.
- Native macOS JDK 17.0.20 and Kotlin 2.4.10 exercised. Labelled malformed-response fixtures prove transport behavior, not compiler semantics; other operating systems and UI/frame latency remain untested. Stderr retains its existing replacement-decoding policy. No commits, pushes, branches or deployment.

### 2026-10-18 — C# strict response UTF-8 and BOM handling

- Previous turn fixed JVM response replacement decoding. Parallel C#/Swift audit found the same C# issue: StandardOutput.ReadToEndAsync replaced invalid UTF-8 before JSON deserialization. Swift already decodes successful JSON bytes directly but loses stderr details on failure; retained as next concrete work.
- Delegated only C# transport smoke additions and existing Program/script wiring. Terminal baseline independently failed six cases (FF, C3-28, UTF-8 BOM+FF × IR/HTML), while raw Unicode, valid UTF-8 BOM and failed-invalidstdout/exact-Unicode-stderr controls passed. New --transport mode is included in scripts/plugin-smoke.sh.
- Primary captures stdout bytes with CopyToAsync while stderr drains concurrently, then checks exit status before strict UTF8Encoding decoding. Explicit optional UTF-8 BOM removal avoids StreamReader BOM detection resetting strict fallback. Both public APIs share the correction. This formalizes UTF-8 wire output: UTF-16/32 BOM output formerly autodetected by StreamReader is rejected. Stderr policy and source reading remain unchanged; no new dependencies.
- Solution and smoke project builds pass with warnings treated as errors. Initial no-build run after solution-only build used the old smoke-project dependency copy (solution excludes smoke); explicitly built the smoke project before the authoritative green. All 12 transport cases now pass, plus four existing acceptance modes: current CLI IR/session/frontend/include/large-stderr, input defaults/metadata HTML, link HTML/session, native ABI NUL rejection/Unicode.
- Independent reviewer approved actual production/tests and exception-filter behavior; primary inspected source/test/script/docs diffs and whitespace checks pass. Evidence: `/tmp/crepus-csharp-response-utf8/`; baseline and test snapshots `/tmp/crepus-csharp-response-encoding-fix/`.
- Native macOS .NET SDK 8.0.129 only; labelled Unix/Python stubs do not establish Windows, compiler or UI/frame performance. UTF-16/32 response compatibility is intentionally narrowed to the CLI UTF-8 contract. No commits, pushes, branches or deployment.

### 2026-10-18 — Swift CLI diagnostics without dual-pipe blocking

- Previous turn fixed C# strict response decoding and retained Swift's concrete diagnostic loss: renderIr threw NSError with only domain/status while stderr was inherited. Revalidated shared renderHtml caller and current Swift package/tests; applied existing Swift expert workflow.
- Delegated only two transport regressions. Terminal baseline ran four tests: exactly two localizedDescription failures (IR/HTML); original domain/status checks, noisy-success control and existing actual-CLI large-IR/attribute tests passed. Labelled subprocess fixture writes 16384 Unicode diagnostic lines, invalid stdout on exit 9 and valid Unicode JSON on success.
- Primary captures stderr in a regular file inside a private mode-0700 UUID temporary directory, retaining stdout's existing pipe drainage. Ordered defers close/remove on every path. After exit, best-effort capture is forwarded to parent stderr and nonempty diagnostic text is attached to NSError without changing its domain/status. Empty diagnostics retain the old default description; stdout JSON decoding stays after status checks. Diagnostic read/forward failures cannot mask known CLI status. No dependencies or public API changes.
- All four Swift XCTest tests pass with warnings treated as errors using current checkout CLI and isolated scratch build. Separate warnings-as-errors native probe verifies actual malformed-Svelte CLI error, empty-stderr exit, missing executable, successful-child invalid JSON, diagnostic scratch-directory cleanup and exact successful warning forwarding. Probe initially had a throwing expression in precondition's nonthrowing autoclosure; corrected only the disposable probe before compiling/running authoritative acceptance.
- Independent reviewer approved actual source/tests, cleanup order and meaningful exception assertions. Primary inspected before/current source/test/docs diffs; whitespace checks pass. Evidence: `/tmp/crepus-swift-diagnostics/`.
- Native macOS Swift 6.4 only. Captured stderr is forwarded after process exit, so warning/error visibility is delayed; diagnostic bytes use replacement UTF-8 decoding while JSON behavior is unchanged. Other platforms and UI/frame latency were not tested. No commits, pushes, branches or deployment.

### 2026-10-18 — C# strict UTF-8 source without narrowing BOM support

- Previous turn preserved Swift diagnostics. Parallel source-boundary review identified C# File.ReadAllTextAsync's replacement fallback; Rust reads are already strict and V/Zig delegate source paths to the CLI, with no new confirmed source-decoding gap in that pass.
- Delegated only additions to existing C# TransportChecks. Terminal baseline confirmed six malformed-source cases (FF/C3-28/BOM+FF × IR/HTML) accepted and CLI spawn markers present. Valid UTF-8 BOM source control and all 12 existing response scenarios passed.
- Primary changed one production read to File.ReadAllTextAsync(path, new UTF8Encoding(true,true)), with one why-comment. .NET8 matches the supplied preamble before generic BOM detection, retaining strict UTF-8 decoding for BOM-prefixed input while preserving other BOM-encoded source behavior. Independent review verified official File/StreamReader implementation rather than assuming this ordering: https://raw.githubusercontent.com/dotnet/runtime/v8.0.0/src/libraries/System.Private.CoreLib/src/System/IO/StreamReader.cs . Shared HTML/session callers inherit the rejection before process launch.
- Warnings-as-errors smoke project build passes. Expanded --transport passes all 20 scenarios (12 responses, two valid-BOM source controls, six malformed-source rejection/marker checks). Actual current-CLI IR/session/frontend/include acceptance passes for five source BOM encodings: UTF-8, UTF-16 LE/BE and UTF-32 LE/BE. Existing input HTML, link HTML/session and native ABI Unicode/NUL acceptance modes also pass.
- Primary inspected actual source/test/docs diffs; whitespace checks pass. Evidence: `/tmp/crepus-csharp-source-utf8/`; test snapshot and terminal red `/tmp/crepus-csharp-source-encoding-fix/`.
- Native macOS .NET SDK 8.0.129 only. Unix/Python transport fixtures do not establish Windows or UI/frame behavior. Malformed UTF-16/32 behavior remains the existing BOM decoder policy; this fix hardens UTF-8 source only. No commits, pushes, branches or deployment.

### 2026-10-18 — V successful stderr isolation

- Previous turn hardened C# source decoding. Parallel module review confirmed Swift/Rust HTML omissions belong to their documented reference subset, so did not expand them as alleged regressions. V had a concrete success-path defect: installed os.execute merges stderr into stdout, corrupting successful JSON/HTML responses.
- Delegated only V regression ownership. Terminal baseline shows noisy valid IR rejected and exact HTML contaminated by warning text; both nonzero Unicode stderr controls pass. Labelled CLI fixture emits 32768 Unicode warning lines before valid output, exceeding a typical pipe buffer; actual compiler behavior is tested separately by existing cases.
- Primary reused the already-private per-call context directory for a stderr file and added a quoted command-level stderr redirection, overriding os.execute's merge. Success returns stdout only; failures prefer stderr with captured-output fallback. Existing directory defer removes both files; no new dependencies, process threads or public API changes.
- All six V test functions pass with explicit current CLI and existing current native ABI linkage, including concurrent isolated context files, actual HTML/include behavior, callback/string ABI lifecycle and new noise/error cases. V formatter verification and whitespace checks pass; primary formatted only new test map literals after inspecting the formatter diff. Evidence: `/tmp/crepus-v-stderr/`; baseline `/tmp/crepus-v-stderr-fix/red.log`.
- Independent reviewer approved actual source/tests, redirection ordering, fallback and deferred cleanup. Primary inspected actual source/test/docs diffs. Native macOS V 0.5.1 exercised. Unix shell override and Windows os.execute redirection behavior were source-reviewed, but Windows was not executed. UI/frame latency was not measured. No commits, pushes, branches or deployment.

### 2026-10-18 — Rust large-stdin transport drainage

- Previous turn fixed V stderr isolation. Revalidated Rust reference adapter: it wrote the complete stdin envelope before starting wait_with_output, allowing a child that emits stderr before consuming stdin to deadlock; early exit surfaced only BrokenPipe and discarded the child diagnostic.
- Delegated only a Unix integration test file, kept all Cargo execution with primary. Terminal baseline reproduced both issues: >1MiB Unicode request blocked against ~393KiB stderr-first child until its five-second watchdog exited, and early exit lost exact Unicode diagnostic behind BrokenPipe. Labelled fixtures verify the full template/context, not a simulated compiler.
- Primary takes ownership of child stdin and writes on a standard-library worker while wait_with_output drains both pipes and waits. The writer is always joined before returning; failed status reports stderr before write errors, while successful status still requires the write to succeed before JSON parsing. No payload clone, dependency or public API change; API remains synchronous.
- Final tests add a third control: valid JSON/exit0 without consuming large stdin must retain io::ErrorKind::BrokenPipe. All eight Rust tests pass (seven existing real-CLI/unit checks plus one integration test covering three transport scenarios); strict all-target Clippy, formatter and whitespace checks pass. Independent reviewer approved actual source/test lifecycle and error ordering; primary inspected all diffs. Evidence: `/tmp/crepus-rust-stdin-drain/`.
- Native macOS Rust tooling and Unix/Python fixtures exercised; Windows and UI/frame latency untested. One joined writer thread is added per render; this does not add cancellation, timeouts or a nonblocking public API. No commits, pushes, branches or deployment.

### 2026-10-18 — Java/Kotlin stdout-before-stdin deadlock

- Previous turn fixed Rust request/output drainage. Revalidated JVM adapters: stderr already drained concurrently, but full stdin writes preceded stdout drainage, allowing a child filling stdout before reading a large request to block both processes. Early exit also replaced useful stderr with IOException/BrokenPipe.
- Delegated only existing Unicode test-class/diagnostic-script additions. Terminal baseline independently failed four cases (two languages × stdout-pressure/early-nonzero exit); a five-second child watchdog bounded deadlock reproduction. Exit0/unread-stdin write-error controls and all earlier response/error checks passed. Labelled pressure fixture writes 262144 whitespace bytes before reading and verifies exact source and 1.2MB Unicode context.
- Primary starts a stdout FutureTask reader alongside the existing stderr reader before writing, captures only stdin IOException, then drains/waits. Nonzero status prioritizes stderr; successful status propagates incomplete-write errors before strict successful-output decoding. Removed now-unneeded ByteArrayOutputStream usage. Existing final process destruction and HTML context cleanup remain unchanged; no dependencies or public API changes.
- Java -Xlint:all -Werror and Kotlin -Werror builds pass. Expanded script passes all 22 byte/large-request scenarios plus existing noisy diagnostics and HTML cleanup checks; six actual current-CLI acceptance programs pass. Independent reviewer approved actual source/tests, error ordering, stream ownership and assertion behavior. Primary inspected actual production/test/script/docs diffs; whitespace checks pass. Evidence: `/tmp/crepus-jvm-duplex/`; snapshots and terminal red `/tmp/crepus-jvm-duplex-fix/`.
- Parallel PHP review identified the same stdin-before-stdout ordering in proc_open; its existing stderr file does not prevent stdout pressure. Retained as concrete next work. C# already starts both readers before writing, but its early write-error diagnostic ordering still merits a regression.
- Native macOS JDK17/Kotlin2.4.10 only. Public JVM APIs remain synchronous, and no cancellation, other-OS or UI/frame-latency guarantee was tested. No commits, pushes, branches or deployment.

### 2026-10-18 — PHP stdout-before-stdin drainage

- Previous turn fixed JVM duplex ordering and retained PHP's equivalent issue. Revalidated proc_open: synchronous fwrite to child stdin preceded stdout reads, so stdout pressure could deadlock despite separate stderr storage.
- Delegated only existing PHP transport smoke additions. Terminal baseline independently failed IR/HTML pressure cases after a five-second child watchdog; early nonzero exact-Unicode diagnostic controls and prior bare/context cases passed. Labelled child writes 262144 JSON-whitespace bytes before reading and verifies exact template and 400000-character Unicode context (1.2MB decoded UTF-8; about 2.4MB escaped JSON wire payload).
- Primary prepares a tmpfile stdin resource before spawn, checks every partial write for false/zero progress, verifies rewind, and passes it directly as descriptor0. Parent immediately drains stdout; bare route gets empty-file EOF. Creation/writing occur inside existing stderr-protected try and both resources close in finally. Existing status-first stderr handling and JSON decoding remain unchanged. No dependencies or public API changes.
- All eight PHP smoke scripts pass with current checkout CLI, including full large context, path boundaries, frontend filename, empty context and HTML/session behavior. Direct CLI acceptance and actual native ABI dispatch pass; both PHP syntax checks and whitespace checks pass. Independent reviewer approved actual source/test cleanup, exact-payload checks and assertion behavior. Primary inspected source/test/docs diffs. Evidence: `/tmp/crepus-php-duplex/`; snapshots and terminal red `/tmp/crepus-php-duplex-fix/`.
- Native macOS PHP8.5.8 only. Temporary input now uses disk storage and must be prepared before launch; public calls remain synchronous. PHP8.3, Windows and UI/frame latency were not tested. No commits, pushes, branches or deployment.

### 2026-10-18 — C# early stdin failure diagnostic precedence

- Previous turn fixed PHP duplex blocking. Revalidated C# already drains both output streams before writing, but stdin WriteAsync/Close exceptions bypassed process exit handling and hid stderr diagnostics.
- Delegated only existing TransportChecks additions. Terminal baseline failed exactly two early-nonzero cases (IR/HTML) with IOException/BrokenPipe instead of exact Unicode stderr. Both large stdout-before-stdin pressure controls and exit0/unread-stdin IOException controls passed; prior 20 byte/source scenarios remained green.
- Primary scopes StandardInput in using, captures only IOException from write/disposal, continues draining/waiting, prioritizes nonzero stderr, then rethrows a successful-child write failure with ExceptionDispatchInfo preserving its stack. Existing strict source/response decoding, BOM behavior and output capture remain intact. No dependencies or public API changes.
- All 26 transport scenarios pass, along with warnings-as-errors smoke project build and four existing C# acceptance modes: actual CLI IR/session/frontend/include/noisy output, input HTML, link HTML/session and native ABI NUL/Unicode. Independent reviewer approved actual stdin closure, IO-only capture, stack preservation, status ordering and meaningful test assertions. Primary inspected actual source/test/docs diffs; whitespace checks pass. Evidence: `/tmp/crepus-csharp-early-exit/`; test snapshot and terminal red `/tmp/crepus-csharp-duplex-fix/`.
- Native macOS .NET8.0.129 only. Unix/Python transport fixtures do not establish other operating systems or UI/frame latency. API stays asynchronous with buffered output; cancellation/timeout behavior is unchanged. No commits, pushes, branches or deployment.

### 2026-10-18 — Accessor macros retain authored frontend identity

- Previous turn fixed C# early stdin diagnostics. Parallel framework review found no new runtime hotreload defect in the reviewed publication/prepared-graph/watcher flows, but found accessor macros canonicalizing away symlink extensions. Canonical targets named .txt could parse Vue as JSX, and ID deduplication omitted frontend identity.
- Delegated only three macro unit regressions; primary retained all Cargo work. Terminal baseline failed authored-path constructor and Vue ID collection cases. Separately compiled a real public template_refs!/Ratatui consumer: `<span v-if="show">` through view.vue→nested/source.txt visibly rendered when show=false. Initial offline consumer attempt lacked an existing locked approx dependency; allowed normal locked resolution/download, then captured authoritative runtime failure before production edits. Repository dependencies were not changed.
- ID collection now uses existing include_frontend_path for ordinary templates and keys seen entries by canonical source, selected component and frontend identity. Canonical reads/include directories and named-component parsing stay intact. Generated TUI templates use the real absolute authored path for parser selection and reload, with canonical parent assigned through existing context_mut API. A synthetic canonicalparent/alias path was rejected during design because it need not exist for reload. Updated dependency fixture's deliberately minimal Template stub for the existing context_mut API.
- Added alias-order regression: the same physical source exposed as validVue/invalidJSX must validate both frontends in either include order. All 353 tests across macros and TUI pass, including 35 macro units and the actual Cargo dependency-rebuild integration test. Strict macro all-target Clippy, formatting and whitespace checks pass. Existing ignored cases remain one known registered-file component-origin defect and four macro documentation examples.
- Actual final public macro/TUI consumer verifies false/true branch rendering, canonical include base and reload after editing target source. Independent reviewer approved source/test identity, named-component behavior and reload compatibility; recommended absolute authored path was incorporated. Primary inspected actual production/test/fixture/docs diffs. Evidence: `/tmp/crepus-macro-frontend/`; delegated test snapshot `/tmp/crepus-macro-refs-frontend-fix/`.
- Native macOS symlink behavior exercised; other platforms and UI/frame latency untested. Compile-time ID sets and canonical base remain snapshots; symlink retarget rebuild limitations and known TUI registered-file origins remain separate open work. No commits, pushes, branches or deployment.

### 2026-10-18 — Tauri named-component bundle entries

- Previous turn fixed accessor frontend identity. Parallel web/native/LVGL review found no new include-identity mismatch in those reviewed routes; focused live checks pass seven web and 35 native frontend/include/scope tests. LVGL review was source-only, not runtime verification.
- Parallel Tauri review found validate_bundle tested literal entry as a file key, rejecting ui.crepus#Card before native_ir could pass it to the native renderer's documented file#Component route. Delegated only a public-project regression; primary owned all Cargo execution and production. Terminal red confirms valid fragment entry wrongly reported missing.
- Validator now checks the file portion from split_once('#') while preserving full Bundle.entry for native component selection. All existing unsafe-file-key checks remain intact. Added public bundle/native_ir selected-Card/excluded-Unused verification, missing selected file and unrelated unsafe file controls. Documented the supported native bundle selector; no dependencies or public API changes.
- Full-feature Tauri suite passes 25 tests; default-feature library passes 11 tests. Strict all-feature/all-target Clippy, formatter and whitespace checks pass. Independent reviewer approved actual selector preservation, boundary checks and assertion coverage. Primary inspected source/test/docs diffs. Evidence: `/tmp/crepus-tauri-component-entry/` and delegated before snapshot `/tmp/crepus-tauri-component-entry-fix/`; adjacent target results `/tmp/crepus-target-identity-acceptance/`.
- Native macOS library/IR behavior only; a native Tauri app window, other operating systems and UI/frame latency were not tested. No commits, pushes, branches or deployment.

### 2026-10-18 — Cross-target entry-boundary and real CLI conversion acceptance

- Previous turn fixed Tauri component-selector validation. Parallel CLI/facade/ABI source review found no analogous native-entry selector loss: direct stdin file graphs, explicit ABI overrides and Tauri conversion wrappers retain selectors. Parallel embedded review found current named defaults, target environment, authored frontend dispatch and documented reload behavior consistent; no new defect confirmed.
- Primary ran ten focused embedded tests (filename frontend, symlink/include frontend identity and target context/defaults) and two ABI component-selection tests. All 12 pass, including explicit selector override/clear restoration and dispatch through file/template sessions.
- Rebuilt the slim current checkout CLI with locked dependencies so CLI acceptance includes recent library/macro changes. Actual `crepus tauri convert --target all` on a disposable project with ui.crepus#Card succeeds. Verified root and Android fixture parity, generated Swift/Compose contain SelectedCard but exclude UnusedCard, and both converted views/main.crepus and desktop/views/main.crepus retain selector and re-render through the actual CLI to identical IR.
- Initial acceptance checker incorrectly expected an iOS JSON fixture; inspected current generated SwiftPM/App contract, which consumes generated SwiftUI source instead, and corrected the checker against those actual files. Conversion itself succeeded on first execution; no product code changed for that checker assumption.
- Evidence: `/tmp/crepus-entry-boundary-acceptance/`, retained converted project `/tmp/crepus-entry-boundary-acceptance/tauri-selected-u60wwqy5/`. Primary inspected relevant sources and output artifacts; repository whitespace check passes. This turn changes only this audit ledger; no speculative fix added.
- Native macOS host/IR/generated-source acceptance only. Generated mobile/desktop apps were not compiled, launched or device-tested here; other operating systems and UI/frame latency remain unverified. Known TUI registered-file origin and symlink-retarget rebuild limitations remain separate open work. No commits, pushes, branches or deployment.

### 2026-10-18 — Generated Swift compilation and raw command identifiers

- Built the retained converted project's SwiftPM NativeShell library on macOS with warnings as errors. Build succeeds; evidence: `/tmp/crepus-converted-swift-acceptance/build.log`. Parallel source review found generated helper visibility and platform guards consistent. This compiles the library, not the app entry or linked Rust bridge, and is not iOS/device acceptance.
- Parallel Tauri macro review identified raw command name handling. Primary added an invocation regression before editing production code; terminal red showed both command and handler macros panic while constructing `__crepus_command_r#type`. Normalized bridge identifiers with the existing `IdentExt::unraw` in both macros and registered the unprefixed wire name. `fn r#type` now invokes as `type`; ordinary command registration is covered by existing tests.
- All 26 full-feature Tauri tests pass, including alias, argument naming, async runtime and raw-name regression. Strict all-feature/all-target Clippy for Tauri and its macros, focused formatting and repository whitespace checks pass. Primary inspected the scoped source/test diff against before snapshots. Evidence: `/tmp/crepus-tauri-raw-command/`.
- No dependencies, public API additions, commits, pushes, branches or deployment. Other platforms, full workspace tests, app launch and frame latency remain unverified.

### 2026-10-18 — Converted Rust runtime and linked Swift bridge acceptance

- Revalidated repository instructions and active processes; waited until the unrelated Cargo lane was terminal before starting generated-project Cargo checks. Parallel read-only native scaffold/bridge and LVGL XML/control reviews found no new confirmed supported-input defect. Independent review of the preceding raw-command fix found no blocker.
- Tested the actual retained Tauri-converted Rust project, rather than only its source template: 43 default-feature tests and 44 all-feature tests pass. Built its default-feature static/dynamic/Rust libraries. Evidence: `/tmp/crepus-converted-rust-acceptance.log`, `/tmp/crepus-converted-rust-acceptance-all-features.log`, `/tmp/crepus-converted-rust-build.log`.
- Compiled all generated NativeShell Swift sources together with an assert-based executable and linked the generated Rust dynamic library, using warnings as errors. Executed successfully: action dispatch and explicit result application, numeric state, long Unicode text containing embedded NUL, boolean/list evaluation and scoped Unicode/NUL values all round-trip across the actual bridge. Evidence and executable: `/tmp/crepus-converted-rust-acceptance/` (`BridgeAcceptance.swift`, `swift-link.log`, `swift-run.log`).
- Initial probe incorrectly expected dispatch alone to update the view model; source inspection confirmed the explicit result-sink/applyResult contract, and the probe now applies the returned result before evaluating model state. No production fix was warranted. This turn changes only the audit ledger; primary inspected the added record and repository whitespace check passes.
- Native macOS executable/bridge acceptance only, without a UI window. Android JNI loading, iOS/device execution, app lifecycle and frame latency remain unverified. No dependencies, commits, pushes, branches, deployment or remote mutations.

### 2026-10-18 — WebExtension page URL suffix preservation

- Parallel WebExtension review identified resolve_page rewriting only URLs ending literally in `.crepus`. Primary reproduced the actual CLI failure: a configured `pages/options.crepus?new=1` override references a nonexistent asset despite emitted options.html. The local asset validator already admits query/fragment suffixes. Delegated tests-only public manifest regression also failed before production edits.
- resolve_page now splits at the first query/fragment delimiter, rewrites only the pathname's final `.crepus`, and preserves the suffix verbatim. All action popup/fallback, options page/UI and chrome override callers share the fix. Public regression covers all these fields, precedence, repeated extensions, fragment-before-query and already-HTML URLs with `.crepus` only in their suffix. No dependencies or public API additions.
- All 56 WebExtension tests pass. Strict all-feature/all-target Clippy, focused formatting and repository whitespace checks pass. Rebuilt the actual checkout CLI and verified four successful builds: Chromium and Firefox, each with explicit options_page and options_ui, checking emitted HTML and exact manifest suffixes. Independent review found no blocker; primary inspected actual scoped diff. Evidence: `/tmp/crepus-webext-page-suffix/`.
- Acceptance also exposed separate existing auto-detection behavior: presence of pages/options.crepus synthesizes options_ui even with an explicit options_page, suppressing the configured page. This remains concrete follow-up work; suffix acceptance uses a non-auto-detected settings.crepus entry to isolate this fix. Safari fixture stopped at its required bundle_identifier configuration, so no Safari build success is claimed. Browser installation/UI execution and WASM compilation were not exercised by these runtime-free fixtures.
- Generated converted Rust scaffold also passes strict all-feature/all-target Clippy, closing the preceding host lint gate (`/tmp/crepus-converted-rust-acceptance/clippy.log`). Parallel embedded clipping/layout review found no new confirmed contract defect. No commits, pushes, branches, deployment or remote mutations.

### 2026-10-18 — WebExtension explicit page configuration precedence

- Closed the preceding confirmed options-page follow-up. Revalidated source and actual CLI: with conventional options.crepus/popup.crepus present, auto-detected options_ui suppressed explicit options_page, and a parallel review identified the equivalent explicit popup_html override. Actual pre-fix CLI output confirmed both configured choices were lost. Two focused regressions failed in a terminal run before production edits.
- Auto-detection now requires both popup fields to be absent before discovering a popup, and both options fields to be absent before discovering options UI. Updated inaccurate comments. Existing action_popup-over-popup_html and options_ui-over-options_page authored precedence remains intact. Regressions assert public manifest output for explicit fields, URL suffix preservation, automatic defaults and repeated detection.
- All 27 CLI WebExtension checks pass. Strict CLI binary/test Clippy with no default features, focused formatting, and repository whitespace checks pass. Rebuilt the actual CLI and verified six successful Chromium/Firefox builds: explicit legacy fields survive, explicit higher-priority fields win when both are authored, and absent configuration still discovers conventional files. Evidence and before snapshot: `/tmp/crepus-webext-page-precedence/`. Independent review found no blocker; primary inspected the actual scoped diff.
- No dependencies, public API additions, commits, pushes, branches, deployment or remote mutations. These are runtime-free generated-output checks; browser installation/UI behavior, WASM compilation, Safari builds and full default-feature/workspace gates were not exercised here.

### 2026-10-18 — WebExtension icon directory detection

- Adjacent CLI acceptance passes six integration tests covering dev error recovery/directory recreation, transactional output preservation, repeated builds and missing assets. Evidence: `/tmp/crepus-webext-adjacent-acceptance.log`.
- Parallel review found auto_detect_icons selecting entries by name without checking file type. Primary reproduced a real CLI build failure from an otherwise unrelated icons/icon128 directory (`expected a regular file`), then ran a delegated regression to terminal red before production edits (two detected icons instead of one).
- Skip non-file entries before applying existing icon naming rules. Regular-file symlink behavior and explicit-map precedence are preserved; no broader filename-policy change. Regression covers directory exclusion, regular extension/action icon discovery and explicit maps. All 28 CLI WebExtension checks and five output integration tests pass after the fix, plus strict slim CLI binary/test Clippy, focused formatting and whitespace checks.
- Rebuilt actual CLI and verified four successful Chromium/Firefox builds: directory alone is ignored, and adding regular PNG extension/action icons retains their exact manifest entries and packaged bytes. Evidence/before snapshot: `/tmp/crepus-webext-icon-directory/`. Independent review found no blocker; primary inspected actual scoped source/test diff and this ledger record.
- No dependencies, commits, pushes, branches, deployment or remote mutations. Browser installation/UI execution, image dimension validation, WASM compilation, Safari builds and full default-feature/workspace gates remain outside this scoped acceptance.

### 2026-10-18 — Safari packaging preserves previous browser output

- Closed Safari packaging configuration acceptance with a real local Xcode packager run: generated Xcode project copies popup/options manifests with query/fragment suffixes intact. Then reproduced a failed repeat build replacing previous HTML before reporting that the project already exists. Delegated output regression reached terminal red before production edits; previous dist/safari bytes were lost on missing-config failure.
- Package Safari from the existing staging directory before publishing it; keep --copy-resources so generated project resources survive stage publication. Package and argument helpers now return Result instead of exiting, allowing existing staging cleanup to run. Reused existing ancestor-resolving output-path helper through crate-local visibility to reject overlapping project/browser output paths before writes. Existing projects remain protected; no new dependencies or external API changes.
- All 28 CLI WebExtension checks and six output integration tests pass, including missing configuration, existing project, invalid platform and overlapping location failures preserving complete prior output with no stage leaks. Strict slim CLI binary/test Clippy, focused formatting and whitespace checks pass. Independent design/actual-diff review found no blocker; primary inspected all four affected source/test files against before snapshots.
- Real post-fix staged Safari packaging succeeds. Verified published HTML, exact popup/options suffixes, copied Xcode project resources, no stale temporary-stage references in project files and no leftover stage directory. A controlled xcrun process failure retains its diagnostic, previous browser output and cleans staging. Evidence: `/tmp/crepus-safari-transaction/`; initial Safari package/repeat-failure evidence: `/tmp/crepus-safari-package-acceptance/`.
- Separate generated Xcode project directories can remain after partial packager failure or a later publication failure; documented this ownership/retry limit rather than deleting arbitrary project paths. Project build/signing, Safari installation/UI, device behavior, WASM runtime and full workspace/default-feature gates remain unverified. No commits, pushes, branches, deployment or remote mutations.

### 2026-10-18 — Safari generated-project build and packager identifier diagnosis

- Parallel read-only review found no new defect in the preceding resolved path checks: existing symlink/case aliases canonicalize, dangling symlinks/file ancestors fail closed, and containment checks are component-aware. Revalidated live processes and preserved an unrelated Cargo lane; no Cargo checks started here.
- Built the real staged Safari project's macOS host and extension with signing disabled and Swift warnings as errors. Compilation/linking completed, but embedded-binary validation failed: requested base dev.crepus.staged became extension dev.crepus.staged.Extension while host was dev.crepus.Staged-Safari. Direct invocation of the installed safari-web-extension-packager reproduces identical mismatched settings, independently of Crepuscularity. Installed tool: Xcode 27.0 beta, build 27A5194q; its local help describes --bundle-identifier as the generated app identifier.
- Tested a disposable-artifact-only correction: read project objects via plutil JSON, follow application target configuration lists, and normalize only the two host configuration identifiers to the requested base. Extension configurations remain unchanged. Direct plutil -replace on OpenStep failed (conversion unsupported), so converted the generated project to XML plist first, then replaced the targeted keys. An accidentally continued unchanged build repeated the same embedded-ID failure; no framework source was changed during these probes.
- Corrected XML project builds successfully through embedded-extension validation. Inspected built host/extension executable files and Info.plist identifiers, embedded manifest popup/options suffixes and packaged HTML. App Intents emits a metadata-skipped warning because the generated app has no AppIntents dependency; no Swift compilation failure. Evidence: `/tmp/crepus-safari-transaction/` (xcode-build.log, direct-tool-project, packager-help.log, project-before-identifier-fix.pbxproj, xcode-build-normalized-xml.log).
- Concrete follow-up remains: decide and implement bounded generated-project host identifier normalization with a regression, then repeat the actual build without manual artifact correction. Current framework still relies on the installed packager's generated identifiers; this turn changes only this audit ledger. Signing, installation, Safari UI and device execution remain unverified. No commits, pushes, branches, deployment or remote mutations.

### 2026-10-18 — Automatic Safari host bundle identifier correction

- Closed the preceding generated-project follow-up. A fresh public CLI reproduction reached terminal red before production edits: requested dev.crepus.requested yielded host dev.crepus.Identifier-regression. Evidence: `/tmp/crepus-safari-identifier-red.log`. Preserved an unrelated live Cargo lane until it completed; delegated the pure graph implementation and actual packaging regression with separate file ownership.
- After successful Safari packaging, find exactly one newly generated Xcode project and read it through installed plutil JSON conversion. Validate native target/configuration references before mutation, reject shared application/nonapplication configurations, and update only application PRODUCT_BUNDLE_IDENTIFIER settings to the requested base. Preserve extension IDs and unrelated settings; already-correct projects skip writing. Changed projects are serialized through a temporary JSON file and plutil XML conversion, which Xcode accepts. All errors propagate before browser-output publication; existing partial-project retry limitation remains explicit.
- All 32 CLI WebExtension checks pass, including four graph regressions (host configurations/extension preservation/no-op, malformed later configuration atomicity, shared configuration rejection and absent application). Real Safari CLI packaging regression verifies every generated host and extension configuration. All six output integration tests and strict slim CLI binary/test Clippy pass. Focused formatting and repository whitespace checks pass; independent actual-diff review found no blocker. Primary inspected the complete new helper and scoped source/test diffs against before snapshots.
- Fresh CLI-generated Automatic Safari project builds on macOS with signing disabled and Swift warnings as errors, without manual project edits. Verified built app/extension Info.plist IDs dev.crepus.automatic and dev.crepus.automatic.Extension, embedded popup/options query/fragment suffixes and HTML contents. Evidence/before snapshots: `/tmp/crepus-safari-identifiers/` (unit.log, cli-test.log, output-test.log, clippy.log, final-package.log, xcode-build.log). Existing Xcode derived-data cache was reused.
- No dependencies, public API additions, installed-tool modifications, commits, pushes, branches, deployment or remote mutations. Signing, Safari installation/UI, iOS/device builds and full workspace/default-feature checks remain unverified. A separate partial Xcode project can still remain after packaging or later publication failure.

### 2026-10-18 — Shared Svelte unsupported binding rejection and native queue acceptance

- Parallel reactive scheduler review found no new confirmed defect in queue membership, panic recovery, observer disposal/dependency restoration or memo retry. Parallel shared frontend review identified unsupported Svelte bindings silently lowering to ordinary attributes. Primary reproduced public CLI clientWidth="42" output for bind:clientWidth and ran a delegated rejection regression to terminal red before production edits.
- Shared Svelte binding lowering now rejects names other than documented value/checked/group/files, preserving existing empty-name/expression checks and shorthand handling. An obsolete test asserted non-form attribute passthrough; replaced it with explicit/shorthand rejection cases (this, clientWidth, currentTime, custom), all four admitted binding controls and ordinary clientWidth attribute preservation. Clarified the shared-template boundary in docs/frontends.md. Authored module compilation remains separate.
- All 524 core unit tests and all 180 slim CLI unit tests pass. Strict all-feature/all-target core Clippy, focused formatting and repository whitespace checks pass. Native authored component-reference compilation control passes for DOM/SSR, retaining its separate bind:this support. Rebuilt actual CLI; verified eight rejected directive forms, eight accepted form-binding forms and ordinary attribute rendering. Evidence/before snapshots: `/tmp/crepus-svelte-binding-fix/`; initial delegated fixture `/tmp/crepus-svelte-binding-audit/`. Independent actual-diff review found no blocker; primary inspected source/test/docs diffs.
- Fresh full native action-queue gate passes generated and snapshot Swift/Kotlin probes: serial FIFO/exactly-once/captured hooks, blocked-handler main heartbeat, sink ordering/error delivery, immediate input drafts and asynchronous picker imports. Evidence: `/tmp/crepus-native-queue-acceptance-2026-10-18.log`. These compile extracted real queue/state code with labelled platform/FFI stubs and some real filesystem copies; they are not device UI or frame-latency proof, nor a guarantee against arbitrary blocking application code.
- Preserved concurrent work and serialized the primary Cargo lane after the other checkout build ended. No dependencies, commits, pushes, branches, deployment or remote mutations. Full workspace/default-feature gates, WASM/browser execution for this parser change and native device/frame-latency tests remain unverified.

### 2026-10-18 — Reactive browser acceptance and shared Vue directive review

- Parallel read-only shared Vue review found no new confirmed supported-input miscompile in branch guards/order, content replacement, v-show, model restrictions and event/binding validation. Parallel reactive DOM review found no new confirmed defect in live properties, namespaces, text replacement, disposal or hydration decoding. No speculative parser tightening or product change added.
- Current native reactive suite passes 92 tests including doctests; one explicitly manual timing comparison remains ignored. Evidence: `/tmp/crepus-reactive-review-2026-10-18.log`.
- Built current reactive dom_bind WASM test with the coherent installed stable Rust toolchain, then ran all 14 tests in Chromium 151.0.7922.34 and all 14 in Playwright WebKit 26.5. Both pass. Covered edited controls, empty selects, file values, Boolean/XLink attributes, URL recovery, text mutation avoidance, class changes, hydration validation and effect disposal. Evidence: `/tmp/crepus-dom-review-{chromium,webkit}-2026-10-18.log`; build/server log `/tmp/crepus-dom-review-server-2026-10-18.log`.
- Interactive wasm-bindgen runner remains alive by design after tests; inspected its exact DOM-test process/listener, stopped only that owned server, observed the Cargo handle terminate with its expected interrupt exit 101 and confirmed port 8874 no longer has a listener. Browser assertions themselves completed successfully before shutdown; this is not reported as a zero-exit interactive Cargo run.
- This turn changes only the audit ledger; primary inspected the added record and whitespace check passes. No dependencies, commits, pushes, branches, deployment or remote mutations. Playwright WebKit is not installed Safari/iOS validation; Firefox, actual device UI, arbitrary application code and frame-latency guarantees remain unverified.

### 2026-10-18 — Astro quoted content directive literals

- Parallel Angular review confirmed the previously noted attribute-provenance issue is resolved in current source/CLI: false attributes remain present, null omits, false properties remain off, and conflicting class inputs reject explicitly. Evidence: `/tmp/crepus-angular-attribute-current-audit/`. This pass did not implement or take ownership of that concurrent fix. Pre-change full core suite passes 772 tests (`/tmp/crepus-core-frontend-audit-2026-10-18.log`).
- Parallel Astro review identified shared set:text/set:html quoted values incorrectly becoming expression source, unlike the existing authored-mode literal branch. Primary CLI red rendered context-value for set:text="label" with label in context; delegated public-parser regression reached terminal red before production edits.
- Reused the established literal HTML-attribute decoder and JSON string quoting for both shared/authored content directives. Braced expressions remain unchanged. Quoted empty/whitespace values are valid literals; bare directives and empty braces remain errors. Added both-directive literal/entity/empty/whitespace/expression regression, corrected the stale whitespace-negative test/name, and documented the boundary. No new helper/dependency/API needed; production lowering is smaller than before.
- Full core suite passes all 773 tests after the fix. Strict all-feature/all-target core Clippy, focused formatting and whitespace checks pass. Rebuilt actual CLI and verified ten literal/dynamic render cases plus four invalid-directive controls: context cannot override quoted text, HTML entities decode once, raw HTML and whitespace survive, and braced strings retain entity spelling. Evidence/before snapshots: `/tmp/crepus-astro-content-literals/`. Independent actual-diff review found no blocker; primary inspected source/test/docs diffs.
- No commits, pushes, branches, deployment or remote mutations. Acceptance covers parser/AST and actual CLI HTML rendering; browser execution, full workspace/default-feature gates, other emitted framework runtimes and device/frame-latency behavior remain unverified for this change.

### 2026-10-18 — JSX ID literal escape consistency

- Parallel native View IR review found no new confirmed literal/control-state mismatch. Baseline focused native frontend/binding checks pass 32 tests (`/tmp/crepus-native-lowering-audit-2026-10-18.log`). Parallel JSX review identified double-quoted constant ID folding using the indentation-text decoder, while single-quoted IDs and ordinary expression attributes used the expression evaluator. Public CLI red confirmed a real CR versus literal backslash-r difference; core and web regressions both reached terminal red before production edits.
- Constant ID folding now matches eval_literal String values for either quote style, removing the separate scanner/text-decoder path. Existing expression escape semantics are preserved: unknown escapes remain literal, n/t/backslash and matching quotes decode normally. Ordinary quoted ID attributes stay literal; compound expressions remain bindings. Added core AST/escape/compound tests, web ID/title parity across plain and SSR modes, and concise frontend documentation. Production code is smaller; no evaluator expansion or new dependency.
- Full core suite passes 775 tests. Web checks pass 78 tests: 24 ID-render, 33 text-interpolation and 21 hydration tests with hydration explicitly enabled. Initial default-feature command skipped the gated hydration suite; reran with the required feature and observed its actual tests pass. Strict all-feature/all-target core Clippy, focused formatting and repository whitespace checks pass. Independent actual-diff review found no blocker; primary inspected all changed source/test/docs and the complete new core test file.
- Rebuilt actual CLI and verified HTML plus native IR output for six escape cases in both quote styles, ordinary static attribute text and context-dependent compound IDs. Evidence/before snapshots: `/tmp/crepus-jsx-id-literal-fix/`; initial delegated fixture `/tmp/crepus-jsx-id-escape-audit/`. These checks confirm matching ID/title expression values and portable style.id identity without changing the shared evaluator's rules.
- No commits, pushes, branches, deployment or remote mutations. Browser/device execution, generated application lifecycles, full workspace/default-feature gates and frame latency remain unverified for this change.

### 2026-10-18 — HTML attribute carriage-return preservation

- Parallel review mapped static ID paths in plain/SSR, rotating text and legacy hydration to text escaping without CR references, unlike generic bindings. Similar gaps affected handler metadata, embed attributes and SSR document attributes. Primary public CLI/Chromium red demonstrated DOM id becoming a LF while title retained the intended CR. Three of four new public-renderer tests failed before production edits; text-escaping control passed.
- Added a crate-private attribute escaping helper reusing HTML escaping plus CR character references, and routed quoted attribute-value serialization through it. Generic bindings retain their existing CR behavior; ID, handler/embed metadata, class/document and animation attributes now preserve data consistently. Kept ordinary text, textarea-specific handling and document title on their separate paths; URL filtering still precedes escaping. Updated frontend documentation. No dependency or public API addition.
- Full web suite with hydration enabled passes 174 tests including doctest. New tests cover CR/CRLF ordinary IDs and handlers through plain/SSR/legacy hydration, rotating-text IDs through valid supplied AST/plain/SSR, embed source/scope and document language/body classes, with text controls. The initial rotating-text fixture had only one phrase, then used a comment syntax unsupported by JSX; corrected it to the existing valid indentation fixture and supplied AST values, without changing production parsing. Strict all-feature/all-target web Clippy, focused formatting and whitespace checks pass.
- Rebuilt actual CLI. Chromium 151.0.7922.34 and Playwright WebKit 26.5 both retain exact CR and CRLF for input IDs, title, label target and handler keys. Verified label.control association and actual label clicks toggling the corresponding checkbox in all four engine/newline combinations. Evidence/before snapshots: `/tmp/crepus-web-attribute-cr/` (browser.red.log, red.log, web-tests.log, clippy.log, cli-build.log, browser.green.log and emitted HTML). Independent actual-diff review found no blocker; primary inspected the three serializer diffs, complete new tests and docs.
- Parallel framework-emitter review found no additional generator-side CR/ID bypass; emitted JS/template expression paths retain their current contracts. No commits, pushes, branches, deployment or remote mutations. Installed Safari/iOS/device execution, full workspace gates, WASM application runtime and frame latency remain unverified.


### 2026-10-18 — Parallel control review and textarea browser acceptance

- Parallel read-only web, native View IR, Flutter and TUI control reviews found no new confirmed defect within supported contracts. Web textarea handling keeps value bindings out of ordinary attributes, escapes body data once, avoids SSR text markers, preserves CR with references and compensates for HTML initial-LF consumption. TUI documents renderer/template bindings with host-owned input state; absent native control editing is not treated as a supported-behavior regression. No speculative control semantics added.
- Current public CLI output passed six textarea cases each in headless Chromium and Playwright WebKit: initial LF, initial CR, initial CRLF, mixed line endings, HTML-sensitive/Unicode data and empty values. Verified exact textContent/defaultValue preservation, browser-standard value newline normalization, actual editing and restoration from defaultValue. Evidence: /tmp/crepus-textarea-browser-2026-10-18/ emitted HTML and context fixture. Both engines passed all twelve cases.
- Only this audit record changed in this pass; inspected the appended diff and whitespace. No new product change, dependency, commit, push, branch, deployment or remote mutation. Cargo suites were not repeated because product code did not change. Installed Safari/iOS/device execution, full workspace gates and arbitrary application/frame-latency guarantees remain unverified.


### 2026-10-18 — Embedded/LVGL boundary review and browser asset gates

- Previous turn completed textarea browser acceptance; classified as progress through new runtime evidence. Re-read current repository instructions and preserved concurrent work. Parallel read-only embedded geometry/framebuffer/transport and LVGL XML/control/numeric-boundary reviews found no new confirmed supported-input defect. Existing screen clipping, exact buffer validation, native-versus-wire byte handling, XML character/name/whitespace validation and explicit unsupported-control errors remain consistent with their documented contracts. No speculative product edits.
- Observed live external Cargo test PID 59505 for native web-module tests; left it untouched and used the independent JavaScript lane. Current Bun browser event-delegation and WebExtension asset suites pass 175 tests with 422 expectations, zero failures; optional AssemblyScript compiler gate initially skipped because its environment selector was absent. Located the existing disposable compiler installation and explicitly reran that gate with CREPUS_ASC_PATH=/tmp/crepus-as-string-abi/node_modules/assemblyscript/bin/asc.js: one additional test and 44 expectations pass, including actual managed strings, forced collection and long Unicode/NUL data. No installation or dependency change.
- Primary inspected this appended record and whitespace. Product code unchanged; no commits, pushes, branches, deployment or remote mutation. Embedded/LVGL findings are source-review evidence, not newly executed firmware or hardware acceptance. Browser asset tests are local harness checks; installed extensions, device UI and frame-latency guarantees remain unverified.


### 2026-10-18 — LSP/reload review and full runtime gate in progress

- Prior turn completed browser asset/compiler gates and recorded new runtime evidence. Parallel read-only LSP document/diagnostic lifecycle and runtime reload reviews found no new confirmed supported-contract defect. Current LSP advertises FULL sync, rejects stale versions, orders diagnostics with document mutation and clears on close. Runtime reload rebuilds prepared includes and retains stale-result guards for root/context/include publication; failed edits remain recoverable. No product edits from these reviews.
- Revalidated preceding external Cargo process 59505 as absent before starting the primary runtime gate. Offline locked runtime test stopped before compilation because accesskit 0.24.1 was not cached. Retried the same full runtime test with downloads enabled and --locked, one build job, without manifest/lock changes. Dependencies downloaded and compilation is progressing; session 49122 remains live, log /tmp/crepus-runtime-full-audit-2026-10-18.log. This is a verified ongoing build, not a passing test claim. Resume by polling this exact session; do not start a duplicate.
- Primary inspected the audit addition and whitespace. No commits, pushes, branches, deployment or remote mutation. Runtime test result, GPU/device acceptance and latency evidence remain pending.


### 2026-10-18 — Worker failure acceptance and watcher lifetime review

- Resumed exact runtime Cargo session 49122 rather than duplicating it; live poll confirms compilation continues through current locked dependencies. Full runtime gate remains pending, log /tmp/crepus-runtime-full-audit-2026-10-18.log. Previous turn is a verified wait plus completed source review.
- Independent read-only watcher lifetime review found no concrete ownership leak: cancellation/weak-entity failure release worker results, replacement drops old handles, managed watcher teardown signals cleanup without joining the UI thread, failed setup does not orphan monitors. In-flight filesystem work still completes before cleanup; no cancellation guarantee added.
- All 16 current worker client/queue/app tests pass with 61 expectations. Existing actual Chromium worker transport probe passes clone/unprintable failures, FIFO recovery without replay, and initialization failure delivery to subsequent calls. Ran the same probe from a disposable copy using Playwright WebKit instead of Chromium; it also passes. Browser processes/local servers close through existing finally cleanup. Evidence: /tmp/crepus-worker-tests-2026-10-18.log, /tmp/crepus-worker-browser-2026-10-18.log, /tmp/crepus-worker-webkit-2026-10-18.log and disposable WebKit probe. These tests use a controlled JS runtime fixture, not production WASM or frame-latency measurement.
- Product code unchanged; primary inspected this record and whitespace. No commits, pushes, branches, deployment or remote mutations. Runtime suite result, installed Safari/iOS, real device UI and arbitrary blocking application latency remain unverified.


### 2026-10-18 — Deterministic blocked-worker main heartbeat gate

- Previous turn completed real browser worker failure acceptance while runtime compilation continued. Resumed exact runtime session 49122; it remains live and compiling locked dependencies, with no duplicate Cargo lane. Full runtime test result remains pending in /tmp/crepus-runtime-full-audit-2026-10-18.log. Parallel worker-generation review found no new confirmed stale-commit/lost-update defect.
- Extended the existing manual browser transport regression with a blocking worker export and shared-atomic handshake: after worker entry, a main-thread timer writes/notifies the flag, the call resumes and success requires the heartbeat and returned shared value. Local fixture server supplies COOP/COEP headers to enable SharedArrayBuffer. Reuses existing bounded calls and worker/browser/server finally cleanup; no dependencies or production runtime change.
- Updated probe passes in Chromium and, via disposable engine-switched copy, Playwright WebKit. Negative control removing the flag release fails with Main heartbeat did not release worker, demonstrating that successful transport alone cannot satisfy the added gate. Initial fixture incorrectly supplied an array to the variadic client.call API; corrected the fixture to pass the buffer directly after inspecting the actual client signature. Evidence: /tmp/crepus-worker-heartbeat-{chromium,webkit,negative}.log; before snapshot /tmp/crepus-worker-browser.before.py. Independent scoped test review found no blocker; primary inspected the actual diff and repository whitespace passes.
- This verifies main-thread timer progress during a controlled blocked JS worker call; production WASM, DOM rendering cost and general frame/input latency remain unverified. Runtime suite still pending. No commits, pushes, branches, deployment or remote mutations.


### 2026-10-18 — Reproducible Chromium/WebKit worker gate

- Previous turn added and verified the blocked-worker heartbeat regression. Added a standard-library argparse browser selector to that existing manual probe, keeping Chromium default and admitting WebKit explicitly. This removes disposable source rewriting from future cross-engine checks and labels the result with the tested engine. No product API or dependency change.
- Repository script passes with its default Chromium and explicit --browser webkit, covering heartbeat, clone/unprintable error recovery, FIFO/state retention and initialization failure. Invalid browser choice exits 2 before server startup. Primary inspected exact before/current script diff and audit addition; whitespace passes. Evidence: /tmp/crepus-worker-browser-engine-{chromium,webkit}.log and /tmp/crepus-worker-browser-engine.before.py.
- Polled exact existing Cargo session 49122; still live compiling macOS accessibility dependencies. Full runtime gate remains pending in /tmp/crepus-runtime-full-audit-2026-10-18.log, without a restart or duplicate. No commits, pushes, branches, deployment or remote mutations. Controlled JS worker checks do not establish production WASM/device/frame-latency behavior.

- Completion update: exact runtime session 49122 terminated successfully, exit 0. Full ordinary suite passes 94 tests; 11 Metal-dependent units and one manual construction-timing integration remain ignored by default; doctests contain zero tests. Explicitly ran the 11 ignored library tests serially on this host's Metal device: all 11 pass. This covers painted text, clipping/scroll, runtime shadow state and prepared nested includes, beyond source-only review. Logs: /tmp/crepus-runtime-full-audit-2026-10-18.log and /tmp/crepus-runtime-metal-audit-2026-10-18.log. Manual timing integration was not run; no general frame-latency guarantee.


### 2026-10-18 — Slot construction measurement and SSR classification finding

- Previous turn completed full runtime and explicit Metal gates. Ran remaining manual runtime slot construction test explicitly: passes, debug medians 1,755,791 ns at 100 rows, 6,571,584 ns at 400 and 16,179,125 ns at 1,000. This is roughly linear element construction including include lookup, without layout/paint or a timing assertion; the largest debug sample approaches a 60 Hz frame budget and is not a no-lag claim. Evidence: /tmp/crepus-runtime-slot-scaling-2026-10-18.log.
- Parallel slot review found ownership/forwarding consistent but identified adjacent SSR quadratic classification work. Primary confirmed web render.rs element calls classify_node(&Node::Element(el.clone())) for every emitted element; core analysis.rs recursively visits/counts the entire subtree, including after dynamic status is known. A depth-D element chain therefore deep-copies and visits D(D+1)/2 element occurrences for classification alone. This source-derived operation count is a concrete optimization target, not a measured speedup; next step is a deterministic scaling/semantic regression before changing production behavior, preserving descendant-dynamic SSR marker semantics.
- Started strict current all-target runtime Clippy with one job, offline/locked; exact session 89648 remains live compiling dependency metadata. Resume this session, not a duplicate. Log /tmp/crepus-runtime-clippy-2026-10-18.log. Product code unchanged in this pass; primary inspected this record and whitespace. No commits, pushes, branches, deployment or remote mutation.


### 2026-10-18 — SSR allocation regression prepared

- Previous turn identified concrete SSR deep-copy work and completed manual runtime construction measurement. Reconfirmed the only Walker element callsite and core classifier behavior. Delegated only new web/tests/ssr_allocations.rs; primary inspected complete test source. It counts allocator calls thread-locally around public render_nodes_ssr, excluding parsing/setup, compares static/dynamic depth32/64 trees with a generous subquadratic allocation bound, and verifies unchanged static HTML plus descendant-dynamic marker/binding semantics. No dependency added.
- Test is prepared but not executed: exact primary runtime Clippy session 89648 is still live compiling dependency metadata. Polled that session successfully; do not duplicate it. Next required action after it terminates: run new web test to terminal red before production edits, then remove deep subtree cloning and rerun semantic/allocation gates. Repeated read-only classification scans remain a separately identified cost; the allocator regression deliberately does not claim to measure traversal complexity.
- No production code edits, commits, pushes, branches, deployment or remote mutations. Primary inspected audit addition and whitespace. Test outcome and runtime strict lint remain pending; no speedup claimed.


### 2026-10-18 — SSR subtree clone removal verified

- Revalidated exact runtime lint session 89648; it ultimately exits 0, strict all-target runtime Clippy passes (third-party block future-incompatibility notice remains). Before production edits, compiled the allocation regression directly against the existing web renderer and its exact Cargo-fingerprint-matched core dependency, avoiding a second Cargo lane. Initial newest-core guess produced a metadata/type mismatch and no test execution; corrected dependency selection from recorded fingerprints. Terminal red: depth32/64 static SSR allocations 1,251 -> 4,547, violating the <3x bound. Evidence: /tmp/crepus-ssr-allocations-red.log.
- The sole element dispatch now classifies the borrowed original node and passes its marker decision into element serialization. Preserves slot/slot-rotate bypasses, plain-mode short circuit and descendant-dynamic markers; removes constructing/cloning an owned Element subtree solely for classification. No new cache, dependency or public API. Repeated read-only classification traversal remains an explicit separate optimization target.
- Directly rebuilt current web source with exact fingerprint-matched dependencies: allocation regression passes, as do 76 existing SSR/ID/attribute/text tests. Then, after runtime lint terminated, normal Cargo full web suite with hydration passes all 175 tests including doctest and new allocator regression; strict all-feature/all-target web Clippy passes. Focused rustfmt and repository whitespace pass. Independent scoped review found no blocker; primary inspected complete new test and source diff against /tmp/crepus-ssr-render.before.rs. Evidence: /tmp/crepus-ssr-fixed/, /tmp/crepus-ssr-allocations-green.log, /tmp/crepus-ssr-web-tests.log, /tmp/crepus-ssr-web-clippy.log.
- No commits, pushes, branches, deployment or remote mutation. Allocation scaling regression establishes removal of quadratic subtree-copy allocation growth, not elimination of repeated analysis traversal, total render byte complexity or a frame-latency guarantee. No browser/device or full-workspace gate repeated for this internal SSR-only change.


### 2026-10-18 — Linear scoped SSR classification

- Previous turn removed quadratic subtree-copy allocations and passed the corresponding regression. Addressed the remaining repeated classifier scans: core analysis now exposes a borrowed RegionMap populated in one bottom-up traversal, sharing the existing exact classifier/expr-count rules through a monomorphized callback. Non-index analysis keeps its no-map path; indexing additionally visits include-slot and rotating-text children without altering legacy expression counts. Identity keys are never dereferenced and the index lifetime borrows its AST.
- Web SSR builds one index per root or newly loaded ordinary/named include AST and threads it through element/control/loop rendering. SlotFrame captures its caller index; external owned context slots get their own scoped index. Maps drop before temporary include ASTs and never survive a render, avoiding stale pointer-address reuse. Plain/textarea paths do not build indexes. No dependency or persistent cache added.
- Core tests verify parity over elements/branches/loops/matches/include slots/rotating text, foreign-node exclusion, unchanged include expression counts and exactly one callback visit per node at depths32/64. Full core gate passes779 tests. Full web hydration gate passes175 tests, including prior allocation scaling/marker checks and borrowed-slot forwarding; web no-default-features gate also passes. Strict all-feature/all-target core+web Clippy, focused rustfmt and whitespace pass. Transient concurrent Vue warnings seen during the initial build were absent in the final strict gate; no unrelated repair made.
- Primary inspected complete core and web diffs against /tmp/crepus-ssr-region-map-fix/analysis.before.rs and /tmp/crepus-ssr-index-render.before.rs. Independent lifetime/slot/include/plain/textarea integration review found no blocker. Logs: /tmp/crepus-region-core-tests.log, /tmp/crepus-ssr-region-web-tests.log, /tmp/crepus-region-web-minimal.log, /tmp/crepus-region-clippy.log. Source-derived classifier work changes from repeated nested scans to one pass per AST scope plus lookups; total rendering/serialization and repeated include loads have separate costs.
- No commits, pushes, branches, deployment or remote mutations. No end-to-end timing speedup, browser/device frame-latency or full-workspace claim; rendering still incurs output construction, DOM/layout/paint and application work.


### 2026-10-18 — Controlled SSR index cost comparison

- Previous turn completed scoped index integration and relevant full core/web/minimal/lint gates. Parallel adversarial review found no uncovered None-index SSR path, transient component/map lifetime mismatch or error-unwind ownership gap. Plain textarea rendering remains outside indexing. No production edits this pass.
- Compared the immediately preceding clone-free/repeated-scan renderer snapshot against current indexed rendering, compiling both from source with identical dependency artifacts, SSR-only cfg and native debug rustc settings. Initial artifact selection encountered overwritten fingerprint metadata; ran a fresh locked/offline SSR-only Cargo build, then resolved exact matching dependency fingerprints without changing manifests/lock. Parsing/setup excluded, output rendering included. Seven samples of100 renders per case; three process runs with reversed order; report median of process medians. Six static/dynamic nested HTML outputs are byte-identical.
- Static depth16/64/128 medians: 28.9/209.3/664.0 us before, 38.6/153.9/316.3 us indexed. Dynamic equivalents: 65.2/352.0/1037.8 us before, 75.9/295.2/616.5 us indexed. The index improves these deepest cases by2.10x/1.68x but adds about10us at depth16; do not characterize it as a universal speedup. First process had much larger timings; reversed-order repeats and median aggregation limit that observation's influence. No speculative threshold or second renderer path added.
- Artifacts and reproducible source/build outputs: /tmp/crepus-region-measure/ (baseline/indexed source libraries, timing*.csv, summary.json, measure.rs, identical HTML). This is local debug render construction/serialization for supplied nested ASTs, not production release throughput, browser DOM/layout/paint or frame latency. Primary inspected appended evidence and whitespace. No commits, pushes, branches, deployment or remote mutations.


### 2026-10-18 — Empty palette values reject before output replacement

- Previous turn completed controlled SSR index measurement and recorded its shallow/deep tradeoff. Parallel component registry and shared static color lookup reviews found no new confirmed contract defect. Primary generator review found empty oklch() and bare # declarations silently skipped by nonempty-only regular expressions; a valid sibling color allowed partial output to overwrite the existing palette. Both Python and Bun generators share that root cause.
- Added invalid-input regressions retaining a valid white entry and existing-output sentinel. Python regression reached terminal red. Initial Bun suite failed before exercising generator behavior because the existing culori import was unavailable; installed culori4.0.2 (MIT, scripts disabled) in disposable /tmp/crepus-palette-empty-toolchain and supplied NODE_PATH without project dependency/lock changes. Then Bun reached meaningful red:19 pass,2 new regressions fail.
- Changed only the two token-content regex quantifiers in each generator from + to *, allowing empty values to reach existing OKLCH/hex validators before output writing. Python offline generation/conversion check passes, including compilation/execution of generated Rust; Bun passes21 tests/65 expectations. Both scripts produce byte-identical valid local fixture output compared with their before snapshots. Independent scoped review found no blocker; primary inspected actual four-file diffs and whitespace. Evidence: /tmp/crepus-palette-empty-{red,bun-red,python-green,bun-green}.log and generator/test before snapshots; disposable compiler dependency manifest declares MIT.
- No palette regeneration, runtime color/API change, new project dependency, commit, push, branch, deployment or remote mutation. Verification covers local fixture input and existing generator suites; remote upstream theme download and full workspace/device behavior were not exercised.


### 2026-10-18 — Wide reactive measurement and current native gate

- Previous turn fixed palette empty-value output loss and passed both generator suites. Parallel scheduler burst review found no new confirmed duplicate scheduling, disposed-queue or warm-diamond consistency defect. Current queued membership is removed before callbacks, disposal removes pending work, memo processing precedes effects and active observer guards prevent reentry. Primary inspected source/dependency backup flows without speculative changes.
- Explicit wide_dependencies gate including its manual benchmark passes both tests. Native debug medians:1,725,083 ns at1,000 dependencies,8,110,500 ns at4,000 and18,117,208 ns at8,000. The manual case times a signal write plus lazy wide memo recomputation, without DOM/painting or a timing assertion; it is not evidence that large graphs cannot block a UI thread. Existing branch-switch/duplicate-read test also passes. Log /tmp/crepus-reactive-wide-2026-10-18.log.
- Current full native reactive Cargo gate exits0, including compile-fail thread-ownership doctests; the manual timing test is ignored by that default command but was explicitly run above. Log /tmp/crepus-reactive-native-current-2026-10-18.log. No product edits, dependencies, commits, pushes, branches, deployment or remote mutations. Primary inspected appended record and whitespace. Browser/WASM/device/frame-latency checks were not repeated in this pass.


### 2026-10-18 — Current facade target gate

- Previous turn completed wide reactive measurement and current native gate. Parallel facade dispatch/output-path review and primary manifest/context/conversion review found no new confirmed defect: effective generated IDs match selection, ambiguous IDs fail before rendering, output files remain manifest-relative, web appends index.html and preflight rejects equality/ancestor aliases before writes. The historical web template-field precedence question remains outside this change without an explicit supported-field contract.
- Current full facade Cargo gate passes all8 tests, zero failures/ignored; doctests contain no tests. Strict all-target facade Clippy passes, rebuilding/checking its current core/native/LVGL/web dependency paths after recent changes. Logs: /tmp/crepus-facade-current-2026-10-18.log and /tmp/crepus-facade-clippy-current-2026-10-18.log. No speculative product changes.
- Primary inspected this audit addition and whitespace. No commits, pushes, branches, deployment or remote mutation. This is library manifest/render/write acceptance; CLI full-workspace, device and frame-latency behavior were not exercised.


### 2026-10-18 — Refresh stale checked-in WASM parser payloads

- Previous turn completed current facade acceptance/lint. Parallel WASM wrapper/export review found no new wrapper/Rust contract mismatch. Primary actual JS probe showed both checked-in parser payloads were stale: shared Svelte bind:clientWidth still silently accepted and Astro quoted set:text="label" evaluated the context variable, despite current Rust fixes. Added package/legacy regressions; old payload suite reached terminal red68pass2fail.
- Rebuilt through existing build.sh using coherent installed stable Rust/Cargo with RUSTC_WRAPPER empty, matching wasm-bindgen0.2.126. Existing script resolves locked dependency version, builds release wasm32-unknown-unknown, checks all parser frontend fingerprints, regenerates npm/wasm and legacy pkg and runs parser tests. Build exits0; all70 tests/281 expectations pass. Literal test correctly covers set:text; set:html remains a rejection control because View IR does not support raw HTML (initial proposed expectation corrected before acceptance). Independent test/IR review confirmed that contract and exact Svelte diagnostic.
- Primary compared every generated file to scoped snapshots. Only each identical wasm binary changed864,148->939,253 bytes; JS glue and both declaration files are unchanged. Both package directories remain byte-identical file-for-file. New payload SHA256903fad65ecc54df1741b640c00d7cb9077bdc910af00547d18bef0ec6b7bfa42. Complete appended test diff and whitespace inspected. Evidence/snapshots: /tmp/crepus-wasm-refresh/ (red.log, build.log, parser.before.ts, pkg/, npm/wasm/). No Rust parser source edits were needed in this pass.
- No commits, pushes, branches, publication, deployment or remote mutations. Cargo metadata downloaded missing locked cross-platform artifacts; no project dependency/lock change. Verification executes real release WASM under Bun through package and legacy bridges; browser bundling, installed npm consumers, mobile/device/frame latency and full workspace checks remain unverified. Payload grew about75KB with accumulated current-source changes; this is not a binary-size optimization.


### 2026-10-18 — Explicit browser/worker WASM parser entry

- Previous turn refreshed checked-in Node/Bun WASM payloads. Current loader inspection confirms the root package entry synchronously reads wasm through fs and cannot directly run in browsers; added an explicit additive ./browser entry rather than changing root initialization semantics. Shared parser.ts factory retains the existing validation/context/frontend methods and type exports. Browser loadParser() asynchronously initializes and returns that API, caching the same promise and failure; optional served wasm URL supports relocated assets.
- Existing build.sh now additionally emits wasm-bindgen web-target bindings/WASM into npm/browser from the same compiled release module. Package exports/files include browser.ts, parser.ts and browser assets. README documents worker use, synchronous parsing after load, shared failures, asset serving/custom URL and manual browser gate. No new dependency or Rust changes.
- Existing root Node/Bun suite passes all70 tests/281 expectations after the refactor. New manual Python Playwright gate bundles the browser entry with installed Bun and executes actual generated release WASM in module workers on Chromium and WebKit. Both pass six frontend formats, Astro literal fix, unsupported Svelte/Unicode/nonfinite rejection, repeated initialization identity, and missing-WASM failure propagation/caching. Existing finally cleanup terminates workers and closes browser/server. Initial browser probe exposed bundled default-asset relocation; fixed wrapper default URL plus explicit override before acceptance. Corrected one test fixture's doubled surrogate escape to exercise actual invalid UTF16.
- Checked package export targets/assets exist and are included in files; shell syntax and repository whitespace pass. Primary inspected shared factory against before source, new loader/test, generated web bindings, build/package diffs and README. Independent compatibility and actual-diff review found no blocker. Evidence: /tmp/crepus-browser-parser/ (index/build/package before snapshots, build.log, node-tests.log, browser.log, bundled site). No standalone TypeScript type checker was installed; Bun transpilation/bundling and real browser execution passed.
- No commits, pushes, branches, publication, deployment or remote mutations. This adds browser/worker parser availability; host bundlers must serve/include the documented WASM asset or supply its URL. No claim of arbitrary bundler compatibility, automatic worker placement, release application/frame-latency or device validation.


### 2026-10-18 — Browser parser package-boundary/type acceptance

- Previous turn added and verified explicit browser/worker parser entry. Located an existing disposable TypeScript7.0.2 compiler at /tmp/crepus-dynamic-url-fix/node_modules/typescript/bin/tsc; strict noEmit ESNext/bundler-resolution DOM typecheck of both npm/index.ts and npm/browser.ts passes under Bun. No compiler installation or project dependency change. Log /tmp/crepus-browser-parser/typecheck.log.
- Strengthened manual browser gate: stages only manifest.files plus package.json under isolated node_modules/@tschk/crepuscularity-wasm; smoke-tests the synchronous root export from that temporary cwd, bundles the public ./browser subpath from a temporary consumer entry, and serves generated assets only from the staged package. Removes direct checkout-entry bundling and unnecessary external-module flag. This exercises real export resolution and declared asset inclusion, without npm publication or claiming exact npm tarball semantics.
- Final exact probe passes in Chromium and WebKit for all six frontends, initialization identity/failure and validation using actual release WASM in module workers. Independent reviewer confirmed isolated import resolution and suggested staging assets rather than checkout copies; primary had made that correction and reran the final source successfully. Primary inspected actual test diff against /tmp/crepus-browser-parser/browser-test.before.py and audit addition; whitespace passes. Evidence: /tmp/crepus-browser-parser/package-browser.log.
- No production API edit, dependency, commit, push, branch, publication, deployment or remote mutation. Other bundlers/npm-installed consumers, device UI and end-to-end frame latency remain outside these local checks.


### 2026-10-18 — Browser parser asset override and esbuild acceptance

- Previous turn closed strict TypeScript and isolated declared-package-file gates. Located already-installed esbuild0.21.5 in a disposable toolchain and ran the same staged-package consumer probe through its browser ESM bundler using Bun. Actual release WASM works in Chromium and WebKit; no dependency installation or repository bundler configuration added.
- Strengthened checked-in browser probe with a valid relocated asset mode and server-side request recording. Default, relocated and missing-WASM modes each assert exactly one request to their expected resource, alongside shared promise identity/failure checks. This proves the explicit URL argument is used rather than accidentally loading the still-available default. Fixture assets remain copied from the isolated declared package files. Replaced nested path selection with explicit mode/path pairs.
- Final exact Bun fixture and disposable esbuild variant both pass on both engines, including all six frontend formats and validation in workers. Independent reviewer found no request-counter race: recording happens before serving, while assertions happen after worker initialization/rejection. Primary inspected scoped test diff against /tmp/crepus-browser-parser/browser-url-test.before.py and audit addition; whitespace passes. Logs /tmp/crepus-browser-parser/browser-url.log and esbuild-browser.log; esbuild variant source in esbuild.test.py.
- No production API change, dependency, commit, push, branch, publication, deployment or remote mutation. These checks cover Bun and the installed esbuild version, not every bundler or real application/frame-latency behavior.


### 2026-10-18 — Lite compute-worker blocking contract and verification limit

- Previous turn verified browser parser assets with Bun/esbuild on both engines. Parallel Lite worker reply/lifetime review found no new confirmed lost reply/JSON/exception recovery defect. Primary inspected public compute-worker and V8Thread APIs: compute WorkerHandle eval/post_message block on reply channels and WorkerRuntime Drop joins its thread; the GPUI host separately uses awaitable V8Thread replies and off-thread abort/join. Added concise public rustdoc and docs/lite.md statements explaining that compute calls and teardown must stay off the UI thread and can wait indefinitely for nonreturning guest code. No implementation/API change.
- Attempted current real V8 worker_messages gate. Offline attempt lacked locked bindgen0.72.1; enabled downloads without changing dependencies. Compilation then failed before tests with E0514: shared target shlex-e83f7f8e7defec97 was produced by rustup Rust1.98.1 but Homebrew Rust1.98.1 is incompatible despite matching release/hash. Targeted normal shlex rebuild succeeded but did not repair Cargo's other artifact variant; command-local host build debug override also still selected the incompatible artifact. Stopped this diagnostic route rather than deleting shared target artifacts or claiming a test result. No live process remains.
- Relevant current test execution is unverified due mixed compiler metadata; clean coherent target isolation is a possible future verification route, not performed for these documentation-only edits. Logs: /tmp/crepus-lite-worker-current-2026-10-18.log and /tmp/crepus-lite-worker-dependency-rebuild.log. Scoped source/doc snapshots in /tmp/crepus-lite-worker-audit/. Primary inspected both complete doc diffs, focused rustfmt and whitespace pass.
- No dependencies, commits, pushes, branches, deployment, remote mutation or runtime behavior change. Do not infer compute-worker UI responsiveness, V8 runtime acceptance or arbitrary guest cancellation from this source review.


### 2026-10-18 — Flutter standalone decoder depth boundary

- Previous turn documented Lite's synchronous worker contract and recorded unavailable V8 verification. Current Flutter baseline passes548 tests including decoder/rendering/source fuzz and tap sequences; library/test Dart analysis passes. Parallel decoder audit identified a public standalone API mismatch: ViewNode.fromJson raw preflight counted root depth0, while ViewIr.fromJson typed/source checks count root depth1. Document decoding already rejected default-depth9, but standalone decoding violated its documented default depth8 cap.
- Primary confirmed both shared helper callers and public exports. Added public API boundary regression accepting depth8 and rejecting9 for both standalone/document decoding. After correcting an initial test-command cwd, actual regression reaches terminal red: expected UnsupportedNode, got StackNode. Changed shared raw enqueue root depth from0 to1; no other implementation change. Existing positive custom limits, empty children and label-part/node counts remain consistent.
- Full final Flutter suite passes549 tests including fuzz/rendering/tap checks. Dart analysis of lib/test reports no issues; focused formatting and repository whitespace pass. Independent scoped review found no blocker; primary inspected complete one-line production diff and new test. Evidence: /tmp/crepus-flutter-depth-fix/ (view_ir.before.dart, red.log, full-tests.log, analyze.log); baseline /tmp/crepus-flutter-current-2026-10-18.log.
- No dependencies, commits, pushes, branches, deployment or remote mutations. Fix affects oversized standalone model decoding; ordinary widget document entry already rejected that boundary. No physical device/release-mode/frame-latency claim.


### 2026-10-18 — Flutter supported palette parity with current Rust IR

- Previous turn fixed standalone Flutter depth boundary and passed549 tests. Parallel numeric follow-up found no finite-huge round crash: Dart specifies saturation, and existing finite/duration/layout guards remain applicable. Primary/parallel review instead confirmed supported-source palette drift: six Flutter named entries still used Tailwind v3 while shared Rust/native View IR uses v4. README promises source/IR agreement on shared constructs; no legacy-palette contract found. Actual rebuilt Rust WASM probe confirms current text/bg/border RGB hints.
- Added regression over all six supported names and all three source style channels, rendering both source and IR widgets and checking Text plus decorated background/border colors. Test reaches terminal red on old red #ef4444 versus current #fb2c36. Updated only six palette entries (red500/green500/blue500/blue900/amber500/slate500), stale source/roundtrip/widget expectations, and concise documentation. Semantic renderer danger/success/warning values, arbitrary literal IR colors and custom muted/white/black/transparent entries remain intact.
- First full run exposed an additional stale blue900 rendered-background expectation; corrected that test and strengthened new regression's rendered decoration checks. Final full suite passes550 tests including fuzz/rendering/taps; Dart lib/test analysis has no issues. Focused formatting and whitespace pass. Independent scoped review found no blocker; primary inspected every affected before/current diff and full new widget test. All18 expected RGB channels also match actual current Rust WASM-generated hints (/tmp/crepus-flutter-palette-fix/current-rust-wasm-colors.json).
- Evidence/snapshots: /tmp/crepus-flutter-palette-fix/ (red.log, full-tests.log, analyze.log, per-file before snapshots). No dependencies, commits, pushes, branches, deployment or remote mutations. This aligns the existing supported named subset, not full palette/frontend support. Rendered Flutter test frames/styles verified; physical devices, release performance and frame latency remain unverified.


### 2026-10-18 — Automatic Flutter/shared palette drift gate

- Previous turn aligned Flutter's six named palette entries and passed550 tests. Parallel cross-target audit found no remaining old Tailwind hex table in native/embedded adapters; their resolution uses the shared palette. TUI's family/shade mapping intentionally uses ANSI approximations, not exact RGB parity. No speculative adapter changes.
- Extended the existing Python palette CI gate to extract the generated Rust table and only Flutter's curated _palette block, compare overlapping names numerically and expand short Rust hex forms. A red500 anchor prevents an empty-match false pass; Flutter-only semantic names are not compared. This detects future shared-palette updates that leave supported Flutter colors stale. Uses existing tooling with no Flutter SDK/network dependency. Explicit UTF8 reads address the reviewer's Windows-locale portability concern.
- Final offline generation/parity gate passes, including its existing generated-Rust compile/run. Negative control mocks only the Dart source read to restore old red and confirms Flutter palette drift: red-500, without editing production files. Direct native rustc compile/run of the no-std shared color crate's source tests passes all3 tests, bypassing unrelated Cargo metadata conflicts. Evidence: /tmp/crepus-flutter-palette-fix/parity-gate.log, shared-colors-tests.log and palette-gate.before.py. Independent scoped review found no blocker after UTF8 correction; primary inspected exact script and audit diffs, whitespace passes.
- No dependencies, commits, pushes, branches, deployment or remote mutations. Windows CI was not executed locally; TUI ANSI output is deliberately not RGB-identical. No device/release/frame-latency claim.


### 2026-10-18 — Flutter empty-action control state

- Parallel scoped review confirmed buttons, switches and checkboxes supplied native callbacks for empty action strings while dispatch discarded those strings. Links and gesture wrappers already treat empty actions as absent. Changed only the three native callback conditions to require a nonempty action; whitespace remains an enabled action and forwards unchanged. Binding state and independent long-press handlers remain intact.
- Added 12 widget cases covering null, empty, whitespace and ordinary actions across all three controls, checking callback availability and actual tap dispatch. Regression reached terminal red only for the three empty-action cases. Final full Flutter suite passes562 tests, including keyboard/focus and fuzz/rendering coverage; Dart analysis of lib/test reports no issues. Independent review found no blocker; primary inspected complete production diff and new test, and repository whitespace passes. Evidence: /tmp/crepus-flutter-empty-actions/ (renderer.before.dart, red.log, full-tests.log, analyze.log).
- No dependencies, commits, pushes, branches, deployment or remote mutations. Native disabled callbacks use Flutter standard control behavior; physical-device accessibility and release frame latency were not verified.


### 2026-10-18 — Nim identical-signature identifier collisions

- Previous turn fixed Flutter empty-action controls. Parallel current-source reviews covered native adapters, TUI/embedded/LVGL and CLI/plugin bindings. Native/terminal reviewers found no new confirmed issue in their bounded passes; documented unbound-input/interval fallback behavior was not promoted as a bug. Existing-binary LSP stdio protocol probe and current-source offline palette/Flutter parity gate pass; LSP was not freshly rebuilt.
- Plugin reviewer identified Nim style-insensitive function names. Installed Nim reproduces redefinition for crepus_extra/crepusExtra with identical parameters, while first-letter case and distinct parameter-type overloads are valid. Added selected-header no-output-on-rejection regression; actual terminal red confirms generator accepted the invalid pair. Generator now tracks normalized names plus emitted parameter type lists and rejects this collision before writing the binding file. Return types do not disambiguate. Preserves valid tested int32/cstring overloads and first-character case.
- New overload regression exercises actual Nim checking and C compilation. Corrected its fixture to include the existing compile helper's expected extra function. Final isolated coherent Cargo target avoids shared mixed-toolchain artifacts: complete plugin-bindgen suite passes33 tests, with4 explicit real-Rust-ABI tests ignored; strict all-target Clippy, focused rustfmt and repository whitespace pass. Independent scoped review found no blocker; primary inspected complete before/current implementation and test diffs. Evidence: /tmp/crepus-nim-name-audit/ (snapshots, red.log, all-tests.log, clippy.log); reviewer probes /tmp/crepus-nim-name-equivalence/.
- Remaining: Nim parameter-local normalized-name collisions and collisions across differently spelled equivalent Nim type aliases require separate coverage. This fix covers identical emitted signatures, not universal identifier/overload validation. No fresh real Rust ABI integration, full-workspace/platform/device or frame-latency claim. No dependencies, commits, pushes, branches, deployments or remote mutations.


### 2026-10-18 — Nim parameter namespace validation

- Previous turn fixed identical emitted function-signature collisions. Rechecked current generator and added selected-header regression for some_value/someValue parameters in one function. Actual terminal red confirms invalid parameters were accepted. Added a per-function normalized-name set using existing identifier_key; rejection happens before writing bindings and names in different functions remain independent. No argument renaming or ordinary generated API changes.
- Added positive Value/value parameter fixture; actual Nim semantic checking and C compilation verify first-character case remains distinct. Complete isolated plugin-bindgen suite passes35 tests, with4 explicit real-Rust-ABI integrations ignored. Strict all-target Clippy, focused rustfmt and repository whitespace pass. Primary inspected complete source and new-test diffs. Evidence: /tmp/crepus-nim-name-audit/ (params-*.before.rs, params-red.log, params-tests.log, params-clippy.log).
- Parallel installed-Nim probes confirmed the next independent gap: equivalent emitted aliases cint/int32, cuint/uint32, cshort/int16, clonglong/int64 and ptr cint/ptr int32 collide despite distinct signature strings. clong/int64 remains a valid overload on this host; normalizing by width would be incorrect. Probe evidence /tmp/crepus-nim-alias-equivalence/. Alias identity remains pending; no universal collision-validation claim. No fresh real Rust ABI/full-workspace/device check, dependencies, commits, pushes, branches, deployment or remote mutations.


### 2026-10-18 — Nim fixed type-alias collision identity

- Previous turn fixed parameter-local name collisions and identified emitted-type alias gaps. Rechecked current generator plus installed Nim2.2.10 system/ctypes.nim definitions: fixed signed integer aliases, unsigned short/int/long-long aliases, and cchar/cuchar char identity. clong/culong are target-dependent and cannot be collapsed by width. Parallel read-only reviewer independently confirmed source and compiler evidence.
- Added selected-header alias rejection regression; terminal red shows int/int32_t accepted previously. Collision signature keys now canonicalize only known fixed Nim aliases, recursively through ptr levels, without changing emitted declarations or ABI type spellings. Ten scalar/pointer alias pairs reject before binding file output. Positive fixture actually Nim-checks and C-compiles distinct long/int64 and unsigned-char/uint8 overloads. Explicit shortcut comment records target-dependent clong/culong limitation.
- Complete isolated plugin-bindgen suite passes37 tests, with4 explicit real-Rust-ABI tests ignored. Strict all-target Clippy, focused rustfmt and repository whitespace pass. Independent final review found no blocker; primary inspected complete scoped implementation/test diffs and audit addition. Evidence: /tmp/crepus-nim-name-audit/ (alias-nim.before.rs, alias-tests.before.rs, alias-red.log, alias-tests.log, alias-clippy.log); local Nim source /opt/homebrew/Cellar/nim/2.2.10/nim/lib/system/ctypes.nim.
- Remaining: target-dependent long aliases require target metadata, especially Windows; no universal overload validation claim. Four real Rust ABI integrations, full workspace and cross-platform execution not rerun. No dependencies, commits, pushes, branches, deployment or remote mutations.


### 2026-10-18 — Fresh real ABI acceptance for generated bindings

- Previous turn normalized fixed Nim aliases and passed37 ordinary tests. Inspected current BindgenOptions/manifest: no target metadata exists, so no host-based clong/culong normalization was added. A portable target-aware contract remains pending; arbitrary target behavior must not be inferred from this Mac.
- Corrected prior shorthand: the4 ignored tests consist of3 actual Rust ABI integrations (Nim, D, Odin) plus1 V compiler/spaced-path gate, not4 real-ABI tests. Fresh offline locked crepuscularity-abi build succeeds in the coherent isolated target /tmp/crepus-nim-name-audit/target, without touching the mixed shared target. Set CREPUS_ABI_LIB to its newly built debug dylib and explicitly ran all ignored plugin-bindgen tests: all4 pass,0 ignored. Nim/D/Odin each exercise all13 real exports; V compiles generated declarations with spaced header/library directories against its own fixture.
- Evidence: /tmp/crepus-nim-name-audit/current-abi-build.log and current-abi-tests.log; dylib SHA256 b70ac4a37d47c3902b6fc88368ca97e5ffcfe431d028bee4e9c2c63946da83cb. This closes the local ignored-test gap alongside preceding37 ordinary tests. No production source changes this turn; audit diff and repository whitespace inspected. Cross-platform/device/full-workspace behavior and UI frame latency remain unverified. No dependencies, commits, pushes, branches, deployment or remote mutations.
- Parallel adjacent C# review reproduced CS0542 for selected-header function EquilibriumImports, which collides with the generated enclosing class even with @ escaping. Compiler fixture/log: /tmp/crepus-csharp-class-name-collision/. No source fix yet; this is the next concrete bounded binding-generator issue.


### 2026-10-18 — C# generated class and enum lookup collisions

- Previous turn closed fresh ABI integration gates and found selected-header EquilibriumImports collision. Rechecked actual C# generator/callers; extended existing no-output rejection fixture. Terminal red confirms invalid enclosing-class member was accepted. Generator now explicitly rejects exact EquilibriumImports function name before writing bindings, preserving other identifiers.
- Parallel compiler probe confirmed another collision: valid exported function CallingConvention shadows unqualified enum expression in DllImport attributes (CS0119). Added this function to existing selected-header compiler fixture (16 declarations); real regression reaches terminal red with that compiler error. Fully qualified global::System.Runtime.InteropServices.CallingConvention.Cdecl resolves lookup while retaining valid export and ABI spelling. Enclosing-class collision cannot be solved by @ escaping and remains an explicit unsupported-name diagnostic.
- Final full plugin-bindgen suite with --include-ignored passes41 tests,0 ignored, including actual C# compilation,3 real Rust ABI integrations against the freshly built current dylib and V spaced-path compilation. Strict all-target Clippy, focused rustfmt and repository whitespace pass. Independent final read-only review found no blocker. Primary inspected complete scoped source/test diffs and audit addition. Evidence /tmp/crepus-csharp-class-name-fix/ (before snapshots, red.log, enum-red.log, tests.log, clippy.log, source.sha256); independent compiler probes /tmp/crepus-csharp-enum-shadow/.
- Target-dependent Nim long identity remains pending; no full-workspace, Windows/device or UI latency claim. No dependencies, commits, pushes, branches, deployments or remote mutations.


### 2026-10-18 — Linear reactive queue cancellation during bulk teardown

- Previous turn fixed C# binding names. Parallel responsiveness review identified remove_node retaining each whole pending FIFO per disposed observer; queued subtree teardown therefore scanned K(K+1)/2 entries. Primary checked all queue consumers, monotonic normal-lifetime IDs and membership ownership. Runtime hot-reload preparation remains background-owned; no speculative rewrite. Separate Flutter review identified eager collection comparison pair allocation as a pending measured-work candidate, not fixed here.
- Established matched release benchmark before source edit: disposal plus final drain,4000 effects median8.505291ms and8000 median28.499833ms,3 warmups/7 samples. Added internal bulk256 memos/effects regression requiring deferred physical FIFO cleanup; actual terminal red confirms old implementation eagerly rebuilds queues. remove_node now removes membership only; flush skips canceled IDs until live work or empty queue, removing live membership before callback. This removes repeated FIFO scans while preserving memo priority and surviving FIFO order. Disposed graph payloads still release immediately; numeric IDs/capacity remain until flush.
- Updated existing FIFO physical-queue expectation; new tests cover live memo/effect behind canceled jobs and cancellation of a later effect by an earlier flushing callback. Final native suite passes94 tests,0 failures,2 manual measurements ignored; explicit new release benchmark passes separately. After medians1.198166ms/3.024792ms (about7.1x/9.4x faster for this fixture). Strict all-target/all-feature Clippy, focused rustfmt and repository whitespace pass. Independent final review found no blocker; primary inspected complete source/test/journal diffs. Durable bottleneck note appended to .jules/bolt.md.
- Evidence /tmp/crepus-queued-disposal/ (before snapshots, before.log, red.log, tests.log, after.log, clippy.log, source.sha256). Measurements are local native release teardown, not browser/device/end-to-end frame latency or a no-lag guarantee. Existing other manual wide-dependency measurement not rerun; no full workspace/browser gate. No dependencies, commits, pushes, branches, deployment or remote mutations.


### 2026-10-18 — Flutter lazy structural comparison value pairs

- Previous turn removed repeated reactive disposal queue scans. Rechecked current Flutter comparison flow and existing scalar/cycle contracts. Confirmed eager width-sized pair stack reads every list/map value before scalar comparison. Added counted host ListBase/MapBase rendered-condition fixtures; corrected initial test-helper values getter collision before actual terminal red. Both real regressions observe10000 reads per side versus expected1.
- Replaced eager pair stack with lazy child-iterator frames. Lists keep reverse traversal; maps validate all keys before lazy value pairing, preserving absent-key/null distinction. Identity-pair cycle tracking, scalar coercion and iterative traversal remain intact. Pending pair storage follows depth instead of width; seen-container storage still follows visited identity pairs. Map value visitation changes from reverse to insertion order; ordinary JSON structural outcomes are unchanged, arbitrary side-effecting/mutating custom maps are outside this verification.
- Both unequal fixtures now perform1 value read per side. Equal1000-entry list/map checks visit all values and preserve numeric/string coercion;4096-level nested list comparison verifies nonrecursive traversal. Full Flutter suite passes566 tests including existing cyclic/structural/scalar/rendering/fuzz checks. Dart lib/test analysis reports no issues; focused formatting and repository whitespace pass. Independent final review found no blocker for JSON-like data. Primary inspected complete production diff/new test and journal/audit additions.
- Evidence /tmp/crepus-flutter-lazy-comparison/ (renderer.before.dart, red.log, focused.log, tests.log, analyze.log). Measured value-access counts, not direct heap profiling or wall-clock/frame latency. Maps still requireO(N) key validation. Durable lesson appended to .jules/bolt.md. No device/release/browser/full-workspace execution, dependencies, commits, pushes, branches, deployment or remote mutations.


### 2026-10-18 — Current WASM queued DOM disposal acceptance

- Previous turn made Flutter comparison value traversal lazy. Parallel current-source core/frontend and native target passes found no new confirmed bug in their bounded scopes. Closed a concrete recent scheduler verification gap instead: native disposal checks alone did not prove actual browser DOM behavior.
- Added browser WASM regression constructing64 text bindings through a shared memo, updating inside a batch, explicitly reading the memo to queue all DOM effects, then disposing63 before flushing. Connected canceled nodes retain initial text; live binding updates, then stops after its own disposal. Memo and DOM fixture cleanup are explicit. Independent read-only review confirmed this truly queues effects before cancellation rather than merely unlinking unqueued subscribers.
- Fresh coherent rustup-stable isolated wasm32 build succeeds; actual wasm-bindgen-test-runner module executes all15 current DOM tests in both Chromium and WebKit with0 failures/ignored, including queued disposal and existing live control/attribute/namespace/hydration cases. Corrected a disposable harness expected-count14 to authoritative15 after first Chromium success; final exact harness passes both engines and terminates its owned server. Target-specific strict WASM Clippy for dom_bind, focused rustfmt and repository whitespace pass. Primary inspected complete new test and audit diffs.
- Evidence /tmp/crepus-reactive-browser-current/ (tests.before.rs, build.log, browser.py, browser.log, chromium.log, webkit.log, clippy.log, source.sha256). No production runtime edit or latency measurement this turn. Mobile/device, end-to-end UI frame latency, full workspace and target-dependent Nim long metadata remain unverified/pending. No dependencies, commits, pushes, branches, deployment or remote mutations.


### 2026-10-18 — Idempotent safe DOM attribute updates

- Previous turn added browser queued-disposal acceptance. Parallel core parser scan found no new confirmed defect; DOM review identified unconditional bind_attr writes for unchanged safe values. Independent Chromium platform probe shows identical iframe srcdoc assignment reloads document and loses edited input state (/tmp/crepus-reactive-attr-repeat-audit/). Primary traced current safe/unsafe/xlink paths.
- Added actual WASM MutationObserver regression for title, xlink:href and srcdoc attributes. Chromium terminal red records1000 mutations for1000 unchanged effect reruns for each. Changed-value and external-edit recovery checks each require1 mutation, disposal0. Safe writes now compare live current DOM value before setting; xlink uses namespace/local-name lookup matching its setter. Unsafe-value removal remains unconditional. No authored-value cache, so external edits still recover.
- Fresh current WASM suite passes16 tests in both Chromium and WebKit, including zero unchanged mutations for all3 fixtures and existing unsafe removal/rendered xlink/control/hydration checks. Native suite passes94 tests with2 manual measurements ignored. Target-specific strict WASM Clippy, focused rustfmt and repository whitespace pass. Independent final review found no blocker; primary inspected complete production/test/journal/audit diffs. Durable lesson appended to .jules/bolt.md.
- Evidence /tmp/crepus-reactive-stable-attr/ (snapshots, red-build.log, red-browser.log, red-chromium.log, build.log, browser.py, browser.log, chromium.log, webkit.log, native-tests.log, clippy.log). Mutation counts prove eliminated writes; iframe reset evidence is a separate direct-browser probe, not the WASM fixture (which uses ordinary elements). No end-to-end latency/device/full-workspace claim. No dependencies, commits, pushes, branches, deployments or remote mutations.


### 2026-10-18 — Idempotent reactive class presence

- Previous turn eliminated unchanged safe attribute writes. Current bind_class still called DOMTokenList add/remove unconditionally; parallel review of other DOM bindings found no new confirmed issue. Added actual WASM MutationObserver regression for both present and absent desired class states, external opposite edits, unrelated-class preservation and disposal.
- Corrected initial test-helper Signal.update signature before authoritative execution; an initial shell continued to a stale16-test module, which was rejected by the17-test harness and is not acceptance evidence. Rebuilt with fail-fast sequencing; actual Chromium terminal red measures1000 class mutations for1000 unchanged reruns in each state. Added live contains-versus-desired check before add/remove, preserving authored callbacks and external repair without caching values.
- Fresh WASM suite passes17 tests in Chromium and WebKit,0 failures/ignored; both stable states now emit0 mutations, each external repair emits1, disposal0. Existing state-transition and unrelated binding checks pass. Target-specific strict WASM Clippy, focused rustfmt and repository whitespace pass. Independent read-only review found no blocker; primary inspected complete production/test/journal/audit diffs. Evidence /tmp/crepus-reactive-stable-class/ (before snapshots, red-build.log, red-browser.log, red-chromium.log, build.log, browser.py, browser.log, chromium.log, webkit.log, clippy.log).
- Durable class mutation lesson appended to .jules/bolt.md. No native suite rerun for this browser-only branch/test change, full workspace/device/frame-latency claim, dependencies, commits, pushes, branches, deployment or remote mutations.


### 2026-10-18 — Fresh core/native/ABI default-feature checkpoint

- Previous turn eliminated unchanged class mutations. Rechecked current manifests/state and ran fresh offline locked tests for crepuscularity-core, crepuscularity-native and crepuscularity-abi in the coherent isolated /tmp/crepus-nim-name-audit/target, preserving shared mixed-toolchain artifacts and unrelated running work. Same live process was polled through compilation and execution; no restarts on observation timeouts.
- Full selected default-feature suites pass1486 tests,0 failed,0 ignored, including core parsing, native conversion/direct-emitter regressions and ABI session tests. Logs /tmp/crepus-core-native-current/tests.log. Cargo exit0 is authoritative completion; intermediate quiet compilation and partial green output were not treated as success. Authored source-module compiler integration files are feature-disabled here (some execute0 tests); this result does not verify web-modules feature behavior.
- Parallel read-only ABI lifecycle/patch/dispatch audit found no new confirmed ownership/atomicity bug; numeric/string native/embedded/LVGL review found current target policies explicit, not a new inconsistency to repair. No speculative changes. Current default gate is fresh evidence against accumulated local changes, not full-workspace/platform/device/latency proof. Primary inspected audit addition and repository whitespace; no production source edit this turn.
- Next concrete verification gap: feature-enabled native web-module compiler tests. Existing Nim target-dependent long identity remains pending. No dependencies, commits, pushes, branches, deployment or remote mutations.


### 2026-10-18 — Web-module feature gate and keyed-each assertion repair

- Previous turn passed1486 default core/native/ABI tests. Ran fresh offline locked native --features web-modules in coherent isolated target. This gate executes compiler/admission/source/AST/Oxc checks, not emitted JS; parallel review confirmed no Bun/Node/browser processes inside native tests. Existing temporary runtime harnesses need fresh emitted inputs before reuse and were not claimed as current acceptance.
- Initial feature run failed keyed_each_callback_keeps_its_raw_item_shorthand. Captured actual DOM/SSR output with temporary diagnostic then removed it. Current compiler deliberately retains SSR key callback under false && for authored typechecking, so old expectation of no raw shorthand in SSR was stale. Updated only regression: one raw shorthand in both targets; AST visitor now additionally requires SSR shorthand under literal-false && and DOM shorthand outside it. Preserved computed-read and zero unsafe __proto__ setter checks. Focused2 tests and strict feature-enabled Clippy for that target pass; independent review found no blocker. No production compiler change.
- Full --no-fail-fast feature rerun completed with1722 passing,5 failing tests,0 ignored. Remaining failures: svelte_ordered_fragments namespace_admission_is_authored_dom_only_and_keeps_executable_guards; svelte_source_modules authored_bare_value_exceptions_are_limited_to_textarea_select_and_option and value_bindings_reject_invalid_targets_and_unsupported_controls; svelte_ssr_source_intrinsics generic_jsx_and_unknown_svg_guards_remain_unchanged; vue_text_model unsupported_elements_and_input_types_reject. Full feature gate remains FAILED, not acceptance. These include newly admitted SVG namespace/date/select behavior and changed emitted source shape; each needs contract-specific classification before editing assertions.
- Namespace review confirms script remains explicitly rejected, while witnessed SVG iframe creates a foreign-namespace element; broader authored HTML witness admission needs contract/runtime confirmation before weakening executable-guard coverage. No namespace test edit this turn. Logs/snapshots /tmp/crepus-web-modules-current/ (tests.log, each-proto.before.rs, each-proto-red.log, each-proto-tests.log, clippy.log). Primary inspected complete scoped test/audit diffs; focused rustfmt and repository whitespace pass.
- Next concrete work is resolving the5 feature-gate failures, then current emitted-module runtime acceptance. No universal framework compatibility/device/latency claim, dependencies, commits, pushes, branches, deployment or remote mutations.


### 2026-10-18 — Reconcile current source-module feature contracts

- Previous turn repaired keyed-each SSR shape check and exposed5 remaining feature failures. Parallel read-only classification against current compiler/docs found stale admission boundaries and one helper-observation gap, not a demonstrated production defect. Snapshotted all4 affected test files plus frontend documentation before scoped edits.
- Svelte bare-option value assertion now recognizes the actual imported setOptionValue helper through resolved semantic symbols, reusing generalized import lookup; existing ordinary attribute and textarea checks remain. Replaced outdated date bind:value rejection with still-unsupported file bind:value, adding DOM/SSR compile/parse positives for date, datetime-local, month, week, time and color. Vue text-only negatives no longer reject supported select/checkbox/radio models; dedicated select/checked suites run alongside retained unsupported-type/target negatives.
- Namespace tests now positively assert authored SVG iframe uses SVG URI/tag construction and SSR circle/path use source intrinsics, while preserving explicit script, generic-JSX SVG and SVG raw-HTML rejection. Independent direct Chromium/WebKit construction probes confirm SVG iframe lacks contentWindow while HTML iframe has it; this is platform namespace evidence, not execution of emitted modules. Clarified raw-HTML docs to known SVG restriction only; review caught that MathML rejection was not established, so no unsupported MathML claim was added.
- Focused6-target suite passes, followed by complete fresh native --features web-modules --no-fail-fast:1728 tests pass,0 failed/ignored. Strict feature-enabled Clippy for affected4 targets, focused rustfmt and repository whitespace pass. Independent review found no test blocker; primary inspected complete actual test/doc/audit diffs. Evidence /tmp/crepus-feature-contracts/ (before snapshots, focused.log, full.log, clippy.log, svg-iframe.py, svg-iframe.log).
- No production compiler change. These gates validate compilation/admission/source/AST behavior, not complete emitted JS execution. Next verification is freshly emitted module runtime acceptance. No device/end-to-end latency claim, dependencies, commits, pushes, branches, deployment or remote mutations.


### 2026-10-18 — Fresh emitted receiver-runtime acceptance

- Previous turn reconciled stale feature assertions and passed1728 compiler tests. Inspected existing receiver acceptance harness and found it referenced possibly stale shared CLI output. Built current native compiler library with web-modules in coherent isolated target, selected its actual rlib from Cargo JSON artifacts, linked a disposable Rust compilation driver and regenerated DOM/SSR modules from the14-case Comprehensive.svelte fixture. An early artifact lookup while the build handle was still live returned no artifact; it did not produce acceptance evidence. Waited for terminal successful build and regenerated from authoritative output.
- Parallel harness review found absent-DOM-node and partial-SSR assertion gaps. New disposable acceptance requires exact14-ID sets and correct values for every DOM and SSR case, unconditionally. Covers state/prop/const/snippet ordinary, optional and tagged-call TypeScript instantiation receivers, cast control and preserved object-method receiver. Also checks mount/unmount removes fixture elements. No claims about reactive callback replacement or null optional short-circuit.
- Fresh unminified Bun execution passes; minified Bun bundle passes; minified actual browser bundle passes in Chromium and WebKit, each checking all14 DOM+SSR values and unmount cleanup. Runtime packages resolve to installed local Moonshine checkout sources, not published-package acceptance. Recorded package manifests/version paths in runtime-provenance.json. Independent final read-only review found no blocker for this initial-execution scope.
- Evidence /tmp/crepus-fresh-module-runtime/ (build.log, artifacts.jsonl, compile.rs, Comprehensive.svelte/dom.ts/ssr.ts, assertions.ts, runtime.ts/log, bundle.ts/log, minified.log, browser.ts/py/log, chromium.json, webkit.json, artifact.sha256). Primary inspected disposable harness and exact audit addition; repository whitespace passes. No production source or dependencies changed. CLI graph/build integration, reactive replacement, other fixtures/frameworks, devices and end-to-end frame latency remain separate gates. No commits, pushes, branches, deployments or remote mutations.


### 2026-10-18 — Reactive optional-call receiver and listener lifecycle acceptance

- Previous turn verified14 initial emitted DOM/SSR receiver cases. Revalidated current native web-module compiler library with offline locked Cargo JSON artifact gate, linked a fresh disposable driver, and generated new Probe DOM/SSR modules. Fixture uses typed nullable raw callback state and an event-triggered `(callback<number>)?.(++calls)` probe, avoiding render-time counter writes. Named snapshot export exposes model state for independent DOM/model assertions.
- Actual sequence asserts idle/0, true/1, clear-to-null then skipped/1, replace callback then true/2. Both callbacks require this===undefined, so accidental cell receivers fail. Unmount is awaited; a retained detached probe button is clicked afterward and must leave snapshot true/2, proving listener cleanup in this path. DOM output and snapshots are checked after runtime tick. SSR separately checks initial idle/0 only, not interaction semantics.
- Fresh unminified Bun and minified Bun executions pass; minified browser bundle passes exact sequence in Chromium and WebKit without page errors, with no retained fixture elements. Existing installed local Moonshine checkout runtime is used, not published-package acceptance. Independent final read-only review found no bounded assertion gap. Evidence /tmp/crepus-reactive-receiver-runtime/ (build.log, artifacts.jsonl, Probe.svelte/dom.ts/ssr.ts, check.ts, runtime.ts/log, bundle.ts/log, minified.log, browser.ts/py/log, chromium.json, webkit.json, artifact.sha256).
- No production source or dependency changes this turn. Primary inspected disposable source/harness and exact audit addition; repository whitespace passes. General optional-chain semantics, other ownership paths, CLI graph integration, device behavior and frame latency remain separate verification. No commits, pushes, branches, deployments or remote mutations.

### CLI static-output overlap audit (2026-10-18)

Parallel CLI and runtime-error-recovery reviews produced one confirmed CLI data-loss issue; the runtime review found no additional actionable defect. Baseline static self-copy truncated an authored asset to empty. A public CLI fixture targeting its static source overwrote authored app.js and grew output nesting on consecutive builds.

Added an early resolved-path output guard before WASM runtime setup or output writes. Static-source output and symlink aliases fail explicitly. The copier treats identical roots as a no-op and rejects other parent/child overlaps before mutation. Normal dist and in-place site output remain admitted. Independent review found no blockers.

Verification: 18 build unit tests passed, including new red-before-fix overlap/self-copy cases. CLI integration suite: 30 passed, two real-WASM site builds failed with E0463 because the selected native compiler has no wasm32 core/std target. These failures do not exercise the repaired overlap path. Fresh public CLI probes rejected both consecutive overlapping builds, preserved authored bytes, and created no nested outputs. Evidence: /tmp/crepus-static-overlap/{red.log,copy-red.log,baseline-repro.json,unit-tests.log,cli-tests.log,after-repro.json}.

Scope limits: root overlap protection does not establish arbitrary nested source-symlink graph safety or transactional build output. No commits or pushes performed; unrelated checkout changes preserved.

Strict thin-CLI Clippy passed for the binary and web_cli integration target with -D warnings; repository whitespace checks passed. Clippy evidence: /tmp/crepus-static-overlap/clippy.log.

### Static source aliases and native output names (2026-10-18)

Previous goal turn classified as progress: root-overlap static copying was repaired with red/green evidence. Continued parallel reviews identified two additional bounded output-safety defects. OMP lanes remain unavailable as previously established; existing native review agents were reused without external provider access.

- Static copying: disjoint roots did not prevent a source file/directory symlink from resolving into the destination. New regression failed against baseline (copy unexpectedly succeeded). Each collected source entry is now canonicalized and rejected if it resolves within the destination tree before destination creation/copying. File and directory alias regressions preserve both affected and unrelated sentinel assets; a positive external-directory-alias test preserves supported copying. All 20 build unit tests pass. Independent final review found no blockers. Evidence: /tmp/crepus-static-overlap/{build.alias-before.rs,alias-red.log,alias-green.log}. Hard-link aliases and concurrent filesystem changes are not covered by this path-based guarantee.
- Native codegen: raw --view-name ../Victim escaped --out and could overwrite a sibling source file. Public CLI regression failed on baseline accepting that name. Codegen now requires a single normal filename, rejecting separators and Windows drive prefixes before reading templates or creating output. SwiftUI and Compose tests cover traversal, absolute/nested paths, Windows forms, dot/dotdot/empty names, preserved victim bytes and no invalid-output directory; positive filenames with spaces/dashes retain exact output paths. Full native_ir_cli suite passes all 35 tests. Independent final review found no blockers. Evidence: /tmp/crepus-native-name-audit/{ir.before.rs,tests.before.rs,red.log,green.log,clippy.log}. Native device compilation and every platform-specific reserved filename are not established by these CLI tests.

Primary inspected both actual before/current source and test diffs. Strict thin-CLI Clippy for the binary and native integration target passed with -D warnings; affected-file rustfmt and repository whitespace passed. Earlier real-WASM toolchain failures were not rerun; these changes do not claim transactional builds, device acceptance, or lag-free UI. No commits, pushes, branches, deployments, destructive cleanup or remote mutations. Ongoing goal remains active.

### Native generated-file links and WASM acceptance (2026-10-18)

Previous goal turn was progress: source-alias and output-name guards changed authoritative code with red/green tests. Parallel continuation reviewed native destination writes and reactive DOM cleanup; no new DOM cleanup defect was found (binding effects retain IDs/callbacks and require explicit disposal, not automatic element-removal disposal). No new runtime checks were claimed from that read-only review.

Native codegen still followed existing generated-file symlinks outside its selected output directory. A public CLI regression failed against baseline accepting an existing SwiftUI destination link. Codegen now canonicalizes the explicit output root and reuses scaffold::validate_copy_destination immediately before writing. Tests cover external existing/dangling leaf links for SwiftUI and Compose, preserved authored bytes/no new external target, and allowed explicit output-root aliases plus internal leaf links. Full native_ir_cli suite passes 37 tests; strict thin-CLI binary/native integration Clippy passes with -D warnings. Primary inspected scoped before/current source/test diffs; independent final review found no blockers. Affected rustfmt and repository whitespace checks pass. Evidence: /tmp/crepus-native-leaf-audit/{ir.before.rs,tests.before.rs,red.log,green.log,clippy.log}. Hard links and concurrent filesystem replacement remain outside the stable-filesystem preflight guarantee.

Resolved earlier WASM verification gap without changing installed runtimes or dependencies: executed the existing web_cli integration binary with explicit installed rustup stable Cargo and matching PATH. Four selected web_build_ tests passed, including actual docs-site and example-site WASM compilation/bindgen outputs, cargo-home/explicit-Cargo behavior, and multi-target selection. This confirms the earlier E0463 failures were compiler-selection/environment failures. Evidence /tmp/crepus-static-overlap/wasm-retry.log (4 passed, 28 filtered). This was build/artifact acceptance, not browser rendering or user-device latency proof.

Next concrete audit item: native fixture sync also follows existing output links; default scaffold destinations and explicitly requested outputs need separate containment policy before a fix. It was investigated but not changed this turn. No commits, pushes, branches, deployments, destructive cleanup or remote mutations; unrelated checkout changes preserved and ongoing goal active.

### Native fixture-sync destination preflight (2026-10-18)

Previous goal turn was progress (native leaf guard plus successful real-WASM build acceptance). Continued the recorded fixture-sync investigation with parallel read-only policy/final review. Baseline public regression failed accepting a default fixture leaf symlink outside the scaffold.

Sync now collects and validates the complete eligible output plan before any fixture comparison/write or explicit parent-directory creation. Default destinations are checked against the canonical scaffold root. Explicit --out paths retain absolute and parent-relative external directory selection; their leaf is checked against the resolved selected parent using existing scaffold destination validation and the existing path resolver. Common write handling preserves byte-equal skip behavior and original output order. Documentation now explains containment, explicit external output, and stable-filesystem/nontransactional limits.

Full native_ir_cli suite: 39 passed. New cases reject default leaf/parent escapes, dangling default/explicit leaves and explicit leaf escapes while preserving the earlier root fixture and outside sentinel (or absent target). Positive coverage preserves an internal default link and creates nested absolute/parent-relative explicit outputs. Existing stale-fixture repair/unchanged-mtime coverage still passes. Strict thin-CLI binary/native integration Clippy with -D warnings, affected rustfmt and repository whitespace pass. Independent final review found no blockers; primary inspected actual source/test/doc diffs and this ledger addition. Evidence: /tmp/crepus-native-sync-audit/{ir.before.rs,tests.before.rs,native.before.md,red.log,green.log,clippy.log}.

Validation is a stable-filesystem preflight, not handle-relative or hard-link isolation; later actual write errors can still leave earlier fixtures updated. No user-device builds or UI latency claims. No commits, pushes, branches, deployments, destructive cleanup or remote mutations; concurrent edits preserved. Ongoing goal remains active; next review can return to target/runtime responsiveness beyond these output-safety repairs.

### Worker disposal browser acceptance and responsiveness review (2026-10-18)

Previous goal turn classified as progress: complete fixture-sync destination validation was implemented and verified. Returned to responsiveness with parallel read-only reviews of web worker transport lifecycle and native action queues. Neither review found a concrete new supported-flow defect. Web request accumulation/nonreturning exports and native unbounded action queue/no disappearance cancellation remain documented limitations; synchronous compatibility APIs are not classified as newly broken async behavior. No new bounded-queue or universal no-lag guarantee was introduced.

Strengthened existing manual browser regression using the production runtime-client/worker assets and its labelled JavaScript fixture runtime: after normal queue recovery cases, a new worker call blocks on a separate SharedArrayBuffer flag; a mutation is queued behind it. Once the worker reports blocked, client disposal must settle the in-flight, queued and future requests with exactly the disposal error, while the new flag stays unreleased. The earlier heartbeat callback only releases its original flag. Independent review confirmed the assertions support request-settlement claims; they do not independently prove physical thread termination or execute WASM. Production transport source was unchanged.

Acceptance: 24 Bun tests across worker/client/app suites passed (98 assertions); manual actual-worker regression passed Chromium and WebKit, including prior heartbeat/clone/unprintable/init-failure checks plus new blocked-disposal assertions. Python syntax compilation and repository whitespace pass. Updated docs/web-runtime.md with both browser invocation and disposal coverage. Primary inspected exact before/current harness/doc diffs and audit addition. Evidence /tmp/crepus-worker-disposal-audit/{browser.before.py,docs.before.md,bun.log,chromium.log,webkit.log}. This proves scoped browser transport behavior, not native-device responsiveness, physical thread-stop timing, or frame latency under DOM load.

No commits, pushes, branches, deployments, destructive cleanup or remote mutations; concurrent work preserved. Ongoing goal active. Next meaningful responsiveness work requires workload-specific queue/DOM measurements rather than assuming documented unbounded queues are automatically defects.

### Large root replacement without argument overflow (2026-10-18)

Previous goal turn made progress by adding verified blocked-worker disposal acceptance. Parallel repeated-render review rejected a blanket identical-innerHTML optimization: live form values/checked state can differ from serialized HTML, so skipping authoritative replacement would change behavior. Primary then investigated setRootHtml's top-level-node spread. Actual Chromium baseline with 150000 sibling nodes failed with RangeError: Maximum call stack size exceeded.

Root replacement now moves the DOMParser body's children into one DocumentFragment and supplies that single argument to replaceChildren, retaining preserveFocus cleanup/reset boundaries. A first Range.extractContents approach was explicitly rejected after WebKit phase measurements: 10000 nodes parsed/extracted/replaced in 3/623/5ms; 20000 in 5/2758/7ms. Its large CPU-bound probe was intentionally canceled after that new scaling evidence (only owned probe PIDs), and its handle confirmed terminal exit137. This was a diagnostic change of approach, not a restart on observation timeout.

Final direct-transfer acceptance: actual Chromium 150000-node commit succeeded (~71.6ms single sample); actual WebKit succeeded (~161ms). WebKit baseline spread already succeeded (~111ms), so this is a Chromium capacity/correctness fix with cross-browser acceptance, not a universal speedup or frame-latency claim. New manual web_root_browser.py also checks complete node/text counts, dirty form value reset, focus/selection retention, mixed text/comment nodes, inert body scripts and empty output using production reconciliation source. All 114 Bun event/worker-app tests passed (251 assertions), including existing cleanup ownership cases. Python syntax and repository whitespace pass. Independent final source review found no blockers; primary inspected actual source/harness/doc/journal diffs.

Updated web-runtime documentation with manual invocation and appended durable rejected-optimization/argument-limit lessons to .jules/bolt.md. Evidence /tmp/crepus-root-arity-audit/{app.before.js,red.log,bun-direct.log,chromium-direct.log,webkit-direct.log,baseline.py,webkit-red.log,phases.py,phases.log,bolt.before.md,docs.before.md}. No WASM/device builds rerun for this browser reconciliation change. Rendering remains on the browser thread and very large DOM commits can still exceed frame budgets. No commits, pushes, branches, deployment, destructive cleanup or remote mutations; ongoing goal active.

### Production app/worker integration acceptance (2026-10-18)

Previous goal turn was progress: root replacement no longer exceeds Chromium argument limits, with browser acceptance. Parallel lifecycle reviews this turn found no new supported-flow defect in pending island mount/dispose ownership or native action sequence/error cleanup. Arbitrary host-driven island reparenting was not established as supported; documented native disappearance-cancellation limits remain. Those were read-only reviews, not new runtime/device test passes.

Closed an integration evidence gap: existing app tests stub worker transport, while transport browser tests omit the app. Added manual web_worker_app_browser.py serving unchanged production app/client/worker assets with an explicitly labelled JavaScript runtime fixture. The fixture throws if initialization, rendering or handlers execute on the main thread; both empty-root and SSR startup must boot visibly, process one click, then ten synchronous burst clicks. Strengthened the initial count-only probe after review: per-event data-sequence must equal the worker's next expected counter before mutation, making final state11 order-sensitive and exposing lost/duplicated/reordered actions.

Actual Chromium and WebKit each passed both modes with all11 FIFO event mutations and no page/console errors. Focused Bun worker/client/app suites passed24 tests/98 assertions. Python syntax compilation and repository whitespace pass. Primary inspected full new harness and exact documentation diff. Initial independent review found no blockers beyond the count-only FIFO assertion gap, addressed by the sequence check and fresh browser reruns. Evidence /tmp/crepus-worker-app-audit/{chromium.log,webkit.log,chromium-ordered.log,webkit-ordered.log,bun.log,docs.before.md}. Updated web-runtime documentation with invocation and fixture limitations.

Production code was unchanged this turn. This is real app/worker integration using a JS fixture, not Rust/WASM execution, native-device acceptance, or frame-latency proof. No commits, pushes, branches, deployments, destructive cleanup or remote mutations; ongoing goal active. Remaining responsiveness limits include main-thread DOM application and workload-dependent queue growth; no universal no-lag guarantee.

Final independent review confirmed the strengthened sequence assertions support FIFO for the tested burst and both startup modes; no remaining blocker.

### Fresh local Rust/WASM app-worker acceptance (2026-10-18)

Previous goal turn made progress by integrating production app/client/worker browser tests with a labelled JS fixture. Closed the remaining Rust/WASM execution gap this turn. Parallel fresh bounded TUI/embedded/LVGL review found no new confirmed panic/input-loss/resource defect; that read-only result is not a new target build pass.

Rebuilt current thin CLI offline in the coherent native target. Used its web new scaffold in owned /tmp/crepus-worker-wasm-audit/wasm-counter, retaining original runtime source with the worker opt-in marker. Custom fixture runtime uses actual local crepuscularity-core TemplateContext and crepuscularity-web render_bundle_with_context plus a worker-owned Cell counter. Event JSON data-sequence must equal next before state mutation. Fixture dependencies point to current local crates; this is not published-package acceptance. Rustup rustc1.98.1 plus wasm-bindgen0.2.126 built the site offline; runtime Cargo target config points to the existing coherent rustup WASM cache.

Initial browser failure correctly exposed a fixture error: quoted data-sequence="{next}" is literal under the authored DSL contract; replaced with data-sequence={next}, verified CLI HTML has sequence1, then rebuilt. A later strict console check found the optional crepus-islands.json404 emitted by this no-island site. Final probe tolerates only console404s whose location URL is that manifest; all page and other console errors remain fatal. Original failures are retained, not counted as acceptance.

Final actual Chromium and WebKit each passed generated SSR index and empty-root startup with eleven sequence-checked Rust/WASM event mutations, final count11, exactly one observed Worker and zero main-thread instantiate/instantiateStreaming calls. Each page observed one expected optional-manifest404. The custom empty-root entry embeds the same generated bundle. prepare.py verifies production app/client/worker bytes exactly match emitted assets and records SHA256 for JS/WASM/bundle/index. Probe syntax and repository whitespace pass. Independent final read-only review found no blocker; primary inspected fixture runtime/manifests, browser/prepare scripts, build logs and actual outputs. Evidence /tmp/crepus-worker-wasm-audit/{README.md,cli-build.log,scaffold.log,build.log,rebuild.log,template.html,browser.py,prepare.py,browser.log,artifact-sha256.json,wasm-counter/runtime/Cargo.lock}.

Only audit documentation changed in the repository this turn. This verifies current local compiled Rust renderer/state through browser workers, not UI frame latency, every WebAssembly constructor, published dependencies, native devices or deployment. No commits, pushes, branches, deployments, destructive cleanup or remote mutations; concurrent work preserved and goal remains active.

### Empty island manifest emitted for WASM sites (2026-10-18)

Previous goal turn was progress: actual current local Rust/WASM counter acceptance passed in two browsers, exposing the no-island optional-manifest404. Audited this follow-up with an independent compatibility review. App startup always requests crepus-islands.json, but build_web_islands previously removed it and returned when refs were empty. New fresh/stale manifest regression failed on baseline with NotFound.

WASM site/island builds now serialize the normal version1 manifest with an empty islands map even when there are no references. They do not create an unused islands directory. Rebuilding removes stale logical entries by rewriting the map, without deleting files. The manifest write now uses existing canonical-output destination validation, rejecting outward/dangling leaf links rather than introducing an empty-build write through an authored-file symlink. Unit coverage preserves the outside sentinel. Existing contained-link/stable-filesystem policy remains; this does not make island generation transactional.

Acceptance: all6 web_islands unit tests passed; strict thin-CLI Clippy with -D warnings, CLI build, affected rustfmt and repository whitespace pass. Rebuilt the actual local Rust/WASM counter site using the fresh CLI, asserted emitted JSON exactly {version:1,islands:{}}, and reran generated SSR and empty-root pages in Chromium and WebKit with all console/page errors fatal (removed previous optional404 allowance). All4 pages reached state11 through sequence-checked mutations, observed1worker/0main instantiate-or-streaming calls, and now report0manifest404s. Independent source review found no blockers; primary inspected actual source/test/doc diffs and this audit addition.

Evidence /tmp/crepus-empty-manifest-audit/{before.rs,red.log,green.log,clippy.log,build.log,site-build.log,manifest.json,browser.py,browser.log,docs.before.md}. Browser fixture still uses current local Rust crates, not published-package acceptance or latency/device proof. No commits, pushes, branches, deployments, destructive cleanup or remote mutations; concurrent work preserved, ongoing goal active.

### Nonempty island bundle destination preflight (2026-10-18)

Previous goal turn made progress by emitting empty manifests and removing generated-site startup404s. Parallel output-path review found nonempty build_web_islands invoked Bun before validating island directory/files; an outward islands symlink was accepted on baseline. New public-function regression failed with unexpected Ok after real Bun invocation. A separate bounded Imba control emission review found no concrete new interpolation/silent-drop defect; it was read-only, not a fresh Imba compiler gate.

Canonical output root and manifest destination validation now precede generation. The islands directory and every non-Rust generated filename are preflighted using the existing destination validator before mkdir or any Bun call. Final manifest validation remains. This prevents an unsafe later destination from leaving earlier generated bundles updated, while preserving selected root aliases and contained directory/file links. Existing callers already establish the output root.

All8 web_islands tests passed with --include-ignored, including the explicitly opt-in real Bun positive case for contained root/directory/leaf aliases. Negative coverage includes outward directory, generated leaf and manifest links, preserving an earlier bundle, outside sentinel and existing manifest. The real-Bun positive stays ignored by default so ordinary Cargo unit gates do not require an external bundler. Strict thin-CLI Clippy with -D warnings, affected rustfmt and repository whitespace pass. Primary inspected full before/current source/test/doc diffs; independent final review found no blockers. Evidence /tmp/crepus-island-output-audit/{before.rs,red.log,green.log,clippy.log,docs.before.md}.

Documented output containment. Preflight assumes stable links and does not isolate hard links, prevent concurrent path replacement, or make later compiler/IO failures transactional. No browser/device/latency claim from this build-path change. No commits, pushes, branches, deployments, destructive cleanup or remote mutations; concurrent work preserved and ongoing goal active.

### Broader thin-CLI, parser and reactive gates (2026-10-18)

Previous goal turn made progress by preflighting nonempty island output plans and verifying real Bun aliases. Broadened current-state acceptance beyond the recent focused build-path tests. Full thin-CLI binary unit suite passed188 tests with1 opt-in Bun case ignored by default; that exact Bun case passed explicitly in the prior module --include-ignored gate. Core parser unit filter passed343 tests (188 non-parser unit tests filtered). Full native default reactive package gate passed94 total tests/doctests, including4 compile-fail doctests;2 manual performance tests remained ignored. Native DOM/browser-specific test binaries run0 cases by cfg, so this is not a new DOM/WASM/browser gate.

Parallel read-only frontend-dispatch review found no concrete defect: known extension/suffix dispatch is case-insensitive and takes precedence over content fallback. Component selector semantics belong to higher-level include APIs, not the filename API. Parallel reactive panic/dependency review found no defect: failed computations restore prior+new subscriptions for retry; successful reruns prune old edges, and disposal prevents resurrection. Existing panic_subscriptions cases passed in the actual package gate. No speculative coverage-only code changes were made.

Evidence /tmp/crepus-cli-full-audit/{tests.log,parser.log,reactive.log}. All commands used offline/locked Cargo in the coherent isolated native target. Repository whitespace passes; primary inspected current dispatch and observer guard flow plus terminal gate output. Only this audit record changed for the turn. No full workspace/GPUI, feature-enabled source-module compiler, browser/device or deployment claim. No commits, pushes, branches, destructive cleanup or remote mutations; concurrent work preserved and goal remains active.

### Fresh language-server lifecycle and protocol gate (2026-10-18)

Previous goal turn made progress with broader current thin-CLI/parser/reactive gates. Moved to language-server document lifecycle and Unicode position handling with a parallel read-only review. No new concrete defect was found: current full-sync versions and diagnostic lifecycle ordering are consistent, UTF16 cursor conversion stays on UTF8 boundaries, and line handling shares core CR/LF/CRLF support. Primary inspected source lifecycle locks, completion/hover snapshots, position conversion and protocol probe assertions.

Built and tested current crepuscularity-lsp offline/locked using the coherent isolated native Cargo target. All38 tests passed (36 library,2 binary). Fresh crepus-lsp binary then passed the existing stdio protocol probe twice: graceful shutdown+exit status0 and exit without shutdown status1 while stdin remains open. The probe exercises open/change versions, stale/duplicate rejection, close/reopen, completion/hover contexts, frontend paths and Unicode diagnostics. To avoid the mixed shared target, used a disposable exact script copy with only the server binary path replaced; primary inspected that one-line diff. Strict all-target LSP Clippy with -D warnings and repository whitespace passed.

Evidence /tmp/crepus-lsp-current-audit/{tests.log,build.log,protocol.py,protocol.log,protocol-no-shutdown.log,clippy.log}. Only this audit record changed in the repository for this turn. This is scoped library+stdio acceptance, not editor UI, full workspace, browser/native-device or deployment proof. No commits, pushes, branches, destructive cleanup or remote mutations; concurrent work preserved and ongoing goal active.

### Current C ABI and Python plugin gates (2026-10-18)

Previous goal turn made progress with fresh language-server tests and two stdio shutdown scenarios. Moved to C ABI error/returned-buffer ownership plus public Python plugin acceptance. Parallel bounded review found no new confirmed defect: returned CString allocations are independent of session lifetime, error retrieval consumes its slot, and successful calls retaining an untaken prior error is consistent with the take-last-error contract. Malformed UTF8 C strings violate the documented caller contract rather than establishing a supported-input regression. Primary inspected error retrieval/freeing and wrapper library selection.

All13 C ABI tests passed (11 unit,2 component-selection integration). Rebuilt current local ABI dylib and thin CLI offline/locked in the coherent native target, then configured CREPUS_ABI_LIB and CREPUS_BIN explicitly for full Python unittest discovery: all23 tests passed. The suite includes actual dylib/session and CLI calls, plus clearly labelled mocks for HTML IR adapters and ASCII-locale transport stubs; it is not23 independent end-to-end native tests. Strict all-target ABI Clippy with -D warnings and repository whitespace pass. SHA256 records identify the exact dylib/CLI used.

Evidence /tmp/crepus-abi-current-audit/{tests.log,build.log,python.log,clippy.log,artifacts.json}. Only this audit record changed in the repository this turn. No new cross-language host/device/browser or published-package acceptance claim. No commits, pushes, branches, destructive cleanup, deployments or remote mutations; concurrent work preserved and ongoing goal active.

### Bun and Go CLI-adapter acceptance (2026-10-18)

Previous goal turn made progress with current ABI/Python gates. Continued adapter review without assuming every generated binding stub implements a native runtime. TypeScript/Bun abi_generated.ts is guidance/declarations rather than an implemented ABI session; the maintained adapter uses the CLI. Parallel review of that actual async adapter found no new concrete defect. Dispatch binding/callback/render behavior is consistent; interleaved concurrent handlers are not promised serialized transactions or invocation-time snapshots. No speculative concurrency rewrite was made.

An initial Bun command used a substring filter and also discovered an old copied test under editors/zed/grammars/crepus. That unrelated copy expected IRversion4 while current native IR is7 (root maintained test already expects7). Preserved the copy and reran with explicit ./plugins/typescript-bun directory scope: all24 tests across7 files passed (52 assertions). The original misrouted result is retained, not classified as a maintained-plugin regression. Tests use the current isolated CLI via CREPUS_BIN, including labelled subprocess/IR collaborators as well as real CLI cases.

Go package suite passed offline with GOTOOLCHAIN=local, GOPROXY=off and CGO_ENABLED=0, explicitly testing its maintained CLI adapter without linking the generated cgo stub against the mixed shared target. CREPUS_BIN likewise points to the freshly built isolated CLI from the prior gate. This is not Go C-ABI or race-detector acceptance. Repository whitespace passes. Primary inspected transport/session flow, actual test scope/results and this ledger diff. Evidence /tmp/crepus-bun-go-audit/{bun.log,bun-scoped.log,go.log}.

Only this audit record changed in the repository this turn. No browser/device/published-package or latency claim. No commits, pushes, branches, deployments, destructive cleanup or remote mutations; concurrent work preserved and ongoing goal active.

### Current native and feature-enabled source-module gates (2026-10-18)

Previous goal turn made progress with maintained Bun/Go adapter gates and corrected test-directory routing. Reviewed native handler lowering/bindings in parallel and ran both current native configurations. Scope clarification: crepuscularity-native emits immutable View IR and dispatch glue; runtime event JSON mutation belongs to ABI and CLI shell templates. No new concrete lowering defect was found.

Default package gate passed692 tests with0ignored across195 suite results. Feature-enabled native --features web-modules gate passed1738 tests with0ignored across195 suite results. Both used offline/locked Cargo and the coherent isolated native target. Feature compilation used CARGO_BUILD_JOBS=2 and reused the existing target; disk stayed around40GiB available. Long compilation was tracked on the same live session and confirmed active compiler children, then allowed to reach terminal success; no restart/cancellation on observation timeouts. This is current IR/compiler/AST/admission acceptance, not execution of every generated framework/runtime or physical-device UI. Repository whitespace passes. Evidence /tmp/crepus-native-current-audit/{tests.log,web-modules.log}.

Parallel responsiveness feasibility investigation found the default Swift result sink synchronously calls Rust store_result_json, not a Foundation decoder. Primary inspected the actual Rust template and confirmed store_result_json parses raw JSON, then parse_action_error(raw) parses it again while updating metadata. This identifies a bounded redundant-work seam before considering a larger prepare/commit handle API. No benchmark or speedup is claimed yet; no production change was made. Next action is to measure the actual compiled Rust template, keeping result semantics and main-thread commits intact.

Only this audit record changed in the repository this turn. No commits, pushes, branches, deployments, destructive cleanup or remote mutations; concurrent work preserved and ongoing goal active.

### Native result JSON reuse and parallel target review (2026-10-18)

Measured the actual standalone native Rust scaffold template before editing production behavior. store_result_json parsed JSON twice: once for model staging and again for error extraction. Extracted a borrowed-Value helper, retaining the raw-string wrapper for other dispatch callers. Validation, lock order, raw error fallback (including whitespace and empty errors) and rejected-result commit behavior are unchanged. Expanded semantic assertions and added an opt-in release storage measurement. Primary inspected the exact before/current diff; independent native review approved semantics and measurement scope.

Same locked standalone scaffold dependencies, release flags and warmed median11×5 calls: 36,030-byte result 155.775→86.308µs (44.6% lower), 1,800,030-byte result 7235.917→6493.300µs (10.3% lower). Default template43 tests passed; filesystem configuration44 passed, each with1 manual benchmark ignored. Explicit release benchmark passed separately. All-target/all-feature strict Clippy, rustfmt and repository whitespace passed. These are full Rust storage-call measurements, not isolated parsing or device/frame-latency proof; UI commits remain synchronous. Evidence /tmp/crepus-result-parse-audit/{before.rs,baseline-source.rs,baseline.log,after.log,tests.log,filesystem.log,clippy.log,fixture/}.

Reactivated seven separate native agents for CLI/plugins, core, Flutter, native semantics, authored modules, terminal/embedded and web/runtime reviews. Core lookup/evaluation, terminal/embedded arithmetic and bounded authored-module paths yielded no new confirmed defect. CLI review identified a source-derived YAML ref quoting candidate; Flutter review identified an unbounded sparkline-width candidate. Those require focused reproduction before fixes. No full-workspace, physical-device or published-package acceptance claimed. No commits, pushes, branches, deployments, destructive cleanup or remote mutations; concurrent work preserved and ongoing goal active.

### Parallel review fixes: Flutter refs, chart constraints and extension cleanup (2026-10-18)

Completed the native template integration gate: all39 native_ir_cli tests passed in addition to the template checks above. Continued independent read-only reviews of core lookup/evaluation, GPUI macro event admission, Tauri command/error/event lifecycle, authored Vue helper/directive admission, and embedded layout/display arithmetic. No new concrete defect was established in those bounded flows; this is not exhaustive package/runtime acceptance.

CLI Flutter helper: actual CLI output parsed by installed PyYAML showed valid Git refs #release/null becoming null and true becoming a boolean. New regression tests failed terminally before production edits (8passed/2failed). The shared dependency helper now serializes git_ref as a quoted JSON-compatible YAML string using existing serde_json. Both dep and add are covered; no new dependency. All190 thin CLI unit tests passed with1 existing opt-in Bun test ignored; strict all-target thin CLI Clippy, build and rustfmt passed. Fresh CLI dep/add outputs for8 refs parsed as exact YAML strings; repeated add left bytes unchanged. Primary inspected exact task diff. Evidence /tmp/crepus-flutter-ref-audit/{red.log,green.log,clippy.log,build.log,yaml-before.json,yaml-after.json} and /tmp/crepus-flutter-ref-quoting-fix/{flutter.before.rs,task.patch}.

Flutter renderer: source and View IR row/horizontal-scroll sparklines reproduced4 infinite-width failures before production edits; bounded cases passed. _sparkline now follows the existing chart width policy, using120px when horizontal constraints are unbounded while retaining bounded full width.143 related tests passed; strengthened source/IR scroll cases use4 charts and assert actual scroll movement. Scoped analysis clean. Rendered row/bounded/scrolled images inspected by worker and scrolled output inspected by primary; source/IR image pairs match. Screenshots are fixture evidence, not physical-device acceptance. Evidence /tmp/crepus-flutter-sparkline-width-fix/.

Web extension: actual current content asset Chromium probe established pending iframe removal leaves its Blob URL readable without a load/revoke, and synchronous src assignment failure leaks its URL. Minimal pending per-frame cleanup now revokes on load, reassignment, setup failure and observed disconnected removal; synchronous connected moves retain their pending URL. No sandbox or preview source changes. New Bun lifecycle regression covers exactly-once cleanup, descendant removal, reassignment and moves. Full directory Bun suite71passed/1existing AssemblyScript managed-string skip (214 assertions). Actual Chromium and WebKit probes confirm removed pending URL unreadable, normal preview text rendered and URL revoked, and synchronous failure fallback cleaned. Browser probes stub extension imports/fetch and expose the private function; not installed-extension acceptance. Primary reviewed source/test diff and result artifacts. Evidence /tmp/crepus-webext-blob-lifecycle/{probe.py,results.json} and /tmp/crepus-webext-blob-fix/{content.before.js,scoped.patch,bun.log,all-bun.log,probe.py,results-chromium.json,results-webkit.json}.

No full-workspace/device/published-package or lag-free guarantee. No commits, pushes, branches, deployment, destructive cleanup or remote mutations. Concurrent changes preserved; ongoing goal active.

### Web all-feature acceptance and parallel lifecycle review (2026-10-18)

Previous goal turn made progress with verified Flutter, CLI YAML, native result-storage and extension resource fixes. Refreshed repository instructions/current audit state and ran seven independent bounded reviews of target manifest/dispatch, shared AST/cache/include paths, Flutter control admission, native mobile action queue ownership, authored-module graphs/output naming, TUI events/focus, and extension background/popup state. No new supported-flow defect established outside the TUI focus policy described below. Flutter picker/filePicker/tabs/dropzone are explicitly unsupported by this adapter, so no event parity claim was invented. Authored resolver depth stress and installed-host runtime coverage were not rerun.

Current crepuscularity-web --all-features package gate passed191 tests with0failed/0ignored across29 suite results, including3 doctests. Strict --all-targets --all-features Clippy with -D warnings also passed. Both used offline/locked Cargo and the existing coherent isolated native target. Primary inspected current SSR handler/stream implementation and tests: rendering/parsing use blocking lanes, and tests cover head/body, escaped render errors and parse error behavior. This is Rust native feature/configuration acceptance; wasm-only DOM test bodies may be cfg-excluded, and it does not replace browser/installed-extension/device acceptance. Evidence /tmp/crepus-web-current-audit/{tests.log,clippy.log}.

TUI review found handle_tui_event overrides an intentionally empty manual focus list and dispatches paste to an automatically discovered button; set_source also silently restores automatic policy. Two new focused tests failed terminally before production changes (Handled vs Unhandled; replacement list vs empty). Minimal fix gates automatic refresh consistently on automatic_focus in both paths, preserving source/cache invalidation and constructor/clone/explicit-refresh behavior. Tests cover suppressed dispatch, empty/nonempty lists across source replacement, subsequent events and explicit recovery. Updated method/docs contract. Independent final source review found no blocker; primary inspected exact before/current source/test/docs diff. Evidence /tmp/crepus-tui-manual-focus-audit/{template.before.rs,docs.before.md,red.log,scoped.patch}.

Full TUI package gate passed319 tests with0failed across41 suite results (including5 doctests);1 existing registered-file origin regression remains explicitly ignored. Strict all-target TUI Clippy with -D warnings, affected rustfmt and repository whitespace passed. No UI painting behavior changed, so no new visual acceptance claimed. Primary confirmed terminal success for every Cargo session and inspected actual source/test/docs/audit diffs. Evidence /tmp/crepus-tui-manual-focus-audit/{green.log,clippy.log}.

Revalidated ignored registry provenance constraint: current Component and ComponentRegistry expose public struct literals, and register_file retains section source/defaults without path; existing docs specify caller-base include semantics. A sidecar field would also break those literals. An additive opt-in file-aware registry/render API could preserve the existing contract, while automatic existing-API provenance needs the already-pending compatibility decision. No speculative global metadata/source rewriting was introduced and the existing ignored regression was preserved. No commits, pushes, branches, deployment, destructive cleanup or remote mutation. Ongoing goal active; browser-only/device/full-workspace gates remain separate.

### Core current-state acceptance and target contract reviews (2026-10-18)

Previous goal turn made progress with the TUI manual-focus repair and web all-feature acceptance. Refreshed current instructions, diff whitespace and audit ledger; preserved concurrent work. Seven independent bounded read-only passes covered bindgen target-dependent scalar lowering, core comparison/index admission, supported Flutter controls, native iOS/Android control payloads, authored Vue/Svelte binding defaults, LVGL XML/control coercion, and plain/SSR control serialization. No new concrete supported-flow defect was established; no speculative production edits or tests were added to force a finding.

Primary reviewed current cache snapshot/content/frontend-key handling and ran the complete crepuscularity-core --all-features package gate using offline/locked Cargo and the existing coherent isolated native target. All803 tests passed,0failed/0ignored across48 suite results (doctest runner had0cases), including real watcher callback teardown, symlink and replacement behavior. Same live Cargo handle was tracked through terminal success rather than restarted on observation timeouts. Evidence /tmp/crepus-core-current-audit/tests.log. This verifies current core native tests/admitted contracts, not every emitted framework runtime or a device UI.

Contract findings retained: Flutter Input/Slider and the previously reviewed picker family remain explicitly unsupported by this adapter; supported Button/Checkbox/Toggle use documented action-only callbacks and host rebuild state. Native emitters retain typed toggle/slider payloads and lexical binding paths. LVGL/HTML serializers share the reviewed coercion/validation paths. Nim clong/culong and Odin c.long/c.ulong are target-aware ABI spellings; no new width bug found from installed compiler definitions. Deferred Nim target-dependent overload collision normalization is separate and still pending. Authored model cleanup runtime and cross-target C ABI builds were not rerun in this pass.

Strict core --all-targets --all-features Clippy with -D warnings passed; repository whitespace passed. Primary inspected this exact audit diff and terminal results. Only the audit ledger changed this turn; no production/dependency changes, commits, pushes, branches, deployment, destructive cleanup or remote mutations. Goal remains active. Concrete remaining work includes cross-target Nim overload identity and the separately documented file-component provenance API decision; neither was disguised as solved by scoped tests.

### Angular global event boundary and target-aware Nim overload guards (2026-10-18)

Previous goal turn made progress with complete803-test core acceptance. Continued parallel read-only review of runtime bridge/hot reload, Lite worker shutdown/isolate ownership, embedded include/paint transaction boundaries, Flutter supported control-flow scope, worker web cleanup, Angular/Astro unsupported admission and Nim target identity. No new confirmed issue in the bounded runtime/embedded/Flutter/worker flows; synchronous Lite waits remain documented rather than misreported as new bugs.

Angular parser accepted (window:resize) as a local EventHandler despite no global target field. Public filename regression failed terminally before production changes. Minimal guard now rejects known window/document/body target syntax with explicit unsupported diagnostic; ordinary local/custom event behavior and existing pseudo/empty validation remain. All3 Angular filename forms are covered. Documentation lists global targets as rejected. Full current core --all-features suite804passed/0failed/0ignored across48 suite results; strict all-target/all-feature Clippy and affected rustfmt passed. Independent review approved scope, primary inspected exact source/test/doc diffs. Evidence /tmp/crepus-angular-global-events/{before/,red.log,green.log,clippy.log,scoped.patch,frontends.before.md}. This is explicit unsupported admission, not new global listener support.

Installed Nim probes established that consumer compile-time type identity can close deferred clong/culong overload collision detection without target metadata or ABI changes. New deterministic generator regression failed terminally before production edits. Generator retains unconditional normalized signature errors and additionally compares prior same-name/same-arity emitted signatures involving target-dependent long types. Parameter identity uses Nim is joined with AND; prior candidate collisions use OR. Later declaration appears only in else, with a deliberate target-specific collision diagnostic in when. Pointer depth and original ABI spellings are preserved, public BindgenOptions unchanged. Independent structural review found no blocker; primary inspected actual source and test logic.

First explicit compiler run exposed an invalid test-only third name crepus__extra (Nim lexical rejection even in backticks), not a guard failure. Preserved failed log and replaced fixture with valid style-equivalent crepusEXtra after actual compiler confirmation. Corrected opt-in test2/2passed: generated bindings Nim-check on explicit Linux/Windows amd64 targets, checking signed/reverse-order/three-overload OR/unsigned/nested-pointer collisions plus long/int64 and multiargument noncollision controls. Windows collisions emit deliberate diagnostics without duplicate declarations; Linux valid overloads remain admitted. These are semantic compiler checks without C linking or real Windows runtime acceptance. Evidence /tmp/crepus-nim-long-guards/ and /tmp/crepus-nim-target-guards/{nim.before.rs,red.log,green.log,green-corrected.log,task.patch,readme.before.md}. Updated plugin guide. Separate bounded candidate remains: invalid Nim identifier spellings from legal consecutive-underscore C names need explicit validation; not silently treated as repaired here.

Complete bindgen default package suite38passed/0failed with5 opt-in tests ignored (three real Rust ABI language gates, V compiler gate, and new Nim target check). The new Nim target gate was separately enabled and passed as above; existing real Rust ABI/V gates were not rerun this turn. Strict all-target bindgen Clippy, affected Rust formatting and repository whitespace passed. Primary confirmed all live Cargo pipelines terminal before finishing and inspected complete scoped source/test/docs/audit diffs. Evidence /tmp/crepus-nim-target-guards/{all-tests.log,clippy.log}. No dependencies, commits, pushes, branches, deployment, destructive cleanup or remote mutations. Concurrent work preserved and goal remains active. Physical-device/full-workspace and actual Windows ABI/runtime acceptance remain separate.

### Nim identifier validation, Flutter gap precedence and source resolution (2026-10-18)

Previous goal turn made progress with Angular global event admission and Nim target guards. Refreshed current instructions/audit state and continued seven independent bounded module reviews. New concrete findings were reproduced before production edits; unaffected macro identifier, native numeric/JSON admission and Angular/Astro directive paths yielded no new confirmed defect.

Nim selected-header function/parameter names with leading/trailing/consecutive underscores emitted invalid Nim despite backticks. Actual compiler probes established lexical rejection, while ordinary internal underscores/camel/backticked keywords and lone underscore are valid Nim spellings. Public generator regressions failed terminally (1passed/2failed/1ignored) before the minimal validation helper/calls. Function/parameter diagnostics identify unsupported names before that binding is written; ABI names/types/target guards unchanged. Corrected focused4/4 tests passed including explicitly enabled Nim compiler controls. Complete default bindgen41passed/0failed with6opt-in tests ignored; strict all-target Clippy passed. A later private positive unit asserts the consciously retained lone-underscore lexical exception without expanding shared C-header admission. Plugin guide updated. Independent review approved normal validation and requested that explicit exception coverage. Evidence /tmp/crepus-nim-identifiers/{nim.before.rs,red.log,green.log,all-tests.log,clippy.log,task.patch,lone_export.nim,lone_export.log,readme.before.md}.

Flutter _gap selected the first class despite documented last-recognized precedence. Eight widget regressions failed terminally; one-line reversed scan fixes stack and synthetic scroll-content spacing while malformed trailing tokens retain prior valid gap.166related widget/parser/scroll/node tests passed; scoped analysis clean. Tests assert parsed spacing and rendered inter-text distance for both class orders, malformed trailing values and zero. Temporary rendered stack/scroll fixture inspected by worker and primary; screenshot harness remains only under /tmp. Primary inspected exact parser/test diff. Evidence /tmp/crepus-flutter-gap-precedence-fix/{parser.before.dart,task.patch,red.log,green.log,analyze.log,visual_test.dart,rendered.png}.

Embedded public snapshot command discarded authored filename/base by using Ui::new. Both public CLI regressions failed terminally: misleading caller-directory sibling rendered blue instead of red, and scripted Vue input selected wrong frontend. Minimal Ui::from_path substitution preserves existing context/component/output pipeline. Both regression tests pass with exact16x8PPM pixel/header comparisons. All190 thin CLI unit tests passed with1existing opt-in Bun test ignored. Method/error behavior uses the existing path-aware template error wording; path/cause still retained. Docs describe relative includes/filename selection. Independent review approved and primary inspected exact source/test/docs diff. Evidence /tmp/crepus-embedded-snapshot-path/{embedded.before.rs,red.log,green.log,cli-tests.log,scoped.patch,docs.before.md}.

Web render_from_files promised no filesystem access but ordinary/named/caller-slot missing sources fell back to native disk; two public regressions failed terminally. Private per-Walker virtual-only policy now selects lexical syntactically validated include paths and virtual lookup, with no exists/canonicalize/read fallback. Bundle entrypoints explicitly select it for plain/named entries and SSR markersfalse/true; recursive includes, borrowed slots and textarea's nested plain walker preserve it. No global/TLS/public context field or public signature change. Normal rendering retains native disk fallback, now accurately documented. Independent integration review found no blocker. Full web --all-features package196passed/0failed/0ignored; strict all-target/all-feature Clippy and formatting passed. New5tests include disk decoys, virtual precedence, normal fallback isolation, textarea/slots, lexical escapes and disk symlink shadows. Primary inspected complete actual production/test diff and doc correction. Evidence /tmp/crepus-bundle-virtual-only/{lib.rs,render.rs,ssr.rs,red.log,green.log,clippy.log,scoped.patch,lib.before-doc.rs}. This is source-resolution correctness/native acceptance, not installed-browser/physical-device validation.

Also ran current embedded default package148passed/0failed/1existing manual scaling test ignored across38suite results and --no-default-features cargo check passed, confirming host std tests and no-std configuration compilation. Evidence /tmp/crepus-embedded-current-audit/{tests.log,no-std.log}. No panel/firmware acceptance claim.

Final acceptance: thin CLI strict all-target Clippy passed. Added private lone-underscore unit passed independently and final bindgen strict all-target Clippy passed; integration positive syntax gate had already passed. Web --no-default-features check also passed after policy changes. All affected Rust formatting and repository whitespace pass. Primary inspected complete source/test/doc/audit deltas and every Cargo pipeline reached terminal success. Evidence /tmp/crepus-embedded-snapshot-path/clippy.log, /tmp/crepus-nim-identifiers/{lone-unit.log,clippy-final.log}, /tmp/crepus-bundle-virtual-only/no-default.log. Existing opt-in real Rust ABI/V/Nim-target gates were not rerun as part of the default bindgen suite; manual embedded scaling remained ignored. Physical panels, installed browser-extension/Windows/mobile hosts and full-workspace verification remain separate. No dependencies, commits, pushes, branches, deployment, destructive cleanup or remote mutations. Concurrent work preserved; ongoing goal active.

### Frontend collection, bundle namespaces and padding admission (2026-10-18)

Previous goal turn made progress with virtual-only bundles, Nim identifier checks, Flutter gap precedence and snapshot path preservation. Refreshed current instructions/audit state, then reviewed integration in seven separate module lanes. Concrete issues below were reproduced before production edits. Remaining bounded macro/default/directive/worker paths yielded no new supported-flow defect.

HTML collector integration: normal load_all_crepus omitted Vue/Svelte/JSX/Astro/Angular source dependencies, so self-contained bundle rendering failed on those includes. Two new actual collector→render_bundle regressions failed terminally (five frontend table plus named component/forwarded slots). Minimal flag change reuses the existing supported-template scanner for HTML bundles, preserving source bytes, .crepus-only fullwidth-brace normalization, directory skips, sibling symlink aliases and active-ancestor cycle prevention. Updated prior normal-collector count expectation and retained source/normalization assertions. Optional SSR error handling unchanged; authored scripts remain extraction-only. Independent review approved and primary inspected exact source/test patch. Evidence /tmp/crepus-html-frontends-bundle/{build.before.rs,red.log,task.patch,frontends.before.md}. This tests the actual collection/bundle render integration; no new whole-site WASM/browser acceptance claim.

Bundle namespace: root-level entry inherited unrelated ctx.base_dir while nested entry replaced it. Competing root/unrelated child maps confirmed wrong selection: web baseline1passed/2failed, native1passed/1failed. Web plain/SSR and native file-map APIs now always derive local cloned context base from the selected entry path, clearing it to map-root for root entries. Named selectors preserved, caller context unchanged, ordinary template rendering retains caller base. New tests cover root/named/nested/parallel modes and positive ordinary-render controls. Independent review approved; primary inspected exact web/native source/test diffs. Full web --all-features suite199passed/0failed/0ignored across31suite results; strict all-target/all-feature Clippy passed. Evidence /tmp/crepus-bundle-entry-base/{lib.before.rs,ssr.before.rs,red.log,green.log,clippy.log,scoped.patch} and /tmp/crepus-native-entry-base/{render.before.rs,red.log,render.patch}. Frontend guide documents entry-relative namespaces and collected template-only support.

TUI px/py utilities reset the other padding axis. Both regression tests failed terminally before two assignment changes that preserve untouched sides; p shorthand remains an all-side reset. Twelve combinations cover both orders, zeros/arbitrary zero, repeated axes and side/shorthand interactions; actual TestBackend bordered output matches explicit side padding and expected text origins. Full package321passed/0failed/1existing ignored registered-file-origin defect across42suite results; strict all-target Clippy passed. Temporary visual driver initially linked stale workspace artifacts, then was corrected to the fresh coherent test target; exact source/rlib hashes retained. Correct fresh output inspected by worker and primary: X at(3,2), visible left/top padding and complete border. No stale-artifact success claim. Evidence /tmp/crepus-tui-padding-axes/{style.before.rs,red.log,green.log,clippy.log,scoped.patch,visual.rs,visual.txt,visual-hashes.txt}.

Flutter unknown rounded classes overwrote authored radius with8 despite documented unknown-class ignore policy. New tests failed terminally (1passed/4failed). Minimal nullable helper/call now ignore unknown rounded-prefixed tokens, preserving named radius ramp and deliberately tested complete malformed arbitrary-radius fallback8. Existing rounded-bogus fallback assertion was intentionally corrected to null; interim old-expectation failure retained rather than hidden. Final138 related tests passed, scoped analysis clean; new cases check unknown-alone, before/after valid radius and actual DecoratedBox radius. Temporary rendered20px-vs-legacy8px fixture inspected by worker and primary, screenshot tooling only under /tmp. Primary inspected full actual parser/new test/single prior-test assertion diff. Evidence /tmp/crepus-flutter-rounded-admission-fix/{parser.before.dart,source_parse.before.dart,task.patch,red.log,green-final.log,analyze-final.log,visual_test.dart,rendered.png}.

Final gates: native default package694passed/0failed/0ignored across197suite results and strict all-target Clippy passed. Thin CLI192unit tests passed with1existing opt-in Bun test ignored; strict all-target thin CLI Clippy passed, including actual collector→bundle tests. All used offline/locked Cargo and the coherent isolated native target; every pipeline reached terminal success. Primary inspected source/test/doc/audit deltas and affected Rust formatting/repository whitespace passed. Evidence /tmp/crepus-native-entry-base/{green.log,clippy.log} and /tmp/crepus-html-frontends-bundle/{green.log,clippy.log}. Feature-enabled authored native compiler suite, whole-site WASM/browser and physical-host/full-workspace acceptance were not rerun this turn. Known TUI file-origin API decision remains separate. No dependencies, commits, pushes, branches, deployment, destructive cleanup or remote mutations; concurrent work preserved and goal active.


## 2026-10-18 — frontend dev sources and island discovery integration

Parallel module reviews completed: dev/build integration and island discovery produced scoped fixes; bounded Flutter callback/rebuild, native binding paths, Svelte keyed ownership, embedded/LVGL padding, and extension transport reviews found no additional concrete supported-flow defect. Independent final integration review found no blocker. Configured external OMP lanes remain unavailable; native agents used. Follow-up messages initially failed to resume two idle workers; corrected with followup_task before integration.

Dev startup omitted supported frontend sources and watcher events did not refresh their bundle bytes. Actual collector→render_bundle regression failed before production edits. Shared frontend predicate now drives initial dev collection and watcher classification. Extracted existing async update logic retains its empty-batch fast path; create/edit/rename/remove refreshes the map. Non-.crepus sources retain full-page reload semantics because they can also be authored island modules; JS/TS/CSS remain island-only events. Selected entry is threaded through startup, rebuild and static island builds. Nine extensions have deterministic event→queue→map regressions; five frontend fixtures exercise real bundle rendering. Startup validation remains .crepus-only.

Island discovery previously parsed every collected source without its filename. After behavior-preserving API alignment, terminal semantic red was 11passed/1failed/1ignored: malformed reached Vue was incorrectly accepted. Two earlier attempts were compile failures from partially aligned internal signatures, not semantic red evidence. Discovery now seeds .crepus sources and the normalized selected entry, follows literal template includes with authored filename dispatch, and leaves island modules opaque. All named component definitions remain scanned; missing lazy includes/selectors remain the renderer's responsibility. Logical include namespaces are preserved, ./ identities normalized, depth capped64 and accepted traversal states capped4096 with an explicit diagnostic. Minimum-depth revisits preserve shorter-route coverage. Initial green attempt exposed two incorrect Vue/Astro custom-tag fixture assumptions; fixtures corrected to admitted JSX aliases, with explicit Vue/Astro ordinary-tag controls and no parser feature expansion.

Primary terminal gates: focused island17/17 including normally ignored real Bun build; complete thin CLI204passed/0failed/1existing opt-in ignored; strict all-target thin CLI Clippy and fresh CLI build passed. Offline/locked Cargo used coherent isolated native target. Exact source/test patches inspected, affected whitespace checked. Evidence /tmp/crepus-dev-frontends/{web_serve.before.rs,task.patch,red.log,green.log,clippy.log,build.log} and /tmp/crepus-island-template-roles/{web_islands.before.rs,scoped.patch,red-interface-fixed.log,green.log,green-corrected.log}.

Real local Rust/WASM dev-server acceptance passed Chromium and WebKit in explicit main mode with a Bun-built TSX module island, then marked-worker mode without JS islands. Both modes verified Vue content edits, rename plus entry update, opaque unused JSX create/remove, full reloads, bundle key replacement/removal and Rust counter mutations before/after reloads. Main mode additionally rebuilt the TSX island visibly. Worker mode observed5workers across reloads per browser and0main-page instantiate/instantiateStreaming calls initially/finally. All final browser runs reported0console/page errors. Primary inspected main WebKit and worker Chromium rendered screenshots. Initial harness attempts waited for EventSource OPEN; WebKit reports CONNECTING until payload while delivering edits, so the readiness assertion was corrected to an existing nonclosed source; no production SSE change. Evidence /tmp/crepus-dev-frontends/{live-run.py,live-browser.py,main-browser.json,worker-browser.json,main-harness.log,worker-harness.log,main-server.log,worker-server.log,main-webkit.png,worker-chromium.png}. Owned servers terminated normally.

Limits: local path-dependent WASM fixture; Google Fonts CSS stubbed; TSX test is a module island, not React runtime coverage. Observed WASM API calls are not a latency/lag guarantee or exhaustive threading proof. No full-workspace/physical-target/release gates rerun. JS islands still require explicit main mode. No dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations; concurrent work preserved and ongoing goal remains active.


## 2026-10-18 — dev directory event reconciliation

Previous goal turn classified progress (implemented frontend source/discovery fixes and terminal tests/browser evidence). Continued seven parallel bounded reviews. Runtime/Lite lifecycle, supported Flutter Checkbox/Switch rebuilds, ABI ownership/reentry, native View IR control coercion, and embedded numeric bounds yielded no new concrete supported-flow defect. Unsupported Flutter text/select/slider were not misreported as implemented. Extension exact-asset probe confirmed settings:get rejection aborts startup before observer/content_main; no existing retry contract or routine-transient evidence established, so no speculative retry policy added. Probe /tmp/crepus-content-settings-retry-audit/probe.ts; no production changes there.

Directory events were classified island-only and left descendant virtual keys stale/missing. Tests-only baseline initially failed canonical temporary-root setup, corrected to actual dev root normalization; authoritative red then0keys vs2expected for populated-directory event. Scoped web_serve.rs change queues authored directory and island events through shared update logic, scans affected directories outside VFM write lock with existing ancestry-cycle collector, evicts path-component prefixes and replaces source bytes. Unknown/non-template module files never become VFM sources. Missing paths and directories (including names ending in frontend extensions) request full reload. Regressions cover populate/create/move-in, rename/removal, extension-named directories, file replacing directory, duplicate parent/child notifications, unrelated similarly prefixed siblings, ignored roots and symlink cycle. Initial full gates206passed/1ignored, strict Clippy/build succeeded.

Independent integration review found directory scans bypassed descendant event-path policy; tests-only terminal red5keys vs2expected demonstrated nested runtime, external symlink and ignored canonical-target bytes. Directory-event sources now pass is_authored_site_path before reads, retaining ordinary and allowed internal aliases. Shared initial collector contract unchanged; traversal itself still precedes the source-read filter. Primary inspected full original scoped patch and final guard/test delta. Final full thin CLI207passed/0failed/1existing opt-in ignored; strict all-target thin CLI Clippy and fresh CLI build passed, offline/locked coherent isolated native target. Evidence /tmp/crepus-dev-directory-events/{web_serve.before.rs,task.patch,red.log,red-canonical.log,descendants.before.rs,descendants-red.log,descendants.patch,green-final.log,clippy-final.log,build-final.log}.

Existing npm parser gate70passed/0failed/281assertions against checked-in WASM artifacts. Existing installed-package/browser script passed Chromium and WebKit: real WASM module workers, six frontend fixtures, default/custom WASM URLs, shared successful/failed initialization and validation. These checks do not rebuild WASM payloads from current Rust. Evidence /tmp/crepus-dev-directory-events/{parser-tests.log,parser-browser.log}.

Real local Rust/WASM worker-mode dev site passed Chromium/WebKit populated directory atomic move-in, visible Vue include, folder rename to renamed.vue, old/new key assertions, move-out/removal and Rust counter mutation. Initial live pass exposed a harness acceptance gap: final action could precede scheduled full reload; strengthened page-identity change and reset-to-zero waits, then reran terminally green in both browsers. Final rendered WebKit output inspected by primary. No pageerror events; console errors were not instrumented in this directory harness. Google Fonts CSS stubbed; no JavaScript islands in worker-mode fixture. Evidence /tmp/crepus-dev-directory-events/live/{browser.py,run.py,results.json,harness-final.log,server-final.log,webkit.png}. Owned server stopped normally. Docs now describe directory reconciliation; exact docs/audit deltas and affected whitespace inspected.

Limits: no full-workspace, physical-device, release, Windows filesystem or exhaustive symlink-race validation; worker fixture uses current local path dependencies and no lag guarantee is inferred. Startup collector policy and extension startup retry policy remain separate. Existing TUI file-origin compatibility decision remains pending. No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations; concurrent work preserved and ongoing goal active.


## 2026-10-18 — logical asset MIME and generated Python session cleanup

Previous turn classified progress: directory reconciliation state changed with terminal gates and real browser evidence. Seven parallel bounded reviews continued. LSP full-document/version/UTF16 paths, TUI loop/conditional scope, supported Flutter loop/action scopes, and web select/textarea serialization yielded no new confirmed defect. Native authored Svelte style-object behavior remains an unverified compatibility candidate: existing test deliberately admits it, no pinned runtime differential established, no edit justified. Configured external runners remain unavailable; native parallel agents used. Concurrent dirty work preserved.

Dev asset serving chose MIME from canonical symlink target rather than requested URL. Tests-only real handle_connection socket regression terminally failed: static/module.js?revision=1 returned application/octet-stream for payload.bin. Scoped web_serve.rs fix extracts the existing percentdecoder, shares it with logical filename MIME selection, and preserves resolver return/canonical containment/read and cache headers. Allstatic/pkg/islands/docs paths use logical MIME; generated known-path assets retain existing helper behavior. Regression asserts exact module bytes, query stripping, percentencoded extension, and outside-symlink404 across allfour routes. Independent review found no blocker; primary inspected full source/test patch. Full thin CLI208passed/0failed/1existing opt-in ignored; strict all-target thin CLI Clippy/build passed offline/locked coherent target. Evidence /tmp/crepus-dev-asset-mime/{web_serve.before.rs,red.log,task.patch,green.log,clippy.log,build.log}.

Real Rust/WASM local dev server served contained mime-audit-module.js symlinks to .bin payloads. Chromium and WebKit both fetched and dynamically imported percentencoded .js URLs with query strings through allfour routes, receiving200/application/javascript and exported42. No pageerror events; console errors not instrumented. Fonts CSS stubbed, no visual modification claimed or screenshot required for import acceptance. Owned server terminated normally. Evidence /tmp/crepus-dev-asset-mime/live/{browser.py,run.py,results.json,harness.log,server.log}. Canonical containment remains covered by actual socket tests, not asserted from module success alone.

Generated Python ctypes scaffold allocated native sessions but exposed no release method. Public generate_all→Python fakeCDLL regression failed terminally with AttributeError missingclose. Minimal generator string adds idempotent close, normal/exception context manager cleanup and guarded destructor cleanup through getattr pointer check, clearing pointer/callbackref after free. No speculative event/reentry API added. Test exercises repeated close plus GC, with exits, GC-only release, null constructor and CDLL-load failure without destructor stderr. Complete bindgen43passed/0failed/6existing opt-in ignored; strict all-target Clippy passed. Fresh ABI dylib built; actual CLI emitted adapter then exercised200native context scopes,200collected sessions, repeatedclose and exceptional exit without error. Mock counters prove exact free calls; live run proves native interoperability, not a heap profiler result. Primary inspected exact generator/test diff. Evidence /tmp/crepus-generated-python-lifecycle/{lib.before.rs,task.patch,red.log,green.log,clippy.log,abi-build.log,cli-generation.log,native.log,generated/python/abi_generated.py}.

Docs describe logical asset MIME and generated ctypes ownership. Exact docs/audit deltas and affected whitespace checked. Limits: no full-workspace/physical-device/Windows-host/release gates rerun; opt-in integration tests remain ignored as recorded. No dependency changes, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Ongoing goal remains active.


## 2026-10-18 — Angular branch guards and Tauri command key collisions

Previous turn classified progress (logical asset MIME and emitted Python cleanup fixes with terminal/native/browser evidence). Continued seven parallel bounded reviews. Web extension browser/output routing, LVGL escaping/identifiers, supported Flutter IR fallback handling, React/Solid escaping, and GPUI animation identity yielded no new confirmed supported-flow defect. Invalid arbitrary component names, nonfinite animation endpoint policy, and decoder version rejection were not promoted without a contrary contract. Source-only reviews do not imply target execution. Shared dirty work preserved.

Tauri command case conversion silently mapped distinct foo_bar/fooBar arguments onto one payload key. Public offline Cargo-consumer regression terminally failed because ambiguous camelCase compiled. Minimal private HashSet guard after injected State/AppHandle/Window exits and final key conversion now reports duplicate command argument wire key at second conflicting identifier. Existing case conversion unchanged. Compile-fail consumer checks both camelCase and snake_case diagnostics; actual invoke controls preserve distinct exact spellings without rename and injected names that normalize onto payload names. Independent review approved; primary inspected exact macro/test diff. Full Tauri all-features28passed/0failed/0ignored; strict all-target all-feature Tauri+macro Clippy passed. Evidence /tmp/crepus-tauri-argument-collisions/{macros.before.rs,tests.before.rs,macros.patch,red.log,green.log,clippy.log}. Consumer checks intentionally run offline with their own generated locks; root Cargo gates offline/locked.

Angular else-if recursion reused depth, bypassing documented256 limit; condition.contains(" as ") rejected quoted literal data and missed whitespace variants of unsupported aliases. Actual existing CLI probes accepted301conditions and rejected literal as; tests-only authoritative red29passed/3failed reproduced depth, literal false-positive and accepted compact alias. Minimal blocks.rs changes increment else-if depth into existing body/node guard and reuse shared quote/nesting-aware scan_top_level for top-level semicolons/alias words. Alias boundaries check only a positions with adjacent whitespace, avoiding repeated suffix trim scans. Test coverage includes short branch parse/evaluation, excessivechain rejection, single/doublequoted as, quoted semicolon and parenthesized contents, aliases across tabs/newlines. No unrelated block keyword changes. Independent review found no regression; primary inspected source/test deltas. Evidence /tmp/crepus-angular-branches-fix/{blocks.before.rs,tests.before.rs,production.patch,tests.patch,red.log}.

Full core all-features807passed/0failed/0ignored across48suite results and strict all-target all-feature Clippy passed. Thin CLI208passed/0failed/1existing opt-in ignored and fresh CLI build passed. Actual rebuilt CLI renders literal/shortchain cases and rejects compactalias/overdepth cases with expected diagnostics/statuses. Fresh Rustup WASM parser built from current source into reused isolated target, wasm-bindgen Node output written only under /tmp. Bun direct-WASM acceptance2passed/0failed/12assertions verifies quoted condition/branch preservation and alias/depth diagnostics. Initial WASM assertion incorrectly expected eager branch flattening; actual View IR retains If nodes, assertion corrected to existing branch structure without production changes. This is Bun-hosted WASM acceptance, not browser/GPU/threading evidence. Evidence /tmp/crepus-angular-branches-fix/{green.log,clippy.log,cli-tests.log,cli-build.log,cli-check.py,cli-acceptance.log,wasm/build.log,wasm/bindgen.log,wasm/check.ts,wasm/test-initial.log,wasm/test-final.log}. Checked-in npm WASM artifacts untouched.

Docs describe Angular aliases/nesting and Tauri collision admission. Primary inspected exact docs/audit deltas and affected whitespace. Limits: no full-workspace, physical-target, browser-UI, Windows-host or release gates rerun; existing TUI file-origin compatibility decision remains separate. No dependencies, commits, pushes, branches, deployment, destructive cleanup or remote mutations. Ongoing goal remains active.


## 2026-10-18 — Vue terminal branch provenance and TUI focus tokens

Previous turn classified progress (Angular branch guards and Tauri collision diagnostics with terminal/native/WASM evidence). Seven parallel bounded reviews continued. Imba/Svelte/Vue View IR event forwarding and host-owned updates, Tauri event snapshot/reentrancy, CLI framework emit selection, and Flutter link host dispatch yielded no new confirmed defect. Source-only reviews do not establish host interaction acceptance. Configured external runners remain unavailable; native parallel agents used. Shared dirty work preserved.

Vue terminal v-else body containing a single nested If looked like generated else-if AST. A following sibling v-else or v-else-if attached inside that nested body instead of erroring. Actual existing CLI reproduced C output; tests-only authoritative shared-parser baseline44passed/1failed confirmed malformedchain admission. Minimal parser-local open_branch_chain state now opens only explicit sibling If/ElseIf, closes after terminalElse/ordinarytag/meaningfultext, and preserves comment/whitespace adjacency. Shared AST/schema unchanged; existing slot walk runs only with known sibling provenance. Tests cover both illegal trailing directives/separators, valid genuine chains with independent nested terminal If, and ordinary template interruptions. Independent review found no blocker; primary inspected exact source/test patches. Evidence /tmp/crepus-vue-terminal-branch-fix/{template.before.rs,tests.before.rs,production.patch,tests.patch,red.log}.

Public syntactic TUI focus helpers used substring matching and admitted not-focusable/focusable-extra/prefixfocusablesuffix IDs. Source+AST public regression terminal1passed/1failed before production change. Syntactic focus now splits static whitespace tokens, resolves existing TUI/current-platform prefixes, and compares exactfocusable. It does not evaluate branch conditions, iterators, class interpolation or conditionalclasses; recursive context/ID behavior unchanged. Controls cover both syntactic branches plus unevaluated missing iterator, plain/TUI/platform positives, web/unknown/substrings/dynamic negatives and whitespace-containing AST class entry. Automatic Template focus already had exact active-token semantics and was not claimed broken. Full TUI324passed/0failed/1existing ignored file-origin test; strict all-target Clippy passed. Evidence /tmp/crepus-tui-focus-token-fix/{render.before.rs,scoped.patch,red.log,green.log,clippy.log}, new tests/focus_tokens.rs. No visual/layout behavior change claimed.

Full core all-features810passed/0failed/0ignored and strict all-target/all-feature Clippy passed. Rebuilt actual CLI rejects both malformed Vue chains with expected diagnostics/statuses and renders the valid nested chain. Fresh Rustup WASM parser built from current source, Node glue only under /tmp; Bun-hosted direct WASM2passed/0failed/8assertions rejects malformedchains and preserves genuine/nested If structure. Checked-in npm payloads untouched. Evidence /tmp/crepus-vue-terminal-branch-fix/{core-green.log,core-clippy.log,cli-build.log,cli-check.py,cli-acceptance.log,wasm/build.log,wasm/bindgen.log,wasm/check.ts,wasm/test.log}.

Native authored Vue integration exposed an acceptance-fixture assumption, not a new implementation regression: valid conditional templates still reach existing unsupported native Vue template control/content. Initial native gate stopped at1passed/1failed in new vue_terminal_branches.rs after negative parser-boundary cases succeeded. Corrected integration test preserves that explicit admission boundary, retains branch-specific malformed errors, and adds plain typed-props component DOM/SSR compilation. Did not implement or claim authored Vue conditional support. Entire corrected native web-modules suite1754passed/0failed/0ignored across198suite results. Broad strict native all-target web-modules Clippy FAILED on two existing len_zero assertions in unrelated untracked tests/moonshine_astro_source.rs125–126; file not changed. Strict affected native --lib --test vue_terminal_branches Clippy passed. Evidence /tmp/crepus-vue-terminal-branch-fix/{native-green.log,native-green-final.log,native-clippy.log,native-clippy-scoped.log}, new native/tests/vue_terminal_branches.rs. Broad lint failure remains explicit, not hidden by suppressions.

Docs clarify terminal branch closure, static focus tokens and authored Vue conditional/loop limits. Primary inspected exact docs/audit deltas and affected whitespace. Limits: no full-workspace/physical-device/browser-UI/Windows-host/release gates rerun; no heap/latency guarantee from these checks. Existing TUI file-origin compatibility decision remains separate, and broad native Clippy needs its unrelated Astro assertions corrected in a separately scoped change. No dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations; ongoing goal remains active.


## 2026-10-18 — Flutter unbounded column stretch and JSX attribute admission

Previous turn classified progress (Vue chain and TUI focus fixes, scoped/broad gates and explicit native lint blocker). Continued seven parallel bounded reviews. CLI profile/artifact selection, native picker duplicate/string semantics, embedded framebuffer/PPM output, and authored module graph ownership yielded no new confirmed defect. Component section markers inside JSX text remain an unverified framing candidate: reserved delimiter contract needs resolution before a lexical scanner change. No speculative rewrite added. Shared dirty work preserved; native parallel agents continued existing authorized lanes.

Separately scoped prior lint blocker cleanup: repeated strict moonshine_astro_source Clippy reproduced the two len_zero failures before edit. Only its two len()>0 assertions changed to !is_empty(), preserving all errors/expectations. All32Astro source tests passed. Broad native --features web-modules --all-targets strict Clippy passed, then passed again after the final JSX source/test edits; prior broad lint blocker is now cleared. Primary inspected exact2line diff and Rust formatting. Evidence /tmp/crepus-astro-lint-cleanup/{before.rs,red.log,green.log,clippy.log,clippy-final.log}.

Flutter column items-stretch inside Row/horizontal Scroll supplied tight infinite-width child constraints. Public source+IR widget regressions terminal2passed/4failed before production edit; bounded controls already passed. flow now wraps only verticalstretch in LayoutBuilder, retaining stretch for bounded width and selecting start for unbounded width. Local captured signed-margin mode preserves construction-time flow selection; normal and custom signed-margin flows share the same bound check. Expanded12widgetcases cover both source/IR ×bounded/row/scroll ×normal/signedmargins, finitegeometry, actualbounded240pxtextwidth and Columnstretch. Full plugin597tests passed; scoped renderer/newtest analysis clean. Temporary actual rendered fixture uses local Arial font, finitegeometry assertions and long overflowing content; maxScrollExtent>0, drag movedpixels>0 and no exceptions before/after. Primary inspected before/after screenshots: blueboundedfills320px, greennaturalwidth, orangehorizontaltextoffsetchanges. No screenshot tooling added to repo. Evidence /tmp/crepus-flutter-column-stretch-fix/{renderer.before.dart,task.patch,red.log,green.log,analyze.log,visual_test.dart,visual.log,before.png,after.png}. Independent review found no blocker.

Shared JSX parser accepted `<div {value}/>` as a literal brace-named Boolean attribute, producing invalid HTML instead of a hard error. Actual existing CLI reproduced it; tests-only authoritative red2passed/1failed showed malformed bindingprop retained. Final minimal guard is immediately after JSX tag attribute parsing, before ordinary/specialtag routing, so include/island/crepus-island also reject. An initial unbuilt ordinary-builder-only guard was moved to this complete boundary; builders restored byte-identical to snapshot. Shared scanner unchanged. Existing spread diagnostic retained; named JSX expressionattrs and Svelte/Astro identifier shorthand (including whitespace) remain supported. Core full all-features813passed/0failed/0ignored; strict all-target/all-feature Clippy and fresh CLI build passed. Expanded frontend table target rerun3/3 after Astro control added. Primary inspected exact guard/tests. Evidence /tmp/crepus-jsx-attribute-shorthand-fix/{builders.before.rs,tags.before.rs,production.patch,red.log,green.log,clippy.log,cli-build.log,positive-final.log}.

Actual rebuilt CLI rejects JSX/TSX/CSX shorthand and renders value=kept for named JSX and Svelte/Astro shorthand. Fresh Rustup WASM parser built from currentsource, Node glue only under/tmp; Bun direct-WASM2passed/0failed/6assertions verifies the same admission boundaries with real Input IR. Checked-in npm WASM artifacts untouched. Evidence /tmp/crepus-jsx-attribute-shorthand-fix/{cli-check.py,cli-acceptance.log,wasm/build.log,wasm/bindgen.log,wasm/check.ts,wasm/test.log}.

Docs describe constraint-aware columnstretch and frontend-specific attribute shorthand. Primary inspected exact source/test/doc/audit deltas; affected Rust formatting/whitespace clean. Limits: Flutter is headless widget/rendered acceptance, not physicaldevice/GPU/latency proof; WASM executed in Bun, not browserUI. No full-workspace/physical-target/release gates rerun, and no universal lag guarantee inferred. Native full test suite was not repeated for behavior-preserving Astro assertion cleanup; relevant32tests and broadClippy were run. Existing TUI file-origin compatibility decision and authored Vue conditional/loop admission remain separate. No dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations; ongoing goal remains active.


## 2026-10-18 — complete JSX branch admission, inspect namespaces and Flutter display

Previous turn classified progress (Flutter column constraint fix, JSX ordinarytag admission and prior native lint cleanup with terminal/visual evidence). Seven parallel bounded reviews continued. Evaluator shortcircuit/coercion, reactive disposal/reentry, real terminal key/modifier routing yielded no new confirmed defect. Imported Vue props syntax diagnostics remain a source-backed candidate: direct/barrel leaf parse failures omit path/details (vue_module.rs500/526); deferred without edits. Memory registry moved; scoped verification/concurrent-work guidance revalidated at current MEMORY.md549. No memories modified.

The previous JSX shorthand guard missed direct alternate branch attribute parsing. Tests-only authoritative red4passed/1failed reproduced else-if silent acceptance; sourceaudit found exactly4JSX scannerseams (ordinarytag/else-if/else/case). Private JSX parse_attrs now wraps shared scanner plus the existing brace-key guard; all4seams use it, eliminating percaller duplication. Shared scanner and Svelte/Astro semantics unchanged. Tests include paired/selfclosing case, else/elseif,3JSXextensions, original spread diagnostics and named condition/pattern AST controls. Full core all-features815passed/0failed/0ignored and strict all-target/all-feature Clippy passed. Actual rebuilt CLI rejects6alternatebranch files acrossjsx/tsx/csx. Fresh Rustup WASM built from currentsource, Node glue only/tmp; Bun directWASM2passed/0failed/12assertions coversalternatebranch rejection, namedbranch andSvelte/Astroshorthand positives. Primary inspected exact helper/callsite/test deltas; independent review found no blocker. Evidence /tmp/crepus-jsx-control-shorthand-fix/{tags.before.rs,tests.before.rs,production.patch,red.log,green.log,clippy.log,cli-check.py,cli-acceptance.log,wasm/build.log,wasm/bindgen.log,wasm/check.ts,wasm/test.log}. Checked-in npm payloads untouched.

Inspect render/IR contexts omitted source base_dir and selected caller-directory includes. Actual publicCLI baseline0passed/2failed rendered WRONG CWD HTML andIR despite a correct source sibling. Two base_dir assignments now derive parent from inspected path before loading variables. Subprocess tests avoid globalcwd races; render coversDSL+JSX andIR coversnamedcomponent+validJSON. Public integration2/2 passed, all208thin CLI unit tests passed with1existing opt-in ignored, strict all-target thinCLI Clippy passed. Evidence /tmp/crepus-inspect-relative-includes/{inspect.before.rs,task.patch,red.log,green.log,clippy.log}, newCLI tests/inspect_relative_includes.rs.

Expanded full thinCLI suite stopped at unrelated existing native_targets_reject_vue_script_blocks_before_creating_outputs in sfc_script_emit_cli.rs: it expects const-only setup rejection, while current compiler deliberately uses emptyprops schema and native/tests/vue_propsless.rs explicitly admits empty/const-only setup inDOM+SSR. StaticVue template never traverses newJSXcontrol attr paths andwebbuild nevercallsinspecthelpers, establishing unrelated expectation mismatch. Failedsuite11cases10passed/1failed; remaininglaterintegration binaries not executed. Test not changed or skipped to manufacture a broadgreen. Existing docs/frontends.md requirement for typeddefineProps in everysetupscript is also stale; normal<script> negatives must remain. Both need a separately scoped test/docs correction. FullCLI pass is NOTclaimed.

Flutter display consumer used Dart42.0/-0.0 where core explicitly normalizes smallintegralfloats. Public widget baseline3passed/2failed. Only _displayValue adds finite+integral+strictabs<1e15 double conversion tointstring; negativezero becomes0. Comparisonformatter unchanged.5widgetcases coverboundtext/interpolation/dynamicbuttonlabels/livehrefcallback, fractions/integers andthresholdfallback. All602plugin tests passed, scopedrenderer/newtest analysisclean. Temporary rendered fixture uses localArial, visiblyshows42/0 intext/interpolation/buttons andtapsOpen42/Open0 toassertcallbacks['42','0']; primaryinspectedrendered.png. This is bounded normalization, not universal Rust/Dart numericparity: large/exponent spelling deliberately remains Dart. Evidence /tmp/crepus-flutter-integral-display-fix/{renderer.before.dart,task.patch,red.log,green.log,analyze.log,visual_test.dart,visual.log,rendered.png}. Independent review approved boundedclaims.

Docs describebranch attribute rules, inspectincludebase andsmallintegraldisplaythreshold. Primary inspected exactsource/test/doc/audit diffs and affectedwhitespace. Limits: broadCLI suite retains the recorded staleVue negative; no fullworkspace/physicaldevice/browserUI/release gates rerun. Flutterwidget/rendered proof andBunWASM execution do not prove GPU/latency/universal lag guarantees. Existing TUI file-origin decision and authoredVueconditional/loop admission remain separate. No dependencies, commits, pushes, branches, deployments, destructive cleanup orremote mutations; ongoinggoal active.


## Vue contract and imported-type diagnostics follow-up — 2026-10-18

Parallel bounded CLI/native review corrected the previously recorded stale Vue contract. Existing native propsless setup positives and the empty props schema establish intended support. CLI negative coverage retains both ordinary script rejection cases and all diagnostic/no-output assertions; eight JS/TS empty/const setup DOM/SSR builds now verify authored module manifests, empty props, interpolation and absence of View IR. docs/frontends.md now states that defineProps is typed/top-level when used, rather than mandatory. No production admission changes.

Imported props syntax diagnostics previously omitted the leaf module path; named barrel reexports also discarded parser details. Authoritative tests-only red: 1 passed/1 failed. Only two production error-format branches now include module.path and Oxc parser details. New public compiler tests cover direct/barrel failures on both hosts plus valid reexports: 2 passed. Actual rebuilt CLI independently rejects four malformed direct/barrel DOM/SSR builds with the site-relative types/broken.ts path and Unexpected token. The first temporary acceptance assertion incorrectly required an absolute path; CLI uses site-relative module paths, and the corrected assertion passed. No runtime execution claim.

Primary inspected exact before/current patches and affected whitespace; independent review found no issue. Strict all-target Clippy passed for native with web-modules and CLI without default features. Expanded CLI suite cleared the old Vue blocker, reaching 425 passed, 2 failed, 2 ignored across 39 test binaries. Both failures were web CLI WASM builds under a toolchain missing wasm32 core/std. Reused the installed Rustup target-capable toolchain/cache and the already-built test harness: web_build_ subset 4 passed, including both failed sites. This establishes those builds but is not a single full-suite green; later binaries after the initial failure were not run. No full workspace, device or latency gates claimed.

Evidence: /tmp/crepus-cli-propsless-vue-contract/{task.patch,green.log,clippy.log,wasm-recheck.log}; /tmp/crepus-vue-import-diagnostics/{scoped.patch,red.log,green.log,clippy.log,cli-acceptance.log}. Concurrent edits preserved, incidental formatting restored. No dependencies, commits, pushes, branches, deployment, deletion or remote mutations. Ongoing audit goal remains active.


## Parallel expression-escape follow-up — 2026-10-18

Previous turn classified progress: corrected stale Vue contract/tests and improved imported-type error provenance. Revalidated repository instructions and memory scoped verification guidance at MEMORY.md549. Continued six bounded parallel review lanes. ABI error/serialization, embedded/LVGL malformed numeric/clipping and native import cycles yielded no new confirmed defect in reviewed supported paths. No broad completeness claim.

Core review reproduced Astro content literal mismatch: HTML entity CR became a JSON \r escape that the shared evaluator preserves literally. Tests-only authoritative red1passed/1failed. Only quoted content lowering now chooses existing quote_expression_string for shared evaluation, retaining serde_json quoting for authored JS. Braced expressions/admission unchanged. Both set:text/set:html tests cover CR,U+0001,LF,TAB,quote and literal backslashes; authored output remains valid JSON-escaped JS. All817core all-feature tests passed, strict all-target/all-feature Clippy passed,32native authored Astro tests passed. Fresh CLI rebuild plus2inspect tests passed. Actual CLI rendered4control cases: set:text retains CR/U+0001; set:html retains U+0001 and existing Ammonia sanitizer normalizes CR to LF. First temporary assertion wrongly expected raw CR after sanitization; corrected against actual sanitizer flow. Independent review approved shared/authored separation. Evidence /tmp/crepus-astro-content-controls/{production.patch,red.log,green.log,native.log,cli-build.log,clippy.log,cli-acceptance.log}.

Flutter review found expression _operand reused broader source-text unquoting. Tests-only red6passed/2failed for literal backslash-r and backslash-apostrophe. Extracted the already-correct mixed-expression decoder, reused for operand values; mixed behavior and source parser remain unchanged. README now distinguishes expression escapes. Eight public regression cases exercise condition selection, dynamic button label and tapped href callback, plus independent source-text behavior. All610Flutter tests passed; renderer/newtest analysis clean. Temporary Arial rendered fixture shows both Matched conditions and literal escape labels; both links tapped and raw callback bytes asserted. Primary inspected rendered.png. Independent source review approved parity with Rust's double-quoted expression subset. Evidence /tmp/crepus-flutter-expression-escapes/{task.patch,red.log,green.log,analyze.log,visual_test.dart,visual.log,rendered.png}.

Closed remaining prior CLI binary gap: webext_dev_cli1passed and webext_output_cli6passed. Prior broad CLI result and separate WASM recheck remain scoped evidence, not one full-suite green. Read-only toolchain review confirmed test prerequisite probes rustup inventory while effective production tools can select a different installed compiler; no test skip or toolchain rewrite introduced. Primary inspected scoped source/test/README/audit deltas and whitespace. No fullworkspace/device/WASM bridge/latency gates rerun this turn; no universal lag claim. No dependencies, commits,pushes,branches,deployments,deletions or remote mutations. Ongoing audit goal active.


## Static class-object key admission — 2026-10-18

Previous goal turn classified progress (Astro controls and Flutter expression escape fixes). Revalidated current instructions, dirty checkout and MEMORY.md549 scoped verification guidance. Six bounded parallel review lanes continued. Worker protocol/lifecycle, TUI Unicode/focus, target dynamic attrs/events and LSP document lifecycle yielded no new confirmed supported-path defect. Worker tests rerun directly with Bun:132passed/0failed/333assertions across5files. These reviews are scoped, not exhaustive target validation; LSP stdio and actual browser latency not rerun.

Core review found malformed static class-object keys accepted as literal names. Authoritative tests-only red1passed/1failed accepts Vue { 'a' + 'b': true }. Validation now requires quoted keys to be one complete literal token; original unescape_crepus_text_literal decoder remains unchanged. Bare letters/digits/_/$/- keys preserve the existing hyphenated utility extension; complete numeric literals remain supported. Other punctuation requires quoting. Computed-key rejection and duplicate final-value/original-key ordering unchanged. Regressions span Vue/Angular/Astro: quoted concatenation/adjacency, unquoted plus/whitespace/dot/slash negatives and quoted multiword/escaped apostrophe/backslashes, numeric, hyphen utilities and duplicate-order positives. docs/frontends.md states exact grammar.

All819core all-feature tests passed, strict all-target/all-feature Clippy passed. Fresh CLI rebuild with2inspect regressions passed; actual CLI rejects18malformed cases across3frontends with explicit key diagnostics. Fresh Rustup WASM build and Node glue generated only under /tmp: Bun2passed/0failed/21assertions, covering all18negative cases and quoted arbitrary utilities/bare utilities/numeric positives. Checked-in npm payloads untouched. Independent review found no blocker; primary inspected exact production/test/docs deltas and whitespace.

Evidence /tmp/crepus-class-key-syntax/{builders.before.rs,production.patch,red.log,green.log,cli-build.log,clippy.log,cli-acceptance.log,wasm/build.log,wasm/bindgen.log,wasm/test.log}; worker evidence /tmp/crepus-worker-audit-current/bun.log. Limits: no fullworkspace, physical-device, browserUI or universal performance gates this turn. CLI toolchain preflight mismatch recorded previously remains open; no skip introduced. No dependencies,commits,pushes,branches,deployments,deletions orremote mutations; ongoing goal active.


## LSP and binding-generator verification pass — 2026-10-18

Previous turn classified progress: static class-key admission fixed and verified through core/CLI/freshWASM. Revalidated current file state and scoped-verification memory guidance. Parallel bounded reviews covered textual include/preprocess limits, generated-host UTF8/error ownership, runtime HotReloadState/FrontendBridge recovery, scaffold paths and WASM toolchain prereq selection. No new confirmed supported-path bug found in these scopes. Deep accessor chains remain recursive but no new failure reproduced; no speculative production rewrite. OMP remains unavailable from earlier environment checks; existing authorized native lanes reused.

WASM preflight limitation confirmed precisely: test rustup inventory can disagree with Cargo's effective compiler chosen through CARGO/RUSTC, CARGO_HOME, working-directory config or toolchain files. A separate PATH rustc probe would not faithfully match Cargo. Left preflight unchanged instead of manufacturing a skip-green; actual prior acceptance explicitly selected installed Rustup tools. This remains a verification-tooling limitation, not a newly fixed product behavior.

Primary ran current LSP and plugin-bindgen test gates offline/locked in existing cache:81passed/0failed/6existing opt-in ignored. Strict all-target Clippy for both crates passed. Freshly built crepus-lsp binary exercised over real framed stdio: initialize advertises FULL document sync; open malformed Vue static class key publishes version1 diagnostics; version2 valid replacement publishes empty diagnostics; close publishes empty diagnostics; shutdown/exit returns0. Temporary harness initially omitted required --stdio and then sent invalid shutdown params; corrected harness uses --stdio and parameterless shutdown, with terminal successful evidence. These were harness errors, not product regressions. No source changes needed.

Evidence /tmp/crepus-lsp-bindgen-gates/{test.log,clippy.log,build.log,stdio.py,stdio.log}. Primary inspected audit append; no source diff introduced. Tests include current unit/integration coverage, but six optional external ABI/toolchain cases remain unexecuted; no fullworkspace, device, browser rendering or latency claim. No dependencies,commits,pushes,branches,deployments,deletions orremote mutations. Goal remains active.


## Facade, extension and LVGL verification — 2026-10-18

Previous goal turn classified progress through new LSP/bindgen gates and real stdio lifecycle evidence. Revalidated current manifests and scoped memory guidance at MEMORY.md549; shared checkout remains dirty and unrelated work preserved. Parallel bounded reviews covered facade manifest-relative output precedence/collision preflight, contextual accessor selection, extension permissions/CSP/browser conversion/staged assets and LVGL nesting/text/schema behavior. No new confirmed supported-path bug found. Browser platform acceptance, filesystem race guarantees and full LVGL runtime schema acceptance were not inferred from source review.

Primary current offline/locked Cargo gates for crepuscularity facade, webext and lvgl:142passed/0failed/1ignored. The ignored case is repeated_used_slots_scaling, explicitly a manual scaling comparison with no wall-clock assertion. Strict all-target Clippy for these3crates passed. Extension Bun suite71passed/0failed/1optional skip/214assertions across8files. Located the existing AssemblyScript compiler at /tmp/crepus-as-string-abi/node_modules/assemblyscript/bin/asc.js and ran optional runtime-as-strings separately with CREPUS_ASC_PATH:1passed/0failed/44assertions. This exchanges actual managed WASM strings across allocation/GC, avoiding a mocked-string-only claim. No dependency installation or runtime modification.

Evidence /tmp/crepus-target-build-gates/{test.log,clippy.log,bun.log,as-strings.log}. No production edits in this pass; primary inspected exact audit append and whitespace. These gates do not establish fullworkspace, physicaldevice, livebrowserpermission or universal performance acceptance. Known toolchain prerequisite mismatch and manual scaling check remain open. No commits,pushes,branches,deployments,deletions orremote mutations. Ongoing audit goal active.


## Reactive and embedded verification — 2026-10-18

Previous turn classified progress via current facade/webext/LVGL gates and real AssemblyScript GC/string acceptance. Revalidated manifests and MEMORY.md549 scoped guidance. Parallel source reviews examined reactive scheduling/self-update/disposal and embedded no_std empty dimensions/fill/blit/text bounds. No new confirmed bug. Zero-width embedded-graphics image handling explicitly returns a zero image in installed dependency rather than dividing by zero; no speculative patch. Existing instructions/concurrent changes preserved.

Primary offline/locked gates on current worktree: reactive all-feature host tests94passed/0failed/2manual timing cases ignored; embedded default148passed/0failed/1ignored; embedded no-default-features library32passed/0failed/0ignored. Strict reactive all-feature/all-target and embedded default/all-target Clippy passed. Host tests do not execute cfg-WASM browser DOM tests or physical-panel behavior.

Explicitly ran the previously ignored LVGL repeated_used_slots_scaling case:1passed,100/400/1000rows with5samples each and last-row content assertions. This run is debug-mode functional/scaling observation, not a release benchmark or speedup claim. Reactive queued teardown/wide memo manual release timing cases remain unrun. No universal lag or browser-thread latency guarantee claimed.

Evidence /tmp/crepus-reactive-embedded-gates/{reactive.log,embedded.log,nostd.log,scaling.log,reactive-clippy.log,embedded-clippy.log}. No production source edits needed; primary inspected exact audit append and whitespace. No fullworkspace, cross-target firmware compilation, physicaldevice or livebrowser gates this turn. No dependencies,commits,pushes,branches,deployments,deletions orremote mutations. Ongoing audit goal active.

Embedded has its own separately ignored repeated_used_slots_scaling case; explicitly ran it as well:1passed at100/400/1000rows with5samples and content assertions. Evidence /tmp/crepus-reactive-embedded-gates/embedded-scaling.log. Both manual slot cases are now executed for functional evidence; no release timing claim.


## Browser DOM and embedded_template path fix — 2026-10-18

Previous turn classified progress through reactive/embedded gates and both manual slot functional checks. Revalidated current DOM source/test setup and MEMORY.md549 scoped guidance. Fresh reactive DOM WASM built offline/locked with installed Rustup target and existing cache. Actual wasm-bindgen browser harness driven through local Playwright:17passed/0failed inChromium and17passed/0failed inWebKit. Covers current live binding/disposal tests, not universal latency. Evidence /tmp/crepus-reactive-dom-current/{build.log,browser.py,browser.log,chromium.log,webkit.log,server.log}; checked-in npm payloads untouched.

Parallel Tauri argument/error review found no new confirmed bug. Primary current Tauri host/macros tests27passed/0failed/0ignored and strict all-target Clippy passed, evidence /tmp/crepus-tauri-macro-gates/{test.log,clippy.log}. No Tauri desktopwindow/platform acceptance inferred.

Macro review found embedded_template! validates an absolute path correctly but emitted include_str prepended manifest directory again. Authoritative consumer tests-only red0passed/1failed, missing duplicatedmanifest/absolutepath. Minimal production fix emits resolved path LitStr at originalargument span, with manifest-relative/absolute behavior doccomment. Same path now drives compile validation and dependency-tracked inclusion; no other macro path changed. New regression reuses one consumer target for relative+absolute source equality, filename containing spaces, Unicode source content, template-only edit rebuild, and malformed absolute source parse diagnostics. Full macro tests37passed/0failed with4existingignored documentation examples; strict all-target Clippy passed. Independent source review approved; primary inspected exact production/test/audit deltas and whitespace.

Evidence /tmp/crepus-embedded-template-path/{lib.before.rs,dependency_tracking.before.rs,production.patch,red.log,green.log,clippy.log}. No fullworkspace, physicaldevice, firmwarecrosscompile or release/latency gates claimed. No dependencies,commits,pushes,branches,deployments,deletions orremote mutations; ongoing audit goal active.


## Current thin CLI integration completion — 2026-10-18

Previous goal turn classified progress: embedded_template absolute path fixed, macro/Tauri gates and actual reactiveDOM Chromium/WebKit acceptance. Revalidated current macro paths and MEMORY.md549 scoped guidance. Parallel remaining-file-macro review found no further manifest-prefix inconsistency; resolved source/dependency paths are already shared by view_file, crepus_refs and template_refs. Documented symlink-retarget limitations remain unchanged. No new production changes in this pass.

Primary fresh thinCLI test run offline/locked with --no-default-features, filtering only2known mismatched-toolchain WASM cases:432passed/0failed/2ignored/2filtered across41binaries. This reached both extension test binaries and every integration binary after web_cli. Ran freshest compiled web_cli harness directly with explicit matching installed Rustup CARGO and RUSTC paths, RUSTUP_TOOLCHAIN and offline mode: web_build_4passed, including both filtered actualWASM site builds (other2cases duplicate mainrun). Both ordinary ignored cases have installed prerequisites; explicitly ran --ignored: realBunisland containedoutputalias andXcodeGensharetargetroundtrip bothpassed. Thus436distinct thinCLI tests passed across the split executions; no assertion was removed and no ignored/filtered case remains unexecuted for this configuration. Strict all-target thinCLI Clippy passed.

Toolchain pins were process-local; no installedruntime/config/testpreflight rewrite. The original default probe mismatch still exists, but actualWASM gates are now current and verified with matching tools. Split execution is not one uniformly configured fullworkspace/defaultdesktop run. Evidence /tmp/crepus-cli-current-full/{test.log,wasm.log,optional.log,clippy.log}. Primary inspected exact audit append and whitespace; all concurrent source edits preserved. No dependencies,commits,pushes,branches,deployments,deletions orremote mutations. Ongoing audit goal active; fullfeature desktop/device/firmware/release latency gates remain separate.


## Full-feature desktop compile/lint — 2026-10-18

Previous turn classified progress through all436distinct thinCLI test cases across main/pinnedWASM/explicitopt-in runs. Revalidated manifests, feature graph, cache and MEMORY.md549 scoped guidance. Parallel desktop CLI gating and runtime renderer/prepared identity reviews found no new confirmed supported-path bug. Full defaultfeature includesdesktop+TUI; thinfallback dispatch matchescurrentcode. Prepared caches retain AST/source identity while runtime context drives styles/conditions. No hypothetical renderer rewrite.

Primary cargo check --offline --locked -p crepuscularity-cli --features full passed, including pinned gpui_macos/gpui_platform, runtime and CLI. Strict cargo clippy for fullfeatures/alltargets with -Dwarnings also passed. Existing /tmp cache reused, no linkeddesktop/testexecutable build claimed. Disk observed about18GiB free during check. Cargo reports thirdparty block0.1.6 future-incompatibility warning; not a current compile failure or dependency change. Evidence /tmp/crepus-desktop-current/{check.log,clippy.log}.

Source-backed follow-up candidate: builder::cargo_build discards Cargo stderr under assumption all diagnostics arrive asJSON. Setup/manifest errors before compiler JSON may be hidden; candidate not yet reproduced through desktoppublicflow, no edit/no repair claim. Also child.wait result ignored in current implementation; requires bounded acceptance before changes. Fullfeature desktop check/lint now verified; linkedunit/integrationtests, actualwindow interaction, device and latency gates remain separate. Primary inspected audit append and whitespace; no production edits. No dependencies,commits,pushes,branches,deployments,deletions orremote mutations. Goal active.


## Desktop Cargo diagnostics and process status — 2026-10-18

Previous turn classified progress via fullfeature desktop compile/lint gate. Revalidated actual Cargo runner/source and MEMORY.md549 scoped guidance. Raw Cargo malformedmanifest reproduction exits101 with0stdoutbytes and usefulstderr. Existing desktop builder discardedstderr and ignored child.wait status. Tests-only first CLI build stopped on unrelated concurrent native Svelte PartialEq code before testexecution; no regressionred claimed from that. Preserved active native edits, isolated the ACTUAL runner/options and ACTUAL repo test file in /tmp consumer with HUDdata shim (no copiedrunner/GPUIlink). Authoritative isolated baseline14passed/2failed: missingmalformedmanifestterminaldiagnostic, and build-finishedsuccess plus exit9 yields success=true/errors=[]/HUDerrors=[].

Runner now concurrently drains rawstderr bytes while parsingJSONstdout, streamsstderrterminal, joinsdrain afterwait, and requires successful exit AND successfulJSONbuildfinished. Failure withnoexistingcompilererrors gets onefallbackreturned/HUDdiagnostic; existingcompilererrors notduplicated. Spawnfailure alsopopulatesfallback. Existing artifact selection/JSONrendereddiagnostics unchanged. NoGPUI/HUDvisual behavior claim.

Regressions include real malformedmanifest, real dependency-free successfulcrate thenJSONtypeerror (exactly1returnederror/HUDparity), syntheticlateexitfailure and8192stderrlinepressure beforeJSONsuccess withwatchdog (nohang/noerrorsonsuccess). Isolatedconsumer18passed and strictalltargetlintpassed. Concurrentnativeeditresolved independently; subsequent fullfeatureCLIcheckpassed, actualrepositorythinbuilder_diagnostics18passed and fullfeature/alltargetstrictClippypassed. Existingthirdpartyblockfutureincompatwarning persists. Independent source review no blocker; primary inspected exactrunner/newtest/auditdiffs andwhitespace.

Evidence /tmp/crepus-builder-diagnostics/{builder.before.rs,task.patch,cargo-raw.log,red.log,isolated-red.log,green.log,isolated-clippy.log,full-check.log,cli-green.log,full-clippy.log}; isolatedconsumer manifest/lock only/tmp. No newtrackeddependencies. This verifies actualrunnerandHUDdata, notdesktopwindowinteraction/fullworkspacesuite/latency. All concurrentnativechanges preserved. No commits,pushes,branches,deployments,deletions orremote mutations; ongoing goal active.


## Desktop binary-name TOML parsing — 2026-10-18

Previous turn classified progress through Cargo stderr/process-status fix and regression/fullfeaturechecks. Revalidated current builder/dev paths and MEMORY.md549 scoped guidance. Parallel restart review found no new concrete process-lifecycle issue beyond selection: authoritative artifact paths still reject missing/ambiguous binaries instead of guessing stalepaths.

find_bin_name manually scanned manifest lines and permanently cached contents. Authoritative tests-only isolatedred0passed/2failed: valid inline comment became part of binaryname, and same-thread lookup afterrename still returnedoriginalname. Singlequoted names alreadyworked and remainpositivecontrols, not a newlyfixedbug. Replacedlineparser/cache withcurrentfile read+existingtoml::Value parser/exactnamekeys. Preserved explicitoverride→firstnamed[[bin]]→package fallback policy; corrected inaccurateoverridecomment. InvalidTOMLnowreturnsNoneunlessoverridegiven. ExistingCargo runnerfix untouched.

Tests coverpackage/bininlinecomments, misleadingname-extra key, escapedTOMLstring, singlequotes, manifestedit/deletion, malformedmanifest andoverrideprecedence. Realsuccessfixture nowhascommentedpackagename and verifies inferredname resolvesCargo'sactual executableartifact. Isolated22passed and actualrepositorythinbuilder_diagnostics22passed, strictfullfeature/alltargetCLIClippypassed. Independent review foundno blocker; primary inspected exactsource/test/auditdiffs andwhitespace. Evidence /tmp/crepus-builder-bin-selection/{builder.before.rs,tests.before.rs,task.patch,red.log,green.log,cli-green.log,clippy.log}.

Limits: devselectsnameatstartup; no hotreloadbinaryrename claim. Existingselectionpolicy doesn'timplementdefault-run/autodiscovery; not addedinthisparsingfix. No visualHUD/window/fullworkspacesuite/device/latency gates rerun. Existingblockthirdpartyfutureincompatwarning persists. No newdependencies,commits,pushes,branches,deployments,deletions orremote mutations; all concurrentworkpreserved and goalactive.


## Cargo default-run selection — 2026-10-18

Previous turn classified progress through realTOMLbinaryselection/stalecacheremoval. Revalidated current dev/builder paths and MEMORY.md549 guidance. Parallel review identified explicit default-run ignored and distinct inferred-single-bin gap. Authoritative tests-only default-run baseline0passed/2failed: firstnamedbinchoseninsteadconfiguredchosen; actualCargotwobinfixture buildsbutselectedExecutableendsfirst.

Minimal selection now prioritizes explicit--bin, then nonempty package.default-run, then existingfirstnamedbin/packagefallback. ExactCLIoptiondocs/functioncomment reflectpolicy. Existingparser/diagnostic/exitstatus fixes preserved. Regressions coverseconddeclaredbinary, explicitdefaultnamingauto-discovered target, overridewins andactualCargoartifactselection. Repository builder_diagnostics24passed/0failed and strictfullfeature/alltargetCLIClippypassed; independentreviewnoblocker. Primaryinspectedexactsource/test/docs/auditdiffsandwhitespace. Evidence /tmp/crepus-default-run/{builder.before.rs,tests.before.rs,cli.before.md,task.patch,red.log,green.log,clippy.log}.

Separate source-backed follow-up: validpackagehost withonlysrc/bin/worker.rs andnodefault-run/[[bin]] buildsworker, but inferredpackagenamehostmissesartifact. Avoidfallingbackformisspelledexplicitselection orambiguousworkspaceoutputs. Artifact-basedsolebinaryfallbackneedsselectionprovenance andpostbuildresolution; notimplementedinthisexplicitdefaultfix. Devstillselectsatstartup, no hotreloadrenameclaim. Existingblockthirdpartyfuturewarningpersists; nofullworkspacesuite/window/device/latency gates thisturn. No dependencies,commits,pushes,branches,deployments,deletions orremote mutations; concurrentworkpreserved andgoalactive.


## Cargo artifact-based single-bin fallback — 2026-10-18

Previous turn classified progress through explicitdefault-run selection. Revalidated builder/dev actualsource and MEMORY.md549 scoped guidance. Parallel architecture review confirmed provenance must distinguish requirednames frompackageinference. Authoritative tests-only realCargo baseline0passed/1failed: packagehostwithonlysrc/bin/worker.rs compiles successfullybutselectedExecutableisnullratherthanworker.

Introduced small Required/Package selectionenum preservingstartup intent. Explicit--bin/default-run/[[bin]] stayrequired. Package exactartifactname wins beforefallback; onlyPackage withnoexactmatch mayselectonesoleCargo-reportedbinary. Empty/multiple outputs errorwith--binguidance; duplicateexactmatches remainambiguous. Existingstrict executable(name) methodunchanged. Selectionflows throughterminal+HUDlaunchpaths; bothresolveartifactsafterbuildandprintactualresolvedname/path. HUDinitialprojectnamepreferredpackage retained; noHUDsource/layoutchanges. Legacyfind_bin_name wrappernowtest-only, productionusesprovenanceAPI.

Repositorybuilder27tests passed. RealsmallCargo fixtures verifysoleworker, packagemainpreferencewithhelper, multipleunmatchedautobinsnotguessed, missingoverride(strict). Unitcontrolsmissingdefault-run/declarednames, zero/multiplefallback, duplicateexactmatches andvirtualworkspace(noimplicitpackage). FullfeatureCLIcheckandstrictalltargetClippypassed. Independentreviewnoblocker; primaryinspectedexactbuilder/dev/test/docsdiffsandwhitespace. Testsverifyactualbuild/artifactresolutionAPIusedbydev, notinteractivewindow/process-restartvisualproof.

Evidence /tmp/crepus-auto-bin/{builder.before.rs,dev.before.rs,tests.before.rs,cli.before.md,task.patch,red.log,green.log,check.log,clippy.log}. Existingblockthirdpartyfuturewarningpersists. No fullworkspacesuite/device/browserlatency/releasegate claimed. No dependencies,commits,pushes,branches,deployments,deletions orremote mutations; concurrentworkpreserved andgoalactive.


## ABI C-host and component verification — 2026-10-18

Previous turn classified progress through singleauto-bin selectionfix andrealCargo/fullfeaturegates. Revalidated currentfiles and MEMORY.md549 scopedguidance. Parallel devreload review foundno confirmednewlaunch/reloadbug: bothmodeswatchsrc+Cargo.toml, registrationerrorsdiscarded. Customsourcepathsoutside src andatomicmanifestreplacementneedactualwatcherreproduction; notpromotedtofixwithoutboundedcontract/evidence. RepeatedcleanABIreview rotatedtocomponentscatalog/theme/CLIlookup; no newconcretecatalogdefect. No newproductionedits.

Primary currentABItests13passed/0failed/0ignored; strictalltargetABIClippypassed. BuiltcurrentABIcdyliboffline/locked; existingmacos_relocation.py compilespublicsmoke.c with-Wall-Wextra-Werror, copieslibrarytotemporarylocation, verifies@rpathlinkandDYLDactualcopiedlibraryselection, executessmokeassertions:passed. This exercisesrealCpublicAPIincludingcallbackreentry/ownedstrings ratherthanRust-onlyclaims. ComponentsRusttests5passed/0failed/0ignored. Evidence /tmp/crepus-abi-current/{test.log,clippy.log,build.log,c-relocation.log,components.log}.

Primaryinspectedauditappendandwhitespace. No fullworkspacesuite/desktopwindow/device/browserlatencyguarantee. Diskobserved13GiBfree; existingcachereusedwithoutcleanup/dependencyinstallation. No dependencies,commits,pushes,branches,deployments,deletions orremote mutations. Concurrentworkpreserved; goalactive.


## Declared Cargo-file dev watching — 2026-10-18

Previous turn classified progress throughcurrentABI/Csmoke/componentgates. RevalidateddevwatchconfigurationandMEMORY.md549guidance. Extractedduplicatedwatchregistrationexactlyunchangedintodev_watch.rs(sharedbybothdevmodes) beforesemanticfix, enablingactualhelperNotifytestswithoutGPUI. Authoritative baseline1passed/1failed: editingexplicit[[bin]]app/main.rs inprojectwithnosrcproducesnoevent within4s; srcandmanifestcontrols pass.

Sharedhelpernowregistersexistingdeclared[[bin]].path/[lib].pathandpackage.buildfile, ordefaultbuild.rsunlessbuild=false. Existing src recursiveandCargo.toml watches retained. CustomfilesNonRecursive; no recursiveprojectroot/buildoutputexpansion. Bothdevmodesrefreshafterdebouncedbatchbeforebuild; malformedmanifestpreservesexistingregistrations. Registrationfailuresprintpath/errorinsteadsilentdiscard. Existingtomldependencyreused. CLIwatchcoverageandlimitsdocumented.

ActualNotifytests4passedisolatedand4passedrepositorytesttarget: custombin/no-src, customlib/buildscript/defaultbuild, src/manifest controls andbuild=false/exactpath/manifestedit/malformedselectionunitchecks. Strictfullfeature/alltargetCLIClippypassed; independentevidence-reviewnoblocker. Primaryinspectedexacthelper/dev/test/docs/auditdeltasandwhitespace. Evidence /tmp/crepus-dev-custom-watch/{dev.before.rs,task.patch,red.log,isolated-green.log,green.log,clippy.log}; standaloneconsumer/lockonly/tmp.

Scope: existingdeclaredfileedits, notcompleteCargo dependencygraph. Arbitrarysibling/nestedmodulesoutside src, build-scriptdynamicinputs, atomicfilereplacement and replacedparentdirectoriesremainunverified/unsupportedwatchguarantees. Obsoletefileregistrationsarenotremovedaftermanifestchanges; no fullinteractiveGUI/restartproofclaimed. Existingblockthirdpartyfuturewarningpersists. No newtrackeddependencies,commits,pushes,branches,deployments,deletions orremote mutations; concurrentworkpreserved andgoalactive.


## macOS atomic-save watcher acceptance — 2026-10-18

Previous turn classified progress throughdeclaredCargo-filewatchfix+realNotifygates. Revalidatedcurrenthelper/testsandMEMORY.md549scopedguidance. AddedactualNotifyprobe fordeclaredapp/main.rs andCargo.toml: writetemporaryfile/renameovertarget, requiretargetevent, invokeactualwatch_projectrefresh, drainuntil300msquietwith4ssettlecap, editreplacedfileagainandrequirematchingnewevent. Originalbaselinepasses1/1onthismacOShost; no productiondefect/fixclaimed.

RetainedacceptancetestexplicitlycfgmacOS andnamedaccordingly; latermanifesteditusesvalidTOMLcomment. Repositorywatchertests5passed/0failed/0ignoredhere; strictthinCLItesttargetClippypassed. Independentreviewconfirmedboundedwaits/queuequietdrain/validmanifest/exactplatformscope. Primaryinspectedtestpatchandauditappend/whitespace. Evidence /tmp/crepus-dev-atomic-watch/{test.before.rs,task.patch,baseline.log,green.log,clippy.log}.

This narrowsprioratomicreplacementunknownonlyforthereproducedmacOSsequencewithregistrationrefresh. Linux/WindowsNotifybackends,parentdirectoryreplacement, completeCargodependencytrackingandinteractiveGUIrestartremainunverified. Generaldocsdo notpromiseuniversalatomicreplacement. No productionedits/dependencies,commits,pushes,branches,deployments,deletions orremote mutations; concurrentworkpreserved andgoalactive.


## SSR title RCDATA marker fix — 2026-10-18

Previous turn classified progress throughmacOSatomicwatchacceptance. RevalidatedcurrentwebsourceandMEMORY.md549guidance. Parallel SSRreviewfounddynamic titlechildrenreceivedspan/commentregionmarkup, whichHTMLRCDATAexposesasvisibletitletext. AuthoritativefocusedpublicAPIred0passed/2failed: directUnicode/controltextandinclude/control/forwardedslot titlecontainregionmarkup.

Minimaltitle-onlycase-insensitivebranchrenderschildrenwithplainWalker carryingdepth,virtual_onlyandcallerSlotFrame. Existingtextareahelper/value/CR/leadingLFpolicyunchanged; titlegetsno textareaextraLF. Parentmarkersrestorethroughsameouterflowandlatersiblingmarkersremain. Titleouterelementmanifestentryretained, descendantsnowomittedinaccordwithabsentDOMboundaries; nonewtitlehydration-updatecapabilityclaimed. Independentreviewfoundnoblocker; primaryinspectedexactproduction/testdeltas.

Finalwebgates:201all-featuretests,124no-defaulttests,148hydrationtests allpassed with0failures; strictallfeature/alltargetClippypassed. SeparatecurrentRustconsumer emitsactualHTMLinfourmodes. ChromiumandWebKit bothverifyidenticalcorrectdocument.title andrawtitle.textContent forUnicode,<>&andnewlines; browsernormalizesdocument.titlewhitespacewhileRCDATAtextretainsleadingLF. NoSSRmarkupliteralsvisible. Evidence /tmp/crepus-ssr-title/{render.before.rs,production.patch,red.log,green.log,plain.log,hydration.log,clippy.log,consumer.log,html.json,browser.py,browser.log}; consumermanifest/lockonly/tmp,no trackeddependencyadded.

ParallelentityreviewreportedVue/Angularliteral&amp; doubleescapingcandidatewith/tmp/crepus-shared-entity-audit fixtures. Requiresliteral-sourceprovenancepolicybeforeediting; runtimeexpression/IDliteralcontractsnotchangedhere. Noentityfixclaimed. Primaryinspectedauditappendandwhitespace. Nofullworkspace/device/release/latencyguarantee; no dependencies,commits,pushes,branches,deployments,deletions orremote mutations. Concurrentworkpreserved; goalactive.


## Shared Vue/Angular literal entities and parallel review — 2026-10-18

Confirmed literal entity double escaping on the shared AST route. Focused baseline: one test passed and three failed before production edits. The shared parsers now reuse the existing HTML decoder once for literal text and ordinary static attributes, after recognizing source interpolation. Encoded braces remain data; runtime values, raw CR/CRLF, dedicated ID/class semantics and the authored native Vue decoder remain unchanged. No Svelte decoder behavior changed. Scoped before/current production diff and independent review found no blocker.

Final focused core regressions: 5 passed, including named/numeric/multiscalar entities, single-pass decoding, unknown entities, encoded moustaches, runtime isolation, raw newlines, ID/class compatibility, native isolation and entity-only whitespace. Native Vue entity acceptance: 2 passed. Web all-features: 201 passed. Core strict all-feature/all-target Clippy passed. A current Rust consumer rendered both frontend fixtures; Chromium and WebKit both verified actual DOM text/title and existing ID/class values. Docs updated with the bounded shared-parser contract.

Broad core all-features run stopped on two concurrent Svelte intrinsic-spread negative expectations in svelte_modules.rs (37 passed, 2 failed in that binary). The in-progress compiler now accepts those constructs; this patch does not alter its decoder paths. Preserved that concurrent work, did not repair or skip those tests, and do not claim a full-core/full-workspace green gate. Evidence: /tmp/crepus-shared-literal-entities/{red.log,stage1.production.patch,green.log,native.log,web.log,clippy.log,consumer.log,html.json,browser.py,browser.log}; consumer dependencies/artifacts only in /tmp.

Resumed independent CLI/plugin, native/ABI and terminal/embedded reviews. Terminal/embedded found no new confirmed defect. CLI review reproduced inspect silently coercing invalid typed flags to zero/false; no fix yet. Native/ABI review safely reproduced PHP session cloning sharing ownership: a counting stub receives two frees for one session (no real memory freed). Evidence /tmp/crepus-php-session-clone/{fake.c,repro.php,red.log}. That defect remains pending; no PHP fix claimed in this pass.

Primary inspected the actual scoped source/test/docs deltas. No commits, pushes, branches, deployments, destructive cleanup, remote mutations or new tracked dependencies. Goal remains active; no universal lag guarantee, release or device acceptance claimed.


## PHP ABI session clone ownership — 2026-10-18

Previous goal turn made progress with shared Vue/Angular literal decoding and parallel reviews. Revalidated current adapter, repository instructions and MEMORY.md549 scoped evidence rule. The PHP ABI wrapper owned an FFI session pointer but inherited PHP shallow cloning; both owners could release the same pointer. A new safe static-storage counting stub reproduces this without freeing native memory. Authoritative regression before production edit exited255 with four frees instead of three after repeated-close and destructor controls passed.

Minimal production fix: private __clone prevents copying the native owner. Existing close/destructor and ordinary session paths unchanged. New session_ownership_smoke.php compiles a tiny C counter fixture into a random system-temp directory, checks repeated close, open-session destruction and clone rejection, and removes only its generated artifacts. Existing plugin-smoke wildcard selects this regression. It requires PHP FFI and a Unix C compiler; Windows reports its unsupported fixture host explicitly. Serialization already fails on FFI fields; no separate serialization change needed.

Final counting regression passes; both PHP syntax checks pass. Rebuilt current ABI offline/locked in the existing cache. Actual PHP-to-Rust ABI acceptance passes clone rejection followed by initial render, context patch, event dispatch, repeated close and destruction. Existing PHP session smoke also passes using the cached CLI (not a fresh CLI build). Evidence /tmp/crepus-php-session-clone/{adapter.before.php,regression-red.log,green.log,abi-build.log,real-smoke.php,real-green.log}. README documents non-cloneable ownership and the focused check.

Primary inspected the actual adapter/new-test/docs deltas and whitespace. No full polyglot suite, Linux/Windows runtime or release gate claimed. CLI typed-flag coercion remains pending. No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal active.


## Inspect typed-flag validation — 2026-10-18

Previous goal turn made progress by preventing PHP ABI session cloning with safe counter and actual ABI acceptance. Revalidated current inspect implementation, CLI manifest/help, repository instructions and MEMORY.md549 scoped evidence rule. Independent CLI review identified malformed --bool/--int values silently defaulting to false/zero. Tests-only actual-binary baseline: 1 passed, 1 failed because enabled=maybe was accepted with exit0 and no diagnostic; legitimate false/zero/negative render control passed.

Minimal fix replaces only the two parse fallbacks with the existing ui::error path. Diagnostics name the offending flag/raw key=value and expected Boolean or signed64-bit integer. Existing Rust bool/i64 successful parsing and context precedence unchanged. Documentation describes rejection before inspection output. New integration matrix checks maybe/TRUE Boolean values, nonnumeric/fractional/overflow integers, failure status, diagnostic content and empty stdout, plus typed false/zero/negative rendering and branch behavior.

Final actual CLI integration gates: 4 passed (2 typed-flag tests and 2 existing relative-include tests). Strict thin-CLI binary and typed-test Clippy passed with -D warnings. Independent final scoped reviewer found no blocker. Primary inspected before/current source/docs deltas, complete new test and whitespace. Evidence /tmp/crepus-inspect-typed-flags/{inspect.before.rs,cli.before.md,red.log,green.log,clippy.log}. No full desktop/workspace suite or release gate claimed; earlier concurrent Svelte spread negative expectations remain separately recorded.

No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal remains active for further module audits.


## Python ABI copy ownership — 2026-10-18

Previous goal turn made progress by rejecting malformed inspect typed flags. Revalidated current repository instructions, maintained Python adapter, binding generator and MEMORY.md549 evidence guidance. Parallel bounded review safely reproduced copy.copy duplicating a native session pointer. The maintained adapter and newly generated owning Python session class lacked copy guards. Separate shared-parser error-path review found no new confirmed defect.

Added explicit __copy__/__deepcopy__ TypeError guards to both maintained adapter and generator template. Existing lifecycle, callback and ordinary session methods unchanged; checked-in generated artifacts were not regenerated. Before production edits, maintained counting-mock regression failed both copy/deepcopy subcases and public fresh-output generator lifecycle test failed because copying was accepted. No real native memory was freed by either baseline. Extended controls verify no release on rejection, original usability, repeated close and final GC release exactly once.

Final maintained ABI suite: 9 passed using the current ABI library rebuilt in the preceding PHP pass, including real callback/event/error paths and the safely mocked ownership cases. Full bindgen suite: 43 passed, 6 opt-in tests ignored as before. Generated lifecycle runs fresh public generate_all output under Python with labelled counting functions. Strict bindgen all-target Clippy passed. Independent final review found no blocker. Primary inspected scoped adapter/generator/test/docs deltas and whitespace.

Evidence /tmp/crepus-python-session-copy/{adapter.before.py,bindgen.before.rs,maintained-red.log,generated-red.log,maintained-green.log,generated-green.log,clippy.log}. README records non-copyable ownership and focused gate. No full polyglot/workspace, physical-device or release gate claimed. No dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal active.


## Current Python/PHP integration and complete core result collection — 2026-10-18

Previous goal turn made progress with Python owner-copy guards and maintained/generated regression gates. Revalidated repository instructions and MEMORY.md549 scoped evidence guidance. No production edits this pass. Full maintained Python discovery: 24 tests passed with current ABI library and cached current CLI on PATH. All nine PHP smoke scripts passed, including ownership, path, transport, public HTML/control metadata and session behavior. Evidence /tmp/crepus-python-current/tests.log and /tmp/crepus-php-current/tests.log. No fresh CLI/ABI build claimed in this pass; these are artifacts built and verified in the preceding focused turns.

Ran current core all-features offline/locked with --no-fail-fast so the previously failing Svelte binary no longer hid later gates. Terminal exit101: 824 passed, 2 failed, 0 ignored; every test binary and doc-test phase completed. The sole failed target remains svelte_modules.rs: authored_spread_boundaries_remain_explicit:616 and unsupported_constructs_and_malformed_markup_are_errors:70 expect intrinsic spreads to fail while concurrent compiler work accepts them. Preserved source and negative tests unchanged; no full-core green claim. Parser robustness, later Vue/shared-entity and watcher binaries completed. Evidence /tmp/crepus-core-current/tests.log.

Parallel bounded runtime/Lite and extension lifecycle source reviews found no new confirmed supported-flow defect. Reviewed worker abort/join, isolate replacement/failure recovery, process-lifetime watcher contract, iframe Blob/listener cleanup, detached nodes, weak resize ownership, batching and mount deduplication. These are read-only findings, not new browser/device lifecycle acceptance. Earlier shared-parser source review likewise found no new defect; no speculative fixes added.

Primary inspected this append against its snapshot and whitespace. No full workspace/polyglot/release/physical-device gate claimed. No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Goal remains active; concurrent Svelte acceptance reconciliation and further module audits remain unfinished.


## PHP C-string NUL admission — 2026-10-18

Previous goal turn made progress by completing broad Python/PHP acceptance and collecting all core results (824 pass, two concurrent Svelte spread expectation failures). Revalidated repository instructions, PHP adapter and MEMORY.md549 scoped guidance. Parallel PHP boundary probe established literal NUL silently truncated supported string arguments at FFI. Tests-only actual ABI baseline exited255: template, baseDir and raw event inputs all escaped rejection and changed prior session state. The raw event uses handler=bind:count:9 to prove unintended mutation. Unicode and JSON-escaped NUL controls already passed.

Minimal guards reject literal NUL in template/non-null baseDir before calling set_template_string and in dispatch payload before the C event call. InvalidArgumentException names the invalid boundary. JSON-encoded context/array-event NUL remains escaped data and round-trips. Existing nullable baseDir and ordinary strings unchanged. Added abi_input_smoke.php, automatically selected by plugin-smoke wildcard, with actual ABI state-preservation and valid-data controls. No stub mistaken for native acceptance.

Final focused actual ABI regression and all ten PHP smoke scripts passed using the current existing ABI/CLI artifacts; no fresh native rebuild claimed for this PHP-only change. Both PHP syntax checks pass. Primary inspected exact source/new-test/docs deltas and whitespace. Evidence /tmp/crepus-php-nul-fix/{adapter.before.php,probe-before.log,red.log,green.log,all-green.log}. README documents C-string admission separately from JSON data support.

Parallel output/scaffold source review found no new confirmed defect; SSR streaming source review found existing error-escaping acceptance, no speculative change. Host adapter review identified an unpromoted picker snapshot candidate: Go/Ruby/C# option branches omit initial selection, but View IR/context provenance and supported contract need checking before a fix. No picker parity claim or change in this pass.

No full workspace/polyglot/device/release gate claimed. No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal remains active.


## Host picker contract trace and Go/Ruby verification — 2026-10-18

Previous goal turn made progress by rejecting PHP C-string NUL truncation with actual ABI state-preservation tests. Revalidated current repository instructions and MEMORY.md549 scoped evidence guidance. Traced the pending picker candidate through native ViewNode::Picker/lowering, CLI output and Go/Ruby/C# HTML traversal. Actual cached CLI with mode=b emits bind/options without a value snapshot; actual Ruby public render_html likewise emits data-bind/options with no selected attribute. Evidence fixture /tmp/crepus-picker-context-audit. Current plugin-surface excludes a live browser binding runtime/full parity, while view-ir-contract explicitly scopes SSR selection to Solid/Vue/Imba. Classified as a documented host-integration feature gap rather than a newly established contract bug; no IR/schema/render rewrite.

Added precise Go/Ruby/C# picker selection limitation to docs/plugin-surface.md. Host must apply bound selection; static browser HTML uses its default first option. Preserved the separate full-framework SSR contract. Primary inspected exact before/current documentation delta and whitespace. No production code edits.

Current maintained Go package race suite passed (go -C plugins/go test -race ./...). Current Ruby suite passed: 15 runs, 22 assertions, zero failures/errors/skips. Both used the CLI artifact built in the preceding inspect pass; no fresh CLI build claimed. Evidence /tmp/crepus-host-adapters-current/{go.log,ruby.log,plugin-surface.before.md}. Parallel output/scaffold source review found no new confirmed defect; project destination guards and prior escape fixes remain intact. No browser lifecycle or C# runtime acceptance was rerun here.

No full workspace/polyglot/device/release gate claimed. Earlier two concurrent Svelte spread negative-expectation failures remain recorded. No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal remains active.


## Current worker transport and browser integration acceptance — 2026-10-18

Previous goal turn made progress with picker contract tracing/documentation and current Go/Ruby acceptance. Revalidated repository instructions, web-runtime contract and MEMORY.md549 scoped guidance. Rotated parallel reviews back to the user's responsiveness concern. Independent source passes found no new confirmed worker startup/failure/order defect: FIFO execution, correlated fatal cleanup, event/render transactions, generation/revision publication guards, early rejection handling, pagehide disposal and no-replay/no-main-thread-fallback behavior match documented contracts. Unbounded backlog remains a load limitation without a newly reproduced supported-contract violation; no speculative event dropping added.

Current Bun worker/client/app/bootstrap suites: 27 passed, 117 expectations, zero failures. Actual Chromium and WebKit runs of existing web_worker_browser.py both passed: blocked worker permits main heartbeat, disposal settles in-flight/queued/future calls, response clone/unprintable errors reject, FIFO queue recovers without replay and startup failure reaches later calls. Both engines also passed web_worker_app_browser.py in empty-root and SSR modes: production app/client/worker boot and all eleven ordered event mutations. All four subprocess handles reached terminal exit0.

These browser checks exercise current production JS assets with labelled JavaScript fixture runtimes, not current Rust/WASM rendering or a universal latency benchmark. They establish transport isolation and lifecycle behavior while DOM/layout remain on the main thread. Existing docs/web-runtime.md already states worker opt-in marker/default scaffold behavior, compatibility limits and this test scope; no broader lag guarantee added. Evidence /tmp/crepus-worker-current/{bun.log,web_worker_browser.py.chromium.log,web_worker_browser.py.webkit.log,web_worker_app_browser.py.chromium.log,web_worker_app_browser.py.webkit.log}.

Primary inspected this append against its snapshot and whitespace. No production edits/dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. No full workspace/device/release gate claimed; earlier concurrent Svelte expectation failures remain recorded. Concurrent work preserved; goal remains active.


## Fresh local Rust/WASM worker acceptance — 2026-10-18

Previous goal turn made progress with current production worker transport and browser fixture acceptance. Revalidated repository instructions and MEMORY.md549 scoped guidance. Closed the immediate fixture-only evidence gap by rebuilding the existing scaffold-derived Rust/WASM counter runtime against current local core/web crates, using the stable rustup WASM compiler, offline dependencies and its existing build cache. New isolated output /tmp/crepus-worker-wasm-current/dist; prior fixture/evidence preserved. CLI artifact reused from the preceding inspect build; production JS assets were verified byte-identical to current source after emission.

The custom fixture preserves the original scaffold worker marker and uses worker-owned Rust thread-local Cell state. Its exported event handler checks the authored data-sequence before each mutation, and actual render_bundle_with_context renders the counter. Updated only disposable harness paths for the new dist, regenerated artifact SHA256 hashes and exercised real generated WASM rather than a JavaScript stand-in. Build and browser subprocesses both reached terminal exit0.

Chromium and WebKit each passed generated SSR index and empty-root startup. Every page observed exactly one Worker, zero intercepted main-thread WebAssembly.instantiate/instantiateStreaming calls and final Rust count11 after one click plus ten queued mutations. No optional manifest404s or unexpected console/page errors. Independent source review found no critical assertion gap and confirmed the bounded claims. Evidence /tmp/crepus-worker-wasm-current/{build.log,prepare.py,prepare.log,browser.py,browser.log,artifact-sha256.json}; runtime source/manifest retained at /tmp/crepus-worker-wasm-audit/wasm-counter/runtime.

This verifies current local Rust/WASM worker state, rendering and ordering through the production app transport. It does not establish every WASM construction path, universal frame latency, arbitrary workload responsiveness or package-release acceptance. DOM/layout remain on the main thread as documented. No production source/dependency change or broad test rerun needed for this acceptance-only pass. Primary inspected audit append and whitespace. No commits, pushes, branches, deployments, destructive cleanup or remote mutations; concurrent work preserved and goal active.


## Quoted match fallback across five backends — 2026-10-18

Previous goal turn made progress by rebuilding current local Rust/WASM and verifying worker startup/state/order in Chromium/WebKit. Revalidated current repository instructions and MEMORY.md549 scoped guidance. Parallel LVGL review identified quoted patterns incorrectly also matching their raw authored spelling. Primary traced the same defect in embedded, web, native View IR and GPUI runtime. TUI already uses exclusive decoded/bare comparisons and remained unchanged. Source status containing quote characters around ready wrongly selected the arm intended for decoded ready.

Added tests-only matrices to existing four backend match_escapes targets before production edits. Terminal --no-fail-fast baseline: each target had one positive escape test passing and one new matrix failing (four targets failed). Embedded regression verifies actual uniform RGB565 branch pixels plus hit-tested branch ID; LVGL verifies selected/fallback XML IDs; web/native use public HTML/IR render APIs. Matrices cover plain/quoted value distinction, escaped newline/quote decoding, raw source-spelling rejection, bare matches/misses and wildcard behavior.

Minimal production changes make quoted decoding exclusive from bare-pattern fallback in all five affected renderers. Wildcard handling and arm ordering preserved. DSL docs state decoded literal semantics. Final focused four-target acceptance: eight tests passed. Broader web all-features: 202 passed. Embedded std/LVGL complete host suites: 228 passed, two existing manual tests ignored. Strict all-target Clippy for all five affected packages passed, including compiling GPUI runtime. Actual interactive GPUI rendering was not exercised; its identical conditional fix is source-reviewed and compile-verified only. Independent final diff review found no blocker. Third-party block0.1.6 future-incompatibility warning remains.

A first broad command selected embedded MCU all-features and stopped offline before execution because az1.2.1 was not cached. Corrected the feature scope to relevant host std/LVGL plus web all-features; no packages installed or MCU hardware compatibility claimed. Before this fix, separate current Tauri all-feature acceptance passed28 tests and strict Clippy; that earlier audit gate is not claimed as post-fix native integration proof. Parallel native shell state-transaction and target-manifest dispatch reviews found no new confirmed defect.

Primary inspected actual before/current five source deltas, all four test additions, DSL docs and this append/whitespace. Evidence /tmp/crepus-match-quoted-fallback-fix/{web.before.rs,native.before.rs,runtime.before.rs,lvgl.before.rs,embedded.before.rs,red.log,green.log,clippy.log,broad.log,web-all.log,embedded-lvgl.log}; Tauri evidence /tmp/crepus-tauri-current/{tests.log,clippy.log}. No full workspace/device/release gate claimed. No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal remains active.


## Inspect flat-template IR dispatch and fresh CLI match acceptance — 2026-10-18

Previous goal turn made progress by fixing quoted-match raw fallback across five backends with public rendering tests, broad host suites and strict compile checks. Revalidated repository instructions and MEMORY.md549 scoped guidance. Built current thin CLI offline/locked for end-to-end acceptance. The initial HTML/IR/PPM script exposed a separate existing bug: inspect IR always selected a named component by file stem, so an ordinary flat template failed component-not-found. Recorded original script failure before changing production.

Tests-only baseline for flat DSL/JSX/Vue IR: three passed, one failed. Preserved existing named Card selection and missing-named rejection. First implementation classified every file with the DSL component parser; independent review identified literal markup lines such as --- Card being misclassified. A separate tests-only marker baseline failed (four passed, one failed). Corrected dispatch so explicit Vue/Svelte/Astro/Angular paths and JSX filename/content routes skip DSL section classification; other sources reuse parse_component_file and only nonempty named sections select the filename stem. Flat sources use native render_template_to_ir_with_path. Named defaults/base directory and parse errors remain delegated to existing APIs; no fallback to arbitrary named components.

Final actual CLI acceptance: seven tests passed, including flat templates for all six shared frontends, source-relative includes for supported DSL/JSX syntax with external-cwd decoys, literal section-like markup text, named section preservation and typed flag validation. The initial matrix incorrectly assumed include-tag semantics in Vue; corrected tests to avoid inventing unsupported include behavior. Strict thin CLI binary/affected-test Clippy passed. Fresh CLI script now passes both ready and quote-wrapped ready through HTML and View IR, plus exact16x8 PPM red/blue branch pixels. Evidence /tmp/crepus-inspect-flat-ir/{inspect.before.rs,red.log,green.log,marker-red.log,final-green.log,final-clippy.log}; /tmp/crepus-match-cli-current/{build.log,check.py,check-before-flat-fix.log,check.log,frame0.ppm,frame1.ppm}.

Remaining match consumer review found no analogous macro fallback: proc macros emit Rust match patterns directly. Clarified DSL docs that bare identifiers follow Rust binding rules in view!, while runtime bare patterns compare literally. CLI docs describe flat filename dispatch and strict named selection. Primary inspected actual source/test/docs deltas and whitespace. No interactive GPUI/device/release/full workspace gate claimed; prior concurrent Svelte expectation failures remain recorded. No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal active.


## Broad CLI recheck and measured TUI slot baseline — 2026-10-19

Previous goal turn made progress by fixing inspect flat IR dispatch with six-frontend/named/literal-marker acceptance and fresh CLI HTML/IR/PPM checks. Revalidated repository instructions and MEMORY.md549 scoped guidance. Current complete thin CLI --no-fail-fast run reached every test binary: 469 passed, two actual WASM builds failed under the default compiler's missing wasm std/core, two opt-in cases ignored. Re-ran the freshest web_cli harness with matching installed Rustup Cargo/Rustc/PATH/RUSTUP_TOOLCHAIN and the existing coherent WASM cache: four selected tests passed, including both previously failed actual site builds (two others duplicate main-run cases). Explicit installed-Bun island and XcodeGen share tests each passed. Thus473 distinct thin CLI tests pass across split configurations; original environment failures remain preserved rather than calling the default run green. Initial attempt at the ios_cli ignored harness selected zero tests; corrected to native_share_cli and required its actual one-test pass. Evidence /tmp/crepus-cli-after-inspect/{tests.log,wasm.log,bun-optional.log,ios-optional.log,share-optional.log}.

Parallel component-default/include source review found no new correctness defect in web/native/GPUI propagation. Separate TUI review identified per-supplied-slot prepare_include_context cloning its entire caller context and AST. A loop's caller retains the full rows list, so each include repeats that list clone; focus traversal shares the same helper. No production optimization implemented yet.

Added ignored manual tests/slot_scaling.rs with calling-thread-gated System allocator counters. Setup and assertions are outside measurement; cumulative requested bytes include alloc/alloc_zeroed/realloc, not retained heap or elapsed latency. Public BufferRenderer draws100/200/400 forwarded-slot rows through outer/card virtual templates; asserts every caller-scoped label against a conflicting callee prop. Warm-root Template.refresh_focus measured separately; every ordered focus ID, count and caller-context isolation asserted. Both baseline runs pass with identical counts; strict probe Clippy passes.

Measured render requested bytes:100rows10,921,179;200rows38,334,747;400rows142,661,883 (calls90,585/321,086/1,202,087). Focus bytes10,742,802/37,975,154/141,939,850 (calls89,957/319,859/1,199,661). This supports the repeated full-context-copy bottleneck; no claimed speedup yet. Evidence /tmp/crepus-tui-slot-scaling/{baseline.log,baseline-final.log,clippy.log,clippy-final.log}. Next implementation requires private borrowed SlotFrame threading through synchronous build/focus recursion, preserving public owned ctx.slot fallback, named defaults, caller forwarding and depth limits. Source-reviewed plan only; no unmeasured optimization claimed.

Primary inspected the complete new probe, tightened every-focus-ID assertions and reran it, plus inspected this append/whitespace. No full workspace/device/release gate claimed. No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal remains active and TUI slot optimization unfinished.


## Borrowed TUI supplied slots: measured allocation reduction — 2026-10-19

Previous goal turn made progress with473 distinct thin CLI acceptance cases across coherent split toolchains and a measured quadratic TUI slot baseline. Revalidated repository instructions/current source and MEMORY.md549 scoped guidance. Primary retained architecture/integration, delegated only tui/src/render.rs implementation, and owned added compatibility tests. Existing borrowed-slot design from other backends was adapted to synchronous TUI build/focus traversal; this is a private lifetime/threading change rather than a speculative cache or event policy.

Private SlotFrame borrows authored slot nodes, caller context and prior caller frame. Nine recursive build/focus helpers carry the optional frame; roots startNone. Supplied includes create local frames, unsupplied includes startNone rather than inheriting the outer slot. Consuming slots restores caller scope/frame, then retains public owned ctx.slot fallback and authored fallback behavior. Removed per-include inc.slot.clone and full ctx.clone from prepare_include_context. No public API change, stored borrowed state, TLS or production unsafe. Named defaults, explicit props, environment/bundle/base directories, existing depth/error handling and owned widget/focus outputs remain intact. Narrow existing-private-container helper argument-count allowance avoids unrelated public parameter restructuring.

New slot_context.rs covers named forwarding with caller/callee collisions, callee-relative includes, caller-relative includes with a decoy path, exact focus order and unsupplied-include fallback isolation, through both disk and virtual sources. Existing context_redraw/include_focus tests cover public owned slot mutation and cache invalidation. First custom VFS fixture used noncanonical absolute /var keys alongside canonical /private/var resolution and duplicate tail filenames; corrected only fixture keys to standard logical relative bundle names, preserving path policy. Corrected compatibility tests all pass. Independent final source/lifetime review found no blocker; primary inspected the complete actual patch and both new tests.

Allocation probe after change: render bytes100/200/400rows=1,760,111/3,505,587/7,009,715, calls13,385/26,685/53,287. Focus bytes1,577,942/3,144,994/6,279,090, calls12,557/25,059/50,061. Prior400row baseline render142,661,883/focus141,939,850 bytes: reductions95.09%/95.58%. Doubling rows now approximately doubles requested allocation bytes in this fixture. Every forwarded row label and focus ID remains correct. Counts measure cumulative successful alloc/alloc_zeroed/realloc requested bytes on the calling thread, not retained heap, peak RAM or frame latency; no wall-clock speedup or universal lag claim.

Full TUI host suite326 passed/0failed/2ignored; manual allocation test explicitly passed separately, leaving the existing known registry-file-origin compatibility test ignored. Strict all-target TUI Clippy passed. Docs describe borrowed supplied scopes, preserved owned API and repeatable measurement. Evidence /tmp/crepus-tui-slot-scaling/{render.before.rs,baseline-final.log,compatibility.log,compatibility-final.log,optimized.log,full.log,full-clippy.log,tui.before.md}; worker scoped patch /tmp/crepus-tui-borrowed-slots-fix/scoped.patch. Primary inspected docs/audit append and whitespace. No actual terminal event-latency/device/full-workspace/release gate claimed.

No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal remains active, including the separately recorded registry-origin and concurrent Svelte acceptance gaps.


## Android application ID inline comments and DOM commit measurement — 2026-10-19

Previous goal turn made progress by replacing TUI supplied-slot context copies with borrowed frames and measuring95%lower allocation bytes at400rows, with complete host gates. Revalidated repository instructions/current Component/ComponentRegistry public fields and MEMORY.md549 scoped guidance. File-origin tracking still cannot be stored without changing public struct shape or introducing the previously rejected hidden metadata; preserved existing compatibility and documented render-context workaround. No registry-origin fix claimed. Parallel embedded numeric/viewport review found no new supported-path defect.

Ran existing production setRootHtml large-root browser probe in both engines:150,000 children committed completely; elapsed65ms Chromium/107ms WebKit in these single host runs. Focus/selection restoration, mixed nodes, inert scripts and empty replacement assertions passed. This measures synchronous DOM parsing/moving/replacement only, not whole-frame rendering, a statistical benchmark or a regression/speedup. DOM cost remains main-thread work despite runtime-worker isolation. Evidence /tmp/crepus-dom-commit-current/{chromium.log,webkit.log}; no DOM algorithm change.

Parallel native build review found gradle_kts_value trimmed quotes from an entire assignment remainder, retaining inline //comment text in application ID. Unit tests before fix:2passed/1failed. Actual public CLI regression with labelled Gradle/ADB executable stubs failed with component `com.example.app" // release identity/dev.crepuscularity.nativeshell.MainActivity`. No actual Android build/device launch occurred. Unit command and integration command briefly queued on the same Cargo target lock before both reached terminal red; no process restarted or source changed while either baseline ran.

Minimal production scanner removes only //comments outside double-quoted strings, respecting escaped quotes/backslashes and UTF8 boundaries. Existing bare/empty values, first assignment and key-prefix behavior retained; no full Kotlin expression evaluation promised. The first local patch matched an identical TOML helper; immediately restored that helper from its snapshot before running checks and applied the scanner only to gradle_kts_value. Final actual diff contains no TOML change. Added quote/comment unit controls and cfgUnix public CLI argv regression across commented/plain/spaced assignments, with applicationIdSuffix decoy. Native docs corrected existing launch description and describe the bounded reader.

Final13native unit tests and one actual CLI regression passed. Strict all-target thin CLI Clippy passed. Independent final review found no blocker; primary inspected exact production/unit/integration/docs deltas and whitespace. Evidence /tmp/crepus-gradle-inline-comment/{build.before.rs,tests.before.rs,unit-red.log,cli-red.log,unit-green.log,cli-green.log,clippy.log,native.before.md}. Stub transport acceptance is not actual Gradle/ADB/device execution, Windows acceptance or full Kotlin parsing. No full workspace/release gate claimed.

No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal remains active, with registry-origin compatibility and concurrent Svelte acceptance gaps still separate.


## Android configuration assignment boundaries — 2026-10-19

Parallel native review reproduced native build android configuration updates overwriting applicationIdSuffix/applicationIdName/namespaceSuffix and an applicationId comparison expression because the writer matched key prefixes. Trailing comments were also lost. New native unit and public CLI regressions both failed before the production fix; the CLI fixture uses a labelled Gradle executable stub, not a real Android build.

The writer now requires an exact key followed by a simple assignment, rejects comparison operators, and preserves trailing unquoted line comments. Extracted the previous reader's quote-aware scanner into a shared private helper without changing its reader semantics. Unrelated fields and existing final-newline normalization remain unchanged. Added an exact-file CLI regression and native configuration unit coverage; documented the bounded simple-assignment behavior. No full Kotlin parser claim.

Final14native unit tests and two CLI regressions passed, including the previous Android-run reader regression. Strict all-target thin CLI Clippy passed. Independent final review found no blocker; primary inspected the complete scoped production/test/docs deltas and whitespace. Evidence /tmp/crepus-gradle-assignment-boundary/{build.before.rs,tests.before.rs,unit-red.log,cli-red.log,unit-green.log,cli-green.log,clippy.log,native.before.md}. Actual Gradle, device execution, Windows and full workspace gates were not run.

No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal remains active.


## Extension page frontend dependencies — 2026-10-19

Previous goal turn completed the verified Android exact-assignment/comment fix. Revalidated repository instructions/current files and MEMORY.md549 scoped-work guidance. Parallel bounded reviews found no new numeric/serialization or LVGL/embedded supported-path defect. Extension review reproduced a real CLI build failure: options.crepus including child.vue failed strict virtual-bundle lookup because collect_crepus_pages loaded only .crepus files.

Delegated tests-only ownership of webext_output_cli.rs; primary retained production integration and gates. New public CLI regression reached terminal red before production edits: missing parts/nested/card.vue. Collector now reuses the existing shared frontend classifier for its dependency map, while preserving exact lowercase .crepus page-entry behavior. Nested Vue/Svelte/Astro/Angular/JSX template dependencies render without generating dependency pages or clients. Existing template-only parser/script contracts remain unchanged.

Independent review identified eager UTF8 decoding of previously opaque HTML as a compatibility risk. Non-entry InvalidData files remain outside the template map and are copied verbatim by the existing asset path; no lossy decoding. Other IO errors retain explicit failure. Regression checks ordinary HTML, legacy-encoded HTML and invalid-UTF8 JavaScript bytes unchanged. Explicitly including the legacy HTML fails and preserves the complete previous output snapshot. Docs describe this boundary. Separate views/popup.crepus prerender still builds a single-file map; its include behavior was not expanded in this patch.

Final33extension CLI unit tests and seven output integration tests passed; strict all-target thin CLI Clippy passed. One combined command's webext name filter selected zero integration tests, so the unfiltered output binary was explicitly rerun and all seven passed. Original agent fixture rerun with fresh CLI built successfully and actual options.html contains VISIBLE. Primary inspected complete scoped production/test/docs diffs and whitespace; independent production review found no blocker. Evidence /tmp/crepus-webext-frontend-includes/{webext.before.rs,test.before.rs,red.log,green.log,final-tests.log,final-output-tests.log,repro-green.log,clippy.log,docs.before.md}.

Separate current Rust webext/LVGL/LSP gates173passed/0failed/1existing manual test ignored. Extension JavaScript71passed/0failed/1optional skip; optional actual AssemblyScript managed-string/GC gate explicitly passed separately with44assertions using the existing compiler. Evidence /tmp/crepus-target-review-20261019/{rust-tests.log,extension-js.log,assemblyscript.log}. These gates do not prove installed extension/browser runtime behavior, actual WASM host integration, hardware acceptance or full workspace compatibility.

No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal remains active.


## Authored extension popup includes and literal titles — 2026-10-19

Previous goal turn fixed extension page frontend dependencies with terminal acceptance. Revalidated repository instructions/current source and MEMORY.md549 scoped guidance. Parallel Tauri bridge review found no new confirmed command/event/state/capability defect. Popup follow-up confirmed its separate prerender path populated a map containing only popup.crepus. Nested includes therefore failed even after the page collector fix. Popup titles also interpolated manifest names without HTML escaping.

Delegated tests-only ownership in webext_output_cli.rs; primary owned production and final gates. Tests-only terminal red: one existing popup preservation test passed, both new tests failed. Include case failed missing parts/panel.crepus; title case produced a literal closing title and injected script element in generated HTML. Minimal production changes populate the popup map through the existing verified collector rooted at the template parent and escape extension names with the existing HTML helper. Three independent state contexts and staged error propagation remain unchanged. Docs describe nested views includes and literal title text.

All nine extension-output CLI tests passed, covering nested .crepus/Vue templates across mutually exclusive main/help/crepus states, malformed dependency failure with complete previous-output snapshot retention, title escaping, and earlier output/asset cases. Strict all-target thin CLI Clippy passed. Independent final review found no blocker. Primary inspected actual scoped source/test/docs deltas and whitespace.

Fresh CLI-generated popup HTML also passed Chromium and WebKit DOM acceptance: exact literal document.title, injected-script marker absent, all three isolated state contents with included Vue footer, main visible and other containers hidden, no pageerror events. External font CSS and popup.js were stubbed; this verifies prerendered HTML rather than installed extension runtime/WASM behavior or event wiring. Browser/server handles closed normally. Evidence /tmp/crepus-webext-popup-includes/{webext.before.rs,test.before.rs,red.log,green.log,clippy.log,browser.py,browser.log,docs.before.md}. No full workspace/device/release gate claimed.

No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal remains active.


## Build and macro frontend section boundaries — 2026-10-19

Previous goal turn completed authored popup include/title fixes with CLI/browser gates. Revalidated repository instructions/current source and MEMORY.md549 scoped guidance. Parallel core/macro investigations found both build validation and reference collection tried DSL component parsing before filename frontend dispatch. Core accepted malformed markup containing a section marker; macro collection rejected valid Vue literal marker text. Build file admission also skipped mixed-case frontend filenames despite runtime support.

Delegated tests-only ownership in core/build.rs and macros/lib.rs; primary retained architecture/production/integration. Core initial terminal red8passed/1failed; expanded uppercase/content-JSX controls red7passed/2failed. Macro terminal red4passed/1failed. Added shared parser::is_markup_mode using existing dispatch predicates and BOM-aware JSX detection. Build validation now interprets DSL sections only for indentation input and recognizes mixed-case supported extensions. Macro reference traversal retains explicit component-selector semantics and ordinary multi-component discovery, but parses markup directly through its selected frontend. No parser syntax admission changes.

Final core build tests9passed; full all-feature macro library37passed. Strict all-target/all-feature Clippy for both crates passed. Complete all-feature core library run reached552passed/2failed: existing concurrent Svelte spread-binding negatives files_binding_rejects_non_file_dynamic_duplicate_spread_and_explicit_files and open_binding_rejects_other_dynamic_conflicting_duplicate_spread_and_invalid_targets expect rejection of spreads now admitted by concurrent authored compiler work. Those direct authored parser tests do not call the changed build/macro paths. No full core/workspace green claim and no unrelated expectation repair. Initial combined core/macro command stopped after core failure, so macro library was explicitly run separately to terminal37pass.

Primary inspected scoped production/test/docs deltas; independent final review found no blocker. Restored incidental parser import reordering after rustfmt. Formatting parser/mod.rs also traversed the concurrent untracked svelte/authored.rs; retained that source rather than restoring an unavailable baseline or risking concurrent semantic edits. No intentional authored parser behavior changes. Docs explain frontend/section distinction and mixed-case validation. Evidence /tmp/crepus-build-frontend-boundary/{build.before.rs,parser.before.rs,red.log,red-expanded.log,full-lib.log,green.log,clippy.log,docs.before.md} and /tmp/crepus-macro-frontend-boundary/{lib.before.rs,red.log,green.log}. Scoped whitespace checks passed. No browser/device/release gate claimed for these compile-time changes.

No new dependencies, commits, pushes, branches, deployments, destructive cleanup or remote mutations. Concurrent work preserved; goal remains active with the separately recorded Svelte acceptance gaps.


## Directory cycles and publication handoff — 2026-10-19

Finished the pending build traversal fix before publication. Tests-only baseline10passed/2failed reproduced repeated ancestor traversal ending in ELOOP and masking an invalid sibling. Private active-canonical-directory tracking now skips only ancestor cycles, restores the set after traversal, retains sorted lexical paths/rebuild directives and validates sibling aliases. All12build tests passed, including authored Vue file-alias identity. Evidence /tmp/crepus-build-directory-cycles/{build.before.rs,red.log,green.log}.

User explicitly authorized logical commits, pushing to main and publishing, then refined delivery to grouped PRs with review and merge before publication. This authorization supersedes the earlier local-only boundary for the current release task. Keep normal commit timestamps and reference verified recorded work dates; do not fabricate historical commit times.

Prepared immutable tracked patch/untracked archive in /tmp/crepus-release-checkpoint-20261019. Local subsystem checkpoints include all required source/tests and preserve backup/cache files on disk without staging them. Full current core/macro acceptance873passed/0failed/4documentation examples ignored; strict all-feature/all-target Clippy passed. Reconciled four stale Svelte spread-negative expectations against existing intrinsic-spread implementation/native positive contracts; added positive ordering/binding-marker assertions and retained malformed/unsupported controls. Snapshots /tmp/crepus-release-svelte-tests; full logs /tmp/crepus-release-checkpoint-20261019/{core-macro-tests.log,core-macro-clippy.log}.

Registry inventory confirmed occupied versions and newer published versions for eight crates; colors is unpublished and npmWASM latest0.1.3. Remote main contains162commits absent from the prior local pin branch, while that branch has5notonmain. Main already uses registry GPUI dependencies, requiring explicit integration rather than retaining the old git-only publication blocker. No force overwrite, merged PR or package publication has occurred at this checkpoint. Required remaining release work: integrate main without losing either side, arrange focused PRs/reviews and green acceptance, coordinate fresh package/dependency versions, merge and publish validated artifacts, then verify actual registry/release state.
