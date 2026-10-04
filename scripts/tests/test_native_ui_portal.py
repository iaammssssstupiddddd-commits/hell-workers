from __future__ import annotations

import unittest
import os
import json
import tempfile
from pathlib import Path
from unittest import mock

from scripts.native_ui_input import InputRejected
from scripts.native_ui_portal import ConsentStore, PortalSession, PortalX11Input, physical_monitor_scale


class PersistentConsentTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "consent.json"

    def store(self):
        store = ConsentStore(self.path)
        self.addCleanup(store.close)
        return store

    def test_consume_rotates_and_unknown_result_cannot_reuse(self):
        store = self.store()
        self.assertIsNone(store.consume("desktop"))
        store.save("desktop", "first")
        self.assertEqual(store.consume("desktop"), "first")
        self.assertIsNone(store.consume("desktop"))
        store.save("desktop", "rotated")
        store.close()
        self.assertEqual(self.store().consume("desktop"), "rotated")
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)

    def test_busy_foreign_desktop_and_corruption_rejected(self):
        store = self.store()
        store.save("desktop", "first")
        with self.assertRaisesRegex(InputRejected, "busy"):
            ConsentStore(self.path)
        with self.assertRaisesRegex(InputRejected, "context"):
            store.consume("foreign")
        self.assertEqual(json.loads(self.path.read_text())["token"], "first")
        store.close()
        self.path.write_text("broken")
        with self.assertRaisesRegex(InputRejected, "journal"):
            self.store().consume("desktop")

    def test_symlink_hardlink_open_permissions_and_large_file_rejected(self):
        target = self.path.parent / "target"
        target.write_text("keep")
        self.path.symlink_to(target)
        with self.assertRaises(OSError):
            self.store()
        self.assertEqual(target.read_text(), "keep")
        self.path.unlink()
        os.link(target, self.path)
        with self.assertRaises(InputRejected):
            self.store()
        self.path.unlink()
        self.path.write_text("{}")
        self.path.chmod(0o644)
        with self.assertRaises(InputRejected):
            self.store()
        self.path.chmod(0o600)
        self.path.write_text("x" * 16385)
        with self.assertRaisesRegex(InputRejected, "bound"):
            self.store().consume("desktop")

    def session(self, *, persistent=True, response=None, version=2):
        session = PortalSession.__new__(PortalSession)
        session.ready, session.session = False, None
        session.consent_file = self.path if persistent else None
        session.consent_store, session.persistence = None, "disabled"
        session.GLib = mock.Mock()
        session.GLib.Variant.side_effect = lambda kind, value: value
        session.Gio, session.bus = mock.Mock(), mock.Mock()
        session.bus.call_sync.return_value.unpack.return_value = ("desktop",)
        session.call = mock.Mock()
        session.call.return_value.unpack.return_value = (version,)
        session.request = mock.Mock(side_effect=[{"session_handle": "/session"}, {},
            response if response is not None else {"devices": 3, "restore_token": "rotated"}])
        self.addCleanup(lambda: session.consent_store and session.consent_store.close())
        return session

    def test_fresh_permission_then_second_session_restores_rotated_token(self):
        first = self.session()
        first.start(100)
        options = first.request.call_args_list[1].args[2][1]
        self.assertEqual(options["persist_mode"], 2)
        self.assertNotIn("restore_token", options)
        self.assertTrue(first.ready)
        self.assertEqual(first.persistence, "saved")
        first.start(101)
        self.assertEqual(first.request.call_count, 3)  # reuse live session
        first.consent_store.close()
        second = self.session()
        second.start(102)
        self.assertEqual(second.request.call_args_list[1].args[2][1]["restore_token"], "rotated")

    def test_default_is_nonpersistent_and_old_interface_is_rejected_before_request(self):
        plain = self.session(persistent=False)
        plain.start(100)
        self.assertEqual(plain.request.call_args_list[1].args[2][1]["persist_mode"], 0)
        old = self.session(version=1)
        with self.assertRaises(InputRejected):
            old.start(100)
        old.request.assert_not_called()
        self.assertFalse(self.path.exists())

    def test_denial_timeout_and_partial_grant_never_save_old_token_or_enable_input(self):
        for response in (InputRejected("denied"), InputRejected("timeout"), {"devices": 1}):
            with self.subTest(response=type(response).__name__):
                store = self.store()
                store.save("desktop", "single-use")
                store.close()
                session = self.session(response=response)
                with self.assertRaises(InputRejected):
                    session.start(100)
                self.assertFalse(session.ready)
                self.assertIsNone(json.loads(self.path.read_text())["token"])
                session.consent_store.close()

    def test_os_declines_persistence_live_input_grant_is_not_claimed_persistent(self):
        session = self.session(response={"devices": 3})
        session.start(100)
        self.assertTrue(session.ready)
        self.assertEqual(session.persistence, "not-granted")
        self.assertIsNone(json.loads(self.path.read_text())["token"])


class PortalInputTests(unittest.TestCase):
    def test_parent_is_presented_then_focus_is_required_after_consent(self):
        bridge = PortalX11Input.__new__(PortalX11Input)
        bridge.window = 100
        bridge._check_owner = mock.Mock()
        bridge.root_size = mock.Mock(return_value=(2880, 1800))
        bridge.portal = mock.Mock()
        events = []
        bridge._request_activation = mock.Mock(side_effect=lambda: events.append("present"))
        bridge.portal.start.side_effect = lambda window: events.append("consent")
        with mock.patch("scripts.native_ui_input.X11Input.activate",
                        side_effect=lambda: events.append("focus")):
            bridge.activate()
        self.assertEqual(events, ["present", "consent", "focus"])

    def test_missing_focus_after_consent_still_rejects_input(self):
        bridge = PortalX11Input.__new__(PortalX11Input)
        bridge.window = 100
        bridge._check_owner = mock.Mock()
        bridge._request_activation = mock.Mock()
        bridge.root_size = mock.Mock(return_value=(2880, 1800))
        bridge.portal = mock.Mock()
        with mock.patch("scripts.native_ui_input.X11Input.activate",
                        side_effect=InputRejected("parent not active")):
            with self.assertRaises(InputRejected):
                bridge.activate()
        bridge.portal.start.assert_called_once_with(100)
        bridge.portal.notify.assert_not_called()

    def test_no_notify_before_consent_or_after_closed(self):
        session = PortalSession.__new__(PortalSession)
        session.ready = False
        session.pump, session.call = mock.Mock(), mock.Mock()
        with self.assertRaises(InputRejected):
            session.notify("NotifyKeyboardKeycode", "iu", 57, 1)
        session.call.assert_not_called()
        session.ready, session.session = True, "/session/test"
        session._closed(None, None, session.session, None, None, None)
        with self.assertRaises(InputRejected):
            session.notify("NotifyPointerButton", "iu", 272, 1)
        session.call.assert_not_called()

    def test_revoked_session_is_not_silently_reopened(self):
        session = PortalSession.__new__(PortalSession)
        session.ready, session.session = False, "/session/test"
        with self.assertRaises(InputRejected):
            session.start(100)

    def test_owned_client_guard_precedes_portal_input(self):
        bridge = PortalX11Input.__new__(PortalX11Input)
        bridge.nonce, bridge.sent = "nonce", set()
        bridge._check_owner = mock.Mock()
        bridge._check_focus = mock.Mock(side_effect=InputRejected("lost focus"))
        bridge.portal = mock.Mock()
        with self.assertRaises(InputRejected):
            bridge.send("1", "nonce", button=1, pressed=True)
        bridge.portal.notify.assert_not_called()

    def test_pointer_request_uses_observed_root_delta_once(self):
        bridge = PortalX11Input.__new__(PortalX11Input)
        bridge.portal = mock.Mock()
        bridge.mapping = (1, 2.0)
        bridge.portal.motion_mapping.return_value = bridge.mapping
        bridge.root_size = mock.Mock(return_value=(2880, 1800))
        bridge.client_pointer = mock.Mock(return_value={"same_screen": True, "root": [500, 400]})
        bridge._motion(520, 370)
        bridge.portal.notify.assert_called_once_with("NotifyPointerMotion", "dd", 10.0, -15.0)

    def test_monitor_contract_requires_matching_physical_extent(self):
        spec = ('eDP-1', 'vendor', 'model', 'serial')
        modes = [('current', 2880, 1800, 60., 1., [1., 2.], {'is-current': True})]
        state = (5, [(spec, modes, {})], [(0, 0, 2., 0, True, [spec], {})], {'layout-mode': 1})
        self.assertEqual(physical_monitor_scale(state, (2880, 1800)), (5, 2.))
        with self.assertRaises(InputRejected):
            physical_monitor_scale(state, (1440, 900))
        with self.assertRaises(InputRejected):
            physical_monitor_scale((*state[:3], {'layout-mode': 2}), (2880, 1800))

    def test_wheel_release_and_key_mapping(self):
        bridge = PortalX11Input.__new__(PortalX11Input)
        bridge.portal = mock.Mock()
        bridge._button(5, True)
        bridge.portal.notify.assert_not_called()
        bridge._button(5, False)
        bridge._key(65, True)
        self.assertEqual(bridge.portal.notify.call_args_list, [
            mock.call("NotifyPointerAxisDiscrete", "ui", 0, 1),
            mock.call("NotifyKeyboardKeycode", "iu", 57, 1)])


if __name__ == "__main__":
    unittest.main()
