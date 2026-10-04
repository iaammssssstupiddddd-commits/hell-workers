"""Coordinator-only paired M6 static matrix; never grants art/release approval.

Requires eight real released sets plus unchanged Door g7. Missing releases fail
planning, including legacy-control. Capture then Memory, N/4N, three adjacent
pairs with reversed middle order. Use primary dev.py validation registration.
"""

from __future__ import annotations

import argparse
from dataclasses import asdict
import json
import math
import os
from pathlib import Path
import statistics
import struct

if __package__:
    from . import building_art_acceptance as art
else:
    import building_art_acceptance as art

native, pipeline, static = art.native, art.pipeline, art.static
from perf_tool.artifact_readers.building_m6 import (  # noqa: E402
    DOOR_HASH,
    RESOURCE_KEYS,
    validate_sidecar,
)
from perf_tool.artifacts import frame_summary, read_frames, validate_run  # noqa: E402
from perf_tool.execution import read_native_memory, read_resource_usage  # noqa: E402
from perf_tool.model import Case  # noqa: E402

PROFILE = "building-m6-paired-static-v1"
METRICS = {
    "p95_ms",
    "p99_ms",
    "peak_live_bytes",
    "max_rss_kib",
    "asset_mesh_accessor_bytes",
    *RESOURCE_KEYS,
}


def matrix():
    return [
        (instrument, size, repeat, mode)
        for instrument in ("capture", "memory")
        for size in ("small", "medium")
        for repeat in range(1, 4)
        for mode in (
            ("candidate", "legacy-control")
            if repeat == 2
            else ("legacy-control", "candidate")
        )
    ]


def label(case):
    return "-".join(map(str, case))


def command(root, case, adapter):
    instrument, size, _repeat, mode = case
    argv = static.command(
        root, instrument, size, 15 if size == "small" else 60, adapter
    )
    argv[argv.index("--repeat") + 1] = "1"
    argv[argv.index("--output") + 1] = str(root / label(case))
    return [*argv, "--building-m6-mode", mode]


def budget(value):
    pipeline.require(
        set(value) == {"max_delta", "max_relative_mad", "reason"}
        and set(value["max_delta"]) == METRICS
        and isinstance(value["reason"], str)
        and value["reason"].strip(),
        "complete numeric budget/reason required",
    )
    pipeline.require(
        all(
            type(n) in (int, float) and math.isfinite(n) and n >= 0
            for n in [*value["max_delta"].values(), value["max_relative_mad"]]
        ),
        "invalid numeric budget",
    )
    return value


def mesh_accessor_bytes(payload):
    """GLB source accessor payload, not Bevy CPU/GPU allocation or padded buffer size."""
    magic, version, length = struct.unpack_from("<III", payload)
    size, kind = struct.unpack_from("<II", payload, 12)
    pipeline.require(
        magic == 0x46546C67
        and version == 2
        and length == len(payload)
        and kind == 0x4E4F534A,
        "invalid GLB inventory",
    )
    value = json.loads(payload[20 : 20 + size])
    primitive = value["meshes"][0]["primitives"][0]
    accessors = set(primitive["attributes"].values()) | {primitive["indices"]}
    widths = {5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4}
    lanes = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}
    return sum(
        value["accessors"][i]["count"]
        * widths[value["accessors"][i]["componentType"]]
        * lanes[value["accessors"][i]["type"]]
        for i in accessors
    )


def inventory(repo, codec):
    assets = repo / "assets"
    bindings = pipeline.release_bindings(assets, codec, "m5")
    manifests = [
        pipeline.load_set(assets, item["identity"]["kind"], codec)[0]
        for item in bindings
    ]
    door = pipeline.read(assets / "manifests/door-production-v1.doorset")
    pipeline.require(
        door["authority"] == "release_approved"
        and door["asset_set_generation"] == 7
        and door["manifest_sha256"] == DOOR_HASH,
        "normal Door g7 required",
    )
    pipeline.require(
        pipeline.digest((assets / "manifests/door-production-v1.doorset").read_bytes())
        == "21075f6b5c6e4412ccc17632ecc3b44bcb619bb901a06062fa0905c0be303002",
        "Door locator bytes differ",
    )
    records = {}
    for record in [
        *door["core"],
        door["receipt"],
        *(r for manifest in manifests for r in manifest["artifacts"]),
    ]:
        payload = pipeline.file_bytes(assets, record)
        records[record["path"]] = {
            "bytes": len(payload),
            "sha256": pipeline.digest(payload),
            "mesh_accessor_payload_bytes": mesh_accessor_bytes(payload)
            if record["path"].endswith(".glb")
            else 0,
        }
    return {"bindings": bindings, "manifests": manifests, "files": records}


def check_inputs(value):
    pipeline.require(
        value["profile"] == PROFILE
        and value["accepted"] is False
        and value["promotion_authority"] is False,
        "invalid M6 scope",
    )
    repo, codec = Path(value["repo"]), Path(value["codec"])
    static.require_identity(repo, value["identity"])
    pipeline.require(
        pipeline.digest(Path(__file__).read_bytes()) == value["driver_sha256"]
        and pipeline.digest(codec.read_bytes()) == value["codec_sha256"],
        "M6 driver/codec changed",
    )
    pipeline.require(
        inventory(repo, codec) == value["assets"], "M6 release inputs changed"
    )
    budget(value["budget"])
    static.check_root(repo, Path(value["job_root"]))
    if value.get("foundation") is not None:
        frozen_foundation(value)


def frozen_foundation(value):
    reference = value["foundation"]
    root = Path(reference["root"])
    manifest = native.read_json(root / "manifest.json")
    pipeline.require(
        pipeline.digest((root / "manifest.json").read_bytes())
        == reference["manifest_sha256"]
        and manifest["identity"] == reference["identity"],
        "foundation input changed",
    )
    before, after = reference["identity"], value["identity"]
    pipeline.require(
        before["subject_commit"] != after["subject_commit"]
        and before["source_fingerprint"] != after["source_fingerprint"],
        "foundation needs distinct sources",
    )
    art.source_roles(Path(manifest["repo"]), Path(value["repo"]))

    def common_assets(repo):
        assets = Path(repo) / "assets"
        return {
            str(path.relative_to(assets)): pipeline.digest(path.read_bytes())
            for path in assets.rglob("*")
            if path.is_file()
            and path.relative_to(assets).parts[0] != "building_sets"
            and not (
                path.relative_to(assets).parts[0] in {"authority", "manifests"}
                and path.name.startswith("building-")
            )
        }

    pipeline.require(
        common_assets(manifest["repo"]) == common_assets(value["repo"]),
        "foundation common Wall/Door/terrain/actor/fallback asset bytes differ",
    )
    return root, manifest


def plan(args):
    repo, codec = native.validate_repo(str(args.repo)), args.codec.resolve()
    root = (
        args.job_root.resolve()
        if args.job_root
        else native.unique_job_root(repo, "building-m6")
    )
    static.check_root(repo, root)
    pipeline.require(
        not root.exists() and not args.output.resolve().is_relative_to(root),
        "fresh root and external plan required",
    )
    value = {
        "profile": PROFILE,
        "repo": str(repo),
        "job_root": str(root),
        "codec": str(codec),
        "codec_sha256": pipeline.digest(codec.read_bytes()),
        "driver_sha256": pipeline.digest(Path(__file__).read_bytes()),
        "identity": static.identity(repo),
        "assets": inventory(repo, codec),
        "budget": budget(pipeline.read(args.budget)),
        "adapter": args.adapter,
        "accepted": False,
        "promotion_authority": False,
        "status": "ready",
    }
    pipeline.require(
        (args.foundation_baseline is None) == (args.foundation_identity is None),
        "foundation root and identity must be paired",
    )
    value["foundation"] = None
    if args.foundation_baseline is not None:
        baseline = args.foundation_baseline.resolve()
        value["foundation"] = {
            "root": str(baseline),
            "identity": pipeline.read(args.foundation_identity),
            "manifest_sha256": pipeline.digest(
                (baseline / "manifest.json").read_bytes()
            ),
        }
        baseline, manifest = frozen_foundation(value)
        art.verify_frozen_static(baseline, manifest, value["foundation"]["identity"])
    check_inputs(value)
    resources = native.resource_snapshot(repo, require_launcher=True)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    value["resources"] = resources
    value["launcher_command"] = [
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
        "--plan",
        str(args.output.resolve()),
    ]
    value["commands"] = [command(root, case, args.adapter) for case in matrix()]
    pipeline.put(args.output, pipeline.canonical(value))
    return value


def verify_case(value, item):
    instrument, size, repeat, mode = item
    session = Path(value["job_root"]) / label(item)
    manifest, config = (
        native.read_json(session / "manifest.json"),
        native.read_json(session / "matrix.json"),
    )
    case = Case(
        "building-art-static", size, "gpu", 20260920, 15 if size == "small" else 60, 0
    )
    pipeline.require(
        manifest["status"] == "valid"
        and config["repeat"] == 1
        and config["preflight_runs"] == 0
        and config["warmup_secs"] == 30.0
        and config["measure_secs"] == 60.0
        and config["sizes"] == [size]
        and config["renders"] == ["gpu"],
        "invalid paired session",
    )
    frozen = value["identity"]
    pipeline.require(
        manifest["git"]["commit"] == frozen["subject_commit"]
        and manifest["git"]["dirty_paths"] == []
        and manifest["source"]["fingerprint_start"]
        == manifest["source"]["fingerprint_end"]
        == frozen["source_fingerprint"]
        and manifest["source"]["unchanged"] is True
        and manifest["binary"]["instrumentation"] == instrument,
        "session subject/instrumentation differs",
    )
    case_root = session / "cases" / case.identifier
    pipeline.require(
        {p.name for p in case_root.iterdir() if p.is_dir()} == {"run-001"},
        "paired run inventory differs",
    )
    run = case_root / "run-001"
    metadata = native.read_json(run / "run-metadata.json")
    requested = native.read_json(run / "requested-environment.json")
    pipeline.require(
        requested.get("HW_BUILDING_M6_MODE") == mode, "requested M6 mode differs"
    )
    pipeline.require(
        metadata["case"] == asdict(case) and metadata["returncode"] == 0,
        "run identity differs",
    )
    validation = validate_run(
        run,
        returncode=0,
        expected_case=case,
        expected_adapter=value["adapter"],
        expected_backend="vulkan",
        allow_log_patterns=[],
        capture_kind="frame-time",
        expected_warmup_secs=30.0,
        expected_measure_secs=60.0,
        expected_window_backend="x11",
        expected_present_mode="novsync",
        expected_window_width=1280,
        expected_window_height=720,
        expected_window_scale_factor=1.0,
        expected_rtt_quality="high",
    )
    pipeline.require(validation.valid, str(validation.reasons))
    sidecar = native.read_json(run / "data/building_art_static.json")
    observed = validate_sidecar(sidecar, case)
    pipeline.require(
        observed["m6"]["mode"] == mode
        and observed["m6"]["identities"]
        == [binding["identity"] for binding in value["assets"]["bindings"]],
        "paired authority/mode differs",
    )
    if mode == "candidate":
        manifests = {
            manifest["identity"]["kind"]: manifest
            for manifest in value["assets"]["manifests"]
        }
        for row in observed["records"]:
            if row["kind"] == "Door":
                continue
            expected = sorted(
                manifests[row["kind"]]["parts"], key=lambda part: part["mesh_role"]
            )
            for actual, part in zip(row["parts"], expected, strict=True):
                pipeline.require(
                    all(
                        actual[key] == part[key]
                        for key in (
                            "mesh_role",
                            "translation_wu",
                            "rotation_xyzw",
                            "scale",
                        )
                    ),
                    "M6 authored production part differs from frozen release",
                )
    samples, errors = read_frames(
        run / "data/frames.csv", int(validation.summary["samples"])
    )
    pipeline.require(
        not errors and samples and sidecar["stable_frames"] >= len(samples),
        "unobserved measurement frames",
    )
    frames = frame_summary(samples)
    metrics = {key: frames[key] for key in ("p95_ms", "p99_ms")}
    if instrument == "memory":
        allocation, errors = read_native_memory(
            run / "data/memory.csv", frame_samples=len(samples)
        )
        process, process_errors = read_resource_usage(run / "resource-usage.txt")
        pipeline.require(
            not errors + process_errors, "invalid native counters/time evidence"
        )
        validation.profile_artifact = {
            "instrumentation": "memory",
            "allocation_memory": allocation,
            "process_memory": process,
        }
        pipeline.require(
            native.read_json(run / "profile-artifact.json")
            == validation.profile_artifact,
            "native summary drift",
        )
        metrics = {
            "peak_live_bytes": allocation["peak_live_bytes"],
            "max_rss_kib": process["max_rss_kib"],
        }
    else:
        pipeline.require(
            not (run / "profile-artifact.json").exists(),
            "Memory evidence in Capture leg",
        )
    pipeline.require(
        native.read_json(run / "validation.json") == validation.to_json()
        and metadata["actual_adapter"] == validation.adapter
        and metadata["actual_window"] == validation.window
        and metadata["runtime_data_cleaned"] is True,
        "stored validation/environment differs",
    )
    metrics.update(observed["m6"]["resources"])
    metrics["asset_mesh_accessor_bytes"] = sum(
        record["mesh_accessor_payload_bytes"]
        for path, record in value["assets"]["files"].items()
        if mode == "candidate" or path.startswith("door_sets/")
    )
    return {
        "instrumentation": instrument,
        "size": size,
        "repeat": repeat,
        "mode": mode,
        "binary_sha256": manifest["binary"]["sha256"],
        "adapter": validation.adapter,
        "window": validation.window,
        "session_sha256": static.session_digest(session),
        "metrics": metrics,
        "pool_active": observed["m6"]["pool_active"],
        "target_mesh_entities": observed["m6"]["target_mesh_entities"],
    }


def aggregate(rows, limits):
    result = []
    pipeline.require(
        rows
        and all(
            row["adapter"] == rows[0]["adapter"] and row["window"] == rows[0]["window"]
            for row in rows
        ),
        "Capture/Memory environment drift",
    )
    for instrument in ("capture", "memory"):
        selected = [row for row in rows if row["instrumentation"] == instrument]
        pipeline.require(
            len({row["binary_sha256"] for row in selected}) == 1,
            "same-source pair binary drift",
        )
        pipeline.require(
            all(
                row["adapter"] == selected[0]["adapter"]
                and row["window"] == selected[0]["window"]
                for row in selected
            ),
            "paired environment drift",
        )
        for size in ("small", "medium"):
            modes = {}
            for mode in ("legacy-control", "candidate"):
                group = [
                    row
                    for row in selected
                    if row["size"] == size and row["mode"] == mode
                ]
                pipeline.require(
                    sorted(row["repeat"] for row in group) == [1, 2, 3],
                    "three distinct repeats required",
                )
                modes[mode] = {}
                for metric in group[0]["metrics"]:
                    values = [row["metrics"][metric] for row in group]
                    pipeline.require(
                        all(
                            type(n) in (int, float) and math.isfinite(n) and n >= 0
                            for n in values
                        ),
                        "invalid metric",
                    )
                    median = statistics.median(values)
                    mad = statistics.median(abs(n - median) for n in values)
                    pipeline.require(
                        mad <= median * limits["max_relative_mad"],
                        f"noise budget exceeded: {metric}",
                    )
                    modes[mode][metric] = {"median": median, "mad": mad}
            for metric, after in modes["candidate"].items():
                pipeline.require(
                    after["median"] - modes["legacy-control"][metric]["median"]
                    <= limits["max_delta"][metric],
                    f"M6 budget exceeded: {instrument}/{size}/{metric}",
                )
            result.append({"instrumentation": instrument, "size": size, "modes": modes})
        for mode in ("legacy-control", "candidate"):
            for metric in RESOURCE_KEYS:
                values = {
                    row["metrics"][metric] for row in selected if row["mode"] == mode
                }
                pipeline.require(
                    len(values) == 1,
                    f"N/4N shared resources did not plateau: {mode}/{metric}",
                )
    pipeline.require(
        rows[0]["binary_sha256"] != rows[-1]["binary_sha256"],
        "Capture/Memory binary not separated",
    )
    return result


def verify(plan_path, *, in_progress=False):
    value = pipeline.read(plan_path)
    check_inputs(value)
    job = native.read_json(Path(value["job_root"]) / "job.json")
    pipeline.require(
        job["plan_sha256"] == pipeline.digest(plan_path.read_bytes())
        and job["status"] == ("running" if in_progress else "valid"),
        "plan changed or job did not finish",
    )
    rows = [verify_case(value, case) for case in matrix()]
    result = {
        "profile": PROFILE,
        "status": "pass",
        "scope": "same-source-paused-asset-increment-only",
        "accepted": False,
        "promotion_authority": False,
        "source": value["identity"],
        "capture_runs": 12,
        "memory_runs": 12,
        "rows": rows,
        "aggregate": aggregate(rows, value["budget"]),
        "unverified": [
            "separate-source M1-0 cumulative cost",
            "active state/lifecycle",
            "actual-window art",
            "release",
        ],
    }
    if value.get("foundation") is not None:
        baseline, manifest = frozen_foundation(value)
        historical = art.verify_frozen_static(
            baseline, manifest, value["foundation"]["identity"]
        )
        comparisons = []
        for before in historical["sessions"]:
            after = next(
                row
                for row in result["aggregate"]
                if row["size"] == before["size"]
                and row["instrumentation"] == before["instrumentation"]
            )
            current = [
                row
                for row in rows
                if row["size"] == before["size"]
                and row["instrumentation"] == before["instrumentation"]
                and row["mode"] == "candidate"
            ]
            pipeline.require(
                all(row["binary_sha256"] != before["binary_sha256"] for row in current)
                and all(
                    row["adapter"] == current[0]["adapter"]
                    for row in before["observations"]
                ),
                "foundation binary/adapter conditions differ",
            )
            metrics = (
                ("p95_ms", "p99_ms")
                if before["instrumentation"] == "capture"
                else ("peak_live_bytes", "max_rss_kib")
            )
            comparisons.append(
                {
                    "size": before["size"],
                    "instrumentation": before["instrumentation"],
                    "before": {key: before["aggregate"][key] for key in metrics},
                    "after": {key: after["modes"]["candidate"][key] for key in metrics},
                }
            )
        result["foundation_comparison"] = comparisons
        result["unverified"][0] = (
            "foundation N Memory, resource-byte baseline and independently approved cumulative budget"
        )
    check_inputs(value)
    if not in_progress:
        pipeline.require(
            native.read_json(Path(value["job_root"]) / "result.json") == result,
            "stored M6 result differs from raw evidence",
        )
    return result


@native.activity_locked
def run(args):
    pipeline.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "use the registered no-prompt launcher",
    )
    value = pipeline.read(args.plan)
    check_inputs(value)
    repo, root = Path(value["repo"]), Path(value["job_root"])
    resources = native.resource_snapshot(repo, require_launcher=False)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    root.mkdir(parents=True, exist_ok=False)
    state = {
        "profile": PROFILE,
        "status": "running",
        "pid": os.getpid(),
        "started_at": native.utc_now(),
        "heartbeat_at": native.utc_now(),
        "plan_sha256": pipeline.digest(args.plan.read_bytes()),
    }
    native.atomic_write_json(root / "job.json", state)
    try:
        environment = native.cargo_environment(repo)
        for key in (
            "HW_BUILDING_ART_SESSION",
            "HW_BUILDING_M6_MODE",
            "HW_M2_ACCEPTANCE_PROBE",
            "HW_BRIDGE_ACCEPTANCE_PROBE",
        ):
            environment.pop(key, None)
        environment["HW_BUILDING_ASSET_RELEASES"] = json.dumps(
            value["assets"]["bindings"]
        )
        for case in matrix():
            pipeline.require(
                pipeline.read(args.plan) == value, "plan changed during execution"
            )
            check_inputs(value)
            native.run_command(
                label(case),
                command(root, case, value["adapter"]),
                repo=repo,
                env=environment,
                log_path=root / (label(case) + ".log"),
                job_file=root / "job.json",
                state=state,
                timeout_seconds=3600,
            )
            verify_case(value, case)
        result = verify(args.plan, in_progress=True)
        native.atomic_write_json(root / "result.json", result)
        native.update_state(
            root / "job.json",
            state,
            status="valid",
            child_pid=None,
            completed_at=native.utc_now(),
        )
        return result
    except Exception as error:
        native.update_state(
            root / "job.json",
            state,
            status="invalid",
            child_pid=None,
            failure=str(error),
        )
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    create = sub.add_parser("plan")
    create.add_argument("--repo", type=Path, required=True)
    create.add_argument("--codec", type=Path, required=True)
    create.add_argument("--budget", type=Path, required=True)
    create.add_argument("--adapter", required=True)
    create.add_argument("--job-root", type=Path)
    create.add_argument("--output", type=Path, required=True)
    create.add_argument("--foundation-baseline", type=Path)
    create.add_argument("--foundation-identity", type=Path)
    for action in ("run", "verify"):
        sub.add_parser(action).add_argument("--plan", type=Path, required=True)
    args = parser.parse_args()
    result = (
        plan(args)
        if args.action == "plan"
        else run(args)
        if args.action == "run"
        else verify(args.plan)
    )
    print(json.dumps(result, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
