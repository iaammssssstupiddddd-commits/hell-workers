"""Mechanically pack image-generated cut-core art; never repaint exterior pixels.

ImageMagick only resamples/packs the supplied artwork. Authoring the grain and
fractures belongs to the image-generation step, not a procedural raster script.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
from pathlib import Path

from PIL import Image, ImageChops, ImageStat

from wall_surface_uv import CORE_PACK_RECT, PROFILE
from workflow_common import asset_root, staging_path, write_json_atomic


def verify_packed(original: Path, packed: Path, *, emissive: bool) -> dict:
    with Image.open(original) as source, Image.open(packed) as result:
        if source.size != (1024, 1024) or result.size != source.size or result.mode != "RGB":
            raise ValueError("Wall atlas must remain 1024x1024 RGB")
        x, y, width, height = CORE_PACK_RECT
        # Compare every pixel outside the dedicated slot, not just UV samples.
        for box in ((0, 0, 1024, y), (0, y + height, 1024, 1024),
                    (0, y, x, y + height), (x + width, y, 1024, y + height)):
            if ImageChops.difference(source.convert("RGB").crop(box), result.crop(box)).getbbox():
                raise ValueError("Wall packing modified protected exterior pixels")
        tile = result.crop((x, y, x + width, y + height))
        stats = ImageStat.Stat(tile)
        if emissive and any(high != 0 for _, high in tile.getextrema()):
            raise ValueError("Wall cut core must not emit light")
        return {"sha256": hashlib.sha256(packed.read_bytes()).hexdigest(),
                "exterior_pixels_unchanged": True, "mean_rgb": stats.mean,
                "stddev_rgb": stats.stddev}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, required=True)
    parser.add_argument("--core-art", type=Path, required=True)
    args = parser.parse_args()
    root = asset_root()
    output_dir = root / "staging/exports/textures/buildings/wall"
    outputs = [staging_path(output_dir / name, "exports")
               for name in ("wall_albedo.png", "wall_emissive.png")]
    if any(path.exists() for path in outputs):
        raise FileExistsError("Use a fresh staging root; atlas outputs are immutable")
    with Image.open(args.core_art) as art:
        if art.width != art.height or art.width < 256:
            raise ValueError("Cut-core artwork must be a square of at least 256px")
    x, y, width, height = CORE_PACK_RECT
    records = {}
    for path in outputs:
        source = args.source_dir / path.name
        if path.name == "wall_albedo.png":
            tile_args = [str(args.core_art), "-resize", "256x256!",
                         "-virtual-pixel", "edge", "-set", "option:distort:viewport",
                         "260x260-2-2", "-distort", "SRT", "0", "+repage"]
        else:
            tile_args = ["-size", f"{width}x{height}", "xc:black"]
        subprocess.run(["magick", str(source), "(", *tile_args, ")", "-geometry",
                        f"+{x}+{y}", "-compose", "Over", "-composite", "-depth", "8",
                        f"PNG24:{path}"], check=True)
        records[path.name] = verify_packed(source, path, emissive=path == outputs[1])
        records[path.name]["source_sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
    write_json_atomic(staging_path(root / "staging/reports/core-atlas.json", "reports"), {
        "profile": PROFILE, "pack_rect": list(CORE_PACK_RECT),
        "core_art": str(args.core_art.resolve()),
        "core_art_sha256": hashlib.sha256(args.core_art.read_bytes()).hexdigest(),
        "textures": records,
    })


if __name__ == "__main__":
    main()
