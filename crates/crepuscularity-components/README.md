# crepuscularity-components

Rust-side registry for the `crepus components` CLI. Catalog metadata is vendored
from `plugins/crepuscularity-components/catalog/components.json`.

UI implementations live elsewhere:

| Target | Home |
|--------|------|
| React (Moonshine) | [`@tschk/moonshine-components`](https://github.com/softwarecorporation/moonshine) (`components/` in that repo) |
| Flutter / Svelte (legacy in-tree) | `plugins/crepuscularity-components/packages/{flutter,svelte}` |

## Sync

When the plugin catalog changes, copy it into this crate:

```bash
cp plugins/crepuscularity-components/catalog/components.json \
   crates/crepuscularity-components/catalog/components.json
```

## API

```rust
crepuscularity_components::component_ids();
crepuscularity_components::theme_names();
crepuscularity_components::list_components();
```
