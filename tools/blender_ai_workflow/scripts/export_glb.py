"""Validate the current scene and export a GLB into the staging area."""

from __future__ import annotations

import argparse
import hashlib
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
    parser.add_argument("--output", required=True, help="GLB output under staging/exports")
    parser.add_argument("--report", required=True, help="JSON report under staging/reports")
    parser.add_argument("--require-uv", action="store_true")
    parser.add_argument("--require-material", action="store_true")
    parser.add_argument("--max-triangles", type=positive_int)
    return parser


def main() -> None:
    args = build_parser().parse_args(script_arguments())
    output_path = staging_path(args.output, "exports")
    report_path = staging_path(args.report, "reports")
    if output_path.suffix.lower() != ".glb":
        raise ValueError(f"staging export must use .glb: {output_path}")

    validation = inspect_scene(
        require_uv=args.require_uv,
        require_material=args.require_material,
        max_triangles=args.max_triangles,
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

    result = bpy.ops.export_scene.gltf(
        filepath=str(output_path),
        export_format="GLB",
        export_yup=True,
        export_texcoords=True,
        export_normals=True,
        export_materials="EXPORT",
        export_animations=True,
        use_renderable=True,
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
        "validation": validation,
    }
    write_json_atomic(report_path, report)
    print(f"GLB_EXPORTED output={output_path} sha256={digest} report={report_path}")


if __name__ == "__main__":
    main()
