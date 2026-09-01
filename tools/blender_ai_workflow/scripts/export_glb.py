"""Validate the current scene and export a GLB into the staging area."""

from __future__ import annotations

import argparse
import hashlib
import math
import sys
from datetime import UTC, datetime
from pathlib import Path

import bpy

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from validate_scene import inspect_scene
from workflow_common import (
    positive_int,
    script_arguments,
    staging_path,
    write_json_atomic,
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output", required=True, help="GLB output under staging/exports"
    )
    parser.add_argument(
        "--report", required=True, help="JSON report under staging/reports"
    )
    parser.add_argument("--require-uv", action="store_true")
    parser.add_argument("--require-material", action="store_true")
    parser.add_argument("--max-triangles", type=positive_int)
    parser.add_argument("--collection")
    parser.add_argument("--require-single-mesh", action="store_true")
    parser.add_argument("--geometry-scale", type=float, default=1.0)
    parser.add_argument(
        "--materials-mode",
        choices=("export", "placeholder", "none"),
        default="export",
    )
    return parser


def main() -> None:
    args = build_parser().parse_args(script_arguments())
    if not math.isfinite(args.geometry_scale) or args.geometry_scale <= 0.0:
        raise ValueError("geometry scale must be finite and positive")
    output_path = staging_path(args.output, "exports")
    report_path = staging_path(args.report, "reports")
    if output_path.suffix.lower() != ".glb":
        raise ValueError(f"staging export must use .glb: {output_path}")

    validation = inspect_scene(
        require_uv=args.require_uv,
        require_material=args.require_material,
        max_triangles=args.max_triangles,
        collection_name=args.collection,
        require_single_mesh=args.require_single_mesh,
    )
    if validation["summary"]["errors"]:
        write_json_atomic(
            report_path,
            {
                "schema_version": 1,
                "status": "validation_failed",
                "validation": validation,
            },
        )
        raise RuntimeError("scene validation failed before export")

    use_selection = args.collection is not None
    if use_selection:
        collection = bpy.data.collections.get(args.collection)
        if collection is None:
            raise RuntimeError("validated collection disappeared before export")
        bpy.ops.object.select_all(action="DESELECT")
        selected = list(collection.all_objects)
        for obj in selected:
            obj.select_set(True)
        if selected:
            bpy.context.view_layer.objects.active = selected[0]

    scaled_objects = (
        selected
        if use_selection
        else [obj for obj in bpy.context.scene.objects if obj.type == "MESH"]
    )
    if not math.isclose(args.geometry_scale, 1.0, abs_tol=1.0e-12):
        for obj in scaled_objects:
            obj.data = obj.data.copy()
            for vertex in obj.data.vertices:
                vertex.co *= args.geometry_scale

    result = bpy.ops.export_scene.gltf(
        filepath=str(output_path),
        export_format="GLB",
        export_yup=True,
        export_texcoords=True,
        export_normals=True,
        export_materials=args.materials_mode.upper(),
        export_animations=True,
        use_renderable=True,
        use_selection=use_selection,
    )
    if "FINISHED" not in result:
        raise RuntimeError(f"Blender glTF exporter did not finish: {result}")

    digest = hashlib.sha256(output_path.read_bytes()).hexdigest()
    report = {
        "schema_version": 1,
        "status": "exported",
        "created_at_utc": datetime.now(UTC).isoformat(),
        "blender_version": bpy.app.version_string,
        "source_blend": bpy.data.filepath or None,
        "output": str(output_path),
        "bytes": output_path.stat().st_size,
        "sha256": digest,
        "geometry_scale": args.geometry_scale,
        "materials_mode": args.materials_mode,
        "validation": validation,
    }
    if args.collection is not None:
        report["collection"] = args.collection
    write_json_atomic(report_path, report)
    print(f"GLB_EXPORTED output={output_path} sha256={digest} report={report_path}")


if __name__ == "__main__":
    main()
