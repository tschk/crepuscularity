"""Check Java, Kotlin, and PHP plugin smoke tests with a noisy CLI fixture."""
import os
from pathlib import Path
import subprocess
import signal
import sys
import tempfile

if os.name != "posix":
    print("Skipping plugin diagnostic fixture: requires a POSIX executable")
    raise SystemExit(0)

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="crepus-plugin-diagnostics-") as directory:
    binary = Path(directory) / "crepus"
    binary.write_text(
        f"#!{sys.executable}\n"
        "import json,sys,os\n"
        "if sys.argv[1] == 'render':\n"
        "    with open(os.environ['CREPUS_HTML_CONTEXT_LOG'], 'w') as log: log.write(sys.argv[sys.argv.index('--ctx') + 1])\n"
        "    sys.stderr.write('html diagnostic\\n' * 32768)\n"
        "    sys.stderr.flush()\n"
        "    sys.exit(3)\n"
        "payload=json.load(sys.stdin)\n"
        "text='Frontend content' if 'Frontend content' in payload['template'] else ('Included ' if payload['template'].startswith('include') else 'Hello ')+payload['context']['name']\n"
        "sys.stderr.write('diagnostic\\n' * 32768)\n"
        "sys.stderr.flush()\n"
        "if os.getenv('CREPUS_FIXTURE_FAILURE'): sys.exit(3)\n"
        "print(json.dumps({'version':7,'root':[{'kind':'text','content':text}]}))\n"
    )
    binary.chmod(0o700)
    # Labelled CLI fixture: exact response bytes exercise decoding independently of Rust.
    encoding_binary = Path(directory) / "crepus-encoding"
    encoding_binary.write_text(
        f"#!{sys.executable}\n"
        "import json,os,sys,threading,time\n"
        "mode=os.environ['CREPUS_RESPONSE_MODE']\n"
        "if mode.startswith('large'):\n"
        " threading.Thread(target=lambda:(time.sleep(5),os._exit(88)),daemon=True).start()\n"
        " if mode == 'large_failure':\n"
        "  sys.stderr.buffer.write('雪 early diagnostic\\n'.encode('utf-8')); sys.stderr.buffer.flush(); sys.exit(9)\n"
        " if mode == 'large_unread':\n"
        "  sys.stdout.buffer.write(b'{\"version\":7,\"root\":[]}'); sys.stdout.buffer.flush(); sys.exit(0)\n"
        " sys.stdout.buffer.write(b' ' * 262144); sys.stdout.buffer.flush()\n"
        " payload=json.loads(sys.stdin.buffer.read().decode('utf-8'))\n"
        " assert payload['context']['blob'] == '雪' * 400000\n"
        " with open(payload['filename'],encoding='utf-8',newline='') as source: assert payload['template'] == source.read()\n"
        " sys.stdout.buffer.write(json.dumps({'version':7,'root':[{'kind':'text','content':'Exact payload 雪'}]},ensure_ascii=False).encode('utf-8'))\n"
        " sys.exit(0)\n"
        "sys.stdin.buffer.read()\n"
        "if mode == 'failure':\n"
        " sys.stdout.buffer.write(b'\\xff')\n"
        " sys.stderr.buffer.write('雪 diagnostic\\n'.encode('utf-8'))\n"
        " sys.exit(9)\n"
        "value={'ff':b'\\xff','sequence':b'\\xc3('}.get(mode,'雪 🦋'.encode('utf-8'))\n"
        "if sys.argv[1] == 'render': sys.stdout.buffer.write(b'<p>'+value+b'</p>')\n"
        "else: sys.stdout.buffer.write(b'{\"version\":7,\"root\":[{\"kind\":\"text\",\"content\":\"'+value+b'\"}]}')\n",
        encoding="utf-8",
    )
    encoding_binary.chmod(0o700)
    fixture = str(root / "plugins/fixtures/hello.crepus")
    for command in [
        ["java", "-classpath", sys.argv[1], "dev.crepuscularity.plugin.CrepuscularityPluginTest", fixture],
        ["kotlin", "-classpath", sys.argv[2], "dev.crepuscularity.plugin.CrepuscularityPluginTestKt", fixture],
        ["php", str(root / "plugins/php/path_smoke.php")],
    ]:
        for failure in (False, True):
            process = subprocess.Popen(
                command + ["failure" if failure else "success"],
                env={**os.environ, "CREPUS_BIN": "crepus", "PATH": directory + os.pathsep + os.environ.get("PATH", ""),
                     "CREPUS_FIXTURE_FAILURE": "1" if failure else ""}, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, text=True, start_new_session=True,
            )
            try:
                stdout, stderr = process.communicate(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGTERM)
                process.communicate()
                raise AssertionError(f"{command[0]} blocked while draining CLI diagnostics")
            assert process.returncode == 0, f"{command[0]} failed: {stderr[-1024:]}"
    for command in [
        ["java", "-classpath", sys.argv[1], "dev.crepuscularity.plugin.CrepuscularityHtmlTest"],
        ["kotlin", "-classpath", sys.argv[2], "dev.crepuscularity.plugin.CrepuscularityHtmlTestKt"],
    ]:
        process = subprocess.Popen(
            command + ["failure", fixture],
            env={**os.environ, "CREPUS_BIN": str(binary), "CREPUS_HTML_CONTEXT_LOG": str(Path(directory) / "context-path.log")},
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True,
        )
        try:
            stdout, stderr = process.communicate(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            process.communicate()
            raise AssertionError(f"{command[0]} HTML blocked while draining CLI diagnostics")
        assert process.returncode == 0, f"{command[0]} HTML failed: {stderr[-1024:]}"
    response_failures = []
    for command in [
        ["java", "-classpath", sys.argv[1], "dev.crepuscularity.plugin.CrepuscularityUnicodeTest"],
        ["kotlin", "-classpath", sys.argv[2], "dev.crepuscularity.plugin.CrepuscularityUnicodeTestKt"],
    ]:
        for mode in ("valid", "failure", "ff", "sequence", "large", "large_failure", "large_unread"):
            for route in (("ir",) if mode.startswith("large") else ("ir", "html")):
                result = subprocess.run(command + ["transport", mode, fixture, route],
                    env={**os.environ, "CREPUS_BIN": str(encoding_binary), "CREPUS_RESPONSE_MODE": mode},
                    capture_output=True, timeout=10)
                if result.returncode != 0:
                    response_failures.append(f"{command[0]} response mode={mode} route={route}: {result.stderr.decode('utf-8', 'replace')}")
    assert not response_failures, "\n".join(response_failures)
print("Java/Kotlin HTML cleanup and Java, Kotlin, and PHP diagnostic isolation passed")
