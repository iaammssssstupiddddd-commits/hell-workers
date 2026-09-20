"""Dispatch workload sidecars without sharing production expectations."""
from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from ..model import Case
from .building_art_static import read_building_art_static
from .building_art_active import read_building_art_active
from . import (
    read_deconstruction_fixture,
    read_indoor_light_consumers,
    read_indoor_light_field,
    read_indoor_light_gpu,
    read_indoor_light_runtime,
    read_indoor_light_sidecars,
    read_save_transaction,
    read_wall_density_sidecars,
    read_door_density_sidecars,
)


@dataclass
class WorkloadSidecars:
    indoor_light_fixture: dict[str, str] | None = None
    indoor_light_layout: list[dict[str, str]] | None = None
    indoor_light_presentation: list[dict[str, str]] | None = None
    indoor_light_field: dict[str, Any] | None = None
    indoor_light_runtime: dict[str, Any] | None = None
    indoor_light_gpu: dict[str, Any] | None = None
    indoor_light_consumers: dict[str, Any] | None = None
    deconstruction_fixture: dict[str, str] | None = None
    save_transaction: dict[str, str] | None = None
    wall_density_fixture: dict[str, Any] | None = None
    wall_density_layout: list[dict[str, str]] | None = None
    door_density_fixture: dict[str, Any] | None = None
    door_density_layout: list[dict[str, str]] | None = None
    reasons: list[str] = field(default_factory=list)


def read_workload_sidecars(
    data_dir: Path,
    *,
    expected_case: Case,
    capture_kind: str,
    expected_contract: str | None,
    expected_stage: str | None,
    expected_lane: str | None,
) -> WorkloadSidecars:
    result = WorkloadSidecars()
    reasons = result.reasons
    indoor_sidecar_paths = tuple(data_dir / name for name in (
        "indoor_light_fixture.csv", "indoor_light_layout.csv", "indoor_light_presentation.csv",
    ))
    if expected_case.workload == "building-art-active":
        if capture_kind != "frame-time":
            reasons.append("building-art-active requires a frame-time capture")
        _, errors = read_building_art_active(data_dir, expected_case=expected_case)
        reasons.extend(errors)
    elif (data_dir / "building_art_active.json").exists():
        reasons.append("building-art-active sidecar is forbidden for another workload")
    if expected_case.workload == "building-art-static":
        if capture_kind != "frame-time":
            reasons.append("building-art-static requires a frame-time capture")
        _, errors = read_building_art_static(data_dir, expected_case=expected_case)
        reasons.extend(errors)
    elif (data_dir / "building_art_static.json").exists():
        reasons.append("building-art-static sidecar is forbidden for another workload")
    if expected_case.workload == "wall-density":
        if capture_kind != "frame-time":
            reasons.append("wall-density validation requires a frame-time capture")
        (
            result.wall_density_fixture,
            result.wall_density_layout,
            wall_density_errors,
        ) = read_wall_density_sidecars(data_dir, expected_case=expected_case)
        reasons.extend(wall_density_errors)
    elif expected_case.workload == "door-density":
        (
            result.door_density_fixture,
            result.door_density_layout,
            door_density_errors,
        ) = read_door_density_sidecars(data_dir, expected_case=expected_case)
        reasons.extend(door_density_errors)
    elif expected_case.workload == "indoor-light" and capture_kind == "consumer-core":
        if (
            expected_contract != "rtt-light-v1"
            or expected_stage not in {"p07", "p08"}
            or expected_lane != "consumer-core"
        ):
            reasons.append("consumer-core requires rtt-light-v1/p07|p08/consumer-core")
        result.indoor_light_consumers, consumer_errors = read_indoor_light_consumers(data_dir)
        reasons.extend(consumer_errors)
    elif expected_case.workload == "indoor-light" and capture_kind == "field-core":
        if (
            expected_contract != "rtt-light-v1"
            or expected_stage not in {"p03", "p04", "p05", "p06", "p07", "p08"}
            or expected_lane != "field-core"
        ):
            reasons.append("field-core requires rtt-light-v1/p03|p04|p05|p06|p07|p08/field-core")
        result.indoor_light_field, field_errors = read_indoor_light_field(data_dir)
        reasons.extend(field_errors)
        if expected_stage in {"p04", "p05", "p06", "p07", "p08"} and expected_contract is not None:
            result.indoor_light_runtime, runtime_errors = read_indoor_light_runtime(
                data_dir,
                expected_case=expected_case,
                contract_id=expected_contract,
                stage_id=expected_stage,
                field_core=True,
            )
            reasons.extend(runtime_errors)
        unexpected_sidecars = [path.name for path in indoor_sidecar_paths if path.exists()]
        if unexpected_sidecars:
            reasons.append("field-core must not write ECS fixture sidecars")
    elif expected_case.workload == "indoor-light":
        if expected_contract is None or expected_stage is None or expected_lane is None:
            reasons.append(
                "indoor-light validation requires expected contract, stage, and lane"
            )
        else:
            (
                result.indoor_light_fixture,
                result.indoor_light_layout,
                result.indoor_light_presentation,
                indoor_errors,
            ) = read_indoor_light_sidecars(
                data_dir,
                expected_case=expected_case,
                contract_id=expected_contract,
                stage_id=expected_stage,
                lane=expected_lane,
            )
            reasons.extend(indoor_errors)
            if expected_stage in {"p04", "p05", "p06", "p07", "p08"}:
                result.indoor_light_runtime, runtime_errors = read_indoor_light_runtime(
                    data_dir,
                    expected_case=expected_case,
                    contract_id=expected_contract,
                    stage_id=expected_stage,
                    field_core=False,
                )
                reasons.extend(runtime_errors)
                if (
                    expected_stage in {"p06", "p08"}
                    and expected_lane == "static"
                    and expected_case.render == "gpu"
                    and capture_kind == "frame-time"
                ):
                    result.indoor_light_gpu, gpu_errors = read_indoor_light_gpu(
                        data_dir,
                        runtime=result.indoor_light_runtime,
                    )
                    reasons.extend(gpu_errors)
    elif expected_case.workload == "deconstruction":
        if capture_kind != "fixed-step-determinism":
            reasons.append("deconstruction validation requires fixed-step determinism capture")
        result.deconstruction_fixture, deconstruction_errors = read_deconstruction_fixture(
            data_dir / "deconstruction_fixture.csv"
        )
        reasons.extend(deconstruction_errors)
    elif expected_case.workload == "save-transaction":
        if capture_kind != "frame-time":
            reasons.append("save-transaction validation requires frame-time capture")
        result.save_transaction, save_transaction_errors = read_save_transaction(
            data_dir / "save_transaction.csv"
        )
        reasons.extend(save_transaction_errors)
    return result
