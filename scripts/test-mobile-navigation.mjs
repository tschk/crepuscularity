import assert from "node:assert/strict";
import { resolve } from "node:path";
import { chromium, devices } from "playwright";

const dist =
  process.argv[2] ?? new URL("../docs-site/dist/", import.meta.url).pathname;
const entry = "/docs/cli.html";
const server = Bun.serve({
  port: 0,
  async fetch(request) {
    const path = new URL(request.url).pathname;
    const file = Bun.file(
      resolve(dist, `.${path === "/" ? "/index.html" : path}`),
    );
    return (await file.exists())
      ? new Response(file)
      : new Response("Not found", { status: 404 });
  },
});
const browser = await chromium.launch({
  executablePath: process.env.CHROME_BIN,
  args: ["--no-sandbox"],
});

try {
  for (const scenario of [
    "foreign release",
    "owner release",
    "outside second finger",
    "cancel",
    "outside release",
  ]) {
    const context = await browser.newContext({
      ...devices["Pixel 7"],
      viewport: { width: 390, height: 844 },
    });
    try {
      const page = await context.newPage();
      const events = [];
      await page.exposeFunction("recordTouch", (event) => events.push(event));
      await page.addInitScript(() => {
        for (const type of ["touchstart", "touchend", "touchcancel"])
          document.addEventListener(
            type,
            (event) => {
              window.recordTouch({
                type,
                trusted: event.isTrusted,
                remaining: [...event.touches].map((touch) => touch.identifier),
                changed: [...event.changedTouches].map(
                  (touch) => touch.identifier,
                ),
              });
            },
            true,
          );
      });
      const url = `${server.url.origin}${entry}`;
      await page.goto(url);
      const toggle = page.locator(".page-switcher-toggle");
      const list = page.locator(".page-switcher-list");
      const box = await toggle.boundingBox();
      const owner = { id: 1, x: box.x + 22, y: box.y + 22 };
      const other = { id: 2, x: box.x + 16, y: box.y + 22 };
      const cdp = await context.newCDPSession(page);
      const send = (type, touchPoints) =>
        cdp.send("Input.dispatchTouchEvent", { type, touchPoints });
      const target = list.locator(":scope > a").nth(1);
      await send("touchStart", [owner]);
      const link = await target.boundingBox();
      const destination = new URL(await target.getAttribute("href"), url).href;
      const overLink = {
        x: link.x + link.width / 2,
        y: link.y + link.height / 2,
      };
      const ownerEndsFirst =
        scenario === "owner release" || scenario === "outside second finger";
      if (ownerEndsFirst) {
        Object.assign(owner, overLink);
        await send("touchMove", [owner]);
      }
      if (scenario === "outside second finger")
        Object.assign(other, { x: 20, y: 500 });
      await send("touchStart", [owner, other]);
      assert.equal(
        await toggle.getAttribute("aria-expanded"),
        "true",
        "a second finger must not cancel the owner's drag",
      );
      if (scenario === "foreign release") {
        Object.assign(other, overLink);
        await send("touchMove", [owner, other]);
        await send("touchEnd", [other]);
        assert.equal(page.url(), url, "another finger must not navigate");
        assert.equal(await toggle.getAttribute("aria-expanded"), "true");
        assert(
          events.some(
            (event) =>
              event.type === "touchend" &&
              event.changed.includes(2) &&
              event.remaining.includes(1),
          ),
        );
        Object.assign(owner, overLink);
        await send("touchMove", [owner]);
        await send("touchEnd", []);
        await page.waitForURL(destination);
      } else if (ownerEndsFirst) {
        await send("touchEnd", [owner]);
        await page.waitForURL(destination);
        assert(
          events.some(
            (event) =>
              event.type === "touchend" &&
              event.changed.includes(1) &&
              event.remaining.includes(2),
          ),
        );
      } else {
        Object.assign(other, overLink);
        await send("touchMove", [owner, other]);
        if (scenario === "cancel") await send("touchCancel", []);
        else {
          owner.x = 2;
          owner.y = overLink.y;
          await send("touchMove", [owner, other]);
          await send("touchEnd", [owner]);
        }
        assert.equal(page.url(), url);
        assert.equal(await toggle.getAttribute("aria-expanded"), "false");
        if (scenario === "outside release") await send("touchCancel", []);
        const fresh = { id: 3, x: box.x + 22, y: box.y + 22 };
        await send("touchStart", [fresh]);
        await send("touchEnd", []);
        assert.equal(
          await toggle.getAttribute("aria-expanded"),
          "true",
          "a fresh gesture must still work",
        );
      }
      assert(
        events.some(
          (event) =>
            event.type === "touchstart" && event.remaining.length === 2,
        ),
      );
      assert(
        events.every((event) => event.trusted),
        "the test must use browser-generated touch events",
      );
      console.log(`PASS ${scenario}`);
    } finally {
      await context.close();
    }
  }
} finally {
  await browser.close();
  server.stop(true);
}
