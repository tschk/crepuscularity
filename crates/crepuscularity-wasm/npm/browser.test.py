"""Manual browser gate: python3 crates/crepuscularity-wasm/npm/browser.test.py."""
import functools
import http.server
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading

from playwright.sync_api import sync_playwright

package = Path(__file__).resolve().parent


class Handler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        self.server.request_paths.append(self.path)
        super().do_GET()

    def log_message(self, *_args):
        pass


with tempfile.TemporaryDirectory(prefix="crepus-parser-browser-") as directory:
    root = Path(directory)
    manifest = json.loads((package / "package.json").read_text())
    installed = root / "node_modules" / manifest["name"]
    installed.mkdir(parents=True)
    shutil.copyfile(package / "package.json", installed / "package.json")
    for name in manifest["files"]:
        source, destination = package / name, installed / name
        if source.is_dir():
            shutil.copytree(source, destination)
        else:
            shutil.copyfile(source, destination)
    subprocess.run([
        "bun", "--eval", 'import { parseTemplate } from "@tschk/crepuscularity-wasm";'
        'if (parseTemplate("<p>hello</p>", "App.svelte").root[0].content !== "hello")'
        'throw new Error("Installed root entry failed");',
    ], cwd=root, check=True, capture_output=True)
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
            for name in ["chromium", "webkit"]:
                browser = getattr(playwright, name).launch(headless=True)
                try:
                    page = browser.new_page()
                    page.goto(f"http://127.0.0.1:{server.server_port}/")
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
