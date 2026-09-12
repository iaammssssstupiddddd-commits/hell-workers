"""Mechanically pack image-generated masonry and cut-core art into fixed slots.

ImageMagick only resamples/packs the supplied artwork. Authoring the grain and
fractures belongs to the image-generation step, not a procedural raster script.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
from pathlib import Path

from PIL import Image, ImageChops, ImageStat

from wall_surface_uv import PACK_RECTS, PROFILE
from workflow_common import asset_root, staging_path, write_json_atomic


def verify_packed(original: Path, packed: Path, *, emissive: bool) -> dict:
    with Image.open(original) as source, Image.open(packed) as result:
        if source.size != (1024, 1024) or result.size != source.size or result.mode != "RGB":
            raise ValueError("Wall atlas must remain 1024x1024 RGB")
        # Partition at every slot boundary and compare every protected pixel.
        xs = sorted({0, 1024, *(v for x, _, w, _ in PACK_RECTS.values() for v in (x, x + w))})
        ys = sorted({0, 1024, *(v for _, y, _, h in PACK_RECTS.values() for v in (y, y + h))})
        original_rgb = source.convert("RGB")
        for left, right in zip(xs, xs[1:]):
            for top, bottom in zip(ys, ys[1:]):
                if any(x <= left < x + w and y <= top < y + h
                       for x, y, w, h in PACK_RECTS.values()):
                    continue
                box = (left, top, right, bottom)
                if ImageChops.difference(original_rgb.crop(box), result.crop(box)).getbbox():
                    raise ValueError("Wall packing modified protected exterior pixels")
        surfaces = {}
        for name, (x, y, width, height) in PACK_RECTS.items():
            tile = result.crop((x, y, x + width, y + height))
            stats = ImageStat.Stat(tile)
            if emissive and any(high != 0 for _, high in tile.getextrema()):
                raise ValueError(f"Wall {name} must not emit light")
            surfaces[name] = {"mean_rgb": stats.mean, "stddev_rgb": stats.stddev}
        return {"sha256": hashlib.sha256(packed.read_bytes()).hexdigest(),
                "protected_pixels_unchanged": True, "surfaces": surfaces}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-dir", type=Path, required=True)
    parser.add_argument("--core-art", type=Path, required=True)
    parser.add_argument("--stone-art", type=Path, required=True)
    args = parser.parse_args()
    root = asset_root()
    output_dir = root / "staging/exports/textures/buildings/wall"
    outputs = [staging_path(output_dir / name, "exports")
               for name in ("wall_albedo.png", "wall_emissive.png")]
    if any(path.exists() for path in outputs):
        raise FileExistsError("Use a fresh staging root; atlas outputs are immutable")
    artwork = {"stone": args.stone_art, "core": args.core_art}
    for name, path in artwork.items():
        with Image.open(path) as art:
            if art.width != art.height or art.width < PACK_RECTS[name][2] - 4:
                raise ValueError(f"{name} artwork must be square and at least the packed tile size")
    records = {}
    for path in outputs:
        source = args.source_dir / path.name
        command = ["magick", str(source)]
        for name, (x, y, width, height) in PACK_RECTS.items():
            if path.name == "wall_albedo.png":
                tile_args = [str(artwork[name]), "-resize", f"{width - 4}x{height - 4}!",
                             "-virtual-pixel", "edge", "-set", "option:distort:viewport",
                             f"{width}x{height}-2-2", "-distort", "SRT", "0", "+repage"]
            else:
                tile_args = ["-size", f"{width}x{height}", "xc:black"]
            command.extend(["(", *tile_args, ")", "-geometry", f"+{x}+{y}",
                            "-compose", "Over", "-composite"])
        subprocess.run([*command, "-depth", "8", f"PNG24:{path}"], check=True)
        records[path.name] = verify_packed(source, path, emissive=path == outputs[1])
        records[path.name]["source_sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
    write_json_atomic(staging_path(root / "staging/reports/core-atlas.json", "reports"), {
        "profile": PROFILE, "pack_rects": {name: list(rect) for name, rect in PACK_RECTS.items()},
        "artwork": {name: {"path": str(path.resolve()),
                           "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
                    for name, path in artwork.items()},
        "textures": records,
    })


if __name__ == "__main__":
    main()
