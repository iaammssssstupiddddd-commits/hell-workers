"""Bounded caller-owned X11 geometry readback. No input, resize or focus APIs."""
from __future__ import annotations

import argparse
import ctypes
import json
import re
import subprocess


def client_geometry(window: str, pid: int) -> dict:
    if not isinstance(window, str) or not re.fullmatch(r"0x[0-9a-fA-F]{1,16}", window) or int(window, 16) == 0:
        raise ValueError("invalid owned client ID")
    if type(pid) is not int or pid <= 0:
        raise ValueError("invalid owned client PID")
    owner = subprocess.run(["xprop", "-id", window, "_NET_WM_PID", "_NET_FRAME_EXTENTS", "_NET_WM_STATE"], check=False,
                           capture_output=True, text=True, timeout=2)
    match = re.search(r"^_NET_WM_PID\([^)]*\)\s*=\s*(\d+)\s*$", owner.stdout, re.MULTILINE)
    if owner.returncode != 0 or len(owner.stdout) > 8192 or not match or int(match[1]) != pid:
        raise ValueError("X11 geometry client owner changed")
    x11 = ctypes.CDLL("libX11.so.6")
    pointer, ulong, integer, uint = ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_uint
    x11.XOpenDisplay.argtypes, x11.XOpenDisplay.restype = [ctypes.c_char_p], pointer
    x11.XCloseDisplay.argtypes, x11.XCloseDisplay.restype = [pointer], integer
    x11.XGetGeometry.argtypes = [pointer, ulong, ctypes.POINTER(ulong), ctypes.POINTER(integer),
                                ctypes.POINTER(integer), ctypes.POINTER(uint), ctypes.POINTER(uint),
                                ctypes.POINTER(uint), ctypes.POINTER(uint)]
    x11.XGetGeometry.restype = integer
    display = x11.XOpenDisplay(None)
    if not display:
        raise ValueError("X11 geometry display unavailable")
    try:
        root, x, y = ulong(), integer(), integer()
        width, height, border, depth = uint(), uint(), uint(), uint()
        if not x11.XGetGeometry(display, int(window, 16), ctypes.byref(root), ctypes.byref(x), ctypes.byref(y),
                ctypes.byref(width), ctypes.byref(height), ctypes.byref(border), ctypes.byref(depth)):
            raise ValueError("owned X11 geometry unavailable")
        result = {"window_id": window, "pid": pid, "physical": [width.value, height.value], "depth": depth.value, "wm_properties": owner.stdout}
        validate(result, window, pid)
        return result
    finally:
        x11.XCloseDisplay(display)


def validate(value, window, pid):
    if (not isinstance(value, dict) or set(value) != {"window_id", "pid", "physical", "depth", "wm_properties"}
            or value["window_id"] != window or type(value["pid"]) is not int or value["pid"] != pid
            or not isinstance(value["wm_properties"], str) or len(value["wm_properties"]) > 8192
            or type(value["depth"]) is not int or not 1 <= value["depth"] <= 64
            or not isinstance(value["physical"], list) or len(value["physical"]) != 2
            or not all(type(n) is int and 0 < n <= 16384 for n in value["physical"])):
        raise ValueError("invalid owned X11 geometry readback")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--window", required=True)
    parser.add_argument("--pid", type=int, required=True)
    args = parser.parse_args()
    print(json.dumps(client_geometry(args.window, args.pid), sort_keys=True))


if __name__ == "__main__":
    main()
