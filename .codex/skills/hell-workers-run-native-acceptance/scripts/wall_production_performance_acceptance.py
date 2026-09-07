#!/usr/bin/env python3
"""Run the final Wall production versus fallback-control Capture pair."""

from __future__ import annotations

import argparse
import csv
import os
import re
import sys
from contextlib import contextmanager
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_acceptance as native  # noqa: E402
import wall_art_acceptance as art  # noqa: E402
import wall_density_acceptance as density  # noqa: E402


SCHEMA_VERSION = 2
# The runtime sidecar carries its own frozen schema from
# `wall_density_presentation.rs`; it is not versioned with this profile.
PRESENTATION_SIDECAR_SCHEMA_VERSION = 1
PROFILE = "wall-production-performance-v2"
PRESENTATIONS = ("fallback-control", "production")
EXPECTED_FAMILY_MULTIPLIERS = {
    "isolated": 1,
    "end": 4,
    "straight": 2,
    "corner": 4,
    "t_junction": 4,
    "cross": 1,
}
EXPECTED_ROTATION_MULTIPLIERS = {"0": 6, "1": 4, "2": 3, "3": 3}
CANDIDATE_ENV_KEYS = (
    "HW_WALL_CANDIDATE",
    "HW_WALL_CANDIDATE_GENERATION",
    "HW_WALL_CANDIDATE_MANIFEST_SHA256",
)
CAPTURE_ENV_KEYS = (*CANDIDATE_ENV_KEYS, "HW_WALL_PERF_PRESENTATION")
# A window the desktop compositor paces presents exactly at the display frame
# clock, so p95 / p99 stop describing wall cost: job
# `wall-production-performance-20260903T164122Z-3b424e6f` measured 8041 frames
# at a 7.43 ms median in its first run and then 3601 frames at 16.63 ms for
# every later run while single frames still finished in 7.60 ms. Every valid
# historical capture stayed unpaced. This rejects the paced regime itself and
# leaves the `+5%` acceptance thresholds untouched.
DISPLAY_FRAME_CLOCK_HZ = 60.0
FRAME_CLOCK_FPS_TOLERANCE = 1.0
FRAME_CLOCK_P50_TOLERANCE_MS = 0.5
# The three runs of one cell are captured minutes apart, so a mid-session
# regime flip has to fail even when each run is individually unpaced.
MAX_CELL_P50_SPREAD = 1.25
COMPLETED_CORE_ROLES = (
    "mesh:isolated",
    "mesh:end",
    "mesh:straight",
    "mesh:corner",
    "mesh:t_junction",
    "mesh:cross",
    "texture:albedo",
    "texture:emissive",
)


def inventory_contract(schema_version: Any, roles: tuple[Any, ...]) -> dict[str, Any]:
    if schema_version == 1:
        native.require(
            roles == COMPLETED_CORE_ROLES,
            "Wall schema-v1 performance inventory is not closed",
        )
        return {
            "runtime_schema_version": 1,
            "production_mesh_count": 6,
            "triangle_budgets": [72] * 6,
        }
    native.require(
        schema_version == 2 and roles == art.FORMWORK_PREVIEW_ROLES,
        "Wall schema-v2 performance inventory is not the closed formwork set",
    )
    return {
        "runtime_schema_version": 2,
        "production_mesh_count": 12,
        "triangle_budgets": [72] * 6 + [240] * 6,
    }


def runtime_inventory_contract(
    repo: Path, candidate: dict[str, Any]
) -> dict[str, Any]:
    """Resolve the finite mesh contract from the pinned runtime projection."""
    wallset = native.read_json(repo / "assets/manifests/wall-production-v1.wallset")
    core = wallset.get("core")
    native.require(
        wallset.get("authority") == candidate["authority"]
        and wallset.get("asset_set_generation")
        == candidate["asset_set_generation"]
        and wallset.get("manifest_sha256") == candidate["manifest_sha256"]
        and isinstance(core, list)
        and all(isinstance(record, dict) for record in core),
        "Wall performance runtime projection differs from its candidate identity",
    )
    roles = tuple(record.get("role") for record in core)
    return inventory_contract(wallset.get("schema_version"), roles)


def require_triangle_inventory(
    triangles: Any, maximum: Any, inventory: dict[str, Any]
) -> None:
    triangle_budgets = inventory["triangle_budgets"]
    native.require(
        isinstance(triangles, list)
        and len(triangles) == len(triangle_budgets)
        and all(
            type(value) is int and 0 < value <= budget
            for value, budget in zip(triangles, triangle_budgets, strict=True)
        )
        and maximum == max(triangles),
        "Wall production triangle budget differs",
    )


def presentation_command(
    repo: Path,
    root: Path,
    presentation: str,
    phase: str,
    adapter: str,
) -> list[str]:
    command = density.phase_command(repo, root, phase, adapter)
    command.extend(["--wall-presentation", presentation])
    return command


def presentation_environment(
    presentation: str, candidate: dict[str, Any]
) -> dict[str, str]:
    environment = os.environ.copy()
    for key in CANDIDATE_ENV_KEYS:
        environment.pop(key, None)
    environment["HW_WALL_PERF_PRESENTATION"] = presentation
    if presentation == "production":
        environment.update(
            {
                "HW_WALL_CANDIDATE": "1",
                "HW_WALL_CANDIDATE_GENERATION": str(
                    candidate["asset_set_generation"]
                ),
                "HW_WALL_CANDIDATE_MANIFEST_SHA256": candidate["manifest_sha256"],
            }
        )
    return environment


def capture_schedule() -> list[dict[str, Any]]:
    """Pair controls with production while counterbalancing which mode runs first."""
    schedule = []
    sequence = 0
    for run_number in range(1, density.REPEAT + 1):
        for phase_index, phase in enumerate(density.PHASES):
            for size_index, size in enumerate(density.SIZES):
                fallback_first = (run_number + phase_index + size_index) % 2 == 1
                presentations = (
                    PRESENTATIONS if fallback_first else tuple(reversed(PRESENTATIONS))
                )
                for presentation in presentations:
                    sequence += 1
                    schedule.append(
                        {
                            "sequence": sequence,
                            "presentation": presentation,
                            "phase": phase,
                            "size": size,
                            "run": run_number,
                        }
                    )
    return schedule


def frame_regime(samples: int, p50_ms: float) -> dict[str, Any]:
    """Describe whether a run was paced by the display instead of by the wall."""
    fps = samples / density.MEASURE_SECONDS
    period_ms = 1000.0 / DISPLAY_FRAME_CLOCK_HZ
    return {
        "samples": samples,
        "p50_ms": p50_ms,
        "fps": round(fps, 3),
        "display_paced": (
            abs(fps - DISPLAY_FRAME_CLOCK_HZ) <= FRAME_CLOCK_FPS_TOLERANCE
            and abs(p50_ms - period_ms) <= FRAME_CLOCK_P50_TOLERANCE_MS
        ),
    }


def run_frame_regime(session: Path, case_identifier: str, run_number: int) -> dict[str, Any]:
    run_dir = density.locate_run(session, case_identifier, run_number)
    summary = run_dir / "data" / "summary.csv"
    native.require(
        summary.is_file() and not summary.is_symlink(), "Wall run summary is absent"
    )
    with summary.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    native.require(len(rows) == 1, "Wall run summary is not a single case")
    try:
        samples = int(rows[0]["samples"])
        p50_ms = float(rows[0]["p50_ms"])
    except (KeyError, TypeError, ValueError) as error:
        raise native.AcceptanceError("Wall run summary lacks frame metrics") from error
    return frame_regime(samples, p50_ms)


def require_unpaced(regime: dict[str, Any], scheduled: dict[str, Any]) -> None:
    native.require(
        regime["display_paced"] is False,
        "Wall capture ran at the display frame clock instead of free running "
        f"({scheduled['presentation']}/{scheduled['phase']}/{scheduled['size']}"
        f"/run{scheduled['run']}: {regime['samples']} frames, "
        f"{regime['fps']} fps, p50 {regime['p50_ms']} ms). Frame times describe "
        "the compositor, not the wall; capture again in an unpaced regime.",
    )


def require_stable_cell(cell: tuple[str, str, str], regimes: list[dict[str, Any]]) -> None:
    medians = [regime["p50_ms"] for regime in regimes]
    lowest = min(medians)
    native.require(lowest > 0.0, "Wall capture reported a non-positive median")
    spread = max(medians) / lowest
    native.require(
        spread <= MAX_CELL_P50_SPREAD,
        "Wall capture regime changed between the runs of "
        f"{'/'.join(cell)} (p50 medians {medians}, spread {spread:.3f} exceeds "
        f"{MAX_CELL_P50_SPREAD})",
    )


@contextmanager
def selected_presentation_environment(
    presentation: str, candidate: dict[str, Any]
):
    replacement = presentation_environment(presentation, candidate)
    previous = {key: os.environ.get(key) for key in CAPTURE_ENV_KEYS}
    try:
        for key in CAPTURE_ENV_KEYS:
            value = replacement.get(key)
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value
        yield
    finally:
        for key, value in previous.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value


def expected_distribution(
    multipliers: dict[str, int], target_count: int
) -> dict[str, int]:
    native.require(target_count % 16 == 0, "Wall target count is not mask-balanced")
    repeat = target_count // 16
    return {key: value * repeat for key, value in multipliers.items()}


def verify_presentation_sidecar(
    path: Path,
    *,
    presentation: str,
    phase: str,
    size: str,
    candidate: dict[str, Any],
    inventory: dict[str, Any],
) -> dict[str, Any]:
    value = native.read_json(path)
    native.require(
        set(value) == {"schema_version", "stable", "initial", "final"},
        "Wall presentation sidecar fields differ",
    )
    native.require(
        value["schema_version"] == PRESENTATION_SIDECAR_SCHEMA_VERSION
        and value["stable"] is True,
        "Wall presentation sidecar is not stable",
    )
    initial = value["initial"]
    native.require(
        isinstance(initial, dict) and value["final"] == initial,
        "Wall presentation changed during capture",
    )
    target_count = 96 if size == "small" else 384
    expected_completed = 0 if phase == "provisional" else target_count
    expected_provisional = target_count if phase == "provisional" else 0
    if phase == "mixed":
        expected_completed = expected_provisional = target_count // 2
    native.require(
        initial.get("schema_version") == PRESENTATION_SIDECAR_SCHEMA_VERSION
        and initial.get("expected_mode") == presentation
        and initial.get("phase") == phase
        and initial.get("target_wall_count") == target_count
        and initial.get("completed_wall_count") == expected_completed
        and initial.get("provisional_wall_count") == expected_provisional
        and initial.get("visual_count") == target_count,
        "Wall presentation fixture identity differs",
    )
    expected_production = target_count if presentation == "production" else 0
    expected_fallback = target_count if presentation == "fallback-control" else 0
    mixed = phase == "mixed"
    if mixed:
        native.require(
            inventory["runtime_schema_version"] == 2 and size == "medium",
            "Wall mixed presentation is not the schema-v2 4N contract",
        )
    expected_active_materials = 2 if mixed else 1
    if presentation == "production":
        expected_active_meshes = 12 if mixed else 6
        expected_active_pairs = expected_active_meshes
    else:
        expected_active_meshes = 1
        expected_active_pairs = 2 if mixed else 1
    native.require(
        initial.get("production_count") == expected_production
        and initial.get("fallback_count") == expected_fallback
        and initial.get("active_mesh_count") == expected_active_meshes
        and initial.get("active_material_count") == expected_active_materials
        and initial.get("active_mesh_material_pair_count") == expected_active_pairs,
        "Wall presentation active residency differs",
    )
    native.require(
        initial.get("resident_production_mesh_count")
        == inventory["production_mesh_count"]
        and initial.get("resident_production_material_count") == 2
        and initial.get("resident_fallback_mesh_count") == 1
        and initial.get("resident_fallback_material_count") == 2
        and initial.get("total_wall_mesh_pool_count")
        == inventory["production_mesh_count"] + 1
        and initial.get("total_wall_material_pool_count") == 4
        and initial.get("production_materials_lit") is True,
        "Wall presentation finite pool differs",
    )
    triangles = initial.get("production_mesh_triangles")
    require_triangle_inventory(
        triangles,
        initial.get("max_production_mesh_triangles"),
        inventory,
    )
    native.require(
        initial.get("family_counts")
        == expected_distribution(EXPECTED_FAMILY_MULTIPLIERS, target_count)
        and initial.get("rotation_counts")
        == expected_distribution(EXPECTED_ROTATION_MULTIPLIERS, target_count),
        "Wall topology distribution differs",
    )
    native.require(
        initial.get("asset_set_generation") == candidate["asset_set_generation"]
        and initial.get("authority") == candidate["authority"]
        and initial.get("manifest_sha256") == candidate["manifest_sha256"],
        "Wall presentation candidate identity differs",
    )
    for field in (
        "session_id",
        "readiness_revision",
        "decision_revision",
        "presentation_revision",
    ):
        native.require(
            type(initial.get(field)) is int and initial[field] >= 0,
            f"Wall presentation {field} is invalid",
        )
    return value


def verify_session_presentations(
    session: Path,
    *,
    repo: Path,
    presentation: str,
    phase: str,
    candidate: dict[str, Any],
) -> list[dict[str, Any]]:
    Case, _ = density.load_perf_modules(repo)
    inventory = runtime_inventory_contract(repo, candidate)
    sidecars = []
    for size in density.SIZES:
        case = Case(
            "wall-density", size, "gpu", density.SEED, 0, 0, wall_phase=phase
        )
        for run_number in range(1, density.REPEAT + 1):
            run = density.locate_run(session, case.identifier, run_number)
            sidecars.append(
                verify_presentation_sidecar(
                    run / "data" / "wall_density_presentation.json",
                    presentation=presentation,
                    phase=phase,
                    size=size,
                    candidate=candidate,
                    inventory=inventory,
                )
            )
    return sidecars


def comparison_command(
    baseline: Path, candidate: Path, metric: str, output: Path
) -> list[str]:
    return [
        "python3",
        "scripts/perf.py",
        "compare",
        "--baseline",
        str(baseline),
        "--candidate",
        str(candidate),
        "--metric",
        metric,
        "--max-regression-pct",
        "5",
        "--min-runs",
        "3",
        "--output",
        str(output),
    ]


def interleaved_command(
    *,
    repo: Path,
    root: Path,
    subject_commit: str,
    source_fingerprint: str,
    harness_fingerprint: str,
    asset_view_fingerprint: str,
    candidate: dict[str, Any],
    adapter: str,
) -> list[str]:
    return [
        "python3",
        str(Path(__file__).resolve()),
        "capture-interleaved",
        "--repo",
        str(repo),
        "--job-root",
        str(root),
        "--subject-commit",
        subject_commit,
        "--source-fingerprint",
        source_fingerprint,
        "--harness-fingerprint",
        harness_fingerprint,
        "--asset-view-fingerprint",
        asset_view_fingerprint,
        "--candidate-generation",
        str(candidate["asset_set_generation"]),
        "--candidate-manifest-sha256",
        candidate["manifest_sha256"],
        "--adapter",
        adapter,
    ]


def comparison_evidence(path: Path, *, phase: str, metric: str) -> dict[str, Any]:
    native.require(path.is_file() and not path.is_symlink(), "Wall comparison is absent")
    with path.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    native.require(len(rows) == len(density.SIZES), "Wall comparison case count differs")
    expected_cases = {
        f"wall-density-{size}-gpu-seed-{density.SEED}-souls-0-familiars-0-wall-{phase}"
        for size in density.SIZES
    }
    native.require(
        {row.get("case_id") for row in rows} == expected_cases,
        "Wall comparison cases differ",
    )
    for row in rows:
        native.require(
            row.get("metric") == metric and row.get("regression") == "false",
            "Wall comparison exceeds the five-percent gate",
        )
        try:
            baseline_ms = float(row["baseline_ms"])
            production_ms = float(row["candidate_ms"])
            delta_pct = float(row["delta_pct"])
        except (KeyError, TypeError, ValueError) as error:
            raise native.AcceptanceError("Wall comparison values are invalid") from error
        native.require(
            baseline_ms > 0.0 and production_ms >= 0.0 and delta_pct <= 5.0,
            "Wall comparison values exceed the formal gate",
        )
    return {
        "phase": phase,
        "metric": metric,
        "file": path.name,
        "sha256": density.sha256(path),
        "rows": rows,
    }


def case_identifiers(repo: Path, phase: str) -> dict[str, str]:
    Case, _ = density.load_perf_modules(repo)
    return {
        size: Case(
            "wall-density", size, "gpu", density.SEED, 0, 0, wall_phase=phase
        ).identifier
        for size in density.SIZES
    }


def session_index(
    repo: Path, root: Path
) -> dict[tuple[str, str], tuple[Path, dict[str, str]]]:
    return {
        (presentation, phase): (
            root / presentation / "sessions" / phase,
            case_identifiers(repo, phase),
        )
        for presentation in PRESENTATIONS
        for phase in density.PHASES
    }


def capture_order_evidence(
    path: Path,
    *,
    sessions: dict[tuple[str, str], tuple[Path, dict[str, str]]] | None = None,
) -> dict[str, Any]:
    value = native.read_json(path)
    native.require(
        set(value) == {"schema_version", "profile", "status", "schedule", "completed"}
        and value["schema_version"] == SCHEMA_VERSION
        and value["profile"] == PROFILE
        and value["status"] == "pass"
        and value["schedule"] == capture_schedule(),
        "Wall production capture order differs",
    )
    completed = value["completed"]
    native.require(
        isinstance(completed, list) and len(completed) == len(value["schedule"]),
        "Wall production capture completion count differs",
    )
    cells: dict[tuple[str, str, str], list[dict[str, Any]]] = {}
    for expected, observed in zip(value["schedule"], completed, strict=True):
        native.require(
            isinstance(observed, dict)
            and set(observed) == {*expected, "started_at", "completed_at", "regime"}
            and all(observed[key] == expected[key] for key in expected)
            and isinstance(observed["started_at"], str)
            and isinstance(observed["completed_at"], str),
            "Wall production completed capture order differs",
        )
        regime = observed["regime"]
        native.require(
            isinstance(regime, dict)
            and set(regime) == {"samples", "p50_ms", "fps", "display_paced"}
            and regime == frame_regime(regime["samples"], regime["p50_ms"]),
            "Wall production capture regime is not self-consistent",
        )
        if sessions is not None:
            session, identifiers = sessions[
                (observed["presentation"], observed["phase"])
            ]
            native.require(
                run_frame_regime(
                    session, identifiers[observed["size"]], observed["run"]
                )
                == regime,
                "Wall production capture regime differs from the raw run artifacts",
            )
        require_unpaced(regime, observed)
        cells.setdefault(
            (observed["presentation"], observed["phase"], observed["size"]), []
        ).append(regime)
    for cell, regimes in cells.items():
        native.require(
            len(regimes) == density.REPEAT, "Wall production capture cell is incomplete"
        )
        require_stable_cell(cell, regimes)
    return {
        "file": path.name,
        "sha256": density.sha256(path),
        "runs": len(completed),
        "pairing": "adjacent-counterbalanced",
        "regime": "unpaced",
        "slowest_fps": min(entry["regime"]["fps"] for entry in completed),
    }


def load_interleaved_modules(repo: Path) -> tuple[Any, Any, Any, Any, Any]:
    scripts = str(repo / "scripts")
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    from perf_tool.arguments import build_parser, validate_arguments
    from perf_tool.execution import prepare_session, run_one
    from perf_tool.model import Case
    from perf_tool.summary import summarize_session

    return build_parser, validate_arguments, prepare_session, run_one, (Case, summarize_session)


def capture_interleaved(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "Wall subject changed")
    native.require(
        native.source_fingerprint(repo) == args.source_fingerprint,
        "Wall source changed before interleaved capture",
    )
    native.require(
        native.native_harness_fingerprint(repo) == args.harness_fingerprint,
        "Wall harness changed before interleaved capture",
    )
    density.assert_fully_clean(repo, args.subject_commit)
    native.require(
        density.asset_view_fingerprint(repo) == args.asset_view_fingerprint,
        "Wall asset view changed before interleaved capture",
    )
    candidate = art.candidate_identity(repo)
    native.require(
        int(args.candidate_generation) == candidate["asset_set_generation"]
        and args.candidate_manifest_sha256 == candidate["manifest_sha256"],
        "Wall candidate changed before interleaved capture",
    )
    binary = repo / "target/profiling/bevy_app"
    native.require(binary.is_file() and not binary.is_symlink(), "profiling binary missing")
    root = Path(args.job_root).resolve()
    native.require(root.is_dir() and not root.is_symlink(), "Wall job root is unavailable")

    build_parser, validate_arguments, prepare_session, run_one, loaded = (
        load_interleaved_modules(repo)
    )
    Case, summarize_session = loaded
    sessions: dict[tuple[str, str], tuple[Any, Path, dict[str, Any]]] = {}
    for presentation in PRESENTATIONS:
        for phase in density.PHASES:
            command = presentation_command(
                repo, root / presentation, presentation, phase, args.adapter
            )
            perf_args = build_parser().parse_args(command[2:])
            cases = {
                size: Case(
                    "wall-density",
                    size,
                    "gpu",
                    density.SEED,
                    0,
                    0,
                    wall_phase=phase,
                )
                for size in density.SIZES
            }
            with selected_presentation_environment(presentation, candidate):
                validate_arguments(perf_args)
                session = prepare_session(
                    perf_args,
                    binary,
                    list(cases.values()),
                    args.source_fingerprint,
                )
            sessions[(presentation, phase)] = (perf_args, session, cases)

    order_path = root / "capture-order.json"
    order = {
        "schema_version": SCHEMA_VERSION,
        "profile": PROFILE,
        "status": "running",
        "schedule": capture_schedule(),
        "completed": [],
    }
    native.atomic_write_json(order_path, order)
    for scheduled in order["schedule"]:
        presentation = scheduled["presentation"]
        phase = scheduled["phase"]
        perf_args, session, cases = sessions[(presentation, phase)]
        native.require(
            native.source_fingerprint(repo) == args.source_fingerprint,
            "source changed before Wall capture",
        )
        native.require(
            density.asset_view_fingerprint(repo) == args.asset_view_fingerprint,
            "asset view changed before Wall capture",
        )
        completed = {**scheduled, "started_at": native.utc_now()}
        with selected_presentation_environment(presentation, candidate):
            validation = run_one(
                args=perf_args,
                binary=binary,
                session_dir=session,
                case=cases[scheduled["size"]],
                run_number=scheduled["run"],
                preflight=False,
            )
        native.require(
            validation.valid,
            "Wall interleaved capture failed raw validation: "
            + "; ".join(validation.reasons),
        )
        regime = run_frame_regime(
            session, cases[scheduled["size"]].identifier, scheduled["run"]
        )
        require_unpaced(regime, scheduled)
        completed["completed_at"] = native.utc_now()
        completed["regime"] = regime
        order["completed"].append(completed)
        native.atomic_write_json(order_path, order)
        cell = (presentation, phase, scheduled["size"])
        observed = [
            entry["regime"]
            for entry in order["completed"]
            if (entry["presentation"], entry["phase"], entry["size"]) == cell
        ]
        if len(observed) == density.REPEAT:
            require_stable_cell(cell, observed)

    for _, session, _ in sessions.values():
        native.require(summarize_session(session), "Wall performance session is invalid")
    order["status"] = "pass"
    native.atomic_write_json(order_path, order)
    native.print_json(
        capture_order_evidence(order_path, sessions=session_index(repo, root))
    )
    return 0


def verify_root(root: Path) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    native.require(
        manifest.get("schema_version") == SCHEMA_VERSION
        and manifest.get("status") == "pass"
        and manifest.get("profile") == PROFILE,
        "Wall production performance manifest differs",
    )
    repo = native.validate_repo(manifest["repo"])
    density.assert_fully_clean(repo, manifest["subject_commit"])
    native.require(
        native.source_fingerprint(repo) == manifest["source_fingerprint"]
        and native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"]
        and density.asset_view_fingerprint(repo) == manifest["asset_view_fingerprint"],
        "Wall production performance provenance changed",
    )
    candidate = art.candidate_identity(repo)
    native.require(
        candidate == manifest.get("candidate_identity"),
        "Wall production performance candidate changed",
    )
    binary = repo / "target/profiling/bevy_app"
    native.require(
        density.sha256(binary) == manifest["binary_sha256"],
        "Wall production performance binary changed",
    )
    sessions = manifest.get("sessions")
    native.require(
        isinstance(sessions, dict) and set(sessions) == set(PRESENTATIONS),
        "Wall production performance session modes differ",
    )
    for presentation in PRESENTATIONS:
        entries = sessions[presentation]
        native.require(
            isinstance(entries, list) and len(entries) == len(density.PHASES),
            "Wall production performance phase set differs",
        )
        for entry, phase in zip(entries, density.PHASES, strict=True):
            session = root / presentation / "sessions" / phase
            recalculated = density.verify_session(
                repo=repo,
                session=session,
                phase=phase,
                adapter=manifest["adapter"],
                subject_commit=manifest["subject_commit"],
                source_fingerprint=manifest["source_fingerprint"],
                binary_sha256=manifest["binary_sha256"],
            )
            recalculated["presentation"] = presentation
            recalculated["presentation_sidecars"] = len(
                verify_session_presentations(
                    session,
                    repo=repo,
                    presentation=presentation,
                    phase=phase,
                    candidate=candidate,
                )
            )
            native.require(entry == recalculated, "Wall presentation session differs")
    native.require(
        manifest.get("capture_order")
        == capture_order_evidence(
            root / "capture-order.json", sessions=session_index(repo, root)
        ),
        "Wall production capture-order evidence changed",
    )
    recorded_comparisons = manifest.get("comparisons")
    native.require(
        isinstance(recorded_comparisons, list)
        and len(recorded_comparisons) == len(density.PHASES) * 2,
        "Wall comparison manifest differs",
    )
    recalculated_comparisons = []
    for phase in density.PHASES:
        for metric in ("p95", "p99"):
            path = root / "comparisons" / f"fallback-control-vs-production-{phase}-{metric}.csv"
            recalculated_comparisons.append(
                comparison_evidence(path, phase=phase, metric=metric)
            )
    native.require(
        recorded_comparisons == recalculated_comparisons,
        "Wall comparison evidence changed",
    )
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "root": str(root),
        "capture_runs": len(PRESENTATIONS)
        * len(density.PHASES)
        * len(density.SIZES)
        * density.REPEAT,
        "comparisons": len(density.PHASES) * 2,
    }


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    failures.extend(
        f"Wall performance is missing runtime asset {relative}"
        for relative in density.missing_runtime_assets(repo)
    )
    candidate: dict[str, Any] | None = None
    try:
        candidate = art.candidate_identity(repo)
    except native.AcceptanceError as error:
        failures.append(str(error))
    subject = native.git_subject(repo)
    source = native.source_fingerprint(repo)
    harness = native.native_harness_fingerprint(repo)
    assets = density.asset_view_fingerprint(repo)
    try:
        density.assert_fully_clean(repo, subject)
    except native.AcceptanceError as error:
        failures.append(str(error))
    root = (
        Path(args.job_root).resolve()
        if args.job_root
        else native.unique_job_root(repo, "wall-production-performance")
    )
    if root.exists():
        failures.append(f"job root already exists: {root}")
    command = [
        "kitty",
        "--directory",
        str(repo),
        "--detach",
        "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1",
        "PYTHONDONTWRITEBYTECODE=1",
        "python3",
        str(Path(__file__).resolve()),
        "run",
        "--repo",
        str(repo),
        "--job-root",
        str(root),
        "--subject-commit",
        subject,
        "--source-fingerprint",
        source,
        "--harness-fingerprint",
        harness,
        "--asset-view-fingerprint",
        assets,
        "--candidate-generation",
        str(candidate["asset_set_generation"] if candidate else 0),
        "--candidate-manifest-sha256",
        candidate["manifest_sha256"] if candidate else "invalid",
        "--adapter",
        args.adapter,
    ]
    native.print_json(
        {
            "schema_version": SCHEMA_VERSION,
            "status": "ready" if not failures else "blocked",
            "profile": PROFILE,
            "job_root": str(root),
            "subject_commit": subject,
            "source_fingerprint": source,
            "harness_fingerprint": harness,
            "asset_view_fingerprint": assets,
            "candidate_identity": candidate,
            "adapter": args.adapter,
            "failures": failures,
            "resources": resources,
            "launcher_command": command,
            "status_command": [
                "python3",
                str(Path(__file__).resolve()),
                "status",
                "--job-root",
                str(root),
            ],
            "execution_contract": {
                "actual_window_required": True,
                "parallel_game_processes": 1,
                "capture_runs": 24,
                "comparisons": 4,
                "pairing": "adjacent-counterbalanced",
            },
        }
    )
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "Wall production performance must use the planned kitty command",
    )
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "Wall subject changed")
    native.require(
        native.source_fingerprint(repo) == args.source_fingerprint,
        "Wall source changed",
    )
    native.require(
        native.native_harness_fingerprint(repo) == args.harness_fingerprint,
        "Wall harness changed",
    )
    density.assert_fully_clean(repo, args.subject_commit)
    native.require(
        density.asset_view_fingerprint(repo) == args.asset_view_fingerprint,
        "Wall asset view changed",
    )
    candidate = art.candidate_identity(repo)
    native.require(
        int(args.candidate_generation) == candidate["asset_set_generation"]
        and args.candidate_manifest_sha256 == candidate["manifest_sha256"],
        "Planned Wall candidate identity changed",
    )
    root = Path(args.job_root).resolve()
    native.require(not root.exists(), f"job root already exists: {root}")
    root.mkdir(parents=True)
    state: dict[str, Any] = {
        "schema_version": SCHEMA_VERSION,
        "status": "running",
        "profile": PROFILE,
        "subject_commit": args.subject_commit,
        "source_fingerprint": args.source_fingerprint,
        "harness_fingerprint": args.harness_fingerprint,
        "asset_view_fingerprint": args.asset_view_fingerprint,
        "candidate_identity": candidate,
        "adapter": args.adapter,
        "started_at": native.utc_now(),
        "current_stage": "build",
        "child_pid": None,
        "commands": [],
    }
    native.atomic_write_json(root / "job.json", state)
    try:
        native.run_command(
            "build",
            density.build_command(),
            repo=repo,
            env=os.environ.copy(),
            log_path=root / "build.log",
            job_file=root / "job.json",
            state=state,
        )
        binary = repo / "target/profiling/bevy_app"
        native.require(binary.is_file() and not binary.is_symlink(), "profiling binary missing")
        binary_hash = density.sha256(binary)
        command = interleaved_command(
            repo=repo,
            root=root,
            subject_commit=args.subject_commit,
            source_fingerprint=args.source_fingerprint,
            harness_fingerprint=args.harness_fingerprint,
            asset_view_fingerprint=args.asset_view_fingerprint,
            candidate=candidate,
            adapter=args.adapter,
        )
        state["commands"].append({"stage": "capture-interleaved", "argv": command})
        native.run_command(
            "capture-interleaved",
            command,
            repo=repo,
            env=os.environ.copy(),
            log_path=root / "capture-interleaved.log",
            job_file=root / "job.json",
            state=state,
            timeout_seconds=density.RUN_TIMEOUT_SECONDS * len(capture_schedule()) + 600.0,
        )
        sessions: dict[str, list[dict[str, Any]]] = {}
        for presentation in PRESENTATIONS:
            entries = []
            for phase in density.PHASES:
                session = root / presentation / "sessions" / phase
                entry = density.verify_session(
                    repo=repo,
                    session=session,
                    phase=phase,
                    adapter=args.adapter,
                    subject_commit=args.subject_commit,
                    source_fingerprint=args.source_fingerprint,
                    binary_sha256=binary_hash,
                )
                entry["presentation"] = presentation
                entry["presentation_sidecars"] = len(
                    verify_session_presentations(
                        session,
                        repo=repo,
                        presentation=presentation,
                        phase=phase,
                        candidate=candidate,
                    )
                )
                entries.append(entry)
            sessions[presentation] = entries
        capture_order = capture_order_evidence(
            root / "capture-order.json", sessions=session_index(repo, root)
        )
        comparisons = root / "comparisons"
        comparisons.mkdir()
        comparison_results = []
        for phase in density.PHASES:
            for metric in ("p95", "p99"):
                output = comparisons / f"fallback-control-vs-production-{phase}-{metric}.csv"
                command = comparison_command(
                    root / "fallback-control" / "sessions" / phase,
                    root / "production" / "sessions" / phase,
                    metric,
                    output,
                )
                native.run_command(
                    f"compare-{phase}-{metric}",
                    command,
                    repo=repo,
                    env=os.environ.copy(),
                    log_path=root / f"compare-{phase}-{metric}.log",
                    job_file=root / "job.json",
                    state=state,
                )
                comparison_results.append(
                    comparison_evidence(output, phase=phase, metric=metric)
                )
        manifest = {
            "schema_version": SCHEMA_VERSION,
            "status": "pass",
            "profile": PROFILE,
            "repo": str(repo),
            "subject_commit": args.subject_commit,
            "source_fingerprint": args.source_fingerprint,
            "harness_fingerprint": args.harness_fingerprint,
            "asset_view_fingerprint": args.asset_view_fingerprint,
            "candidate_identity": candidate,
            "adapter": args.adapter,
            "binary_sha256": binary_hash,
            "sessions": sessions,
            "capture_order": capture_order,
            "comparisons": comparison_results,
            "completed_at": native.utc_now(),
        }
        native.atomic_write_json(root / "manifest.json", manifest)
        state.update({"status": "valid", "completed_at": native.utc_now(), "child_pid": None})
        native.atomic_write_json(root / "job.json", state)
        native.print_json(verify_root(root))
        return 0
    except Exception as error:
        state.update(
            {
                "status": "invalid",
                "failure": f"{type(error).__name__}: {error}",
                "completed_at": native.utc_now(),
                "child_pid": None,
            }
        )
        native.atomic_write_json(root / "job.json", state)
        raise


def status(args: argparse.Namespace) -> int:
    job = native.read_json(Path(args.job_root).resolve() / "job.json")
    native.print_json(job)
    return 2 if job.get("status") == "running" else 0 if job.get("status") == "valid" else 1


def self_test() -> int:
    candidate = {
        "authority": "isolated_candidate",
        "asset_set_generation": 2,
        "manifest_sha256": "a" * 64,
    }
    production = presentation_environment("production", candidate)
    fallback = presentation_environment("fallback-control", candidate)
    native.require(
        production["HW_WALL_CANDIDATE"] == "1"
        and "HW_WALL_CANDIDATE" not in fallback
        and production["HW_WALL_PERF_PRESENTATION"] == "production"
        and fallback["HW_WALL_PERF_PRESENTATION"] == "fallback-control",
        "Wall performance environments are not isolated",
    )
    command = presentation_command(
        Path("/repo"), Path("/job"), "production", "completed", "Intel"
    )
    native.require(
        command[command.index("--wall-presentation") + 1] == "production",
        "Wall presentation command is not explicit",
    )
    build_parser, validate_arguments, _, _, _ = load_interleaved_modules(Path.cwd())
    perf_args = build_parser().parse_args(command[2:])
    with selected_presentation_environment("production", candidate):
        validate_arguments(perf_args)
    schedule = capture_schedule()
    native.require(len(schedule) == 24, "Wall capture schedule length differs")
    for first, second in zip(schedule[::2], schedule[1::2], strict=True):
        native.require(
            (first["phase"], first["size"], first["run"])
            == (second["phase"], second["size"], second["run"])
            and {first["presentation"], second["presentation"]} == set(PRESENTATIONS),
            "Wall control and production captures are not adjacent pairs",
        )
    first_modes = [entry["presentation"] for entry in schedule[::2]]
    native.require(
        first_modes.count("fallback-control") == first_modes.count("production") == 6,
        "Wall capture pair order is not counterbalanced",
    )
    schema_v1 = inventory_contract(1, COMPLETED_CORE_ROLES)
    schema_v2 = inventory_contract(2, art.FORMWORK_PREVIEW_ROLES)
    native.require(
        schema_v1["production_mesh_count"] == len(schema_v1["triangle_budgets"])
        and schema_v2["production_mesh_count"]
        == len(schema_v2["triangle_budgets"])
        and max(schema_v1["triangle_budgets"]) == 72
        and schema_v2["triangle_budgets"][:6] == [72] * 6
        and schema_v2["triangle_budgets"][6:] == [240] * 6,
        "Wall runtime inventory budgets differ",
    )
    require_triangle_inventory([72] * 6, 72, schema_v1)
    require_triangle_inventory([72] * 6 + [240] * 6, 240, schema_v2)
    for invalid_triangles, invalid_maximum in (
        ([73] + [72] * 5 + [240] * 6, 240),
        ([72] * 6 + [241] + [240] * 5, 241),
        ([72] * 6 + [240] * 5, 240),
        ([72] * 6 + [240] * 6, 239),
    ):
        try:
            require_triangle_inventory(invalid_triangles, invalid_maximum, schema_v2)
        except native.AcceptanceError:
            pass
        else:
            raise native.AcceptanceError("Wall invalid triangle inventory was accepted")
    for schema_version, roles in (
        (1, COMPLETED_CORE_ROLES[:-1]),
        (2, art.FORMWORK_PREVIEW_ROLES[:-1]),
        (3, art.FORMWORK_PREVIEW_ROLES),
    ):
        try:
            inventory_contract(schema_version, roles)
        except native.AcceptanceError:
            pass
        else:
            raise native.AcceptanceError("Wall open runtime inventory was accepted")
    # Observed regimes: a free running capture and the 60 Hz paced capture that
    # replaced it inside job `wall-production-performance-20260903T164122Z-3b424e6f`.
    free_running = frame_regime(8041, 7.43)
    paced = frame_regime(3601, 16.63)
    native.require(
        free_running["display_paced"] is False and paced["display_paced"] is True,
        "Wall frame regime does not separate free running from display paced",
    )
    scheduled = schedule[0]
    require_unpaced(free_running, scheduled)
    try:
        require_unpaced(paced, scheduled)
    except native.AcceptanceError:
        pass
    else:
        raise native.AcceptanceError("Wall display paced capture was accepted")
    cell = ("production", "completed", "small")
    require_stable_cell(cell, [free_running, frame_regime(7900, 7.60)])
    try:
        require_stable_cell(cell, [free_running, paced])
    except native.AcceptanceError:
        pass
    else:
        raise native.AcceptanceError("Wall mid-session regime flip was accepted")
    native.print_json(
        {"schema_version": SCHEMA_VERSION, "status": "pass", "profile": PROFILE}
    )
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    plan_parser = commands.add_parser("plan")
    plan_parser.add_argument("--repo", required=True)
    plan_parser.add_argument("--job-root")
    plan_parser.add_argument("--adapter", default="Intel")
    run_parser = commands.add_parser("run")
    capture_parser = commands.add_parser("capture-interleaved")
    for command_parser in (run_parser, capture_parser):
        command_parser.add_argument("--repo", required=True)
        command_parser.add_argument("--job-root", required=True)
        command_parser.add_argument("--subject-commit", required=True)
        command_parser.add_argument("--source-fingerprint", required=True)
        command_parser.add_argument("--harness-fingerprint", required=True)
        command_parser.add_argument("--asset-view-fingerprint", required=True)
        command_parser.add_argument("--candidate-generation", required=True)
        command_parser.add_argument("--candidate-manifest-sha256", required=True)
        command_parser.add_argument("--adapter", default="Intel")
    status_parser = commands.add_parser("status")
    status_parser.add_argument("--job-root", required=True)
    verify_parser = commands.add_parser("verify")
    verify_parser.add_argument("--job-root", required=True)
    commands.add_parser("self-test")
    return root


def main() -> int:
    args = parser().parse_args()
    if args.command == "plan":
        return plan(args)
    if args.command in {"run", "capture-interleaved"}:
        for value, label, length in (
            (args.subject_commit, "subject commit", 40),
            (args.source_fingerprint, "source fingerprint", 64),
            (args.harness_fingerprint, "harness fingerprint", 64),
            (args.asset_view_fingerprint, "asset-view fingerprint", 64),
            (args.candidate_manifest_sha256, "candidate manifest", 64),
        ):
            native.require(
                re.fullmatch(f"[0-9a-f]{{{length}}}", value) is not None,
                f"invalid {label}",
            )
        return run(args) if args.command == "run" else capture_interleaved(args)
    if args.command == "status":
        return status(args)
    if args.command == "verify":
        native.print_json(verify_root(Path(args.job_root).resolve()))
        return 0
    if args.command == "self-test":
        return self_test()
    raise native.AcceptanceError(f"unsupported command: {args.command}")


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        native.print_json(
            {"schema_version": SCHEMA_VERSION, "status": "invalid", "error": str(error)}
        )
        raise SystemExit(1) from error
