import { expect, test } from "bun:test"
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { renderIr } from "./crepuscularity"

async function withCli(body: string, run: (source: string, directory: string) => Promise<void>) {
  const directory = await mkdtemp(join(tmpdir(), "crepus-async-cli-"))
  const previous = process.env.CREPUS_BIN
  try {
    const executable = join(directory, "crepus")
    const source = join(directory, "source 雪 with spaces.crepus")
    await writeFile(source, 'text "Hello"\n')
    // Labelled executable stub exercises the actual subprocess transport, not Rust parsing.
    await writeFile(executable, `#!${process.execPath}\n${body}\n`)
    await chmod(executable, 0o755)
    process.env.CREPUS_BIN = executable
    await run(source, directory)
  } finally {
    if (previous === undefined) delete process.env.CREPUS_BIN
    else process.env.CREPUS_BIN = previous
    await rm(directory, { recursive: true, force: true })
  }
}

test("renderIr yields while a child waits for a parent event-loop callback", async () => {
  await withCli(`
import { dirname, join } from 'node:path'
const directory = dirname(process.argv[1])
await Bun.write(join(directory, 'ready'), '')
const deadline = Date.now() + 2000
while (!(await Bun.file(join(directory, 'release')).exists())) {
  if (Date.now() >= deadline) { console.error('parent gate was not released'); process.exit(7) }
  await Bun.sleep(5)
}
console.log(JSON.stringify({version:7,root:[{kind:'text',content:'released'}]}))
`, async (source, directory) => {
    let parentCallbackRan = false
    const timer = setInterval(async () => {
      if (parentCallbackRan || !(await Bun.file(join(directory, "ready")).exists())) return
      if (parentCallbackRan) return
      parentCallbackRan = true
      await writeFile(join(directory, "release"), "released by parent")
    }, 5)
    try {
      const ir = await renderIr(source)
      expect(parentCallbackRan).toBe(true)
      expect(ir.root[0].content).toBe("released")
    } finally { clearInterval(timer) }
  })
})

test("renderIr forwards argv and the full UTF-8 stdin envelope while draining both pipes", async () => {
  await withCli(`
const payload = JSON.parse(await Bun.stdin.text())
await Bun.write(Bun.stderr, 'diagnostic '.repeat(32768))
console.log(JSON.stringify({version:7,root:[{kind:'text',content:JSON.stringify({args:process.argv.slice(2),payload,padding:'x'.repeat(262144)})}]}))
`, async (source) => {
    const template = 'text "雪"\n'.repeat(65536)
    await writeFile(source, template)
    const context = { title: 'Ada 雪 "<&', count: 3 }
    const ir = await renderIr(source, context)
    const received = JSON.parse(String(ir.root[0].content))
    expect(received.args).toEqual(["native", "ir", "--stdin-json"])
    expect(received.payload).toEqual({ template, context, filename: source, baseDir: dirname(source) })
    expect(received.padding.length).toBe(262144)
  })
})

test("renderIr preserves a nonzero stderr diagnostic despite large output", async () => {
  await withCli(`
await Bun.stdin.text()
await Bun.write(Bun.stdout, new Uint8Array([0xff]))
await Bun.write(Bun.stdout, 'x'.repeat(262144))
await Bun.write(Bun.stderr, 'specific failure 雪')
process.exit(9)
`, async (source) => {
    await expect(renderIr(source)).rejects.toThrow("specific failure 雪")
  })
})

test("renderIr still rejects malformed JSON after successful child exit", async () => {
  await withCli(`await Bun.stdin.text(); console.log('not JSON')`, async (source) => {
    await expect(renderIr(source)).rejects.toBeInstanceOf(SyntaxError)
  })
})

for (const bytes of [[0xff], [0xc3, 0x28]]) {
  test(`renderIr rejects malformed UTF-8 response bytes ${bytes}`, async () => {
    await withCli(`
await Bun.stdin.text()
await Bun.write(Bun.stdout, Buffer.concat([
  Buffer.from('{"version":7,"root":[{"kind":"text","content":"'),
  Buffer.from(${JSON.stringify(bytes)}),
  Buffer.from('"}]}'),
]))
`, async (source) => {
      await expect(renderIr(source)).rejects.toBeInstanceOf(TypeError)
    })
  })
}

test("renderIr preserves raw Unicode and strips a response UTF-8 BOM", async () => {
  await withCli(`
await Bun.stdin.text()
await Bun.write(Bun.stdout, Buffer.concat([
  Buffer.from([0xef, 0xbb, 0xbf]),
  Buffer.from(JSON.stringify({version:7,root:[{kind:'text',content:'雪 🦋'}]})),
]))
`, async (source) => {
    const ir = await renderIr(source)
    expect(ir.root[0].content).toBe("雪 🦋")
  })
})
