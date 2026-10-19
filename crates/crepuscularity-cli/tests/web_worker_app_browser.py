"""Manual production-app/worker integration with a labelled JavaScript runtime fixture."""
import argparse
import functools
import http.server
import json
import tempfile
import threading
from pathlib import Path
from playwright.sync_api import sync_playwright

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--browser", choices=("chromium", "webkit"), default="chromium")
args = parser.parse_args()
assets = Path(__file__).resolve().parents[1] / "assets/web"


class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass


with tempfile.TemporaryDirectory(prefix="crepus-worker-app-") as directory:
    root = Path(directory)
    (root / "pkg").mkdir()
    for name in ("app.js", "runtime-client.js", "runtime-worker.js"):
        (root / name).write_bytes((assets / name).read_bytes())
    (root / "pkg/runtime.js").write_text('''
let initialized = false, count = 0;
function workerOnly() {
  if (typeof window !== 'undefined') throw new Error('Runtime executed on main thread');
  if (!initialized) throw new Error('Runtime was not initialized');
}
export default async function() {
  if (typeof window !== 'undefined') throw new Error('Runtime initialized on main thread');
  initialized = true;
}
export function crepus_worker_runtime() {}
export function increment(json) {
  workerOnly();
  if (Number(JSON.parse(json)['data-sequence']) !== count + 1) throw new Error('Out of order mutation');
  count++;
}
export function crepus_render(json) {
  workerOnly(); JSON.parse(json);
  return '<button id="increment" data-onclick="increment" data-sequence="'+(count+1)+'">Add</button><output id="count">'+count+'</output>';
}
''')
    (root / "crepus-islands.json").write_text('{"islands":{}}')
    for ssr in (False, True):
        initial = '<section data-crepus-root><button id="increment" data-onclick="increment" data-sequence="1">Add</button><output id="count">0</output></section>' if ssr else ''
        bundle = json.dumps({"entry": "index.crepus", "files": {"index.crepus": 'span "fixture"'}})
        (root / f"{ssr}.html").write_text(
            '<!doctype html><meta charset="utf-8"><title>Worker app integration</title>'
            f'<main id="crepus-root" style="opacity:0">{initial}</main>'
            f'<script id="__crepus_bundle__" type="application/json">{bundle}</script>'
            '<script type="module" src="./app.js"></script>'
        )
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Handler, directory=directory))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with sync_playwright() as playwright:
            browser = getattr(playwright, args.browser).launch(headless=True)
            try:
                for ssr in (False, True):
                    page = browser.new_page()
                    errors = []
                    page.on("pageerror", lambda error: errors.append(str(error)))
                    page.on("console", lambda message: errors.append(message.text) if message.type == "error" else None)
                    page.set_default_timeout(10000)
                    page.goto(f"http://127.0.0.1:{server.server_port}/{ssr}.html")
                    page.wait_for_function("document.querySelector('#count')?.textContent === '0' && document.querySelector('#crepus-root').style.opacity === '1'")
                    page.locator("#increment").click()
                    page.wait_for_function("document.querySelector('#count')?.textContent === '1'")
                    page.evaluate("""() => {
                      const button = document.getElementById('increment');
                      for (let i=0; i<10; i++) { button.dataset.sequence = String(i+2); button.click(); }
                    }""")
                    page.wait_for_function("document.querySelector('#count')?.textContent === '11'")
                    assert not errors, errors
                    page.close()
                    print(f"PASS ({args.browser}, SSR={ssr}): production app/worker boot and 11 FIFO event mutations", flush=True)
            finally:
                browser.close()
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
