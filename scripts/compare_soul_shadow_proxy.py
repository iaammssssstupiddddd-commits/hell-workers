#!/usr/bin/env python3
"""Compare the original Soul GLB shadow proxy against blob proxy candidates.

This script reads `assets/models/characters/soul.glb`, reconstructs the bind-pose
skinned mesh, applies the original outer shadow proxy transform
(`SOUL_SHADOW_PROXY_PITCH_CORRECTION_DEGREES = -40`), then projects both the old
and candidate blob proxies into light space.

It prints projection metrics so we can tell whether a blob plane family can
approximate the original caster at all.
"""

from __future__ import annotations

import json
import math
import struct
from dataclasses import dataclass
from pathlib import Path


GLB_PATH = Path("assets/models/characters/soul.glb")
SUN_DIR = (0.18, 0.58, 0.79)
SOUL_GLB_SCALE = 0.8
BLOB_RADIUS = 0.28
BLOB_WOBBLE = 0.18
SEGMENTS = 48


@dataclass
class ProjectionStats:
    bbox_w: float
    bbox_h: float
    bbox_aspect: float
    pca_aspect: float
    pca_angle_deg: float


def normalize(v):
    length = math.sqrt(sum(x * x for x in v))
    return tuple(x / length for x in v)


def cross(a, b):
    return (
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )


def dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def quat_to_mat(q):
    x, y, z, w = q
    xx, yy, zz = x * x, y * y, z * z
    xy, xz, yz = x * y, x * z, y * z
    wx, wy, wz = w * x, w * y, w * z
    return [
        [1 - 2 * (yy + zz), 2 * (xy - wz), 2 * (xz + wy), 0],
        [2 * (xy + wz), 1 - 2 * (xx + zz), 2 * (yz - wx), 0],
        [2 * (xz - wy), 2 * (yz + wx), 1 - 2 * (xx + yy), 0],
        [0, 0, 0, 1],
    ]


def mat_mul(a, b):
    return [[sum(a[i][k] * b[k][j] for k in range(4)) for j in range(4)] for i in range(4)]


def mat_vec(m, v):
    x, y, z = v
    return (
        m[0][0] * x + m[0][1] * y + m[0][2] * z + m[0][3],
        m[1][0] * x + m[1][1] * y + m[1][2] * z + m[1][3],
        m[2][0] * x + m[2][1] * y + m[2][2] * z + m[2][3],
    )


def rotation_x(deg):
    rad = math.radians(deg)
    c, s = math.cos(rad), math.sin(rad)
    return [
        [1, 0, 0, 0],
        [0, c, -s, 0],
        [0, s, c, 0],
        [0, 0, 0, 1],
    ]


def rotation_z(deg):
    rad = math.radians(deg)
    c, s = math.cos(rad), math.sin(rad)
    return [
        [c, -s, 0, 0],
        [s, c, 0, 0],
        [0, 0, 1, 0],
        [0, 0, 0, 1],
    ]


def trs(node):
    t = node.get("translation", [0, 0, 0])
    r = node.get("rotation", [0, 0, 0, 1])
    s = node.get("scale", [1, 1, 1])
    rot = quat_to_mat(r)
    out = [[rot[i][j] * ([s[0], s[1], s[2], 1][j]) for j in range(4)] for i in range(4)]
    out[0][3], out[1][3], out[2][3] = t
    out[3] = [0, 0, 0, 1]
    return out


def load_glb():
    data = GLB_PATH.read_bytes()
    magic, version, length = struct.unpack_from("<III", data, 0)
    assert magic == 0x46546C67
    assert version == 2
    offset = 12
    chunks = []
    while offset < length:
        chunk_len, chunk_type = struct.unpack_from("<II", data, offset)
        offset += 8
        chunks.append((chunk_type, data[offset : offset + chunk_len]))
        offset += chunk_len
    json_chunk = next(c for t, c in chunks if t == 0x4E4F534A)
    bin_chunk = next(c for t, c in chunks if t == 0x004E4942)
    return json.loads(json_chunk.decode("utf-8")), bin_chunk


def accessor(js, bin_chunk, index):
    acc = js["accessors"][index]
    view = js["bufferViews"][acc["bufferView"]]
    comp_size = {5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4}[acc["componentType"]]
    count_per = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[acc["type"]]
    fmt = {5120: "b", 5121: "B", 5122: "h", 5123: "H", 5125: "I", 5126: "f"}[acc["componentType"]]
    stride = view.get("byteStride", comp_size * count_per)
    offset = view.get("byteOffset", 0) + acc.get("byteOffset", 0)
    values = []
    for i in range(acc["count"]):
        pos = offset + i * stride
        values.append(struct.unpack_from("<" + fmt * count_per, bin_chunk, pos))
    return values


def extract_bind_pose_positions():
    js, bin_chunk = load_glb()

    world = [None] * len(js["nodes"])
    parents = {}
    for i, node in enumerate(js["nodes"]):
        for child in node.get("children", []):
            parents[child] = i
    roots = [i for i in range(len(js["nodes"])) if i not in parents]

    def visit(node_idx, parent_world=None):
        local = trs(js["nodes"][node_idx])
        world[node_idx] = mat_mul(parent_world, local) if parent_world else local
        for child in js["nodes"][node_idx].get("children", []):
            visit(child, world[node_idx])

    for root in roots:
        visit(root)

    skin = js["skins"][0]
    inverse_bind = []
    for m in accessor(js, bin_chunk, skin["inverseBindMatrices"]):
        cols = [m[i : i + 4] for i in range(0, 16, 4)]
        inverse_bind.append([[cols[j][i] for j in range(4)] for i in range(4)])

    joint_mats = [mat_mul(world[node], inv) for node, inv in zip(skin["joints"], inverse_bind)]

    positions = []
    for node_idx in (8, 9):
        node = js["nodes"][node_idx]
        prim = js["meshes"][node["mesh"]]["primitives"][0]
        pos = accessor(js, bin_chunk, prim["attributes"]["POSITION"])
        joints = accessor(js, bin_chunk, prim["attributes"]["JOINTS_0"])
        weights = accessor(js, bin_chunk, prim["attributes"]["WEIGHTS_0"])
        for p, j, w in zip(pos, joints, weights):
            out = [0.0, 0.0, 0.0]
            for joint_idx, weight in zip(j, w):
                if not weight:
                    continue
                tp = mat_vec(joint_mats[joint_idx], p)
                out = [out[k] + weight * tp[k] for k in range(3)]
            positions.append(tuple(coord * SOUL_GLB_SCALE for coord in out))
    return positions


def make_blob_xy_points():
    points = [(0.0, 0.0, 0.0)]
    for i in range(SEGMENTS):
        angle = (i / SEGMENTS) * math.tau
        wobble = 1.0 + BLOB_WOBBLE * (
            math.sin(angle * 3.0 + 1.3) * 0.50
            + math.sin(angle * 5.0 + 2.7) * 0.30
            + math.sin(angle * 7.0 + 0.8) * 0.20
        )
        radius = BLOB_RADIUS * wobble
        points.append((math.cos(angle) * radius, math.sin(angle) * radius, 0.0))
    return points


def project_to_light_space(points):
    sun_dir = normalize(tuple(-x for x in SUN_DIR))
    u = normalize(cross((0.0, 1.0, 0.0), sun_dir))
    v = cross(sun_dir, u)
    projected = [(dot(p, u), dot(p, v)) for p in points]
    xs = [p[0] for p in projected]
    ys = [p[1] for p in projected]
    mean_x = sum(xs) / len(xs)
    mean_y = sum(ys) / len(ys)
    sxx = sum((x - mean_x) ** 2 for x in xs) / len(xs)
    syy = sum((y - mean_y) ** 2 for y in ys) / len(ys)
    sxy = sum((x - mean_x) * (y - mean_y) for x, y in projected) / len(projected)
    tr = sxx + syy
    det = sxx * syy - sxy * sxy
    disc = max(tr * tr / 4.0 - det, 0.0)
    l1 = tr / 2.0 + math.sqrt(disc)
    l2 = tr / 2.0 - math.sqrt(disc)
    return ProjectionStats(
        bbox_w=max(xs) - min(xs),
        bbox_h=max(ys) - min(ys),
        bbox_aspect=(max(xs) - min(xs)) / max(max(ys) - min(ys), 1e-9),
        pca_aspect=math.sqrt(l1 / max(l2, 1e-9)),
        pca_angle_deg=math.degrees(0.5 * math.atan2(2.0 * sxy, sxx - syy)),
    )


def transform_points(points, *mats):
    out = list(points)
    for mat in mats:
        out = [mat_vec(mat, p) for p in out]
    return out


def print_stats(label, stats):
    print(
        f"{label:20s} "
        f"bbox=({stats.bbox_w:.4f}, {stats.bbox_h:.4f}) "
        f"bbox_aspect={stats.bbox_aspect:.4f} "
        f"pca_aspect={stats.pca_aspect:.4f} "
        f"pca_angle={stats.pca_angle_deg:.4f}"
    )


def main():
    old_points = transform_points(extract_bind_pose_positions(), rotation_x(-40.0))
    blob_points = make_blob_xy_points()
    current_blob = transform_points(blob_points, rotation_z(-70.0), rotation_x(-70.0))

    old_stats = project_to_light_space(old_points)
    blob_stats = project_to_light_space(current_blob)

    print_stats("old_glb_shadow", old_stats)
    print_stats("current_blob_xy", blob_stats)

    print("\nDelta")
    print(f"bbox_w      {blob_stats.bbox_w - old_stats.bbox_w:+.4f}")
    print(f"bbox_h      {blob_stats.bbox_h - old_stats.bbox_h:+.4f}")
    print(f"bbox_aspect {blob_stats.bbox_aspect - old_stats.bbox_aspect:+.4f}")
    print(f"pca_aspect  {blob_stats.pca_aspect - old_stats.pca_aspect:+.4f}")
    print(f"pca_angle   {blob_stats.pca_angle_deg - old_stats.pca_angle_deg:+.4f}")

    print("\nInterpretation")
    print(
        "- If bbox/pca metrics stay far apart, a single blob plane is not a faithful replacement "
        "for the original GLB shadow proxy."
    )
    print(
        "- If the Soul body shading still disagrees with world shadows, that is a separate path "
        "from CharacterMaterial, not the blob caster."
    )


if __name__ == "__main__":
    main()
