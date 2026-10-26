import unittest
import gc
import weakref
import copy
from contextlib import ExitStack
from pathlib import Path
from unittest.mock import MagicMock, patch

import abi_generated
import crepuscularity_abi

from crepuscularity_abi import CrepusAbiSession


class CrepuscularityAbiTests(unittest.TestCase):
    def test_session_ownership_cannot_be_copied(self):
        for operation in [copy.copy, copy.deepcopy]:
            with self.subTest(operation=operation.__name__):
                # Every ABI call is a stub: an accidental copy cannot double-free Rust.
                library = MagicMock()
                library.crepus_session_new.return_value = 123
                library.crepus_session_set_template_string.return_value = 0
                with patch.object(crepuscularity_abi.ctypes, "CDLL", return_value=library):
                    session = CrepusAbiSession("counting-stub")
                    duplicate = None
                    try:
                        with self.assertRaises(TypeError):
                            duplicate = operation(session)
                        library.crepus_session_free.assert_not_called()
                        session.set_template("span")
                        library.crepus_session_set_template_string.assert_called_once_with(
                            123, b"span", None)
                        session.close()
                        session.close()
                        library.crepus_session_free.assert_called_once_with(123)
                    finally:
                        if duplicate is not None:
                            duplicate.close()
                        session.close()
                    del session, duplicate
                    gc.collect()
                    library.crepus_session_free.assert_called_once_with(123)

    def test_callback_errors_propagate_after_native_dispatch(self):
        error = ValueError("handler failed")

        def callback(event):
            raise error

        with CrepusAbiSession() as session:
            session.set_template('span\n  "Count {count}"')
            session.set_context({"count": "1"})
            session.on_event(callback)
            with patch.object(session._lib, "crepus_string_free",
                              wraps=session._lib.crepus_string_free) as free:
                with self.assertRaises(ValueError) as raised:
                    session.dispatch_event("bind:count:2")
                free.assert_called_once()
            self.assertIs(raised.exception, error)
            self.assertIn("Count 2", str(session.render_ir()))
            events = []
            session.on_event(events.append)
            self.assertIn("Count 3", str(session.dispatch_event("bind:count:3")))
            self.assertEqual(len(events), 1)

    def test_callback_cannot_reenter_session(self):
        with CrepusAbiSession() as session:
            session.set_template('span\n  "Count {count}"')
            session.set_context({"count": "1"})
            attempted = []

            def callback(event):
                operations = [
                    session.close,
                    session.render_ir,
                    lambda: session.set_template('span'),
                    lambda: session.set_context({"count": "3"}),
                    lambda: session.patch_context({"count": "3"}),
                    lambda: session.on_event(lambda event: None),
                    lambda: session.dispatch_event("bind:count:3"),
                ]
                for operation in operations:
                    with self.assertRaisesRegex(RuntimeError, "callback"):
                        operation()
                    attempted.append(operation)

            session.on_event(callback)
            # Mock every native entry point the callback tries to use so a
            # regression cannot free or alias the live Rust session in this test.
            names = ["free", "render_ir_json", "set_template_string", "set_context_json",
                     "apply_context_patch_json", "set_event_callback", "dispatch_event_json"]
            with ExitStack() as stack:
                mocks = [stack.enter_context(patch.object(session._lib, "crepus_session_" + name))
                         for name in names]
                session._callback_ref(b'{"handler":"event"}', None)
                for mock in mocks:
                    mock.assert_not_called()
            self.assertEqual(len(attempted), 7)
            self.assertIn("Count 1", str(session.render_ir()))

    def test_close_releases_callback_and_is_idempotent(self):
        class Handler:
            def __call__(self, event):
                pass

        session = CrepusAbiSession()
        handler = Handler()
        reference = weakref.ref(handler)
        session.on_event(handler)
        del handler
        gc.collect()
        self.assertIsNotNone(reference())
        session.close()
        session.close()
        gc.collect()
        self.assertIsNone(reference())

    def test_generated_and_handwritten_library_paths_match(self):
        for platform, filename in [
            ("darwin", "libcrepuscularity_abi.dylib"),
            ("linux", "libcrepuscularity_abi.so"),
            ("win32", "crepuscularity_abi.dll"),
        ]:
            with self.subTest(platform=platform), patch("sys.platform", platform):
                generated = abi_generated._default_lib()
                self.assertEqual(Path(generated).name, filename)
                self.assertEqual(generated, crepuscularity_abi._default_lib())

    def test_nul_template_inputs_are_rejected_without_replacing_template(self):
        with CrepusAbiSession() as session:
            session.set_template('span\n  "original"')
            for template, base_dir in [('span\x00ignored', None), ('span', '/tmp\x00ignored')]:
                with self.subTest(template=template, base_dir=base_dir):
                    with self.assertRaises(ValueError):
                        session.set_template(template, base_dir)
                    self.assertIn("original", str(session.render_ir()))

    def test_nul_event_is_rejected_without_dispatching(self):
        events = []
        with CrepusAbiSession() as session:
            session.set_template('span\n  "Count {count}"')
            session.set_context({"count": "1"})
            session.on_event(events.append)
            with self.assertRaises(ValueError):
                session.dispatch_event('{"handler":"bind:count:2"}\x00ignored')
            self.assertIn("Count 1", str(session.render_ir()))
        self.assertEqual(events, [])

    def test_nonfinite_event_numbers_do_not_dispatch_or_mutate_state(self):
        events = []
        with CrepusAbiSession() as session:
            session.set_template('span "Count {count}"')
            session.set_context({"count": "1"})
            session.on_event(events.append)
            before = session.render_ir()
            for value in [float("inf"), float("-inf"), float("nan")]:
                for field in ["payload", "context"]:
                    with self.subTest(value=value, field=field):
                        with self.assertRaises(ValueError):
                            session.dispatch_event({
                                "handler": "bind:count:2", field: {"bad": value}
                            })
                        self.assertEqual(session.render_ir(), before)
                        self.assertEqual(events, [])
            result = session.dispatch_event({"handler": "bind:count:3", "payload": 1.25})
            self.assertIn("Count 3", str(result["ir"]))
            self.assertEqual(events[0]["payload"], 1.25)

    def test_render_and_dispatch_event(self):
        events = []
        with CrepusAbiSession() as session:
            session.set_template('input bind=count\nspan\n  "Count {count}"')
            session.set_context({"count": "1"})
            session.on_event(events.append)
            first = session.render_ir()
            self.assertGreaterEqual(first["version"], 4)  # ponytail: version may bump, just check range
            self.assertIn("Count 1", str(first))
            result = session.dispatch_event({"handler": "bind:count:2"})
            self.assertEqual(result["handler"], "bind:count:2")
            self.assertIn("Count 2", str(result["ir"]))
        self.assertEqual(events[0]["handler"], "bind:count:2")


if __name__ == "__main__":
    unittest.main()
