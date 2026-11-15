"""Verify actual body uniforms against a codec-verified, identity-bound manifest."""

from __future__ import annotations

import math
import hashlib
from pathlib import Path


def require(condition, message):
    if not condition:
        raise ValueError(message)


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def fields(manifest):
    projection = manifest.get("body_paint_projection")
    result = (
        {}
        if projection is None
        else {
            "body_paint_contract": {
                "projection": projection,
                "world_preview": manifest["world_preview"],
            }
        }
    )
    surface = manifest.get("surface_paint")
    if surface is not None:
        result["surface_paint_contract"] = surface
    return result


def verifier_sha256():
    return hashlib.sha256(Path(__file__).read_bytes()).hexdigest()


def expected_texture(contract, part, handles, witness):
    """Return one exact image handle; legacy consumers keep their existing checks."""
    paint = contract.get("body_paint_contract")
    surface = contract.get("surface_paint_contract")
    if surface is not None:
        kind = contract["identity"]["kind"]
        names = {
            "MudMixer": ("body", "rotor"),
            "RestArea": ("body",),
            "SoulSpa": ("body", "slot0", "slot1", "slot2", "slot3"),
        }
        require(
            kind in names
            and part in names[kind]
            and surface == {"method": "uv_ink_v1"}
            and paint is None,
            "UV ink requires its closed kind/method/part contract",
        )
        require(
            isinstance(witness, dict)
            and witness.get("mode") == 4.0
            and witness.get("ink_base") is True,
            "actual UV ink material differs",
        )
        require(
            isinstance(handles.get("albedo"), str) and handles["albedo"],
            "UV ink atlas handle missing",
        )
        return handles["albedo"]
    if paint is None:
        if witness is not None:
            mode = witness.get("mode")
            expected = 0.82 if contract["identity"]["kind"] == "Tank" else 0.0
            require(
                finite(mode) and abs(mode - expected) < 1e-6,
                "undeclared projected body material",
            )
        return handles["albedo"]
    require(contract["identity"]["kind"] == "Tank", "body paint is Tank-only")
    require(isinstance(witness, dict), "actual body paint material facts missing")
    if part != "body":
        require(
            finite(witness.get("mode")) and abs(witness["mode"] - 0.82) < 1e-6,
            "water material must retain legacy shading",
        )
        return handles["albedo"]
    projection, preview = paint["projection"], paint["world_preview"]
    require(
        set(projection) == {"method", "matte_linear_rgb"}
        and projection["method"] == "world_preview_v1"
        and preview["image_role"] == "world_preview",
        "unknown body projection contract",
    )
    matte = projection["matte_linear_rgb"]
    require(
        isinstance(matte, list)
        and len(matte) == 3
        and all(finite(v) and 0 <= v <= 1 for v in matte),
        "invalid body matte",
    )
    canvas = preview["canvas_wu"] + [
        a / size
        for a, size in zip(preview["anchor_px"], preview["canvas_px"], strict=True)
    ]
    require(
        witness.get("standard") is True and witness.get("mode") == 3.0,
        "body paint requires exact mode3 and bounded standard material",
    )
    for key, expected in (("canvas", canvas), ("matte", matte + [1.0])):
        observed = witness.get(key)
        require(
            isinstance(observed, list)
            and len(observed) == 4
            and all(
                finite(a) and abs(a - b) < 1e-5
                for a, b in zip(observed, expected, strict=True)
            ),
            "actual body projection uniform differs: " + key,
        )
    require(
        isinstance(handles.get("world_preview"), str) and handles["world_preview"],
        "resident world-preview handle missing",
    )
    return handles["world_preview"]
