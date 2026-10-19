import { expect, test } from "bun:test";

// Execute the generated page body; mock only browser/native-module collaborators.
const rust = await Bun.file(new URL("../../crepuscularity-cli/src/webext.rs", import.meta.url)).text();
const match = rust.match(/r#"import init, \* as runtime from [\s\S]*?"#,/);
if (!match) throw new Error("generated extension page script missing");
const body = match[0].slice(3, -3).split("\n").slice(1).join("\n")
  .replaceAll("{{", "{").replaceAll("}}", "}")
  .replaceAll("{prefix}", "../../")
  .replaceAll("{main_export}", '"options_main"')
  .replaceAll("{render_export}", '"render_options"');
const run = new (Object.getPrototypeOf(async function () {}).constructor)("runtime", "init", "document", "fetch", body);
const fetchWasm = async () => ({ arrayBuffer: async () => new ArrayBuffer(0) });

const failure = new Error("ordinary failure");
for (const [label, value, expected] of [
  ["Error stack", failure, failure.stack],
  ["string", "failed", "failed"],
  ["unprintable", Object.create(null), "Runtime threw an unprintable value"],
  ["throwing getters", new Proxy({}, { get() { throw new Error("getter failed"); } }), "Runtime threw an unprintable value"],
  ["broken stack", { get stack() { throw new Error("stack failed"); }, toString() { return "printable"; } }, "printable"],
]) {
  test(`page displays ${label} runtime failures`, async () => {
    const root = { textContent: "before" };
    await run({ options_main() { throw value; } }, async () => {}, { getElementById: () => root }, fetchWasm);
    expect(root.textContent).toBe(expected);
  });
}

test("page without a root preserves the original failure", async () => {
  const failure = Object.create(null);
  await expect(run({ options_main() { throw failure; } }, async () => {}, { getElementById: () => null }, fetchWasm)).rejects.toBe(failure);
});
