import { describe, expect, test, beforeEach } from "bun:test"
import { CrepusViewSession, crepusBin, renderHtml, renderIr } from "./crepuscularity"

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
})

test("CrepusViewSession dispatches bind events and rerenders", async () => {
  const fixture = new URL("../fixtures/interactive.crepus", import.meta.url).pathname
  const session = new CrepusViewSession(fixture, { count: "1" })
  expect(await session.renderHtml()).toContain("Count 1")
  const ir = await session.dispatch("bind:count:2")
  expect(JSON.stringify(ir)).toContain("Count 2")
  expect(await session.renderHtml()).toContain("Count 2")
})
