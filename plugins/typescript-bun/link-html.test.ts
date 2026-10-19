import { expect, test } from "bun:test"
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { CrepusViewSession, renderHtml, renderIr } from "./crepuscularity"

test("public Link HTML recursively preserves escaped attributes and filters unsafe schemes", async () => {
  const directory = await mkdtemp(join(tmpdir(), "crepus-ts-links-"))
  const previous = process.env.CREPUS_BIN
  try {
    const source = join(directory, "links.crepus")
    const executable = join(directory, "crepus")
    const root = [
      { kind: "link", href: '/雪?q="<&', hrefBind: "scope.next", target: '"_blank', rel: 'next & "safe"', children: [
        { kind: "text", content: 'Help 雪 <&"' },
        { kind: "stack", children: [{ kind: "button", label: "Save", onClick: 'save"&' }] },
      ] },
      { kind: "link", href: "", target: "", rel: null, children: [{ kind: "text", content: "Empty" }] },
      { kind: "link", href: "/plain", children: [] },
      ...["javascript:alert(1)", "\u0000 \tJaVa\nScRiPt:bad()", "VBScript:bad", "data:text/html,bad", "&#106;avascript:literal", "/relative?q=1&x=2", "https://example.test/雪", "mailto:user@example.test"].map(href => ({ kind: "link", href, children: [{ kind: "text", content: "safe" }] })),
    ]
    await writeFile(source, 'a href="/unused"\n')
    // CLI stub isolates the public HTML adapter's View IR contract.
    await writeFile(executable, `#!${process.execPath}\nprocess.stdout.write(${JSON.stringify(JSON.stringify({ version: 7, root }))})\n`)
    await chmod(executable, 0o755)
    process.env.CREPUS_BIN = executable
    const expected = '<a href="/雪?q=&quot;&lt;&amp;" target="&quot;_blank" rel="next &amp; &quot;safe&quot;">Help 雪 &lt;&amp;"<div data-crepus-kind="stack" data-axis="column"><button data-onclick="save&quot;&amp;">Save</button></div></a>' +
      '<a href="" target="">Empty</a><a href="/plain"></a>' +
      '<a>safe</a>'.repeat(4) + '<a href="&amp;#106;avascript:literal">safe</a><a href="/relative?q=1&amp;x=2">safe</a><a href="https://example.test/雪">safe</a><a href="mailto:user@example.test">safe</a>'
    expect(await renderHtml(source)).toBe(expected)
    expect(await new CrepusViewSession(source).renderHtml()).toBe(expected)
    if (process.env.CREPUS_HTML_OUTPUT) await writeFile(process.env.CREPUS_HTML_OUTPUT, expected)
  } finally {
    if (previous === undefined) delete process.env.CREPUS_BIN
    else process.env.CREPUS_BIN = previous
    await rm(directory, { recursive: true, force: true })
  }
})

test("actual CLI Link nodes render through both public HTML entry points", async () => {
  const directory = await mkdtemp(join(tmpdir(), "crepus-ts-link-cli-"))
  try {
    const source = join(directory, "links.crepus")
    await writeFile(source, 'a href="/help" target="_blank" rel="help"\n  span "Help {name}"\n')
    const ir = await renderIr(source, { name: "Ada" })
    expect(ir.root[0].kind).toBe("link")
    const expected = '<a href="/help" target="_blank" rel="help">Help Ada</a>'
    expect(await renderHtml(source, { name: "Ada" })).toBe(expected)
    expect(await new CrepusViewSession(source, { name: "Ada" }).renderHtml()).toBe(expected)
  } finally {
    await rm(directory, { recursive: true, force: true })
  }
})
