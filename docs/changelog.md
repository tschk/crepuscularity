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
| `crepuscularity-cli` | 0.16.1 | Docs search, path sandbox, Markdown sanitization, changelog page. crates.io publish still blocked by the git `gpui-ce` desktop dep (install from git). |
| `crepuscularity-webext` | 0.3.6 | `sanitizeHTML` attribute allowlist |
| `crepuscularity-lite` | 0.4.14 | Inauguration spawn hardening (git `gpui-ce` dep — not republished to crates.io) |
| `crepuscularity-wasm` | 0.1.1 | Already ahead of crates.io 0.1.0 |
| `crepuscularity-gpui` | 0.5.9 | git `gpui-ce` — not republished to crates.io |

### Docs site

- This changelog is generated with the rest of `docs/` by `crepus web build`.

Earlier GitHub binary tags: [v0.11.1](https://github.com/tschk/crepuscularity/releases/tag/v0.11.1) (2026-07-30).
