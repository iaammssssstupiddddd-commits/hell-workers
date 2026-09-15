from __future__ import annotations

import unittest
from unittest import mock

from scripts.native_ui_input import InputRejected
from scripts.native_ui_portal import PortalSession, PortalX11Input, physical_monitor_scale


class PortalInputTests(unittest.TestCase):
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
