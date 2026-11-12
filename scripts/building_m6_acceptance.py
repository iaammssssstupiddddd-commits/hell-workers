"""Coordinator-only paired M6 static matrix; never grants art/release approval.

Requires eight real released sets plus unchanged Door g7. Missing releases fail
planning, including legacy-control. Capture then Memory, N/4N, three adjacent
pairs with reversed middle order. Use primary dev.py validation registration.
Explicit --candidate-roots selects a separate pre-release profile with eight
provenance-verified isolated candidates in an immutable copied asset view.
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
import shutil

if __package__:
    from . import building_art_acceptance as art
else:
    import building_art_acceptance as art

native, pipeline, static = art.native, art.pipeline, art.static
if __package__:
    from .perf_tool.artifact_readers.building_m6 import (  # noqa: E402
        DOOR_HASH, RESOURCE_KEYS, GROUP_TARGETS, GROUP_FIXED, validate_group_declaration,
        validate_group_sidecar, validate_sidecar,
    )
    from .perf_tool.artifacts import frame_summary, read_frames, validate_run  # noqa: E402
    from .perf_tool.execution import read_native_memory, read_resource_usage  # noqa: E402
    from .perf_tool.model import Case  # noqa: E402
else:
    from perf_tool.artifact_readers.building_m6 import (  # noqa: E402
        DOOR_HASH, RESOURCE_KEYS, GROUP_TARGETS, GROUP_FIXED, validate_group_declaration,
        validate_group_sidecar, validate_sidecar,
    )
    from perf_tool.artifacts import frame_summary, read_frames, validate_run  # noqa: E402
    from perf_tool.execution import read_native_memory, read_resource_usage  # noqa: E402
    from perf_tool.model import Case  # noqa: E402

PROFILE = "building-m6-paired-static-v1"
PRE_RELEASE_PROFILE = "building-pre-release-paired-static-v1"
GROUP_PROFILE = "building-group-paired-static-v1"
GROUP_BASELINE_PROFILE = "building-group-static-baseline-v1"
GROUP_ENV = "HW_BUILDING_GROUP_COMPARISON"
METRICS = {
    "p95_ms",
    "p99_ms",
    "peak_live_bytes",
    "max_rss_kib",
    "asset_mesh_accessor_bytes",
    *RESOURCE_KEYS,
}


def matrix(*, baseline=False):
    cases = [
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
    return [case for case in cases if case[3] == "legacy-control"] if baseline else cases


def grouped(value):
    return value["profile"] in {GROUP_PROFILE, GROUP_BASELINE_PROFILE}


def label(case):
    return "-".join(map(str, case))


def command(root, case, adapter, *, pre_release=False):
    instrument, size, _repeat, mode = case
    argv = static.command(
        root, instrument, size, 15 if size == "small" else 60, adapter
    )
    argv[argv.index("--repeat") + 1] = "1"
    argv[argv.index("--output") + 1] = str(root / label(case))
    argv.extend(["--building-m6-mode", mode])
    if pre_release:
        argv.extend(["--building-m6-asset-root", str(root / "asset-view")])
    return argv


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


def file_inventory(root):
    pipeline.no_symlinks(root)
    result = {}
    for path in sorted(root.rglob("*")):
        pipeline.require(not path.is_symlink(), "asset inventory cannot contain symlinks")
        if path.is_file():
            result[str(path.relative_to(root))] = pipeline.digest(path.read_bytes())
    return result


def inventory(repo, codec, candidate_roots=None, group=None):
    if group is not None:
        return group_inventory(repo, codec, candidate_roots, group)
    assets = repo / "assets"
    sources = []
    if candidate_roots is None:
        bindings = pipeline.release_bindings(assets, codec, "m5")
        manifests = [pipeline.load_set(assets, item["identity"]["kind"], codec)[0] for item in bindings]
        roots = {m["identity"]["kind"]: assets for m in manifests}
    else:
        from_types = pipeline.KINDS.keys() - {"Bridge"}
        pipeline.require(isinstance(candidate_roots, dict) and set(candidate_roots) == from_types,
                         "pre-release requires all eight candidate roots")
        bindings, manifests, roots = [], [], {}
        for kind in sorted(from_types):
            root = Path(candidate_roots[kind])
            pipeline.require(root.is_absolute() and root == root.resolve(), "canonical candidate root required")
            verified = pipeline.verify_set(root, kind, codec)
            pipeline.require(verified["identity"]["authority"] == "isolated_candidate",
                             "pre-release rejects preview/release authority")
            manifest = pipeline.load_set(root, kind, codec)[0]
            manifests.append(manifest)
            bindings.append({"identity": manifest["identity"], "locator": pipeline.locator(kind)})
            roots[kind] = root
            sources.append({"kind": kind, "root": str(root), "files": file_inventory(root)})
    return assemble_inventory(assets, bindings, manifests, roots, sources if candidate_roots is not None else None)


def group_inventory(repo, codec, candidate_roots, group):
    pipeline.require(group in GROUP_TARGETS and isinstance(candidate_roots, dict)
                     and set(candidate_roots) == set(GROUP_TARGETS[group]), "exact group candidate roots required")
    assets = repo / "assets"
    bindings = [] if group == "m2" else pipeline.release_bindings(assets, codec, "m2" if group == "m3" else "m3")
    manifests, roots, sources = [], {}, []
    for kind in (*GROUP_FIXED[group], *GROUP_TARGETS[group]):
        fixed = kind in GROUP_FIXED[group]
        root = assets if fixed else Path(candidate_roots[kind])
        pipeline.require(root.is_absolute() and root == root.resolve(), "canonical group source root required")
        verified = pipeline.verify_set(root, kind, codec)
        authority = "release_approved" if fixed else "isolated_candidate"
        pipeline.require(verified["identity"]["authority"] == authority, "group source authority differs")
        manifest = pipeline.load_set(root, kind, codec)[0]
        if fixed:
            pipeline.require(next(b["identity"] for b in bindings if b["identity"]["kind"] == kind)
                             == manifest["identity"], "fixed group pointer identity differs")
        else:
            bindings.append({"identity": manifest["identity"], "locator": pipeline.locator(kind)})
        manifests.append(manifest)
        roots[kind] = root
        sources.append({"kind": kind, "root": str(root), "files": file_inventory(root)})
    return assemble_inventory(assets, bindings, manifests, roots, sources)


def group_declaration(assets, group):
    value = {"group": group, "fixed": [], "candidates": []}
    for binding, manifest in zip(assets["bindings"], assets["manifests"], strict=True):
        pipeline.require(binding["identity"] == manifest["identity"]
                         and binding["locator"] == pipeline.locator(manifest["identity"]["kind"]),
                         "group binding/manifest differs")
        field = "fixed" if manifest["identity"]["authority"] == "release_approved" else "candidates"
        value[field].append({**binding, "paths": sorted(r["path"] for r in manifest["artifacts"])})
    return validate_group_declaration(value)


def group_budget_reference(value, envelope):
    pipeline.require(isinstance(envelope, dict) and set(envelope) == {"group", "baseline_plan", "limits"}
                     and envelope["group"] == value["group"], "group-specific budget envelope required")
    path = Path(envelope["baseline_plan"])
    pipeline.require(path.is_absolute() and path == path.resolve(), "canonical group baseline plan required")
    baseline = pipeline.read(path)
    pipeline.require(baseline["profile"] == GROUP_BASELINE_PROFILE
                     and pipeline.exact_json(baseline["identity"], value["identity"])
                     and pipeline.exact_json(baseline["group_declaration"], value["group_declaration"]),
                     "group budget requires the same-source same-inventory control baseline")
    result = verify(path)
    pipeline.require(envelope["limits"]["max_relative_mad"] <= baseline["baseline_max_relative_mad"],
                     "group budget cannot loosen the calibrated noise gate")
    root = Path(baseline["job_root"])
    return {"plan": str(path), "plan_sha256": pipeline.digest(path.read_bytes()), "root": str(root),
            "result_sha256": pipeline.digest((root / "result.json").read_bytes()), "files": file_inventory(root),
            "adapter": result["rows"][0]["adapter"], "window": result["rows"][0]["window"]}


def check_baseline_reference(value):
    required = value["profile"] == GROUP_PROFILE
    pipeline.require(("baseline_reference" in value) == required, "baseline reference/profile mismatch")
    if not required:
        return
    reference = value["baseline_reference"]
    pipeline.require(set(reference) == {"plan", "plan_sha256", "root", "result_sha256", "files", "adapter", "window"},
                     "baseline reference fields differ")
    path, root = Path(reference["plan"]), Path(reference["root"])
    pipeline.require(path.is_absolute() and path == path.resolve() and root.is_absolute() and root == root.resolve()
                     and pipeline.digest(path.read_bytes()) == reference["plan_sha256"]
                     and pipeline.digest((root / "result.json").read_bytes()) == reference["result_sha256"]
                     and file_inventory(root) == reference["files"], "frozen group baseline changed")
    baseline = pipeline.read(path)
    result = native.read_json(root / "result.json")
    pipeline.require(baseline["profile"] == GROUP_BASELINE_PROFILE and baseline["job_root"] == str(root)
                     and pipeline.exact_json(baseline["identity"], value["identity"])
                     and pipeline.exact_json(baseline["group_declaration"], value["group_declaration"]),
                     "baseline reference belongs to another source/group")
    pipeline.require(pipeline.exact_json(verify(path), result),
                     "baseline result differs from independently verified evidence")
    pipeline.require(value["budget"]["max_relative_mad"] <= baseline["baseline_max_relative_mad"],
                     "group budget cannot loosen the calibrated noise gate")
    pipeline.require(pipeline.exact_json(reference["adapter"], result["rows"][0]["adapter"])
                     and pipeline.exact_json(reference["window"], result["rows"][0]["window"]),
                     "baseline environment reference differs")


def assemble_inventory(assets, bindings, manifests, roots, sources):
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
    entries = [(assets, r) for r in [*door["core"], door["receipt"]]]
    entries.extend((roots[m["identity"]["kind"]], r) for m in manifests for r in m["artifacts"])
    for source, record in entries:
        payload = pipeline.file_bytes(source, record)
        if record["path"] in records:
            pipeline.require(records[record["path"]]["sha256"] == pipeline.digest(payload), "asset path collision")
        records[record["path"]] = {
            "bytes": len(payload),
            "sha256": pipeline.digest(payload),
            "mesh_accessor_payload_bytes": mesh_accessor_bytes(payload)
            if record["path"].endswith(".glb")
            else 0,
        }
    result = {"bindings": bindings, "manifests": manifests, "files": records}
    if sources is not None:
        result["sources"] = sources
    return result


def check_inputs(value):
    pipeline.require(
        value["profile"] in {PROFILE, PRE_RELEASE_PROFILE, GROUP_PROFILE, GROUP_BASELINE_PROFILE}
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
    pre_release = value["profile"] != PROFILE
    check_foreground_verifier(value)
    check_candidate_verifiers(value)
    roots = value.get("candidate_roots")
    pipeline.require((roots is not None) == pre_release, "candidate roots/profile mismatch")
    is_group = grouped(value)
    pipeline.require(("group" in value) == is_group and ("group_declaration" in value) == is_group,
                     "group fields/profile mismatch")
    pipeline.require(inventory(repo, codec, roots, value.get("group")) == value["assets"], "M6 asset inputs changed")
    if is_group:
        pipeline.require(pipeline.exact_json(value["group_declaration"], group_declaration(value["assets"], value["group"])),
                         "group declaration differs from frozen manifests")
    baseline = value["profile"] == GROUP_BASELINE_PROFILE
    pipeline.require(("baseline_max_relative_mad" in value) == baseline, "baseline noise/profile mismatch")
    if baseline:
        pipeline.require(value["budget"] is None and type(value["baseline_max_relative_mad"]) is float
                         and value["baseline_max_relative_mad"] == .1 and value.get("foundation") is None,
                         "baseline is control calibration only, not an increment budget")
    else:
        budget(value["budget"])
    check_baseline_reference(value)
    static.check_root(repo, Path(value["job_root"]))
    if value.get("foundation") is not None:
        frozen_foundation(value)


def check_foreground_verifier(value):
    pipeline.require(value.get("foreground_verifier_sha256") == art.foreground.verifier_sha256(),
                     "M6 foreground verifier changed")


def check_candidate_verifiers(value):
    if value["profile"] != PROFILE:
        pipeline.require(value.get("numeric_verifiers") == pipeline.numeric_verifier_inventory()
                         and value.get("art_driver_sha256") == pipeline.digest(Path(art.__file__).read_bytes()),
                         "pre-release provenance verifiers changed")
    else:
        pipeline.require("numeric_verifiers" not in value and "art_driver_sha256" not in value,
                         "candidate verifier binding requires pre-release profile")


def expected_view(value):
    files = file_inventory(Path(value["repo"]) / "assets")
    for source, manifest in zip(value["assets"]["sources"], value["assets"]["manifests"], strict=True):
        root = Path(source["root"])
        for record in manifest["artifacts"]:
            path, sha = record["path"], record["sha256"]
            pipeline.require(path not in files or files[path] == sha, "candidate artifact collides with primary")
            files[path] = sha
        locator = pipeline.locator(source["kind"])
        files[locator] = pipeline.digest((root / locator).read_bytes())
    return files


def prepare_view(value):
    view = Path(value["job_root"]) / "asset-view"
    expected = expected_view(value)
    shutil.copytree(Path(value["repo"]) / "assets", view / "assets")
    for source, manifest in zip(value["assets"]["sources"], value["assets"]["manifests"], strict=True):
        root = Path(source["root"])
        for record in manifest["artifacts"]:
            pipeline.put(pipeline.rooted(view / "assets", record["path"]), pipeline.file_bytes(root, record))
        locator = pipeline.locator(source["kind"])
        pipeline.put(pipeline.rooted(view / "assets", locator), (root / locator).read_bytes(), replace=True)
    pipeline.require(file_inventory(view / "assets") == expected, "constructed candidate view differs")


def check_view(value):
    if value["profile"] != PROFILE:
        pipeline.require(file_inventory(Path(value["job_root"]) / "asset-view/assets") == expected_view(value),
                         "candidate view changed")


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
    baseline = getattr(args, "baseline", False)
    pipeline.require(not baseline or (getattr(args, "group", None) is not None and args.budget is None),
                     "baseline requires a group and no candidate increment budget")
    pipeline.require(baseline or args.budget is not None, "comparison requires a complete candidate increment budget")
    supplied_budget = None if baseline else pipeline.read(args.budget)
    group = getattr(args, "group", None)
    if group is not None and not baseline:
        pipeline.require(isinstance(supplied_budget, dict)
                         and set(supplied_budget) == {"group", "baseline_plan", "limits"},
                         "group-specific budget envelope required")
    value = {
        "profile": GROUP_BASELINE_PROFILE if baseline else GROUP_PROFILE if getattr(args, "group", None) else PRE_RELEASE_PROFILE if getattr(args, "candidate_roots", None) else PROFILE,
        "repo": str(repo),
        "job_root": str(root),
        "codec": str(codec),
        "codec_sha256": pipeline.digest(codec.read_bytes()),
        "driver_sha256": pipeline.digest(Path(__file__).read_bytes()),
        "foreground_verifier_sha256": art.foreground.verifier_sha256(),
        "identity": static.identity(repo),
        "budget": None if baseline else budget(supplied_budget["limits"] if group is not None else supplied_budget),
        "adapter": args.adapter,
        "accepted": False,
        "promotion_authority": False,
        "status": "ready",
    }
    roots = pipeline.read(args.candidate_roots) if getattr(args, "candidate_roots", None) else None
    if roots is not None:
        value["candidate_roots"] = roots
        value["numeric_verifiers"] = pipeline.numeric_verifier_inventory()
        value["art_driver_sha256"] = pipeline.digest(Path(art.__file__).read_bytes())
    pipeline.require(group is None or roots is not None, "group comparison requires explicit candidate roots")
    value["assets"] = inventory(repo, codec, roots, group)
    if group is not None:
        value["group"] = group
        value["group_declaration"] = group_declaration(value["assets"], group)
    if baseline:
        value["baseline_max_relative_mad"] = .1
    elif group is not None:
        value["baseline_reference"] = group_budget_reference(value, supplied_budget)
    pipeline.require(
        (args.foundation_baseline is None) == (args.foundation_identity is None),
        "foundation root and identity must be paired",
    )
    value["foundation"] = None
    if args.foundation_baseline is not None:
        baseline_root = args.foundation_baseline.resolve()
        value["foundation"] = {
            "root": str(baseline_root),
            "identity": pipeline.read(args.foundation_identity),
            "manifest_sha256": pipeline.digest(
                (baseline_root / "manifest.json").read_bytes()
            ),
        }
        baseline_root, manifest = frozen_foundation(value)
        art.verify_frozen_static(baseline_root, manifest, value["foundation"]["identity"])
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
    value["commands"] = [command(root, case, args.adapter, pre_release=roots is not None) for case in matrix(baseline=baseline)]
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
    pipeline.require(requested.get("RUST_LOG") == "info,wgpu=error", "M6 renderer logging differs")
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
    if value["profile"] == GROUP_PROFILE:
        reference = value["baseline_reference"]
        pipeline.require(validation.adapter == reference["adapter"] and validation.window == reference["window"],
                         "group calibration/comparison environment drift")
    sidecar = native.read_json(run / "data/building_art_static.json")
    if value["profile"] != PROFILE:
        try:
            from .perf_tool.artifact_readers.building_m6 import validate_pre_release_sidecar
        except ImportError:
            from perf_tool.artifact_readers.building_m6 import validate_pre_release_sidecar
        observed = (validate_group_sidecar(sidecar, case) if grouped(value)
                    else validate_pre_release_sidecar(sidecar, case))
        pipeline.require(requested.get("BEVY_ASSET_ROOT") == str(Path(value["job_root"]) / "asset-view"),
                         "pre-release view differs")
        if grouped(value):
            pipeline.require("HW_BUILDING_M6_CANDIDATES" not in requested
                and pipeline.exact_json(json.loads(requested.get(GROUP_ENV, "null")), value["group_declaration"])
                and pipeline.exact_json(observed["m6"]["group"], value["group_declaration"]),
                "group requested/frozen manifest declaration differs")
        else:
            pipeline.require(GROUP_ENV not in requested
                and json.loads(requested.get("HW_BUILDING_M6_CANDIDATES", "null")) == value["assets"]["bindings"],
                "pre-release requested identities differ")
    else:
        observed = validate_sidecar(sidecar, case)
    pipeline.require(
        observed["m6"]["mode"] == mode
        and pipeline.exact_json(observed["m6"]["identities"],
             [binding["identity"] for binding in value["assets"]["bindings"]]),
        "paired authority/mode differs",
    )
    loaded = (observed["m6"]["loaded_identities"] if grouped(value)
              else observed["m6"]["identities"] if mode == "candidate" else [])
    loaded_kinds = {identity["kind"] for identity in loaded}
    if loaded_kinds:
        manifests = {
            manifest["identity"]["kind"]: manifest
            for manifest in value["assets"]["manifests"]
        }
        for row in observed["records"]:
            if row["kind"] not in loaded_kinds:
                continue
            expected = projected_parts(manifests[row["kind"]], row)
            if row["kind"] in art.foreground.KINDS:
                asset_manifest = manifests[row["kind"]]
                preview = asset_manifest["world_preview"]
                role = "world_on" if row["kind"] == "OutdoorLamp" and row["ordinal"] % 4 >= 2 else preview["image_role"]
                art.foreground.validate_record(row, asset_manifest["identity"], preview,
                    {record["role"]: record for record in asset_manifest["artifacts"]}, role)
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
    loaded_paths = {record["path"] for manifest in value["assets"]["manifests"]
                    if manifest["identity"]["kind"] in loaded_kinds for record in manifest["artifacts"]}
    metrics["asset_mesh_accessor_bytes"] = sum(
        record["mesh_accessor_payload_bytes"]
        for path, record in value["assets"]["files"].items()
        if path in loaded_paths or path.startswith("door_sets/")
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


def projected_parts(manifest, row):
    """Exact authored f32 poses plus the declared paused Tank state, never epsilon."""
    result = []
    for authored in sorted(manifest["parts"], key=lambda part: part["mesh_role"]):
        part = dict(authored)
        for key, length in (("translation_wu", 3), ("rotation_xyzw", 4), ("scale", 3)):
            numbers = part[key]
            pipeline.require(isinstance(numbers, list) and len(numbers) == length
                             and all(type(n) in (int, float) and math.isfinite(n) for n in numbers),
                             "invalid authored M6 pose")
            part[key] = [struct.unpack("<f", struct.pack("<f", n))[0] for n in numbers]
            pipeline.require(all(math.isfinite(n) for n in part[key]), "nonfinite f32 M6 pose")
        if row["kind"] == "Tank" and part["mesh_role"] == "water":
            state = manifest["production_state"]
            pipeline.require(state["kind"] == "Tank", "Tank motion contract differs")
            y = state["full_y_wu" if row["state"]["stored_water"] >= 50 else "partial_y_wu"]
            part["translation_wu"][1] = struct.unpack("<f", struct.pack("<f", y))[0]
        result.append(part)
    return result


def mode_summary(rows, max_relative_mad):
    pipeline.require(sorted(row["repeat"] for row in rows) == [1, 2, 3], "three distinct repeats required")
    keys = set(rows[0]["metrics"])
    pipeline.require(all(set(row["metrics"]) == keys for row in rows), "metric inventory drift")
    result = {}
    for metric in rows[0]["metrics"]:
        values = [row["metrics"][metric] for row in rows]
        pipeline.require(all(type(n) in (int, float) and math.isfinite(n) and n >= 0 for n in values), "invalid metric")
        median = statistics.median(values)
        mad = statistics.median(abs(n - median) for n in values)
        pipeline.require(mad <= median * max_relative_mad, f"noise budget exceeded: {metric}")
        result[metric] = {"median": median, "mad": mad}
    return result


def check_completed_subsets(rows, limits):
    """Fail early on already conclusive real rows; never report a partial pass."""
    pipeline.require(rows and all(row["adapter"] == rows[0]["adapter"] and row["window"] == rows[0]["window"]
                                 for row in rows), "Capture/Memory environment drift")
    keys = [(r["instrumentation"], r["size"], r["repeat"], r["mode"]) for r in rows]
    pipeline.require(len(set(keys)) == len(keys), "duplicate verified case")
    binaries = {}
    for instrument in {r["instrumentation"] for r in rows}:
        selected = [r for r in rows if r["instrumentation"] == instrument]
        binaries[instrument] = {r["binary_sha256"] for r in selected}
        pipeline.require(len(binaries[instrument]) == 1, "same-source pair binary drift")
        for mode in {r["mode"] for r in selected}:
            for metric in RESOURCE_KEYS:
                pipeline.require(len({r["metrics"][metric] for r in selected if r["mode"] == mode}) == 1,
                                 f"N/4N shared resources did not plateau: {mode}/{metric}")
        for size in {r["size"] for r in selected}:
            summaries = {}
            for mode in {r["mode"] for r in selected if r["size"] == size}:
                subset = [r for r in selected if r["size"] == size and r["mode"] == mode]
                pipeline.require(len(subset) <= 3, "too many verified repeats")
                if len(subset) == 3:
                    summaries[mode] = mode_summary(subset, .1 if limits is None else limits["max_relative_mad"])
            if limits is not None and set(summaries) == {"legacy-control", "candidate"}:
                for metric, after in summaries["candidate"].items():
                    pipeline.require(after["median"] - summaries["legacy-control"][metric]["median"] <= limits["max_delta"][metric],
                                     f"M6 budget exceeded: {instrument}/{size}/{metric}")
    if set(binaries) == {"capture", "memory"}:
        pipeline.require(binaries["capture"] != binaries["memory"], "Capture/Memory binary not separated")


def aggregate_baseline(rows):
    pipeline.require(sorted((r["instrumentation"], r["size"], r["repeat"], r["mode"]) for r in rows)
                     == sorted(matrix(baseline=True)), "complete control-only baseline matrix required")
    check_completed_subsets(rows, None)
    return [{"instrumentation": instrument, "size": size,
             "modes": {"legacy-control": mode_summary(
                 [r for r in rows if r["instrumentation"] == instrument and r["size"] == size], .1)}}
            for instrument in ("capture", "memory") for size in ("small", "medium")]


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
                modes[mode] = mode_summary(group, limits["max_relative_mad"])
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
    check_view(value)
    job = native.read_json(Path(value["job_root"]) / "job.json")
    pipeline.require(
        job["plan_sha256"] == pipeline.digest(plan_path.read_bytes())
        and job["status"] == ("running" if in_progress else "valid"),
        "plan changed or job did not finish",
    )
    baseline = value["profile"] == GROUP_BASELINE_PROFILE
    rows = [verify_case(value, case) for case in matrix(baseline=baseline)]
    result = {
        "profile": value["profile"],
        "status": "pass",
        "scope": "same-source-group-control-baseline-only" if baseline else "same-source-paused-group-increment-only" if grouped(value) else "same-source-paused-asset-increment-only",
        "accepted": False,
        "promotion_authority": False,
        "source": value["identity"],
        "capture_runs": 6 if baseline else 12,
        "memory_runs": 6 if baseline else 12,
        "rows": rows,
        "aggregate": aggregate_baseline(rows) if baseline else aggregate(rows, value["budget"]),
        "unverified": [
            "separate-source M1-0 cumulative cost",
            "active state/lifecycle",
            "actual-window art",
            "release",
        ],
    }
    if grouped(value):
        result["group"] = value["group"]
        result["group_declaration"] = value["group_declaration"]
    if baseline:
        result["unverified"].insert(0, "candidate increment and group performance acceptance")
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
    pipeline.require(value == args.frozen_plan, "plan changed across activity admission")
    check_inputs(value)
    repo, root = Path(value["repo"]), Path(value["job_root"])
    resources = native.resource_snapshot(repo, require_launcher=False)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    root.mkdir(parents=True, exist_ok=False)
    state = {
        "profile": value["profile"],
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
            "HW_BUILDING_ASSET_RELEASES",
            "HW_BUILDING_M6_CANDIDATES",
            GROUP_ENV,
            "HW_BUILDING_ORDINARY_ART_SESSION",
            "HW_BUILDING_PRODUCTION_OBSERVATIONS",
            "HW_NATIVE_UI_ROOT",
        ):
            environment.pop(key, None)
        pre_release = value["profile"] != PROFILE
        if pre_release:
            prepare_view(value)
        if grouped(value):
            environment[GROUP_ENV] = json.dumps(value["group_declaration"])
        else:
            environment["HW_BUILDING_M6_CANDIDATES" if pre_release else "HW_BUILDING_ASSET_RELEASES"] = json.dumps(
                value["assets"]["bindings"]
            )
        rows = []
        for case in matrix(baseline=value["profile"] == GROUP_BASELINE_PROFILE):
            pipeline.require(
                pipeline.read(args.plan) == value, "plan changed during execution"
            )
            check_inputs(value)
            check_view(value)
            native.run_command(
                label(case),
                command(root, case, value["adapter"], pre_release=pre_release),
                repo=repo,
                env=environment,
                log_path=root / (label(case) + ".log"),
                job_file=root / "job.json",
                state=state,
                timeout_seconds=3600,
            )
            rows.append(verify_case(value, case))
            native.atomic_write_json(root / "verified_rows.json", rows)
            check_completed_subsets(rows, value["budget"])
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
    create.add_argument("--budget", type=Path)
    create.add_argument("--candidate-roots", type=Path,
                        help="explicit candidate root map: eight kinds by default or only the --group targets")
    create.add_argument("--group", choices=tuple(GROUP_TARGETS),
                        help="compare only this isolated candidate group against the fixed prior releases")
    create.add_argument("--baseline", action="store_true",
                        help="group control calibration only; requires no increment budget and never runs candidate")
    create.add_argument("--adapter", required=True)
    create.add_argument("--job-root", type=Path)
    create.add_argument("--output", type=Path, required=True)
    create.add_argument("--foundation-baseline", type=Path)
    create.add_argument("--foundation-identity", type=Path)
    for action in ("run", "verify"):
        sub.add_parser(action).add_argument("--plan", type=Path, required=True)
    args = parser.parse_args()
    if args.action == "run":
        # The common activity decorator must receive the frozen repository and
        # exact output root before it checks admission or acquires the lease.
        value = pipeline.read(args.plan)
        args.repo, args.job_root = value["repo"], value["job_root"]
        args.frozen_plan = value
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
