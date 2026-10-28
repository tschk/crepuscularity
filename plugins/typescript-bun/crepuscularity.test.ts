import { describe, expect, test, beforeEach, afterEach } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { CrepusViewSession, crepusBin, renderHtml, renderIr } from "./crepuscularity"

test("context rendering preserves frontend filename", async () => {
  const fixture = new URL("../fixtures/frontend.svelte", import.meta.url).pathname
  const ir = JSON.stringify(await renderIr(fixture, { name: "Ada" }))
  expect(ir).toContain("Frontend content")
  expect(ir).not.toContain("ignored")
})

test("relative includes resolve beside the template", async () => {
  const directory = await mkdtemp(join(tmpdir(), "crepus-ts-include-"))
  try {
    const source = join(directory, "index.crepus")
    await Bun.write(source, "include part.crepus name={name}\n")
    await Bun.write(join(directory, "part.crepus"), 'span\n  "Included {name}"\n')
    expect(JSON.stringify(await renderIr(source, { name: "Ada" }))).toContain("Included Ada")
  } finally {
    await rm(directory, { recursive: true })
  }
})

test("renderIr decodes ViewIr", async () => {
  const fixture = new URL("../fixtures/hello.crepus", import.meta.url).pathname
  const ir = await renderIr(fixture, { name: "Ada" })
  expect(ir.version).toBe(7)
  expect(ir.root.length).toBe(1)
  expect(await renderHtml(fixture, { name: "Ada" })).toBe('<div data-crepus-kind="stack" data-axis="column">Hello Ada</div>')
})

describe("crepusBin", () => {
  const ORIG = process.env.CREPUS_BIN
  beforeEach(() => { process.env.CREPUS_BIN = ORIG })
  afterEach(() => {
    if (ORIG === undefined) delete process.env.CREPUS_BIN
    else process.env.CREPUS_BIN = ORIG
  })

  test("accepts absolute path", () => {
    process.env.CREPUS_BIN = "/usr/local/bin/crepus"
    expect(crepusBin()).toBe("/usr/local/bin/crepus")
  })

  test("accepts simple binary name", () => {
    process.env.CREPUS_BIN = "crepus"
    expect(crepusBin()).toBe("crepus")
  })

  test("rejects relative path with directory", () => {
    process.env.CREPUS_BIN = "../evil"
    expect(() => crepusBin()).toThrow("Security Error")
  })

  test("defaults to crepus when unset", () => {
    delete process.env.CREPUS_BIN
    expect(crepusBin()).toBe("crepus")
  })

  test("renderIr preserves a missing executable error", async () => {
    const directory = await mkdtemp(join(tmpdir(), "crepus-ts-missing-bin-"))
    try {
      process.env.CREPUS_BIN = join(directory, "crepus")
      const fixture = new URL("../fixtures/hello.crepus", import.meta.url).pathname
      await expect(renderIr(fixture)).rejects.toThrow("ENOENT")
    } finally {
      await rm(directory, { recursive: true })
    }
  })
})

test("CrepusViewSession dispatches bind events and rerenders", async () => {
  const fixture = new URL("../fixtures/interactive.crepus", import.meta.url).pathname
  const session = new CrepusViewSession(fixture, { count: "1" })
  expect(await session.renderHtml()).toContain("Count 1")
  const ir = await session.dispatch("bind:count:2")
  expect(JSON.stringify(ir)).toContain("Count 2")
  expect(await session.renderHtml()).toContain("Count 2")
})

test("bind events require a value separator but allow empty values", async () => {
  const fixture = new URL("../fixtures/interactive.crepus", import.meta.url).pathname
  const session = new CrepusViewSession(fixture, { count: "1" })
  await session.dispatch("bind:count")
  expect(session.context.count).toBe("1")
  await session.dispatch("bind:count:")
  expect(session.context.count).toBe("")
  await session.dispatch("bind:count:12:30")
  expect(session.context.count).toBe("12:30")
})
