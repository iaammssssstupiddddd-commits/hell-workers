"""Unapproved low-deck Bridge proposal; no gameplay or authority changes.

All dimensions require clay passage acceptance before planes/strokes packaging.
The coordinator executes this authoring source; import performs no work.
"""
from __future__ import annotations

import math
from building_clay_geometry import FIXTURES, Geometry, read_json, require

CONTRACT = FIXTURES / "building-bridge-v1.geometry.json"
KINDS = {"Bridge": "bridge"}


def contract(kind):
    value = read_json(CONTRACT)
    require(kind == "Bridge" and value["schema_version"] == 1
            and value["stage"] == "unapproved_bridge_production_proposal", "wrong Bridge fixture")
    require(all(value[key] is False for key in ("numeric_freeze", "art_approved", "runtime_published")),
            "fixture cannot grant approval")
    require(value["ordered_relative_tiles"] == [[x, y] for y in range(5) for x in range(2)]
            and value["center_offset_tiles"] == [0.5, 2] and value["anchor_basis"] == "RiverYMin"
            and value["geometry_scale"] == 32, "Bridge shape/units drift")
    return value, value[kind]


def parts(kind):
    contract(kind)
    return [{"name": "body", "mesh_role": "body", "material_role": "opaque_albedo",
             "translation_wu": [0, 0, 0], "rotation_xyzw": [0, 0, 0, 1], "scale": [1, 1, 1]}]


def geometry(kind):
    _, spec = contract(kind)
    body = Geometry()
    deck = spec["deck"]
    length = deck["length_wu"] / deck["planks"]
    for index in range(deck["planks"]):
        # Full width and continuous deck at both lane centers. Sparse face strokes
        # carry seams; no holes, fake ramps or actor elevation are introduced.
        body.box((0, deck["top_wu"] / 2, -deck["length_wu"]/2 + (index+0.5)*length),
                 (deck["width_wu"], deck["top_wu"], length), "wood")
    strap = spec["edge_strap"]
    for x in strap["centers_x_wu"]:
        height = strap["top_wu"] - deck["top_wu"]
        body.box((x, deck["top_wu"] + height/2, 0),
                 (strap["width_wu"], height, deck["length_wu"]), "iron")
    return {"body": body}


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
            edges = [[q[i]-p[i] for i in range(3)] for p, q in zip(points, points[1:] + points[:1])]
            u = max(edges, key=lambda edge: sum(n*n for n in edge))
            norm = math.sqrt(sum(n*n for n in u))
            u = [n / norm for n in u]
            v = [normal[1]*u[2]-normal[2]*u[1], normal[2]*u[0]-normal[0]*u[2], normal[0]*u[1]-normal[1]*u[0]]
            planar = [(sum((p[i]-a[i])*u[i] for i in range(3)),
                       sum((p[i]-a[i])*v[i] for i in range(3))) for p in points]
            low = [min(p[i] for p in planar) for i in range(2)]
            span = max(max(p[i] for p in planar)-low[i] for i in range(2))
            x, y = (index % (size//cell))*cell, (index//(size//cell))*cell
            require(y + cell <= size, "Bridge atlas overflow")
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
    draw = ImageDraw.Draw(image)
    for faces in face_layout(kind).values():
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
    image.save(output / "albedo.png")



def states(kind):
    contract(kind)
    return ["Complete"]
