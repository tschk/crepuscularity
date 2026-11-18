import { expect, test } from "bun:test"
import { mkdtemp, rm, writeFile, chmod } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { CrepusViewSession, renderIr } from "./crepuscularity"

test("renderIr yields to its caller and drains output while sending a large request", async () => {
  const directory = await mkdtemp(join(tmpdir(), "crepus-transport-"))
  const previous = process.env.CREPUS_BIN
  const source = join(directory, "source.crepus")
  const signal = join(directory, "signal")
  const binary = join(directory, "crepus")
  const template = 'span "雪🦋\r\nline"' + " ".repeat(262144)
  let timer: ReturnType<typeof setTimeout> | undefined
  try {
    await writeFile(source, template)
    await writeFile(binary, `#!/usr/bin/env python3
import json,sys,time,pathlib
sys.stderr.buffer.write(b'diagnostic' * 32768)
sys.stderr.buffer.flush()
request = json.load(sys.stdin)
for _ in range(200):
    if pathlib.Path(${JSON.stringify(signal)}).exists(): break
    time.sleep(0.01)
else:
    sys.stderr.write('caller event loop blocked')
    sys.exit(9)
print(json.dumps({'version':7,'root':[{'kind':'text','content':request['template']},
    {'kind':'text','content':request['filename']}, {'kind':'text','content':request['baseDir']}]}))
`)
    await chmod(binary, 0o700)
    process.env.CREPUS_BIN = binary
    timer = setTimeout(() => { void writeFile(signal, "ready") }, 10)
    const ir = await renderIr(source)
    expect(ir.root[0].content).toBe(template)
    expect(ir.root[1].content).toBe(source)
    expect(ir.root[2].content).toBe(directory)
  } finally {
    clearTimeout(timer)
    if (previous === undefined) delete process.env.CREPUS_BIN
    else process.env.CREPUS_BIN = previous
    await rm(directory, { recursive: true, force: true })
  }
})

test("renderIr rejects malformed UTF-8 instead of silently replacing bytes", async () => {
  const directory = await mkdtemp(join(tmpdir(), "crepus-utf8-"))
  const previous = process.env.CREPUS_BIN
  const source = join(directory, "source.crepus")
  const binary = join(directory, "crepus")
  try {
    await writeFile(source, new Uint8Array([255]))
    await expect(renderIr(source)).rejects.toThrow()
    await writeFile(source, 'span "valid"')
    await writeFile(binary, `#!/usr/bin/env python3
import sys
sys.stdin.buffer.read()
sys.stdout.buffer.write(b'{"version":7,"root":[{"kind":"text","content":"' + bytes([255]) + b'"}]}')
`)
    await chmod(binary, 0o700)
    process.env.CREPUS_BIN = binary
    await expect(renderIr(source)).rejects.toThrow()
  } finally {
    if (previous === undefined) delete process.env.CREPUS_BIN
    else process.env.CREPUS_BIN = previous
    await rm(directory, { recursive: true, force: true })
  }
})

test("binding keys remain serializable own properties without changing the context prototype", async () => {
  const fixture = new URL("../fixtures/interactive.crepus", import.meta.url).pathname
  const session = new CrepusViewSession(fixture, { count: "1" })
  const context = session.context
  const prototype = Object.getPrototypeOf(context)
  await session.dispatch("bind:__proto__:literal")
  await session.dispatch("bind:count")
  expect(session.context).toBe(context)
  expect(Object.getPrototypeOf(context)).toBe(prototype)
  expect(Object.hasOwn(context, "__proto__")).toBe(true)
  expect(JSON.parse(JSON.stringify(context))["__proto__"]).toBe("literal")
  expect(context.count).toBe("1")
  await session.dispatch("bind:count:")
  expect(context.count).toBe("")
})
