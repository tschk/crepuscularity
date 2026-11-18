# Example Usage: Crepuscularity Action

To use the crepuscularity setup action in your own GPUI project:

```yaml
name: Build

on: [push, pull_request]

jobs:
  build:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Crepuscularity
        uses: softwarecorporation/crepuscularity/.github/actions/setup-crepuscularity@main
        with:
          rust-version: stable
          cache: true
          wasm: true
          install-cli: true
      
      - name: Build your project
        run: cargo build --release
      
      - name: Run tests
        run: cargo test
```

## Inputs

| Input | Description | Default |
|-------|-------------|---------|
| `rust-version` | Rust toolchain version | `stable` |
| `cache` | Enable cargo caching | `true` |
| `install-cli` | Install the `crepus` CLI from this repository | `false` |
| `wasm` | Install `wasm32-unknown-unknown` and `wasm-bindgen-cli` | `false` |
| `wasm-bindgen-version` | `wasm-bindgen-cli` version | `0.2.121` |

## GitHub Pages

For a static Crepus site, add this workflow to `.github/workflows/pages.yml` and set `CREPUS_MANIFEST` and `CREPUS_TARGET` for your project:

```yaml
name: Crepus Pages

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: crepus-pages
  cancel-in-progress: true

env:
  CREPUS_MANIFEST: crepus.toml
  CREPUS_TARGET: site

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Configure Pages
        if: github.event_name != 'pull_request'
        uses: actions/configure-pages@v5

      - name: Setup Crepuscularity
        uses: softwarecorporation/crepuscularity/.github/actions/setup-crepuscularity@main
        with:
          rust-version: stable
          cache: true
          wasm: true
          install-cli: true

      - name: Build site
        run: crepus web build --manifest "$CREPUS_MANIFEST" --target "$CREPUS_TARGET"

      - name: Verify site artifact
        run: |
          test -f dist/index.html
          test -f dist/.nojekyll
          test -d dist/pkg

      - uses: actions/upload-pages-artifact@v3
        with:
          path: dist

  deploy:
    if: github.event_name != 'pull_request'
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

## What it does

1. Installs Rust toolchain
2. Sets up SDKROOT on macOS for GPUI
3. Configures cargo caching
4. Optionally installs WASM web build tooling
5. Optionally installs the `crepus` CLI
6. Verifies installation
