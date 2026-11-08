"""Read-only exact M5 numeric decision checks. Never generate approval or art."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

from building_m3_numeric import canonical, require, unique_object

CONTRACT = Path(__file__).resolve().parents[1] / "fixtures/building-m5-v1.contract.json"
KINDS = frozenset(("WheelbarrowParking", "SandPile", "BonePile", "OutdoorLamp"))


def numeric_contract(payload: bytes, kind: str) -> dict:
    require(kind in KINDS, "numeric geometry kind is not M5")
    geometry = json.loads(payload, object_pairs_hook=unique_object)
    # Version 1 is a fixed proposal, not permission to choose new numeric values.
    require(canonical(geometry) == canonical(json.loads(CONTRACT.read_bytes())),
            "M5 geometry proposal differs")
    spec = geometry["kinds"][kind]
    return {"kind": kind, "canvas_px": geometry["canvas_px"], "anchor_px": geometry["anchor_px"],
            "catalog_margin_px": geometry["catalog_margin_px"], "world_roles": spec["world_roles"],
            "canvas_wu": spec["canvas_wu"], "representative_state": spec["representative_state"]}


def validate(approval: dict, preview: dict, art_digest: str, geometry: bytes, manifest: dict) -> None:
    require(isinstance(approval, dict) and set(approval) == {
        "schema_version", "decision", "art_preview_identity", "art_approval_sha256",
        "geometry_contract_sha256", "numeric_contract"}, "M5 numeric decision fields differ")
    require(type(approval["schema_version"]) is int and approval["schema_version"] == 1
            and approval["decision"] == "building_m5_numeric_approved"
            and preview.get("kind") in KINDS and preview.get("authority") == "art_preview"
            and canonical(approval["art_preview_identity"]) == canonical(preview)
            and approval["art_approval_sha256"] == art_digest
            and approval["geometry_contract_sha256"] == hashlib.sha256(geometry).hexdigest(),
            "M5 numeric preview/art/geometry binding differs")
    numeric = numeric_contract(geometry, preview["kind"])
    require(canonical(approval["numeric_contract"]) == canonical(numeric),
            "M5 approved numeric values differ from geometry original")
    common = {key: numeric[key] for key in ("canvas_px", "canvas_wu", "anchor_px", "representative_state")}
    # Runtime codec uses u32 pixels and f32 world/anchor coordinates. Compare its
    # canonical representation, not authoring JSON's incidental int spelling.
    for key in ("canvas_wu", "anchor_px"):
        common[key] = [float(value) for value in common[key]]
    require(canonical(manifest["world_preview"]) == canonical({"image_role": numeric["world_roles"][0], **common})
            and canonical(manifest["catalog_preview"]) == canonical({"image_role": "catalog", **common})
            and manifest["parts"] == [] and manifest.get("production_state") is None,
            "M5 manifest geometry differs from numeric decision")
    require([entry["role"] for entry in manifest["artifacts"]]
            == ["image:" + role for role in numeric["world_roles"] + ["catalog"]],
            "M5 numeric artifact roles differ")
