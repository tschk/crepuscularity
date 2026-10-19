import { expect, test } from "bun:test"
import { chmod, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { renderIr } from "./crepuscularity"

test("renderIr rejects invalid template UTF-8 before invoking the CLI", async () => {
  const directory = await mkdtemp(join(tmpdir(), "crepus-wrapper-utf8-"))
  const previous = process.env.CREPUS_BIN
  try {
    const source = join(directory, "view.crepus")
    const marker = join(directory, "invoked")
    const executable = join(directory, "crepus")
    // Labelled CLI stub records transport only; it does not parse templates.
    await writeFile(executable, `#!${process.execPath}
await Bun.write(${JSON.stringify(marker)}, "invoked")
const payload = await new Response(Bun.stdin.stream()).json()
console.log(JSON.stringify({version:1,root:[{kind:"text",content:payload.template}]}))
`)
    await chmod(executable, 0o755)
    process.env.CREPUS_BIN = executable
    await writeFile(source, new Uint8Array([115, 112, 97, 110, 32, 34, 255, 34]))
    await expect(renderIr(source)).rejects.toThrow()
    expect(await Bun.file(marker).exists()).toBe(false)

    // Keep the existing Bun text-decoding behavior for valid Unicode and BOM.
    const valid = '\uFEFFspan "雪 🦋"\r\n'
    await writeFile(source, valid)
    const decoded = await Bun.file(source).text()
    const ir = await renderIr(source)
    expect(ir.root[0].content).toBe(decoded)
    expect(String(ir.root[0].content)).toContain("雪 🦋")
    expect(await readFile(marker, "utf8")).toBe("invoked")
  } finally {
    if (previous === undefined) delete process.env.CREPUS_BIN
    else process.env.CREPUS_BIN = previous
    await rm(directory, { recursive: true, force: true })
  }
})
