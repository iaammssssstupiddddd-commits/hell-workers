"""Blender-only source packing/audit; never changes geometry or grants approval."""

from pathlib import Path
import argparse
import hashlib
import json
import sys


def geometry(bpy):
    records = []
    for obj in sorted(bpy.data.objects, key=lambda x: x.name):
        if obj.type != "MESH":
            continue
        records.append(
            {
                "name": obj.name,
                "matrix": [list(row) for row in obj.matrix_world],
                "vertices": [list(v.co) for v in obj.data.vertices],
                "faces": [list(p.vertices) for p in obj.data.polygons],
                "uv": [
                    [list(loop.uv) for loop in layer.data]
                    for layer in obj.data.uv_layers
                ],
            }
        )
    return hashlib.sha256(json.dumps(records, sort_keys=True).encode()).hexdigest()


def main():
    import bpy

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("pack", "audit"))
    parser.add_argument("--base", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--config", type=Path, required=True)
    args = parser.parse_args(sys.argv[sys.argv.index("--") + 1 :])
    config = json.loads(args.config.read_text())
    bpy.ops.wm.open_mainfile(filepath=str(args.base))
    original = geometry(bpy)
    text_name = "HW_Tank_WorldPreview_Projection_v1"
    if args.action == "pack":
        if args.source.exists() or bpy.data.texts.get(text_name):
            raise ValueError("fresh illustrated source required")
        for name, record in config["packed_images"].items():
            if bpy.data.images.get(name):
                raise ValueError("packed illustration name already exists")
            image = bpy.data.images.load(
                str(args.config.parent / record["path"]), check_existing=False
            )
            image.name = name
            image.use_fake_user = True
            image.pack()
        text = bpy.data.texts.new(text_name)
        text.write(args.config.read_text())
        if geometry(bpy) != original:
            raise ValueError("packing changed geometry")
        bpy.ops.wm.save_as_mainfile(filepath=str(args.source), compress=True)
    else:
        bpy.ops.wm.open_mainfile(filepath=str(args.source))
    if geometry(bpy) != original:
        raise ValueError("illustration source geometry differs from immutable base")
    if bpy.data.texts[text_name].as_string() != args.config.read_text():
        raise ValueError("packed projection configuration differs")
    packed = {}
    for name, record in config["packed_images"].items():
        image = bpy.data.images[name]
        if image.packed_file is None:
            raise ValueError("illustration is not packed")
        sha = hashlib.sha256(bytes(image.packed_file.data)).hexdigest()
        if sha != record["sha256"]:
            raise ValueError("packed illustration bytes differ")
        packed[name] = sha
    print(
        "HW_TANK_SOURCE_AUDIT_JSON:"
        + json.dumps(
            {
                "geometry_sha256": original,
                "packed_images": packed,
                "source_only": True,
                "art_approved": False,
            },
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
