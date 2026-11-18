"""Manual browser gate: python3 crates/crepuscularity-wasm/npm/browser.test.py."""
import functools
import hashlib
import http.server
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading

from playwright.sync_api import sync_playwright

package = Path(__file__).resolve().parent


class Handler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        # Count initialization fetches independently of cross-context caching.
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def do_GET(self):
        self.server.request_paths.append(self.path)
        super().do_GET()

    def log_message(self, *_args):
        pass


with tempfile.TemporaryDirectory(prefix="crepus-parser-browser-") as directory:
    root = Path(directory)
    manifest = json.loads((package / "package.json").read_text())
    subprocess.run([
        "bun", "pm", "pack", "--ignore-scripts", "--filename", str(root / "parser.tgz"),
    ], cwd=package, check=True, capture_output=True)
    (root / "package.json").write_text('{"private":true,"type":"module"}')
    subprocess.run([
        "bun", "add", "--ignore-scripts", str(root / "parser.tgz"),
    ], cwd=root, check=True, capture_output=True)
    installed = root / "node_modules" / manifest["name"]
    for name in manifest["files"]:
        source = package / name
        for path in source.rglob("*") if source.is_dir() else [source]:
            if path.is_file() and not path.name.startswith("."):
                assert hashlib.sha256(path.read_bytes()).digest() == hashlib.sha256(
                    (installed / path.relative_to(package)).read_bytes()
                ).digest(), path
    print("PASS packed file hashes:", hashlib.sha256((root / "parser.tgz").read_bytes()).hexdigest())
    check = ('import { parseTemplate } from "@tschk/crepuscularity-wasm";'
        'for (const [filename, source] of [["App.crepus", \'p "hello"\'],'
        '["App.tsx", "<p>hello</p>"], ["App.svelte", "<p>hello</p>"],'
        '["App.vue", "<template><p>hello</p></template>"],'
        '["App.astro", "<p>hello</p>"], ["App.component.html", "<p>hello</p>"]])'
        'if (parseTemplate(source, filename).root[0].content !== "hello")'
        'throw new Error(`Installed root entry failed: ${filename}`);'
        'await import("@tschk/crepuscularity-wasm/browser");')
    for runtime in ["node", "bun"]:
        subprocess.run([runtime, "--eval", check], cwd=root, check=True, capture_output=True)
        print(f"PASS {runtime}: packed install root and browser exports")
    entry = root / "browser.ts"
    entry.write_text('export { loadParser } from "@tschk/crepuscularity-wasm/browser";')
    subprocess.run([
        "bun", "build", str(entry), "--target", "browser", "--outdir", str(root),
    ], check=True, capture_output=True)
    shutil.copytree(installed / "browser", root / "browser")
    shutil.copyfile(installed / "browser/crepuscularity_wasm_bg.wasm", root / "relocated.wasm")
    (root / "index.html").write_text("<!doctype html><title>Parser worker test</title>")
    (root / "worker.js").write_text(r'''
import { loadParser } from './browser.js';
self.onmessage = async ({data:mode}) => {
  try {
    if (mode === 'failed') {
      const first = loadParser('/missing.wasm');
      if (first !== loadParser()) throw new Error('Failed initialization was not shared');
      const results = await Promise.allSettled([first, loadParser()]);
      if (results.some(result => result.status !== 'rejected')) throw new Error('Missing WASM did not reject');
      self.postMessage({ok:true, failed:true});
      return;
    }
    const first = loadParser(mode === 'custom' ? '/relocated.wasm' : undefined);
    if (first !== loadParser()) throw new Error('Initialization was not shared');
    const parser = await first;
    const fixtures = [
      ['App.crepus', 'p "hello"'],
      ['App.tsx', '<p>hello</p>'],
      ['App.svelte', '<p>hello</p>'],
      ['App.vue', '<template><p>hello</p></template>'],
      ['App.astro', '<p>hello</p>'],
      ['App.component.html', '<p>hello</p>'],
    ];
    for (const [filename, source] of fixtures) {
      const ir = parser.parseTemplate(source, filename);
      if (ir.version !== parser.IR_VERSION || ir.root[0].content !== 'hello')
        throw new Error(`Wrong frontend result: ${filename}`);
    }
    if (parser.parseTemplate('<p set:text="label"/>', 'App.astro', {label:'wrong'}).root[0].content !== 'label')
      throw new Error('Astro literal regression');
    if (parser.parseCrepus('p "🌙"').root[0].content !== '🌙')
      throw new Error('Unicode regression');
    for (const [run, message] of [
      [() => parser.parseTemplate('<p bind:clientWidth={n}/>', 'App.svelte'), 'unsupported Svelte binding'],
      [() => parser.parseCrepus('\ud800'), 'unpaired UTF-16 surrogate'],
      [() => parser.parseCrepus('p "hello"', {n:NaN}), 'finite'],
    ]) {
      let caught;
      try {run();} catch (error) {caught = String(error);}
      if (!caught?.includes(message)) throw new Error(`Expected rejection: ${message}, got ${caught}`);
    }
    if (await loadParser() !== parser) throw new Error('Parser instance changed');
    self.postMessage({ok:true, frontends:fixtures.length, version:parser.IR_VERSION});
  } catch (error) {self.postMessage({error:String(error)});}
};
''')
    server = http.server.ThreadingHTTPServer(
        ("127.0.0.1", 0), functools.partial(Handler, directory=directory)
    )
    server.request_paths = []
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with sync_playwright() as playwright:
            for name in os.environ.get("CREPUS_BROWSERS", "chromium,webkit").split(","):
                options = {"headless": True}
                if name == "chromium" and os.environ.get("CHROME_PATH"):
                    options["executable_path"] = os.environ["CHROME_PATH"]
                browser = getattr(playwright, name).launch(**options)
                try:
                    page = browser.new_page()
                    page.goto(f"http://127.0.0.1:{server.server_port}/")
                    result = page.evaluate('''async () => {
                      const {loadParser} = await import('./browser.js');
                      const first = loadParser();
                      if (first !== loadParser()) throw new Error('Page initialization was not shared');
                      const parser = await first;
                      return parser.parseTemplate('<template><p>page</p></template>', 'Page.vue').root[0].content;
                    }''')
                    assert result == "page", result
                    for mode, expected_path in [
                        ("ready", "/browser/crepuscularity_wasm_bg.wasm"),
                        ("custom", "/relocated.wasm"),
                        ("failed", "/missing.wasm"),
                    ]:
                        before = server.request_paths.count(expected_path)
                        result = page.evaluate('''(mode) => new Promise((resolve, reject) => {
                          const worker = new Worker('./worker.js', {type:'module'});
                          const timer = setTimeout(() => {worker.terminate(); reject(new Error('Worker timeout'));}, 10000);
                          worker.onmessage = ({data}) => {clearTimeout(timer); worker.terminate(); resolve(data);};
                          worker.onerror = (event) => {clearTimeout(timer); worker.terminate(); reject(new Error(event.message));};
                          worker.postMessage(mode);
                        })''', mode)
                        assert server.request_paths.count(expected_path) == before + 1, server.request_paths
                        assert result.get("ok"), result
                        assert result.get("failed") if mode == "failed" else result.get("frontends") == 6, result
                    print(f"PASS {name}: real WASM worker, six frontends, shared initialization and validation")
                finally:
                    browser.close()
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
