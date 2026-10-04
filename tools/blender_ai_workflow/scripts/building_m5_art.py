"""Deterministic fixed-canvas M5 raster proposals; never art/release authority.

Broad vector-sketch shapes are authored directly at the world-role resolution.
No existing product image is read, cropped, resized or overwritten.
"""
from __future__ import annotations

import argparse
import io
import math
from pathlib import Path

import building_asset_pipeline as pipeline
from build_building_m5_candidate import KINDS, contract, originals, staged
from workflow_common import asset_root


def render(kind: str, *, on: bool = False) -> bytes:
    from PIL import Image, ImageDraw
    value, spec = contract(kind)
    pipeline.require(not on or kind == "OutdoorLamp", "only Lamp has an on state")
    image = Image.new("RGBA", tuple(value["canvas_px"]))
    draw = ImageDraw.Draw(image)
    scale = value["canvas_px"][0] / spec["canvas_wu"][0]
    ax, ay = value["anchor_px"]

    def point(x, z, height=0):
        return (round(ax+x*scale), round(ay+(z-0.6*height)*scale))

    def plane(vertices, color):
        draw.polygon([point(*p) for p in vertices], fill=(*color, 255))

    def line(vertices, color, width=1):
        draw.line([point(*p) for p in vertices], fill=(*color, 255), width=width)

    if kind == "WheelbarrowParking":
        # Empty dark timber rack with two ground wheel ruts; no vehicle painted.
        for x in (-13, 13):
            plane([(x-1.4, -13, 0), (x+1.4, -13, 0), (x+1.4, 13, 0), (x-1.4, 13, 0)], (76, 62, 49))
            line([(x, -10, 0), (x-0.3, 9, 0)], (99, 80, 60), 2)
        plane([(-15, -13, 0), (15, -13, 0), (15, -10, 0), (-15, -10, 0)], (101, 78, 56))
        for x in (-7, 7):
            line([(x, -6, 0), (x-0.5, 2, 0), (x, 12, 0)], (112, 91, 66), 4)
            plane([(x-1, -13, 0), (x+1, -13, 0), (x+1, -13, 6), (x-1, -13, 6)], (82, 68, 53))
        line([(-14, -13, 6), (14, -13, 6)], (88, 71, 52), 5)
    elif kind == "SandPile":
        rim = [(-12, -4, 0), (-9, -9, 0), (0, -11, 0), (10, -6, 0),
               (12, 3, 0), (7, 9, 0), (-2, 10, 0), (-12, 4, 0)]
        peak = (-2, -1, 6)
        colors = [(163, 126, 72), (179, 141, 82), (194, 156, 92), (181, 137, 76),
                  (163, 121, 66), (155, 116, 66), (171, 129, 73), (183, 141, 82)]
        for a, b, color in zip(rim, rim[1:]+rim[:1], colors, strict=True):
            plane([a, b, peak], color)
        for points in ([(-7, 1, 3), (-3, 2, 3), (1, 1, 3)], [(2, 4, 2), (6, 5, 1)],
                       [(-5, -5, 3), (-2, -6, 4)]):
            line(points, (151, 116, 68), 2)
    elif kind == "BonePile":
        # Uneven large joints and visible gaps, rather than a solid white heap.
        bones = [((-9, -5), (8, 0), 2.2), ((-7, 5), (9, 3), 1.9),
                 ((-4, -8), (1, 8), 2.1), ((-10, 2), (6, -5), 1.7)]
        for index, (a, b, radius) in enumerate(bones):
            dx, dz = b[0]-a[0], b[1]-a[1]
            length = math.hypot(dx, dz)
            ux, uz = -dz/length, dx/length
            border = []
            for sign, rows in ((1, ((0, 1), (0.16, 0.55), (0.84, 0.6), (1, 1))),
                               (-1, ((1, 1), (0.84, 0.6), (0.16, 0.55), (0, 1)))):
                border.extend((a[0]+dx*t+ux*(radius*s*sign+0.5*math.sin(math.pi*t)),
                               a[1]+dz*t+uz*(radius*s*sign+0.5*math.sin(math.pi*t)),
                               0.6+index*0.45) for t, s in rows)
            color = (145+index*7, 135+index*6, 111+index*5)
            plane(border, color)
            # Each end has two round joint lobes with a shallow central cleft.
            # This changes shape, not color contrast, to distinguish bone from
            # the straight flat-ended boards used by the empty parking rack.
            for end in (a, b):
                for side in (-1, 1):
                    cx, cz = end[0]+ux*radius*0.52*side, end[1]+uz*radius*0.52*side
                    plane([(cx+radius*0.7*math.cos(i*math.tau/12),
                            cz+radius*0.7*math.sin(i*math.tau/12), 0.6+index*0.45)
                           for i in range(12)], color)
            line([(a[0]+dx*0.2, a[1]+dz*0.2, 0.6+index*0.45),
                  (a[0]+dx*0.75, a[1]+dz*0.75, 0.6+index*0.45)], (126, 119, 98), 2)
    else:
        # A separate bent bone carries an open iron cage. Earlier near/normal
        # captures showed a solid shade and a short pale leg, so the background
        # must remain visible between the heavy bars, not merely darkened.
        plane([(-3.2, 0, 0), (-3.8, 0, 2), (-2.8, 0, 5), (-3.4, 0, 7),
               (-1.2, 0, 9), (-1.1, 0, 12), (-2.1, 0, 14), (1.8, 0, 14),
               (2.4, 0, 11), (1.3, 0, 8), (2.6, 0, 6), (1.8, 0, 3),
               (3, 0, 1), (2.2, 0, 0)], (164, 153, 123))
        # Broad knuckles interrupt the shaft silhouette at its two bends.
        for cx, height, radius in ((-1.6, 8, 2.6), (0.7, 12, 2.1)):
            plane([(cx+radius*math.cos(i*math.tau/12), 0,
                    height+radius*0.62*math.sin(i*math.tau/12))
                   for i in range(12)], (190, 180, 145))
        line([(-2.6, 0, 3), (-2.4, 0, 5), (-1.3, 0, 7)], (119, 111, 93), 3)
        line([(0.9, 0, 10), (0.2, 0, 12.5)], (119, 111, 93), 3)
        # The small core changes state. Rims, outer bars and the central spine
        # are separate opaque strokes; the gaps are truly transparent.
        for left, right in ((-3.7, -1.7), (1.7, 3.7)):
            plane([(left, 0, 15.3), (right, 0, 15.3),
                   (right*0.88, 0, 22.7), (left*0.88, 0, 22.7)],
                  (207, 137, 192) if on else (75, 61, 76))
        for x0, x1 in ((-6.8, -5.2), (5.2, 6.8)):
            plane([(x0, 0, 14.3), (x1, 0, 14.3),
                   (x1*0.84, 0, 24), (x0*0.84, 0, 24)], (52, 57, 59))
        line([(0, 0, 14.2), (0, 0, 24)], (52, 57, 59), 8)
        line([(-6.7, 0, 14.3), (6.7, 0, 14.3)], (146, 115, 79), 7)
        line([(-5.7, 0, 23.8), (5.7, 0, 23.8)], (146, 115, 79), 7)
        plane([(-5.7, 0, 24), (0, 0, 25), (5.7, 0, 24)], (52, 57, 59))
        plane([(-4, -2, 0), (4, -2, 0), (3.5, 2, 0), (-3.7, 2, 0)], (124, 113, 94))
    output = io.BytesIO()
    image.save(output, format="PNG")
    return output.getvalue()


def path(name: str) -> Path:
    import re
    pipeline.require(re.fullmatch(r"[a-z0-9][a-z0-9-]{0,79}", name) is not None, "invalid candidate name")
    return staged(asset_root() / "staging/exports/building-m5-art" / name)


def build(name: str) -> dict:
    root = path(name)
    pipeline.require(not root.exists(), "use a fresh candidate name")
    for kind in KINDS:
        _, spec = contract(kind)
        payloads = {role: render(kind, on=role == "world_on") for role in spec["world_roles"]}
        originals(kind, payloads)
        for role, payload in payloads.items():
            pipeline.put(root / kind / (role+".png"), payload)
    report = verify(name)
    pipeline.put(root / "proposal.json", pipeline.canonical(report))
    return report


def verify(name: str) -> dict:
    root = path(name)
    inventory = {}
    for kind in KINDS:
        _, spec = contract(kind)
        payloads = {role: pipeline.rooted(root, kind+"/"+role+".png").read_bytes() for role in spec["world_roles"]}
        originals(kind, payloads)
        pipeline.require(all(payload == render(kind, on=role == "world_on")
                             for role, payload in payloads.items()), "candidate pixels or generator drift")
        inventory[kind] = {role: pipeline.record(kind+"/"+role+".png", payload)
                           for role, payload in payloads.items()}
    result = {"schema_version": 1, "scope": "unapproved-fixed-canvas-raster-proposal",
            "generator_sha256": pipeline.digest(Path(__file__).read_bytes()),
            "inventory": inventory, "art_approved": False, "runtime_published": False}
    report = root / "proposal.json"
    if report.exists():
        pipeline.require(pipeline.read(report) == result, "candidate origin report differs")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("build", "verify"))
    parser.add_argument("--name", required=True)
    args = parser.parse_args()
    import json
    print(json.dumps((build if args.action == "build" else verify)(args.name), indent=2))


if __name__ == "__main__":
    main()
