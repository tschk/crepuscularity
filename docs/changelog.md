# Changelog

Release notes for Crepuscularity crates, the `crepus` CLI, and the documentation site. GitHub Releases (`v*`) ship CLI binaries only; libraries go to [crates.io](https://crates.io/crates/crepuscularity).

## 2026-10-19 — CLI 0.16.5 coordinated release

Work recorded on 19 October 2026 in Australia/Melbourne; Git commits retain their actual timestamps. Publication status is tracked separately by the registry and GitHub releases.

- Require ammonia 4.1.8 for the patched HTML sanitizer ([#531](https://github.com/tschk/crepuscularity/pull/531)).
- Bound the parser AST cache and reconcile template watch roots ([#528](https://github.com/tschk/crepuscularity/pull/528)).
- Prepare hot-reload templates, contexts and include graphs on background workers; retain the last coherent prepared graph during reload. Rendering, evaluation, layout, painting and application callbacks still run on the UI thread. This is not a guarantee against frame stalls ([#540](https://github.com/tschk/crepuscularity/pull/540)).
- Pair native platform font backends with default GPUI font support, and emit valid desktop scaffold manifests ([#541](https://github.com/tschk/crepuscularity/pull/541)).
- Distinguish literal, expression and attribute bindings throughout supported render targets. **Rust migration:** core 0.6 adds `Binding.origin`; update struct initializers and exhaustive matches, and use the matching library versions in this release ([#534](https://github.com/tschk/crepuscularity/pull/534)).
- Validate typed Rust, C#, D, Nim and Odin bindings against the selected C header. Reject unsupported declarations before writing output; keep optional external compiler checks explicit ([#535](https://github.com/tschk/crepuscularity/pull/535)).
- Deduplicate TUI focus targets and guard cyclic event bubbling ([#536](https://github.com/tschk/crepuscularity/pull/536)).
- ZPUI adapter 0.2.1 retains callback data beyond a frame, validates collection keys in expected linear time, and admits the editor into host-managed Tab focus traversal ([#530](https://github.com/tschk/crepuscularity/pull/530), [#532](https://github.com/tschk/crepuscularity/pull/532), [#537](https://github.com/tschk/crepuscularity/pull/537)). Documents must still outlive listeners. The adapter remains experimental.
- Add an optional, unpublished GPUI Kit adapter with its own pinned GPUI family ([#518](https://github.com/tschk/crepuscularity/pull/518)).
- Isolate Pages CI per pull request and use static executable Java probe fixtures ([#538](https://github.com/tschk/crepuscularity/pull/538), [#539](https://github.com/tschk/crepuscularity/pull/539)).

The C ABI and View IR version remain unchanged. npm parser package 0.1.4 rebuilds the WASM artifact from the coordinated sources. Publish runtime 0.5.0 before GPUI 0.6.0; the latter requires the new runtime font feature.

## 2026-09-16

### Security

- Docs search no longer concatenates index titles or paths into `innerHTML`. Results are built with DOM APIs and `textContent`; hrefs must be relative.
- `crepus web dev` now canonicalizes `/pkg`, `/docs`, and `/islands` the same way as static files, blocking symlink escapes.
- Webext `sanitizeHTML` drops `style` and `srcdoc` in addition to event-handler attributes.
- Generated Markdown docs run through ammonia so raw HTML in `.md` cannot inject scripts.
- Inauguration `processRun` only accepts the `in` basename and runs with the sandbox working directory.
- `rustls` 0.23.45 ([RUSTSEC-2026-0285](https://rustsec.org/advisories/RUSTSEC-2026-0285)).
- CI `security-audit` fails the job instead of continuing on error.

### Crates

| Crate | Version | Notes |
| --- | --- | --- |
| `crepuscularity-cli` | 0.16.2 | Desktop deps are crates.io `gpui-ce` / `gpui_ce_platform` (publishable) |
| `crepuscularity-runtime` | 0.4.20 | Same gpui-ce crates.io deps |
| `crepuscularity-gpui` | 0.5.10 | Same gpui-ce crates.io deps |
| `crepuscularity-lite` | 0.4.15 | Same gpui-ce crates.io deps |
| `crepuscularity-webext` | 0.3.6 | `sanitizeHTML` attribute allowlist |
| `crepuscularity-wasm` | 0.1.1 | Parser/View IR WASM bindings |

### Docs site

- This changelog is generated with the rest of `docs/` by `crepus web build`.

Earlier GitHub binary tags: [v0.11.1](https://github.com/tschk/crepuscularity/releases/tag/v0.11.1) (2026-07-30).
