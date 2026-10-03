"""M3 deterministic geometry and face atlas proposal; no approval or execution on import.

The coordinator must accept clay, planes, strokes and actual-window evidence.
Slot positions are world X/-Y mapped into glTF X/Z, using the logical shape order.
"""
from __future__ import annotations

import math

from building_clay_geometry import FIXTURES, Geometry, read_json, require

CONTRACT = FIXTURES / "building-m3-v1.geometry.json"
KINDS = {"RestArea": "rest-area", "SoulSpa": "soul-spa"}


def contract(kind):
    value = read_json(CONTRACT)
    require(kind in KINDS and value["schema_version"] == 1
            and value["stage"] == "unapproved_m3_production_proposal", "wrong M3 fixture")
    require(all(value[key] is False for key in ("numeric_freeze", "art_approved", "runtime_published")),
            "fixture cannot grant approval")
    require(value["ordered_relative_tiles"] == [[0, 0], [1, 0], [0, -1], [1, -1]]
            and value["center_offset_tiles"] == [0.5, -0.5]
            and value["geometry_scale"] == 32, "logical shape/units drift")
    return value, value[kind]


def parts(kind):
    value, spec = contract(kind)
    result = [{"name": "body", "mesh_role": "body", "material_role": "opaque_albedo",
               "translation_wu": [0, 0, 0], "rotation_xyzw": [0, 0, 0, 1], "scale": [1, 1, 1]}]
    if kind == "SoulSpa":
        cx, cy = value["center_offset_tiles"]
        for index, (x, y) in enumerate(value["ordered_relative_tiles"]):
            result.append({"name": f"slot{index}", "mesh_role": "slot", "material_role": "spa_slot",
                           "translation_wu": [(x - cx) * 32, spec["slot_height_wu"], -(y - cy) * 32],
                           "rotation_xyzw": [0, 0, 0, 1], "scale": [1, 1, 1]})
    return result


def bone(geometry, a, b, radius):
    # Horizontal tapered octagonal bone with broad knuckles, no tiny geometry detail.
    dx, dz = b[0] - a[0], b[2] - a[2]
    length = math.hypot(dx, dz)
    start = len(geometry.vertices)
    for t, scale in ((0, 1), (0.18, 0.63), (0.82, 0.63), (1, 1)):
        for i in range(8):
            angle = math.tau * i / 8
            r = radius * scale
            geometry.vertices.append((a[0] + dx * t - dz / length * r * math.cos(angle),
                                      a[1] + (b[1] - a[1]) * t + r * math.sin(angle),
                                      a[2] + dz * t + dx / length * r * math.cos(angle)))
    geometry.face([start + i for i in range(8)], "bone:end")
    for row in range(3):
        for i in range(8):
            j = (i + 1) % 8
            geometry.face([start + row * 8 + j, start + row * 8 + i,
                           start + (row + 1) * 8 + i, start + (row + 1) * 8 + j], "bone:shaft")
    geometry.face([start + 24 + i for i in reversed(range(8))], "bone:end")


def geometry(kind):
    _, spec = contract(kind)
    body = Geometry()
    if kind == "RestArea":
        posts = spec["posts"]
        profile = spec["roof"]["x_height_profile"]
        for x in posts["x"]:
            left, right = next((a, b) for a, b in zip(profile, profile[1:]) if a[0] <= x <= b[0])
            height = left[1] + (right[1] - left[1]) * (x - left[0]) / (right[0] - left[0])
            # Slight embed meets the sloping cloth; supports never float below it.
            height += 0.5
            for z in posts["z"]:
                width, depth = posts["width_depth_wu"]
                body.box((x, height/2, z), (width, height, depth), "wood")
                body.box((x, height-3, z), (5, 3, 5), "iron")
        # Each thick cloth strip joins its neighbours at a shared profile point.
        # The front remains open between posts; the canopy has actual depth.
        profile = spec["roof"]["x_height_profile"]
        near, far = spec["roof"]["z_edges"]
        thickness = spec["roof"]["thickness_wu"]
        for (x0, y0), (x1, y1) in zip(profile, profile[1:]):
            start = len(body.vertices)
            body.vertices.extend([(x0, y0, near), (x1, y1, near), (x1, y1, far), (x0, y0, far),
                                  (x0, y0 + thickness, near), (x1, y1 + thickness, near),
                                  (x1, y1 + thickness, far), (x0, y0 + thickness, far)])
            for face in ((0, 1, 2, 3), (4, 7, 6, 5), (0, 4, 5, 1),
                         (1, 5, 6, 2), (2, 6, 7, 3), (3, 7, 4, 0)):
                body.face([start + i for i in face], "cloth:canopy")
        return {"body": body}
    for a, b in spec["bone_segments"]:
        bone(body, a, b, spec["bone_radius_wu"])
    slot = Geometry()
    radius, half = spec["slot_radius_wu"], spec["slot_half_thickness_wu"]
    slot.lathe([(0, -half), (radius, -half), (radius, half), (0, half)], 12, "slot")
    return {"body": body, "slot": slot}


def face_layout(kind):
    value, _ = contract(kind)
    atlas = value["atlas"]
    size, cell, gutter = atlas["size_px"], atlas["cell_px"], atlas["gutter_px"]
    result, index = {}, 0
    for role, mesh in geometry(kind).items():
        result[role] = []
        for face, surface in zip(mesh.faces, mesh.surfaces, strict=True):
            points = [mesh.vertices[i] for i in face]
            a, b, c = points[:3]
            u = [b[i] - a[i] for i in range(3)]
            v = [c[i] - a[i] for i in range(3)]
            normal = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]]
            norm = math.sqrt(sum(n*n for n in normal))
            require(norm > 0, "degenerate face")
            normal = [n / norm for n in normal]
            norm = math.sqrt(sum(n*n for n in u))
            u = [n / norm for n in u]
            v = [normal[1]*u[2]-normal[2]*u[1], normal[2]*u[0]-normal[0]*u[2], normal[0]*u[1]-normal[1]*u[0]]
            planar = [(sum((p[i]-a[i])*u[i] for i in range(3)),
                       sum((p[i]-a[i])*v[i] for i in range(3))) for p in points]
            low = [min(p[i] for p in planar) for i in range(2)]
            span = max(max(p[i] for p in planar)-low[i] for i in range(2))
            x, y = (index % (size//cell))*cell, (index//(size//cell))*cell
            require(y + cell <= size, "M3 atlas overflow")
            pixels = [(x+gutter+(p[0]-low[0])*(cell-2*gutter-1)/span,
                       y+gutter+(p[1]-low[1])*(cell-2*gutter-1)/span) for p in planar]
            result[role].append({"cell": index, "surface": surface, "normal": normal,
                                 "polygon_px": pixels, "uv": [(px/size, 1-py/size) for px, py in pixels]})
            index += 1
    return result


def paint(kind, output, stage):
    from PIL import Image, ImageDraw
    require(stage in ("clay", "planes", "strokes"), "unknown paint stage")
    value, spec = contract(kind)
    size, cell = value["atlas"]["size_px"], value["atlas"]["cell_px"]
    image = Image.new("RGBA", (size, size), (65, 59, 52, 255))
    emission = Image.new("RGBA", (size, size), (0, 0, 0, 255))
    draw, glow = ImageDraw.Draw(image), ImageDraw.Draw(emission)
    for role, faces in face_layout(kind).items():
        for face in faces:
            index = face["cell"]
            x, y = (index % (size//cell))*cell, (index//(size//cell))*cell
            material = face["surface"].split(":")[0]
            base = (125, 125, 125) if stage == "clay" else spec["palette"][material]
            # No camera lighting or shadows baked into albedo.
            draw.rectangle((x, y, x+cell-1, y+cell-1), fill=(*base, 255))
            if stage == "strokes":
                mask = Image.new("L", (size, size))
                ImageDraw.Draw(mask).polygon(face["polygon_px"], fill=255)
                layer = image.copy()
                brush = ImageDraw.Draw(layer)
                dark = tuple(round(c*0.78) for c in base)
                # Sparse grain/seams, without black borders around every island.
                for offset in (10, 21):
                    brush.line([(x+6, y+offset), (x+16, y+offset-1), (x+25, y+offset)],
                               fill=(*dark, 255), width=1)
                image.paste(layer, (0, 0), mask)
            if role == "slot" and face["normal"][1] > 0.5:
                # Only top-facing slot faces carry the purple sigil, never body.
                glow.polygon(face["polygon_px"], fill=(*spec["palette"]["emissive"], 255))
    image.save(output / "albedo.png")
    if kind == "SoulSpa":
        emission.save(output / "slot_emissive.png")


def states(kind):
    return ["Empty"] if kind == "RestArea" else ["Constructing"] + [f"Mask{i:02d}" for i in range(16)]
