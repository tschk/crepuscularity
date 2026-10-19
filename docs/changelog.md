# Changelog

Release notes for Crepuscularity crates, the `crepus` CLI, and the documentation site. GitHub Releases (`v*`) ship CLI binaries only; libraries go to [crates.io](https://crates.io/crates/crepuscularity).

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
