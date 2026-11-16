"""Seeded Tank generation transition observations, without release or input approval."""
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
import time

if __package__:
    from . import building_art_acceptance as art
    from . import building_m2_native_acceptance as m2
else:
    import building_art_acceptance as art
    import building_m2_native_acceptance as m2

pipeline, native = art.pipeline, art.native
PROFILE = "tank-generation-feedback-v1"
G3_PATH = "crates/bevy_app/src/assets/building_asset_set/testdata/tank-g3.buildingset"
G3_LOCATOR = "manifests/acceptance-tank-g3.buildingset"
G4_LOCATOR = "manifests/building-tank-v1.buildingset"
IDENTITIES = [
    {"kind": "Tank", "generation": 3, "authority": "release_approved", "manifest_sha256": "3eb6285f60f837280e3d97604ce055e0c651cfa80d2dd95c0937c86f27fd4ba2"},
    {"kind": "Tank", "generation": 4, "authority": "release_approved", "manifest_sha256": "f452eefadd39c9a5b250fb928838356ffd071eec69fdfbdfe0d65e5538e6af61"},
]


def drivers():
    return {str(Path(m.__file__).resolve()): pipeline.digest(Path(m.__file__).read_bytes()) for m in (art, m2, pipeline)} | {
        str(Path(__file__).resolve()): pipeline.digest(Path(__file__).read_bytes())}


def validate_set(root, path, codec, expected):
    text = pipeline.no_symlinks(path).read_text()
    manifest = json.loads(text)
    pipeline.require(manifest["identity"] == expected, "Tank identity differs")
    receipt = pipeline.file_bytes(root, manifest["receipt"]).decode()
    pipeline.codec(codec, "validate", text, receipt)
    provenance = pipeline.rooted(root, f"building_sets/tank/{expected['generation']}")
    pipeline.check_numeric_provenance(provenance, manifest, codec)
    for artifact in manifest["artifacts"]:
        pipeline.file_bytes(root, artifact)
    for relative, key in (("provenance/source", "source_sha256"), ("provenance/geometry.json", "geometry_contract_sha256"),
                          ("provenance/art-approval.json", "art_approval_sha256")):
        pipeline.require(pipeline.digest(pipeline.rooted(provenance, relative).read_bytes()) == manifest[key],
                         "Tank provenance differs")
    pipeline.check_art_approval(pipeline.read(provenance / "provenance/art-approval.json"),
                                pipeline.preview_projection(manifest, codec)["identity"])
    return manifest


def check_plan(value):
    pipeline.require(value.get("profile") == PROFILE and value.get("scope") == "seeded-generation-observation-only"
        and value.get("accepted") is False and value.get("promotion_authority") is False
        and value.get("identities") == IDENTITIES and value.get("drivers") == drivers()
        and pipeline.exact_json(value.get("numeric_verifiers"), pipeline.numeric_verifier_inventory()), "generation contract differs")
    repo = native.validate_repo(value["repo"])
    art.validate_subject(value["subject"], feedback=True)
    pipeline.require(art.subject(repo) == value["subject"], "generation source changed")
    codec = pipeline.no_symlinks(Path(value["codec"]))
    pipeline.require(pipeline.digest(codec.read_bytes()) == value["codec_sha256"], "generation codec changed")
    for path, identity in ((repo / G3_PATH, IDENTITIES[0]), (repo / "assets" / G4_LOCATOR, IDENTITIES[1])):
        validate_set(repo / "assets", path, codec, identity)
    pipeline.require(value["drivers"] == drivers() and art.subject(repo) == value["subject"], "generation inputs changed")
    m2.adapter_environment(value["adapter"])
    art.static.check_root(repo, pipeline.no_symlinks(Path(value["job_root"])))


def plan(args):
    repo, codec = native.validate_repo(str(args.repo)), pipeline.no_symlinks(args.codec.resolve())
    root = args.job_root.absolute() if args.job_root else native.unique_job_root(repo, "tank-generation-feedback")
    pipeline.require(not root.exists() and not args.output.absolute().is_relative_to(root), "fresh root required")
    value = {"schema_version": 1, "profile": PROFILE, "scope": "seeded-generation-observation-only", "status": "ready",
        "repo": str(repo), "job_root": str(root), "subject": art.subject(repo), "codec": str(codec),
        "codec_sha256": pipeline.digest(codec.read_bytes()), "identities": IDENTITIES, "drivers": drivers(),
        "numeric_verifiers": pipeline.numeric_verifier_inventory(), "adapter": args.adapter,
        "accepted": False, "promotion_authority": False}
    check_plan(value)
    resources = native.resource_snapshot(repo, require_launcher=True)
    pipeline.require(not resources["failures"], str(resources["failures"]))
    value["resources"] = resources
    value["launcher_command"] = ["kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1", "python3", str(Path(__file__).resolve()),
        "run", "--plan", str(args.output.absolute())]
    pipeline.put(args.output, pipeline.canonical(value))
    return value


def check_snapshot(snapshot, phase, nonce, pid):
    pipeline.require(type(pid) is int and pid > 0 and isinstance(nonce, str) and re.fullmatch(r"[0-9a-f]{32}", nonce)
        and type(phase) is int and 0 <= phase <= 4
        and snapshot.get("schema_version") == 1 and snapshot.get("nonce") == nonce
        and snapshot.get("pid") == pid and snapshot.get("phase") == phase and type(snapshot.get("stable")) is int
        and snapshot["stable"] >= 30 and snapshot.get("failure") is None
        and snapshot.get("pending") == (IDENTITIES[1] if phase == 2 else None)
        and snapshot.get("retired_gone") is True and snapshot.get("state") == "Some(TankPartial)"
        and snapshot.get("active") == ([None, IDENTITIES[0], IDENTITIES[0], IDENTITIES[1], None][phase])
        and snapshot.get("root_identity") == snapshot.get("active")
        and all(isinstance(snapshot.get(key), str) and snapshot[key] for key in ("owner", "root")), "generation checkpoint differs")
    parts = snapshot.get("parts")
    pipeline.require(isinstance(parts, list) and len(parts) == (1 if phase in (0, 4) else 2)
        and all(isinstance(p.get("entity"), str) and p["entity"] and isinstance(p.get("mesh"), str) and p["mesh"]
                and p.get("visible") is True and p.get("mesh_resident") is True and p.get("layer_3d") is True for p in parts)
        and len({p["entity"] for p in parts}) == len(parts),
        "generation parts are hidden or nonresident")
    if phase in (1, 2, 3):
        handles = snapshot.get("handles") or {}
        pipeline.require(all(isinstance(handles.get(k), str) and handles[k] for k in ("body", "water", "albedo", "world_preview"))
            and {p.get("name") for p in parts} == {"body", "water"}
            and all(p.get("mesh") == handles.get(p["name"]) and p.get("albedo_resident") is True
                    and p.get("albedo") == handles.get("world_preview" if phase == 3 and p["name"] == "body" else "albedo")
                    and p.get("opaque") is True and p.get("lit") is True
                    and p.get("translation") == ([0., 12., 0.] if p["name"] == "water" else [0., 0., 0.])
                    and p.get("rotation") == [0., 0., 0., 1.] and p.get("scale") == [1., 1., 1.]
                    and isinstance(p.get("body_paint"), dict)
                    and abs(p["body_paint"].get("mode", -99) - (3. if phase == 3 and p["name"] == "body" else .82)) < .0001
                    for p in parts),
            "generation descriptor/parts differ")
    owner, root = snapshot.get("owner_pos"), snapshot.get("root_pos")
    pipeline.require(isinstance(owner, list) and len(owner) == 3 and isinstance(root, list) and len(root) == 3
        and all(type(x) in (int, float) and math.isfinite(x) for x in owner + root)
        and abs(root[0] - owner[0]) < .001 and abs(root[2] + owner[1]) < .001
        and abs(root[1] - (12.8 if phase in (0, 4) else 0.)) < .001, "generation root pose differs")
    camera = snapshot.get("camera") or {}
    position = camera.get("position")
    pipeline.require(isinstance(position, list) and len(position) == 3
        and all(type(n) in (int, float) and math.isfinite(n) for n in position)
        and abs(position[0] - owner[0]) < .001 and abs(position[1] - owner[1]) < .001
        and camera.get("scale") == [1., 1., 1.] and camera.get("zoom") == 1., "generation camera differs")


def check_sequence(shots, nonce, pid, *, complete=True):
    pipeline.require(len(shots) == 5 if complete else 1 <= len(shots) <= 5,
                     "generation checkpoint count differs")
    previous = None
    for phase, shot in enumerate(shots):
        before, after = shot["before"], shot["after"]
        check_snapshot(before, phase, nonce, pid)
        check_snapshot(after, phase, nonce, pid)
        pipeline.require({k: v for k, v in before.items() if k != "stable"} ==
                         {k: v for k, v in after.items() if k != "stable"} and after["stable"] > before["stable"],
                         "generation changed during capture")
        if previous is not None:
            pipeline.require(before["owner"] == previous["owner"] and before["root"] == previous["root"]
                and before["owner_pos"] == previous["owner_pos"]
                and before["root_pos"][0::2] == previous["root_pos"][0::2]
                and before["camera"] == previous["camera"]
                and (phase != 2 or (before["parts"] == previous["parts"] and before["handles"] == previous["handles"]))
                and ({p["entity"] for p in before["parts"]} == {p["entity"] for p in previous["parts"]}
                     if phase == 2 else {p["entity"] for p in before["parts"]}.isdisjoint({p["entity"] for p in previous["parts"]})),
                "generation replaced owner/root or retained old parts")
        previous = before


def verify(root):
    result = pipeline.read(root / "manifest.json")
    value = result["plan"]
    check_plan(value)
    pipeline.require(Path(value["job_root"]) == root and result["status"] == "pass"
        and result["accepted"] is False and result["promotion_authority"] is False
        and result.get("termination") == "bounded-stop" and result.get("exit_code") in (0, -15), "generation result differs")
    check_sequence(result["shots"], result["nonce"], result["pid"])
    for shot in result["shots"]:
        for key in ("before", "after", "ack"):
            raw = pipeline.rooted(root, shot[key + "_path"]).read_bytes()
            pipeline.require(pipeline.digest(raw) == shot[key + "_sha256"] and json.loads(raw) == shot[key],
                             "generation raw observation or acknowledgement differs")
        pipeline.require(shot["ack"] == {"nonce": result["nonce"], "phase": shot["before"]["phase"], "pid": result["pid"]},
                         "generation acknowledgement differs")
        image = pipeline.rooted(root, shot["path"])
        m2.check_capture_pixels(image)
        pipeline.require(shot["capture_scope"] == "owned-x11-client" and shot["window_pid"] == result["pid"]
            and pipeline.digest(image.read_bytes()) == shot["sha256"]
            and native.validate_png_structure(image.read_bytes()) == (1280, 720), "generation screenshot differs")
    log = (root / "actual-window.log").read_text()
    pipeline.require(pipeline.digest(log.encode()) == result["log_sha256"], "generation log changed")
    m2.check_renderer_record(log, value["adapter"], result["renderer"])
    view, codec = root / "asset-view/assets", Path(value["codec"])
    for path, identity in ((view / G3_LOCATOR, IDENTITIES[0]), (view / G4_LOCATOR, IDENTITIES[1])):
        manifest = validate_set(view, path, codec, identity)
    m2.check_asset_view(Path(value["repo"]), root / "asset-view", manifest, (view / G4_LOCATOR).read_text(),
        extra_inventory={G3_LOCATOR: pipeline.digest((Path(value["repo"]) / G3_PATH).read_bytes())})
    binary = pipeline.no_symlinks(Path(value["repo"]) / "target/debug/bevy_app")
    pipeline.require(pipeline.digest(binary.read_bytes()) == result["binary_sha256"], "generation binary changed")
    check_plan(value)
    return {"status": "pass", "scope": value["scope"], "accepted": False, "promotion_authority": False}


@native.activity_locked
def run(args):
    pipeline.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "registered launcher required")
    value = pipeline.read(args.plan)
    check_plan(value)
    repo, root = Path(value["repo"]), Path(value["job_root"])
    root.mkdir(parents=True, exist_ok=False)
    state = {"status": "running", "profile": PROFILE, "pid": os.getpid(), "started_at": native.utc_now(), "heartbeat_at": native.utc_now()}
    job = root / "job.json"
    native.atomic_write_json(job, state)
    try:
        env = native.cargo_environment(repo)
        native.run_command("build", ["python3", "scripts/dev.py", "feedback", "--build-only"], repo=repo, env=env,
                           log_path=root / "build.log", job_file=job, state=state, timeout_seconds=3600)
        binary = repo / "target/debug/bevy_app"
        binary_hash = pipeline.digest(binary.read_bytes())
        check_plan(value)
        view = root / "asset-view"
        shutil.copytree(repo / "assets", view / "assets")
        pipeline.require(art.asset_view_fingerprint(view) == value["subject"]["assets"], "generation asset view differs")
        pipeline.put(view / "assets" / G3_LOCATOR, (repo / G3_PATH).read_bytes())
        for key in list(env):
            if key.startswith(("HW_BUILDING_", "HW_M2", "HW_TANK_GENERATION", "HW_NATIVE_", "HW_PERF_", "HW_WALL", "HW_DOOR", "HW_DISABLE_")):
                del env[key]
        nonce = secrets.token_hex(16)
        status, ack = root / "observation.json", root / "ack.json"
        env.update({"BEVY_ASSET_ROOT": str(view), "HW_WINDOW_BACKEND": "x11", "HW_PRESENT_MODE": "novsync",
            "WINIT_X11_SCALE_FACTOR": "1", **m2.adapter_environment(value["adapter"]),
            "HW_TANK_GENERATION_PROBE": json.dumps({"nonce": nonce, "status_path": str(status),
                                                     "ack_path": str(ack), "identities": IDENTITIES})})
        native.admit_stage_start("actual-window", state=state, job_file=job)
        shots = []
        with (root / "actual-window.log").open("w") as log:
            process = subprocess.Popen([str(binary)], cwd=view, env=env, stdout=log, stderr=subprocess.STDOUT,
                                       start_new_session=True, pass_fds=native.activity_pass_fds(env))
            native.update_state(job, state, child_pid=process.pid, current_stage="actual-window")
            try:
                deadline = time.monotonic() + 180
                while len(shots) < 5 and time.monotonic() < deadline:
                    pipeline.require(process.poll() is None, "generation process exited")
                    native.update_state(job, state, heartbeat_at=native.utc_now())
                    if status.exists():
                        before_bytes = status.read_bytes()
                        before = json.loads(before_bytes)
                        pipeline.require(before.get("failure") is None, str(before.get("failure")))
                        if before.get("phase") == len(shots) and before.get("stable", 0) >= 30:
                            capture_root = root / "checkpoints" / str(len(shots))
                            capture_root.mkdir(parents=True, exist_ok=True)
                            shot = art.capture(capture_root, process.pid)
                            if shot is not None:
                                time.sleep(0.2)
                                after_bytes = status.read_bytes()
                                after = json.loads(after_bytes)
                                shot.update(before=before, after=after, path=str((capture_root / shot["path"]).relative_to(root)))
                                check_sequence(shots + [shot], nonce, process.pid, complete=False)
                                m2.check_capture_pixels(root / shot["path"])
                                ack_value = {"nonce": nonce, "phase": len(shots), "pid": process.pid}
                                ack_bytes = pipeline.canonical(ack_value)
                                for key, payload in (("before", before_bytes), ("after", after_bytes), ("ack", ack_bytes)):
                                    path = capture_root / (key + ".json")
                                    pipeline.put(path, payload)
                                    shot[key + "_path"] = str(path.relative_to(root))
                                    shot[key + "_sha256"] = pipeline.digest(payload)
                                shot["ack"] = ack_value
                                shots.append(shot)
                                pipeline.put(ack, ack_bytes, replace=True)
                    time.sleep(0.2)
            finally:
                native.stop_command_process(process)
        check_sequence(shots, nonce, process.pid)
        pipeline.require(process.returncode in (0, -15), "generation process terminated unexpectedly")
        log_text = (root / "actual-window.log").read_text()
        result = {"schema_version": 1, "status": "pass", "plan": value, "nonce": nonce, "pid": process.pid,
            "shots": shots, "renderer": m2.check_renderer(log_text, value["adapter"]), "log_sha256": pipeline.digest(log_text.encode()),
            "binary_sha256": binary_hash, "termination": "bounded-stop", "exit_code": process.returncode,
            "accepted": False, "promotion_authority": False}
        pipeline.put(root / "manifest.json", pipeline.canonical(result))
        verify(root)
        native.update_state(job, state, status="valid", child_pid=None, completed_at=native.utc_now())
        return result
    except Exception as error:
        native.update_state(job, state, status="invalid", child_pid=None, failure=str(error))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "run", "verify"))
    for name in ("repo", "codec", "job-root", "output", "plan"):
        parser.add_argument("--" + name, type=Path)
    parser.add_argument("--adapter")
    args = parser.parse_args()
    if args.command == "plan":
        pipeline.require(all((args.repo, args.codec, args.output, args.adapter)), "incomplete generation plan")
        result = plan(args)
    elif args.command == "run":
        value = pipeline.read(args.plan)
        args.repo, args.job_root = value["repo"], value["job_root"]
        result = run(args)
    else:
        result = verify(args.job_root)
    native.print_json(result)


if __name__ == "__main__":
    main()
