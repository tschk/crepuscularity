import pathlib
import unittest
import json
import subprocess
import tempfile

import os
from unittest.mock import patch
from crepuscularity_plugin import ViewSession, render_html, render_ir, _crepus_bin


class PathBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.root = pathlib.Path(self.scratch.name).resolve()
        self.allowed = self.root / "allowed"
        self.allowed.mkdir()
        self.inside = self.allowed / "inside.crepus"
        self.inside.write_text('span "allowed"')
        self.outside = self.root / "outside.crepus"
        self.outside.write_text('span "outside"')
        self.reply = subprocess.CompletedProcess(
            [], 0, stdout='{"version":7,"root":[{"kind":"text","content":"allowed"}]}', stderr=""
        )

    def test_rejects_outside_paths_before_reading_or_launching(self):
        sibling = self.root / "allowed-sibling"
        sibling.mkdir()
        sibling_file = sibling / "view.crepus"
        sibling_file.write_text('span "outside"')
        paths = [self.outside, self.allowed / ".." / "outside.crepus", sibling_file]
        for render in (render_ir, render_html):
            for context in (None, {}):
                for candidate in paths:
                    with self.subTest(render=render.__name__, context=context, path=candidate):
                        with patch("crepuscularity_plugin.subprocess.run") as run, patch.object(pathlib.Path, "read_text") as read:
                            with self.assertRaisesRegex(ValueError, "Path traversal detected"):
                                render(candidate, context, self.allowed)
                            run.assert_not_called()
                            read.assert_not_called()

    def test_rejects_symlink_escape(self):
        alias = self.allowed / "escape.crepus"
        try:
            alias.symlink_to(self.outside)
        except OSError as err:
            self.skipTest(f"symlinks unavailable: {err}")
        for render in (render_ir, render_html):
            for context in (None, {}):
                with self.subTest(render=render.__name__, context=context):
                    with patch("crepuscularity_plugin.subprocess.run") as run:
                        with self.assertRaisesRegex(ValueError, "Path traversal detected"):
                            render(alias, context, self.allowed)
                        run.assert_not_called()

    def test_checked_alias_uses_canonical_path_and_include_base(self):
        alias = self.root / "alias.crepus"
        allowed_alias = self.root / "allowed-alias"
        try:
            alias.symlink_to(self.inside)
            allowed_alias.symlink_to(self.allowed, target_is_directory=True)
        except OSError as err:
            self.skipTest(f"symlinks unavailable: {err}")
        for context in (None, {"name": "Ada"}):
            with self.subTest(context=context):
                with patch("crepuscularity_plugin.subprocess.run", return_value=self.reply) as run:
                    ir = render_ir(alias, context, allowed_alias)
                self.assertEqual(ir.root[0]["content"], "allowed")
                if context is None:
                    self.assertEqual(run.call_args.args[0][-1], str(self.inside))
                    self.assertIsNone(run.call_args.kwargs["input"])
                else:
                    payload = json.loads(run.call_args.kwargs["input"])
                    self.assertEqual(payload["baseDir"], str(self.allowed))
                    self.assertEqual(payload["template"], self.inside.read_text())
                    self.assertEqual(payload["context"], context)

    def test_context_defaults_to_current_directory_boundary(self):
        with patch("crepuscularity_plugin.Path.cwd", return_value=self.allowed):
            with patch("crepuscularity_plugin.subprocess.run") as run:
                with self.assertRaisesRegex(ValueError, "Path traversal detected"):
                    render_ir(self.outside, {})
                run.assert_not_called()

    def test_no_context_or_policy_preserves_unrestricted_file_mode(self):
        for candidate in (self.outside, "./-view.crepus"):
            with self.subTest(path=candidate):
                with patch("crepuscularity_plugin.subprocess.run", return_value=self.reply) as run:
                    render_ir(candidate)
                self.assertEqual(run.call_args.args[0][-1], str(candidate))
                self.assertIsNone(run.call_args.kwargs["input"])

    def test_valid_inside_template_for_each_public_render(self):
        for context in (None, {}):
            with self.subTest(context=context):
                with patch("crepuscularity_plugin.subprocess.run", return_value=self.reply):
                    self.assertEqual(render_ir(self.inside, context, self.allowed).version, 7)
                    self.assertEqual(render_html(self.inside, context, self.allowed), "allowed")


class CrepuscularityPluginTests(unittest.TestCase):
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
        with self.assertRaises(ValueError):
            render_ir(fixture, {"name": "Ada"}, dummy_dir)

        with self.assertRaises(ValueError):
            # Also test relative path traversal out of allowed_dir
            render_ir(dummy_dir / ".." / "fixtures" / "hello.crepus", {"name": "Ada"}, dummy_dir)


if __name__ == "__main__":
    unittest.main()
