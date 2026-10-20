// Emit ESM sidecars without bundling or changing the wasm-bindgen glue.
const directory = new URL("./npm/", import.meta.url);
const transpiler = new Bun.Transpiler({ loader: "ts", target: "browser" });
for (const name of ["index", "browser", "parser", "types"]) {
  const source = await Bun.file(new URL(`${name}.ts`, directory)).text();
  await Bun.write(new URL(`${name}.js`, directory), transpiler.transformSync(source));
}
