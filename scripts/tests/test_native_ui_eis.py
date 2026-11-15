import unittest
from unittest import mock

from scripts.native_ui_eis import ABSOLUTE, BUTTON, KEYBOARD, SCROLL, EisSender, PortalEisInput
from scripts.native_ui_input import InputRejected


def sender():
    value = EisSender.__new__(EisSender)
    value.lib = mock.Mock()
    value.context = 1
    value.ready = True
    value.allow_scroll = False
    value.devices, value.active, value.seats = {10: {ABSOLUTE, BUTTON}, 20: {KEYBOARD}}, {10, 20}, {30}
    value.held_keys, value.held_buttons = set(), set()
    value.root_size, value.mapping = (2880, 1800), (1, 2.)
    value.lib.ei_get_event.return_value = None
    value.lib.ei_device_get_type.return_value = 1
    value.lib.ei_device_get_region.side_effect = lambda _device, index: 40 if index == 0 else None
    for name, result in zip(("x", "y", "width", "height", "physical_scale"), (0, 0, 1440, 900, 2.)):
        getattr(value.lib, "ei_region_get_" + name).return_value = result
    value.lib.ei_now.return_value = 1000
    return value


class EisTests(unittest.TestCase):
    def test_scroll_flag_is_boolean_and_capability_binding_is_opt_in(self):
        portal = mock.Mock(ready=True, granted_devices=3)
        for flag in (1, "true", None):
            with mock.patch("scripts.native_ui_eis.bind_library") as bind:
                with self.assertRaises(InputRejected):
                    EisSender(portal, (2880, 1800), (1, 2.), allow_scroll=flag)
                bind.assert_not_called()
        for enabled, offered in ((False, False), (True, True), (True, False)):
            value = sender()
            value.allow_scroll = enabled
            value.seats.clear()
            value.lib.ei_get_event.side_effect = [99, None]
            value.lib.ei_event_get_type.return_value = 3
            value.lib.ei_event_get_seat.return_value = 30
            value.lib.ei_seat_has_capability.side_effect = lambda _, cap: cap != SCROLL or offered
            if enabled and not offered:
                with self.assertRaises(InputRejected):
                    value.pump()
                value.lib.ei_seat_bind_capabilities.assert_not_called()
            else:
                value.pump()
                bound = value.lib.ei_seat_bind_capabilities.call_args.args
                self.assertEqual([cap.value for cap in bound[1:-1]],
                                 [ABSOLUTE, KEYBOARD, BUTTON] + ([SCROLL] if enabled else []))

    def test_scroll_is_explicit_single_discrete_click_on_the_owned_region(self):
        value = sender()
        with self.assertRaises(InputRejected):
            value.send("scroll_discrete", 0, 120)
        value.lib.ei_device_scroll_discrete.assert_not_called()
        value.allow_scroll = True
        value.devices[10].add(SCROLL)
        value.send("scroll_discrete", 0, 120)
        value.lib.ei_device_scroll_discrete.assert_called_once_with(10, 0, 120)
        value.lib.ei_device_frame.assert_called_once_with(10, 1000)
        value.lib.ei_device_button_button.assert_not_called()
        self.assertFalse(value.held_buttons or value.held_keys)

    def test_scroll_rejects_missing_multiple_foreign_or_invalid_requests(self):
        for mode in ("missing", "duplicate", "foreign", "horizontal", "fraction", "bool", "multiple", "zero", "unknown"):
            value = sender()
            value.allow_scroll = True
            value.devices[10].add(SCROLL)
            operation, args = "scroll_discrete", (0, 120)
            if mode == "missing":
                value.devices[10].remove(SCROLL)
            elif mode in ("duplicate", "foreign"):
                value.devices[30] = {SCROLL}
                value.active.add(30)
                if mode == "foreign":
                    value.devices[10].remove(SCROLL)
            else:
                operation, args = {"horizontal": (operation, (120, 120)), "fraction": (operation, (0, 120.)),
                    "bool": (operation, (False, 120)), "multiple": (operation, (0, 240)),
                    "zero": (operation, (0, 0)), "unknown": ("scroll_delta", (0, 120))}[mode]
            with self.subTest(mode=mode), self.assertRaises(InputRejected):
                value.send(operation, *args)
            value.lib.ei_device_scroll_discrete.assert_not_called()

    def test_existing_consent_is_required_before_connect_or_library(self):
        for ready, devices in ((False, 3), (True, 1), (True, True)):
            portal = mock.Mock(ready=ready, granted_devices=devices)
            with mock.patch("scripts.native_ui_eis.bind_library") as bind:
                with self.assertRaises(InputRejected):
                    EisSender(portal, (2880, 1800), (1, 2.))
                bind.assert_not_called()
                portal.bus.call_with_unix_fd_list_sync.assert_not_called()

    def test_unique_absolute_region_and_direct_frame(self):
        value = sender()
        value.send("pointer_motion_absolute", 1200, 900)
        value.lib.ei_device_pointer_motion_absolute.assert_called_once_with(10, 600., 450.)
        value.lib.ei_device_frame.assert_called_once_with(10, 1000)
        value.lib.ei_device_keyboard_key.assert_not_called()
        self.assertEqual(value.region_contract, {"rect": [0, 0, 1440, 900], "scale": 2.0, "root_size": [2880, 1800]})

    def test_keyboard_is_not_sent_on_pointer_device(self):
        value = sender()
        value.send("keyboard_key", 48, True)
        value.lib.ei_device_keyboard_key.assert_called_once_with(20, 48, True)
        value.lib.ei_device_frame.assert_called_once_with(20, 1000)
        self.assertEqual(value.held_keys, {(20, 48)})

    def test_mismatched_or_multiple_regions_are_rejected_before_send(self):
        for name, result in (("x", 1), ("y", 1), ("width", 2880), ("height", 1800),
                             ("physical_scale", 1.), ("physical_scale", float("nan"))):
            value = sender()
            getattr(value.lib, "ei_region_get_" + name).return_value = result
            with self.subTest(name=name, result=result), self.assertRaises(InputRejected):
                value.send("pointer_motion_absolute", 1200, 900)
            value.lib.ei_device_pointer_motion_absolute.assert_not_called()
        value = sender()
        value.lib.ei_device_get_region.side_effect = None
        value.lib.ei_device_get_region.return_value = 40
        with self.assertRaises(InputRejected):
            value.region()

    def test_duplicate_device_missing_button_and_physical_device_fail(self):
        for change in ("duplicate", "button", "physical"):
            value = sender()
            if change == "duplicate":
                value.devices[11], value.active = {ABSOLUTE, BUTTON}, {10, 11, 20}
            elif change == "button":
                value.devices[10].remove(BUTTON)
            else:
                value.lib.ei_device_get_type.return_value = 2
            with self.subTest(change=change), self.assertRaises(InputRejected):
                value.region()

    def test_rejected_region_records_actual_and_expected_contract_without_input(self):
        value = sender()
        value.lib.ei_region_get_width.return_value = 720
        with self.assertRaisesRegex(InputRejected, r"observed rect=\(0, 0, 720, 900\), scale=2.0; "
                                    r"expected logical rect=\(0, 0, 1440.0, 900.0\), scale=2.0"):
            value.send("pointer_motion_absolute", 1200, 900)
        value.lib.ei_device_pointer_motion_absolute.assert_not_called()

    def test_odd_physical_target_preserves_exact_half_logical_pixel(self):
        value = sender()
        value.send("pointer_motion_absolute", 1463, 1173)
        value.lib.ei_device_pointer_motion_absolute.assert_called_once_with(10, 731.5, 586.5)

    def test_unscaled_declared_coordinates_and_invalid_physical_target(self):
        value = sender()
        value.mapping = (1, 1.)
        value.lib.ei_region_get_physical_scale.return_value = 1.
        value.lib.ei_region_get_width.return_value = 2880
        value.lib.ei_region_get_height.return_value = 1800
        value.send("pointer_motion_absolute", 1200, 900)
        value.lib.ei_device_pointer_motion_absolute.assert_called_once_with(10, 1200., 900.)
        for point in ((-1, 0), (2880, 0), (0, 1800), (True, 0), (1200.5, 900)):
            value = sender()
            with self.subTest(point=point), self.assertRaises(InputRejected):
                value.send("pointer_motion_absolute", *point)
            value.lib.ei_device_pointer_motion_absolute.assert_not_called()

    def test_disconnect_removed_and_pause_fail_before_input(self):
        for event_type in (2, 4, 6, 7, 999):
            value = sender()
            value.lib.ei_get_event.side_effect = [99, None]
            value.lib.ei_event_get_type.return_value = event_type
            with self.subTest(event_type=event_type), self.assertRaises(InputRejected):
                value.send("pointer_motion_absolute", 1200, 900)
            value.lib.ei_event_unref.assert_called_once_with(99)
            value.lib.ei_device_pointer_motion_absolute.assert_not_called()

    def test_event_cap_and_duplicate_resumption_fail_closed(self):
        value = sender()
        value.lib.ei_get_event.return_value = 99
        value.lib.ei_event_get_type.return_value = 1
        with self.assertRaises(InputRejected):
            value.pump()
        self.assertEqual(value.lib.ei_event_unref.call_count, 256)
        value = sender()
        value.lib.ei_get_event.side_effect = [99, None]
        value.lib.ei_event_get_type.return_value = 8
        value.lib.ei_event_get_device.return_value = 10
        with self.assertRaises(InputRejected):
            value.pump()
        value.lib.ei_device_start_emulating.assert_not_called()

    def test_context_and_device_refs_close_once(self):
        value = sender()
        value.close()
        value.close()
        self.assertEqual(value.lib.ei_device_stop_emulating.call_count, 2)
        self.assertEqual(value.lib.ei_device_unref.call_count, 2)
        self.assertEqual(value.lib.ei_device_close.call_count, 2)
        value.lib.ei_unref.assert_called_once_with(1)

    def bridge(self):
        value = PortalEisInput.__new__(PortalEisInput)
        value.portal = mock.Mock(ready=True)
        value.mapping = (1, 2.)
        value.portal.motion_mapping.return_value = value.mapping
        value.root_size = mock.Mock(return_value=(2880, 1800))
        value.sender = mock.Mock(ready=True)
        return value

    def wheel_bridge(self):
        value = self.bridge()
        value.allow_scroll, value.nonce, value.sent = True, "owned", set()
        value.held_keys, value.held_buttons = set(), set()
        value._check_owner, value._check_focus = mock.Mock(), mock.Mock()
        value.client_size = mock.Mock(return_value=(1920, 1080))
        value.client_pointer = mock.Mock(return_value={"same_screen": True, "client": [960, 540]})
        value.x11, value.display = mock.Mock(), 1
        value.window, value.owner_pid, value.record = 10, 20, mock.Mock()
        return value

    def test_wheel_owned_step_is_recorded_once_without_button_or_notify(self):
        value = self.wheel_bridge()
        value.scroll("one", "owned", 1)
        value.sender.send.assert_called_once_with("scroll_discrete", 0, 120)
        value.portal.notify.assert_not_called()
        value.record.assert_called_once()
        self.assertEqual(value.record.call_args.args[0]["scroll_discrete"], [0, 120])
        with self.assertRaises(InputRejected):
            value.scroll("one", "owned", 1)
        value.sender.send.assert_called_once()

    def test_wheel_guard_failures_do_not_send_and_unknown_send_is_not_retried(self):
        for mode in ("disabled", "nonce", "held", "focus", "owner", "pointer", "monitor", "consent", "direction"):
            value = self.wheel_bridge()
            if mode == "disabled":
                value.allow_scroll = False
            elif mode == "held":
                value.held_buttons.add(1)
            elif mode in ("focus", "owner"):
                getattr(value, "_check_" + mode).side_effect = InputRejected("ownership changed")
            elif mode == "pointer":
                value.client_pointer.return_value["client"] = [-1, 10]
            elif mode == "monitor":
                value.portal.motion_mapping.return_value = (2, 2.)
            elif mode == "consent":
                value.portal.ready = False
            with self.subTest(mode=mode), self.assertRaises(InputRejected):
                value.scroll("one", "foreign" if mode == "nonce" else "owned", True if mode == "direction" else 1)
            value.sender.send.assert_not_called()
            value.portal.notify.assert_not_called()
        value = self.wheel_bridge()
        value.sender.send.side_effect = InputRejected("unknown send outcome")
        for _ in range(2):
            with self.assertRaises(InputRejected):
                value.scroll("one", "owned", 1)
        value.sender.send.assert_called_once()
        value.record.assert_not_called()

    def test_absolute_transport_does_not_read_or_infer_initial_cursor(self):
        value = self.bridge()
        value.client_pointer = mock.Mock(side_effect=AssertionError("stale Xwayland cursor"))
        value._motion(1200, 900)
        value.sender.send.assert_called_once_with("pointer_motion_absolute", 1200, 900)
        value.client_pointer.assert_not_called()
        value.portal.notify.assert_not_called()

    def test_monitor_change_revocation_bounds_and_wheel_reject_without_send(self):
        for change in ("monitor", "consent", "sender", "bounds", "wheel"):
            value = self.bridge()
            if change == "monitor":
                value.portal.motion_mapping.return_value = (2, 2.)
            elif change == "consent":
                value.portal.ready = False
            elif change == "sender":
                value.sender.ready = False
            with self.subTest(change=change), self.assertRaises(InputRejected):
                if change == "wheel":
                    value._button(4, True)
                else:
                    value._motion(-1 if change == "bounds" else 1200, 900)
            value.sender.send.assert_not_called()
            value.portal.notify.assert_not_called()

    def test_keys_and_buttons_use_eis_only(self):
        value = self.bridge()
        value._key(56, True)
        value._button(1, False)
        self.assertEqual(value.sender.send.call_args_list,
                         [mock.call("keyboard_key", 48, True), mock.call("button_button", 0x110, False)])
        value.portal.notify.assert_not_called()

    def test_cleanup_releases_only_recorded_holds_without_region_lookup(self):
        value = sender()
        value.held_keys, value.held_buttons = {(20, 48)}, {(10, 0x110)}
        value.region = mock.Mock(side_effect=InputRejected("monitor changed"))
        value.close()
        value.lib.ei_device_keyboard_key.assert_called_once_with(20, 48, False)
        value.lib.ei_device_button_button.assert_called_once_with(10, 0x110, False)
        value.region.assert_not_called()
        value.lib.ei_device_pointer_motion_absolute.assert_not_called()
        self.assertFalse(value.held_keys or value.held_buttons)

    def test_cleanup_does_not_inject_on_removed_or_paused_device(self):
        value = sender()
        value.held_keys, value.held_buttons = {(20, 48)}, {(10, 0x110)}
        value.active.clear()
        value.close()
        value.lib.ei_device_keyboard_key.assert_not_called()
        value.lib.ei_device_button_button.assert_not_called()
        self.assertEqual(value.lib.ei_device_close.call_count, 2)
        value.lib.ei_unref.assert_called_once_with(1)

    def test_real_pause_event_clears_active_before_release_only_cleanup(self):
        value = sender()
        value.held_keys, value.held_buttons = {(20, 48)}, {(10, 0x110)}
        value.lib.ei_get_event.side_effect = [99, None]
        value.lib.ei_event_get_type.return_value = 7
        value.lib.ei_event_get_device.return_value = 10
        with self.assertRaises(InputRejected):
            value.pump()
        self.assertNotIn(10, value.active)
        value.close()
        value.lib.ei_device_keyboard_key.assert_called_once_with(20, 48, False)
        value.lib.ei_device_button_button.assert_not_called()

    def test_bridge_cleanup_survives_monitor_change_and_revocation_without_notify(self):
        for change in ("monitor", "revoked"):
            value = self.bridge()
            value.closed, value.held_keys, value.held_buttons = False, {56}, {1}
            value.sender = sender()
            value.sender.held_keys, value.sender.held_buttons = {(20, 48)}, {(10, 0x110)}
            value.x11, value.display = mock.Mock(), 123
            if change == "monitor":
                value.portal.motion_mapping.side_effect = InputRejected("monitor changed")
            else:
                value.portal.ready = False
            value.close()
            value.sender.lib.ei_device_keyboard_key.assert_called_once_with(20, 48, False)
            value.sender.lib.ei_device_button_button.assert_called_once_with(10, 0x110, False)
            value.portal.notify.assert_not_called()
            value.portal.motion_mapping.assert_not_called()
            value.x11.XCloseDisplay.assert_called_once_with(123)
            self.assertTrue(value.closed)


if __name__ == "__main__":
    unittest.main()
