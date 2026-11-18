# crepuscularity-components

Multi-target dither / primitive / motion component registry for Crepuscularity
(Flutter, Svelte, Moonshine, GPUI, and `.crepus`).

## Ownership / Moonshine

**Moonshine React implementations live in a separate product:**
[`github.com/softwarecorporation/moonshine`](https://github.com/softwarecorporation/moonshine) under `components/`,
published as `@tschk/moonshine-components`.

This plugin tree keeps:

- Catalog + specs (source of truth; synced into `crates/crepuscularity-components`)
- Flutter + Svelte packages (still used by omi and other path-dep consumers)
- A local `packages/moonshine/` package for historical / in-tree tests — prefer
  `@tschk/moonshine-components` for new React work

Rust CLI registry: `crates/crepuscularity-components` (embed via `include_str!`).

## Layout

| Path | Role |
|------|------|
| `catalog/components.json` | Component registry (ids, specs, platforms, themes) |
| `catalog/themes/` | Named theme JSON (7 themes) |
| `specs/` | Per-component source-of-truth specs (44) |
| `packages/{flutter,svelte,moonshine,gpui}/` | Target implementations (Flutter/Svelte kept; Moonshine → softwarecorporation/moonshine) |

## Themes

`dither-kit`, `kumo`, `night`, `chalk`, `aurora`, `dawn`, `zinc`

## CLI

```bash
crepus components list
crepus components themes
crepus components add button --target moonshine
crepus moonshine dep   # @tschk/moonshine* snippets
```

## Sync catalog → Rust crate

```bash
cp plugins/crepuscularity-components/catalog/components.json \
   crates/crepuscularity-components/catalog/components.json
```

## Validate / test

```bash
bun run scripts/validate-catalog.ts
(cd packages/flutter && flutter test)
(cd packages/svelte && bun test)
(cd packages/moonshine && bun test)
cargo test --manifest-path packages/gpui/Cargo.toml
cargo test -p crepuscularity-components
```
