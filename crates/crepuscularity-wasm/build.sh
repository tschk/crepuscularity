#!/usr/bin/env bash
set -euo pipefail

crate_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
workspace_root="$(cd "$crate_dir/../.." && pwd)"

# The generator refuses to run when its schema differs from the wasm-bindgen
# crate's, and an unrelated `cargo install` can swap the binary underneath us.
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

mkdir -p "$crate_dir/npm/wasm"
wasm-bindgen \
    "$target_dir/wasm32-unknown-unknown/release/crepuscularity_wasm.wasm" \
    --out-dir "$crate_dir/npm/wasm" \
    --target nodejs

rm -f "$crate_dir/npm/wasm/.gitignore"
# The nodejs target emits CommonJS glue inside this ESM npm package.
printf '{"type":"commonjs"}\n' > "$crate_dir/npm/wasm/package.json"

# A binding layer that silently lacks a frontend is worse than one that fails:
# an extension it does not know falls through to generic markup and returns
# plausible but wrong IR. The expected set is read from the parser directory
# rather than listed here, so a new frontend is required without editing this.
missing=""
for frontend_dir in "$workspace_root/crates/crepuscularity-core/src/parser"/*/; do
    frontend="$(basename "$frontend_dir")"
    if ! grep -aq "parser/$frontend/" "$crate_dir/npm/wasm/crepuscularity_wasm_bg.wasm"; then
        missing="$missing $frontend"
    fi
done
if [ -n "$missing" ]; then
    echo "built wasm is missing parser frontends:$missing" >&2
    exit 1
fi

mkdir -p "$crate_dir/pkg"
cp "$crate_dir/npm/wasm"/crepuscularity_wasm* "$crate_dir/pkg/"

mkdir -p "$crate_dir/npm/browser"
wasm-bindgen \
    "$target_dir/wasm32-unknown-unknown/release/crepuscularity_wasm.wasm" \
    --out-dir "$crate_dir/npm/browser" \
    --target web

bun "$crate_dir/build-js.ts"
bun test "$crate_dir/npm/parser.test.ts" "$crate_dir/npm/package.test.ts"

echo "built $crate_dir/npm/wasm"
