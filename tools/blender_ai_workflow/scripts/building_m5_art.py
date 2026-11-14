"""Deterministic fixed-canvas M5 raster proposals; never art/release authority.

Broad vector-sketch shapes are authored directly at the world-role resolution.
No existing product image is read, cropped, resized or overwritten.
"""
from __future__ import annotations

import argparse
import io
import math
from pathlib import Path

import building_infernal_style as infernal
import building_asset_pipeline as pipeline
from build_building_m5_candidate import KINDS, contract, originals, staged
from workflow_common import asset_root


def render(kind: str, *, on: bool = False, style: str = "legacy") -> bytes:
    from PIL import Image, ImageDraw
    value, spec = contract(kind)
    pipeline.require(not on or kind == "OutdoorLamp", "only Lamp has an on state")
    pipeline.require(style == "legacy" or (style == infernal.STYLE and kind == "OutdoorLamp"), "unsupported paint style")
    image = Image.new("RGBA", tuple(value["canvas_px"]))
    draw = ImageDraw.Draw(image)
    scale = value["canvas_px"][0] / spec["canvas_wu"][0]
    ax, ay = value["anchor_px"]

    def point(x, z, height=0):
        return (round(ax+x*scale), round(ay+(z-0.6*height)*scale))

    def plane(vertices, color):
        draw.polygon([point(*p) for p in vertices], fill=(*color, 255),
                     outline=(*infernal.INK, 255) if style == infernal.STYLE else None)

    def line(vertices, color, width=1):
        draw.line([point(*p) for p in vertices], fill=(*color, 255), width=width)

    if kind == "WheelbarrowParking":
        # Empty dark timber rack with two ground wheel ruts; no vehicle painted.
        for x in (-13, 13):
            plane([(x-1.4, -13, 0), (x+1.4, -13, 0), (x+1.4, 13, 0), (x-1.4, 13, 0)], (76, 62, 49))
            # Sparse worn timber edges remain distinct from dark Dirt; keep
            # the original dark faces, stroke geometry and alpha unchanged.
            line([(x, -10, 0), (x-0.3, 9, 0)], (169, 139, 99), 2)
        plane([(-15, -13, 0), (15, -13, 0), (15, -10, 0), (-15, -10, 0)], (101, 78, 56))
        for x in (-7, 7):
            line([(x, -6, 0), (x-0.5, 2, 0), (x, 12, 0)], (174, 146, 106), 4)
            plane([(x-1, -13, 0), (x+1, -13, 0), (x+1, -13, 6), (x-1, -13, 6)], (82, 68, 53))
        line([(-14, -13, 6), (14, -13, 6)], (145, 123, 88), 5)
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
        # Tripod feet sit on the original logical anchor, never on the lamp head.
        for x, z in ((-11, 7), (11, 7), (0, -10)):
            line([(0, 0, 17), (x, z, 0)], (infernal.IRON if style == infernal.STYLE else (54, 58, 57)), 7)
            line([(x-2, z, 0), (x+2, z, 0)], (infernal.WORN_EDGE if style == infernal.STYLE else (114, 106, 85)), 4)
        # Two telescoping steel sections and a conspicuous height-lock collar.
        plane([(-1.5, 0, 10), (1.5, 0, 10), (1.5, 0, 43), (-1.5, 0, 43)], ((98, 85, 89) if style == infernal.STYLE else (93, 94, 85)))
        plane([(-1, 0, 40), (1, 0, 40), (1, 0, 67), (-1, 0, 67)], ((146, 129, 119) if style == infernal.STYLE else (156, 153, 130)))
        line([(-2.6, 0, 40), (2.6, 0, 40)], (infernal.RUST if style == infernal.STYLE else (160, 116, 58)), 6)
        line([(2, 0, 6), (3.2, 0, 29), (2.4, 0, 53), (3.5, 0, 67)], (infernal.INK if style == infernal.STYLE else (44, 47, 45)), 2)
        # U-bracket and a broad rectangular high-mounted work-light housing.
        line([(-8, 0, 69), (-8, 0, 63), (8, 0, 63), (8, 0, 69)], (infernal.RUST if style == infernal.STYLE else (81, 75, 65)), 5)
        plane([(-10, 0, 65), (10, 0, 65), (10, 0, 77), (-10, 0, 77)], (infernal.IRON if style == infernal.STYLE else (55, 58, 56)))
        plane([(-8, 0, 67), (8, 0, 67), (8, 0, 75), (-8, 0, 75)],
              (infernal.LAMP_GLASS_ON if style == infernal.STYLE else (246, 221, 155)) if on else (infernal.LAMP_GLASS_OFF if style == infernal.STYLE else (138, 143, 132)))
        line([(-8, 0, 75), (8, 0, 75)], (infernal.WORN_EDGE if style == infernal.STYLE else (173, 151, 108)), 2)
        if style == infernal.STYLE:
            silhouette = image.getchannel("A")
            # Worn pole and rusted housing; no new geometry, emission, or alpha.
            line([(-9, 0, 66), (-4, 0, 66), (-2, 0, 68)], infernal.RUST, 3)
            line([(0.4, 0, 44), (0.1, 0, 53), (0.5, 0, 59)], infernal.WORN_EDGE, 1)
            image.putalpha(silhouette)
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
        payloads = {role: render(kind, on=role == "world_on", style=infernal.STYLE if kind == "OutdoorLamp" else "legacy") for role in spec["world_roles"]}
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
        pipeline.require(all(payload == render(kind, on=role == "world_on", style=infernal.STYLE if kind == "OutdoorLamp" else "legacy")
                             for role, payload in payloads.items()), "candidate pixels or generator drift")
        inventory[kind] = {role: pipeline.record(kind+"/"+role+".png", payload)
                           for role, payload in payloads.items()}
    result = {"schema_version": 1, "scope": "unapproved-fixed-canvas-raster-proposal",
            "generator_sha256": pipeline.digest(Path(__file__).read_bytes()),
            "style_sha256": pipeline.digest(Path(infernal.__file__).read_bytes()),
            "lamp_style": infernal.STYLE,
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
