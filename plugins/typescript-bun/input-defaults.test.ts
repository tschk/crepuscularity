import { expect, test } from "bun:test"
import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { CrepusViewSession, renderHtml } from "./crepuscularity"

test("public HTML preserves secure and unbound defaults with textarea parsing compensation", async () => {
  const fixture = JSON.parse(await readFile(new URL("../fixtures/input-defaults.view-ir.json", import.meta.url), "utf8"))
  // Multiline controls must remain textarea elements even with secure=true.
  fixture.root.find((node: Record<string, unknown>) => node.multiline).secure = true
  const directory = await mkdtemp(join(tmpdir(), "crepus-ts-input-defaults-"))
  const previous = process.env.CREPUS_BIN
  try {
    const source = join(directory, "defaults.crepus")
    const executable = join(directory, "crepus")
    await writeFile(source, 'input value="Ada"\n')
    // Labelled CLI stub isolates the public HTML adapter from the Rust parser.
    await writeFile(executable, `#!${process.execPath}\nprocess.stdout.write(${JSON.stringify(JSON.stringify({ version: fixture.version, root: fixture.root }))})\n`)
    await chmod(executable, 0o755)
    process.env.CREPUS_BIN = executable
    const html = await renderHtml(source)
    expect(html).toBe(fixture.expectedHtml)
    expect(await new CrepusViewSession(source).renderHtml()).toBe(html)
    if (process.env.CREPUS_HTML_OUTPUT) await writeFile(process.env.CREPUS_HTML_OUTPUT, html)
  } finally {
    if (previous === undefined) delete process.env.CREPUS_BIN
    else process.env.CREPUS_BIN = previous
    await rm(directory, { recursive: true, force: true })
  }
})
