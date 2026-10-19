import pathlib
import unittest

import os
from unittest.mock import patch
import tempfile
from crepuscularity_plugin import ViewSession, render_html, render_ir, _crepus_bin


class CrepuscularityPluginTests(unittest.TestCase):
    def test_context_render_preserves_frontend_filename(self):
        fixture = pathlib.Path(__file__).parents[1] / "fixtures" / "frontend.svelte"
        ir = render_ir(fixture, {"name": "Ada"}, fixture.parent)
        self.assertIn("Frontend content", str(ir.root))
        self.assertNotIn("ignored", str(ir.root))

    def test_secure_input_renders_as_password(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = pathlib.Path(directory) / "password.crepus"
            fixture.write_text('input secure bind=password\n')
            self.assertEqual(
                render_html(fixture),
                '<input type="password" data-bind="password" placeholder="">',
            )

    def test_crepus_bin_validation(self):
        valid_paths = [
            "crepus",
            "crepus.exe",
            "/usr/bin/crepus",
            "/opt/crepuscularity/crepus.exe",
        ]
        invalid_paths = [
            "sh",
            "/bin/sh",
            "../crepus",
            "./crepus",
        ]

        for path in valid_paths:
            with patch.dict(os.environ, {"CREPUS_BIN": path}):
                self.assertEqual(_crepus_bin(), path)

        for path in invalid_paths:
            with patch.dict(os.environ, {"CREPUS_BIN": path}):
                with self.assertRaises(ValueError):
                    _crepus_bin()
    def test_render_ir(self):
        fixture = pathlib.Path(__file__).parents[1] / "fixtures" / "hello.crepus"
        allowed_dir = pathlib.Path(__file__).parents[1] / "fixtures"
        ir = render_ir(fixture, {"name": "Ada"}, allowed_dir)
        self.assertEqual(ir.version, 7)
        self.assertEqual(ir.root[0]["children"][0]["content"], "Hello Ada")
        self.assertEqual(render_html(fixture, {"name": "Ada"}, allowed_dir), '<div data-crepus-kind="stack" data-axis="column">Hello Ada</div>')

    def test_view_session_dispatches_bind_and_rerenders(self):
        fixture = pathlib.Path(__file__).parents[1] / "fixtures" / "interactive.crepus"
        allowed_dir = pathlib.Path(__file__).parents[1] / "fixtures"
        session = ViewSession(fixture, {"count": "1"}, allowed_dir)
        self.assertIn("Count 1", session.render_html())
        ir = session.dispatch("bind:count:2")
        self.assertEqual(session.context["count"], "2")
        self.assertIn("Count 2", str(ir.root))
        self.assertIn("Count 2", session.render_html())

    def test_path_traversal_validation(self):
        fixture = pathlib.Path(__file__).parents[1] / "fixtures" / "hello.crepus"
        # Provide a dummy allowed directory that does not contain the fixture
        dummy_dir = pathlib.Path(__file__).parent
        with patch("crepuscularity_plugin.subprocess.run") as run:
            with self.assertRaises(ValueError):
                render_ir(fixture, allowed_dir=dummy_dir)
            run.assert_not_called()
        with self.assertRaises(ValueError):
            render_ir(fixture, {"name": "Ada"}, dummy_dir)

        with self.assertRaises(ValueError):
            # Also test relative path traversal out of allowed_dir
            render_ir(dummy_dir / ".." / "fixtures" / "hello.crepus", {"name": "Ada"}, dummy_dir)


if __name__ == "__main__":
    unittest.main()
