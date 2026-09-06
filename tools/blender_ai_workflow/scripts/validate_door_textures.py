"""Validate the production Door albedo and two fixed-canvas previews."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from PIL import Image


class TextureError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise TextureError(message)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(root: Path) -> dict[str, object]:
    albedo_path = root / "door_albedo.png"
    require(albedo_path.is_file() and not albedo_path.is_symlink(), "Door albedo is absent")
    with Image.open(albedo_path) as albedo:
        require(albedo.size == (512, 512), "Door albedo must be 512x512")
        require("A" not in albedo.getbands(), "Door albedo must be opaque RGB")
    previews = {}
    for axis in ("ew", "ns"):
        path = root / f"door_preview_{axis}.png"
        require(path.is_file() and not path.is_symlink(), f"Door {axis} preview is absent")
        with Image.open(path) as image:
            require(image.size == (256, 256), f"Door {axis} preview must be 256x256")
            require("A" in image.getbands(), f"Door {axis} preview requires alpha")
            alpha = image.getchannel("A")
            bbox = alpha.getbbox()
            require(bbox is not None, f"Door {axis} preview is empty")
            require(
                bbox[0] >= 16 and bbox[1] >= 16 and bbox[2] <= 240 and bbox[3] <= 224,
                f"Door {axis} preview exceeds the fixed canvas safe area: {bbox}",
            )
            previews[axis] = {"alpha_bbox": list(bbox), "sha256": digest(path)}
    require(
        previews["ew"]["sha256"] != previews["ns"]["sha256"],
        "Door axis previews are identical",
    )
    return {
        "albedo": {"opaque": True, "sha256": digest(albedo_path), "size": [512, 512]},
        "asset_set_id": "door-production-v1",
        "previews": previews,
        "schema_version": 1,
        "status": "pass",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--texture-root", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    result = validate(args.texture_root.resolve())
    args.report.resolve().write_text(
        json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(f"DOOR_TEXTURE_VALIDATION status=pass report={args.report.resolve()}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (TextureError, OSError, ValueError) as error:
        print(f"Door texture validation failed: {error}")
        raise SystemExit(1) from error
