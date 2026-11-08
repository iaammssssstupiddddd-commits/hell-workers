"""Passive M5 gallery evidence, not art, preview/context or release authority."""
from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path

KINDS = frozenset(("WheelbarrowParking", "SandPile", "BonePile", "OutdoorLamp"))


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError("foreground " + message)


def canonical(value) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)


def verifier_sha256() -> str:
    return hashlib.sha256(Path(__file__).read_bytes()).hexdigest()


def descriptor(manifest: dict) -> dict:
    preview = manifest["world_preview"]
    roles = {"image:" + preview["image_role"]}
    if manifest["identity"]["kind"] == "OutdoorLamp":
        roles.add("image:world_on")
    artifacts = [entry for entry in manifest["artifacts"] if entry["role"] in roles]
    require(len(artifacts) == len(roles) and {entry["role"] for entry in artifacts} == roles,
            "requires unique exact world image artifacts")
    return {"schema": 1, "descriptor": preview, "artifacts": artifacts,
            "verifier_sha256": verifier_sha256()}


def settings(value: dict, *, context=False) -> dict | None:
    observation = value.get("foreground_observation")
    if observation is None:
        return None  # Historical gallery: never claim the new observations.
    lamp_ui = context and value.get("variant") == "lamp-supply-art-context"
    require(not lamp_ui or (value.get("kind") == "OutdoorLamp" and value.get("mode") == "feedback"),
            "Lamp UI requires its dedicated feedback kind")
    require(isinstance(observation, dict)
            and set(observation) == {"schema", "descriptor", "artifacts", "verifier_sha256"}
            and type(observation["schema"]) is int and observation["schema"] == 1
            and observation["verifier_sha256"] == verifier_sha256()
            and value.get("kind") in KINDS
            and value.get("variant", "gallery") == ("lamp-supply-art-context" if lamp_ui else "m5-art-context" if context else "gallery")
            and value.get("scope") == ("seeded-paused-lamp-supply-ui" if lamp_ui else "seeded-paused-art-comparison" if context else "paused-static-presentation")
            and value.get("promotion_authority") is False, "plan schema/kind/scope differs")
    return observation


def vector(value, length: int) -> list:
    require(isinstance(value, list) and len(value) == length
            and all(type(item) in (float, int) and math.isfinite(item) for item in value),
            "non-finite or malformed vector")
    return value


def matrix(pose: dict) -> list:
    require(isinstance(pose, dict) and set(pose) == {"translation", "rotation", "scale"}, "pose differs")
    translation = vector(pose["translation"], 3)
    x, y, z, w = vector(pose["rotation"], 4)
    scale = vector(pose["scale"], 3)
    require(abs(x*x + y*y + z*z + w*w - 1) <= .001, "rotation is not normalized")
    columns = ((1-2*(y*y+z*z), 2*(x*y+z*w), 2*(x*z-y*w)),
               (2*(x*y-z*w), 1-2*(x*x+z*z), 2*(y*z+x*w)),
               (2*(x*z+y*w), 2*(y*z-x*w), 1-2*(x*x+y*y)))
    return [number for index, column in enumerate(columns)
            for number in (*[component * scale[index] for component in column], 0.)] + [*translation, 1.]


def product(left: list, right: list) -> list:
    return [sum(left[k*4 + row] * right[column*4 + k] for k in range(4))
            for column in range(4) for row in range(4)]


def close(actual: list, expected: list) -> bool:
    vector(actual, len(expected))
    return all(abs(a-b) <= .001 for a, b in zip(actual, expected, strict=True))


def validate(probe: dict, value: dict, *, context=False) -> None:
    observation = settings(value, context=context)
    if observation is None:
        return
    fixture = probe["fixture"]
    require(canonical(probe.get("identity")) == canonical(value["identity"]), "root identity differs")
    require(type(fixture.get("foreground_schema")) is int and fixture["foreground_schema"] == 1,
            "runtime schema missing")
    records = [record for record in fixture.get("records", []) if record.get("kind") == value["kind"]]
    require(len(records) == 4 and all(type(record.get("ordinal")) is int for record in records)
            and sorted(record["ordinal"] for record in records) == list(range(4)),
            "owner inventory differs")
    preview = observation["descriptor"]
    artifacts = {artifact["role"]: artifact for artifact in observation["artifacts"]}
    identities = []
    for record in records:
        actual = record.get("foreground", {})
        require(type(actual.get("schema")) is int and actual["schema"] == 1
                and canonical(actual.get("identity")) == canonical(value["identity"])
                and canonical(actual.get("descriptor")) == canonical(preview),
                "consumed identity/descriptor differs")
        owner, entity = actual.get("owner"), actual.get("entity")
        identities.extend((owner, entity))
        require(type(owner) is int and owner > 0 and type(entity) is int and entity > 0
                and type(record.get("owner")) is int and owner == record["owner"]
                and type(actual.get("parent")) is int and actual["parent"] == owner,
                "parent/owner differs")
        role = "world_on" if value["kind"] == "OutdoorLamp" and record["ordinal"] >= 2 else preview["image_role"]
        require(actual.get("image_role") == role
                and canonical(actual.get("artifact")) == canonical(artifacts.get("image:" + role))
                and actual.get("artifact") is not None and actual.get("image_matches") is True
                and isinstance(actual.get("image_handle"), str) and actual["image_handle"],
                "actual image role/handle differs")
        require(canonical(actual.get("canvas_wu")) == canonical(preview["canvas_wu"])
                and close(actual.get("anchor"), [preview["anchor_px"][0] / preview["canvas_px"][0] - .5,
                                                  .5 - preview["anchor_px"][1] / preview["canvas_px"][1]])
                and actual.get("visible") is True and isinstance(actual.get("layers"), list)
                and all(type(layer) is int for layer in actual["layers"]) and actual["layers"] == [0],
                "canvas/anchor/visibility/layers differs")
        # A finite non-white color is intentionally retained, never accepted as white.
        color = vector(actual.get("color_srgba"), 4)
        require(all(0 <= channel <= 1 for channel in color), "color is outside sRGBA range")
        owner_matrix = matrix(actual.get("owner_pose"))
        local_matrix = matrix(actual.get("local_pose"))
        expected = product(owner_matrix, local_matrix)
        center = vector(record.get("center"), 2)
        require(close(actual["owner_pose"]["translation"][:2], center)
                and close(actual.get("global_matrix"), expected)
                and close(actual.get("parent_local_matrix"), expected)
                and close(actual["global_matrix"], matrix(actual.get("global_pose"))),
                "owner/local/global transform differs")
    require(len(set(identities)) == len(identities), "duplicate owner or sprite")
