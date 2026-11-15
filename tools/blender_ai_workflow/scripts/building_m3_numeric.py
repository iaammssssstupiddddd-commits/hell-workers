"""Read-only M3 numeric decision validation. Never issue approval or choose values."""
from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path

CONTRACT = Path(__file__).resolve().parents[1] / "fixtures/building-m3-v1.geometry.json"
KINDS = {"RestArea", "SoulSpa"}
COMMON = ("units", "geometry_scale", "tolerance_wu", "ordered_relative_tiles",
          "center_offset_tiles", "slot_emissive_multiplier_linear", "atlas")


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise ValueError(reason)


def canonical(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":"),
                       ensure_ascii=False, allow_nan=False) + "\n").encode()


def unique_object(pairs: list) -> dict:
    result = {}
    for key, value in pairs:
        require(key not in result, "duplicate geometry field")
        result[key] = value
    return result


def shape(value: object, template: object) -> None:
    """Versioned proposal shape, not an automatic numeric/art acceptance rule."""
    if type(template) in (int, float):
        require(type(value) in (int, float) and math.isfinite(value),
                "M3 geometry requires finite numbers, not booleans")
    elif isinstance(template, dict):
        require(isinstance(value, dict) and set(value) == set(template), "M3 geometry fields differ")
        for key, expected in template.items():
            shape(value[key], expected)
    elif isinstance(template, list):
        require(isinstance(value, list) and len(value) == len(template), "M3 geometry shape differs")
        for item, expected in zip(value, template, strict=True):
            shape(item, expected)
    else:
        require(type(value) is type(template) and value == template, "M3 geometry metadata differs")


def numeric_contract(payload: bytes, kind: str) -> dict:
    require(kind in KINDS, "numeric geometry kind is not M3")
    geometry = json.loads(payload, object_pairs_hook=unique_object)
    revision = geometry.get("revision", 1)
    require(type(revision) is int and revision in (1, 2, 3, 4, 5, 6, 7), "unsupported M3 revision")
    require(revision not in (3, 4, 5, 6, 7) or kind == "RestArea", "M3 v3 is RestArea-only")
    template_path = CONTRACT if revision == 1 else CONTRACT.with_name(f"building-m3-v{revision}.geometry.json")
    template = json.loads(template_path.read_bytes())
    shape(geometry, template)
    require(type(geometry["schema_version"]) is int and geometry["schema_version"] == 1,
            "unsupported M3 geometry schema")
    # The proposal remains unapproved. This projection omits palette/rejection
    # prose: separate exact-preview art approval owns those decisions.
    spec = {key: value for key, value in geometry[kind].items() if key not in {"palette", "reject_if"}}
    return {**{key: geometry[key] for key in COMMON}, "kind": kind, "geometry": spec}


def validate(approval: dict, preview: dict, art_digest: str, geometry: bytes) -> None:
    require(isinstance(approval, dict) and set(approval) == {
        "schema_version", "decision", "art_preview_identity", "art_approval_sha256",
        "geometry_contract_sha256", "numeric_contract"}, "M3 numeric decision fields differ")
    require(type(approval["schema_version"]) is int and approval["schema_version"] == 1
            and approval["decision"] == "building_m3_numeric_approved"
            and preview.get("kind") in KINDS and preview.get("authority") == "art_preview"
            and canonical(approval["art_preview_identity"]) == canonical(preview)
            and approval["art_approval_sha256"] == art_digest
            and approval["geometry_contract_sha256"] == hashlib.sha256(geometry).hexdigest(),
            "M3 numeric preview/art/geometry binding differs")
    require(canonical(approval["numeric_contract"]) == canonical(numeric_contract(geometry, preview["kind"])),
            "M3 approved numeric values differ from geometry original")
