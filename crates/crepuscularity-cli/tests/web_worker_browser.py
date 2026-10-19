"""Manual transport regression: python3 crates/crepuscularity-cli/tests/web_worker_browser.py."""
import argparse
import functools
import http.server
import tempfile
import threading
from pathlib import Path

from playwright.sync_api import sync_playwright

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--browser", choices=("chromium", "webkit"), default="chromium")
args = parser.parse_args()

assets = Path(__file__).resolve().parents[1] / "assets/web"


class Handler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        super().end_headers()

    def log_message(self, *_args):
        pass


with tempfile.TemporaryDirectory(prefix="crepus-worker-browser-") as directory:
    root = Path(directory)
    for mode in ("ready", "failed"):
        site = root / mode
        (site / "pkg").mkdir(parents=True)
        for name in ("runtime-client.js", "runtime-worker.js"):
            (site / name).write_bytes((assets / name).read_bytes())
        initialization = "throw new Error('fixture init failed');" if mode == "failed" else ""
        (site / "pkg/runtime.js").write_text(
            "let state=0; export default async function(){globalThis.postMessage({initializationAttempt:true});"
            + initialization
            + "} export function read(){return state;}"
            "export function uncloneable(){state++;return ()=>state;}"
            "export function change(){state++;}"
            "export function rendersBadly(){return ()=>state;}"
            "export function throwsUnprintable(){state++;throw Object.create(null);}"
            "export function blocked(buffer){const flag=new Int32Array(buffer);"
            "globalThis.postMessage({blocked:true});"
            "if(Atomics.wait(flag,0,0,5000)==='timed-out')throw new Error('Main heartbeat did not release worker');"
            "return Atomics.load(flag,0);}"
        )
        (site / "index.html").write_text("<!doctype html><meta charset=utf-8><title>Worker transport test</title>")
    server = http.server.ThreadingHTTPServer(
        ("127.0.0.1", 0), functools.partial(Handler, directory=directory)
    )
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with sync_playwright() as playwright:
            browser = getattr(playwright, args.browser).launch(headless=True)
            try:
                for mode in ("ready", "failed"):
                    page = browser.new_page()
                    errors = []
                    page.on("pageerror", lambda error: errors.append(str(error)))
                    page.set_default_timeout(10000)
                    page.goto(f"http://127.0.0.1:{server.server_port}/{mode}/")
                    result = page.evaluate("""async (mode) => {
                      const {createWorkerRuntime} = await import('./runtime-client.js');
                      const worker = new Worker('./runtime-worker.js', {type:'module'});
                      let attempts = 0;
                      worker.addEventListener('message', ({data}) => {
                        if(data?.initializationAttempt) attempts++;
                      });
                      const client = createWorkerRuntime(worker);
                      const bounded = async promise => {
                        let timer;
                        try {
                          return await Promise.race([promise, new Promise((_, reject) => {
                            timer = setTimeout(() => reject(new Error('Transport timeout')), 10000);
                          })]);
                        } finally {clearTimeout(timer);}
                      };
                      const failure = async promise => {
                        try {await bounded(promise); throw new Error('Expected rejection');}
                        catch(error) {
                          if(error.message === 'Expected rejection') throw error;
                          return String(error);
                        }
                      };
                      try {
                        if(mode === 'failed') {
                          const [ready,later,queued] = await Promise.all([
                            failure(client.ready), failure(client.call('read')),
                            failure(client.call('read'))]);
                          return {ready,later,queued,attempts};
                        }
                        const names = await bounded(client.ready);
                        const flag = new Int32Array(new SharedArrayBuffer(4));
                        let heartbeat = false;
                        worker.addEventListener('message', ({data}) => {
                          if (data?.blocked) setTimeout(() => {
                            heartbeat = true;
                            Atomics.store(flag, 0, 1);
                            Atomics.notify(flag, 0);
                          }, 0);
                        });
                        const released = await bounded(client.call('blocked', flag.buffer));
                        if (!heartbeat || released !== 1) throw new Error('Main heartbeat failed');
                        const bad = failure(client.call('uncloneable'));
                        const next = client.call('read');
                        const cloneError = await bad;
                        const state = await bounded(next);
                        const renderError = await failure(client.event('change','{}',
                          {name:'rendersBadly',args:[]}));
                        const afterRender = await bounded(client.call('read'));
                        const thrown = failure(client.call('throwsUnprintable'));
                        const afterThrown = await bounded(client.call('read'));
                        const thrownError = await thrown;
                        const usableAfterThrown = await bounded(client.call('read'));
                        // Disposal must settle requests without releasing a real blocked worker.
                        const blocked = new Promise(resolve => {
                          worker.addEventListener('message', ({data}) => {
                            if (data?.blocked) resolve();
                          });
                        });
                        const stoppedFlag = new Int32Array(new SharedArrayBuffer(4));
                        const stopping = failure(client.call('blocked', stoppedFlag.buffer));
                        const queued = failure(client.call('change'));
                        await bounded(blocked);
                        client.dispose();
                        const disposed = await Promise.all([stopping, queued,
                          failure(client.call('read'))]);
                        if (Atomics.load(stoppedFlag, 0) !== 0) throw new Error('Disposed worker was released');
                        return {names,cloneError,state,renderError,afterRender,thrownError,afterThrown,usableAfterThrown,attempts,disposed};
                      } finally {client.dispose();}
                    }""", mode)
                    if mode == "failed":
                        assert "fixture init failed" in result["ready"], result
                        assert result["ready"] == result["later"] == result["queued"], result
                    else:
                        assert "read" in result["names"], result
                        assert "clone" in result["cloneError"].lower(), result
                        assert "clone" in result["renderError"].lower(), result
                        assert result["state"] == 1, result
                        assert result["afterRender"] == 2, result
                        assert "timeout" not in result["thrownError"].lower(), result
                        assert result["afterThrown"] == 3, result
                        assert result["usableAfterThrown"] == 3, result
                        assert result["disposed"] == ["Error: Runtime worker disposed"] * 3, result
                    assert result["attempts"] == 1, result
                    assert not errors, errors
                    page.close()
            finally:
                browser.close()
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
print(f"PASS ({args.browser}): blocked worker permits main heartbeat and disposal settles queued work; real worker clone/unprintable failures reject, FIFO recovers without replay, init failure reaches later calls")
