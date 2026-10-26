"""Manual browser regression: python3 crates/crepuscularity-cli/tests/web_reload_browser.py."""
import json
import re
from pathlib import Path

from playwright.sync_api import sync_playwright

crate = Path(__file__).resolve().parents[1]
source = (crate / "src/web_serve.rs").read_text()
literal = re.search(r'const RELOAD_SCRIPT: &str = (".*?\n</script>");', source, re.S)[1]
literal = re.sub(r'\\u\{([0-9a-f]+)\}', lambda m: chr(int(m[1], 16)), literal)
reload_script = json.loads(literal, strict=False)
app = (crate / "assets/web/app.js").read_text()

with sync_playwright() as playwright:
    browser = playwright.chromium.launch(headless=True)
    for module in [True, False]:
        page = browser.new_page()
        errors = []
        requests = []
        page.on("pageerror", lambda error: errors.append(str(error)))
        def route(request):
            path = request.request.url.split("example.test", 1)[1]
            requests.append(path)
            if path == "/app.js":
                body, mime = app, "application/javascript"
            elif path == "/runtime-client.js":
                body = (crate / "assets/web/runtime-client.js").read_text()
                mime = "application/javascript"
            elif path == "/pkg/runtime.js":
                body = 'export default async function(){}; export function crepus_render(bundle){return JSON.parse(bundle).html;}'
                mime = "application/javascript"
            elif path == "/crepus-bundle.json":
                body, mime = json.dumps({"html": "<p>updated</p>"}), "application/json"
            elif path == "/crepus-islands.json":
                body, mime = '{"islands":{}}', "application/json"
            else:
                body = '<meta charset="utf-8"><div id="crepus-root"></div><script id="__crepus_bundle__" type="application/json">{"html":"<p>initial</p>"}</script>'
                body += '<script>window.EventSource=class{constructor(){window.source=this;this.listeners={}} addEventListener(name,callback){this.listeners[name]=callback}}</script>'
                body += reload_script
                if module:
                    body += '<script type="module" src="/app.js"></script>'
                mime = "text/html"
            request.fulfill(body=body, content_type=mime)
        page.route("**/*", route)
        page.goto("http://example.test/nested/page.html")
        if module:
            page.wait_for_function('document.querySelector("#crepus-root").textContent === "initial"')
        page.evaluate('window.marker=1; window.source.listeners["crepus-reload"]({data: JSON.stringify({file:"page.crepus"})})')
        if module:
            page.wait_for_function('document.querySelector("#crepus-root").textContent === "updated"')
            assert page.evaluate('window.marker') == 1, "entry page unexpectedly reloaded"
        else:
            page.wait_for_function('window.marker === undefined')
        assert "/crepus-bundle.json" in requests
        assert "/nested/crepus-bundle.json" not in requests
        assert not errors, errors
        page.close()
    browser.close()
print("PASS: module updates without navigation; secondary preview reloads; nested URL fetches root bundle")
