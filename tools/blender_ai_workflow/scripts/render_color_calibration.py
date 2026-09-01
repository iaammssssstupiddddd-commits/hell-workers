"""Render the sealed wall color-calibration board with Blender."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import bpy

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from workflow_common import script_arguments, staging_path, write_json_atomic


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def srgb_channel_to_linear(value: int) -> float:
    normalized = value / 255.0
    if normalized <= 0.04045:
        return normalized / 12.92
    return ((normalized + 0.055) / 1.055) ** 2.4


def hex_to_linear_rgba(value: str) -> tuple[float, float, float, float]:
    if len(value) != 7 or not value.startswith("#"):
        raise ValueError(f"expected #RRGGBB color, got {value!r}")
    channels = tuple(int(value[index : index + 2], 16) for index in (1, 3, 5))
    return tuple(srgb_channel_to_linear(channel) for channel in channels) + (1.0,)


def load_contract(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict) or payload.get("schema_version") != 1:
        raise ValueError(f"unsupported wall color contract: {path}")
    return payload


def clear_scene() -> None:
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    for collection in (bpy.data.meshes, bpy.data.materials, bpy.data.cameras):
        for item in list(collection):
            if item.users == 0:
                collection.remove(item)


def create_emission_material(
    name: str, color: tuple[float, float, float, float], strength: float
) -> bpy.types.Material:
    material = bpy.data.materials.new(name=name)
    material.use_nodes = True
    nodes = material.node_tree.nodes
    nodes.clear()
    output = nodes.new("ShaderNodeOutputMaterial")
    emission = nodes.new("ShaderNodeEmission")
    emission.inputs["Color"].default_value = color
    emission.inputs["Strength"].default_value = strength
    material.node_tree.links.new(emission.outputs["Emission"], output.inputs["Surface"])
    return material


def add_patch(
    *,
    patch_id: str,
    center_px: list[int],
    color_hex: str,
    strength: float,
    image_width: int,
    image_height: int,
    patch_size: int,
) -> None:
    if len(center_px) != 2:
        raise ValueError(f"invalid center for {patch_id}: {center_px!r}")
    world_x = float(center_px[0]) - image_width / 2.0
    world_y = image_height / 2.0 - float(center_px[1])
    bpy.ops.mesh.primitive_plane_add(
        size=float(patch_size),
        enter_editmode=False,
        align="WORLD",
        location=(world_x, world_y, 0.0),
    )
    patch = bpy.context.object
    patch.name = f"wall_color_patch_{patch_id}"
    patch.data.materials.append(
        create_emission_material(
            f"wall_color_material_{patch_id}",
            hex_to_linear_rgba(color_hex),
            strength,
        )
    )


def configure_color_pipeline(scene: bpy.types.Scene, contract: dict[str, Any]) -> None:
    pipeline = contract["color_pipeline"]
    display_device = pipeline["display_device"]
    view_transform = pipeline["reference_view_transform"]
    look = pipeline["look"]
    scene.display_settings.display_device = display_device
    scene.view_settings.view_transform = view_transform
    scene.view_settings.look = look
    if scene.display_settings.display_device != display_device:
        raise RuntimeError(f"Blender did not retain display device {display_device!r}")
    if scene.view_settings.view_transform != view_transform:
        raise RuntimeError(f"Blender did not retain view transform {view_transform!r}")
    if scene.view_settings.look != look:
        raise RuntimeError(f"Blender did not retain look {look!r}")
    scene.view_settings.exposure = float(pipeline["exposure"])
    scene.view_settings.gamma = float(pipeline["gamma"])
    scene.view_settings.use_curve_mapping = False


def resolve_ocio_evidence() -> dict[str, Any]:
    candidates: list[Path] = []
    configured = os.environ.get("OCIO")
    if configured:
        candidates.append(Path(configured))
    override = getattr(bpy.context.preferences.system, "ocio_config_override", "")
    if override:
        candidates.append(Path(override))
    for resource_kind in ("LOCAL", "SYSTEM"):
        resource = bpy.utils.resource_path(resource_kind)
        if resource:
            candidates.append(Path(resource) / "datafiles" / "colormanagement" / "config.ocio")
    config_path = next((candidate.resolve() for candidate in candidates if candidate.is_file()), None)
    try:
        import PyOpenColorIO as ocio

        runtime_version = getattr(ocio, "__version__", None) or ocio.GetVersion()
    except (ImportError, AttributeError) as error:
        runtime_version = f"unavailable:{type(error).__name__}"
    config_version = None
    if config_path:
        config_prefix = config_path.read_text(encoding="utf-8", errors="replace")[:4096]
        match = re.search(r"ocio_profile_version\s*:\s*['\"]?([0-9]+\.[0-9]+)", config_prefix)
        config_version = match.group(1) if match else None

    def major_minor(value: str | None) -> tuple[int, int] | None:
        if not value:
            return None
        match = re.search(r"([0-9]+)\.([0-9]+)", value)
        return (int(match.group(1)), int(match.group(2))) if match else None

    config_pair = major_minor(config_version)
    runtime_pair = major_minor(str(runtime_version))
    incompatible_version = (
        config_pair is not None
        and runtime_pair is not None
        and (config_pair[0] > runtime_pair[0] or (config_pair[0] == runtime_pair[0] and config_pair[1] > runtime_pair[1]))
    )
    return {
        "config_path": str(config_path) if config_path else "",
        "config_sha256": sha256_file(config_path) if config_path else None,
        "config_version": config_version,
        "runtime_version": str(runtime_version),
        "fallback": (
            config_path is None
            or config_version is None
            or runtime_pair is None
            or str(runtime_version).startswith("unavailable:")
            or incompatible_version
        ),
    }


def render_board(contract: dict[str, Any], output_path: Path) -> None:
    clear_scene()
    scene = bpy.context.scene
    image = contract["image"]
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = int(image["width"])
    scene.render.resolution_y = int(image["height"])
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = False
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGB"
    scene.render.image_settings.color_depth = str(image["bit_depth"])
    scene.render.image_settings.compression = 15
    scene.render.use_file_extension = False
    scene.render.filepath = str(output_path)
    configure_color_pipeline(scene, contract)

    world = bpy.data.worlds.new("wall_color_calibration_world")
    world.use_nodes = True
    background = world.node_tree.nodes.get("Background")
    background.inputs["Color"].default_value = (0.0, 0.0, 0.0, 1.0)
    background.inputs["Strength"].default_value = 0.0
    scene.world = world

    camera_data = bpy.data.cameras.new("wall_color_calibration_camera")
    camera_data.type = "ORTHO"
    camera_data.ortho_scale = float(image["height"])
    camera = bpy.data.objects.new("wall_color_calibration_camera", camera_data)
    scene.collection.objects.link(camera)
    camera.location = (0.0, 0.0, 10.0)
    scene.camera = camera

    for patch in contract["base_patches"]:
        add_patch(
            patch_id=patch["id"],
            center_px=patch["center_px"],
            color_hex=patch["input_hex_srgb"],
            strength=1.0,
            image_width=image["width"],
            image_height=image["height"],
            patch_size=image["patch_size_px"],
        )
    emissive = contract["emissive_sanity"]
    add_patch(
        patch_id=emissive["id"],
        center_px=emissive["center_px"],
        color_hex=emissive["input_hex_srgb"],
        strength=float(emissive["strength"]),
        image_width=image["width"],
        image_height=image["height"],
        patch_size=image["patch_size_px"],
    )
    result = bpy.ops.render.render(write_still=True)
    if "FINISHED" not in result or not output_path.is_file():
        raise RuntimeError(f"Blender calibration render did not finish: {result}")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", required=True, type=Path)
    parser.add_argument("--output", required=True)
    parser.add_argument("--metadata", required=True)
    parser.add_argument("--source-fingerprint", required=True)
    return parser


def main() -> None:
    args = build_parser().parse_args(script_arguments())
    contract_path = args.contract.expanduser().resolve()
    contract = load_contract(contract_path)
    output_path = staging_path(args.output, "reports")
    metadata_path = staging_path(args.metadata, "reports")
    if output_path.suffix.lower() != ".png":
        raise ValueError(f"calibration output must use .png: {output_path}")
    if metadata_path.suffix.lower() != ".json":
        raise ValueError(f"calibration metadata must use .json: {metadata_path}")
    if output_path.exists() or metadata_path.exists():
        raise FileExistsError("refusing to overwrite an existing calibration artifact")
    render_board(contract, output_path)
    pipeline = contract["color_pipeline"]
    patch_inputs = {
        patch["id"]: patch["input_hex_srgb"] for patch in contract["base_patches"]
    }
    emissive = contract["emissive_sanity"]
    patch_inputs[emissive["id"]] = emissive["input_hex_srgb"]
    metadata = {
        "schema_version": 1,
        "contract_id": contract["contract_id"],
        "renderer": "blender",
        "source_fingerprint": args.source_fingerprint,
        "created_at_utc": datetime.now(UTC).isoformat(),
        "blender_version": bpy.app.version_string,
        "contract": {
            "path": str(contract_path),
            "sha256": sha256_file(contract_path),
        },
        "capture": {
            "encoded_format": "png",
            "height": contract["image"]["height"],
            "image_sha256": sha256_file(output_path),
            "rescaled": False,
            "srgb": True,
            "width": contract["image"]["width"],
        },
        "color": {
            "automatic_exposure": pipeline["automatic_exposure"],
            "display_device": pipeline["display_device"],
            "exposure": pipeline["exposure"],
            "gamma": pipeline["gamma"],
            "look": pipeline["look"],
            "tonemapping": pipeline["tonemapping"],
            "view_transform": pipeline["reference_view_transform"],
        },
        "ocio": resolve_ocio_evidence(),
        "patch_inputs": patch_inputs,
        "emissive_strength": emissive["strength"],
    }
    write_json_atomic(metadata_path, metadata)
    print(
        f"WALL_COLOR_REFERENCE output={output_path} metadata={metadata_path} "
        f"ocio_fallback={str(metadata['ocio']['fallback']).lower()}"
    )


if __name__ == "__main__":
    main()
