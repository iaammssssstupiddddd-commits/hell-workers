"""Consent-gated RemoteDesktop transport for the owned X11 acceptance client.

Uses Notify methods and optional consent-backed, single-use restore tokens.
https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html
"""
from __future__ import annotations

import secrets
import math
import time
import os
import stat
import json
import fcntl
from pathlib import Path

from scripts.native_ui_input import InputRejected, X11Input

DEST = "org.freedesktop.portal.Desktop"
ROOT = "/org/freedesktop/portal/desktop"
REMOTE = "org.freedesktop.portal.RemoteDesktop"


class ConsentStore:
    """Owner-only credential journal; consume before submission, rotate on success.

    Hold the inode lock for the entire session. A crash/unknown response never
    reuses the old single-use token. This file is not evidence of OS consent.
    """
    def __init__(self, path):
        path = Path(path)
        parent = path.parent.stat()
        if not path.is_absolute() or path.parent.resolve() != path.parent or (
            parent.st_uid != os.geteuid() or stat.S_IMODE(parent.st_mode) != 0o700
        ):
            raise InputRejected("consent store requires an absolute path in an owner-only directory")
        self.fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
        try:
            info = os.fstat(self.fd)
            if (not stat.S_ISREG(info.st_mode) or info.st_uid != os.geteuid()
                    or stat.S_IMODE(info.st_mode) != 0o600 or info.st_nlink != 1):
                raise InputRejected("unsafe consent credential file")
            try:
                fcntl.flock(self.fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise InputRejected("consent credential is busy") from None
        except BaseException:
            self.close()
            raise

    def consume(self, bus_id):
        os.lseek(self.fd, 0, os.SEEK_SET)
        raw = os.read(self.fd, 16385)
        if len(raw) > 16384:
            raise InputRejected("consent credential exceeds bound")
        token = None
        if raw:
            try:
                data = json.loads(raw)
            except (ValueError, UnicodeError):
                raise InputRejected("invalid consent credential journal") from None
            if (not isinstance(data, dict) or set(data) != {"schema", "bus_id", "devices", "token"}
                    or data["schema"] != 1 or data["bus_id"] != bus_id or data["devices"] != 3
                    or (data["token"] is not None and
                        (not isinstance(data["token"], str) or not 0 < len(data["token"]) <= 4096))):
                raise InputRejected("consent credential context differs or is invalid")
            token = data["token"]
        self.save(bus_id, None)
        return token

    def save(self, bus_id, token):
        if token is not None and (not isinstance(token, str) or not 0 < len(token) <= 4096):
            raise InputRejected("portal returned an invalid restore token")
        raw = json.dumps({"schema": 1, "bus_id": bus_id, "devices": 3, "token": token}).encode()
        os.lseek(self.fd, 0, os.SEEK_SET)
        os.ftruncate(self.fd, 0)
        with os.fdopen(os.dup(self.fd), "wb", closefd=True) as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())

    def close(self):
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None


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
    def __init__(self, heartbeat, *, consent_file=None) -> None:
        from gi.repository import Gio, GLib

        self.Gio, self.GLib = Gio, GLib
        self.bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        self.heartbeat = heartbeat
        self.responses = {}
        self.session = None
        self.ready = False
        self.granted_devices = 0
        self.pending = None
        self.consent_file = consent_file
        self.consent_store = None
        self.persistence = "disabled"
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
        options = {"types": variant("u", 3), "persist_mode": variant("u", 0),
                   "handle_token": variant("s", token + "devices")}
        bus_id = None
        if self.consent_file:
            version = self.call(ROOT, "org.freedesktop.DBus.Properties", "Get",
                                "(ss)", (REMOTE, "version")).unpack()[0]
            if hasattr(version, "unpack"):
                version = version.unpack()
            if version < 2:
                raise InputRejected("persistent input consent requires RemoteDesktop version 2")
            bus_id = self.bus.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus",
                "org.freedesktop.DBus", "GetId", None, None,
                self.Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
            self.consent_store = ConsentStore(self.consent_file)
            restore = self.consent_store.consume(bus_id)
            options["persist_mode"] = variant("u", 2)
            if restore:
                options["restore_token"] = variant("s", restore)
        data = self.request("CreateSession", "(a{sv})", ({
            "handle_token": variant("s", token), "session_handle_token": variant("s", token)},))
        self.session = data["session_handle"]
        self.request("SelectDevices", "(oa{sv})", (self.session, options))
        data = self.request("Start", "(osa{sv})", (self.session, f"x11:{window:x}", {
            "handle_token": variant("s", token + "start")}), timeout=300)
        if data.get("devices", 0) & 3 != 3:
            raise InputRejected("portal did not grant both pointer and keyboard")
        self.granted_devices = data["devices"]
        if self.consent_store:
            restore = data.get("restore_token")
            self.consent_store.save(bus_id, restore)
            self.persistence = "saved" if restore else "not-granted"
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
        if self.consent_store:
            self.consent_store.close()
            self.consent_store = None


class PortalX11Input(X11Input):
    def __init__(self, *args, portal):
        self.portal = portal
        super().__init__(*args)

    def activate(self):
        self._check_owner()
        self.mapping = self.portal.motion_mapping(self.root_size())
        # Present the owned parent, but consent does not require X11 input focus.
        # The compositor may keep focus on a Wayland surface until consent closes.
        self._request_activation()
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
