"""Observe compiled release selection in an unseeded normal actual window.

No release override, dedicated session, or performance fixture is allowed. The
passive game log observes existing consumers only; missing kinds are not spawned.
This proves startup connection, never all-kind lifecycle/art/performance approval.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

if __package__:
    from . import building_m2_native_acceptance as shared
    from . import building_native_executable as executable
    from . import building_body_paint_observation as body_paint
else:
    import building_m2_native_acceptance as shared
    import building_native_executable as executable
    import building_body_paint_observation as body_paint

art, pipeline, native = shared.art, shared.pipeline, shared.native
PROFILE = "building-default-release-startup-v1"
INVENTORY = "crates/bevy_app/src/assets/building_asset_set/default_release_bindings.json"
MARKER = "BUILDING_DEFAULT_RELEASE_AUDIT: "
EXCLUDED_ENV = ("HW_BUILDING_", "HW_M2_", "HW_BRIDGE_", "HW_PERF_", "HW_WALL", "HW_DOOR", "HW_DISABLE_",
                "HW_NATIVE_UI_", "HW_NATIVE_SAVE_LOAD_", "HW_NATIVE_DECONSTRUCTION_", "HW_NATIVE_NOTIFICATION_")


def dependencies():
    return {Path(module.__file__).name: executable.digest(Path(module.__file__))
            for module in (shared, executable, art, body_paint)} | {Path(__file__).name: executable.digest(Path(__file__))}


def read_inventory(path):
    payload = pipeline.no_symlinks(path).read_bytes()
    value = json.loads(payload)
    pipeline.require(isinstance(value, list) and pipeline.canonical(value) == payload,
                     "compiled inventory must be a canonical JSON array")
    return value


def check(value):
    pipeline.require(value["profile"] == PROFILE and value["status"] == "ready"
        and value["accepted"] is False and value["performance_evidence"] is False
        and value["promotion_authority"] is False and value["drivers"] == dependencies()
        and pipeline.exact_json(value["numeric_verifiers"], pipeline.numeric_verifier_inventory()),
        "normal startup contract/dependencies changed")
    repo, codec = Path(value["repo"]), Path(value["codec"])
    art.validate_subject(value["subject"], feedback=False)
    pipeline.require(art.subject(repo) == value["subject"]
        and executable.digest(codec) == value["codec_sha256"]
        and executable.digest(repo / INVENTORY) == value["inventory_sha256"], "normal startup subject changed")
    bindings = read_inventory(repo / INVENTORY)
    groups = {2: "m2", 4: "m3", 8: "m5"}
    pipeline.require(isinstance(bindings, list) and len(bindings) in groups
        and pipeline.exact_json(bindings, value["bindings"])
        and pipeline.exact_json(bindings, pipeline.release_bindings(repo / "assets", codec, groups[len(bindings)])),
        "compiled inventory must equal the complete installed release pointers")
    expected_paint = {}
    for binding in bindings:
        manifest, _, _ = pipeline.load_set(repo / "assets", binding["identity"]["kind"], codec)
        if fields := body_paint.fields(manifest):
            expected_paint[binding["identity"]["kind"]] = fields
    pipeline.require(pipeline.exact_json(value.get("body_paint_contracts", {}), expected_paint),
                     "compiled body paint contract changed")
    art.static.check_root(repo, Path(value["job_root"]))
    pipeline.require(Path(value["plan_path"]).is_absolute()
        and not Path(value["plan_path"]).is_relative_to(Path(value["job_root"])), "startup plan must remain outside the owned job")
    pipeline.require(value["drivers"] == dependencies() and art.subject(repo) == value["subject"],
                     "normal startup inputs changed while checking")


def plan(args):
    repo, codec = native.validate_repo(str(args.repo)), args.codec.resolve()
    root = args.job_root.resolve() if args.job_root else native.unique_job_root(repo, "building-default-release")
    pipeline.require(not root.exists() and not args.output.resolve().is_relative_to(root), "fresh root/external plan required")
    value = {"schema_version": 1, "profile": PROFILE, "status": "ready", "repo": str(repo),
        "job_root": str(root), "plan_path": str(args.output.resolve()), "subject": art.subject(repo), "codec": str(codec),
        "codec_sha256": executable.digest(codec), "inventory_sha256": executable.digest(repo / INVENTORY),
        "bindings": read_inventory(repo / INVENTORY), "drivers": dependencies(),
        "numeric_verifiers": pipeline.numeric_verifier_inventory(), "adapter": args.adapter,
        "accepted": False, "promotion_authority": False, "performance_evidence": False}
    value["body_paint_contracts"] = {}
    for binding in value["bindings"]:
        manifest, _, _ = pipeline.load_set(repo / "assets", binding["identity"]["kind"], codec)
        if fields := body_paint.fields(manifest):
            value["body_paint_contracts"][binding["identity"]["kind"]] = fields
    shared.adapter_environment(args.adapter)
    check(value)
    resources = native.resource_snapshot(repo, require_launcher=True)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    value["resources"] = resources
    value["launcher_command"] = ["kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1", "python3", str(Path(__file__).resolve()),
        "run", "--plan", str(args.output.resolve())]
    pipeline.put(args.output, pipeline.canonical(value))
    return value


def environment(parent, view, adapter):
    result = {key: value for key, value in parent.items() if not key.startswith(EXCLUDED_ENV)}
    result.update({"BEVY_ASSET_ROOT": str(view), "HW_WINDOW_BACKEND": "x11", "HW_PRESENT_MODE": "novsync",
                   "WINIT_X11_SCALE_FACTOR": "1", **shared.adapter_environment(adapter)})
    return result


def records(log):
    pipeline.require(len(log.encode()) <= 64 * 1024 * 1024, "startup log cap exceeded")
    return [json.loads(line.split(MARKER, 1)[1]) for line in log.splitlines() if MARKER in line]


def check_records(rows, value, pid):
    pipeline.require(len(rows) == 2 and [row["status"] for row in rows] == ["selected", "active"],
                     "normal startup selection/activation record missing or failed")
    for row in rows:
        pipeline.require(type(row["schema_version"]) is int and row["schema_version"] == 1
            and type(row["process_id"]) is int and row["process_id"] == pid
            and row["source"] == "compiled_default" and row["inventory_sha256"] == value["inventory_sha256"]
            and pipeline.exact_json(row["bindings"], value["bindings"])
            and row["accepted"] is False and row["promotion_authority"] is False
            and row["performance_evidence"] is False, "default startup record binding differs")
    generations = rows[-1]["detail"]["generations"]
    pipeline.require(len(generations) == len(value["bindings"]), "startup generation count differs")
    for generation, binding in zip(generations, value["bindings"], strict=True):
        pipeline.require(all(pipeline.exact_json(generation[key], binding["identity"])
                             for key in ("expected", "active", "descriptor_identity"))
            and generation["locator"] == binding["locator"] and generation["pending"] is None
            and generation["failure"] is None, "startup activation is pending/foreign/failed")
        kind = binding["identity"]["kind"]
        for field, roles in [("meshes", pipeline.KINDS[kind]["mesh_roles"]), ("images", pipeline.KINDS[kind]["image_roles"])]:
            actual = generation[field]
            pipeline.require([item["role"] for item in actual] == roles
                and all(item["resident"] is True and isinstance(item["handle"], str) and item["handle"] for item in actual),
                "startup dependency role/residency differs")
    consumers = rows[-1]["detail"]["consumers"]
    pipeline.require(consumers["scope"] == "existing-normal-world-consumers-only"
        and consumers["spawned_for_audit"] is False and len(consumers["roots"]) <= 256
        and len(consumers["sprites"]) <= 256, "normal startup consumer scope differs")
    by_kind = {binding["identity"]["kind"]: binding["identity"] for binding in value["bindings"]}
    owners = consumers["owners"]
    pipeline.require(len(owners) <= 256 and len({owner["entity"] for owner in owners}) == len(owners),
                     "startup owner inventory is duplicated/unbounded")
    owner_map = {owner["entity"]: owner["kind"] for owner in owners}
    for owner in owners:
        pipeline.require(owner["kind"] in by_kind, "startup owner kind is outside the default inventory")
        if pipeline.KINDS[owner["kind"]]["mesh_roles"]:
            matched = [root for root in consumers["roots"] if root["owner"] == owner["entity"]]
            pipeline.require(len(matched) == 1 and not any(row["owner"] == owner["entity"] for row in consumers["sprites"]),
                             "existing 3D owner has missing/duplicate presentation")
        else:
            targets = owner["sprite_targets"]
            matched = [row for row in consumers["sprites"] if row["owner"] == owner["entity"]]
            pipeline.require(1 <= len(targets) <= 16 and len({target["entity"] for target in targets}) == len(targets)
                and len(matched) == len(targets) and {row["entity"] for row in matched} == {target["entity"] for target in targets}
                and not any(root["owner"] == owner["entity"] for root in consumers["roots"]),
                "existing 2D owner has missing/duplicate Sprite or an unexpected root")
            for target in targets:
                row = next(row for row in matched if row["entity"] == target["entity"])
                pipeline.require(row["target_role"] == target["role"] and (
                    target["role"] == "owner" and row["entity"] == owner["entity"]
                    or target["role"] == "child" and row["entity"] != owner["entity"]
                    and row["parent"] == owner["entity"] and row["layer_kind"] in ("Floor", "Struct", "Deco", "Light")),
                    "startup Sprite is not its declared owner/direct layer child")
    pipeline.require(len({root["owner"] for root in consumers["roots"]}) == len(consumers["roots"])
        and len({row["entity"] for row in consumers["sprites"]}) == len(consumers["sprites"]), "duplicate startup consumer")
    pool_by_kind = {row["expected"]["kind"]: row for row in generations}
    for root in consumers["roots"]:
        pipeline.require(owner_map.get(root["owner"]) == root["kind"]
            and pipeline.KINDS[root["kind"]]["mesh_roles"] and root["owner_exists"] is True and root["owner_kind"] == root["kind"]
            and pipeline.exact_json(root["identity"], by_kind[root["kind"]])
            and pipeline.exact_json(root["expected"], by_kind[root["kind"]])
            and len(root["parts"]) == pipeline.KINDS[root["kind"]]["leaf_count"],
            "normal existing root has not consumed the admitted generation")
        names = (["body", "slot0", "slot1", "slot2", "slot3"] if root["kind"] == "SoulSpa"
                 else pipeline.KINDS[root["kind"]]["mesh_roles"])
        pipeline.require(sorted(leaf["name"] for leaf in root["parts"]) == sorted(names), "normal leaf names differ")
        generation = pool_by_kind[root["kind"]]
        meshes = {item["role"]: item["handle"] for item in generation["meshes"]}
        image_handles = {item["role"]: item["handle"] for item in generation["images"]}
        for leaf in root["parts"]:
            role = "slot" if root["kind"] == "SoulSpa" and leaf["name"].startswith("slot") else leaf["name"]
            expected_image = body_paint.expected_texture(
                {"identity": by_kind[root["kind"]], **value.get("body_paint_contracts", {}).get(root["kind"], {})},
                leaf["name"], image_handles, leaf.get("body_paint"))
            pipeline.require(leaf["mesh"] == meshes[role] and leaf["albedo"] == expected_image,
                             "normal leaf uses fallback/foreign dependency")
            pipeline.require(leaf["parent"] == root["entity"] and leaf["mesh_resident"] is True
                and leaf["albedo_resident"] is True, "normal existing leaf is not resident/owned")
    for sprite in consumers["sprites"]:
        image_handles = {item["handle"] for item in pool_by_kind[sprite["kind"]]["images"]
                         if item["role"].startswith("world")}
        pipeline.require(owner_map.get(sprite["owner"]) == sprite["kind"]
            and sprite["matches_released_world_image"] is True and sprite["resident"] is True
            and sprite["image"] in image_handles, "normal existing sprite still uses fallback/foreign data")
    return consumers


def frozen_plan(args):
    current = pipeline.read(args.plan)
    pipeline.require(pipeline.exact_json(current, args.frozen_plan)
        and Path(args.repo) == Path(current["repo"]) and Path(args.job_root) == Path(current["job_root"])
        and Path(args.plan).resolve() == Path(current["plan_path"]), "normal startup plan changed around admission")
    return current


def verify(root):
    result = pipeline.read(root / "manifest.json")
    value = result["plan"]
    pipeline.require(pipeline.exact_json(value, pipeline.read(Path(value["plan_path"]))), "startup result plan changed")
    executable.validate(Path(result["binary"]["path"]), root, result["binary"]["sha256"])
    check(value)
    pipeline.require(Path(value["job_root"]) == root and result["profile"] == PROFILE
        and result["status"] == "pass" and result["argv"] == [result["binary"]["path"]]
        and result["excluded_environment"] == [] and result["exit_code"] in (0, -15)
        and result["accepted"] is False and result["performance_evidence"] is False
        and result["promotion_authority"] is False, "normal startup launch differs")
    for name, key in [("actual-window.log", "log_sha256"), ("build.log", "build_log_sha256")]:
        pipeline.require(executable.digest(root / name) == result[key], "startup log changed")
    log = (root / "actual-window.log").read_text()
    shared.check_renderer_record(log, value["adapter"], result["renderer"])
    observed = check_records(records(log), value, result["pid"])
    pipeline.require(observed == result["observed_consumers"], "startup consumer record changed")
    screenshot = result["screenshot"]
    image = pipeline.rooted(root, screenshot["path"])
    pipeline.require(screenshot["capture_scope"] == "owned-x11-client" and screenshot["window_pid"] == result["pid"]
        and executable.digest(image) == screenshot["sha256"]
        and native.validate_png_structure(image.read_bytes()) == (1280, 720), "startup owned capture differs")
    pipeline.require(art.asset_view_fingerprint(root / "asset-view") == value["subject"]["assets"],
                     "normal asset view differs from installed assets")
    check(value)
    return result


def run_frozen(args):
    pipeline.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "use registered launcher")
    value = frozen_plan(args)
    check(value)
    repo, root = Path(value["repo"]), Path(value["job_root"])
    root.mkdir(exist_ok=False)
    job = root / "job.json"
    state = {"status": "running", "profile": PROFILE, "pid": os.getpid(), "started_at": native.utc_now()}
    native.atomic_write_json(job, state)
    try:
        parent = native.cargo_environment(repo)
        native.run_command("build", ["python3", "scripts/dev.py", "feedback", "--build-only"], repo=repo,
            env=parent, log_path=root / "build.log", job_file=job, state=state, timeout_seconds=3600)
        frozen_plan(args)
        binary = executable.freeze(repo / "target/debug/bevy_app", root)
        check(value)
        view = root / "asset-view"
        shutil.copytree(repo / "assets", view / "assets")
        env = environment(parent, view, value["adapter"])
        excluded = sorted(key for key in env if key.startswith(EXCLUDED_ENV))
        pipeline.require(not excluded, "dedicated/performance/release override leaked into normal startup")
        frozen_plan(args)
        check(value)
        native.admit_stage_start("actual-window", state=state, job_file=job)
        screenshot = None
        with (root / "actual-window.log").open("x") as output:
            process = subprocess.Popen([str(binary)], cwd=view, env=env, stdout=output,
                stderr=subprocess.STDOUT, start_new_session=True, pass_fds=native.activity_pass_fds(env))
            native.update_state(job, state, child_pid=process.pid, current_stage="actual-window")
            started = time.monotonic()
            try:
                while time.monotonic()-started < 180:
                    pipeline.require(process.poll() is None, "normal startup exited before capture")
                    log = (root / "actual-window.log").read_text()
                    rows = records(log)
                    if len(rows) >= 2:
                        check_records(rows, value, process.pid)
                        screenshot = art.capture(root, process.pid)
                        if screenshot is not None:
                            break
                    native.update_state(job, state, heartbeat_at=native.utc_now())
                    time.sleep(.5)
            finally:
                native.stop_command_process(process)
        pipeline.require(screenshot is not None, "normal startup activation/capture timed out")
        result = {"profile": PROFILE, "status": "pass", "plan": value, "pid": process.pid,
            "binary": {"path": str(binary), "sha256": executable.digest(binary)}, "argv": [str(binary)],
            "excluded_environment": excluded, "exit_code": process.returncode, "screenshot": screenshot,
            "renderer": shared.check_renderer((root / "actual-window.log").read_text(), value["adapter"]),
            "observed_consumers": check_records(records((root / "actual-window.log").read_text()), value, process.pid),
            "log_sha256": executable.digest(root / "actual-window.log"), "build_log_sha256": executable.digest(root / "build.log"),
            "accepted": False, "promotion_authority": False, "performance_evidence": False,
            "not_covered": ["consumers absent from the initial world", "all-kind lifecycle", "art approval", "performance", "release decision"]}
        frozen_plan(args)
        pipeline.put(root / "manifest.json", pipeline.canonical(result))
        frozen_plan(args)
        verify(root)
        native.update_state(job, state, status="valid", child_pid=None, completed_at=native.utc_now())
        return result
    except Exception as error:
        native.update_state(job, state, status="invalid", child_pid=None, failure=str(error))
        raise


@native.activity_locked
def run(args):
    return run_frozen(args)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "run", "verify"))
    for name in ("repo", "codec", "job-root", "output", "plan"):
        parser.add_argument("--"+name, type=Path)
    parser.add_argument("--adapter")
    args = parser.parse_args()
    if args.command == "plan":
        pipeline.require(all((args.repo, args.codec, args.output, args.adapter)), "incomplete normal startup plan")
        result = plan(args)
    elif args.command == "run":
        value = pipeline.read(args.plan)
        args.repo, args.job_root = value["repo"], value["job_root"]
        args.frozen_plan = value
        result = run(args)
    else:
        result = verify(args.job_root)
    native.print_json(result)


if __name__ == "__main__":
    main()
