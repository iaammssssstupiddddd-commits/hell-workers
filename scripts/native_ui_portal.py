"""Consent-gated RemoteDesktop transport for the owned X11 acceptance client.

Uses the documented Notify methods, never ConnectToEIS or persistent permissions.
https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html
"""
from __future__ import annotations

import secrets
import math
import time

from scripts.native_ui_input import InputRejected, X11Input

DEST = "org.freedesktop.portal.Desktop"
ROOT = "/org/freedesktop/portal/desktop"
REMOTE = "org.freedesktop.portal.RemoteDesktop"


def physical_monitor_scale(state, root_size):
    """Fail closed outside the inspected Mutter single physical-monitor path."""
    serial, monitors, logical, properties = state
    if properties.get("layout-mode") != 1 or len(logical) != 1:
        raise InputRejected("portal motion requires one physical-layout Mutter monitor")
    x, y, scale, transform, primary, members, details = logical[0]
    if (x, y, transform) != (0, 0, 0) or len(members) != 1 or not math.isfinite(scale) or scale <= 0:
        raise InputRejected("unsupported monitor origin, rotation, mirror or scale")
    modes = [mode for spec, modes, props in monitors if tuple(spec) == tuple(members[0])
             for mode in modes if mode[-1].get("is-current")]
    if len(modes) != 1 or tuple(modes[0][1:3]) != tuple(root_size):
        raise InputRejected("X11 root and Mutter physical monitor extents differ")
    return serial, float(scale)


class PortalSession:
    def __init__(self, heartbeat) -> None:
        from gi.repository import Gio, GLib

        self.Gio, self.GLib = Gio, GLib
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.heartbeat = heartbeat
        self.responses = {}
        self.session = None
        self.ready = False
        self.granted_devices = 0
        self.pending = None
        self.subscriptions = [self.bus.signal_subscribe(
            DEST, "org.freedesktop.portal.Request", "Response", None, None,
            Gio.DBusSignalFlags.NONE, self._response), self.bus.signal_subscribe(
            DEST, "org.freedesktop.portal.Session", "Closed", None, None,
            Gio.DBusSignalFlags.NONE, self._closed)]

    def _response(self, connection, sender, path, interface, signal, parameters):
        if path == self.pending:
            self.responses[path] = parameters.unpack()

    def _closed(self, connection, sender, path, interface, signal, parameters):
        if path == self.session:
            self.ready = False

    def pump(self):
        context = self.GLib.MainContext.default()
        while context.pending():
            context.iteration(False)

    def call(self, path, interface, method, signature=None, values=()):
        return self.bus.call_sync(DEST, path, interface, method,
            self.GLib.Variant(signature, values) if signature else None,
            None, self.Gio.DBusCallFlags.NONE, 5000, None)

    def request(self, method, signature, values, timeout=10):
        # Subscribe before calling, and dispatch only after receiving the handle.
        self.pending = self.call(ROOT, REMOTE, method, signature, values).unpack()[0]
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.pump()
            self.heartbeat(method)
            if self.pending in self.responses:
                code, data = self.responses.pop(self.pending)
                self.pending = None
                if code != 0:
                    raise InputRejected(f"portal {method} rejected: response {code}")
                return data
            time.sleep(0.05)
        raise InputRejected(f"portal {method} timed out; no input sent")

    def start(self, window):
        if self.ready:
            return
        if self.session:
            raise InputRejected("portal session ended; do not silently request consent again")
        token = "hwui" + secrets.token_hex(8)
        variant = self.GLib.Variant
        data = self.request("CreateSession", "(a{sv})", ({
            "handle_token": variant("s", token), "session_handle_token": variant("s", token)},))
        self.session = data["session_handle"]
        self.request("SelectDevices", "(oa{sv})", (self.session, {
            "types": variant("u", 3), "persist_mode": variant("u", 0),
            "handle_token": variant("s", token + "devices")}))
        data = self.request("Start", "(osa{sv})", (self.session, f"x11:{window:x}", {
            "handle_token": variant("s", token + "start")}), timeout=300)
        if data.get("devices", 0) & 3 != 3:
            raise InputRejected("portal did not grant both pointer and keyboard")
        self.granted_devices = data["devices"]
        self.ready = True

    def notify(self, method, suffix, *values):
        self.pump()
        if not self.ready:
            raise InputRejected("portal session is not authorized or has ended")
        self.call(ROOT, REMOTE, method, f"(oa{{sv}}{suffix})", (self.session, {}, *values))

    def motion_mapping(self, root_size):
        state = self.bus.call_sync("org.gnome.Mutter.DisplayConfig",
            "/org/gnome/Mutter/DisplayConfig", "org.gnome.Mutter.DisplayConfig", "GetCurrentState",
            None, None, self.Gio.DBusCallFlags.NONE, 5000, None).unpack()
        return physical_monitor_scale(state, root_size)

    def close(self):
        self.ready = False
        try:
            if self.pending:
                self.call(self.pending, "org.freedesktop.portal.Request", "Close")
        except self.GLib.Error:
            pass  # The request may already have been dismissed by the desktop.
        try:
            if self.session:
                self.call(self.session, "org.freedesktop.portal.Session", "Close")
        except self.GLib.Error:
            pass  # Closing a pending request also destroys its session.
        for subscription in self.subscriptions:
            self.bus.signal_unsubscribe(subscription)
        self.subscriptions.clear()


class PortalX11Input(X11Input):
    def __init__(self, *args, portal):
        self.portal = portal
        super().__init__(*args)

    def activate(self):
        self._check_owner()
        self.mapping = self.portal.motion_mapping(self.root_size())
        self.portal.start(self.window)
        # The consent dialog owned focus; acquire the game once after it closes.
        super().activate()

    def _motion(self, x, y):
        if self.portal.motion_mapping(self.root_size()) != self.mapping:
            raise InputRejected("monitor configuration changed during input")
        observed = self.client_pointer()
        if not observed["same_screen"]:
            raise InputRejected("pointer is not on the owned X11 screen")
        # One relative request, then the existing game observer must ACK the exact
        # client point. No correction/retry and no click after a mismatch.
        # Mutter 50.4 meta_seat_impl_filter_relative_motion multiplies virtual
        # relative deltas by monitor scale in physical layout. Convert once using
        # the current, validated monitor contract, not an empirical correction.
        scale = self.mapping[1]
        self.portal.notify("NotifyPointerMotion", "dd",
                           (x - observed["root"][0]) / scale, (y - observed["root"][1]) / scale)

    def _button(self, button, pressed):
        if button in (4, 5):
            if not pressed:
                self.portal.notify("NotifyPointerAxisDiscrete", "ui", 0, -1 if button == 4 else 1)
        else:
            self.portal.notify("NotifyPointerButton", "iu", {1: 0x110, 2: 0x112, 3: 0x111}[button], int(pressed))

    def _key(self, code, pressed):
        # X11 keycodes are evdev keycodes + 8 on this Xwayland path.
        if code < 8:
            raise InputRejected("invalid Xwayland keycode")
        self.portal.notify("NotifyKeyboardKeycode", "iu", code - 8, int(pressed))

    def close(self):
        if self.closed:
            return
        self.portal.pump()
        self.held_buttons.difference_update((4, 5))
        if not self.portal.ready:
            # A revoked session cannot hold a virtual key/device any longer.
            self.held_buttons.clear()
            self.held_keys.clear()
        super().close()
