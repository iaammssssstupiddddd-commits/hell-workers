"""Validate the opaque 512px Wall formwork albedo candidate."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from PIL import Image

from workflow_common import staging_path, write_json_atomic


class TextureError(RuntimeError):
    pass


def validate(path: Path) -> dict[str, object]:
    if not path.is_file() or path.is_symlink():
        raise TextureError(f"formwork texture is absent: {path}")
    with Image.open(path) as image:
        image.load()
        if image.format != "PNG" or image.size != (512, 512):
            raise TextureError("formwork texture must be a 512x512 PNG")
        alpha = image.getchannel("A") if "A" in image.getbands() else None
        if alpha is not None and alpha.getextrema() != (255, 255):
            raise TextureError("formwork texture must be fully opaque")
        rgb = image.convert("RGB")
        extrema = rgb.getextrema()
        if all(low == high for low, high in extrema):
            raise TextureError("formwork texture has no color variation")
    return {
        "asset_set_id": "wall-production-v1",
        "bytes": path.stat().st_size,
        "height": 512,
        "opaque": True,
        "path": str(path.resolve()),
        "role": "texture:formwork_albedo",
        "schema_version": 1,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "status": "pass",
        "width": 512,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True)
    parser.add_argument("--report", required=True)
    args = parser.parse_args()
    texture = staging_path(args.input, "exports")
    report = staging_path(args.report, "reports")
    result = validate(texture)
    write_json_atomic(report, result)
    print(f"WALL_FORMWORK_TEXTURE status=pass report={report}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except TextureError as error:
        print(f"Wall formwork texture validation failed: {error}")
        raise SystemExit(1) from error
