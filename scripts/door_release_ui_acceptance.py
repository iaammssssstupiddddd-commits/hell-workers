"""Dedicated ordinary-input audit of released Door previews. No release/art authority."""

from __future__ import annotations
import argparse
import copy
import json
import os
import re
from pathlib import Path
import secrets
import subprocess

if __package__:
    from . import building_art_acceptance as art
    from . import door_release_ui_observation as oracle
    from . import building_native_executable as executable
else:
    import building_art_acceptance as art
    import door_release_ui_observation as oracle
    import building_native_executable as executable
import ui_usability_acceptance as ui
import wall_door_joint_acceptance as joint

native, pipeline, static = art.native, art.pipeline, art.static
PROFILE = "door-release-ordinary-planning-v1"


def read(path):
    from scripts.building_group_art_acceptance import read_artifact

    return read_artifact(path)


def hashes():
    modules = (
        Path(__file__),
        Path(oracle.__file__),
        Path(ui.__file__),
        Path(joint.__file__),
        Path(executable.__file__),
    )
    return {p.name: pipeline.digest(p.read_bytes()) for p in modules}


def check(value):
    pipeline.require(
        value["profile"] == PROFILE
        and type(value["schema_version"]) is int
        and value["schema_version"] == 1
        and value["status"] == "ready"
        and value["cases"] == list(oracle.CASES)
        and all(
            value[k] is False
            for k in ("accepted", "promotion_authority", "performance_evidence")
        ),
        "Door planning scope differs",
    )
    repo = Path(value["repo"])
    static.require_identity(repo, value["identity"])
    static.check_root(repo, Path(value["job_root"]))
    pipeline.require(
        pipeline.exact_json(joint.release_identities(repo), value["normal_inventory"])
        and art.asset_view_fingerprint(repo) == value["asset_fingerprint"]
        and hashes() == value["drivers"],
        "Door source/normal inventory/helper changed",
    )


def plan(args):
    repo = native.validate_repo(str(args.repo))
    root = args.job_root.resolve()
    static.check_root(repo, root)
    pipeline.require(
        not root.exists() and not args.output.resolve().is_relative_to(root),
        "fresh Door root and external plan required",
    )
    identities = joint.release_identities(repo)
    pipeline.require(
        identities["door"]["asset_set_generation"] == 7
        and identities["door"]["manifest_sha256"]
        == "4e3f9236db067772b9a60b37ea98437cb29e481f9ec9cbb4d6dcf6517374a449",
        "Door normal release identity differs",
    )
    value = {
        "schema_version": 1,
        "profile": PROFILE,
        "status": "ready",
        "repo": str(repo),
        "job_root": str(root),
        "identity": static.identity(repo),
        "normal_inventory": identities,
        "asset_fingerprint": art.asset_view_fingerprint(repo),
        "drivers": hashes(),
        "cases": list(oracle.CASES),
        "portal_consent_file": str(args.portal_consent_file.resolve()),
        "accepted": False,
        "promotion_authority": False,
        "performance_evidence": False,
    }
    check(value)
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
    pipeline.put(args.output, pipeline.canonical(value))
    return value


def arm(driver):
    driver.last = driver.wait(
        lambda v: v.get("door_release_planning", {}).get("ready") is True,
        after=driver.last["frame"],
    )
    raw = read(driver.root / "door-input.json")
    pipeline.require(
        raw["nonce"] == driver.nonce
        and raw["pid"] == driver.process.pid
        and not raw["events"]
        and not raw["receipts"]
        and raw["failure"] is None
        and raw["arm_frame"] is None,
        "Door pre-arm activity/failure",
    )
    declaration = {
        "schema_version": 1,
        "nonce": driver.nonce,
        "pid": driver.process.pid,
        "world_epoch": driver.last["world_epoch"],
        "observed_frame": driver.last["frame"],
    }
    partial = driver.root / "input-arm.partial"
    with partial.open("xb") as stream:
        stream.write(pipeline.canonical(declaration))
        stream.flush()
        os.fsync(stream.fileno())
    os.link(partial, driver.root / "input-arm.json")
    partial.unlink()
    driver.last = driver.wait(
        lambda v: (
            v.get("door_release_planning", {}).get("input", {}).get("arm_frame")
            is not None
        ),
        after=driver.last["frame"],
    )


def action(driver, label, actions, *, key=None, point=False):
    first = len(driver.events) + 1
    if point:
        pipeline.require(
            driver.last["door_release_planning"]["cursor_grid"]
            == driver.last["door_release_planning"]["anchor"]
            and driver.last["pointer_over_ui"] is False
            and driver.last["world_input_captured"] is False,
            "Door actual point not legal",
        )
    before = copy.deepcopy(driver.last)
    if label == "escape":
        driver.key("Escape")
    elif point:
        driver.send(button=1, pressed=True)
        driver.send(button=1, pressed=False)
    else:
        driver.click(key)

    # Wait for actual message pairs in the passive journal, never resend input.
    def flushed(value):
        raw = read(driver.root / "door-input.json")
        pipeline.require(raw["failure"] is None, "Door actual input journal failed")
        lo = before["door_release_planning"]["real_seconds"]
        hi = value["door_release_planning"]["real_seconds"]
        pair = len([e for e in raw["events"] if lo < e["real_seconds"] <= hi]) == 2
        observed = value["door_release_planning"]
        applied = {
            "dev-minimize": "text-field:DevPoc" not in value["controls"],
            "architect": value["menu_state"] == "Architect",
            "resume": value["paused"] is False,
            "select-door": observed["play_mode"] == "BuildingPlace"
            and observed["build_kind"] == "Door",
            "placement": len(observed["blueprints"]) == 1,
            "escape": observed["play_mode"] == "Normal",
            "pause": value["paused"] is True,
        }[label]
        return pair and applied

    driver.last = driver.wait(flushed, after=driver.last["frame"])
    actions.append(
        {
            "label": label,
            "first_step": first,
            "last_step": len(driver.events),
            "before": before,
            "after": copy.deepcopy(driver.last),
        }
    )
    native.atomic_write_json(driver.root / "actions.json", {"actions": actions})


def exercise(driver, case, identity):
    arm(driver)
    actions = []

    def validate(phase, value):
        return oracle.checkpoint(phase, value, case, identity)

    driver.capture("empty", validate=validate)
    if "text-field:DevPoc" in driver.last["controls"]:
        action(driver, "dev-minimize", actions, key="dev-minimize")
    action(driver, "architect", actions, key="menu:ToggleArchitect")
    driver.capture("catalog", validate=validate)
    # Existing SelectBuild and placement boundaries reject paused input.
    action(driver, "resume", actions, key="menu:SetTimeSpeed(Normal)")
    action(driver, "select-door", actions, key="menu:SelectBuild(Door)")
    # Motion belongs to the next actual placement action's transport interval.
    first = len(driver.events) + 1
    projected = driver.last["door_release_planning"]["point"]
    driver.send(point=tuple(round(n * driver.last["scale_factor"]) for n in projected))
    driver.capture("ghost", validate=validate)
    action(driver, "placement", actions, point=True)
    actions[-1]["first_step"] = first
    native.atomic_write_json(driver.root / "actions.json", {"actions": actions})
    action(driver, "escape", actions)
    action(driver, "pause", actions, key="menu:SetTimeSpeed(Paused)")
    driver.last = driver.wait(
        lambda v: not v["door_release_planning"]["ghosts"], after=driver.last["frame"]
    )
    driver.capture("blueprint", validate=validate)


def verify_case(value, row, binary_sha):
    root = Path(value["job_root"]) / row["directory"]
    case = row["case"]
    pipeline.require(
        root.parent == Path(value["job_root"])
        and case in oracle.CASES
        and type(row["pid"]) is int
        and row["pid"] > 0
        and isinstance(row["nonce"], str)
        and re.fullmatch(r"[0-9a-f]{32}", row["nonce"]) is not None
        and row["binary_sha256"] == binary_sha
        and row["controlled_stop"] is True
        and type(row["exit_code"]) is int
        and row["exit_code"] == -15,
        "Door child/capsule/controlled termination differs",
    )
    raw = read(root / "door-input.json")
    client = read(root / "input-client.json")
    arm = read(root / "input-arm.json")
    pipeline.require(
        set(arm) == {"schema_version", "nonce", "pid", "world_epoch", "observed_frame"}
        and all(
            type(arm[k]) is int
            for k in ("schema_version", "pid", "world_epoch", "observed_frame")
        )
        and arm["schema_version"] == 1
        and arm["nonce"] == row["nonce"]
        and arm["pid"] == row["pid"]
        and arm["world_epoch"] == raw["world_epoch"]
        and 46 <= arm["observed_frame"] <= raw["arm_frame"]
        and raw["arm_frame"] - arm["observed_frame"] <= 120,
        "Door saved input arm owner/clock differs",
    )
    entries = read(root / "checkpoints.json")["checkpoints"]
    inputs = read(root / "events.json")["events"]
    actions = read(root / "actions.json")["actions"]
    pipeline.require(
        client["pid"] == row["pid"]
        and client["nonce"] == row["nonce"]
        and client["input_backend"] == "portal-eis"
        and client["input_transport"] == "portal-eis-absolute-v1"
        and client["geometry"] == [1920, 1080]
        and client["observer_viewport"] == [1920, 1080],
        "Door exact owned input client differs",
    )
    pipeline.require(
        type(client["pid"]) is int
        and type(client["window"]) is int
        and client["window"] > 0
        and type(client["portal_devices"]) is int
        and client["portal_devices"] & 3 == 3
        and isinstance(client.get("portal_session"), str)
        and client["portal_session"],
        "Door portal ownership/devices differs",
    )
    ui.check_input_client({"input_backend": "portal-eis", "case": PROFILE}, row, client)
    for entry in entries:
        pipeline.require(
            entry["pid"] == row["pid"]
            and entry["window"] == client["window"]
            and entry["capture_scope"] == "x11-client-window"
            and entry["screenshot"] == entry["case"] + ".png",
            "Door capture owner/path differs",
        )
        path = root / entry["screenshot"]
        metadata = path.lstat()
        pipeline.require(
            not path.is_symlink()
            and path.is_file()
            and metadata.st_size <= 1920 * 1080 * 5 + 65536,
            "Door PNG must be a bounded regular file",
        )
        data = path.read_bytes()
        pipeline.require(
            not path.is_symlink()
            and pipeline.digest(data) == entry["sha256"]
            and native.validate_png_structure(data) == (1920, 1080),
            "Door owned PNG changed",
        )
    joint.verify_game_log(root / "game.log", "Intel")
    return oracle.verify_inputs(
        raw, actions, inputs, client, entries, case, value["normal_inventory"]["door"]
    )


def verify(root):
    manifest = read(root / "manifest.json")
    value = manifest["plan"]
    check(value)
    pipeline.require(
        str(root) == value["job_root"]
        and manifest["profile"] == PROFILE
        and manifest["status"] == "pass"
        and all(
            manifest[k] is False
            for k in ("accepted", "promotion_authority", "performance_evidence")
        ),
        "Door manifest scope differs",
    )
    binary = Path(manifest["binary"]["path"])
    executable.validate(binary, root, manifest["binary"]["sha256"])
    pipeline.require(
        [r["case"] for r in manifest["sessions"]] == list(oracle.CASES)
        and [r["directory"] for r in manifest["sessions"]]
        == [f"case-{i}" for i in range(3)],
        "Door three-case matrix incomplete",
    )
    portal_sessions = [
        read(root / row["directory"] / "input-client.json")["portal_session"]
        for row in manifest["sessions"]
    ]
    pipeline.require(
        isinstance(manifest["portal_session"], str)
        and manifest["portal_session"]
        and all(session == manifest["portal_session"] for session in portal_sessions),
        "Door suite did not use its one owned portal session",
    )
    observations = [
        verify_case(value, row, manifest["binary"]["sha256"])
        for row in manifest["sessions"]
    ]
    pipeline.require(
        pipeline.exact_json(observations, manifest["observations"]),
        "Door saved observation differs",
    )
    return manifest


@native.activity_locked
def run(args):
    pipeline.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "use registered no-prompt launcher",
    )
    value = read(args.plan)
    pipeline.require(
        pipeline.exact_json(value, args.frozen_plan),
        "Door plan changed across admission",
    )
    check(value)
    repo = Path(value["repo"])
    root = Path(value["job_root"])
    root.mkdir(exist_ok=False)
    job = root / "job.json"
    state = {
        "profile": PROFILE,
        "status": "running",
        "heartbeat_at": native.utc_now(),
        "child_pid": None,
    }
    native.atomic_write_json(job, state)
    portal = None
    try:
        native.run_command(
            "build",
            ["python3", "scripts/dev.py", "feedback", "--build-only"],
            repo=repo,
            env=native.cargo_environment(repo),
            log_path=root / "build.log",
            job_file=job,
            state=state,
            timeout_seconds=3600,
        )
        check(value)
        binary = executable.freeze(repo / "target/debug/bevy_app", root)
        binary_sha = pipeline.digest(binary.read_bytes())

        def heartbeat(method):
            native.update_state(
                job,
                state,
                current_stage="portal-" + method,
                heartbeat_at=native.utc_now(),
            )

        portal = ui.PortalSession(heartbeat, consent_file=value["portal_consent_file"])
        sessions = []
        for index, case in enumerate(value["cases"]):
            check(value)
            pipeline.require(
                pipeline.exact_json(read(args.plan), value),
                "Door plan changed before child",
            )
            directory = root / f"case-{index}"
            directory.mkdir()
            nonce = secrets.token_hex(16)
            env = ui.game_environment(repo)
            for key in tuple(env):
                if key.startswith(
                    (
                        "HW_NATIVE_",
                        "HW_PERF_",
                        "HW_WALL_",
                        "HW_DOOR_",
                        "HW_BUILDING_",
                        "HW_M2_",
                        "HW_BRIDGE_",
                    )
                ):
                    env.pop(key, None)
            env.update(
                BEVY_ASSET_ROOT=str(repo),
                HW_WINDOW_BACKEND="x11",
                HW_PRESENT_MODE="novsync",
                WGPU_BACKEND="vulkan",
                WGPU_ADAPTER_NAME="Intel",
                HELL_WORKERS_WORLDGEN_SEED="20260914",
                HW_NATIVE_UI_ROOT=str(directory),
                HW_NATIVE_UI_NONCE=nonce,
                HW_NATIVE_UI_WIDTH="1920",
                HW_NATIVE_UI_HEIGHT="1080",
                HW_NATIVE_UI_SCALE="1",
                HW_NATIVE_UI_CASE=case,
            )
            native.admit_stage_start(case, state=state, job_file=job)
            driver = None
            controlled = False
            with (directory / "game.log").open("x") as log:
                process = subprocess.Popen(
                    [str(binary)],
                    cwd=repo,
                    env=env,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    start_new_session=True,
                    pass_fds=native.activity_pass_fds(env),
                )
                try:
                    native.update_state(
                        job, state, child_pid=process.pid, current_stage=case
                    )
                    driver = ui.Driver(
                        process,
                        directory,
                        nonce,
                        state,
                        job,
                        portal,
                        eis=True,
                        fresh_ack=True,
                    )
                    exercise(driver, case, value["normal_inventory"]["door"])
                    pipeline.require(
                        process.poll() is None,
                        "Door child exited before declared controlled stop",
                    )
                    controlled = True
                finally:
                    try:
                        if driver and driver.input:
                            driver.input.close()
                    finally:
                        if process.poll() is None:
                            native.stop_command_process(process)
            row = {
                "case": case,
                "directory": directory.name,
                "pid": process.pid,
                "nonce": nonce,
                "binary_sha256": binary_sha,
                "controlled_stop": controlled,
                "exit_code": process.returncode,
            }
            verify_case(value, row, binary_sha)
            sessions.append(row)
        check(value)
        pipeline.require(
            pipeline.exact_json(read(args.plan), value),
            "Door plan changed before final verification",
        )
        observations = [verify_case(value, row, binary_sha) for row in sessions]
        manifest = {
            "profile": PROFILE,
            "status": "pass",
            "plan": value,
            "binary": {"path": str(binary), "sha256": binary_sha},
            "sessions": sessions,
            "observations": observations,
            "portal_session": portal.session,
            "portal_persistence": portal.persistence,
            "accepted": False,
            "promotion_authority": False,
            "performance_evidence": False,
        }
        pipeline.put(root / "manifest.json", pipeline.canonical(manifest))
        verify(root)
        native.update_state(
            job, state, status="valid", child_pid=None, completed_at=native.utc_now()
        )
        return manifest
    except BaseException as error:
        native.update_state(
            job, state, status="invalid", child_pid=None, failure=str(error)
        )
        raise
    finally:
        if portal:
            portal.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    p = commands.add_parser("plan")
    p.add_argument("--repo", type=Path, required=True)
    p.add_argument("--job-root", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--portal-consent-file", type=Path, required=True)
    p = commands.add_parser("run")
    p.add_argument("--plan", type=Path, required=True)
    p = commands.add_parser("verify")
    p.add_argument("--root", type=Path, required=True)
    args = parser.parse_args()
    if args.action == "run":
        args.frozen_plan = read(args.plan)
        args.repo = Path(args.frozen_plan["repo"])
        args.job_root = Path(args.frozen_plan["job_root"])
    result = (
        plan(args)
        if args.action == "plan"
        else run(args)
        if args.action == "run"
        else verify(args.root)
    )
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
