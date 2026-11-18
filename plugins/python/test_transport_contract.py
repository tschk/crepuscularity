import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from crepuscularity_plugin import render_ir


class TransportContractTests(unittest.TestCase):
    def test_binary_transport_preserves_source_bytes_and_drains_both_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.crepus"
            text = 'span "雪🦋\r\nsecond\rthird"' + ' ' * 131072
            source.write_bytes(text.encode("utf-8"))
            binary = root / "crepus"
            binary.write_text(f"#!{sys.executable}\n" + """
import json,sys
sys.stderr.buffer.write(b'diagnostic' * 32768)
request = json.load(sys.stdin)
sys.stdout.buffer.write(json.dumps({'version':7,'root':[
    {'kind':'text','content':request['template']},
    {'kind':'text','content':request['filename']}
]},ensure_ascii=False).encode('utf-8'))
""", encoding="utf-8")
            binary.chmod(0o700)
            with patch.dict(os.environ, {"CREPUS_BIN": str(binary)}):
                ir = render_ir(source, {}, root)
            self.assertEqual(ir.root[0]["content"], text)
            self.assertEqual(ir.root[1]["content"], str(source))

    def test_invalid_utf8_is_rejected_in_source_and_cli_response(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.crepus"
            source.write_bytes(b'span "\xff"')
            with self.assertRaises(UnicodeDecodeError):
                render_ir(source, {}, root)
            source.write_text('span "valid"')
            binary = root / "crepus"
            binary.write_text(f"#!{sys.executable}\nimport sys\nsys.stdin.buffer.read()\nsys.stdout.buffer.write(b'{{\"version\":7,\"root\":[{{\"kind\":\"text\",\"content\":\"\\xff\"}}]}}')\n")
            binary.chmod(0o700)
            with patch.dict(os.environ, {"CREPUS_BIN": str(binary)}):
                with self.assertRaises(UnicodeDecodeError):
                    render_ir(source, {}, root)

    def test_failure_diagnostic_is_separate_from_invalid_stdout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.crepus"
            source.write_text('span "valid"')
            binary = root / "crepus"
            diagnostic = "雪 error\r\nsecond\rthird"
            binary.write_text(f"#!{sys.executable}\nimport sys\nsys.stdin.buffer.read()\nsys.stdout.buffer.write(bytes([255]))\nsys.stderr.buffer.write({diagnostic.encode('utf-8')!r})\nsys.exit(9)\n")
            binary.chmod(0o700)
            with patch.dict(os.environ, {"CREPUS_BIN": str(binary)}):
                with self.assertRaisesRegex(RuntimeError, "雪 error\nsecond\nthird"):
                    render_ir(source, {}, root)
