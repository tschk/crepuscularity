import { expect, test } from "bun:test"
import { CrepusViewSession, type ViewIr } from "./crepuscularity"

function sessionWithoutRendering(context: Record<string, unknown> = {}) {
  const session = new CrepusViewSession("unused.crepus", context)
  // Rendering is deliberately stubbed: these tests exercise public dispatch and
  // context serialization, not the CLI transport or Rust template rendering.
  const rendered: ViewIr = { version: 1, root: [] }
  session.renderIr = async () => rendered
  return { session, rendered }
}

test("dispatch creates an own __proto__ value without changing the context prototype", async () => {
  const { session, rendered } = sessionWithoutRendering()
  const prototype = Object.getPrototypeOf(session.context)
  const event = "bind:__proto__:value:with:colons"
  let callbackRan = false
  session.on(event, (payload, current) => {
    expect(payload.handler).toBe(event)
    expect(Object.hasOwn(current.context, "__proto__")).toBe(true)
    expect(current.context.__proto__).toBe("value:with:colons")
    callbackRan = true
  })
  expect(await session.dispatch(event)).toBe(rendered)
  expect(callbackRan).toBe(true)
  expect(Object.hasOwn(session.context, "__proto__")).toBe(true)
  expect(Object.getPrototypeOf(session.context)).toBe(prototype)
  expect(JSON.stringify(session.context)).toBe('{"__proto__":"value:with:colons"}')
})

test("dispatch updates existing own keys and retains blocked context fields", async () => {
  const { session } = sessionWithoutRendering(JSON.parse('{"__proto__":"old","baseDir":"safe","_":"private"}'))
  const prototype = Object.getPrototypeOf(session.context)
  await session.dispatch("bind:__proto__:updated")
  await session.dispatch("bind:baseDir:unsafe")
  await session.dispatch("bind:_:changed")
  expect(Object.hasOwn(session.context, "__proto__")).toBe(true)
  expect(Object.getPrototypeOf(session.context)).toBe(prototype)
  expect(JSON.stringify(session.context)).toBe('{"__proto__":"updated","baseDir":"safe","_":"private"}')
})
