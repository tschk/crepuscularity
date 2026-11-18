import unittest
import copy
import gc
import weakref
from unittest.mock import MagicMock, patch

from crepuscularity_abi import CrepusAbiSession


class CrepuscularityAbiTests(unittest.TestCase):
    def test_sessions_cannot_duplicate_native_ownership(self):
        library = MagicMock()
        library.crepus_session_new.return_value = 123
        with patch("crepuscularity_abi.ctypes.CDLL", return_value=library):
            session = CrepusAbiSession("stub")
            for operation in (copy.copy, copy.deepcopy):
                with self.assertRaises(TypeError):
                    operation(session)
            session.close()
            session.close()
            library.crepus_session_free.assert_called_once_with(123)

    def test_callback_errors_propagate_and_session_remains_usable(self):
        error = ValueError("handler failed")
        def fail(event):
            raise error
        with CrepusAbiSession() as session:
            session.set_template('span "Count {count}"')
            session.set_context({"count": 1})
            session.on_event(fail)
            with self.assertRaises(ValueError) as raised:
                session.dispatch_event("bind:count:2")
            self.assertIs(raised.exception, error)
            self.assertIn("Count 2", str(session.render_ir()))
            events = []
            session.on_event(events.append)
            session.dispatch_event("bind:count:3")
            self.assertEqual(events[0]["handler"], "bind:count:3")

    def test_callback_cannot_reenter_and_close_releases_capture(self):
        class Handler:
            def __call__(self, event):
                with self_test.assertRaisesRegex(RuntimeError, "callback"):
                    session.close()
                with self_test.assertRaisesRegex(RuntimeError, "callback"):
                    session.render_ir()
        self_test = self
        session = CrepusAbiSession()
        session.set_template('span "Alive"')
        handler = Handler()
        reference = weakref.ref(handler)
        session.on_event(handler)
        del handler
        session.dispatch_event("save")
        self.assertIn("Alive", str(session.render_ir()))
        session.close()
        gc.collect()
        self.assertIsNone(reference())

    def test_invalid_c_string_and_nonfinite_inputs_do_not_mutate_state(self):
        with CrepusAbiSession() as session:
            session.set_template('span "Count {count}"')
            session.set_context({"count": 1})
            before = session.render_ir()
            for operation in [
                lambda: session.set_template('span\0ignored'),
                lambda: session.set_template('span', '/base\0ignored'),
                lambda: session.dispatch_event('bind:count:2\0ignored'),
                lambda: session.dispatch_event({"context": {"count": float("inf")}}),
                lambda: session.set_context({"count": float("nan")}),
                lambda: session.patch_context({"count": float("-inf")}),
            ]:
                with self.assertRaises(ValueError):
                    operation()
                self.assertEqual(session.render_ir(), before)

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
