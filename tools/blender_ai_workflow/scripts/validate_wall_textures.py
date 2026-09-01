"""Validate shared production Wall textures and the optional normal candidate."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import statistics
from pathlib import Path
from typing import Any

from PIL import Image


class TextureError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise TextureError(message)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_rgb(path: Path, role: str) -> tuple[Image.Image, list[tuple[int, int, int]]]:
    require(path.is_file() and not path.is_symlink(), f"{role} texture is absent")
    image = Image.open(path)
    require(image.size == (1024, 1024), f"{role} texture size differs")
    require(image.mode == "RGB", f"{role} texture must be opaque RGB")
    pixels = list(image.get_flattened_data())
    require(len(pixels) == 1024 * 1024, f"{role} pixel count differs")
    return image, pixels


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    return ordered[min(int(fraction * len(ordered)), len(ordered) - 1)]


def validate_textures(
    texture_root: Path,
    *,
    normal_sampling: str,
    normal_convention: str,
) -> dict[str, Any]:
    paths = {
        role: texture_root / f"wall_{role}.png"
        for role in ("albedo", "emissive", "normal")
    }
    _, albedo = load_rgb(paths["albedo"], "albedo")
    _, emissive = load_rgb(paths["emissive"], "emissive")
    _, normal = load_rgb(paths["normal"], "normal")
    require(
        normal_sampling == "linear", "normal texture must be sampled as linear data"
    )
    require(normal_convention == "+Y", "normal texture must use OpenGL +Y convention")

    hot_indices = [index for index, pixel in enumerate(emissive) if max(pixel) > 48]
    require(hot_indices, "emissive texture has no active pixels")
    hot_fraction = len(hot_indices) / len(emissive)
    require(hot_fraction <= 0.05, "emissive active area is too broad")
    for index in hot_indices:
        x = index % 1024
        y = index // 1024
        require(
            x / 1024 < 0.66 and y / 1024 > 0.70,
            "emissive active pixel escapes the purple UV region",
        )
    hot_means = [
        statistics.fmean(emissive[index][channel] for index in hot_indices)
        for channel in range(3)
    ]
    require(
        hot_means[0] > 2.0 * hot_means[1] and hot_means[2] > 2.0 * hot_means[1],
        "emissive active pixels are not magenta-purple",
    )

    vectors = [tuple(channel / 127.5 - 1.0 for channel in pixel) for pixel in normal]
    lengths = [
        math.sqrt(sum(channel * channel for channel in vector)) for vector in vectors
    ]
    length_p05 = percentile(lengths, 0.05)
    length_p50 = percentile(lengths, 0.50)
    length_p95 = percentile(lengths, 0.95)
    blue_positive_fraction = sum(vector[2] > 0.0 for vector in vectors) / len(vectors)
    mean_blue = statistics.fmean(pixel[2] for pixel in normal)
    require(
        length_p05 >= 0.75 and length_p95 <= 1.25,
        "normal vector length distribution differs",
    )
    require(
        blue_positive_fraction >= 0.999, "normal texture contains back-facing vectors"
    )
    require(mean_blue >= 220.0, "normal texture has insufficient positive Z")

    return {
        "schema_version": 1,
        "status": "pass",
        "asset_set_id": "wall-production-v1",
        "textures": {
            role: {
                "path": str(path.resolve()),
                "bytes": path.stat().st_size,
                "sha256": sha256(path),
                "width": 1024,
                "height": 1024,
                "mode": "RGB",
            }
            for role, path in paths.items()
        },
        "emissive": {
            "active_threshold": 48,
            "active_fraction": round(hot_fraction, 8),
            "active_mean_rgb": [round(value, 6) for value in hot_means],
            "allowed_uv_region": {"u_max_exclusive": 0.66, "v_min_exclusive": 0.70},
        },
        "normal": {
            "sampling": normal_sampling,
            "convention": normal_convention,
            "vector_length_p05": round(length_p05, 6),
            "vector_length_p50": round(length_p50, 6),
            "vector_length_p95": round(length_p95, 6),
            "blue_positive_fraction": round(blue_positive_fraction, 8),
            "mean_blue": round(mean_blue, 6),
        },
        "albedo_mean_rgb": [
            round(statistics.fmean(pixel[channel] for pixel in albedo), 6)
            for channel in range(3)
        ],
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--texture-root", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--normal-sampling", required=True, choices=("linear",))
    parser.add_argument("--normal-convention", required=True, choices=("+Y",))
    return parser


def main() -> int:
    args = build_parser().parse_args()
    report = validate_textures(
        args.texture_root.resolve(),
        normal_sampling=args.normal_sampling,
        normal_convention=args.normal_convention,
    )
    args.report.resolve().parent.mkdir(parents=True, exist_ok=True)
    args.report.resolve().write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"WALL_TEXTURE_VALIDATION status=pass report={args.report.resolve()}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (TextureError, OSError, ValueError) as error:
        print(f"Wall texture validation failed: {error}")
        raise SystemExit(1) from error
