#!/usr/bin/env python3
"""Capture, replay, and verify the wall-density-v1 RenderDoc matrix."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_acceptance as native  # noqa: E402
import wall_art_acceptance as art  # noqa: E402
import wall_density_acceptance as density  # noqa: E402


SCHEMA_VERSION = 1
PROFILE = "wall-renderdoc-v1"
CONTRACT_ID = "wall-density-v1"
PHASES = ("completed", "provisional")
SIZES = ("small", "medium")
TARGET_COUNTS = {"small": 96, "medium": 384}
SEED = 20_260_901
WINDOW_WIDTH = 1280
WINDOW_HEIGHT = 720
WINDOW_SCALE_FACTOR = 1.0
CAPTURE_TIMEOUT_SECONDS = 600.0
REPLAY_TIMEOUT_SECONDS = 600.0
RENDERDOC_PROFILE = "profiling-renderdoc"
RENDERDOC_FEATURES = "profiling-renderdoc"
EXTRACTOR_RELATIVE = "scripts/perf_tool/wall_renderdoc_extract.py"
REQUIRED_RUNTIME_ASSETS = (
    "assets/fonts/SourceSerif4-VF.ttf",
    "assets/textures/terrain/mud_floor.png",
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def missing_runtime_assets(repo: Path) -> list[str]:
    return [relative for relative in REQUIRED_RUNTIME_ASSETS if not (repo / relative).is_file()]


def assert_clean(repo: Path, subject_commit: str) -> None:
    native.assert_clean_subject(repo, subject_commit)
    dirty = native.git_dirty_paths(repo)
    native.require(
        not dirty,
        "wall RenderDoc subject includes uncommitted paths: " + ", ".join(dirty[:8]),
    )


def import_renderdoc_modules(repo: Path) -> tuple[Any, Any, Any]:
    scripts = str(repo / "scripts")
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    from perf_tool import renderdoc_capture
    from perf_tool import wall_renderdoc_extract
    from perf_tool.renderdoc_foundation import copy_binary_capsule, verify_capsule_hash

    return renderdoc_capture, wall_renderdoc_extract, (copy_binary_capsule, verify_capsule_hash)


def resolve_tools(args: argparse.Namespace, repo: Path) -> dict[str, Any]:
    capture_module, _, _ = import_renderdoc_modules(repo)
    renderdoccmd = args.renderdoccmd or shutil.which("renderdoccmd")
    qrenderdoc = args.qrenderdoc or shutil.which("qrenderdoc")
    library = args.renderdoc_library or os.environ.get("RENDERDOC_LIBRARY")
    if library is None:
        known_paths = (
            Path("/usr/lib64/renderdoc/librenderdoc.so"),
            Path("/usr/lib/x86_64-linux-gnu/librenderdoc.so"),
        )
        library = next((str(path) for path in known_paths if path.is_file()), None)
    native.require(renderdoccmd is not None, "renderdoccmd is unavailable")
    native.require(qrenderdoc is not None, "qrenderdoc is unavailable")
    native.require(library is not None, "librenderdoc path is unavailable")
    tools = capture_module.inspect_tools(renderdoccmd, qrenderdoc, library)
    extractor = (repo / EXTRACTOR_RELATIVE).resolve()
    native.require(
        extractor.is_file() and not extractor.is_symlink(),
        "wall RenderDoc extractor is unavailable",
    )
    return {
        "renderdoccmd": str(tools["renderdoccmd"]),
        "qrenderdoc": str(tools["qrenderdoc"]),
        "renderdoc_library": str(tools["library"]),
        "renderdoc_version": tools["renderdoc_version"],
        "qrenderdoc_version": tools["qrenderdoc_version"],
        "renderdoccmd_sha256": sha256(tools["renderdoccmd"]),
        "qrenderdoc_sha256": sha256(tools["qrenderdoc"]),
        "renderdoc_library_sha256": sha256(tools["library"]),
        "extractor": str(extractor),
        "extractor_sha256": sha256(extractor),
    }


def build_command() -> list[str]:
    return [
        "python3",
        "scripts/dev.py",
        "cargo",
        "--",
        "build",
        "--profile",
        RENDERDOC_PROFILE,
        "-p",
        "bevy_app@0.1.0",
        "--no-default-features",
        "--features",
        RENDERDOC_FEATURES,
    ]


def capture_command(
    *,
    repo: Path,
    binary: Path,
    tools: dict[str, Any],
    runtime: Path,
    template: Path,
    size: str,
    phase: str,
    candidate: bool,
) -> list[str]:
    command = [
        tools["renderdoccmd"],
        "capture",
        "--capture-file",
        str(template),
        "--wait-for-exit",
        "--working-dir",
        str(repo),
        str(binary),
        "--perf-scenario",
        "--perf-workload",
        "wall-density",
        "--perf-wall-phase",
        phase,
        "--perf-size",
        size,
        "--perf-render",
        "gpu",
        "--perf-clock",
        "realtime",
        "--perf-seed",
        str(SEED),
        "--perf-warmup-secs",
        "30",
        "--perf-measure-secs",
        "60",
        "--perf-output-dir",
        str(runtime),
        "--perf-familiar-policy",
        "baseline",
        "--perf-operation-dialog",
        "hidden",
        "--perf-dashboard",
        "hidden",
        "--spawn-souls",
        "0",
        "--spawn-familiars",
        "0",
        "--perf-window-width",
        str(WINDOW_WIDTH),
        "--perf-window-height",
        str(WINDOW_HEIGHT),
        "--perf-window-scale-factor",
        str(WINDOW_SCALE_FACTOR),
        "--perf-rtt-quality",
        "high",
        "--perf-renderdoc-capture",
    ]
    if candidate:
        command.extend(["--perf-wall-presentation", "production"])
    return command


def replay_command(tools: dict[str, Any], capture: Path) -> list[str]:
    return [tools["qrenderdoc"], "--python", tools["extractor"], str(capture)]


def capture_environment(
    *,
    repo: Path,
    case_root: Path,
    tools: dict[str, Any],
    template: Path,
    candidate: dict[str, Any] | None,
) -> dict[str, str]:
    environment = os.environ.copy()
    for key in (
        "HW_WALL_CANDIDATE",
        "HW_WALL_CANDIDATE_GENERATION",
        "HW_WALL_CANDIDATE_MANIFEST_SHA256",
        "HW_WALL_PERF_PRESENTATION",
    ):
        environment.pop(key, None)
    environment.update(
        {
            "BEVY_ASSET_ROOT": str(repo),
            "HW_PRESENT_MODE": "novsync",
            "HW_WINDOW_BACKEND": "x11",
            "WGPU_BACKEND": "vulkan",
            "WGPU_ADAPTER_NAME": "Intel",
            "HW_RENDERDOC_LIBRARY": tools["renderdoc_library"],
            "HW_RENDERDOC_CAPTURE_TEMPLATE": str(template),
            "RUST_BACKTRACE": "1",
            "TMPDIR": str(case_root),
            "TMP": str(case_root),
            "TEMP": str(case_root),
        }
    )
    if candidate is not None:
        environment.update(
            {
                "HW_WALL_CANDIDATE": "1",
                "HW_WALL_CANDIDATE_GENERATION": str(
                    candidate["asset_set_generation"]
                ),
                "HW_WALL_CANDIDATE_MANIFEST_SHA256": candidate["manifest_sha256"],
                "HW_WALL_PERF_PRESENTATION": "production",
            }
        )
    return environment


def replay_environment(
    environment: dict[str, str],
    *,
    capture: Path,
    checkpoint: Path,
    extraction: Path,
    failure: Path,
) -> dict[str, str]:
    result = environment.copy()
    result.pop("QT_QPA_PLATFORM", None)
    result.update(
        {
            "HW_RENDERDOC_CAPTURE": str(capture),
            "HW_RENDERDOC_RUNTIME_CHECKPOINT": str(checkpoint),
            "HW_RENDERDOC_EXTRACTION": str(extraction),
            "HW_RENDERDOC_EXTRACTION_FAILURE": str(failure),
        }
    )
    return result


def normalized_extraction_digest(module: Any, path: Path) -> str:
    return module.normalized_json_digest(path)


def validate_extraction(
    *,
    extraction_path: Path,
    checkpoint_path: Path,
    capture_path: Path,
    size: str,
    phase: str,
    wall_module: Any,
    candidate: dict[str, Any] | None,
) -> dict[str, Any]:
    extraction = native.read_json(extraction_path)
    checkpoint = wall_module._validate_checkpoint(native.read_json(checkpoint_path))
    expected_keys = {
        "schema_version",
        "contract_id",
        "stage_id",
        "api",
        "capture_sha256",
        "runtime_checkpoint_sha256",
        "validated_frames",
        "event_count",
        "draw_count",
        "scene_target",
        "wall_density",
        "passes",
        "wall_draw_group",
    }
    native.require(set(extraction) == expected_keys, "wall extraction keys differ")
    native.require(
        extraction["schema_version"] == 1
        and extraction["contract_id"] == CONTRACT_ID
        and extraction["stage_id"] == f"wall-density-{size}-{phase}"
        and extraction["api"] == "vulkan"
        and extraction["capture_sha256"] == sha256(capture_path)
        and extraction["runtime_checkpoint_sha256"] == sha256(checkpoint_path)
        and extraction["validated_frames"] == 1,
        "wall extraction identity differs",
    )
    native.require(
        extraction["wall_density"] == checkpoint["wall_density"],
        "wall extraction runtime evidence differs",
    )
    native.require(
        isinstance(extraction["passes"], list) and extraction["passes"],
        "wall extraction contains no render passes",
    )
    group = extraction["wall_draw_group"]
    native.require(isinstance(group, dict), "wall draw group is invalid")
    required_group_keys = {
        "pass_id",
        "draw_group_count",
        "rendered_instance_count",
        "checkpointed_owner_count",
        "direct_scene_target_write",
        "draws",
    }
    wall = checkpoint["wall_density"]
    if wall["schema_version"] == 1:
        required_group_keys.add("wall_mesh_index_count")
    else:
        required_group_keys.update(
            {"wall_mesh_index_counts", "index_instance_counts"}
        )
    native.require(set(group) == required_group_keys, "wall draw-group keys differ")
    native.require(
        isinstance(group["draws"], list)
        and group["draws"]
        and group["draw_group_count"] == len(group["draws"])
        and group["checkpointed_owner_count"] == TARGET_COUNTS[size]
        and 0 < group["rendered_instance_count"] <= TARGET_COUNTS[size],
        "wall draw-group counts differ",
    )
    if wall["schema_version"] == 1:
        native.require(
            candidate is None
            and group["wall_mesh_index_count"] == wall["wall_mesh_index_count"],
            "legacy wall draw-group identity differs",
        )
    else:
        native.require(
            group["wall_mesh_index_counts"] == wall["wall_mesh_index_counts"]
            and group["index_instance_counts"] == wall["wall_index_instance_counts"]
            and group["rendered_instance_count"] == TARGET_COUNTS[size],
            "wall mesh-set draw-group identity differs",
        )
        if candidate is not None:
            native.require(
                wall["presentation"] == "production"
                and wall["candidate_identity"] == candidate
                and wall["active_mesh_count"] == 6,
                "wall production RenderDoc identity differs",
            )
    draw_keys = {
        "pass_id",
        "event_id",
        "indexed",
        "num_indices",
        "num_instances",
        "index_offset",
        "vertex_offset",
        "instance_offset",
        "color_resource_ids",
        "depth_resource_id",
        "fragment_shader_present",
    }
    for draw in group["draws"]:
        native.require(
            isinstance(draw, dict)
            and set(draw) == draw_keys
            and draw["pass_id"] == group["pass_id"]
            and draw["indexed"] is True
            and draw["num_indices"]
            in (
                [group["wall_mesh_index_count"]]
                if wall["schema_version"] == 1
                else group["wall_mesh_index_counts"]
            )
            and isinstance(draw["num_instances"], int)
            and not isinstance(draw["num_instances"], bool)
            and draw["num_instances"] > 0
            and isinstance(draw["color_resource_ids"], list)
            and bool(draw["color_resource_ids"])
            and isinstance(draw["depth_resource_id"], str)
            and draw["depth_resource_id"]
            and draw["fragment_shader_present"] is True,
            "wall draw row differs from the main color+depth predicate",
        )
    native.require(
        group["direct_scene_target_write"]
        == all(
            extraction["scene_target"]["resource_id"] in draw["color_resource_ids"]
            for draw in group["draws"]
        ),
        "wall direct Scene-target classification differs",
    )
    native.require(
        sum(draw["num_instances"] for draw in group["draws"])
        == group["rendered_instance_count"],
        "wall rendered instance total differs",
    )
    return extraction


def case_id(size: str, phase: str) -> str:
    return f"{phase}-{size}"


def locate_capture(raw: Path) -> Path:
    captures = [
        path
        for path in raw.rglob("*.rdc")
        if path.is_file() and not path.is_symlink() and path.stat().st_size > 0
    ]
    native.require(len(captures) == 1, f"RenderDoc produced {len(captures)} RDC files")
    return captures[0].resolve()


def verify_case(
    *,
    repo: Path,
    case_root: Path,
    size: str,
    phase: str,
    renderdoc_module: Any,
    wall_module: Any,
    candidate: dict[str, Any] | None,
) -> dict[str, Any]:
    capture = locate_capture(case_root / "raw")
    checkpoint = case_root / "runtime" / "renderdoc-checkpoint.json"
    extraction = case_root / "extraction.json"
    replay = case_root / "extraction-replay.json"
    first = validate_extraction(
        extraction_path=extraction,
        checkpoint_path=checkpoint,
        capture_path=capture,
        size=size,
        phase=phase,
        wall_module=wall_module,
        candidate=candidate,
    )
    validate_extraction(
        extraction_path=replay,
        checkpoint_path=checkpoint,
        capture_path=capture,
        size=size,
        phase=phase,
        wall_module=wall_module,
        candidate=candidate,
    )
    native.require(
        normalized_extraction_digest(renderdoc_module, extraction)
        == normalized_extraction_digest(renderdoc_module, replay),
        "two wall RenderDoc replays differ",
    )
    group = first["wall_draw_group"]
    return {
        "case_id": case_id(size, phase),
        "size": size,
        "phase": phase,
        "target_wall_count": TARGET_COUNTS[size],
        "presentation": "production" if candidate is not None else "legacy-fallback",
        "draw_group_count": group["draw_group_count"],
        "rendered_instance_count": group["rendered_instance_count"],
        "capture": {"path": str(capture), "bytes": capture.stat().st_size, "sha256": sha256(capture)},
        "runtime_checkpoint": {"path": str(checkpoint), "sha256": sha256(checkpoint)},
        "extraction": {"path": str(extraction), "sha256": sha256(extraction)},
        "replay": {"path": str(replay), "sha256": sha256(replay)},
        "normalized_replay_sha256": normalized_extraction_digest(renderdoc_module, extraction),
    }


def validate_predicates(cases: list[dict[str, Any]]) -> dict[str, Any]:
    indexed = {(case["phase"], case["size"]): case for case in cases}
    native.require(
        set(indexed) == {(phase, size) for phase in PHASES for size in SIZES},
        "wall RenderDoc matrix coverage differs",
    )
    completed_n = indexed[("completed", "small")]["draw_group_count"]
    completed_4n = indexed[("completed", "medium")]["draw_group_count"]
    provisional_n = indexed[("provisional", "small")]["draw_group_count"]
    provisional_4n = indexed[("provisional", "medium")]["draw_group_count"]
    completed_pass = completed_n <= 6 and completed_4n <= 6 and completed_n == completed_4n
    provisional_pass = provisional_4n <= 4 * provisional_n + 6
    native.require(completed_pass, "completed wall draw-group predicate failed")
    native.require(provisional_pass, "provisional wall draw-group predicate failed")
    return {
        "completed": {
            "D_N": completed_n,
            "D_4N": completed_4n,
            "predicate": "D_N <= 6 && D_4N <= 6 && D_4N == D_N",
            "passed": True,
        },
        "provisional": {
            "D_N": provisional_n,
            "D_4N": provisional_4n,
            "predicate": "D_4N <= 4 * D_N + 6",
            "passed": True,
        },
    }


def verify_root(
    root: Path,
    *,
    subject_commit: str | None = None,
    source_fingerprint: str | None = None,
    harness_fingerprint: str | None = None,
) -> dict[str, Any]:
    manifest = native.read_json(root / "manifest.json")
    native.require(
        manifest.get("schema_version") == SCHEMA_VERSION
        and manifest.get("status") == "pass"
        and manifest.get("profile") == PROFILE,
        "wall RenderDoc manifest identity differs",
    )
    repo = native.validate_repo(manifest.get("repo", ""))
    candidate = manifest.get("candidate_identity")
    if candidate is not None:
        native.require(
            isinstance(candidate, dict) and candidate == art.candidate_identity(repo),
            "wall RenderDoc candidate identity changed",
        )
        native.require(
            manifest.get("asset_view_fingerprint")
            == density.asset_view_fingerprint(repo),
            "wall RenderDoc asset view changed",
        )
    for observed, expected, label in (
        (manifest.get("subject_commit"), subject_commit, "subject commit"),
        (manifest.get("source_fingerprint"), source_fingerprint, "source fingerprint"),
        (manifest.get("harness_fingerprint"), harness_fingerprint, "harness fingerprint"),
    ):
        if expected is not None:
            native.require(observed == expected, f"wall RenderDoc {label} differs")
    assert_clean(repo, manifest["subject_commit"])
    native.require(
        native.source_fingerprint(repo) == manifest["source_fingerprint"],
        "wall RenderDoc source no longer matches",
    )
    native.require(
        native.native_harness_fingerprint(repo) == manifest["harness_fingerprint"],
        "wall RenderDoc harness no longer matches",
    )
    tools = manifest.get("tools")
    native.require(isinstance(tools, dict), "wall RenderDoc tools are invalid")
    for path_key, hash_key in (
        ("renderdoccmd", "renderdoccmd_sha256"),
        ("qrenderdoc", "qrenderdoc_sha256"),
        ("renderdoc_library", "renderdoc_library_sha256"),
        ("extractor", "extractor_sha256"),
    ):
        path = Path(tools[path_key])
        native.require(path.is_file() and sha256(path) == tools[hash_key], f"{path_key} changed")
    renderdoc_module, wall_module, capsule_modules = import_renderdoc_modules(repo)
    _, verify_capsule_hash = capsule_modules
    capsule = verify_capsule_hash(root / "capsule")
    native.require(
        capsule.binary_sha256 == manifest["binary_sha256"],
        "wall RenderDoc capsule differs",
    )
    cases = []
    for phase in PHASES:
        for size in SIZES:
            cases.append(
                verify_case(
                    repo=repo,
                    case_root=root / "cases" / case_id(size, phase),
                    size=size,
                    phase=phase,
                    renderdoc_module=renderdoc_module,
                    wall_module=wall_module,
                    candidate=candidate,
                )
            )
    native.require(cases == manifest["cases"], "wall RenderDoc case evidence differs")
    predicates = validate_predicates(cases)
    native.require(predicates == manifest["predicates"], "wall predicates differ")
    return {
        "schema_version": SCHEMA_VERSION,
        "status": "pass",
        "profile": PROFILE,
        "root": str(root),
        "cases": len(cases),
        "predicates": predicates,
    }


def plan(args: argparse.Namespace) -> int:
    repo = native.validate_repo(args.repo)
    resources = native.resource_snapshot(repo, require_launcher=True)
    failures = list(resources["failures"])
    failures.extend(f"missing runtime asset {path}" for path in missing_runtime_assets(repo))
    subject = native.git_subject(repo)
    source = native.source_fingerprint(repo)
    harness = native.native_harness_fingerprint(repo)
    candidate = None
    assets = None
    try:
        assert_clean(repo, subject)
        tools = resolve_tools(args, repo)
        if args.candidate:
            candidate = art.candidate_identity(repo)
            assets = density.asset_view_fingerprint(repo)
    except Exception as error:
        failures.append(str(error))
        tools = None
    root = Path(args.job_root).resolve() if args.job_root else native.unique_job_root(repo, "wall-renderdoc")
    if root.exists():
        failures.append(f"job root already exists: {root}")
    command = []
    if tools is not None:
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
            "--adapter",
            args.adapter,
            "--renderdoccmd",
            tools["renderdoccmd"],
            "--qrenderdoc",
            tools["qrenderdoc"],
            "--renderdoc-library",
            tools["renderdoc_library"],
        ]
        if candidate is not None:
            command.extend(
                [
                    "--candidate-generation",
                    str(candidate["asset_set_generation"]),
                    "--candidate-manifest-sha256",
                    candidate["manifest_sha256"],
                    "--asset-view-fingerprint",
                    assets,
                ]
            )
    native.print_json(
        {
            "schema_version": SCHEMA_VERSION,
            "status": "ready" if not failures else "blocked",
            "profile": PROFILE,
            "job_root": str(root),
            "subject_commit": subject,
            "source_fingerprint": source,
            "harness_fingerprint": harness,
            "adapter": args.adapter,
            "candidate_identity": candidate,
            "asset_view_fingerprint": assets,
            "failures": failures,
            "resources": resources,
            "tools": tools,
            "launcher_command": command,
            "execution_contract": {
                "actual_window_required": True,
                "parallel_game_processes": 1,
                "capture_processes": 4,
                "replays_per_capture": 2,
            },
        }
    )
    return 0 if not failures else 1


@native.activity_locked
def run(args: argparse.Namespace) -> int:
    native.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "wall RenderDoc run must use the planned direct kitty launcher",
    )
    repo = native.validate_repo(args.repo)
    native.require(native.git_subject(repo) == args.subject_commit, "subject changed")
    native.require(native.source_fingerprint(repo) == args.source_fingerprint, "source changed")
    native.require(native.native_harness_fingerprint(repo) == args.harness_fingerprint, "harness changed")
    assert_clean(repo, args.subject_commit)
    native.require(not missing_runtime_assets(repo), "runtime assets are missing")
    candidate_arguments = (
        args.candidate_generation,
        args.candidate_manifest_sha256,
        args.asset_view_fingerprint,
    )
    native.require(
        all(value is None for value in candidate_arguments)
        or all(value is not None for value in candidate_arguments),
        "wall RenderDoc candidate arguments must be provided together",
    )
    candidate = None
    if args.candidate_generation is not None:
        candidate = art.candidate_identity(repo)
        native.require(
            int(args.candidate_generation) == candidate["asset_set_generation"]
            and args.candidate_manifest_sha256 == candidate["manifest_sha256"],
            "planned wall RenderDoc candidate changed",
        )
        native.require(
            args.asset_view_fingerprint == density.asset_view_fingerprint(repo),
            "planned wall RenderDoc asset view changed",
        )
    tools = resolve_tools(args, repo)
    native.require(args.adapter == "Intel", "wall RenderDoc profile currently seals the Intel adapter")
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
        "candidate_identity": candidate,
        "asset_view_fingerprint": args.asset_view_fingerprint,
        "started_at": native.utc_now(),
        "current_stage": None,
        "child_pid": None,
    }
    native.atomic_write_json(root / "job.json", state)
    try:
        environment = os.environ.copy()
        native.run_command(
            "build-renderdoc",
            build_command(),
            repo=repo,
            env=environment,
            log_path=root / "build.log",
            job_file=root / "job.json",
            state=state,
        )
        source_binary = repo / "target" / RENDERDOC_PROFILE / "bevy_app"
        renderdoc_module, wall_module, capsule_modules = import_renderdoc_modules(repo)
        copy_binary_capsule, verify_capsule_hash = capsule_modules
        capsule_manifest = copy_binary_capsule(
            source_binary=source_binary,
            capsule_root=root / "capsule",
            leg="renderdoc",
            profile=RENDERDOC_PROFILE,
            features=RENDERDOC_FEATURES,
            cargo_lock_path=repo / "Cargo.lock",
            rustc_version=native.command_output(["rustc", "--version"], repo=repo),
            linker=native.command_output(["cc", "--version"], repo=repo).splitlines()[0],
            env_allowlist={},
        )
        verify_capsule_hash(root / "capsule")
        binary = root / "capsule" / "bevy_app"
        cases = []
        for phase in PHASES:
            for size in SIZES:
                native.require(native.source_fingerprint(repo) == args.source_fingerprint, "source changed before case")
                case_root = root / "cases" / case_id(size, phase)
                raw = case_root / "raw"
                runtime = case_root / "runtime"
                raw.mkdir(parents=True)
                runtime.mkdir()
                template = raw / "wall-density"
                case_environment = capture_environment(
                    repo=repo,
                    case_root=case_root,
                    tools=tools,
                    template=template,
                    candidate=candidate,
                )
                case_environment["WGPU_ADAPTER_NAME"] = args.adapter
                native.run_command(
                    f"capture-{phase}-{size}",
                    capture_command(
                        repo=repo,
                        binary=binary,
                        tools=tools,
                        runtime=runtime,
                        template=template,
                        size=size,
                        phase=phase,
                        candidate=candidate is not None,
                    ),
                    repo=repo,
                    env=case_environment,
                    log_path=case_root / "capture.log",
                    job_file=root / "job.json",
                    state=state,
                    timeout_seconds=CAPTURE_TIMEOUT_SECONDS,
                )
                capture = locate_capture(raw)
                checkpoint = runtime / "renderdoc-checkpoint.json"
                wall_module._validate_checkpoint(native.read_json(checkpoint))
                for ordinal, output_name in ((1, "extraction.json"), (2, "extraction-replay.json")):
                    extraction = case_root / output_name
                    failure = case_root / f"extraction-{ordinal}-failure.json"
                    native.run_command(
                        f"replay-{ordinal}-{phase}-{size}",
                        replay_command(tools, capture),
                        repo=repo,
                        env=replay_environment(case_environment, capture=capture, checkpoint=checkpoint, extraction=extraction, failure=failure),
                        log_path=case_root / f"replay-{ordinal}.log",
                        job_file=root / "job.json",
                        state=state,
                        timeout_seconds=REPLAY_TIMEOUT_SECONDS,
                    )
                    if not extraction.is_file():
                        detail = "qrenderdoc did not produce extraction JSON"
                        if failure.is_file():
                            payload = native.read_json(failure)
                            if isinstance(payload.get("error"), str):
                                detail = payload["error"]
                        raise native.AcceptanceError(detail)
                log_text = "\n".join(
                    path.read_text(encoding="utf-8", errors="replace")
                    for path in (
                        case_root / "capture.log",
                        case_root / "replay-1.log",
                        case_root / "replay-2.log",
                    )
                )
                problems = renderdoc_module.unexpected_log_lines(log_text, ())
                native.require(
                    not problems,
                    "unexpected wall RenderDoc log line: "
                    + (problems[0] if problems else ""),
                )
                cases.append(
                    verify_case(
                        repo=repo,
                        case_root=case_root,
                        size=size,
                        phase=phase,
                        renderdoc_module=renderdoc_module,
                        wall_module=wall_module,
                        candidate=candidate,
                    )
                )
        predicates = validate_predicates(cases)
        native.require(native.source_fingerprint(repo) == args.source_fingerprint, "source changed after captures")
        manifest = {
            "schema_version": SCHEMA_VERSION,
            "status": "pass",
            "profile": PROFILE,
            "repo": str(repo),
            "subject_commit": args.subject_commit,
            "source_fingerprint": args.source_fingerprint,
            "harness_fingerprint": args.harness_fingerprint,
            "adapter": args.adapter,
            "candidate_identity": candidate,
            "asset_view_fingerprint": args.asset_view_fingerprint,
            "binary_sha256": capsule_manifest.binary_sha256,
            "tools": tools,
            "cases": cases,
            "predicates": predicates,
            "completed_at": native.utc_now(),
        }
        native.atomic_write_json(root / "manifest.json", manifest)
        state.update({"status": "valid", "completed_at": native.utc_now(), "child_pid": None})
        native.atomic_write_json(root / "job.json", state)
        native.print_json(verify_root(root, subject_commit=args.subject_commit, source_fingerprint=args.source_fingerprint, harness_fingerprint=args.harness_fingerprint))
        return 0
    except Exception as error:
        state.update({"status": "invalid", "failure": f"{type(error).__name__}: {error}", "completed_at": native.utc_now(), "child_pid": None})
        native.atomic_write_json(root / "job.json", state)
        raise


def self_test() -> int:
    cases = [
        {"phase": "completed", "size": "small", "draw_group_count": 1},
        {"phase": "completed", "size": "medium", "draw_group_count": 1},
        {"phase": "provisional", "size": "small", "draw_group_count": 4},
        {"phase": "provisional", "size": "medium", "draw_group_count": 16},
    ]
    predicates = validate_predicates(cases)
    native.require(predicates["completed"]["passed"], "completed self-test failed")
    native.require(predicates["provisional"]["passed"], "provisional self-test failed")
    command = capture_command(
        repo=Path("/repo"),
        binary=Path("/capsule/bevy_app"),
        tools={"renderdoccmd": "/usr/bin/renderdoccmd"},
        runtime=Path("/job/runtime"),
        template=Path("/job/raw/wall-density"),
        size="small",
        phase="completed",
        candidate=True,
    )
    native.require(
        command.count("--perf-renderdoc-capture") == 1
        and command[command.index("--perf-wall-presentation") + 1] == "production",
        "capture flags differ",
    )
    native.print_json({"schema_version": 1, "status": "pass", "profile": PROFILE})
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser(description=__doc__)
    commands = root.add_subparsers(dest="command", required=True)
    plan_parser = commands.add_parser("plan")
    plan_parser.add_argument("--repo", required=True)
    plan_parser.add_argument("--job-root")
    plan_parser.add_argument("--adapter", default="Intel")
    plan_parser.add_argument("--candidate", action="store_true")
    for name in ("renderdoccmd", "qrenderdoc", "renderdoc-library"):
        plan_parser.add_argument(f"--{name}")
    run_parser = commands.add_parser("run")
    run_parser.add_argument("--repo", required=True)
    run_parser.add_argument("--job-root", required=True)
    run_parser.add_argument("--subject-commit", required=True)
    run_parser.add_argument("--source-fingerprint", required=True)
    run_parser.add_argument("--harness-fingerprint", required=True)
    run_parser.add_argument("--adapter", default="Intel")
    run_parser.add_argument("--candidate-generation")
    run_parser.add_argument("--candidate-manifest-sha256")
    run_parser.add_argument("--asset-view-fingerprint")
    for name in ("renderdoccmd", "qrenderdoc", "renderdoc-library"):
        run_parser.add_argument(f"--{name}", required=True)
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
    if args.command == "run":
        for value, label, length in (
            (args.subject_commit, "subject commit", 40),
            (args.source_fingerprint, "source fingerprint", 64),
            (args.harness_fingerprint, "harness fingerprint", 64),
        ):
            native.require(re.fullmatch(f"[0-9a-f]{{{length}}}", value) is not None, f"invalid {label}")
        if args.candidate_generation is not None:
            native.require(
                args.candidate_generation.isdigit()
                and int(args.candidate_generation) > 0,
                "invalid candidate generation",
            )
            for value, label in (
                (args.candidate_manifest_sha256, "candidate manifest"),
                (args.asset_view_fingerprint, "asset-view fingerprint"),
            ):
                native.require(
                    isinstance(value, str)
                    and re.fullmatch("[0-9a-f]{64}", value) is not None,
                    f"invalid {label}",
                )
        return run(args)
    if args.command == "status":
        job = native.read_json(Path(args.job_root).resolve() / "job.json")
        native.print_json(job)
        return 2 if job.get("status") == "running" else 0 if job.get("status") == "valid" else 1
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
        native.print_json({"schema_version": 1, "status": "invalid", "error": str(error)})
        raise SystemExit(1) from error
