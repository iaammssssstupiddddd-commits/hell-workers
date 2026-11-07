"""Building-art native introduction recipe, separate from Wall/Door acceptance.

Feedback is an incremental development view. Art-preview is clean-source visual
review without art approval. Candidate requires the separately art-approved set.
All three measure only the paused presentation fixture; they neither approve
art nor replace later per-kind lifecycle/state/performance acceptance. Cross-M1
cost comparison consumes two separately frozen static-reference jobs.
"""
from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools/blender_ai_workflow/scripts"))
HELPERS = ROOT / ".codex/skills/hell-workers-run-native-acceptance/scripts"
sys.path.insert(0, str(HELPERS))
import building_asset_pipeline as pipeline  # noqa: E402
import native_acceptance as native  # noqa: E402
from wall_density_acceptance import asset_view_fingerprint  # noqa: E402
if __package__:
    from . import building_art_static_acceptance as static  # noqa: E402
    from .perf_tool.artifacts import read_window  # noqa: E402
else:
    import building_art_static_acceptance as static  # noqa: E402
    from perf_tool.artifacts import read_window  # noqa: E402

MODES = {"feedback": "art_preview", "art-preview": "art_preview", "candidate": "isolated_candidate"}
PROFILE = "building-art"
VIEWS = {"overview": 5.0, "near": 0.5, "normal": 1.0, "far": 2.0}
QUALITIES = ("high", "medium", "low")
CONTEXT_SCOPE = "seeded-paused-art-comparison"
SPA_PAGE_SCOPE = "paused-static-spa-mask-page"


def spa_page(value: dict) -> int | None:
    page = value.get("spa_mask_page")
    if page is None:
        return None
    pipeline.require(type(page) is int and 0 <= page < 4
        and value.get("kind") == "SoulSpa" and value.get("mode") == "candidate"
        and value.get("identity", {}).get("authority") == "isolated_candidate"
        and value.get("variant", "gallery") == "gallery"
        and value.get("lighting", "standard") == "standard"
        and value.get("view") in ("near", "normal", "far")
        and value.get("scope") == SPA_PAGE_SCOPE and value.get("promotion_authority") is False,
        "Spa page requires exact isolated-candidate gallery scope")
    return page


def actor_count(value: dict) -> int:
    page = spa_page(value)
    if page is not None:
        return 8 + sum(mask.bit_count() for mask in range(page * 4, page * 4 + 4))
    return 6 if context_settings(value) else 15


def context_preview(manifest: dict) -> dict:
    preview = manifest["world_preview"]
    artifacts = [entry for entry in manifest["artifacts"] if entry["role"] == "image:" + preview["image_role"]]
    pipeline.require(len(artifacts) == 1, "context requires one exact image: preview artifact")
    return {"descriptor": preview, "artifact": artifacts[0]}


def context_settings(value: dict) -> bool:
    variant = value.get("variant", "gallery")
    lighting = value.get("lighting", "standard")
    pipeline.require(variant in ("gallery", "rest-area-art-context")
                     and lighting in ("standard", "dim"), "unknown art variant/lighting")
    if variant == "gallery":
        pipeline.require(lighting == "standard", "gallery lighting cannot change")
        return False
    pipeline.require(value.get("kind") == "RestArea"
        and value.get("mode") in ("feedback", "art-preview")
        and value.get("view") in ("near", "normal", "far")
        and value.get("identity", {}).get("authority") == "art_preview"
        and value.get("scope") == CONTEXT_SCOPE
        and value.get("promotion_authority") is False, "context authority/view/scope differs")
    return True


def quality(value: dict) -> str:
    selected = value.get("quality", "high")
    pipeline.require(selected in QUALITIES, "unknown building-art quality")
    return selected


def subject(repo: Path) -> dict:
    return {"commit": native.git_subject(repo), "source": native.source_fingerprint(repo),
            "harness": native.native_harness_fingerprint(repo), "assets": asset_view_fingerprint(repo),
            "driver": pipeline.digest(Path(__file__).read_bytes()), "dirty": native.git_dirty_paths(repo)}


def validate_subject(value: dict, *, feedback: bool) -> None:
    pipeline.require(set(value) == {"commit", "source", "harness", "assets", "driver", "dirty"}
                     and re.fullmatch(r"[0-9a-f]{40}", value["commit"]) is not None
                     and all(re.fullmatch(r"[0-9a-f]{64}", value[k]) for k in ("source", "harness", "assets", "driver")),
                     "incomplete source identity")
    pipeline.require(feedback or value["dirty"] == [], "non-feedback requires a clean source")


def check_plan(value: dict, binary: Path) -> dict:
    context_settings(value)
    page = spa_page(value)
    quality(value)
    pipeline.require(value.get("view", "overview") in VIEWS, "unknown camera view")
    pipeline.require(value["kind"] != "Bridge", "Bridge requires its independent M4 recipe; nine-kind static evidence is inapplicable")
    pipeline.require(value["profile"] == PROFILE and value["mode"] in MODES, "recipe mode differs")
    repo = native.validate_repo(value["repo"])
    validate_subject(value["subject"], feedback=value["mode"] == "feedback")
    pipeline.require(subject(repo) == value["subject"], "frozen recipe subject changed")
    root = pipeline.no_symlinks(Path(value["job_root"]))
    static.check_root(repo, root)
    manifest, text, _ = pipeline.load_set(Path(value["source_root"]), value["kind"], binary)
    pipeline.require(manifest["identity"] == value["identity"]
                     and pipeline.digest(text.encode()) == value["manifest_sha256"]
                     and manifest["identity"]["authority"] == MODES[value["mode"]], "candidate identity/authority changed")
    if context_settings(value):
        pipeline.require(value.get("context_preview") == context_preview(manifest),
            "context preview differs from exact source manifest")
    if page is not None:
        pipeline.require(pipeline.canonical(value.get("spa_parts")) == pipeline.canonical(manifest["parts"]),
                         "Spa page parts differ from exact source manifest")
    if value["mode"] == "candidate":
        # Reuse promotion's art approval/provenance checks without approving or
        # installing the set. This target is only a read-only planning namespace.
        pipeline.plan(Path(value["source_root"]), root / "approval-check", value["kind"], binary)
    return manifest


def plan(args) -> dict:
    repo = native.validate_repo(args.repo)
    frozen = subject(repo)
    validate_subject(frozen, feedback=args.mode == "feedback")
    root = Path(args.job_root).absolute() if args.job_root else native.unique_job_root(repo, "building-art")
    static.check_root(repo, root)
    pipeline.require(not root.exists(), "recipe job root already exists")
    manifest, text, _ = pipeline.load_set(args.source_root, args.kind, args.codec)
    value = {"schema_version": 1, "status": "ready", "profile": PROFILE, "mode": args.mode, "repo": str(repo),
             "kind": args.kind, "source_root": str(args.source_root.absolute()), "job_root": str(root),
             "subject": frozen, "identity": manifest["identity"], "manifest_sha256": pipeline.digest(text.encode()),
             "adapter": args.adapter, "codec": str(args.codec.absolute()),
             "codec_sha256": pipeline.digest(args.codec.read_bytes()),
             "view": getattr(args, "view", "overview"),
             "quality": getattr(args, "quality", "high"),
             "variant": getattr(args, "variant", "gallery"),
             "lighting": getattr(args, "lighting", "standard"),
             "spa_mask_page": getattr(args, "spa_mask_page", None),
             "scope": CONTEXT_SCOPE if getattr(args, "variant", "gallery") == "rest-area-art-context" else SPA_PAGE_SCOPE if getattr(args, "spa_mask_page", None) is not None else "paused-static-presentation", "promotion_authority": False,
             "feedback_only": args.mode == "feedback"}
    if value["variant"] == "rest-area-art-context":
        value["context_preview"] = context_preview(manifest)
    if spa_page(value) is not None:
        value["spa_parts"] = manifest["parts"]
    check_plan(value, args.codec)
    resources = native.resource_snapshot(repo, require_launcher=True)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    pipeline.require(args.output is not None, "plan requires --output")
    value["launcher_command"] = ["kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1", "python3",
        str(repo / "scripts/building_art_acceptance.py"), "run", "--plan", str(args.output.absolute())]
    value["resources"] = resources
    pipeline.put(args.output, pipeline.canonical(value))
    return value


def game_command(binary: Path, root: Path, value: dict) -> list[str]:
    context_settings(value)
    return [str(binary), "--perf-scenario", "--perf-seed", str(static.SEED), "--perf-size", "small",
        "--perf-workload", "building-art-static", "--perf-render", "gpu", "--perf-clock", "realtime",
        "--perf-familiar-policy", "baseline", "--perf-operation-dialog", "hidden", "--perf-dashboard", "hidden",
        "--perf-output-dir", str(root / "data"), "--perf-warmup-secs", "30", "--perf-measure-secs", "60",
        "--spawn-souls", str(actor_count(value)), "--spawn-familiars", "0", "--perf-window-width", "1280", "--perf-window-height", "720",
        "--perf-window-scale-factor", "1", "--perf-rtt-quality", quality(value)]


def validate_window(root: Path, value: dict) -> None:
    """Check actual quality/target dimensions, not just the requested argv.

    This reuses the window-only validator: paused art fixtures cannot satisfy
    ordinary gameplay performance predicates and grant no performance authority.
    """
    window, errors = read_window(root / "data/window.csv", expect_headless=False,
        expected_width=1280, expected_height=720, expected_scale_factor=1.0,
        expected_rtt_quality=quality(value), expected_window_backend="x11",
        expected_backend="vulkan", expected_present_mode="novsync")
    pipeline.require(window is not None and not errors, "building-art window differs: " + "; ".join(errors))
    pipeline.require(value["adapter"].casefold() in window["adapter_name"].casefold(),
                     "building-art window adapter differs")


def game_environment(environment: dict, view: Path, value: dict, nonce: str, root: Path) -> dict:
    """Child-only controls: preserve required INFO-level adapter evidence."""
    environment = environment.copy()
    for key in list(environment):
        if key.startswith(("HW_BUILDING_ART", "HW_WALL", "HW_DOOR", "HW_DISABLE_", "HW_PERF_")):
            del environment[key]
    environment.update({"BEVY_ASSET_ROOT": str(view), "HW_WINDOW_BACKEND": "x11", "HW_PRESENT_MODE": "novsync",
        "WGPU_BACKEND": "vulkan", "WGPU_ADAPTER_NAME": value["adapter"], "RUST_LOG": "info,wgpu=error",
        "HW_BUILDING_ART_SESSION": json.dumps({"mode": value["mode"], "identity": value["identity"],
            "locator": pipeline.locator(value["kind"]), "nonce": nonce, "view": value.get("view", "overview"),
            "variant": value.get("variant", "gallery"), "lighting": value.get("lighting", "standard"),
            "spa_mask_page": value.get("spa_mask_page"),
            "status_path": str(root / "probe.json")})})
    return environment


def capture(root: Path, pid: int) -> dict | None:
    clients = native.x11_client_windows_for_process_tree(pid)
    if not clients:
        return None
    pipeline.require(len(clients) == 1, "building-art requires exactly one owned client")
    window, owner = clients[0]
    command = shutil.which("import")
    pipeline.require(command is not None, "ImageMagick import is unavailable")
    path = root / "actual-window.png"
    if not native.capture_client_png(window, path, import_cmd=command):
        return None
    pipeline.require(native.validate_png_structure(path.read_bytes()) == (1280, 720), "actual client dimensions differ")
    return {"path": path.name, "sha256": pipeline.digest(path.read_bytes()), "window_id": window,
            "window_pid": owner, "capture_scope": "owned-x11-client"}


def validate_probe(probe: dict, value: dict, nonce: str) -> None:
    pipeline.require(probe.get("profile") == PROFILE and probe.get("status") == "ready"
        and probe.get("mode") == value["mode"] and probe.get("identity") == value["identity"]
        and probe.get("nonce") == nonce and probe.get("scope") == value["scope"]
        and type(probe.get("stable_frames")) is int and probe["stable_frames"] >= 30,
        "building-art runtime acknowledgement differs")
    if context_settings(value):
        validate_context_fixture(probe, value)
    else:
        pipeline.require(probe["fixture"]["target_count"] == 36, "building-art fixture inventory differs")
        if spa_page(value) is not None:
            validate_spa_page(probe, value)
    view = value.get("view", "overview")
    pipeline.require(view in VIEWS and probe.get("view", "overview") == view,
                     "building-art camera view differs")
    if "view" in value:
        pipeline.require(probe.get("camera_scale") == VIEWS[view]
                         and probe["fixture"].get("view") == view
                         and probe["fixture"].get("camera_scale") == VIEWS[view],
                         "building-art camera scale differs")


def validate_spa_page(probe: dict, value: dict) -> None:
    page = spa_page(value)
    fixture = probe["fixture"]
    pipeline.require(type(probe.get("spa_mask_page")) is int and probe["spa_mask_page"] == page
        and type(fixture.get("spa_mask_page")) is int and fixture["spa_mask_page"] == page
        and fixture.get("scope") == SPA_PAGE_SCOPE and fixture.get("souls") == actor_count(value),
        "Spa page/actor evidence differs")
    records = [record for record in fixture["records"] if record["kind"] == "SoulSpa"]
    kinds = {"Tank", "MudMixer", "RestArea", "SoulSpa", "WheelbarrowParking", "SandPile", "BonePile", "Door", "OutdoorLamp"}
    pipeline.require(len(fixture["records"]) == 36 and {record["kind"] for record in fixture["records"]} == kinds
        and all(sorted(record["ordinal"] for record in fixture["records"] if record["kind"] == kind) == list(range(4)) for kind in kinds),
        "Spa page full gallery inventory differs")
    pipeline.require(len(records) == 4 and sorted(record["ordinal"] for record in records) == list(range(4)),
                     "Spa page owner ordinals differ")
    expected = {part["name"]: part for part in value["spa_parts"]}
    pipeline.require(set(expected) == {"body", "slot0", "slot1", "slot2", "slot3"}, "Spa descriptor names differ")
    entities = []
    for record in records:
        entities.extend([record.get("owner"), record.get("root")])
        pipeline.require(pipeline.canonical(record.get("root_identity")) == pipeline.canonical(value["identity"]),
                         "Spa consumed root generation differs")
        center = record.get("center")
        pipeline.require(isinstance(center, list) and len(center) == 2
            and all(type(v) in (int, float) and math.isfinite(v) for v in center), "Spa center differs")
        for field, wanted in (("root_translation_wu", [center[0], 0, -center[1]]),
                              ("root_rotation_xyzw", [0, 0, 0, 1]), ("root_scale", [1, 1, 1])):
            actual = record.get(field)
            pipeline.require(isinstance(actual, list) and len(actual) == len(wanted)
                and all(type(a) in (int, float) and math.isfinite(a) and abs(a-b) <= .001
                        for a, b in zip(actual, wanted, strict=True)), "Spa actual root pose differs")
        mask = page * 4 + record["ordinal"]
        pipeline.require(type(record["state"].get("mask")) is int and record["state"]["mask"] == mask
            and record["state"].get("operational") is True, "Spa actual tile mask differs")
        parts = record["parts"]
        pipeline.require(len(parts) == 5 and {part.get("name") for part in parts} == set(expected), "Spa actual part names differ")
        for part in parts:
            entities.append(part.get("entity"))
            original = expected[part["name"]]
            active = part["name"] != "body" and bool(mask & (1 << int(part["name"][-1])))
            pipeline.require(part.get("mesh_role") == original["mesh_role"]
                and type(part.get("parent")) is int and part["parent"] == record["root"]
                and part.get("material_role") == original["material_role"]
                and part.get("visible") is True and part.get("slot_active") is active
                and part.get("emissive_white") is active and part.get("emissive_black") is (not active)
                and part.get("emissive_texture_matches") is True and part.get("white_opaque_lit") is True,
                "Spa actual material/mask differs")
            for field in ("translation_wu", "rotation_xyzw", "scale"):
                actual = part.get(field)
                pipeline.require(isinstance(actual, list) and len(actual) == len(original[field])
                    and all(type(a) in (int, float) and math.isfinite(a) and abs(a-b) <= .001
                            for a, b in zip(actual, original[field], strict=True)), "Spa actual part pose differs")
            pipeline.require(all(type(part.get(field)) in (int, float) and part[field] == expected_value
                for field, expected_value in (("roughness", 1), ("metallic", 0), ("reflectance", 0)))
                and isinstance(part.get("material_values"), str) and part["material_values"], "Spa actual material values differ")
    pipeline.require(all(type(entity) is int and entity >= 0 for entity in entities)
        and len(set(entities)) == len(entities), "Spa owner/root/part identities overlap or are missing")


def validate_context_fixture(probe: dict, value: dict) -> None:
    """Seeded image comparison only; never issues an art/numeric decision."""
    fixture = probe["fixture"]
    pipeline.require(probe.get("variant") == fixture.get("variant") == "rest-area-art-context"
        and probe.get("lighting") == fixture.get("lighting") == value["lighting"]
        and fixture.get("scope") == CONTEXT_SCOPE and fixture.get("identity") == value["identity"]
        and all(fixture.get(key) is False for key in ("accepted", "performance_evidence", "promotion_authority")),
        "context identity/authority differs")
    pipeline.require(all(type(fixture.get(key)) is int and fixture[key] == count for key, count in
        (("target_count", 4), ("wall_count", 8), ("preview_count", 4), ("souls", 6)))
        and len(fixture.get("records", [])) == 4, "context inventory differs")
    identities = []
    occupied = set()
    def point(raw):
        pipeline.require(isinstance(raw, list) and len(raw) == 2
            and all(type(number) in (int, float) and math.isfinite(number) for number in raw)
            and 0 <= raw[0] <= 1280 and 0 <= raw[1] <= 720, "context projection is non-finite/offscreen")
        return raw
    for ordinal, record in enumerate(fixture["records"]):
        pipeline.require(record.get("ordinal") == ordinal and record.get("occupants") == (0, 1, 5, 0)[ordinal]
            and len(record.get("tiles", [])) == 4 and len(record.get("walls", [])) == 2
            and len(record.get("parts", [])) > 0, "context specimen differs")
        for tile in record["tiles"]:
            pipeline.require(len(tile) == 2 and all(type(number) is int for number in tile)
                and tuple(tile) not in occupied, "context footprint overlaps")
            occupied.add(tuple(tile))
        preview = record["preview"]
        descriptor = preview["descriptor"]
        artifact = preview["artifact"]
        pipeline.require({"descriptor": descriptor, "artifact": artifact} == value["context_preview"]
            and descriptor["image_role"] == "world_preview" and artifact["role"] == "image:world_preview"
            and re.fullmatch(r"[0-9a-f]{64}", artifact["sha256"]) is not None
            and preview["canvas_wu"] == descriptor["canvas_wu"]
            and preview["anchor"] == [descriptor["anchor_px"][0] / descriptor["canvas_px"][0] - .5,
                                      .5 - descriptor["anchor_px"][1] / descriptor["canvas_px"][1]]
            and preview["visible"] is True, "context descriptor preview differs")
        identities.extend([record["owner"], record["root"], preview["entity"]])
        identities.extend(wall["owner"] for wall in record["walls"])
        pipeline.require(all(part["visible"] is True and all(part["material_values"].get(key) is True
            for key in ("white", "opaque", "lit", "emissive_black", "emissive_texture_absent"))
            for part in record["parts"]), "context part hidden or material differs")
        lower, upper = record["walls"]
        ys = [tile[1] for tile in record["tiles"]]
        pipeline.require(lower["grid"][1] < min(ys) and upper["grid"][1] > max(ys)
            and all(len(wall["visuals"]) == 1 and wall["visuals"][0]["visible"] is True
                    and 1 in wall["visuals"][0]["layers"] for wall in (lower, upper)), "context wall ordering differs")
        for wall in record["walls"]:
            pipeline.require(tuple(wall["grid"]) not in occupied, "context wall overlaps")
            occupied.add(tuple(wall["grid"]))
        owner = point(record["owner_screen"])
        completed = point(record["composite_screen"])
        preview_screen = point(preview["screen_anchor"])
        scale = VIEWS[value["view"]]
        pipeline.require(record["preview_offset_wu"] == [0., -160.]
            and all(abs(a - b) <= 1 for a, b in zip(owner, completed, strict=True))
            and abs(preview_screen[0] - owner[0]) <= .01
            and abs(preview_screen[1] - owner[1] - 160 / scale) <= .01, "context anchor correspondence differs")
    pipeline.require(all(type(entity) is int and entity > 0 for entity in identities)
                     and len(set(identities)) == len(identities), "context duplicate entities")
    ambient, sun = (500., 12000.) if value["lighting"] == "standard" else (100., 2400.)
    pipeline.require(len(fixture["ambient"]) == 1 and fixture["ambient"][0]["brightness"] == ambient
        and len(fixture["directional"]) == 2
        and sum(light["primary"] is True and light["illuminance"] == sun and light["shadows"] is True
                for light in fixture["directional"]) == 1
        and sum(light["extra"] is True and light["illuminance"] == 0 and light["shadows"] is False
                for light in fixture["directional"]) == 1, "context actual lights differ")


def verify(root: Path) -> dict:
    result = pipeline.read(root / "manifest.json")
    value = result["plan"]
    binary = Path(result["binary"]["path"])
    check_plan(value, binary)
    pipeline.require(pipeline.digest(binary.read_bytes()) == result["binary"]["sha256"], "native binary changed")
    pipeline.require(result["profile"] == PROFILE and result["status"] == "pass"
                     and result["mode"] == value["mode"] and result["identity"] == value["identity"]
                     and result["feedback_only"] == (value["mode"] == "feedback")
                     and result["promotion_authority"] is False, "evidence authority differs")
    probe = json.loads((root / "probe.json").read_bytes())
    validate_probe(probe, value, result["nonce"])
    pipeline.require(probe == result["probe"], "sealed runtime probe changed")
    screenshot = pipeline.rooted(root, result["screenshot"]["path"])
    pipeline.require(pipeline.digest(screenshot.read_bytes()) == result["screenshot"]["sha256"]
                     and native.validate_png_structure(screenshot.read_bytes()) == (1280, 720), "native screenshot changed")
    sidecar = json.loads((root / "data/building_art_candidate.json").read_bytes())
    pipeline.require(sidecar["evidence_kind"] == "candidate-paused-static-only"
                     and sidecar["initial"] == sidecar["final"] == probe["fixture"]
                     and sidecar["stable_frames"] >= 30, "candidate fixture did not remain stable")
    if context_settings(value):
        pipeline.require(sidecar.get("contract_id") == "rest-area-art-context-v1", "context sidecar contract differs")
    # Older plans remain high-only; their recorded raw window is checked too.
    validate_window(root, value)
    log = (root / "actual-window.log").read_text()
    pipeline.require(not re.search(r"\b(?:WARN|ERROR)\b|bevy_ecs::error::handler", log), "native log contains warning/error")
    adapters = re.findall(r'AdapterInfo \{ name: "([^"]+)".*?backend: ([A-Za-z0-9_]+)', log)
    pipeline.require(len(adapters) == 1 and value["adapter"].casefold() in adapters[0][0].casefold()
                     and adapters[0][1] == "Vulkan", "actual adapter/backend differs")
    return result


@native.activity_locked
def run(args) -> dict:
    pipeline.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "use the planned no-prompt launcher")
    value = pipeline.read(args.plan)
    codec = Path(value["codec"])
    pipeline.require(pipeline.digest(codec.read_bytes()) == value["codec_sha256"], "planning codec changed")
    manifest = check_plan(value, codec)
    repo, root = Path(value["repo"]), Path(value["job_root"])
    root.mkdir(parents=True, exist_ok=False)
    job = root / "job.json"
    state = {"status": "running", "profile": PROFILE, "pid": os.getpid(), "started_at": native.utc_now(),
             "heartbeat_at": native.utc_now(), "mode": value["mode"]}
    native.atomic_write_json(job, state)
    try:
        feedback = value["mode"] == "feedback"
        environment = native.cargo_environment(repo)
        build = ["python3", "scripts/dev.py", "feedback", "--build-only"] if feedback else [
            "python3", "scripts/dev.py", "cargo", "--", "build", "--profile", "profiling",
            "--no-default-features", "--features", "profiling"]
        native.run_command("build", build, repo=repo, env=environment, log_path=root / "build.log",
                           job_file=job, state=state, timeout_seconds=3600)
        binary = repo / "target" / ("debug" if feedback else "profiling") / "bevy_app"
        check_plan(value, binary)
        # An exact full base asset view is copied. No missing asset is invented
        # as a baseline and no production asset directory is modified.
        view = root / "asset-view"
        shutil.copytree(repo / "assets", view / "assets")
        pipeline.require(asset_view_fingerprint(view) == value["subject"]["assets"], "fallback asset view differs")
        for entry in manifest["artifacts"]:
            pipeline.put(pipeline.rooted(view / "assets", entry["path"]),
                         pipeline.file_bytes(Path(value["source_root"]), entry))
        pipeline.put(pipeline.rooted(view / "assets", pipeline.locator(value["kind"])),
                     pipeline.rooted(Path(value["source_root"]), pipeline.locator(value["kind"])).read_bytes(), replace=True)
        nonce = secrets.token_hex(16)
        environment = game_environment(environment, view, value, nonce, root)
        native.admit_stage_start("actual-window", state=state, job_file=job)
        screenshot = None
        deadline = time.monotonic() + 300
        with (root / "actual-window.log").open("w") as log:
            process = subprocess.Popen(game_command(binary, root, value), cwd=repo, env=environment,
                stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
                pass_fds=native.activity_pass_fds(environment))
            native.update_state(job, state, child_pid=process.pid, current_stage="actual-window")
            try:
                while process.poll() is None:
                    if screenshot is None and (root / "probe.json").exists():
                        probe = json.loads((root / "probe.json").read_bytes())
                        validate_probe(probe, value, nonce)
                        screenshot = capture(root, process.pid)
                    pipeline.require(time.monotonic() < deadline, "building-art native deadline exceeded")
                    native.update_state(job, state, heartbeat_at=native.utc_now())
                    time.sleep(0.5)
            finally:
                if process.poll() is None:
                    native.stop_command_process(process)
        pipeline.require(process.returncode == 0 and screenshot is not None, "native run/capture incomplete")
        check_plan(value, binary)
        result = {"schema_version": 1, "profile": PROFILE, "mode": value["mode"], "status": "pass",
            "identity": value["identity"], "scope": value["scope"], "feedback_only": feedback,
            "promotion_authority": False, "plan": value, "nonce": nonce,
            "probe": json.loads((root / "probe.json").read_bytes()), "screenshot": screenshot,
            "binary": {"path": str(binary), "sha256": pipeline.digest(binary.read_bytes())},
            "not_covered": ["active-simulation", "all-kind-states", "all-preview-consumers", "lifecycle", "performance-budget", "art-approval"]}
        pipeline.put(root / "manifest.json", pipeline.canonical(result))
        verify(root)
        native.update_state(job, state, status="valid", child_pid=None, completed_at=native.utc_now())
        return result
    except Exception as error:
        native.update_state(job, state, status="invalid", failure=str(error), child_pid=None)
        raise


def verify_frozen_static(root: Path, manifest: dict, expected: dict) -> dict:
    """Run the sealed source's verifier with its own Python module inventory.

    Importing both drivers in this interpreter is insufficient: native_acceptance
    and perf_tool would still be cached from the comparison checkout. A separate
    process also keeps historical harness fingerprint definitions source-local.
    """
    repo = native.validate_repo(manifest["repo"])
    driver = pipeline.no_symlinks(repo / "scripts/building_art_static_acceptance.py")
    root = root.resolve()

    def unchanged() -> None:
        pipeline.require(native.git_subject(repo) == expected["subject_commit"]
                         and not native.git_dirty_paths(repo), "frozen verifier source is dirty or changed")
        pipeline.require(driver.is_file() and pipeline.digest(driver.read_bytes()) == expected["driver_sha256"],
                         "frozen verifier driver identity differs")
        pipeline.require(native.read_json(root / "manifest.json") == manifest
                         and manifest["identity"] == expected, "frozen job manifest/identity changed")

    unchanged()
    completed = subprocess.run(
        [sys.executable, "-B", "-E", "-s", str(driver), "verify", "--job-root", str(root)],
        cwd=repo, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        timeout=300, check=False,
    )
    # -B preserves frozen trees; -E/-s exclude inherited Python path overrides
    # and user-site modules. The script directory and its helper imports belong
    # to this source, not to the checkout running the comparison.
    pipeline.require(completed.returncode == 0, "frozen raw-job verification failed: " + completed.stderr)
    result = json.loads(completed.stdout)
    pipeline.require(isinstance(result, dict) and result.get("status") == "pass"
                     and result.get("profile") == manifest["profile"] == static.PROFILE
                     and result.get("subject_commit") == expected["subject_commit"]
                     and result.get("capture_runs") == 6 and result.get("memory_runs") == 3
                     and result.get("active_simulation") == "not-measured"
                     and result.get("sessions") == manifest["sessions"],
                     "frozen verifier result differs from the sealed raw job")
    unchanged()
    return result


def compare(baseline: Path, candidate: Path, baseline_identity: dict, candidate_identity: dict) -> dict:
    """Revalidate raw runs for two clean, separately built fallback subjects."""
    left = native.read_json(baseline / "manifest.json")
    right = native.read_json(candidate / "manifest.json")
    for manifest, expected in ((left, baseline_identity), (right, candidate_identity)):
        pipeline.require(manifest["identity"] == expected and set(expected) == {
            "subject_commit", "source_fingerprint", "harness_fingerprint", "asset_view_fingerprint", "driver_sha256"},
            "full frozen comparison identity differs")
        pipeline.require(re.fullmatch(r"[0-9a-f]{40}", expected["subject_commit"])
            and all(re.fullmatch(r"[0-9a-f]{64}", v) for k, v in expected.items() if k != "subject_commit"),
            "comparison identity is incomplete")
    pipeline.require(baseline_identity["subject_commit"] != candidate_identity["subject_commit"]
        and baseline_identity["source_fingerprint"] != candidate_identity["source_fingerprint"], "comparison requires distinct sources")
    pipeline.require(baseline_identity["asset_view_fingerprint"] == candidate_identity["asset_view_fingerprint"]
        and left["adapter"] == right["adapter"], "comparison assets/adapter differ")
    source_roles(Path(left["repo"]), Path(right["repo"]))
    checked = [verify_frozen_static(root, manifest, expected) for root, manifest, expected in (
        (baseline, left, baseline_identity), (candidate, right, candidate_identity))]
    rows = []
    for before, after in zip(checked[0]["sessions"], checked[1]["sessions"], strict=True):
        pipeline.require(before["instrumentation"] == after["instrumentation"] and before["size"] == after["size"]
            and before["binary_sha256"] != after["binary_sha256"], "comparison is not a separate-source binary pair")
        pipeline.require([r["layout_sha256"] for r in before["observations"]]
            == [r["layout_sha256"] for r in after["observations"]], "fallback fixture layout differs")
        pipeline.require([r["adapter"] for r in before["observations"]]
            == [r["adapter"] for r in after["observations"]], "actual comparison adapters differ")
        rows.append({"instrumentation": before["instrumentation"], "size": before["size"],
                     "before": before["aggregate"], "after": after["aggregate"]})
    return {"status": "compared", "profile": "building-art-foundation", "baseline": baseline_identity,
            "candidate": candidate_identity, "sessions": rows, "budget_decision": "not-evaluated",
            "scope": "paused-static-fallback-only", "promotion_authority": False}


def source_roles(baseline: Path, candidate: Path) -> None:
    marker = "crates/bevy_app/src/assets/building_asset_set/mod.rs"
    fixture = "crates/bevy_app/src/plugins/startup/perf_scenario/building_art_static/mod.rs"
    pipeline.require(not (baseline / marker).exists() and (baseline / fixture).is_file()
                     and (candidate / marker).is_file() and (candidate / fixture).is_file(),
                     "requires the pre-M1 fixture-only source and post-M1 foundation source")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "run", "verify", "status", "compare"))
    for key in ("repo", "source-root", "job-root", "codec", "output", "plan", "baseline", "candidate", "baseline-identity", "candidate-identity"):
        parser.add_argument("--" + key, type=Path)
    parser.add_argument("--kind", choices=tuple(pipeline.KINDS))
    parser.add_argument("--mode", choices=tuple(MODES))
    parser.add_argument("--adapter", default="Intel")
    parser.add_argument("--view", choices=tuple(VIEWS), default="overview")
    parser.add_argument("--quality", choices=QUALITIES, default="high")
    parser.add_argument("--variant", choices=("gallery", "rest-area-art-context"), default="gallery")
    parser.add_argument("--lighting", choices=("standard", "dim"), default="standard")
    parser.add_argument("--spa-mask-page", type=int, choices=range(4))
    args = parser.parse_args()
    if args.command == "plan":
        pipeline.require(all((args.repo, args.source_root, args.codec, args.kind, args.mode)), "incomplete recipe inputs")
        result = plan(args)
    elif args.command == "run":
        value = pipeline.read(args.plan)
        args.repo, args.job_root = value["repo"], value["job_root"]
        result = run(args)
    elif args.command == "verify":
        result = verify(args.job_root)
    elif args.command == "status":
        if native.read_json(args.job_root / "job.json").get("status") == "valid":
            verify(args.job_root)
        return native.status_command(argparse.Namespace(job_root=str(args.job_root), stale_after_secs=120))
    else:
        result = compare(args.baseline, args.candidate, pipeline.read(args.baseline_identity), pipeline.read(args.candidate_identity))
    native.print_json(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
