"""M3 deterministic geometry and face atlas proposal; no approval or execution on import.

The coordinator must accept clay, planes, strokes and actual-window evidence.
Slot positions are world X/-Y mapped into glTF X/Z, using the logical shape order.
"""
from __future__ import annotations

import math

import building_infernal_style as infernal

from building_clay_geometry import FIXTURES, Geometry, read_json, require

CONTRACT = FIXTURES / "building-m3-v2.geometry.json"
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
        # Closed canvas walls and an opaque entry curtain shelter resting Souls.
        for wall in spec["walls"]:
            body.box(wall["center"], wall["size"], "cloth")
        # Each thick cloth strip joins its neighbours at a shared profile point.
        # The roof covers the interior; the canvas curtain marks the entrance.
        profile = spec["roof"]["x_height_profile"]
        near, far = spec["roof"]["z_edges"]
        front = spec["roof"]["front_z_profile"]
        require(len(front) == len(profile) and all(near < z <= far for z in front),
                "roof front profile must stay inside the unapproved envelope")
        thickness = spec["roof"]["thickness_wu"]
        for ((x0, y0), (x1, y1)), (z0, z1) in zip(
                zip(profile, profile[1:]), zip(front, front[1:]), strict=True):
            start = len(body.vertices)
            # Recess the centre of the entry edge, rather than covering the
            # entire opening with a uniform extrusion in top-down projection.
            # Ground envelope, support locations and player contracts are fixed.
            body.vertices.extend([(x0, y0, near), (x1, y1, near), (x1, y1, z1), (x0, y0, z0),
                                  (x0, y0 + thickness, near), (x1, y1 + thickness, near),
                                  (x1, y1 + thickness, z1), (x0, y0 + thickness, z0)])
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


def paint(kind, output, stage, *, style="legacy"):
    from PIL import Image, ImageDraw
    require(stage in ("clay", "planes", "strokes"), "unknown paint stage")
    require(style in ("legacy", infernal.STYLE), "unknown paint style")
    value, spec = contract(kind)
    size, cell = value["atlas"]["size_px"], value["atlas"]["cell_px"]
    image = Image.new("RGBA", (size, size), (65, 59, 52, 255))
    emission = Image.new("RGBA", (size, size), (0, 0, 0, 255))
    draw, glow = ImageDraw.Draw(image), ImageDraw.Draw(emission)
    meshes = geometry(kind)
    for role, faces in face_layout(kind).items():
        for face, indices in zip(faces, meshes[role].faces, strict=True):
            index = face["cell"]
            x, y = (index % (size//cell))*cell, (index//(size//cell))*cell
            material = face["surface"].split(":")[0]
            base = (125, 125, 125) if stage == "clay" else spec["palette"][material]
            if style == infernal.STYLE and kind == "RestArea" and stage != "clay":
                base = {"cloth": infernal.CLOTH, "wood": infernal.WOOD,
                        "iron": infernal.IRON}[material]
            # No camera lighting or shadows baked into albedo.
            draw.rectangle((x, y, x+cell-1, y+cell-1), fill=(*base, 255))
            if stage == "strokes":
                mask = Image.new("L", (size, size))
                ImageDraw.Draw(mask).polygon(face["polygon_px"], fill=255)
                layer = image.copy()
                brush = ImageDraw.Draw(layer)
                dark = tuple(round(c*0.84) for c in base)
                if material == "cloth" and face["normal"][1] > 0.5:
                    # One continuous cloth design in world X/Z, not repeated
                    # timber-like marks in each UV island. The same seams and
                    # repair patch continue across the four sloping strips.
                    points = [meshes[role].vertices[i] for i in indices]
                    a, b, c = points[0], points[1], points[-1]
                    uv_a, uv_b, uv_c = face["polygon_px"][0], face["polygon_px"][1], face["polygon_px"][-1]
                    dx1, dz1 = b[0]-a[0], b[2]-a[2]
                    dx2, dz2 = c[0]-a[0], c[2]-a[2]
                    determinant = dx1*dz2-dx2*dz1
                    require(abs(determinant) > 0, "cloth top must have an X/Z projection")

                    def pixel(world_x, world_z):
                        dx, dz = world_x-a[0], world_z-a[2]
                        u = (dx*dz2-dz*dx2)/determinant
                        v = (dx1*dz-dz1*dx)/determinant
                        return tuple(uv_a[i]+u*(uv_b[i]-uv_a[i])+v*(uv_c[i]-uv_a[i]) for i in range(2))

                    patch = [(-25, -17), (-12, -18), (-11, -3), (-24, -4)]
                    brush.polygon([pixel(*p) for p in patch],
                                  fill=(*(infernal.CLOTH_PATCH if style == infernal.STYLE else
                                          tuple(min(255, c+24) for c in base)), 255))
                    seam = (*(infernal.SEAM if style == infernal.STYLE else
                              tuple(round(c*0.66) for c in base)), 255)
                    # Two continuous joins and one broad fold are drawn in
                    # world coordinates, so their marks cross UV islands
                    # without turning each cloth face into a timber panel.
                    for points in (
                        [(-31, -13), (-7, -12), (15, -14), (31, -13)],
                        [(-31, 10), (-8, 9), (14, 11), (31, 10)],
                    ):
                        brush.line([pixel(*p) for p in points], fill=seam, width=2)
                    brush.line([pixel(-27, 19), pixel(-8, 16), pixel(12, 18), pixel(27, 16)],
                               fill=(*dark, 255), width=2)
                    for world_x in range(-28, 30, 6):
                        brush.line([pixel(world_x, 8), pixel(world_x+2, 12)],
                                   fill=seam, width=2)
                elif material == "cloth" and style == infernal.STYLE:
                    # Opaque entry curtain and wall hems, aligned with each
                    # actual quad rather than the atlas grid.
                    a,b,_c,d = face["polygon_px"]
                    def cloth_point(u,v):
                        return tuple(a[i]+u*(b[i]-a[i])+v*(d[i]-a[i]) for i in range(2))
                    brush.line([cloth_point(.14,.1),cloth_point(.18,.5),cloth_point(.14,.9)],
                               fill=(*infernal.SEAM,255),width=1)
                    brush.polygon([cloth_point(u,v) for u,v in ((.5,.3),(.78,.28),(.8,.61),(.52,.64))],
                                  fill=(*infernal.CLOTH_PATCH,255))
                elif material == "wood":
                    for offset in (10, 21):
                        brush.line([(x+6, y+offset), (x+16, y+offset-1), (x+25, y+offset)],
                                   fill=(*dark, 255), width=1)
                elif material == "bone":
                    if face["surface"] == "bone:shaft":
                        # Shaft UVs are thin quads: fixed atlas-cell marks miss
                        # their polygons. Follow the actual longitudinal edges
                        # without changing geometry, UVs, palette or lighting.
                        q0, q1, q2, q3 = face["polygon_px"]
                        def grain_point(t, across):
                            left = [a + (b-a)*t for a, b in zip(q0, q3, strict=True)]
                            right = [a + (b-a)*t for a, b in zip(q1, q2, strict=True)]
                            return tuple(a + (b-a)*across for a, b in zip(left, right, strict=True))
                        brush.line([grain_point(0.12, 0.4), grain_point(0.48, 0.55),
                                    grain_point(0.88, 0.4)], fill=(*dark, 255), width=1)
                    else:
                        brush.line([(x+9, y+8), (x+12, y+15), (x+10, y+23)],
                                   fill=(*dark, 255), width=1)
                # Iron, inactive sigils and cloth edge thickness stay quiet.
                image.paste(layer, (0, 0), mask)
            if role == "slot" and face["normal"][1] > 0.5:
                # Only top-facing slot faces carry the purple sigil, never body.
                glow.polygon(face["polygon_px"], fill=(*spec["palette"]["emissive"], 255))
    image.save(output / "albedo.png")
    if kind == "SoulSpa":
        emission.save(output / "slot_emissive.png")


def states(kind):
    return ["Empty"] if kind == "RestArea" else ["Constructing"] + [f"Mask{i:02d}" for i in range(16)]
