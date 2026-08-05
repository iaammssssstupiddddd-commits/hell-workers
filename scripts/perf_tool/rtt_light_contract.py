"""Canonical contract loading and pure fixture validation for RtT-light migration."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any


CONTRACT_DIRECTORY = Path(__file__).resolve().parent / "contracts"
CONTRACT_FILES = {
    "rtt-light-v1": CONTRACT_DIRECTORY / "rtt_light_migration_v1.json",
}
EXPECTED_CONTRACT_SHA256 = {
    "rtt-light-v1": "121a365ac3349cd4fa7890ab3069f0392098ced17e0d47f920095a1490c2ba11",
}
RTT_LIGHT_STAGES = ("current", "p01", "p02", "p03", "p04", "p05", "p06", "p07", "p08")
RTT_LIGHT_LANES = ("static", "behavior", "field-core", "consumer-core")
FIELD_CORE_STAGES = frozenset({"p03", "p04", "p05", "p06", "p07", "p08"})
CONSUMER_CORE_STAGES = frozenset({"p07", "p08"})
GATE_UNIT_TYPES = {
    "boolean": "boolean",
    "digest": "sha256",
    "bytes": "u64",
    "bytes_delta": "i64",
    "cells": "u64",
    "count": "u64",
    "effects": "u64",
    "entities": "u64",
    "epochs": "u64",
    "errors": "u64",
    "events": "u64",
    "frames": "u64",
    "handles": "u64",
    "images": "u64",
    "KiB": "u64",
    "lines": "u64",
    "ms": "f64",
    "passes": "u64",
    "percent": "f64",
    "ratio": "f64",
    "reads": "u64",
    "revisions": "u64",
    "runs": "u64",
    "samples": "u64",
    "signatures": "u64",
    "tiles": "u64",
    "transitions": "u64",
    "uploads": "u64",
    "wakes": "u64",
    "bindings": "u64",
    "bindings-per-pipeline": "u64",
    "calls": "u64",
}
EXPECTED_GATE_REFERENCE_LINEAGE: dict[tuple[str, str], object] = {
    ("RLV1-P01-PERF", "wall_frame_p95_relative_pct"): "current",
    ("RLV1-P01-PERF", "wall_frame_p99_relative_pct"): "current",
    ("RLV1-P01-PERF", "max_rss_relative_pct"): "current",
    ("RLV1-P01-PERF", "large_peak_live_delta_bytes"): "current",
    ("RLV1-P02-PERF", "wall_frame_p95_relative_pct"): "p01",
    ("RLV1-P02-PERF", "wall_frame_p99_relative_pct"): "p01",
    ("RLV1-P06-PERF", "wall_frame_p95_relative_pct"): "p02",
    ("RLV1-P06-PERF", "wall_frame_p99_relative_pct"): "p02",
    ("RLV1-P06-PERF", "max_rss_relative_pct"): "p02",
    ("RLV1-P06-PERF", "large_peak_live_delta_bytes"): "p02",
    ("RLV1-P08-FRAME", "wall_frame_p95_relative_pct"): "current",
    ("RLV1-P08-FRAME", "wall_frame_p99_relative_pct"): "current",
    ("RLV1-P08-MEMORY", "max_rss_relative_pct"): "current",
    (
        "RLV1-P08-MEMORY",
        "large_peak_live_delta_bytes",
    ): {"cpu": "p07", "gpu": "p06"},
}
SHOWCASE_REUSED_KINDS = frozenset(
    {"Wall", "Door", "Floor", "SoulSpa", "OutdoorLamp"}
)
SHOWCASE_EXTRA_KINDS = frozenset(
    {
        "Tank",
        "MudMixer",
        "RestArea",
        "Bridge",
        "SandPile",
        "BonePile",
        "WheelbarrowParking",
    }
)


def canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def canonical_sha256(value: Any) -> str:
    return hashlib.sha256(canonical_json_bytes(value)).hexdigest()


def load_rtt_light_contract(contract_id: str) -> dict[str, Any]:
    path = CONTRACT_FILES.get(contract_id)
    if path is None:
        raise ValueError(f"unsupported RtT-light contract: {contract_id}")
    try:
        contract = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot load RtT-light contract {contract_id}: {error}") from error
    validate_rtt_light_contract(contract)
    return contract


def validate_stage_lane(contract: dict[str, Any], stage: str, lane: str) -> None:
    if stage not in RTT_LIGHT_STAGES:
        raise ValueError(f"unsupported RtT-light stage: {stage}")
    if lane not in RTT_LIGHT_LANES:
        raise ValueError(f"unsupported RtT-light lane: {lane}")
    if lane == "field-core" and stage not in FIELD_CORE_STAGES:
        raise ValueError(f"field-core is not applicable at stage {stage}")
    if lane == "consumer-core" and stage not in CONSUMER_CORE_STAGES:
        raise ValueError(f"consumer-core is not applicable at stage {stage}")
    stage_contract = contract["stages"].get(stage)
    if not isinstance(stage_contract, dict):
        raise ValueError(f"contract has no stage definition for {stage}")
    required_lanes = stage_contract.get("required_lanes", [])
    if lane not in required_lanes:
        raise ValueError(f"lane {lane} is not required at stage {stage}")


def _room_floor_cells(origin: tuple[int, int], module_count: int) -> list[tuple[int, int]]:
    cells: list[tuple[int, int]] = []
    for room_y in range(module_count):
        for room_x in range(module_count):
            base_x = origin[0] + room_x * 7
            base_y = origin[1] + room_y * 7
            for local_y in range(1, 7):
                for local_x in range(1, 7):
                    cells.append((base_x + local_x, base_y + local_y))
    return cells


def _boundary_cells(origin: tuple[int, int], module_count: int) -> set[tuple[int, int]]:
    extent = module_count * 7
    cells: set[tuple[int, int]] = set()
    for boundary_index in range(module_count + 1):
        x = origin[0] + boundary_index * 7
        y = origin[1] + boundary_index * 7
        cells.update((x, origin[1] + offset) for offset in range(extent + 1))
        cells.update((origin[0] + offset, y) for offset in range(extent + 1))
    return cells


def _door_cells(origin: tuple[int, int], module_count: int) -> list[tuple[int, int]]:
    return [
        (origin[0] + room_x * 7 + 3, origin[1] + (room_y + 1) * 7)
        for room_y in range(module_count)
        for room_x in range(module_count)
    ]


def _room_boundary_lookup_cells(module_count: int) -> int:
    # RoomBoundaryLookup only contains wall/door contacts adjacent to an
    # interior floor. Grid-line corners have no adjacent floor and are absent.
    return 12 * module_count * (module_count + 1)


def _showcase_counts(size_contract: dict[str, Any]) -> dict[str, int]:
    showcase = size_contract["showcase_buildings"]
    companions = [
        entry["companion"] for entry in showcase if "companion" in entry
    ]
    return {
        "showcase_buildings": len(showcase),
        "showcase_extra_buildings": sum(
            entry["source"] == "dedicated-showcase" for entry in showcase
        ),
        "showcase_footprint_cells": sum(
            len(entry["occupied_grids"]) for entry in showcase
        ),
        "showcase_companions": len(companions),
    }


def build_fixture_layout(contract: dict[str, Any], size: str) -> dict[str, Any]:
    fixture = contract["fixture"]
    size_contract = fixture["sizes"].get(size)
    if not isinstance(size_contract, dict):
        raise ValueError(f"contract has no fixture size {size}")
    origin = tuple(fixture["origin"])
    module_count = size_contract["module_count"]
    floor_cells = _room_floor_cells(origin, module_count)
    boundary_cells = _boundary_cells(origin, module_count)
    door_cells = _door_cells(origin, module_count)
    door_states = size_contract["door_states"]
    doors = [
        {"grid": list(grid), "state": state}
        for grid, state in zip(door_cells, door_states, strict=True)
    ]
    wall_cells = sorted(boundary_cells - set(door_cells))
    supplied_count = size_contract["supplied_lamp_candidates"]
    supplied_lamps = floor_cells[:supplied_count]
    actor_rule = fixture["actor_enumeration"]
    soul_offset = actor_rule["soul_floor_offset"]
    familiar_offset = actor_rule["familiar_floor_offset"]
    souls = [
        floor_cells[(soul_offset + index) % len(floor_cells)]
        for index in range(size_contract["souls"])
    ]
    familiars = [
        floor_cells[(familiar_offset + index) % len(floor_cells)]
        for index in range(size_contract["familiars"])
    ]
    semantic_layout = {
        "fixture_id": fixture["fixture_id"],
        "size": size,
        "origin": list(origin),
        "module_count": module_count,
        "floor_cells": [list(grid) for grid in floor_cells],
        "wall_cells": [list(grid) for grid in wall_cells],
        "doors": doors,
        "supplied_lamp_cells": [list(grid) for grid in supplied_lamps],
        "unsupplied_lamp_cell": size_contract["unsupplied_lamp_cell"],
        "main_yard_grid_bounds": size_contract["main_yard_grid_bounds"],
        "control_yard_grid_bounds": size_contract["control_yard_grid_bounds"],
        "soul_cells": [list(grid) for grid in souls],
        "familiar_cells": [list(grid) for grid in familiars],
        "soul_spas": size_contract["soul_spas"],
        "energy": {
            "generation": size_contract["generation"],
            "active_lamp_demand": size_contract["active_lamp_demand"],
            "headroom": size_contract["headroom"],
        },
        "all_building_showcase": bool(size_contract["showcase_buildings"]),
    }
    if size_contract["showcase_buildings"]:
        semantic_layout["showcase_buildings"] = size_contract["showcase_buildings"]
    return semantic_layout | {
        "layout_checksum": canonical_sha256(semantic_layout),
        "counts": {
            "rooms": module_count * module_count,
            "completed_floors": len(floor_cells),
            "completed_walls": len(wall_cells),
            "doors": len(doors),
            "supplied_lamp_candidates": len(supplied_lamps),
            "unsupplied_lamp_candidates": 1,
            "souls": len(souls),
            "familiars": len(familiars),
            "room_boundary_lookup_cells": _room_boundary_lookup_cells(module_count),
            "yards": 2,
            "operational_soul_spas": len(size_contract["soul_spas"]),
            "generator_souls": sum(
                len(spa["worker_ordinals"]) for spa in size_contract["soul_spas"]
            ),
            **_showcase_counts(size_contract),
        },
    }


def build_fixture_ledger(contract: dict[str, Any], size: str) -> list[dict[str, str]]:
    """Return the exact, ordered semantic rows emitted by the Rust fixture sidecar."""
    layout = build_fixture_layout(contract, size)
    rows: list[dict[str, str]] = []

    def append(
        record_kind: str,
        ordinal: int,
        grid: list[int] | tuple[int, int],
        *,
        state: str,
        relation: str = "",
        grid2: list[int] | tuple[int, int] | None = None,
    ) -> None:
        rows.append(
            {
                "schema_version": "1",
                "record_kind": record_kind,
                "ordinal": str(ordinal),
                "grid_x": str(grid[0]),
                "grid_y": str(grid[1]),
                "grid_x2": "" if grid2 is None else str(grid2[0]),
                "grid_y2": "" if grid2 is None else str(grid2[1]),
                "state": state,
                "relation": relation,
            }
        )

    for ordinal, grid in enumerate(layout["floor_cells"]):
        append("floor", ordinal, grid, state="completed")
    for ordinal, grid in enumerate(layout["wall_cells"]):
        append("wall", ordinal, grid, state="completed")
    for ordinal, door in enumerate(layout["doors"]):
        append("door", ordinal, door["grid"], state=door["state"])
    for ordinal, grid in enumerate(layout["supplied_lamp_cells"]):
        append("supplied_lamp", ordinal, grid, state="supplied", relation="main-yard")
    append(
        "unsupplied_lamp",
        0,
        layout["unsupplied_lamp_cell"],
        state="shed-insufficient-generation",
        relation="control-yard",
    )

    showcase_footprint_ordinal = 0
    showcase_companion_ordinal = 0
    showcase_companion_footprint_ordinal = 0
    for showcase_ordinal, entry in enumerate(layout.get("showcase_buildings", [])):
        append(
            "showcase_building",
            showcase_ordinal,
            entry["anchor"],
            state=entry["kind"],
            relation=f"{entry['source']}:{entry['production_route']}",
        )
        for grid in entry["occupied_grids"]:
            append(
                "showcase_footprint",
                showcase_footprint_ordinal,
                grid,
                state=entry["kind"],
                relation=f"showcase-building-{showcase_ordinal}",
            )
            showcase_footprint_ordinal += 1
        companion = entry.get("companion")
        if companion is None:
            continue
        append(
            "showcase_companion",
            showcase_companion_ordinal,
            companion["anchor"],
            state=companion["kind"],
            relation=f"showcase-building-{showcase_ordinal}:{companion['production_route']}",
        )
        for grid in companion["occupied_grids"]:
            append(
                "showcase_companion_footprint",
                showcase_companion_footprint_ordinal,
                grid,
                state=companion["kind"],
                relation=f"showcase-companion-{showcase_companion_ordinal}",
            )
            showcase_companion_footprint_ordinal += 1
        showcase_companion_ordinal += 1

    worker_relations: dict[int, str] = {}
    for spa_ordinal, spa in enumerate(layout["soul_spas"]):
        for worker_ordinal, tile_ordinal in zip(
            spa["worker_ordinals"],
            (spa["tiles"].index(tile) for tile in spa["worker_tiles"]),
            strict=True,
        ):
            worker_relations[worker_ordinal] = f"soul-spa-{spa_ordinal}-tile-{tile_ordinal}"
    for ordinal, grid in enumerate(layout["soul_cells"]):
        relation = worker_relations.get(ordinal, "")
        append(
            "soul",
            ordinal,
            grid,
            state="generator" if relation else "idle",
            relation=relation,
        )
    for ordinal, grid in enumerate(layout["familiar_cells"]):
        append("familiar", ordinal, grid, state="idle")

    for ordinal, (name, bounds) in enumerate(
        (
            ("main", layout["main_yard_grid_bounds"]),
            ("control", layout["control_yard_grid_bounds"]),
        )
    ):
        append(
            "yard",
            ordinal,
            bounds["min"],
            grid2=bounds["max"],
            state=name,
        )
    for spa_ordinal, spa in enumerate(layout["soul_spas"]):
        append(
            "soul_spa",
            spa_ordinal,
            spa["anchor"],
            state="operational",
            relation="main-yard",
        )
        workers_by_tile = {
            spa["tiles"].index(tile): worker
            for worker, tile in zip(
                spa["worker_ordinals"], spa["worker_tiles"], strict=True
            )
        }
        for tile_ordinal, grid in enumerate(spa["tiles"]):
            worker = workers_by_tile.get(tile_ordinal)
            append(
                "soul_spa_tile",
                spa_ordinal * 4 + tile_ordinal,
                grid,
                state="generate-power",
                relation=(
                    f"soul-{worker}" if worker is not None else f"soul-spa-{spa_ordinal}"
                ),
            )

    floor_to_room = {
        tuple(grid): ordinal // 36 for ordinal, grid in enumerate(layout["floor_cells"])
    }
    for ordinal, grid in enumerate(layout["floor_cells"]):
        append(
            "room_tile",
            ordinal,
            grid,
            state="interior",
            relation=f"room-{floor_to_room[tuple(grid)]}",
        )
    door_cells = {tuple(door["grid"]) for door in layout["doors"]}
    boundary_cells = sorted(
        grid
        for grid in ({tuple(cell) for cell in layout["wall_cells"]} | door_cells)
        if any(
            (grid[0] + dx, grid[1] + dy) in floor_to_room
            for dx, dy in ((0, 1), (1, 0), (0, -1), (-1, 0))
        )
    )
    for ordinal, grid in enumerate(boundary_cells):
        room_ordinals = sorted(
            {
                floor_to_room[(grid[0] + dx, grid[1] + dy)]
                for dx, dy in ((0, 1), (1, 0), (0, -1), (-1, 0))
                if (grid[0] + dx, grid[1] + dy) in floor_to_room
            }
        )
        append(
            "room_boundary",
            ordinal,
            grid,
            state="door" if grid in door_cells else "wall",
            relation="+".join(f"room-{room}" for room in room_ordinals),
        )
    return rows


def build_fixture_presentation_rows(
    contract: dict[str, Any], size: str
) -> list[dict[str, str]]:
    layout = build_fixture_layout(contract, size)
    counts = layout["counts"]
    entity_counts = {
        "Floor": counts["completed_floors"],
        "Wall": counts["completed_walls"],
        "Door": counts["doors"],
        "OutdoorLamp": (
            counts["supplied_lamp_candidates"]
            + counts["unsupplied_lamp_candidates"]
        ),
        "SoulSpa": counts["operational_soul_spas"],
    }
    for entry in layout.get("showcase_buildings", []):
        if entry["source"] == "dedicated-showcase":
            entity_counts[entry["kind"]] = entity_counts.get(entry["kind"], 0) + 1
    rows = []
    presentation_order = (
        contract["fixture"]["building_types"]
        if layout["all_building_showcase"]
        else ("Floor", "Wall", "Door", "OutdoorLamp", "SoulSpa")
    )
    for building_kind in presentation_order:
        if building_kind not in entity_counts:
            continue
        entity_count = entity_counts[building_kind]
        expectation = contract["fixture"]["current_presentation"][building_kind]
        rows.append(
            {
                "schema_version": "1",
                "building_kind": building_kind,
                "entity_count": str(entity_count),
                "root_sprite_count": str(
                    entity_count * expectation["root_sprite_per_entity"]
                ),
                "child_sprite_count": str(
                    entity_count * expectation["child_sprite_per_entity"]
                ),
                "owner_3d_count": str(
                    entity_count * expectation["owner_3d_per_entity"]
                ),
            }
        )
    return rows


def build_fixture_audit_actor_counts(
    contract: dict[str, Any], size: str
) -> dict[str, int]:
    counts = build_fixture_layout(contract, size)["counts"]
    return {
        "indoor-building": (
            counts["completed_floors"]
            + counts["completed_walls"]
            + counts["doors"]
            + counts["supplied_lamp_candidates"]
            + counts["unsupplied_lamp_candidates"]
            + counts["operational_soul_spas"]
            + counts["showcase_extra_buildings"]
        ),
        "indoor-grid": 2,
        "indoor-room": counts["rooms"],
        "indoor-room-lookup": 1,
        "indoor-soul-spa": counts["operational_soul_spas"],
        "indoor-soul-spa-tile": counts["operational_soul_spas"] * 4,
        "indoor-yard": 2,
    }


def _expected_showcase_footprint(
    kind: str, anchor: tuple[int, int]
) -> list[tuple[int, int]]:
    x, y = anchor
    if kind in {"Tank", "MudMixer", "RestArea", "WheelbarrowParking"}:
        return [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)]
    if kind == "SoulSpa":
        return [(x, y), (x + 1, y), (x, y - 1), (x + 1, y - 1)]
    if kind == "Bridge":
        return [(x + dx, 65 + dy) for dy in range(5) for dx in range(2)]
    return [anchor]


def _validate_showcase_contract(
    fixture: dict[str, Any],
    size: str,
    size_contract: dict[str, Any],
    floor_cells: list[tuple[int, int]],
    wall_cells: list[tuple[int, int]],
    door_cells: list[tuple[int, int]],
    supplied_lamps: set[tuple[int, int]],
    spa_tiles: set[tuple[int, int]],
) -> None:
    entries = size_contract.get("showcase_buildings")
    if not isinstance(entries, list):
        raise ValueError(f"{size} showcase_buildings must be a list")
    if size == "small":
        if entries:
            raise ValueError("small must not instantiate the all-building showcase")
        return

    inventory = fixture["building_types"]
    if [entry.get("kind") for entry in entries] != inventory:
        raise ValueError(f"{size} showcase must enumerate BuildingType order exactly")

    expected_sources = {
        "Wall": ("canonical-wall", wall_cells),
        "Door": ("canonical-door", door_cells),
        "Floor": ("canonical-floor", floor_cells),
        "SoulSpa": (
            "canonical-soul-spa",
            [tuple(spa["anchor"]) for spa in size_contract["soul_spas"]],
        ),
        "OutdoorLamp": ("canonical-supplied-lamp", sorted(supplied_lamps)),
    }
    dedicated_cells: set[tuple[int, int]] = set()
    companion_cells: set[tuple[int, int]] = set()
    seen_extra_kinds: set[str] = set()
    companion_count = 0

    for entry in entries:
        kind = entry["kind"]
        source = entry.get("source")
        anchor_value = entry.get("anchor")
        footprint_value = entry.get("occupied_grids")
        if (
            not isinstance(anchor_value, list)
            or len(anchor_value) != 2
            or not all(isinstance(value, int) for value in anchor_value)
        ):
            raise ValueError(f"{size} showcase {kind} anchor is invalid")
        if not isinstance(footprint_value, list):
            raise ValueError(f"{size} showcase {kind} footprint is invalid")
        anchor = tuple(anchor_value)
        footprint = [tuple(grid) for grid in footprint_value]
        if footprint != _expected_showcase_footprint(kind, anchor):
            raise ValueError(f"{size} showcase {kind} differs from production geometry")
        if any(
            len(grid) != 2
            or not all(isinstance(value, int) for value in grid)
            or not (0 <= grid[0] < 100 and 0 <= grid[1] < 100)
            for grid in footprint_value
        ):
            raise ValueError(f"{size} showcase {kind} has an out-of-bounds footprint")

        if kind in SHOWCASE_REUSED_KINDS:
            expected_source, candidates = expected_sources[kind]
            expected_route = {
                "Wall": "area-wall-completion",
                "Door": "completed-blueprint",
                "Floor": "area-floor-completion",
                "SoulSpa": "soul-spa-placement",
                "OutdoorLamp": "completed-blueprint",
            }[kind]
            ordinal = entry.get("source_ordinal")
            if (
                source != expected_source
                or entry.get("production_route") != expected_route
                or not isinstance(ordinal, int)
                or ordinal < 0
                or ordinal >= len(candidates)
                or anchor != candidates[ordinal]
            ):
                raise ValueError(f"{size} showcase {kind} source reference is invalid")
        else:
            if source != "dedicated-showcase" or "source_ordinal" in entry:
                raise ValueError(f"{size} showcase {kind} must be a dedicated root")
            if kind not in SHOWCASE_EXTRA_KINDS or kind in seen_extra_kinds:
                raise ValueError(f"{size} showcase extra kind is missing or duplicated")
            expected_route = (
                "fixture-seeded-completed-blueprint"
                if kind == "Bridge"
                else "completed-blueprint"
            )
            if entry.get("production_route") != expected_route:
                raise ValueError(f"{size} showcase {kind} production route is invalid")
            if kind != "Bridge" and not set(footprint).issubset(floor_cells):
                raise ValueError(f"{size} showcase {kind} must preserve completed Floor cells")
            blocked = supplied_lamps | spa_tiles | dedicated_cells | companion_cells
            if set(footprint) & blocked:
                raise ValueError(f"{size} showcase {kind} footprint overlaps another fixture")
            dedicated_cells.update(footprint)
            seen_extra_kinds.add(kind)

        companion = entry.get("companion")
        if companion is None:
            continue
        companion_count += 1
        if kind != "Tank" or companion_count != 1:
            raise ValueError(f"{size} only Tank may own one showcase companion")
        companion_anchor = tuple(companion.get("anchor", []))
        companion_footprint = [
            tuple(grid) for grid in companion.get("occupied_grids", [])
        ]
        if (
            companion.get("kind") != "BucketStorage"
            or companion.get("production_route") != "tank-companion-placement"
            or companion_footprint
            != [companion_anchor, (companion_anchor[0] + 1, companion_anchor[1])]
            or not set(companion_footprint).issubset(floor_cells)
            or set(companion_footprint)
            & (supplied_lamps | spa_tiles | dedicated_cells | companion_cells)
        ):
            raise ValueError(f"{size} Tank companion geometry is invalid")
        companion_cells.update(companion_footprint)

    if seen_extra_kinds != SHOWCASE_EXTRA_KINDS or companion_count != 1:
        raise ValueError(f"{size} showcase does not cover the exact extra-building matrix")
    if len(dedicated_cells) != 28 or len(companion_cells) != 2:
        raise ValueError(f"{size} showcase dedicated footprint counts differ")


def _stage_index(stage: str) -> int:
    try:
        return RTT_LIGHT_STAGES.index(stage)
    except ValueError as error:
        raise ValueError(f"unsupported RtT-light stage: {stage}") from error


def expected_formal_cases(
    contract: dict[str, Any], stage: str
) -> list[dict[str, Any]]:
    """Expand the exact formal leg/case matrix for one acceptance stage."""
    _stage_index(stage)
    stage_contract = contract.get("stages", {}).get(stage)
    if not isinstance(stage_contract, dict):
        raise ValueError(f"contract has no stage definition for {stage}")
    cases: list[dict[str, Any]] = []
    for leg in contract.get("formal_legs", []):
        if not isinstance(leg, dict):
            raise ValueError("formal_legs entries must be objects")
        first_stage = leg.get("first_required_stage")
        if not isinstance(first_stage, str) or _stage_index(stage) < _stage_index(first_stage):
            continue
        leg_id = leg.get("leg_id")
        lane = leg.get("lane")
        repeat = leg.get("repeat")
        environment = leg.get("environment")
        if leg_id == "behavior":
            for behavior_case in stage_contract.get("required_behavior_cases", []):
                cases.append(
                    {
                        "leg_id": leg_id,
                        "lane": lane,
                        "case_id": f"behavior-{behavior_case}",
                        "size": "small",
                        "render": "cpu",
                        "repeat": repeat,
                        "environment": environment,
                    }
                )
            continue
        for size in leg.get("sizes", []):
            for render in leg.get("renders", []):
                cases.append(
                    {
                        "leg_id": leg_id,
                        "lane": lane,
                        "case_id": f"{leg_id}-{size}-{render}",
                        "size": size,
                        "render": render,
                        "repeat": repeat,
                        "environment": environment,
                    }
                )
    keys = [(case["leg_id"], case["case_id"]) for case in cases]
    if len(keys) != len(set(keys)):
        raise ValueError(f"stage {stage} formal matrix contains duplicate cases")
    return cases


def projection_field_applicability(
    contract: dict[str, Any],
    stage: str,
    leg_id: str,
    case_id: str | None = None,
) -> dict[str, str]:
    """Return the exact availability token for every projection field group."""
    _stage_index(stage)
    groups = contract.get("projection", {}).get("field_groups")
    if not isinstance(groups, dict) or not groups:
        raise ValueError("projection field_groups must be a nonempty object")
    formal_leg = next(
        (
            leg
            for leg in contract.get("formal_legs", [])
            if isinstance(leg, dict) and leg.get("leg_id") == leg_id
        ),
        None,
    )
    if not isinstance(formal_leg, dict):
        raise ValueError(f"unknown formal leg {leg_id}")
    formal_case = None
    if case_id is not None:
        formal_case = next(
            (
                case
                for case in expected_formal_cases(contract, stage)
                if case["leg_id"] == leg_id and case["case_id"] == case_id
            ),
            None,
        )
        if formal_case is None:
            raise ValueError(f"unknown formal case {case_id} for leg {leg_id} / {stage}")
    leg_renders = formal_leg.get("renders", [])
    case_render = (
        formal_case["render"]
        if formal_case is not None
        else leg_renders[0]
        if isinstance(leg_renders, list) and len(leg_renders) == 1
        else None
    )
    availability: dict[str, str] = {}
    for name, group in groups.items():
        if not isinstance(group, dict):
            raise ValueError(f"projection field group {name} must be an object")
        required_stages = group.get("required_stages")
        required_legs = group.get("required_legs")
        if not isinstance(required_stages, list) or not isinstance(required_legs, list):
            raise ValueError(f"projection field group {name} has no exact applicability")
        required_renders = group.get("required_renders")
        if required_renders is not None and (
            not isinstance(required_renders, list) or not required_renders
        ):
            raise ValueError(
                f"projection field group {name} has invalid render applicability"
            )
        if (
            stage in required_stages
            and leg_id in required_legs
            and (required_renders is None or case_render in required_renders)
        ):
            availability[name] = "available"
        elif stage not in required_stages:
            reason = group.get("stage_not_applicable_reasons", {}).get(stage)
            if not isinstance(reason, str):
                reason = group.get("stage_not_applicable_reason")
            if not isinstance(reason, str):
                reason = group.get("not_applicable_reason")
            if not isinstance(reason, str):
                raise ValueError(
                    f"projection field group {name} has no stage not-applicable reason"
                )
            availability[name] = reason
        elif leg_id not in required_legs:
            reason = group.get("leg_not_applicable_reason")
            if not isinstance(reason, str):
                reason = group.get("not_applicable_reason")
            if not isinstance(reason, str):
                raise ValueError(
                    f"projection field group {name} has no leg not-applicable reason"
                )
            availability[name] = reason
        else:
            if case_render is None:
                raise ValueError(
                    f"projection field group {name} needs a case_id to resolve render applicability"
                )
            reason = group.get("render_not_applicable_reason")
            if not isinstance(reason, str):
                raise ValueError(
                    f"projection field group {name} has no render not-applicable reason"
                )
            availability[name] = reason
    return availability


def expected_projection_keys(
    contract: dict[str, Any], stage: str
) -> list[tuple[str, str, str, str]]:
    return [
        (stage, case["lane"], case["leg_id"], case["case_id"])
        for case in expected_formal_cases(contract, stage)
    ]


def _validate_projection_value(column: dict[str, Any], value: str) -> bool:
    kind = column.get("type")
    if kind == "string":
        return bool(value)
    if kind == "availability":
        return bool(value)
    optional = isinstance(kind, str) and kind.endswith("_or_empty")
    if optional and value == "":
        return True
    base_kind = kind.removesuffix("_or_empty") if optional else kind
    if base_kind == "sha256":
        return len(value) == 64 and all(
            character in "0123456789abcdef" for character in value
        )
    try:
        if base_kind in {"u32", "u64"}:
            number = int(value)
            maximum = (1 << 32) - 1 if base_kind == "u32" else (1 << 64) - 1
            return 0 <= number <= maximum and str(number) == value
        if base_kind == "f64":
            number = float(value)
            return (
                number >= 0.0
                and number == number
                and number not in {float("inf"), float("-inf")}
            )
    except (TypeError, ValueError):
        return False
    return False


def validate_projection_rows(
    contract: dict[str, Any], stage: str, rows: list[dict[str, str]]
) -> None:
    projection = contract.get("projection")
    if not isinstance(projection, dict):
        raise ValueError("projection contract is missing")
    columns = projection.get("columns")
    if not isinstance(columns, list) or not columns:
        raise ValueError("projection columns must be a nonempty list")
    column_names = [column.get("name") for column in columns if isinstance(column, dict)]
    if len(column_names) != len(columns) or len(column_names) != len(set(column_names)):
        raise ValueError("projection columns must have unique names")
    expected_keys = expected_projection_keys(contract, stage)
    observed_keys: list[tuple[str, str, str, str]] = []
    availability_values = set(projection.get("availability_values", []))
    groups = projection.get("field_groups", {})
    formal_by_key = {
        (stage, case["lane"], case["leg_id"], case["case_id"]): case
        for case in expected_formal_cases(contract, stage)
    }
    for index, row in enumerate(rows):
        if list(row) != column_names:
            raise ValueError(f"projection row {index} columns differ from schema")
        if row.get("schema_version") != str(projection.get("schema_version")):
            raise ValueError(f"projection row {index} has the wrong schema_version")
        if row.get("contract_id") != contract.get("contract_id"):
            raise ValueError(f"projection row {index} has the wrong contract_id")
        key = (
            row.get("stage_id", ""),
            row.get("lane", ""),
            row.get("leg_id", ""),
            row.get("case_id", ""),
        )
        observed_keys.append(key)
        if key[0] != stage:
            raise ValueError(f"projection row {index} has the wrong stage_id")
        for column in columns:
            name = column["name"]
            if not _validate_projection_value(column, row.get(name, "")):
                raise ValueError(f"projection row {index} has invalid {name}")
        expected_availability = projection_field_applicability(
            contract, stage, key[2], key[3]
        )
        for group_name, group in groups.items():
            availability_column = group["availability_column"]
            observed = row[availability_column]
            expected = expected_availability[group_name]
            if observed not in availability_values or observed != expected:
                raise ValueError(
                    f"projection row {index} {availability_column} is {observed!r}, expected {expected!r}"
                )
            values = [row[column] for column in group["value_columns"]]
            if observed == "available" and any(value == "" for value in values):
                raise ValueError(f"projection row {index} {group_name} has an empty required value")
            if observed != "available" and any(value != "" for value in values):
                raise ValueError(
                    f"projection row {index} {group_name} has values while not applicable"
                )
        formal_case = formal_by_key.get(key)
        if formal_case is None:
            continue
        layout = build_fixture_layout(contract, formal_case["size"])
        expected_fixture = {
            "fixture_checksum": layout["layout_checksum"],
            "rooms": str(layout["counts"]["rooms"]),
            "completed_floors": str(layout["counts"]["completed_floors"]),
            "completed_walls": str(layout["counts"]["completed_walls"]),
            "doors": str(layout["counts"]["doors"]),
            "supplied_lamp_candidates": str(
                layout["counts"]["supplied_lamp_candidates"]
            ),
            "unsupplied_lamp_candidates": str(
                layout["counts"]["unsupplied_lamp_candidates"]
            ),
        }
        for field, expected_value in expected_fixture.items():
            if row[field] != expected_value:
                raise ValueError(
                    f"projection row {index} {field} differs from its fixture contract"
                )
        if expected_availability["indoor_mask"] == "available":
            size_contract = contract["fixture"]["sizes"][formal_case["size"]]
            if row["indoor_mask_cells"] != str(layout["counts"]["completed_floors"]):
                raise ValueError(
                    f"projection row {index} indoor_mask_cells differs from its fixture contract"
                )
            if row["indoor_mask_checksum"] != size_contract["indoor_mask_checksum"]:
                raise ValueError(
                    f"projection row {index} indoor_mask_checksum differs from its fixture contract"
                )
        if expected_availability["emitter"] == "available":
            typed_emitters = (
                layout["counts"]["supplied_lamp_candidates"]
                + layout["counts"]["unsupplied_lamp_candidates"]
            )
            if row["typed_emitter_components"] != str(typed_emitters):
                raise ValueError(
                    f"projection row {index} typed_emitter_components differs from its "
                    "fixture contract"
                )
        if expected_availability["eligible_emitter"] == "available" and row[
            "eligible_supplied_emitters"
        ] != str(layout["counts"]["supplied_lamp_candidates"]):
            raise ValueError(
                f"projection row {index} eligible_supplied_emitters differs from its "
                "fixture contract"
            )
    if observed_keys != expected_keys:
        raise ValueError("projection primary key set or order differs from the formal matrix")


def expand_required_leaf_gates(
    contract: dict[str, Any], stage: str
) -> list[str]:
    stage_contract = contract.get("stages", {}).get(stage)
    if not isinstance(stage_contract, dict):
        raise ValueError(f"contract has no stage definition for {stage}")
    bundles = contract.get("gate_bundles", {})
    known_gates = {
        gate.get("gate_id")
        for gate in contract.get("gates", [])
        if isinstance(gate, dict) and isinstance(gate.get("gate_id"), str)
    }
    leaves: list[str] = []

    def expand(gate_id: str, ancestry: tuple[str, ...]) -> None:
        if gate_id in ancestry:
            raise ValueError("gate bundle cycle: " + " -> ".join((*ancestry, gate_id)))
        members = bundles.get(gate_id)
        if members is None:
            if gate_id not in known_gates:
                raise ValueError(f"unknown required gate {gate_id}")
            if gate_id in leaves:
                raise ValueError(f"required gate {gate_id} is duplicated after bundle expansion")
            leaves.append(gate_id)
            return
        if not isinstance(members, list) or not members:
            raise ValueError(f"gate bundle {gate_id} must have exact nonempty members")
        for member in members:
            if not isinstance(member, str):
                raise ValueError(f"gate bundle {gate_id} has an invalid member")
            expand(member, (*ancestry, gate_id))

    for gate_id in stage_contract.get("required_gate_ids", []):
        expand(gate_id, ())
    return leaves


def _gate_case_set(
    contract: dict[str, Any], stage: str, case_set_name: str
) -> list[str]:
    result = contract.get("gate_result", {}).get("case_sets", {}).get(case_set_name)
    if isinstance(result, list) and all(isinstance(value, str) for value in result):
        return list(result)
    formal_cases = expected_formal_cases(contract, stage)
    if result == "from-stage-required-behavior-cases":
        return [
            case["case_id"] for case in formal_cases if case["leg_id"] == "behavior"
        ]
    if result == "all-required-cases-except-renderdoc":
        return [
            case["case_id"] for case in formal_cases if case["leg_id"] != "renderdoc"
        ]
    if result == "all-required-cases":
        return [case["case_id"] for case in formal_cases]
    raise ValueError(f"unknown gate case set {case_set_name}")


def _template_threshold(
    contract: dict[str, Any],
    gate_id: str,
    template: dict[str, Any],
    *,
    stage: str | None = None,
    case_id: str | None = None,
) -> str:
    if "threshold" in template:
        threshold = template["threshold"]
        if threshold == "behavior-case-contract" and stage is not None and case_id is not None:
            behavior_id = case_id.removeprefix("behavior-")
            behavior_case = next(
                (
                    entry
                    for entry in contract.get("behavior_cases", [])
                    if entry.get("case_id") == behavior_id
                ),
                None,
            )
            if not isinstance(behavior_case, dict):
                raise ValueError(f"unknown behavior threshold case {case_id}")
            threshold_key = "expected_wake_p05" if _stage_index(stage) >= _stage_index("p05") else "expected_wake_current"
            resolved = behavior_case.get(threshold_key)
            if not isinstance(resolved, int) or isinstance(resolved, bool):
                raise ValueError(
                    f"behavior case {behavior_id} has no numeric wake threshold for {stage}"
                )
            return str(resolved)
        if threshold == "fixture-size-contract" and case_id is not None:
            parts = case_id.split("-")
            if len(parts) != 3 or parts[0] != "audit" or parts[2] != "cpu":
                raise ValueError(
                    f"fixture-size threshold cannot resolve non-audit case {case_id}"
                )
            size = parts[1]
            expected_counts = {
                "typed_emitter_components": {"small": 2, "medium": 11, "large": 51},
                "eligible_supplied_emitters": {
                    "small": 1,
                    "medium": 10,
                    "large": 50,
                },
                "indoor_mask_cells": {
                    fixture_size: build_fixture_layout(contract, fixture_size)["counts"][
                        "completed_floors"
                    ]
                    for fixture_size in ("small", "medium", "large")
                },
            }
            metric_values = expected_counts.get(template.get("metric_id"))
            if metric_values is None or size not in metric_values:
                raise ValueError(
                    f"gate {gate_id} has no fixture-size threshold for "
                    f"{template.get('metric_id')} / {size}"
                )
            return str(metric_values[size])
        if threshold == "fixture-mask-contract" and case_id is not None:
            parts = case_id.split("-")
            if len(parts) != 3 or parts[0] != "audit" or parts[2] != "cpu":
                raise ValueError(
                    f"fixture mask threshold cannot resolve non-audit case {case_id}"
                )
            checksum = (
                contract.get("fixture", {})
                .get("sizes", {})
                .get(parts[1], {})
                .get("indoor_mask_checksum")
            )
            if not isinstance(checksum, str):
                raise ValueError(f"gate {gate_id} has no fixture mask threshold")
            return checksum
        return str(threshold).lower() if isinstance(threshold, bool) else str(threshold)
    reference = template.get("threshold_ref")
    gate = next(
        (entry for entry in contract.get("gates", []) if entry.get("gate_id") == gate_id),
        None,
    )
    if not isinstance(reference, str) or not isinstance(gate, dict) or reference not in gate:
        raise ValueError(f"gate {gate_id} template has an unresolved threshold")
    return str(gate[reference])


def _template_reference_stage(template: dict[str, Any], case_id: str) -> str | None:
    reference_stage = template.get("reference_stage")
    by_render = template.get("reference_stage_by_render")
    if reference_stage is not None and by_render is not None:
        raise ValueError("gate template cannot select two reference lineage modes")
    if reference_stage is not None:
        if not isinstance(reference_stage, str) or reference_stage not in RTT_LIGHT_STAGES:
            raise ValueError(f"invalid gate reference stage {reference_stage!r}")
        return reference_stage
    if by_render is None:
        return None
    if not isinstance(by_render, dict) or set(by_render) != {"cpu", "gpu"}:
        raise ValueError("gate render-specific reference lineage must define cpu and gpu")
    render = case_id.rsplit("-", 1)[-1]
    resolved = by_render.get(render)
    if not isinstance(resolved, str) or resolved not in RTT_LIGHT_STAGES:
        raise ValueError(f"gate case {case_id} has no valid reference stage")
    return resolved


def expected_gate_result_rows(
    contract: dict[str, Any], stage: str
) -> list[dict[str, str]]:
    gate_result = contract.get("gate_result", {})
    templates = gate_result.get("metric_templates", {})
    formal_case_ids = {
        case["case_id"] for case in expected_formal_cases(contract, stage)
    }
    rows: list[dict[str, str]] = []
    for gate_id in expand_required_leaf_gates(contract, stage):
        gate_templates = templates.get(gate_id)
        if not isinstance(gate_templates, list) or not gate_templates:
            raise ValueError(f"required leaf gate {gate_id} has no metric templates")
        for template in gate_templates:
            required_stages = template.get("required_stages")
            if required_stages is not None and stage not in required_stages:
                continue
            case_ids = _gate_case_set(contract, stage, template.get("case_set", ""))
            case_filter = template.get("case_filter")
            if case_filter is not None:
                case_ids = [case_id for case_id in case_ids if str(case_filter) in case_id]
            if not case_ids:
                raise ValueError(
                    f"gate {gate_id} metric {template.get('metric_id')} selects no cases"
                )
            for case_id in case_ids:
                if case_id != "attempt" and case_id not in formal_case_ids:
                    raise ValueError(
                        f"gate {gate_id} metric {template.get('metric_id')} selects "
                        f"non-formal case {case_id} at stage {stage}"
                    )
                reference_stage = _template_reference_stage(template, case_id)
                if reference_stage is not None and case_id not in {
                    case["case_id"]
                    for case in expected_formal_cases(contract, reference_stage)
                }:
                    raise ValueError(
                        f"gate {gate_id} reference stage {reference_stage} has no case {case_id}"
                    )
                threshold = _template_threshold(
                    contract,
                    gate_id,
                    template,
                    stage=stage,
                    case_id=case_id,
                )
                value_type = GATE_UNIT_TYPES[template["unit"]]
                parsed_threshold = _parse_gate_scalar(threshold, value_type)
                if template["comparator"] != "eq" and isinstance(
                    parsed_threshold, (bool, str)
                ):
                    raise ValueError(
                        f"gate {gate_id} comparator {template['comparator']} is incompatible "
                        f"with {value_type}"
                    )
                rows.append(
                    {
                        "gate_id": gate_id,
                        "stage_id": stage,
                        "case_id": case_id,
                        "metric_id": template["metric_id"],
                        "unit": template["unit"],
                        "comparator": template["comparator"],
                        "threshold": threshold,
                        "aggregation": template["aggregation"],
                        "value_type": value_type,
                        "reference_stage": reference_stage,
                        "reference_artifact": (
                            ""
                            if reference_stage is None
                            else gate_result["artifact_locators"]["reference"].format(
                                reference_stage_id=reference_stage,
                                case_id=case_id,
                            )
                        ),
                        "subject_artifact": gate_result["artifact_locators"][
                            "subject"
                        ].format(
                            stage_id=stage,
                            case_id=case_id,
                        ),
                    }
                )
    keys = [
        (row["gate_id"], row["stage_id"], row["case_id"], row["metric_id"])
        for row in rows
    ]
    if len(keys) != len(set(keys)):
        raise ValueError(f"stage {stage} gate metric templates expand to duplicate keys")
    return rows


def _parse_gate_scalar(value: str, value_type: str) -> bool | int | float | str:
    if value_type == "boolean":
        if value == "true":
            return True
        if value == "false":
            return False
        raise ValueError(f"expected boolean, got {value!r}")
    if value_type == "u64":
        parsed = int(value)
        if parsed < 0 or parsed > (1 << 64) - 1 or str(parsed) != value:
            raise ValueError(f"expected canonical unsigned integer, got {value!r}")
        return parsed
    if value_type == "i64":
        parsed = int(value)
        if not -(1 << 63) <= parsed <= (1 << 63) - 1 or str(parsed) != value:
            raise ValueError(f"expected canonical signed integer, got {value!r}")
        return parsed
    if value_type == "f64":
        parsed = float(value)
        if parsed != parsed or parsed in {float("inf"), float("-inf")}:
            raise ValueError(f"expected finite float, got {value!r}")
        return parsed
    if value_type == "sha256":
        if len(value) != 64 or any(
            character not in "0123456789abcdef" for character in value
        ):
            raise ValueError(f"expected sha256 digest, got {value!r}")
        return value
    raise ValueError(f"unsupported gate scalar type {value_type}")


def _gate_comparison_passes(
    observed: bool | int | float | str,
    comparator: str,
    threshold: bool | int | float | str,
) -> bool:
    if comparator == "eq":
        return observed == threshold
    if isinstance(observed, (bool, str)) or isinstance(threshold, (bool, str)):
        raise ValueError(f"comparator {comparator} accepts only numeric scalars")
    if comparator == "le":
        return observed <= threshold
    if comparator == "ge":
        return observed >= threshold
    raise ValueError(f"unsupported gate comparator {comparator}")


def validate_gate_result_rows(
    contract: dict[str, Any], stage: str, rows: list[dict[str, str]], *, require_pass: bool
) -> None:
    gate_result = contract.get("gate_result", {})
    columns = gate_result.get("columns")
    if not isinstance(columns, list) or not columns:
        raise ValueError("gate result columns must be a nonempty list")
    expected = expected_gate_result_rows(contract, stage)
    expected_by_key = {
        (row["gate_id"], row["stage_id"], row["case_id"], row["metric_id"]): row
        for row in expected
    }
    observed_keys: list[tuple[str, str, str, str]] = []
    statuses = set(gate_result.get("statuses", []))
    reasons = set(gate_result.get("reason_codes", []))
    for index, row in enumerate(rows):
        if list(row) != columns:
            raise ValueError(f"gate result row {index} columns differ from schema")
        key = tuple(row.get(column, "") for column in gate_result["primary_key"])
        observed_keys.append(key)
        expected_row = expected_by_key.get(key)
        if expected_row is None:
            raise ValueError(f"gate result row {index} has an unknown primary key")
        for field in ("unit", "comparator", "threshold"):
            if row.get(field) != expected_row[field]:
                raise ValueError(f"gate result row {index} has the wrong {field}")
        if row.get("status") not in statuses or row.get("reason_code") not in reasons:
            raise ValueError(f"gate result row {index} has an unknown status or reason")
        if not row.get("observed"):
            raise ValueError(f"gate result row {index} has no observed value")
        if row.get("subject_artifact") != expected_row["subject_artifact"]:
            raise ValueError(f"gate result row {index} has the wrong subject artifact lineage")
        if row.get("reference_artifact") != expected_row["reference_artifact"]:
            raise ValueError(
                f"gate result row {index} has the wrong reference artifact lineage"
            )
        passed = row.get("status") == "pass"
        if passed != (row.get("reason_code") == "none"):
            raise ValueError(
                f"gate result row {index} status and reason_code are inconsistent"
            )
        if row.get("status") in {"pass", "fail"}:
            try:
                observed = _parse_gate_scalar(
                    row["observed"], expected_row["value_type"]
                )
                threshold = _parse_gate_scalar(
                    expected_row["threshold"], expected_row["value_type"]
                )
                comparison_passes = _gate_comparison_passes(
                    observed, expected_row["comparator"], threshold
                )
            except (TypeError, ValueError) as error:
                raise ValueError(
                    f"gate result row {index} has an invalid scalar: {error}"
                ) from error
            if passed != comparison_passes:
                raise ValueError(
                    f"gate result row {index} status disagrees with its comparison"
                )
            if not passed and row.get("reason_code") not in {
                "threshold_exceeded",
                "value_mismatch",
            }:
                raise ValueError(
                    f"gate result row {index} fail status has the wrong reason_code"
                )
        if require_pass and (
            row.get("status") != "pass" or row.get("reason_code") != "none"
        ):
            raise ValueError(f"gate result row {index} does not pass")
    if observed_keys != list(expected_by_key):
        raise ValueError("gate result primary key set or order differs from the contract")


def validate_rtt_light_contract(contract: dict[str, Any]) -> None:
    contract_id = contract.get("contract_id")
    expected_hash = EXPECTED_CONTRACT_SHA256.get(contract_id)
    actual_hash = canonical_sha256(contract)
    if expected_hash is None or actual_hash != expected_hash:
        raise ValueError(
            "RtT-light contract content differs from its pinned contract snapshot"
        )
    if contract.get("schema_version") != 1:
        raise ValueError("RtT-light contract schema_version must be 1")
    if contract.get("contract_id") != "rtt-light-v1":
        raise ValueError("RtT-light contract_id must be rtt-light-v1")
    if contract.get("lifecycle") != {
        "status": "frozen",
        "formal_registration_allowed": True,
        "freeze_blockers": [],
    }:
        raise ValueError("RtT-light lifecycle differs from the frozen pinned snapshot")
    if set(contract.get("stages", {})) != set(RTT_LIGHT_STAGES):
        raise ValueError("RtT-light contract must define every stage exactly once")

    fixture = contract.get("fixture")
    if not isinstance(fixture, dict) or set(fixture.get("sizes", {})) != {
        "small",
        "medium",
        "large",
    }:
        raise ValueError("RtT-light contract must define small, medium, and large fixtures")
    if fixture.get("building_types") != [
        "Wall",
        "Door",
        "Floor",
        "Tank",
        "MudMixer",
        "RestArea",
        "Bridge",
        "SandPile",
        "BonePile",
        "WheelbarrowParking",
        "SoulSpa",
        "OutdoorLamp",
    ]:
        raise ValueError("RtT-light building type inventory differs from rtt-light-v1")
    if fixture.get("runtime_fixture_support") != {
        "small": "current/static production-topology vertical slice",
        "medium": "current/static production-topology all-building showcase",
        "large": "current/static production-topology all-building showcase",
    }:
        raise ValueError("RtT-light runtime fixture support differs from rtt-light-v1")
    if fixture.get("outdoor_lamp_demand") != 0.2:
        raise ValueError("RtT-light OutdoorLamp demand differs from rtt-light-v1")
    if fixture.get("current_presentation") != {
        "Floor": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 0,
            "owner_3d_per_entity": 1,
        },
        "Wall": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 0,
            "owner_3d_per_entity": 1,
        },
        "Door": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 1,
            "owner_3d_per_entity": 1,
        },
        "Tank": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 1,
            "owner_3d_per_entity": 1,
        },
        "MudMixer": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 1,
            "owner_3d_per_entity": 1,
        },
        "RestArea": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 1,
            "owner_3d_per_entity": 1,
        },
        "Bridge": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 1,
            "owner_3d_per_entity": 0,
        },
        "SandPile": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 1,
            "owner_3d_per_entity": 1,
        },
        "BonePile": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 1,
            "owner_3d_per_entity": 1,
        },
        "WheelbarrowParking": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 1,
            "owner_3d_per_entity": 1,
        },
        "SoulSpa": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 1,
            "owner_3d_per_entity": 1,
        },
        "OutdoorLamp": {
            "root_sprite_per_entity": 0,
            "child_sprite_per_entity": 1,
            "owner_3d_per_entity": 1,
        },
    }:
        raise ValueError("RtT-light current presentation topology differs from rtt-light-v1")
    if fixture.get("preflight_rule") != (
        "every canonical occupied cell must be in bounds and free of pre-existing "
        "building, door, stockpile, or Yard ownership; Floor, Wall, Door, lamp, "
        "SoulSpa, equipment, and Tank companion cells must also be terrain-walkable; "
        "Bridge is a fixture-seeded completion probe against the exact current 2x5 "
        "production completion geometry and does not claim that current player "
        "authoring validation accepts the generated river; reject the fixture instead "
        "of deleting or replacing world-generation content"
    ):
        raise ValueError("RtT-light fixture preflight rule differs from rtt-light-v1")
    if fixture.get("building_showcase_id") != "all-building-showcase-v1":
        raise ValueError("RtT-light showcase id differs from rtt-light-v1")
    if fixture.get("building_showcase_rule") != (
        "medium and large enumerate BuildingType order exactly once; canonical Floor, "
        "Wall, Door, SoulSpa, and OutdoorLamp entities are referenced, while the other "
        "seven roots are fixture-seeded through their current production completion "
        "route; every anchor and occupied cell is semantic fixture data"
    ):
        raise ValueError("RtT-light showcase rule differs from rtt-light-v1")
    if fixture.get("showcase_companion_entity_rule") != (
        "showcase_companions counts logical placements; Tank BucketStorage creates "
        "one ECS entity per occupied grid"
    ):
        raise ValueError("RtT-light showcase companion entity rule differs from rtt-light-v1")
    component_contract = fixture.get("showcase_component_contract")
    if component_contract != {
        "Wall": {"required_components": ["Building"], "owned_entities": {}},
        "Door": {
            "required_components": ["Building", "Door"],
            "owned_entities": {},
        },
        "Floor": {"required_components": ["Building"], "owned_entities": {}},
        "Tank": {
            "required_components": ["Building", "Stockpile:Water:50"],
            "owned_entities": {"BucketStorage": 2, "BucketEmpty": 5},
        },
        "MudMixer": {
            "required_components": [
                "Building",
                "MudMixerStorage",
                "Stockpile:Water",
            ],
            "owned_entities": {},
        },
        "RestArea": {
            "required_components": ["Building", "RestArea"],
            "owned_entities": {},
        },
        "Bridge": {
            "required_components": ["Building", "BridgeMarker"],
            "owned_entities": {},
        },
        "SandPile": {
            "required_components": ["Building", "SandPile"],
            "owned_entities": {},
        },
        "BonePile": {
            "required_components": ["Building", "BonePile"],
            "owned_entities": {},
        },
        "WheelbarrowParking": {
            "required_components": ["Building", "WheelbarrowParking"],
            "owned_entities": {"Wheelbarrow": 2},
        },
        "SoulSpa": {
            "required_components": ["Building", "SoulSpaSite", "PowerGenerator"],
            "owned_entities": {"SoulSpaTile": 4},
        },
        "OutdoorLamp": {
            "required_components": ["Building", "PowerConsumer:0.2"],
            "owned_entities": {},
        },
    }:
        raise ValueError("RtT-light showcase component contract differs from rtt-light-v1")

    formal_matrix = contract.get("formal_matrix")
    if not isinstance(formal_matrix, dict):
        raise ValueError("RtT-light contract must define formal_matrix")
    expected_window = {
        "logical_width": 1920,
        "logical_height": 1080,
        "physical_width": 1920,
        "physical_height": 1080,
        "scale_factor": 1.0,
        "rtt_quality": "high",
        "scene_target_width": 1920,
        "scene_target_height": 1080,
    }
    if formal_matrix.get("window") != expected_window:
        raise ValueError("RtT-light formal window matrix differs from rtt-light-v1")
    if formal_matrix.get("seed") != 20260803:
        raise ValueError("RtT-light formal seed differs from rtt-light-v1")
    if formal_matrix.get("backend") != "vulkan":
        raise ValueError("RtT-light formal backend differs from rtt-light-v1")
    if formal_matrix.get("present_mode") != "novsync":
        raise ValueError("RtT-light formal present mode differs from rtt-light-v1")
    if formal_matrix.get("fixed_hz") != 64:
        raise ValueError("RtT-light formal fixed rate differs from rtt-light-v1")
    if contract.get("statistics") != {
        "run_quantile_method": "sorted-index-round-half-up",
        "run_quantile_index_formula": "floor((sample_count - 1) * ratio + 0.5)",
        "ratios": {"p50": 0.5, "p95": 0.95, "p99": 0.99},
        "session_aggregate": "median-of-run-values",
        "dispersion": "median-absolute-deviation",
    }:
        raise ValueError("RtT-light statistics contract differs from rtt-light-v1")

    expected_formal_legs = [
        {
            "leg_id": "audit",
            "lane": "static",
            "row_scope": "leg-session-aggregate",
            "first_required_stage": "current",
            "sizes": ["small", "medium", "large"],
            "renders": ["cpu"],
            "environment": "headless",
            "repeat": 3,
        },
        {
            "leg_id": "behavior",
            "lane": "behavior",
            "row_scope": "leg-session-aggregate",
            "first_required_stage": "current",
            "sizes": ["small"],
            "renders": ["cpu"],
            "environment": "headless",
            "repeat": 3,
            "case_source": "stage-required-behavior-cases",
        },
        {
            "leg_id": "capture",
            "lane": "static",
            "row_scope": "leg-session-aggregate",
            "first_required_stage": "current",
            "sizes": ["small", "medium", "large"],
            "renders": ["cpu", "gpu"],
            "environment": "windowed",
            "repeat": 3,
        },
        {
            "leg_id": "renderdoc",
            "lane": "static",
            "row_scope": "leg-session-aggregate",
            "first_required_stage": "current",
            "sizes": ["medium"],
            "renders": ["gpu"],
            "environment": "windowed",
            "repeat": 1,
        },
        {
            "leg_id": "memory",
            "lane": "static",
            "row_scope": "leg-session-aggregate",
            "first_required_stage": "current",
            "sizes": ["small", "medium", "large"],
            "renders": ["cpu", "gpu"],
            "environment": "windowed",
            "repeat": 3,
        },
        {
            "leg_id": "field-core",
            "lane": "field-core",
            "row_scope": "leg-session-aggregate",
            "first_required_stage": "p03",
            "sizes": ["large"],
            "renders": ["cpu"],
            "environment": "headless",
            "repeat": 3,
        },
        {
            "leg_id": "consumer-core",
            "lane": "consumer-core",
            "row_scope": "leg-session-aggregate",
            "first_required_stage": "p07",
            "sizes": ["large"],
            "renders": ["cpu"],
            "environment": "headless",
            "repeat": 3,
        },
    ]
    if contract.get("formal_legs") != expected_formal_legs:
        raise ValueError("RtT-light formal leg matrix differs from rtt-light-v1")
    if formal_matrix.get("repeat") != 3:
        raise ValueError("RtT-light formal repeat count differs from rtt-light-v1")
    if formal_matrix.get("audit") != {
        "preflight_runs": 0,
        "warmup_ticks": 1920,
        "audit_ticks": 128,
    }:
        raise ValueError("RtT-light formal audit matrix differs from rtt-light-v1")
    if formal_matrix.get("behavior") != {"preflight_runs": 0}:
        raise ValueError("RtT-light formal behavior matrix differs from rtt-light-v1")
    for leg_name in ("capture", "memory"):
        if formal_matrix.get(leg_name) != {
            "preflight_runs": 1,
            "warmup_secs": 30.0,
            "measure_secs": 60.0,
        }:
            raise ValueError(
                f"RtT-light formal {leg_name} matrix differs from rtt-light-v1"
            )
    if formal_matrix.get("field_core") != {
        "warmup_calls": 32,
        "measure_calls": 256,
        "steady_updates": 600,
    }:
        raise ValueError("RtT-light formal field_core matrix differs from rtt-light-v1")
    if formal_matrix.get("consumer_core") != {
        "warmup_calls": 32,
        "measure_calls": 256,
    }:
        raise ValueError("RtT-light formal consumer_core matrix differs from rtt-light-v1")
    if formal_matrix.get("renderdoc") != {
        "size": "medium",
        "render": "gpu",
        "settle_frames": 4,
        "capture_frame": 4,
        "repeat": 1,
    }:
        raise ValueError("RtT-light RenderDoc matrix differs from rtt-light-v1")

    behavior_fixture = contract.get("behavior_fixture")
    if not isinstance(behavior_fixture, dict):
        raise ValueError("RtT-light behavior fixture is missing")
    if {
        key: behavior_fixture.get(key)
        for key in ("size", "render", "clock", "repeat", "save_storage")
    } != {
        "size": "small",
        "render": "cpu",
        "clock": "fixed-step-behavior",
        "repeat": 3,
        "save_storage": "job-owned-private-path",
    }:
        raise ValueError("RtT-light behavior fixture selector differs from rtt-light-v1")
    timeline = behavior_fixture.get("timeline")
    expected_timeline_columns = [
        "case_id",
        "step_index",
        "script_update",
        "simulation_tick",
        "pause_state",
        "world_epoch",
        "intent",
        "attempted",
        "applied",
        "semantic_state",
        "active_presentation_state",
        "registry_phase",
        "registry_step_id",
        "wake_count",
        "field_availability",
        "field_input_revision",
        "field_output_revision",
        "field_read_count",
        "old_epoch_field_read_count",
        "field_is_dark",
        "field_checksum",
        "gpu_availability",
        "gpu_upload_epoch",
        "gpu_checksum",
        "fixture_checksum",
        "terminal_outcome",
    ]
    if timeline != {
        "schema_version": 1,
        "file": "data/timeline.json",
        "row_scope": "script-step",
        "primary_key": ["case_id", "step_index"],
        "columns": expected_timeline_columns,
        "availability_values": [
            "available",
            "stage_before_field_owner",
            "stage_before_registry_owner",
            "stage_before_gpu_owner",
        ],
        "terminal_outcomes": [
            "in_progress",
            "succeeded",
            "rejected",
            "failed_dark",
        ],
    }:
        raise ValueError("RtT-light behavior timeline schema differs from rtt-light-v1")

    door_fixture = behavior_fixture.get("door_state_v1")
    if not isinstance(door_fixture, dict) or door_fixture.get("production_topology") != {
        "domain_root_door": 1,
        "domain_root_sprite": 0,
        "child_sprite": 1,
        "owner_3d": 1,
    }:
        raise ValueError("RtT-light Door production topology differs from rtt-light-v1")
    door_steps = door_fixture.get("steps") if isinstance(door_fixture, dict) else None
    if not isinstance(door_steps, list) or [
        step.get("step_index") for step in door_steps if isinstance(step, dict)
    ] != list(range(5)):
        raise ValueError("RtT-light Door behavior steps must be the exact five-step script")
    expected_door_intents = [
        "observe-initial",
        "auto-open-nearby-soul",
        "pause",
        "manual-lock-while-paused",
        "resume",
    ]
    if [step.get("intent") for step in door_steps] != expected_door_intents:
        raise ValueError("RtT-light Door behavior intent order differs from rtt-light-v1")
    if [step.get("script_update") for step in door_steps] != list(range(5)):
        raise ValueError("RtT-light Door behavior script clock differs from rtt-light-v1")
    if [step.get("pause_state") for step in door_steps] != [
        "running",
        "running",
        "paused",
        "paused",
        "running",
    ]:
        raise ValueError("RtT-light Door behavior pause states differ from rtt-light-v1")
    if [step.get("attempted") for step in door_steps] != [False, True, False, True, False]:
        raise ValueError("RtT-light Door attempted transitions differ from rtt-light-v1")
    if any(step.get("current_applied") is not False for step in door_steps):
        raise ValueError("RtT-light current Door baseline must apply no transition")
    if any(step.get("current_semantic_state") != "closed" for step in door_steps):
        raise ValueError("RtT-light current Door baseline must remain closed")
    if any(
        step.get("current_active_presentation_state") != "closed"
        for step in door_steps
    ):
        raise ValueError("RtT-light current Door presentation must remain closed")
    if [step.get("p02_applied") for step in door_steps] != [
        False,
        True,
        False,
        True,
        False,
    ]:
        raise ValueError("RtT-light P02 Door applied transitions differ from rtt-light-v1")
    if [step.get("p02_semantic_state") for step in door_steps] != [
        "closed",
        "open",
        "open",
        "locked",
        "locked",
    ]:
        raise ValueError("RtT-light P02 Door semantic states differ from rtt-light-v1")
    if any(
        step.get("p02_active_presentation_state")
        != step.get("p02_semantic_state")
        for step in door_steps
    ):
        raise ValueError("RtT-light P02 Door presentation must follow semantic state")

    load_fixture = behavior_fixture.get("load_normal_v1")
    if not isinstance(load_fixture, dict) or {
        key: load_fixture.get(key)
        for key in (
            "world_epoch_delta",
            "save_outcomes",
            "load_outcomes",
            "pause_state_after_load",
            "entity_rebind_key",
            "terminal_checksum_scope",
        )
    } != {
        "world_epoch_delta": 1,
        "save_outcomes": 1,
        "load_outcomes": 1,
        "pause_state_after_load": "unchanged",
        "entity_rebind_key": "building-kind-grid-owner-relation",
        "terminal_checksum_scope": "durable-fixture-after-semantic-rebind",
    }:
        raise ValueError("RtT-light normal-load fixture differs from rtt-light-v1")
    load_steps = load_fixture.get("steps") if isinstance(load_fixture, dict) else None
    if not isinstance(load_steps, list) or [
        step.get("step_index") for step in load_steps if isinstance(step, dict)
    ] != list(range(6)):
        raise ValueError("RtT-light normal-load behavior must be the exact six-step script")
    if [step.get("intent") for step in load_steps] != [
        "observe-initial",
        "request-save",
        "observe-save-succeeded",
        "request-load",
        "observe-load-succeeded",
        "verify-semantic-rebind",
    ] or [step.get("terminal_outcome") for step in load_steps] != [
        "in_progress",
        "in_progress",
        "in_progress",
        "in_progress",
        "in_progress",
        "succeeded",
    ]:
        raise ValueError("RtT-light normal-load behavior sequence differs from rtt-light-v1")

    behavior_cases = contract.get("behavior_cases")
    if not isinstance(behavior_cases, list) or not behavior_cases:
        raise ValueError("RtT-light behavior cases must be nonempty")
    behavior_case_ids = [
        case.get("case_id") for case in behavior_cases if isinstance(case, dict)
    ]
    if len(behavior_case_ids) != len(behavior_cases) or len(behavior_case_ids) != len(
        set(behavior_case_ids)
    ):
        raise ValueError("RtT-light behavior case ids must be unique strings")
    for case in behavior_cases:
        first_stage = case.get("first_required_stage")
        if not isinstance(first_stage, str) or first_stage not in RTT_LIGHT_STAGES:
            raise ValueError(
                f"RtT-light behavior case {case.get('case_id')} has an invalid first stage"
            )
    for stage in RTT_LIGHT_STAGES:
        required_behavior_cases = contract["stages"][stage].get(
            "required_behavior_cases"
        )
        expected_behavior_cases = [
            case["case_id"]
            for case in behavior_cases
            if _stage_index(stage) >= _stage_index(case["first_required_stage"])
        ]
        if required_behavior_cases != expected_behavior_cases:
            raise ValueError(
                f"RtT-light stage {stage} behavior cases differ from first-stage declarations"
            )

    projection = contract.get("projection")
    if not isinstance(projection, dict):
        raise ValueError("RtT-light projection contract is missing")
    if {
        "schema_version": projection.get("schema_version"),
        "file": projection.get("file"),
        "row_scope": projection.get("row_scope"),
        "primary_key": projection.get("primary_key"),
    } != {
        "schema_version": 1,
        "file": "data/rtt_light_migration.csv",
        "row_scope": "leg-session-aggregate",
        "primary_key": ["stage_id", "lane", "leg_id", "case_id"],
    }:
        raise ValueError("RtT-light projection identity differs from rtt-light-v1")
    projection_columns = projection.get("columns")
    if not isinstance(projection_columns, list) or not projection_columns:
        raise ValueError("RtT-light projection columns must be nonempty")
    projection_names = [
        column.get("name") for column in projection_columns if isinstance(column, dict)
    ]
    if len(projection_names) != len(projection_columns) or len(projection_names) != len(
        set(projection_names)
    ):
        raise ValueError("RtT-light projection columns must have unique names")
    projection_types = {"string", "sha256", "availability", "u32", "u64", "f64"}
    for column in projection_columns:
        kind = column.get("type")
        base_kind = kind.removesuffix("_or_empty") if isinstance(kind, str) else kind
        if base_kind not in projection_types or not isinstance(column.get("unit"), str):
            raise ValueError(f"RtT-light projection column {column.get('name')} is invalid")
    groups = projection.get("field_groups")
    if not isinstance(groups, dict) or set(groups) != {
        "fixture",
        "emitter",
        "eligible_emitter",
        "indoor_mask",
        "render_inventory",
        "wall_frame",
        "memory",
        "field_core",
        "emitter_collect_allocation",
        "field_rebuild_allocation",
        "gpu_upload",
        "consumer_core",
    }:
        raise ValueError("RtT-light projection field groups differ from rtt-light-v1")
    availability_columns: set[str] = set()
    value_columns: set[str] = set()
    availability_values = set(projection.get("availability_values", []))
    if availability_values != {
        "available",
        "stage_before_field_owner",
        "stage_before_emitter_owner",
        "stage_before_gpu_owner",
        "stage_without_gpu_owner",
        "stage_before_consumer_owner",
        "render_not_selected",
        "leg_not_selected",
        "headless_window_axis",
    }:
        raise ValueError("RtT-light projection availability values differ from rtt-light-v1")
    for group_name, group in groups.items():
        availability_column = group.get("availability_column")
        values = group.get("value_columns")
        stages = group.get("required_stages")
        legs = group.get("required_legs")
        renders = group.get("required_renders")
        if (
            availability_column not in projection_names
            or not isinstance(values, list)
            or not values
            or any(value not in projection_names for value in values)
            or not isinstance(stages, list)
            or not stages
            or any(stage not in RTT_LIGHT_STAGES for stage in stages)
            or len(stages) != len(set(stages))
            or not isinstance(legs, list)
            or not legs
            or any(leg not in {entry["leg_id"] for entry in expected_formal_legs} for leg in legs)
            or len(legs) != len(set(legs))
            or (
                renders is not None
                and (
                    not isinstance(renders, list)
                    or not renders
                    or any(render not in {"cpu", "gpu"} for render in renders)
                    or len(renders) != len(set(renders))
                )
            )
        ):
            raise ValueError(
                f"RtT-light projection group {group_name} applicability is invalid"
            )
        if availability_column in availability_columns or any(
            value in value_columns for value in values
        ):
            raise ValueError(f"RtT-light projection group {group_name} overlaps another group")
        availability_columns.add(availability_column)
        value_columns.update(values)
        for reason_key in (
            "not_applicable_reason",
            "stage_not_applicable_reason",
            "leg_not_applicable_reason",
            "render_not_applicable_reason",
        ):
            reason = group.get(reason_key)
            if reason is not None and reason not in availability_values - {"available"}:
                raise ValueError(
                    f"RtT-light projection group {group_name} has an unknown reason"
                )
        stage_reason_map = group.get("stage_not_applicable_reasons")
        if stage_reason_map is not None and (
            not isinstance(stage_reason_map, dict)
            or any(stage not in RTT_LIGHT_STAGES for stage in stage_reason_map)
            or any(
                reason not in availability_values - {"available"}
                for reason in stage_reason_map.values()
            )
        ):
            raise ValueError(
                f"RtT-light projection group {group_name} has invalid stage reasons"
            )
    typed_availability_columns = {
        column["name"]
        for column in projection_columns
        if column.get("type") == "availability"
    }
    if availability_columns != typed_availability_columns:
        raise ValueError("RtT-light projection availability columns are not grouped exactly")
    identity_columns = {
        "schema_version",
        "contract_id",
        "stage_id",
        "lane",
        "leg_id",
        "case_id",
    }
    if value_columns != set(projection_names) - identity_columns - availability_columns:
        raise ValueError("RtT-light projection value columns are not grouped exactly")
    columns_by_name = {column["name"]: column for column in projection_columns}
    for group_name, group in groups.items():
        can_be_unavailable = any(
            projection_field_applicability(
                contract, stage, case["leg_id"], case["case_id"]
            )[group_name]
            != "available"
            for stage in RTT_LIGHT_STAGES
            for case in expected_formal_cases(contract, stage)
        )
        for value_column in group["value_columns"]:
            is_optional = columns_by_name[value_column]["type"].endswith("_or_empty")
            if is_optional != can_be_unavailable:
                qualifier = "optional" if can_be_unavailable else "required"
                raise ValueError(
                    f"RtT-light projection {value_column} must be {qualifier} for its "
                    f"availability contract"
                )

    expected_case_counts = {
        "current": 18,
        "p01": 18,
        "p02": 18,
        "p03": 19,
        "p04": 19,
        "p05": 24,
        "p06": 24,
        "p07": 25,
        "p08": 25,
    }
    for stage, expected_count in expected_case_counts.items():
        formal_cases = expected_formal_cases(contract, stage)
        if len(formal_cases) != expected_count:
            raise ValueError(
                f"RtT-light stage {stage} expands to {len(formal_cases)} formal cases; "
                f"expected {expected_count}"
            )
        required_lanes = set(contract["stages"][stage]["required_lanes"])
        if len(contract["stages"][stage]["required_lanes"]) != len(required_lanes):
            raise ValueError(f"RtT-light stage {stage} repeats a required lane")
        if {case["lane"] for case in formal_cases} != required_lanes:
            raise ValueError(f"RtT-light stage {stage} formal cases differ from required lanes")
        for case in formal_cases:
            applicability = projection_field_applicability(
                contract, stage, case["leg_id"], case["case_id"]
            )
            if set(applicability) != set(groups) or any(
                value not in availability_values for value in applicability.values()
            ):
                raise ValueError(
                    f"RtT-light stage {stage} projection applicability is incomplete"
                )
    if projection_field_applicability(contract, "p06", "field-core")[
        "consumer_core"
    ] != "stage_before_consumer_owner":
        raise ValueError("RtT-light P06 consumer projection must be stage-inapplicable")
    if projection_field_applicability(contract, "p06", "renderdoc")[
        "gpu_upload"
    ] != "available":
        raise ValueError("RtT-light P06 GPU projection must be available")
    if projection_field_applicability(contract, "p07", "renderdoc")[
        "gpu_upload"
    ] != "stage_without_gpu_owner":
        raise ValueError("RtT-light P07 GPU projection must be stage-inapplicable")
    if projection_field_applicability(contract, "p07", "renderdoc")[
        "consumer_core"
    ] != "leg_not_selected":
        raise ValueError("RtT-light P07 consumer projection must be leg-inapplicable")

    gate_result = contract.get("gate_result")
    expected_gate_columns = [
        "gate_id",
        "stage_id",
        "case_id",
        "metric_id",
        "status",
        "unit",
        "observed",
        "comparator",
        "threshold",
        "reference_artifact",
        "subject_artifact",
        "reason_code",
    ]
    if not isinstance(gate_result, dict) or {
        "schema_version": gate_result.get("schema_version"),
        "file": gate_result.get("file"),
        "primary_key": gate_result.get("primary_key"),
        "columns": gate_result.get("columns"),
    } != {
        "schema_version": 1,
        "file": "data/rtt_light_gate_results.csv",
        "primary_key": ["gate_id", "stage_id", "case_id", "metric_id"],
        "columns": expected_gate_columns,
    }:
        raise ValueError("RtT-light gate-result schema differs from rtt-light-v1")
    if gate_result.get("statuses") != ["pass", "fail", "invalid", "not_applicable"]:
        raise ValueError("RtT-light gate-result statuses differ from rtt-light-v1")
    if gate_result.get("unit_types") != GATE_UNIT_TYPES:
        raise ValueError("RtT-light gate-result unit types differ from rtt-light-v1")
    if gate_result.get("reason_codes") != [
        "none",
        "missing_artifact",
        "schema_mismatch",
        "source_dirty",
        "source_changed",
        "run_count_mismatch",
        "invalid_run",
        "determinism_mismatch",
        "unexpected_log",
        "environment_mismatch",
        "missing_reference",
        "threshold_exceeded",
        "value_mismatch",
    ]:
        raise ValueError("RtT-light gate-result reason codes differ from rtt-light-v1")
    if gate_result.get("artifact_locators") != {
        "subject": (
            "rtt-light-v1/baseline-index.json#/stages/{stage_id}/cases/{case_id}"
        ),
        "reference": (
            "rtt-light-v1/baseline-index.json#/stages/"
            "{reference_stage_id}/cases/{case_id}"
        ),
    }:
        raise ValueError("RtT-light gate-result artifact locators differ from rtt-light-v1")

    gates = contract.get("gates", [])
    if not isinstance(gates, list) or not gates or any(
        not isinstance(gate, dict)
        or not isinstance(gate.get("gate_id"), str)
        or not gate["gate_id"]
        or gate.get("kind") not in {"bundle", "validity", "stage"}
        for gate in gates
    ):
        raise ValueError("RtT-light gate definitions are invalid")
    gate_ids = [gate["gate_id"] for gate in gates]
    if len(gate_ids) != len(set(gate_ids)):
        raise ValueError("RtT-light gate ids must be unique")
    bundle_ids = set(contract.get("gate_bundles", {}))
    if bundle_ids != {
        gate["gate_id"]
        for gate in contract.get("gates", [])
        if gate.get("kind") == "bundle"
    }:
        raise ValueError("RtT-light gate bundles differ from bundle gate definitions")
    expected_template_ids = set(gate_ids) - bundle_ids
    templates = gate_result.get("metric_templates")
    if not isinstance(templates, dict) or set(templates) != expected_template_ids:
        raise ValueError("RtT-light leaf gate metric templates are not exhaustive")
    used_case_sets = {
        template.get("case_set")
        for gate_templates in templates.values()
        for template in gate_templates
        if isinstance(template, dict)
    }
    if used_case_sets != set(gate_result.get("case_sets", {})):
        raise ValueError("RtT-light gate case sets must be used exactly")
    referenced_gates = {
        gate_id
        for stage_contract in contract["stages"].values()
        for gate_id in stage_contract.get("required_gate_ids", [])
    } | {
        member
        for members in contract.get("gate_bundles", {}).values()
        for member in members
    }
    if referenced_gates != set(gate_ids):
        raise ValueError("RtT-light gates must all be referenced exactly by a stage or bundle")
    comparators = {"eq", "le", "ge"}
    aggregations = {
        "exact",
        "count",
        "distinct-count",
        "sum",
        "all",
        "median-of-run-values",
        "max",
    }
    for gate_id, gate_templates in templates.items():
        if not isinstance(gate_templates, list) or not gate_templates:
            raise ValueError(f"RtT-light gate {gate_id} has no metric templates")
        for template in gate_templates:
            if (
                not isinstance(template, dict)
                or not isinstance(template.get("metric_id"), str)
                or not template["metric_id"]
                or not isinstance(template.get("unit"), str)
                or template.get("unit") not in GATE_UNIT_TYPES
                or template.get("aggregation") not in aggregations
                or template.get("comparator") not in comparators
                or template.get("case_set") not in gate_result.get("case_sets", {})
                or ("threshold" in template) == ("threshold_ref" in template)
                or "reference_required" in template
                or (
                    "reference_stage" in template
                    and template["reference_stage"] not in RTT_LIGHT_STAGES
                )
                or (
                    "reference_stage_by_render" in template
                    and (
                        not isinstance(template["reference_stage_by_render"], dict)
                        or set(template["reference_stage_by_render"]) != {"cpu", "gpu"}
                        or any(
                            reference_stage not in RTT_LIGHT_STAGES
                            for reference_stage in template[
                                "reference_stage_by_render"
                            ].values()
                        )
                    )
                )
                or (
                    "reference_stage" in template
                    and "reference_stage_by_render" in template
                )
                or (
                    "required_stages" in template
                    and (
                        not isinstance(template["required_stages"], list)
                        or not template["required_stages"]
                        or any(
                            stage not in RTT_LIGHT_STAGES
                            for stage in template["required_stages"]
                        )
                        or len(template["required_stages"])
                        != len(set(template["required_stages"]))
                    )
                )
            ):
                raise ValueError(f"RtT-light gate {gate_id} has an invalid metric template")
            expected_lineage = EXPECTED_GATE_REFERENCE_LINEAGE.get(
                (gate_id, template["metric_id"])
            )
            observed_lineage: object = None
            if "reference_stage" in template:
                observed_lineage = template["reference_stage"]
            elif "reference_stage_by_render" in template:
                observed_lineage = template["reference_stage_by_render"]
            if observed_lineage != expected_lineage:
                raise ValueError(
                    f"RtT-light gate {gate_id} metric {template['metric_id']} has "
                    "the wrong reference lineage"
                )
            required_stages = template.get("required_stages")
            if required_stages is not None:
                active_stages = {
                    stage
                    for stage in RTT_LIGHT_STAGES
                    if gate_id in expand_required_leaf_gates(contract, stage)
                }
                if not set(required_stages).issubset(active_stages):
                    raise ValueError(
                        f"RtT-light gate {gate_id} has a dead or inapplicable stage filter"
                    )
            active_stages = {
                stage
                for stage in RTT_LIGHT_STAGES
                if gate_id in expand_required_leaf_gates(contract, stage)
            }
            reference_stages = (
                {template["reference_stage"]}
                if "reference_stage" in template
                else set(template.get("reference_stage_by_render", {}).values())
            )
            if any(
                _stage_index(reference_stage)
                >= min(_stage_index(active_stage) for active_stage in active_stages)
                for reference_stage in reference_stages
            ):
                raise ValueError(
                    f"RtT-light gate {gate_id} reference lineage is not earlier than its owner"
                )
            threshold = _template_threshold(contract, gate_id, template)
            value_type = GATE_UNIT_TYPES[template["unit"]]
            if threshold in {
                "behavior-case-contract",
                "fixture-size-contract",
                "fixture-mask-contract",
            }:
                continue
            try:
                parsed_threshold = _parse_gate_scalar(threshold, value_type)
            except (TypeError, ValueError) as error:
                raise ValueError(
                    f"RtT-light gate {gate_id} metric {template['metric_id']} has an "
                    f"invalid threshold: {error}"
                ) from error
            if template["comparator"] != "eq" and isinstance(
                parsed_threshold, (bool, str)
            ):
                raise ValueError(
                    f"RtT-light gate {gate_id} metric {template['metric_id']} has an "
                    "incompatible comparator"
                )
    for stage in RTT_LIGHT_STAGES:
        expected_gate_result_rows(contract, stage)

    for size in ("small", "medium", "large"):
        size_contract = fixture["sizes"][size]
        expected_runtime_energy = {
            "small": (0.20000000298023224, 0.800000011920929),
            "medium": (2.000000238418579, 0.9999997615814209),
            "large": (9.999995231628418, 1.000004768371582),
        }[size]
        if (
            size_contract.get("runtime_f32_active_lamp_demand"),
            size_contract.get("runtime_f32_headroom"),
        ) != expected_runtime_energy:
            raise ValueError(f"{size} runtime f32 energy totals differ")
        layout = build_fixture_layout_unchecked(contract, size)
        if layout["counts"] != size_contract["expected_counts"]:
            raise ValueError(f"{size} fixture counts differ from expected_counts")
        expected_mask_checksum = canonical_sha256(
            {
                "cells": [
                    list(grid)
                    for grid in _room_floor_cells(
                        tuple(fixture["origin"]), size_contract["module_count"]
                    )
                ]
            }
        )
        if size_contract.get("indoor_mask_checksum") != expected_mask_checksum:
            raise ValueError(f"{size} indoor mask checksum differs from the fixture")
        if len(size_contract["door_states"]) != size_contract["module_count"] ** 2:
            raise ValueError(f"{size} door state count differs from room count")
        module_count = size_contract["module_count"]
        extent = module_count * 7
        origin = fixture["origin"]
        expected_main_yard = {
            "min": origin,
            "max": [origin[0] + extent, origin[1] + extent],
        }
        if size_contract.get("main_yard_grid_bounds") != expected_main_yard:
            raise ValueError(f"{size} main Yard bounds differ from the room fixture")
        control_yard = size_contract.get("control_yard_grid_bounds")
        unsupplied_cell = size_contract.get("unsupplied_lamp_cell")
        if control_yard != {"min": unsupplied_cell, "max": unsupplied_cell}:
            raise ValueError(f"{size} control Yard must contain only the unsupplied lamp")
        if (
            expected_main_yard["min"][0] <= unsupplied_cell[0] <= expected_main_yard["max"][0]
            and expected_main_yard["min"][1]
            <= unsupplied_cell[1]
            <= expected_main_yard["max"][1]
        ):
            raise ValueError(f"{size} control Yard overlaps the supplied fixture Yard")

        floor_cells = _room_floor_cells(tuple(origin), module_count)
        door_cells = _door_cells(tuple(origin), module_count)
        wall_cells = sorted(
            _boundary_cells(tuple(origin), module_count) - set(door_cells)
        )
        supplied_lamps = set(floor_cells[: size_contract["supplied_lamp_candidates"]])
        familiar_cells = {
            floor_cells[
                (fixture["actor_enumeration"]["familiar_floor_offset"] + index)
                % len(floor_cells)
            ]
            for index in range(size_contract["familiars"])
        }
        soul_spas = size_contract.get("soul_spas")
        if not isinstance(soul_spas, list) or len(soul_spas) != size_contract["operational_soul_spas"]:
            raise ValueError(f"{size} SoulSpa site count differs from operational_soul_spas")
        seen_workers: set[int] = set()
        seen_spa_tiles: set[tuple[int, int]] = set()
        for spa in soul_spas:
            anchor = spa.get("anchor")
            expected_tiles = [
                anchor,
                [anchor[0] + 1, anchor[1]],
                [anchor[0], anchor[1] - 1],
                [anchor[0] + 1, anchor[1] - 1],
            ]
            if spa.get("tiles") != expected_tiles:
                raise ValueError(
                    f"{size} SoulSpa tiles must match the production downward 2x2 geometry"
                )
            workers = spa.get("worker_ordinals")
            worker_tiles = spa.get("worker_tiles")
            if not isinstance(workers, list) or len(workers) != len(worker_tiles):
                raise ValueError(f"{size} SoulSpa workers and worker tiles differ")
            for tile in expected_tiles:
                tile_tuple = tuple(tile)
                if tile_tuple not in floor_cells or tile_tuple in seen_spa_tiles:
                    raise ValueError(f"{size} SoulSpa footprint is not a unique fixture floor")
                if tile_tuple in supplied_lamps:
                    raise ValueError(f"{size} SoulSpa footprint overlaps a supplied lamp")
                seen_spa_tiles.add(tile_tuple)
            for ordinal, worker_tile in zip(workers, worker_tiles, strict=True):
                if ordinal < 0 or ordinal >= size_contract["souls"] or ordinal in seen_workers:
                    raise ValueError(f"{size} SoulSpa worker ordinal is invalid or duplicated")
                if list(floor_cells[ordinal % len(floor_cells)]) != worker_tile:
                    raise ValueError(f"{size} SoulSpa worker does not start on its assigned tile")
                if worker_tile not in expected_tiles:
                    raise ValueError(f"{size} SoulSpa worker tile is outside its site")
                if tuple(worker_tile) in familiar_cells:
                    raise ValueError(f"{size} SoulSpa worker overlaps a Familiar start cell")
                seen_workers.add(ordinal)
        if len(seen_workers) != size_contract["generator_souls"]:
            raise ValueError(f"{size} generator Soul count differs from SoulSpa assignments")
        _validate_showcase_contract(
            fixture,
            size,
            size_contract,
            floor_cells,
            wall_cells,
            door_cells,
            supplied_lamps,
            seen_spa_tiles,
        )

    behavior_ids = [case["case_id"] for case in contract.get("behavior_cases", [])]
    if len(behavior_ids) != len(set(behavior_ids)) or len(behavior_ids) != 7:
        raise ValueError("RtT-light behavior case ids must be seven unique values")
    gate_ids = [gate["gate_id"] for gate in contract.get("gates", [])]
    if len(gate_ids) != len(set(gate_ids)) or not gate_ids:
        raise ValueError("RtT-light gate ids must be nonempty and unique")
    known_gates = set(gate_ids)
    for stage, stage_contract in contract["stages"].items():
        unknown_gates = set(stage_contract["required_gate_ids"]) - known_gates
        if unknown_gates:
            raise ValueError(
                f"stage {stage} references unknown gates: {', '.join(sorted(unknown_gates))}"
            )
        for lane in stage_contract["required_lanes"]:
            if lane not in RTT_LIGHT_LANES:
                raise ValueError(f"stage {stage} references unknown lane {lane}")


def build_fixture_layout_unchecked(contract: dict[str, Any], size: str) -> dict[str, Any]:
    """Build a layout while avoiding recursive top-level contract validation."""
    fixture = contract["fixture"]
    size_contract = fixture["sizes"][size]
    origin = tuple(fixture["origin"])
    module_count = size_contract["module_count"]
    floor_cells = _room_floor_cells(origin, module_count)
    boundary_cells = _boundary_cells(origin, module_count)
    door_cells = _door_cells(origin, module_count)
    return {
        "counts": {
            "rooms": module_count * module_count,
            "completed_floors": len(floor_cells),
            "completed_walls": len(boundary_cells - set(door_cells)),
            "doors": len(door_cells),
            "supplied_lamp_candidates": size_contract["supplied_lamp_candidates"],
            "unsupplied_lamp_candidates": 1,
            "souls": size_contract["souls"],
            "familiars": size_contract["familiars"],
            "room_boundary_lookup_cells": _room_boundary_lookup_cells(module_count),
            "yards": 2,
            "operational_soul_spas": len(size_contract["soul_spas"]),
            "generator_souls": sum(
                len(spa["worker_ordinals"]) for spa in size_contract["soul_spas"]
            ),
            **_showcase_counts(size_contract),
        }
    }


def contract_fingerprints(contract: dict[str, Any]) -> dict[str, str]:
    return {
        "measurement_contract_sha256": canonical_sha256(contract),
        "fixture_contract_sha256": canonical_sha256(contract["fixture"]),
    }
