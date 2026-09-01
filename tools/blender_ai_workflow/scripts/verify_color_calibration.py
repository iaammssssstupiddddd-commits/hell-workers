"""Fail-closed offline verifier for the wall color-calibration artifact pair."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import statistics
import struct
import sys
from pathlib import Path
from typing import Any

from PIL import Image

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from workflow_common import write_json_atomic


class ContractError(ValueError):
    """Raised when an input cannot belong to the sealed calibration contract."""


def load_json_object(path: Path, label: str) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ContractError(f"cannot read {label} JSON {path}: {error}") from error
    if not isinstance(payload, dict):
        raise ContractError(f"{label} must be a JSON object: {path}")
    return payload


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require_sha256(value: Any, label: str) -> str:
    if not isinstance(value, str) or len(value) != 64:
        raise ContractError(f"{label} must be a lowercase SHA-256 hex digest")
    if value != value.lower() or any(character not in "0123456789abcdef" for character in value):
        raise ContractError(f"{label} must be a lowercase SHA-256 hex digest")
    return value


def inspect_png(path: Path) -> dict[str, int]:
    try:
        header = path.read_bytes()[:33]
    except OSError as error:
        raise ContractError(f"cannot read PNG {path}: {error}") from error
    if len(header) < 33 or header[:8] != b"\x89PNG\r\n\x1a\n":
        raise ContractError(f"image is not a PNG: {path}")
    chunk_length = struct.unpack(">I", header[8:12])[0]
    if header[12:16] != b"IHDR" or chunk_length != 13:
        raise ContractError(f"PNG does not begin with a canonical IHDR chunk: {path}")
    width, height, bit_depth, color_type, compression, filtering, interlace = struct.unpack(
        ">IIBBBBB", header[16:29]
    )
    if compression != 0 or filtering != 0 or interlace != 0:
        raise ContractError(f"PNG must use standard compression/filtering and no interlace: {path}")
    return {
        "width": width,
        "height": height,
        "bit_depth": bit_depth,
        "color_type": color_type,
    }


def srgb_channel_to_linear(value: float) -> float:
    normalized = value / 255.0
    if normalized <= 0.04045:
        return normalized / 12.92
    return ((normalized + 0.055) / 1.055) ** 2.4


def srgb_to_lab(rgb: tuple[float, float, float]) -> tuple[float, float, float]:
    red, green, blue = (srgb_channel_to_linear(value) for value in rgb)
    x = red * 0.4124564 + green * 0.3575761 + blue * 0.1804375
    y = red * 0.2126729 + green * 0.7151522 + blue * 0.0721750
    z = red * 0.0193339 + green * 0.1191920 + blue * 0.9503041

    def pivot(value: float) -> float:
        delta = 6.0 / 29.0
        if value > delta**3:
            return value ** (1.0 / 3.0)
        return value / (3.0 * delta**2) + 4.0 / 29.0

    fx = pivot(x / 0.95047)
    fy = pivot(y)
    fz = pivot(z / 1.08883)
    return (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))


def relative_luminance(rgb: tuple[float, float, float]) -> float:
    red, green, blue = (srgb_channel_to_linear(value) for value in rgb)
    return red * 0.2126729 + green * 0.7151522 + blue * 0.0721750


def delta_e_2000(
    first: tuple[float, float, float], second: tuple[float, float, float]
) -> float:
    """Return CIEDE2000 using the Sharma/Wu/Dalal reference equations."""

    l1, a1, b1 = first
    l2, a2, b2 = second
    c1 = math.hypot(a1, b1)
    c2 = math.hypot(a2, b2)
    c_bar = (c1 + c2) / 2.0
    c_bar_seventh = c_bar**7
    g = 0.5 * (1.0 - math.sqrt(c_bar_seventh / (c_bar_seventh + 25.0**7)))
    a1_prime = (1.0 + g) * a1
    a2_prime = (1.0 + g) * a2
    c1_prime = math.hypot(a1_prime, b1)
    c2_prime = math.hypot(a2_prime, b2)

    def hue_degrees(a_value: float, b_value: float) -> float:
        if math.isclose(a_value, 0.0, abs_tol=1.0e-15) and math.isclose(
            b_value, 0.0, abs_tol=1.0e-15
        ):
            return 0.0
        return math.degrees(math.atan2(b_value, a_value)) % 360.0

    h1_prime = hue_degrees(a1_prime, b1)
    h2_prime = hue_degrees(a2_prime, b2)
    delta_l_prime = l2 - l1
    delta_c_prime = c2_prime - c1_prime
    if math.isclose(c1_prime * c2_prime, 0.0, abs_tol=1.0e-15):
        delta_h_degrees = 0.0
    elif abs(h2_prime - h1_prime) <= 180.0:
        delta_h_degrees = h2_prime - h1_prime
    elif h2_prime <= h1_prime:
        delta_h_degrees = h2_prime - h1_prime + 360.0
    else:
        delta_h_degrees = h2_prime - h1_prime - 360.0
    delta_h_prime = 2.0 * math.sqrt(c1_prime * c2_prime) * math.sin(
        math.radians(delta_h_degrees / 2.0)
    )

    l_bar_prime = (l1 + l2) / 2.0
    c_bar_prime = (c1_prime + c2_prime) / 2.0
    if math.isclose(c1_prime * c2_prime, 0.0, abs_tol=1.0e-15):
        h_bar_prime = h1_prime + h2_prime
    elif abs(h1_prime - h2_prime) <= 180.0:
        h_bar_prime = (h1_prime + h2_prime) / 2.0
    elif h1_prime + h2_prime < 360.0:
        h_bar_prime = (h1_prime + h2_prime + 360.0) / 2.0
    else:
        h_bar_prime = (h1_prime + h2_prime - 360.0) / 2.0

    t = (
        1.0
        - 0.17 * math.cos(math.radians(h_bar_prime - 30.0))
        + 0.24 * math.cos(math.radians(2.0 * h_bar_prime))
        + 0.32 * math.cos(math.radians(3.0 * h_bar_prime + 6.0))
        - 0.20 * math.cos(math.radians(4.0 * h_bar_prime - 63.0))
    )
    delta_theta = 30.0 * math.exp(-(((h_bar_prime - 275.0) / 25.0) ** 2))
    c_bar_prime_seventh = c_bar_prime**7
    r_c = 2.0 * math.sqrt(
        c_bar_prime_seventh / (c_bar_prime_seventh + 25.0**7)
    )
    l_delta = l_bar_prime - 50.0
    s_l = 1.0 + (0.015 * l_delta * l_delta) / math.sqrt(20.0 + l_delta * l_delta)
    s_c = 1.0 + 0.045 * c_bar_prime
    s_h = 1.0 + 0.015 * c_bar_prime * t
    r_t = -math.sin(math.radians(2.0 * delta_theta)) * r_c
    l_term = delta_l_prime / s_l
    c_term = delta_c_prime / s_c
    h_term = delta_h_prime / s_h
    return math.sqrt(
        l_term * l_term
        + c_term * c_term
        + h_term * h_term
        + r_t * c_term * h_term
    )


def median_rgb(image: Image.Image, center: list[int], roi_size: int) -> tuple[float, float, float]:
    if len(center) != 2 or any(not isinstance(value, int) for value in center):
        raise ContractError(f"invalid patch center: {center!r}")
    if roi_size <= 0 or roi_size % 2 != 0:
        raise ContractError("ROI size must be a positive even integer")
    half = roi_size // 2
    left, top = center[0] - half, center[1] - half
    right, bottom = left + roi_size, top + roi_size
    if left < 0 or top < 0 or right > image.width or bottom > image.height:
        raise ContractError(f"ROI {(left, top, right, bottom)} is outside the image")
    pixels = list(
        image.crop((left, top, right, bottom)).convert("RGB").get_flattened_data()
    )
    return tuple(float(statistics.median(pixel[channel] for pixel in pixels)) for channel in range(3))


def validate_metadata(
    metadata: dict[str, Any],
    *,
    label: str,
    expected_renderer: str,
    image_path: Path,
    contract: dict[str, Any],
) -> None:
    if metadata.get("schema_version") != 1:
        raise ContractError(f"{label} metadata schema_version must be 1")
    if metadata.get("contract_id") != contract.get("contract_id"):
        raise ContractError(f"{label} metadata contract_id mismatch")
    if metadata.get("renderer") != expected_renderer:
        raise ContractError(f"{label} renderer must be {expected_renderer}")
    if not isinstance(metadata.get("source_fingerprint"), str) or not metadata["source_fingerprint"]:
        raise ContractError(f"{label} source_fingerprint is required")

    capture = metadata.get("capture")
    if not isinstance(capture, dict):
        raise ContractError(f"{label} capture metadata must be an object")
    for field in contract["metadata"]["required_capture_fields"]:
        if field not in capture:
            raise ContractError(f"{label} capture is missing {field}")
    image_contract = contract["image"]
    expected_capture = {
        "encoded_format": "png",
        "height": image_contract["height"],
        "rescaled": False,
        "srgb": True,
        "width": image_contract["width"],
    }
    for field, expected in expected_capture.items():
        if capture.get(field) != expected:
            raise ContractError(
                f"{label} capture {field} must be {expected!r}; got {capture.get(field)!r}"
            )
    actual_sha256 = sha256_file(image_path)
    if require_sha256(capture.get("image_sha256"), f"{label} capture image_sha256") != actual_sha256:
        raise ContractError(f"{label} capture image_sha256 does not match {image_path}")

    color = metadata.get("color")
    if not isinstance(color, dict):
        raise ContractError(f"{label} color metadata must be an object")
    for field in contract["metadata"]["required_color_fields"]:
        if field not in color:
            raise ContractError(f"{label} color metadata is missing {field}")
    pipeline = contract["color_pipeline"]
    expected_color = {
        "automatic_exposure": pipeline["automatic_exposure"],
        "display_device": pipeline["display_device"],
        "exposure": pipeline["exposure"],
        "gamma": pipeline["gamma"],
        "look": pipeline["look"],
        "tonemapping": pipeline["tonemapping"],
        "view_transform": pipeline["reference_view_transform"],
    }
    for field, expected in expected_color.items():
        if color.get(field) != expected:
            raise ContractError(
                f"{label} color {field} must be {expected!r}; got {color.get(field)!r}"
            )

    expected_inputs = {
        patch["id"]: patch["input_hex_srgb"] for patch in contract["base_patches"]
    }
    emissive = contract["emissive_sanity"]
    expected_inputs[emissive["id"]] = emissive["input_hex_srgb"]
    if metadata.get("patch_inputs") != expected_inputs:
        raise ContractError(f"{label} patch_inputs do not match the contract")

    if label == "reference":
        ocio = metadata.get("ocio")
        if not isinstance(ocio, dict):
            raise ContractError("reference OCIO metadata must be an object")
        for field in contract["metadata"]["required_ocio_fields"]:
            if field not in ocio:
                raise ContractError(f"reference OCIO metadata is missing {field}")
        if not isinstance(ocio["config_path"], str) or not ocio["config_path"]:
            raise ContractError("reference OCIO config_path is required")
        require_sha256(ocio["config_sha256"], "reference OCIO config_sha256")
        if not isinstance(ocio["runtime_version"], str) or not ocio["runtime_version"]:
            raise ContractError("reference OCIO runtime_version is required")
        if ocio["fallback"] is not False:
            raise ContractError("reference OCIO fallback must be false")


def open_contract_image(path: Path, contract: dict[str, Any]) -> Image.Image:
    png = inspect_png(path)
    image_contract = contract["image"]
    if png["width"] != image_contract["width"] or png["height"] != image_contract["height"]:
        raise ContractError(f"PNG dimensions do not match the contract: {path}")
    if png["bit_depth"] != image_contract["bit_depth"] or png["color_type"] not in (2, 6):
        raise ContractError(f"PNG must be 8-bit RGB or RGBA: {path}")
    try:
        image = Image.open(path)
        image.load()
    except OSError as error:
        raise ContractError(f"Pillow cannot decode {path}: {error}") from error
    if image.mode not in ("RGB", "RGBA"):
        raise ContractError(f"decoded PNG mode must be RGB or RGBA: {path} ({image.mode})")
    return image


def verify_calibration(
    *,
    contract_path: Path,
    reference_path: Path,
    reference_metadata_path: Path,
    candidate_path: Path,
    candidate_metadata_path: Path,
) -> dict[str, Any]:
    contract = load_json_object(contract_path, "contract")
    if contract.get("schema_version") != 1:
        raise ContractError("calibration contract schema_version must be 1")
    reference_metadata = load_json_object(reference_metadata_path, "reference metadata")
    candidate_metadata = load_json_object(candidate_metadata_path, "candidate metadata")
    validate_metadata(
        reference_metadata,
        label="reference",
        expected_renderer=contract["metadata"]["reference_renderer"],
        image_path=reference_path,
        contract=contract,
    )
    validate_metadata(
        candidate_metadata,
        label="candidate",
        expected_renderer=contract["metadata"]["candidate_renderer"],
        image_path=candidate_path,
        contract=contract,
    )
    reference = open_contract_image(reference_path, contract)
    candidate = open_contract_image(candidate_path, contract)
    roi_size = contract["image"]["roi_size_px"]
    patch_reports: list[dict[str, Any]] = []
    delta_values: list[float] = []
    for patch in contract["base_patches"]:
        reference_rgb = median_rgb(reference, patch["center_px"], roi_size)
        candidate_rgb = median_rgb(candidate, patch["center_px"], roi_size)
        reference_lab = srgb_to_lab(reference_rgb)
        candidate_lab = srgb_to_lab(candidate_rgb)
        delta_e = delta_e_2000(reference_lab, candidate_lab)
        delta_values.append(delta_e)
        patch_reports.append(
            {
                "id": patch["id"],
                "center_px": patch["center_px"],
                "roi_size_px": roi_size,
                "reference_median_srgb8": [round(value, 6) for value in reference_rgb],
                "candidate_median_srgb8": [round(value, 6) for value in candidate_rgb],
                "reference_lab_d65": [round(value, 6) for value in reference_lab],
                "candidate_lab_d65": [round(value, 6) for value in candidate_lab],
                "delta_e_2000": round(delta_e, 6),
            }
        )

    emissive_contract = contract["emissive_sanity"]
    base_patch = next(
        (
            patch
            for patch in contract["base_patches"]
            if patch["id"] == emissive_contract["base_patch_id"]
        ),
        None,
    )
    if base_patch is None:
        raise ContractError("emissive base_patch_id is not present in base_patches")
    reference_base_rgb = median_rgb(reference, base_patch["center_px"], roi_size)
    candidate_base_rgb = median_rgb(candidate, base_patch["center_px"], roi_size)
    reference_emissive_rgb = median_rgb(reference, emissive_contract["center_px"], roi_size)
    candidate_emissive_rgb = median_rgb(candidate, emissive_contract["center_px"], roi_size)
    reference_lift = relative_luminance(reference_emissive_rgb) - relative_luminance(
        reference_base_rgb
    )
    candidate_lift = relative_luminance(candidate_emissive_rgb) - relative_luminance(
        candidate_base_rgb
    )
    minimum_lift = float(emissive_contract["minimum_relative_luminance_lift"])
    emissive_pass = reference_lift >= minimum_lift and candidate_lift >= minimum_lift
    each_limit = float(contract["thresholds"]["delta_e_2000_each_max"])
    mean_limit = float(contract["thresholds"]["delta_e_2000_mean_max"])
    mean_delta = statistics.fmean(delta_values)
    base_pass = all(value <= each_limit for value in delta_values) and mean_delta <= mean_limit
    status = "pass" if base_pass and emissive_pass else "failed"
    return {
        "schema_version": 1,
        "contract_id": contract["contract_id"],
        "status": status,
        "contract": {
            "path": str(contract_path.resolve()),
            "sha256": sha256_file(contract_path),
        },
        "reference": {
            "path": str(reference_path.resolve()),
            "sha256": sha256_file(reference_path),
            "metadata_path": str(reference_metadata_path.resolve()),
            "metadata_sha256": sha256_file(reference_metadata_path),
        },
        "candidate": {
            "path": str(candidate_path.resolve()),
            "sha256": sha256_file(candidate_path),
            "metadata_path": str(candidate_metadata_path.resolve()),
            "metadata_sha256": sha256_file(candidate_metadata_path),
        },
        "base_color_gate": {
            "each_limit": each_limit,
            "mean_limit": mean_limit,
            "mean_delta_e_2000": round(mean_delta, 6),
            "passed": base_pass,
            "patches": patch_reports,
        },
        "emissive_sanity_gate": {
            "minimum_relative_luminance_lift": minimum_lift,
            "reference_lift": round(reference_lift, 6),
            "candidate_lift": round(candidate_lift, 6),
            "passed": emissive_pass,
        },
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", required=True, type=Path)
    parser.add_argument("--reference", required=True, type=Path)
    parser.add_argument("--reference-metadata", required=True, type=Path)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--candidate-metadata", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    output = args.output.expanduser().resolve()
    if output.exists():
        raise ContractError(f"refusing to overwrite calibration report: {output}")
    try:
        report = verify_calibration(
            contract_path=args.contract.expanduser().resolve(),
            reference_path=args.reference.expanduser().resolve(),
            reference_metadata_path=args.reference_metadata.expanduser().resolve(),
            candidate_path=args.candidate.expanduser().resolve(),
            candidate_metadata_path=args.candidate_metadata.expanduser().resolve(),
        )
    except ContractError as error:
        report = {
            "schema_version": 1,
            "status": "blocked",
            "error": str(error),
        }
    write_json_atomic(output, report)
    if report["status"] == "blocked":
        print(f"WALL_COLOR_CALIBRATION status=blocked error={report['error']} report={output}")
        return 1
    print(
        f"WALL_COLOR_CALIBRATION status={report['status']} "
        f"mean_delta_e_2000={report['base_color_gate']['mean_delta_e_2000']} "
        f"report={output}"
    )
    return 0 if report["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
