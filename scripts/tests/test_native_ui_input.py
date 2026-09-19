from __future__ import annotations

import unittest
from unittest import mock

from scripts.native_ui_input import InputRejected, X11Input


class NativeUiInputTests(unittest.TestCase):
    def bridge(self) -> X11Input:
        bridge = X11Input.__new__(X11Input)
        bridge.window, bridge.owner_pid, bridge.root_pid = 100, 200, 150
        bridge.nonce, bridge.display = "test-run", 1
        bridge.sent, bridge.held_buttons, bridge.held_keys = set(), set(), set()
        bridge.closed = False
        bridge.record = mock.Mock()
        bridge.x11, bridge.xtest = mock.Mock(), mock.Mock()
        bridge._check_owner, bridge._check_focus = mock.Mock(), mock.Mock()
        bridge.client_pointer = mock.Mock(return_value={"same_screen": True, "client": [10, 10]})
        bridge.client_size = mock.Mock(return_value=(100, 100))
        return bridge

    def test_stale_nonce_and_repeated_step_never_emit_input(self) -> None:
        bridge = self.bridge()
        with self.assertRaises(InputRejected):
            bridge.send("step", "old-run", button=1, pressed=True)
        bridge.xtest.XTestFakeButtonEvent.assert_not_called()
        bridge.send("step", "test-run", button=1, pressed=True)
        with self.assertRaises(InputRejected):
            bridge.send("step", "test-run", button=1, pressed=True)
        self.assertEqual(bridge.xtest.XTestFakeButtonEvent.call_count, 1)

    def test_resize_requires_owned_focused_window_and_fresh_step(self):
        bridge = self.bridge()
        bridge._check_owner.side_effect = InputRejected("wrong owner")
        with self.assertRaises(InputRejected):
            bridge.resize("1", "test-run", 1280, 720)
        bridge.x11.XResizeWindow.assert_not_called()
        bridge._check_owner.side_effect = None
        bridge._check_focus.side_effect = InputRejected("lost focus")
        with self.assertRaises(InputRejected):
            bridge.resize("1", "test-run", 1280, 720)
        bridge.x11.XResizeWindow.assert_not_called()
        bridge._check_focus.side_effect = None
        bridge.resize("1", "test-run", 1280, 720)
        bridge.x11.XResizeWindow.assert_called_once_with(1, 100, 1280, 720)
        with self.assertRaises(InputRejected):
            bridge.resize("1", "test-run", 1920, 1080)
        self.assertEqual(bridge.x11.XResizeWindow.call_count, 1)

    def test_focus_loss_stops_input_and_cleanup_releases_held_button(self) -> None:
        bridge = self.bridge()
        bridge.send("press", "test-run", button=1, pressed=True)
        bridge._check_focus.side_effect = InputRejected("lost focus")
        with self.assertRaises(InputRejected):
            bridge.send("next", "test-run", button=3, pressed=True)
        bridge.close()
        bridge.close()
        self.assertEqual(bridge.xtest.XTestFakeButtonEvent.call_args_list,
                         [mock.call(1, 1, 1, 0), mock.call(1, 1, 0, 0)])
        bridge.x11.XCloseDisplay.assert_called_once()

    def test_wrong_pid_is_rejected_before_focus_or_input(self) -> None:
        bridge = self.bridge()
        with mock.patch("scripts.native_ui_input.process_descends_from", return_value=False):
            with self.assertRaises(InputRejected):
                X11Input._check_owner(bridge)
        bridge.xtest.XTestFakeButtonEvent.assert_not_called()

    def test_pointer_outside_client_rejects_press_but_allows_cleanup_release(self):
        bridge = self.bridge()
        bridge.client_pointer.return_value = {"same_screen": True, "client": [-1, 10]}
        with self.assertRaises(InputRejected):
            bridge.send("press", "test-run", button=1, pressed=True)
        bridge.xtest.XTestFakeButtonEvent.assert_not_called()
        bridge.send("release", "test-run", button=1, pressed=False)
        bridge.xtest.XTestFakeButtonEvent.assert_called_once_with(1, 1, 0, 0)

    def test_xprop_timeout_does_not_fall_back_to_input(self) -> None:
        import subprocess
        bridge = self.bridge()
        with mock.patch("scripts.native_ui_input.process_descends_from", return_value=True), \
             mock.patch("scripts.native_ui_input.subprocess.run", side_effect=subprocess.TimeoutExpired("xprop", 2)):
            with self.assertRaises(subprocess.TimeoutExpired):
                X11Input._check_owner(bridge)
        bridge.xtest.XTestFakeButtonEvent.assert_not_called()

    def test_wm_activation_timeout_never_forces_focus_or_emits_input(self) -> None:
        bridge = self.bridge()
        bridge.x11.XInternAtom.return_value = 10
        bridge._server_timestamp = mock.Mock(return_value=123)
        bridge._check_focus.side_effect = InputRejected("WM refused activation")
        with mock.patch("scripts.native_ui_input.time.monotonic", side_effect=[0, 3]):
            with self.assertRaises(InputRejected):
                bridge.activate()
        bridge.x11.XSendEvent.assert_called_once()
        bridge.x11.XSetInputFocus.assert_not_called()
        bridge.xtest.XTestFakeButtonEvent.assert_not_called()


if __name__ == "__main__":
    unittest.main()
