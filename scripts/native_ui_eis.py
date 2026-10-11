"""Bounded libei sender on the existing consented RemoteDesktop session.

Only one unrotated physical-layout monitor is supported. Absolute coordinates
come from the declared EIS region, not an inferred Xwayland cursor position.
API: https://libinput.pages.freedesktop.org/libei/api/libei_8h_source.html
"""
from __future__ import annotations

import ctypes as c
import math
import time

from scripts.native_ui_input import InputRejected, X11Input
from scripts.native_ui_portal import DEST, ROOT, REMOTE

ABSOLUTE, KEYBOARD, BUTTON = 2, 4, 32


def bind_library():
    lib = c.CDLL("libei.so.1")
    pointer = c.c_void_p
    bindings = {
        "ei_new_sender": ([pointer], pointer),
        "ei_unref": ([pointer], pointer),
        "ei_configure_name": ([pointer, c.c_char_p], None),
        "ei_setup_backend_fd": ([pointer, c.c_int], c.c_int),
        "ei_dispatch": ([pointer], None),
        "ei_get_event": ([pointer], pointer),
        "ei_event_get_type": ([pointer], c.c_int),
        "ei_event_get_seat": ([pointer], pointer),
        "ei_event_get_device": ([pointer], pointer),
        "ei_event_unref": ([pointer], pointer),
        "ei_seat_has_capability": ([pointer, c.c_int], c.c_bool),
        "ei_seat_bind_capabilities": ([pointer], None),  # varargs, NULL sentinel
        "ei_device_ref": ([pointer], pointer),
        "ei_device_unref": ([pointer], pointer),
        "ei_device_close": ([pointer], None),
        "ei_device_has_capability": ([pointer, c.c_int], c.c_bool),
        "ei_device_get_type": ([pointer], c.c_int),
        "ei_device_get_region": ([pointer, c.c_size_t], pointer),
        "ei_region_get_x": ([pointer], c.c_uint32),
        "ei_region_get_y": ([pointer], c.c_uint32),
        "ei_region_get_width": ([pointer], c.c_uint32),
        "ei_region_get_height": ([pointer], c.c_uint32),
        "ei_region_get_physical_scale": ([pointer], c.c_double),
        "ei_device_start_emulating": ([pointer, c.c_uint32], None),
        "ei_device_stop_emulating": ([pointer], None),
        "ei_device_frame": ([pointer, c.c_uint64], None),
        "ei_now": ([pointer], c.c_uint64),
        "ei_device_pointer_motion_absolute": ([pointer, c.c_double, c.c_double], None),
        "ei_device_button_button": ([pointer, c.c_uint32, c.c_bool], None),
        "ei_device_keyboard_key": ([pointer, c.c_uint32, c.c_bool], None),
    }
    for name, (args, result) in bindings.items():
        getattr(lib, name).argtypes, getattr(lib, name).restype = args, result
    return lib


class EisSender:
    def __init__(self, portal, root_size, mapping):
        if not portal.ready or type(portal.granted_devices) is not int or portal.granted_devices & 3 != 3:
            raise InputRejected("EIS requires the existing pointer/keyboard consent")
        self.lib = bind_library()
        self.context = self.lib.ei_new_sender(None)
        self.devices, self.active, self.seats = {}, set(), set()
        self.held_keys, self.held_buttons = set(), set()
        self.root_size, self.mapping = tuple(root_size), mapping
        self.ready = False
        if not self.context:
            raise InputRejected("could not create libei sender")
        try:
            self.lib.ei_configure_name(self.context, b"hell-workers-owned-equipment-feedback")
            # Connect once. Portal Notify* is forbidden after this succeeds.
            response, descriptors = portal.bus.call_with_unix_fd_list_sync(
                DEST, ROOT, REMOTE, "ConnectToEIS",
                portal.GLib.Variant("(oa{sv})", (portal.session, {})), None,
                portal.Gio.DBusCallFlags.NONE, 5000, None, None)
            fd = descriptors.get(response.unpack()[0])
            # libei takes ownership, including closing it on context teardown.
            if self.lib.ei_setup_backend_fd(self.context, fd) != 0:
                raise InputRejected("could not initialize consented EIS connection")
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                portal.pump()
                portal.heartbeat("EIS device readiness")
                if not portal.ready:
                    raise InputRejected("portal ended during EIS readiness")
                self.pump()
                if self.selected(ABSOLUTE, required=False) and self.selected(KEYBOARD, required=False):
                    self.region()  # Reject unsupported mapping before any key.
                    self.ready = True
                    return
                time.sleep(.02)
            raise InputRejected("EIS absolute pointer/keyboard readiness timed out")
        except BaseException:
            self.close()
            raise

    def pump(self):
        self.lib.ei_dispatch(self.context)
        for _ in range(256):
            event = self.lib.ei_get_event(self.context)
            if not event:
                return
            try:
                kind = self.lib.ei_event_get_type(event)
                if kind in (2, 4, 6, 7):  # disconnect, removed seat/device, paused
                    if kind in (2, 4):
                        self.active.clear()
                    else:
                        self.active.discard(self.lib.ei_event_get_device(event))
                    raise InputRejected("EIS ownership/device ended or paused")
                if kind == 3:  # seat added; bind only the already consented devices
                    seat = self.lib.ei_event_get_seat(event)
                    if not seat or self.seats:
                        raise InputRejected("EIS requires exactly one seat")
                    self.seats.add(seat)
                    if not all(self.lib.ei_seat_has_capability(seat, cap)
                               for cap in (ABSOLUTE, KEYBOARD, BUTTON)):
                        raise InputRejected("EIS required capabilities are unavailable")
                    self.lib.ei_seat_bind_capabilities(seat, c.c_int(ABSOLUTE),
                        c.c_int(KEYBOARD), c.c_int(BUTTON), c.c_void_p())
                elif kind == 5:
                    device = self.lib.ei_event_get_device(event)
                    if not device or device in self.devices or len(self.devices) >= 4:
                        raise InputRejected("EIS device identity or cap differs")
                    caps = {cap for cap in (ABSOLUTE, KEYBOARD, BUTTON)
                            if self.lib.ei_device_has_capability(device, cap)}
                    self.devices[self.lib.ei_device_ref(device)] = caps
                elif kind == 8:
                    device = self.lib.ei_event_get_device(event)
                    if device not in self.devices or device in self.active:
                        raise InputRejected("EIS resumed unknown/repeated device")
                    self.lib.ei_device_start_emulating(device, 1)
                    self.active.add(device)
                elif kind not in (1, 9, 90, 91):
                    raise InputRejected("unsupported EIS sender event")
            finally:
                self.lib.ei_event_unref(event)
        raise InputRejected("EIS event queue exceeds bound")

    def selected(self, capability, *, required=True):
        found = [d for d in self.active if capability in self.devices[d]]
        if len(found) > 1 or (required and not found):
            raise InputRejected("EIS capability has no unique active device")
        return found[0] if found else None

    def region(self):
        device = self.selected(ABSOLUTE)
        lib = self.lib
        region = lib.ei_device_get_region(device, 0)
        if (lib.ei_device_get_type(device) != 1 or not region
                or lib.ei_device_get_region(device, 1) or BUTTON not in self.devices[device]):
            raise InputRejected("EIS requires one virtual absolute/button region")
        rect = tuple(getattr(lib, "ei_region_get_" + name)(region)
                     for name in ("x", "y", "width", "height"))
        scale = lib.ei_region_get_physical_scale(region)
        monitor_scale = self.mapping[1]
        if not math.isfinite(monitor_scale) or monitor_scale <= 0:
            raise InputRejected("invalid inspected monitor scale")
        # EIS absolute coordinates are logical pixels. Xwayland root/client
        # coordinates on this inspected path are physical pixels. Require
        # the declared logical extent to equal physical extent / scale;
        # never infer a ratio from an arbitrary/mismatched region.
        expected = (0, 0, *(extent / monitor_scale for extent in self.root_size))
        if rect != expected or scale != monitor_scale:
            raise InputRejected(f"EIS region does not match inspected physical monitor: "
                                f"observed rect={rect}, scale={scale}; "
                                f"expected logical rect={expected}, scale={monitor_scale}")
        return device

    def send(self, operation, *values):
        self.pump()
        device = self.selected(KEYBOARD) if operation == "keyboard_key" else self.region()
        if operation == "pointer_motion_absolute":
            if (len(values) != 2 or any(type(v) is not int for v in values)
                    or not all(0 <= v < extent for v, extent in zip(values, self.root_size, strict=True))):
                raise InputRejected("EIS absolute physical target is outside owned root")
            values = tuple(v / self.mapping[1] for v in values)
        if operation in ("keyboard_key", "button_button"):
            held = self.held_keys if operation == "keyboard_key" else self.held_buttons
            # Record the intended press before submission; cleanup never
            # releases an unowned key/button or retries a press.
            (held.add if values[1] else held.discard)((device, values[0]))
        getattr(self.lib, "ei_device_" + operation)(device, *values)
        self.lib.ei_device_frame(device, self.lib.ei_now(self.context))
        self.lib.ei_dispatch(self.context)

    def close(self):
        if self.context:
            try:
                # No monitor/region/focus re-resolution here. Release only
                # recorded holds on their original active device. If the
                # device has been removed/paused, do not inject into it;
                # close the device/connection instead, without reconnecting.
                for operation, held in (("keyboard_key", self.held_keys),
                                        ("button_button", self.held_buttons)):
                    for device, code in sorted(held):
                        if device in self.active:
                            getattr(self.lib, "ei_device_" + operation)(device, code, False)
                            self.lib.ei_device_frame(device, self.lib.ei_now(self.context))
                for device in self.active:
                    self.lib.ei_device_stop_emulating(device)
                for device in self.devices:
                    self.lib.ei_device_close(device)
                self.lib.ei_dispatch(self.context)
            finally:
                for device in self.devices:
                    self.lib.ei_device_unref(device)
                self.lib.ei_unref(self.context)
                self.context = None
                self.held_keys.clear()
                self.held_buttons.clear()
                self.ready = False
        self.ready = False


class PortalEisInput(X11Input):
    def __init__(self, *args, portal):
        self.portal, self.sender = portal, None
        super().__init__(*args)

    def activate(self):
        self._check_owner()
        self.mapping = self.portal.motion_mapping(self.root_size())
        self._request_activation()
        self.portal.start(self.window)
        super().activate()
        self.sender = EisSender(self.portal, self.root_size(), self.mapping)

    def _guard(self):
        self.portal.pump()
        if not self.portal.ready or not self.sender or not self.sender.ready:
            raise InputRejected("consented EIS sender is unavailable")
        if self.portal.motion_mapping(self.root_size()) != self.mapping:
            raise InputRejected("monitor configuration changed during EIS input")

    def _motion(self, x, y):
        self._guard()
        width, height = self.root_size()
        if not (0 <= x < width and 0 <= y < height):
            raise InputRejected("absolute target is outside declared monitor")
        self.sender.send("pointer_motion_absolute", x, y)

    def _button(self, button, pressed):
        self._guard()
        if button not in (1, 2, 3):
            raise InputRejected("EIS equipment feedback does not permit wheel input")
        self.sender.send("button_button", {1: 0x110, 2: 0x112, 3: 0x111}[button], pressed)

    def _key(self, code, pressed):
        self._guard()
        if code < 8:
            raise InputRejected("invalid Xwayland keycode")
        self.sender.send("keyboard_key", code - 8, pressed)

    def close(self):
        if self.closed:
            return
        try:
            if self.sender:
                self.sender.close()
        finally:
            # Sender owns release-only cleanup independently of the normal
            # guard. Base close now only tears down our X11 connection.
            self.held_keys.clear()
            self.held_buttons.clear()
            super().close()
