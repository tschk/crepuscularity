"""Manual OS-watcher regression: python3 crates/crepuscularity-cli/tests/native_mobile_watch.py [crepus]."""
import json
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/crepus").resolve()

with tempfile.TemporaryDirectory(prefix="crepus-mobile-watch-") as directory:
    root = Path(directory)
    views = root / "views"
    views.mkdir()
    entry = views / "main.crepus"
    child = views / "Child.vue"
    context = root / "context.json"
    context.write_text(json.dumps({"title": "Context title", "greeting": "Context greeting"}))
    child.write_text("<template><span>Before</span></template>")
    entry.write_text('div\n  "{title}: {greeting}"\n  include Child.vue\n')
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    with (root / "server.log").open("w+") as log:
        process = subprocess.Popen(
            [str(binary), "mobile", "dev", "--dir", str(root), "--port", str(port), "--platform", "all",
             "--ctx", str(context), "--var", "title=Override title"],
            stdout=log, stderr=log,
        )

        def fetch(path):
            with urllib.request.urlopen(f"http://127.0.0.1:{port}{path}", timeout=1) as response:
                return response.read().decode()

        def wait_for(path, predicate, retry=None):
            deadline = time.monotonic() + 15
            next_retry = time.monotonic() + 1
            last = ""
            while time.monotonic() < deadline:
                assert process.poll() is None, "mobile dev exited unexpectedly"
                try:
                    last = fetch(path)
                    if predicate(last):
                        return last
                except (OSError, urllib.error.URLError):
                    pass
                if retry and time.monotonic() >= next_retry:
                    retry()
                    next_retry = time.monotonic() + 1
                time.sleep(0.05)
            raise AssertionError(f"Timed out on {path}: {last}")

        try:
            initial = wait_for("/ir", lambda value: "Before" in value)
            assert "Override title: Context greeting" in initial
            assert json.loads((root / "fixture.json").read_text()) == json.loads(initial)
            generated = root / "ios/Sources/NativeShell/Generated/CrepusGeneratedView.swift"
            assert "Before" in generated.read_text()
            kotlin = root / "android/app/src/main/java/dev/crepuscularity/nativeshell/generated/CrepusGeneratedView.kt"
            assert kotlin.read_text().startswith("package dev.crepuscularity.nativeshell\n")
            assert "Before" in kotlin.read_text()
            # Prove the OS watcher is active before testing an include-only edit.
            def signal_ready():
                entry.write_text('div\n  "Ready {title}: {greeting}"\n  include Child.vue\n')
            signal_ready()
            wait_for("/ir", lambda value: "Ready" in value, retry=signal_ready)
            child.write_text("<template><span>After</span></template>")
            updated = wait_for("/ir", lambda value: "After" in value)
            assert "Override title: Context greeting" in updated
            assert "Before" not in updated
            wait_for("/events", lambda value: '"patch"' in value and "After" in value)
            wait_for("/ir", lambda _: "After" in (root / "fixture.json").read_text())
            assert json.loads((root / "fixture.json").read_text()) == json.loads(updated)
            assert "After" in generated.read_text()
            assert "After" in kotlin.read_text()
            child.unlink()
            wait_for("/events", lambda value: '"error"' in value)
            assert json.loads(fetch("/ir")) == json.loads(updated)
            child.write_text("<template><span>Recovered</span></template>")
            wait_for("/ir", lambda value: "Recovered" in value)
            print("PASS: actual mobile watcher updates Vue includes, preserves IR on removal, and recovers")
        except Exception:
            log.flush()
            log.seek(0)
            print(log.read(), file=sys.stderr)
            raise
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
