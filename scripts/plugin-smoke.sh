#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo build -p crepuscularity-cli --no-default-features
cargo build -p crepuscularity-abi

ABI_LINK_ARGS=()
case "$(uname -s)" in
  Darwin) ABI_LIB="$ROOT/target/debug/libcrepuscularity_abi.dylib"; ABI_LINK_ARGS=("-Wl,-rpath,$ROOT/target/debug") ;;
  Linux) ABI_LIB="$ROOT/target/debug/libcrepuscularity_abi.so" ;;
  MINGW*|MSYS*|CYGWIN*) ABI_LIB="$ROOT/target/debug/crepuscularity_abi.dll" ;;
  *) ABI_LIB="$ROOT/target/debug/libcrepuscularity_abi.so" ;;
esac

export PATH="$ROOT/target/debug:$PATH"
export CREPUS_BIN=crepus
export CREPUS_ABI_LIB="$ABI_LIB"

python3 -m unittest discover plugins/python
go test ./plugins/go/...
bun --cwd plugins test
if command -v v >/dev/null 2>&1; then
  v -gc none test plugins/v
else
  echo "skipping V plugin (not installed)"
fi
zig build test --build-file plugins/build.zig
cargo test --manifest-path plugins/rust/Cargo.toml
swift build --package-path plugins
swift test --package-path plugins
gcc plugins/c/crepuscularity_plugin.c -o /tmp/crepus-c-smoke
/tmp/crepus-c-smoke plugins/fixtures/hello.crepus
gcc plugins/c/abi_smoke.c "$ABI_LIB" "${ABI_LINK_ARGS[@]}" -o /tmp/crepus-c-abi-smoke
/tmp/crepus-c-abi-smoke
RUST_ABI_LINK_ARGS=(-C "link-arg=$ABI_LIB")
for argument in "${ABI_LINK_ARGS[@]}"; do
  RUST_ABI_LINK_ARGS+=(-C "link-arg=$argument")
done
rustc --edition=2021 -Dwarnings plugins/rust/abi_smoke.rs "${RUST_ABI_LINK_ARGS[@]}" -o /tmp/crepus-rust-abi-smoke
/tmp/crepus-rust-abi-smoke
g++ plugins/cpp/crepuscularity_plugin.cpp -std=c++17 -o /tmp/crepus-cpp-smoke
/tmp/crepus-cpp-smoke plugins/fixtures/hello.crepus
python3 scripts/test-c-pipes.py /tmp/crepus-c-smoke /tmp/crepus-cpp-smoke
python3 scripts/test-c-html.py
ruby -Iplugins/ruby -e 'require "crepuscularity_plugin"; raise unless CrepuscularityPlugin.render_ir("plugins/fixtures/hello.crepus").version == 7; raise unless CrepuscularityPlugin.render_html("plugins/fixtures/hello.crepus").include?("data-crepus-kind")'
ruby -Iplugins/ruby -e 'Dir.glob("plugins/ruby/tests/test_*.rb").sort.each { |path| require File.expand_path(path) }'
for plugin_test in plugins/php/*_smoke.php; do
  php "$plugin_test"
done
php -r 'require "plugins/php/CrepuscularityPlugin.php"; if (CrepuscularityPlugin::renderIr("plugins/fixtures/hello.crepus")["version"] !== 7) exit(1);'
php -r 'require "plugins/php/CrepuscularityAbi.php"; $s = new CrepuscularityAbiSession(); $s->setTemplate("input bind=count\nspan\n  \"Count {count}\""); $s->setContext(["count" => "1"]); $r = $s->dispatchEvent(["handler" => "bind:count:2"]); if (strpos(json_encode($r), "Count 2") === false) exit(1);'
javac -d /tmp/crepus-java plugins/java/CrepuscularityPlugin.java plugins/java/CrepuscularityPluginTest.java plugins/java/CrepuscularityHtmlTest.java plugins/java/CrepuscularityUnicodeTest.java
java -cp /tmp/crepus-java dev.crepuscularity.plugin.CrepuscularityPluginTest plugins/fixtures/hello.crepus
java -cp /tmp/crepus-java dev.crepuscularity.plugin.CrepuscularityHtmlTest
java -cp /tmp/crepus-java dev.crepuscularity.plugin.CrepuscularityUnicodeTest
kotlinc plugins/kotlin/CrepuscularityPlugin.kt plugins/kotlin/CrepuscularityPluginTest.kt plugins/kotlin/CrepuscularityHtmlTest.kt plugins/kotlin/CrepuscularityUnicodeTest.kt -d /tmp/crepus-kotlin
kotlin -classpath /tmp/crepus-kotlin dev.crepuscularity.plugin.CrepuscularityPluginTestKt plugins/fixtures/hello.crepus
kotlin -classpath /tmp/crepus-kotlin dev.crepuscularity.plugin.CrepuscularityHtmlTestKt
kotlin -classpath /tmp/crepus-kotlin dev.crepuscularity.plugin.CrepuscularityUnicodeTestKt
python3 scripts/test-plugin-diagnostics.py /tmp/crepus-java /tmp/crepus-kotlin
export DOTNET_CLI_TELEMETRY_OPTOUT=1
dotnet build plugins/CrepuscularityPlugins.sln --nologo --verbosity:minimal
dotnet run --project plugins/csharp-smoke -- plugins/fixtures/hello.crepus
dotnet run --project plugins/csharp-smoke -- --input-html plugins/fixtures
dotnet run --project plugins/csharp-smoke -- --link-html
dotnet run --project plugins/csharp-smoke -- --transport
dotnet run --project plugins/csharp-smoke -- --abi-nul
dotnet run --project scripts/fixtures/csharp-abi-lifetime -- "$ABI_LIB"
dotnet run --project scripts/fixtures/csharp-generated-abi -- "$ABI_LIB"
