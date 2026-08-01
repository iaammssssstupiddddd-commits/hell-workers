"""Inspect the open Blender scene and emit a deterministic JSON quality report."""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path
from typing import Any

import bmesh
import bpy

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from workflow_common import (
    positive_int,
    script_arguments,
    staging_path,
    write_json_atomic,
)


def issue(
    severity: str,
    code: str,
    message: str,
    object_name: str | None = None,
) -> dict[str, str]:
    payload = {"severity": severity, "code": code, "message": message}
    if object_name:
        payload["object"] = object_name
    return payload


def finite_vector(values: Any) -> bool:
    return all(math.isfinite(float(value)) for value in values)


def inspect_mesh(
    obj: bpy.types.Object,
    depsgraph: bpy.types.Depsgraph,
    require_uv: bool,
    require_material: bool,
) -> tuple[dict[str, Any], list[dict[str, str]]]:
    findings: list[dict[str, str]] = []
    evaluated = obj.evaluated_get(depsgraph)
    evaluated_mesh = evaluated.to_mesh()
    try:
        evaluated_mesh.calc_loop_triangles()
        triangle_count = len(evaluated_mesh.loop_triangles)
        vertex_count = len(evaluated_mesh.vertices)
        polygon_count = len(evaluated_mesh.polygons)
        if triangle_count == 0:
            findings.append(
                issue(
                    "error",
                    "EMPTY_MESH",
                    "Evaluated mesh has no triangles.",
                    obj.name,
                )
            )

        if not finite_vector(obj.location) or not finite_vector(obj.rotation_euler):
            findings.append(issue("error", "NON_FINITE_TRANSFORM", "Transform contains NaN or infinity.", obj.name))
        if not finite_vector(obj.scale):
            findings.append(issue("error", "NON_FINITE_SCALE", "Scale contains NaN or infinity.", obj.name))
        elif any(abs(float(value)) < 1.0e-8 for value in obj.scale):
            findings.append(issue("error", "ZERO_SCALE", "Object has a zero scale axis.", obj.name))
        elif any(float(value) < 0 for value in obj.scale):
            findings.append(issue("error", "NEGATIVE_SCALE", "Apply or remove negative scale before export.", obj.name))
        elif any(not math.isclose(float(value), 1.0, abs_tol=1.0e-5) for value in obj.scale):
            findings.append(issue("warning", "UNAPPLIED_SCALE", "Object scale is not [1, 1, 1].", obj.name))

        non_finite_vertices = sum(
            1 for vertex in evaluated_mesh.vertices if not finite_vector(vertex.co)
        )
        if non_finite_vertices:
            findings.append(
                issue(
                    "error",
                    "NON_FINITE_VERTEX",
                    f"{non_finite_vertices} evaluated vertices contain NaN or infinity.",
                    obj.name,
                )
            )

        mesh_graph = bmesh.new()
        try:
            mesh_graph.from_mesh(evaluated_mesh)
            loose_vertices = sum(1 for vertex in mesh_graph.verts if not vertex.link_edges)
            degenerate_faces = sum(1 for face in mesh_graph.faces if face.calc_area() <= 1.0e-12)
            boundary_edges = sum(1 for edge in mesh_graph.edges if edge.is_boundary)
            non_manifold_edges = sum(
                1 for edge in mesh_graph.edges if not edge.is_manifold and not edge.is_boundary
            )
        finally:
            mesh_graph.free()

        if loose_vertices:
            findings.append(
                issue("error", "LOOSE_VERTICES", f"{loose_vertices} loose vertices found.", obj.name)
            )
        if degenerate_faces:
            findings.append(
                issue("error", "DEGENERATE_FACES", f"{degenerate_faces} zero-area faces found.", obj.name)
            )
        if non_manifold_edges:
            findings.append(
                issue(
                    "error",
                    "NON_MANIFOLD_EDGES",
                    f"{non_manifold_edges} non-manifold interior edges found.",
                    obj.name,
                )
            )
        if boundary_edges:
            findings.append(
                issue(
                    "warning",
                    "OPEN_BOUNDARY_EDGES",
                    f"{boundary_edges} open boundary edges found; confirm this is intentional.",
                    obj.name,
                )
            )

        active_uv = evaluated_mesh.uv_layers.active
        has_uv = (
            active_uv is not None
            and len(active_uv.data) == len(evaluated_mesh.loops)
            and len(active_uv.data) > 0
        )
        if not has_uv:
            findings.append(
                issue(
                    "error" if require_uv else "warning",
                    "MISSING_UV",
                    "Mesh has no UV map.",
                    obj.name,
                )
            )
        elif any(not finite_vector(loop.uv) for loop in active_uv.data):
            findings.append(
                issue(
                    "error",
                    "NON_FINITE_UV",
                    "Evaluated UV map contains NaN or infinity.",
                    obj.name,
                )
            )
        evaluated_slots = list(evaluated.material_slots)
        assigned_materials = [
            slot.material for slot in evaluated_slots if slot.material is not None
        ]
        has_material = bool(assigned_materials)
        if not has_material:
            findings.append(
                issue(
                    "error" if require_material else "warning",
                    "MISSING_MATERIAL",
                    "Mesh has no assigned material.",
                    obj.name,
                )
            )
        unassigned_polygons = sum(
            1
            for polygon in evaluated_mesh.polygons
            if polygon.material_index >= len(evaluated_slots)
            or evaluated_slots[polygon.material_index].material is None
        )
        if has_material and unassigned_polygons:
            findings.append(
                issue(
                    "error" if require_material else "warning",
                    "UNASSIGNED_POLYGON_MATERIAL",
                    f"{unassigned_polygons} polygons reference an empty material slot.",
                    obj.name,
                )
            )

        metrics = {
            "name": obj.name,
            "vertices": vertex_count,
            "polygons": polygon_count,
            "triangles": triangle_count,
            "uv_layers": len(evaluated_mesh.uv_layers),
            "material_slots": len(evaluated_slots),
            "assigned_materials": [material.name for material in assigned_materials],
            "modifiers": [modifier.type for modifier in obj.modifiers],
            "dimensions": [round(float(value), 6) for value in obj.dimensions],
            "location": [round(float(value), 6) for value in obj.location],
            "rotation_euler": [round(float(value), 6) for value in obj.rotation_euler],
            "scale": [round(float(value), 6) for value in obj.scale],
        }
    finally:
        evaluated.to_mesh_clear()

    return metrics, findings


def inspect_scene(
    *,
    require_uv: bool = False,
    require_material: bool = False,
    max_triangles: int | None = None,
) -> dict[str, Any]:
    depsgraph = bpy.context.evaluated_depsgraph_get()
    findings: list[dict[str, str]] = []
    meshes: list[dict[str, Any]] = []

    for obj in sorted(bpy.context.scene.objects, key=lambda candidate: candidate.name):
        if obj.hide_render:
            continue
        if obj.type in {"CURVE", "SURFACE", "FONT", "META"}:
            findings.append(
                issue(
                    "error",
                    "CONVERT_TO_MESH_REQUIRED",
                    (
                        f"Render-enabled {obj.type} objects bypass mesh quality checks; "
                        "convert to MESH before export."
                    ),
                    obj.name,
                )
            )
            continue
        if obj.type != "MESH":
            continue
        metrics, mesh_findings = inspect_mesh(obj, depsgraph, require_uv, require_material)
        meshes.append(metrics)
        findings.extend(mesh_findings)

    if not meshes:
        findings.append(issue("error", "NO_EXPORTABLE_MESH", "Scene has no render-enabled mesh objects."))

    total_triangles = sum(mesh["triangles"] for mesh in meshes)
    if max_triangles is not None and total_triangles > max_triangles:
        findings.append(
            issue(
                "error",
                "TRIANGLE_BUDGET_EXCEEDED",
                f"Scene has {total_triangles} triangles; budget is {max_triangles}.",
            )
        )

    scene = bpy.context.scene
    if scene.unit_settings.system != "METRIC":
        findings.append(issue("warning", "UNIT_SYSTEM", "Scene unit system is not METRIC."))
    if not math.isclose(scene.unit_settings.scale_length, 1.0, abs_tol=1.0e-8):
        findings.append(
            issue(
                "warning",
                "UNIT_SCALE",
                f"Scene unit scale is {scene.unit_settings.scale_length}; expected 1.0.",
            )
        )

    images: list[dict[str, Any]] = []
    for image in sorted(bpy.data.images, key=lambda candidate: candidate.name):
        if image.source != "FILE" or not image.filepath:
            continue
        resolved = Path(bpy.path.abspath(image.filepath)).expanduser().resolve()
        packed = image.packed_file is not None
        exists = resolved.is_file()
        images.append(
            {
                "name": image.name,
                "filepath": image.filepath,
                "resolved_path": str(resolved),
                "packed": packed,
                "exists": exists,
            }
        )
        if not packed and not exists:
            findings.append(
                issue("error", "MISSING_IMAGE", f"External image does not exist: {resolved}")
            )

    errors = [entry for entry in findings if entry["severity"] == "error"]
    warnings = [entry for entry in findings if entry["severity"] == "warning"]
    return {
        "schema_version": 1,
        "blender_version": bpy.app.version_string,
        "blend_file": bpy.data.filepath or None,
        "scene": scene.name,
        "mesh_count": len(meshes),
        "total_triangles": total_triangles,
        "meshes": meshes,
        "images": images,
        "issues": findings,
        "summary": {
            "errors": len(errors),
            "warnings": len(warnings),
            "status": "error" if errors else "warning" if warnings else "ok",
        },
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", required=True, help="JSON report under staging/reports")
    parser.add_argument("--require-uv", action="store_true")
    parser.add_argument("--require-material", action="store_true")
    parser.add_argument("--max-triangles", type=positive_int)
    parser.add_argument("--strict", action="store_true", help="Treat warnings as a failed gate")
    return parser


def main() -> None:
    args = build_parser().parse_args(script_arguments())
    report_path = staging_path(args.report, "reports")
    report = inspect_scene(
        require_uv=args.require_uv,
        require_material=args.require_material,
        max_triangles=args.max_triangles,
    )
    write_json_atomic(report_path, report)
    print(
        f"SCENE_VALIDATION report={report_path} "
        f"errors={report['summary']['errors']} warnings={report['summary']['warnings']}"
    )
    failed = report["summary"]["errors"] > 0 or (args.strict and report["summary"]["warnings"] > 0)
    if failed:
        raise RuntimeError("scene validation failed; inspect the JSON report")


if __name__ == "__main__":
    main()
