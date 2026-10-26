"""Public wrapper transport under an isolated ASCII locale; CLI is a labelled stub."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


@unittest.skipIf(os.name == "nt", "executable transport stub uses a Unix shebang")
class TransportEncodingTests(unittest.TestCase):
    def run_child(self, root, code, invalid=False, expected_template=None, response_body=None):
        stub = root / "crepus"
        stub.write_text(
            f"#!{sys.executable}\n"
            "import json,sys\n"
            "if '--stdin-json' in sys.argv:\n"
            " payload=json.loads(sys.stdin.buffer.read().decode('utf-8'))\n"
            " assert '\\u96ea' in payload['template']\n"
            + (f" assert payload['template'].encode('utf-8').hex() == {expected_template.hex()!r}\n" if expected_template is not None else "")
            + (response_body if response_body is not None else "sys.stdout.buffer.write(b'\\xff')\n" if invalid else
               "sys.stdout.buffer.write(json.dumps({'version':7,'root':[{'kind':'text','content':'\\u96ea'}]},ensure_ascii=False).encode('utf-8'))\n"),
            encoding="utf-8",
        )
        stub.chmod(0o755)
        env = {**os.environ, "LC_ALL": "C", "PYTHONUTF8": "0", "PYTHONCOERCECLOCALE": "0",
               "CREPUS_BIN": str(stub), "PYTHONPATH": str(Path(__file__).resolve().parent)}
        return subprocess.run([sys.executable, "-c", code], cwd=root, env=env,
                              capture_output=True, timeout=10)

    def test_unicode_output_and_context_template_reads_are_utf8(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "view.crepus").write_text('span "雪"', encoding="utf-8")
            for context in ["None", "{}"]:
                with self.subTest(context=context):
                    result = self.run_child(root,
                        "from crepuscularity_plugin import render_ir\n"
                        f"ir=render_ir('view.crepus',context={context})\n"
                        "assert ir.root[0]['content']=='\\u96ea'\n")
                    self.assertEqual(result.returncode, 0, result.stderr.decode("utf-8", "replace"))

    def test_context_template_preserves_crlf_and_lone_cr_verbatim(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            raw = '<button onClick="save\r\nnext\rend">雪</button>\r\n'.encode("utf-8")
            (root / "view.crepus").write_bytes(raw)
            result = self.run_child(root,
                "from crepuscularity_plugin import render_ir\n"
                "render_ir('view.crepus', context={})\n", expected_template=raw)
            self.assertEqual(result.returncode, 0, result.stderr.decode("utf-8", "replace"))

    def test_invalid_stdout_utf8_is_still_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "view.crepus").write_text('span "ASCII"', encoding="utf-8")
            result = self.run_child(root,
                "from crepuscularity_plugin import render_ir\n"
                "try: render_ir('view.crepus')\n"
                "except UnicodeDecodeError: pass\n"
                "else: raise AssertionError('invalid UTF-8 accepted')\n", invalid=True)
            self.assertEqual(result.returncode, 0, result.stderr.decode("utf-8", "replace"))

    def test_failed_child_uses_normalized_stderr_despite_invalid_stdout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "view.crepus").write_text('span "雪"', encoding="utf-8")
            body = ("sys.stdout.buffer.write(b'\\xff')\n"
                    "sys.stderr.buffer.write('\\u96ea first\\r\\nsecond\\rthird\\n'.encode('utf-8'))\n"
                    "sys.exit(9)\n")
            for context in ["None", "{}"]:
                with self.subTest(context=context):
                    result = self.run_child(root,
                        "from crepuscularity_plugin import render_ir\n"
                        f"try: render_ir('view.crepus',context={context})\n"
                        "except RuntimeError as error: assert str(error)=='\\u96ea first\\nsecond\\nthird'\n"
                        "else: raise AssertionError('CLI failure accepted')\n", response_body=body)
                    self.assertEqual(result.returncode, 0, result.stderr.decode("utf-8", "replace"))

    def test_successful_child_invalid_stderr_is_still_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "view.crepus").write_text('span "雪"', encoding="utf-8")
            result = self.run_child(root,
                "from crepuscularity_plugin import render_ir\n"
                "try: render_ir('view.crepus',context={})\n"
                "except UnicodeDecodeError: pass\n"
                "else: raise AssertionError('invalid stderr accepted')\n",
                response_body="sys.stdout.buffer.write(b'{\"version\":7,\"root\":[]}')\n"
                              "sys.stderr.buffer.write(b'\\xff')\n")
            self.assertEqual(result.returncode, 0, result.stderr.decode("utf-8", "replace"))

    def test_invalid_template_utf8_is_still_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "view.crepus").write_bytes(b'\xff')
            result = self.run_child(root,
                "from crepuscularity_plugin import render_ir\n"
                "try: render_ir('view.crepus',context={})\n"
                "except UnicodeDecodeError: pass\n"
                "else: raise AssertionError('invalid UTF-8 accepted')\n")
            self.assertEqual(result.returncode, 0, result.stderr.decode("utf-8", "replace"))


if __name__ == "__main__":
    unittest.main()
