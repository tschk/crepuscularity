# Crepuscularity Reference Plugins

These plugins demonstrate the polyglot contract: spawn `crepus native ir`, read View IR JSON from stdout, decode it into host-language types, and create UI from that tree. The portable reference UI output is HTML; platform-specific packages can map the same node model to native controls. For in-process sessions and event dispatch, wrap `crates/crepuscularity-abi/include/crepuscularity_abi.h`. Native workspace files live next to the reference packages so a host project can drop in the matching package and use Crepus without adopting Rust.

## Shared packages in this tree

| Path | Role |
|------|------|
| `crepuscularity-flutter/` | Flutter runtime renderer (`crepus flutter dep` / `add`) |
| `crepuscularity-components/` | Shared UI component catalog + themes (`crepus components list` / `add` / `themes`) |

Moonshine is a separate product ([`github.com/tschk/moonshine`](https://github.com/tschk/moonshine): `@tschk/moonshine`, `@tschk/crepus-moonshine`, `@tschk/moonshine-components`). Use `crepus moonshine new` / `crepus moonshine dep` from the CLI. Local `packages/moonshine*` trees are legacy bridges.

Workspace entrypoints:

```bash
go test ./plugins/go/...
bun --cwd plugins test
zig build test --build-file plugins/build.zig
swift build --package-path plugins
dotnet build plugins/CrepuscularityPlugins.sln --nologo --verbosity:minimal
cargo test --manifest-path plugins/rust/Cargo.toml
```

`plugins/crepuscularity-plugins.toml` lists every reference package, workspace file, and smoke command.

Build the CLI first:

```bash
cargo build -p crepuscularity-cli --no-default-features
export CREPUS_BIN="$PWD/target/debug/crepus"
```

Build the optional ABI runtime when testing in-process sessions:

```bash
cargo build -p crepuscularity-abi
export CREPUS_ABI_LIB="$PWD/target/debug/libcrepuscularity_abi.dylib" # macOS
```

Run the full local smoke matrix:

```bash
scripts/plugin-smoke.sh
```

Smoke commands for installed local toolchains:

```bash
python3 -m unittest discover plugins/python
go test ./plugins/go/...
bun --cwd plugins test
v -gc none test plugins/v
zig build test --build-file plugins/build.zig
cargo test --manifest-path plugins/rust/Cargo.toml
cargo test -p crepuscularity-abi
swift build --package-path plugins
gcc plugins/c/crepuscularity_plugin.c -o /tmp/crepus-c-smoke && /tmp/crepus-c-smoke plugins/fixtures/hello.crepus
g++ plugins/cpp/crepuscularity_plugin.cpp -std=c++17 -o /tmp/crepus-cpp-smoke && /tmp/crepus-cpp-smoke plugins/fixtures/hello.crepus
ruby -Iplugins/ruby -e 'require "crepuscularity_plugin"; p CrepuscularityPlugin.render_ir("plugins/fixtures/hello.crepus").version'
ruby -Iplugins/ruby -e 'require "crepuscularity_plugin"; raise unless CrepuscularityPlugin.render_html("plugins/fixtures/hello.crepus").include?("data-crepus-kind")'
```

Optional when the toolchains are installed:

```bash
dotnet build plugins/CrepuscularityPlugins.sln --nologo --verbosity:minimal
javac -d /tmp/crepus-java plugins/java/CrepuscularityPlugin.java
kotlinc plugins/kotlin/CrepuscularityPlugin.kt -d /tmp/crepus-kotlin
php -r 'require "plugins/php/CrepuscularityPlugin.php"; var_dump(CrepuscularityPlugin::renderIr("plugins/fixtures/hello.crepus")["version"]);'
```

ABI-backed session wrappers currently exist for C, Python, PHP, and C#. The wrappers expose the shared session lifecycle, context JSON, IR rendering, and event dispatch path. Other language references remain CLI-backed until their package-specific native loading story is promoted from smoke coverage to maintained SDK surface.
