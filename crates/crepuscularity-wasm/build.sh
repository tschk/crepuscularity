#!/usr/bin/env bash
set -euo pipefail

crate_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_root="$(cd "$crate_dir/../.." && pwd)"

# Resolve the generator from this crate's locked dependency, not an unrelated
# workspace member, and honor Cargo's configured target directory.
metadata="$(cargo metadata --locked --manifest-path "$workspace_root/Cargo.toml" --format-version 1)"
crate_version="$(printf '%s' "$metadata" | bun -e '
const metadata = await Bun.stdin.json();
const owner = metadata.packages.find(pkg => pkg.name === "crepuscularity-wasm" && metadata.workspace_members.includes(pkg.id));
const dependency = metadata.resolve.nodes.find(node => node.id === owner?.id)?.deps.find(dep => dep.name === "wasm_bindgen");
const version = metadata.packages.find(pkg => pkg.id === dependency?.pkg)?.version;
if (!version) throw new Error("cannot resolve crepuscularity-wasm wasm-bindgen dependency");
console.log(version);
')"
target_dir="$(printf '%s' "$metadata" | bun -e 'console.log((await Bun.stdin.json()).target_directory)')"
cli_version="$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')"
if [ "$crate_version" != "$cli_version" ]; then
    echo "wasm-bindgen CLI $cli_version != crate $crate_version; use a matching CLI on PATH" >&2
    exit 1
fi

cargo build --locked --manifest-path "$workspace_root/Cargo.toml" -p crepuscularity-wasm --target wasm32-unknown-unknown --release
wasm="$target_dir/wasm32-unknown-unknown/release/crepuscularity_wasm.wasm"

wasm-bindgen "$wasm" --out-dir "$crate_dir/npm/wasm" --target nodejs
# The parent npm package is ESM; wasm-bindgen's Node output is CommonJS.
printf '{"type":"commonjs"}\n' > "$crate_dir/npm/wasm/package.json"
wasm-bindgen "$wasm" --out-dir "$crate_dir/pkg" --target nodejs
wasm-bindgen "$wasm" --out-dir "$crate_dir/npm/browser" --target web

# Ship JavaScript entry points: Node does not strip TypeScript in node_modules.
bun build "$crate_dir/npm/index.ts" --target node --external './wasm/*' --outdir "$crate_dir/npm"
bun build "$crate_dir/npm/browser.ts" --target browser --external './browser/*' --outdir "$crate_dir/npm"

# Release optimization may erase Rust source paths. Exercise the generated
# bindings instead of guessing parser availability from incidental WASM bytes.
node "$crate_dir/npm/frontend-smoke.mjs"
bun test "$crate_dir/npm/parser.test.ts"

echo "built Node, browser and legacy bindings in $crate_dir"
