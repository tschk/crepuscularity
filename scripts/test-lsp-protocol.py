"""Run after cargo build -p crepuscularity-lsp; test stdio exit with stdin open."""

import json
from pathlib import Path
import subprocess
import sys
import threading

ROOT = Path(__file__).resolve().parents[1]
server = subprocess.Popen(
    [str(ROOT / "target/debug/crepus-lsp"), "--stdio"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
)
deadline = threading.Timer(15, server.kill)
deadline.start()


def send(method, params=None, request_id=None):
    message = {"jsonrpc": "2.0", "method": method}
    if params is not None:
        message["params"] = params
    if request_id is not None:
        message["id"] = request_id
    payload = json.dumps(message).encode()
    server.stdin.write(f"Content-Length: {len(payload)}\r\n\r\n".encode() + payload)
    server.stdin.flush()


def response(request_id, notification=None):
    while True:
        headers = {}
        while True:
            line = server.stdout.readline()
            assert line, "server closed stdout before replying"
            if line == b"\r\n":
                break
            key, value = line.decode().split(":", 1)
            headers[key.lower()] = value.strip()
        message = json.loads(server.stdout.read(int(headers["content-length"])))
        if notification is not None and message.get("method") == notification:
            return message["params"]
        if request_id is not None and message.get("id") == request_id:
            assert "error" not in message, message
            return message.get("result")


try:
    send("initialize", {"processId": None, "rootUri": None, "capabilities": {}}, 1)
    assert response(1)["capabilities"]["completionProvider"]
    send("initialized", {})
    uri = "file:///tmp/crepus-lsp-protocol.crepus"
    source = 'div\n$: letter = 1\n<letter name="ghost" />\n<let surname="phantom" />\n$: let actual = 2\ntext {a\ntext "say \\"{'
    send("textDocument/didOpen", {"textDocument": {
        "uri": uri, "languageId": "crepus", "version": 1, "text": source,
    }})
    opened = response(None, "textDocument/publishDiagnostics")
    assert opened.get("version") == 1, opened
    send("textDocument/completion", {
        "textDocument": {"uri": uri}, "position": {"line": 5, "character": 7},
    }, 3)
    items = response(3)
    assert [item["label"] for item in items] == ["actual"], items
    send("textDocument/completion", {
        "textDocument": {"uri": uri},
        "position": {"line": 6, "character": len(source.splitlines()[6])},
    }, 4)
    assert response(4) is None, "literal text received expression completions"
    for version, text, has_errors in [(2, "div", False), (3, "<div", True)]:
        send("textDocument/didChange", {
            "textDocument": {"uri": uri, "version": version},
            "contentChanges": [{"text": text}],
        })
        changed = response(None, "textDocument/publishDiagnostics")
        assert changed.get("version") == version, changed
        assert bool(changed["diagnostics"]) == has_errors, changed
    for version, text in [(2, "<stale"), (3, "<duplicate"), (4, "div")]:
        send("textDocument/didChange", {
            "textDocument": {"uri": uri, "version": version},
            "contentChanges": [{"text": text}],
        })
    current = response(None, "textDocument/publishDiagnostics")
    assert current.get("version") == 4 and current["diagnostics"] == [], current
    send("textDocument/didClose", {"textDocument": {"uri": uri}})
    cleared = response(None, "textDocument/publishDiagnostics")
    assert cleared["uri"] == uri and cleared["diagnostics"] == [], cleared
    assert "version" not in cleared, cleared
    send("textDocument/completion", {
        "textDocument": {"uri": uri}, "position": {"line": 4, "character": 7},
    }, 5)
    assert response(5) is None, "closed document supplied stale completions"
    send("textDocument/didChange", {
        "textDocument": {"uri": uri, "version": 5},
        "contentChanges": [{"text": "<closed"}],
    })
    send("textDocument/didOpen", {"textDocument": {
        "uri": uri, "languageId": "crepus", "version": 1, "text": "div",
    }})
    reopened = response(None, "textDocument/publishDiagnostics")
    assert reopened.get("version") == 1 and reopened["diagnostics"] == [], reopened
    send("textDocument/didChange", {
        "textDocument": {"uri": uri, "version": 2},
        "contentChanges": [{"text": 'text "for"\ndiv lede\n.lede text-red-500\n'}],
    })
    assert response(None, "textDocument/publishDiagnostics").get("version") == 2
    for request_id, line, character, expected_hover in [(6, 0, 7, False), (7, 1, 5, True)]:
        send("textDocument/hover", {
            "textDocument": {"uri": uri}, "position": {"line": line, "character": character},
        }, request_id)
        assert (response(request_id) is not None) == expected_hover
    send("textDocument/didChange", {
        "textDocument": {"uri": uri, "version": 3},
        "contentChanges": [{"text": '<div\n class="lede"\n>\nfor\n</div>\n.lede text-red-500\n'}],
    })
    assert response(None, "textDocument/publishDiagnostics").get("version") == 3
    send("textDocument/completion", {
        "textDocument": {"uri": uri}, "position": {"line": 1, "character": 10},
    }, 8)
    assert any(item["label"] == "lede" for item in response(8))
    send("textDocument/hover", {
        "textDocument": {"uri": uri}, "position": {"line": 3, "character": 1},
    }, 9)
    assert response(9) is None, "markup body received tag hover"
    jsx_uri = "file:///tmp/crepus-lsp-context.TSX"
    send("textDocument/didOpen", {"textDocument": {
        "uri": jsx_uri, "languageId": "crepus", "version": 1, "text": "for",
    }})
    assert response(None, "textDocument/publishDiagnostics")["uri"] == jsx_uri
    send("textDocument/hover", {
        "textDocument": {"uri": jsx_uri}, "position": {"line": 0, "character": 1},
    }, 10)
    assert response(10) is None, "JSX body received indent tag hover"
    send("textDocument/didChange", {
        "textDocument": {"uri": jsx_uri, "version": 2},
        "contentChanges": [{"text": "<for"}],
    })
    assert response(None, "textDocument/publishDiagnostics").get("version") == 2
    send("textDocument/hover", {
        "textDocument": {"uri": jsx_uri}, "position": {"line": 0, "character": 2},
    }, 11)
    assert response(11) is not None, "JSX tag lost its hover"
    send("textDocument/didChange", {
        "textDocument": {"uri": uri, "version": 4},
        "contentChanges": [{"text": "# lede\n.lede text-red-500\ndiv"}],
    })
    assert response(None, "textDocument/publishDiagnostics").get("version") == 4
    send("textDocument/completion", {
        "textDocument": {"uri": uri}, "position": {"line": 0, "character": 6},
    }, 12)
    assert response(12) is None, "comment received class completions"
    send("textDocument/hover", {
        "textDocument": {"uri": uri}, "position": {"line": 0, "character": 4},
    }, 13)
    assert response(13) is None, "comment received alias hover"
    unicode_source = '<div class="😀 lede"/>\r\n.lede text-red-500\r\n'
    send("textDocument/didChange", {
        "textDocument": {"uri": uri, "version": 5},
        "contentChanges": [{"text": unicode_source}],
    })
    assert response(None, "textDocument/publishDiagnostics").get("version") == 5
    # Use opposite alias boundaries so byte/scalar offsets cannot both pass.
    completion_position = {"line": 0, "character": len('<div class="😀 lede'.encode("utf-16-le")) // 2}
    hover_position = {"line": 0, "character": len('<div class="😀 '.encode("utf-16-le")) // 2}
    send("textDocument/completion", {
        "textDocument": {"uri": uri}, "position": completion_position,
    }, 14)
    assert any(item["label"] == "lede" for item in response(14))
    send("textDocument/hover", {
        "textDocument": {"uri": uri}, "position": hover_position,
    }, 15)
    assert "text-red-500" in response(15)["contents"]["value"]
    send("textDocument/didChange", {
        "textDocument": {"uri": uri, "version": 6},
        "contentChanges": [{"text": "<div>😀\r\n</wrong>"}],
    })
    unicode_diagnostics = response(None, "textDocument/publishDiagnostics")
    assert unicode_diagnostics.get("version") == 6, unicode_diagnostics
    assert len(unicode_diagnostics["diagnostics"]) == 1, unicode_diagnostics
    assert unicode_diagnostics["diagnostics"][0]["range"]["start"]["line"] == 1, unicode_diagnostics
    expected_status = 1 if "--without-shutdown" in sys.argv else 0
    if expected_status == 0:
        send("shutdown", request_id=2)
        response(2)
    send("exit")
    server.wait(timeout=3)
    assert server.returncode == expected_status, server.stderr.read().decode()
    print(f"LSP exit with stdin open, expected status {expected_status}: passed")
finally:
    deadline.cancel()
    if server.poll() is None:
        server.kill()
    server.wait()
    server.stdin.close()
    server.stdout.close()
    server.stderr.close()
