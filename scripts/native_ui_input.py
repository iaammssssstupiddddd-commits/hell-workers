"""Bounded X11/XTest input for a single owned acceptance client window."""
from __future__ import annotations

import ctypes
import re
import subprocess
import time
from pathlib import Path
from typing import Callable


class InputRejected(RuntimeError):
    pass


class ClientData(ctypes.Union):
    _fields_ = [("b", ctypes.c_char * 20), ("s", ctypes.c_short * 10), ("l", ctypes.c_long * 5)]


class ClientMessage(ctypes.Structure):
    _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int),
                ("display", ctypes.c_void_p), ("window", ctypes.c_ulong), ("message_type", ctypes.c_ulong),
                ("format", ctypes.c_int), ("data", ClientData)]


class PropertyEvent(ctypes.Structure):
    _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int),
                ("display", ctypes.c_void_p), ("window", ctypes.c_ulong), ("atom", ctypes.c_ulong),
                ("time", ctypes.c_ulong), ("state", ctypes.c_int)]


class XEvent(ctypes.Union):
    _fields_ = [("client", ClientMessage), ("property", PropertyEvent), ("pad", ctypes.c_long * 24)]


def process_descends_from(pid: int, ancestor: int) -> bool:
    seen: set[int] = set()
    while pid > 1 and pid not in seen:
        if pid == ancestor:
            return True
        seen.add(pid)
        try:
            stat = Path(f"/proc/{pid}/stat").read_text()
            pid = int(stat.rsplit(")", 1)[1].split()[1])
        except (OSError, ValueError, IndexError):
            return False
    return False


class X11Input:
    def __init__(self, window: int, owner_pid: int, root_pid: int, nonce: str,
                 record: Callable[[dict], None]) -> None:
        if not nonce or not process_descends_from(owner_pid, root_pid):
            raise InputRejected("input client does not belong to the launched process tree")
        self.window = window
        self.owner_pid = owner_pid
        self.root_pid = root_pid
        self.nonce = nonce
        self.record = record
        self.sent: set[str] = set()
        self.held_buttons: set[int] = set()
        self.held_keys: set[int] = set()
        self.closed = False
        self.x11 = ctypes.CDLL("libX11.so.6")
        self.xtest = ctypes.CDLL("libXtst.so.6")
        self._bind()
        self.display = self.x11.XOpenDisplay(None)
        if not self.display:
            raise InputRejected("could not open X11 display")
        event = ctypes.c_int()
        error = ctypes.c_int()
        major = ctypes.c_int()
        minor = ctypes.c_int()
        if not self.xtest.XTestQueryExtension(self.display, ctypes.byref(event), ctypes.byref(error),
                                             ctypes.byref(major), ctypes.byref(minor)):
            self.close()
            raise InputRejected("display does not support XTEST")
        try:
            self._check_owner()
        except Exception:
            self.close()
            raise

    def _bind(self) -> None:
        pointer = ctypes.c_void_p
        ulong = ctypes.c_ulong
        integer = ctypes.c_int
        signatures = {
            "XOpenDisplay": ([ctypes.c_char_p], pointer),
            "XCloseDisplay": ([pointer], integer),
            "XDefaultRootWindow": ([pointer], ulong),
            "XGetInputFocus": ([pointer, ctypes.POINTER(ulong), ctypes.POINTER(integer)], integer),
            "XSetInputFocus": ([pointer, ulong, integer, ulong], integer),
            "XRaiseWindow": ([pointer, ulong], integer),
            "XSync": ([pointer, integer], integer),
            "XInternAtom": ([pointer, ctypes.c_char_p, integer], ulong),
            "XSendEvent": ([pointer, ulong, integer, ctypes.c_long, ctypes.POINTER(XEvent)], integer),
            "XSelectInput": ([pointer, ulong, ctypes.c_long], integer),
            "XChangeProperty": ([pointer, ulong, ulong, ulong, integer, integer, ctypes.c_char_p, integer], integer),
            "XCheckTypedWindowEvent": ([pointer, ulong, integer, ctypes.POINTER(XEvent)], integer),
            "XGetGeometry": ([pointer, ulong, ctypes.POINTER(ulong), ctypes.POINTER(integer), ctypes.POINTER(integer),
                              *[ctypes.POINTER(ctypes.c_uint)] * 4], integer),
            "XQueryPointer": ([pointer, ulong, ctypes.POINTER(ulong), ctypes.POINTER(ulong),
                               *[ctypes.POINTER(integer)] * 4, ctypes.POINTER(ctypes.c_uint)], integer),
            "XStringToKeysym": ([ctypes.c_char_p], ulong),
            "XKeysymToKeycode": ([pointer, ulong], ctypes.c_ubyte),
            "XTranslateCoordinates": ([pointer, ulong, ulong, integer, integer,
                                       ctypes.POINTER(integer), ctypes.POINTER(integer), ctypes.POINTER(ulong)], integer),
        }
        for name, (args, result) in signatures.items():
            function = getattr(self.x11, name)
            function.argtypes, function.restype = args, result
        for name, args in {
            "XTestQueryExtension": [pointer, *[ctypes.POINTER(integer)] * 4],
            "XTestFakeMotionEvent": [pointer, integer, integer, integer, ulong],
            "XTestFakeButtonEvent": [pointer, ctypes.c_uint, integer, ulong],
            "XTestFakeKeyEvent": [pointer, ctypes.c_uint, integer, ulong],
        }.items():
            function = getattr(self.xtest, name)
            function.argtypes, function.restype = args, integer

    def _check_owner(self) -> None:
        if self.closed or not process_descends_from(self.owner_pid, self.root_pid):
            raise InputRejected("input owner exited or changed")
        result = subprocess.run(["xprop", "-id", hex(self.window), "_NET_WM_PID"],
                                capture_output=True, text=True, timeout=2, check=False)
        match = re.search(r"=\s*(\d+)\s*$", result.stdout)
        if result.returncode or not match or int(match[1]) != self.owner_pid:
            raise InputRejected("X11 client PID changed")

    def activate(self) -> None:
        """Acquire focus once at recipe start; never reacquire it after input begins."""
        if self.sent:
            raise InputRejected("cannot reacquire focus after input started")
        self._check_owner()
        # Request activation through the WM, then verify it. See EWMH section
        # _NET_ACTIVE_WINDOW: https://specifications.freedesktop.org/wm/latest-single/
        event = XEvent()
        event.client.type = 33  # ClientMessage
        event.client.send_event = 1
        event.client.display = self.display
        event.client.window = self.window
        event.client.message_type = self.x11.XInternAtom(self.display, b"_NET_ACTIVE_WINDOW", 0)
        event.client.format = 32
        event.client.data.l[0] = 2  # External window-control tool (pager source).
        event.client.data.l[1] = self._server_timestamp()
        root = self.x11.XDefaultRootWindow(self.display)
        if not self.x11.XSendEvent(self.display, root, 0, (1 << 20) | (1 << 19), ctypes.byref(event)):
            raise InputRejected("window manager activation request failed")
        self.x11.XRaiseWindow(self.display, self.window)
        self.x11.XSync(self.display, 0)
        deadline = time.monotonic() + 2
        while True:
            try:
                self._check_focus()
                break
            except InputRejected:
                if time.monotonic() >= deadline:
                    raise
                time.sleep(0.05)

    def _server_timestamp(self) -> int:
        """Read an X server timestamp using a property event on our own client."""
        atom = self.x11.XInternAtom(self.display, b"_HW_UI_ACCEPTANCE_TIME", 0)
        self.x11.XSelectInput(self.display, self.window, 1 << 22)  # PropertyChangeMask
        self.x11.XChangeProperty(self.display, self.window, atom, 31, 8, 0, b"1", 1)  # XA_STRING
        self.x11.XSync(self.display, 0)
        event = XEvent()
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            if self.x11.XCheckTypedWindowEvent(self.display, self.window, 28, ctypes.byref(event)):
                if event.property.atom == atom:
                    return int(event.property.time)
            time.sleep(0.01)
        raise InputRejected("X11 timestamp request timed out")

    def _check_focus(self) -> None:
        focus = ctypes.c_ulong()
        revert = ctypes.c_int()
        self.x11.XGetInputFocus(self.display, ctypes.byref(focus), ctypes.byref(revert))
        if focus.value != self.window:
            raise InputRejected(f"acceptance window lost keyboard focus: expected {self.window:#x}, observed {focus.value:#x}")

    def send(self, step: str, nonce: str, *, point: tuple[int, int] | None = None,
             button: int | None = None, pressed: bool | None = None,
             key: str | None = None) -> None:
        if nonce != self.nonce or not step or step in self.sent:
            raise InputRejected("stale nonce or repeated input step")
        if point is None and button is None and key is None:
            raise InputRejected("empty input step")
        if button is not None and (button not in (1, 2, 3, 4, 5) or pressed is None):
            raise InputRejected("invalid button input")
        if key is not None and (not re.fullmatch(r"[A-Za-z0-9_]+", key) or pressed is None):
            raise InputRejected("invalid keysym input")
        self._check_owner()
        self._check_focus()
        if button is not None and pressed:
            pointer = self.client_pointer()
            width, height = self.client_size()
            if (not pointer["same_screen"] or not (0 <= pointer["client"][0] < width
                                                    and 0 <= pointer["client"][1] < height)):
                raise InputRejected("pointer left the owned client before button press")
        if point is not None:
            width, height = self.client_size()
            if (len(point) != 2 or any(type(value) is not int for value in point)
                    or not (0 <= point[0] < width and 0 <= point[1] < height)):
                raise InputRejected("input point falls outside the owned client")
        self.sent.add(step)
        if point is not None:
            x, y = ctypes.c_int(), ctypes.c_int()
            child = ctypes.c_ulong()
            root = self.x11.XDefaultRootWindow(self.display)
            if not self.x11.XTranslateCoordinates(self.display, self.window, root, *point,
                                                  ctypes.byref(x), ctypes.byref(y), ctypes.byref(child)):
                raise InputRejected("could not resolve client input coordinates")
            self._motion(x.value, y.value)
        if button is not None:
            self._button(button, bool(pressed))
            (self.held_buttons.add if pressed else self.held_buttons.discard)(button)
        if key is not None:
            code = self.x11.XKeysymToKeycode(self.display, self.x11.XStringToKeysym(key.encode("ascii")))
            if not code:
                raise InputRejected("unknown X11 keysym")
            self._key(code, bool(pressed))
            (self.held_keys.add if pressed else self.held_keys.discard)(code)
        self.x11.XSync(self.display, 0)
        self.record({"step": step, "nonce": nonce, "window": self.window, "pid": self.owner_pid,
                     "point": point, "button": button, "pressed": pressed, "key": key,
                     "x11_pointer": self.client_pointer()})

    def _motion(self, x: int, y: int) -> None:
        if not self.xtest.XTestFakeMotionEvent(self.display, -1, x, y, 0):
            raise InputRejected("XTEST motion failed")

    def _button(self, button: int, pressed: bool) -> None:
        if not self.xtest.XTestFakeButtonEvent(self.display, button, int(pressed), 0):
            raise InputRejected("XTEST button failed")

    def _key(self, code: int, pressed: bool) -> None:
        if not self.xtest.XTestFakeKeyEvent(self.display, code, int(pressed), 0):
            raise InputRejected("XTEST key failed")

    def client_pointer(self) -> dict:
        root, child = ctypes.c_ulong(), ctypes.c_ulong()
        root_x, root_y, x, y = (ctypes.c_int() for _ in range(4))
        mask = ctypes.c_uint()
        same_screen = self.x11.XQueryPointer(self.display, self.window, ctypes.byref(root), ctypes.byref(child),
            ctypes.byref(root_x), ctypes.byref(root_y), ctypes.byref(x), ctypes.byref(y), ctypes.byref(mask))
        return {"same_screen": bool(same_screen), "client": [x.value, y.value], "mask": mask.value,
                "root": [root_x.value, root_y.value]}

    def client_size(self) -> tuple[int, int]:
        return self._size(self.window)

    def root_size(self) -> tuple[int, int]:
        return self._size(self.x11.XDefaultRootWindow(self.display))

    def _size(self, window: int) -> tuple[int, int]:
        root = ctypes.c_ulong()
        x, y = ctypes.c_int(), ctypes.c_int()
        width, height, border, depth = (ctypes.c_uint() for _ in range(4))
        if not self.x11.XGetGeometry(self.display, window, ctypes.byref(root), ctypes.byref(x), ctypes.byref(y),
                                    ctypes.byref(width), ctypes.byref(height), ctypes.byref(border), ctypes.byref(depth)):
            raise InputRejected("could not observe X11 client geometry")
        return width.value, height.value

    def close(self) -> None:
        if self.closed:
            return
        # Releases only: a failed recipe must never leave global input held.
        try:
            for button in self.held_buttons:
                self._button(button, False)
            for key in self.held_keys:
                self._key(key, False)
        finally:
            self.x11.XSync(self.display, 0)
            self.x11.XCloseDisplay(self.display)
            self.closed = True
