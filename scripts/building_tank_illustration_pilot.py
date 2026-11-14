"""Closed Tank body-paint projection comparison; no release/art/performance authority."""

from __future__ import annotations
import argparse
import json
import io
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import time
from PIL import Image

try:
    from . import building_release_art_acceptance as transport
    from . import building_tank_illustration_observation as oracle
except ImportError:
    import building_release_art_acceptance as transport
    import building_tank_illustration_observation as oracle

m6, native, pipeline, static, executable = (
    transport.m6,
    transport.native,
    transport.pipeline,
    transport.static,
    transport.executable,
)
PROFILE = oracle.PROFILE
ENV = "HW_BUILDING_TANK_ILLUSTRATION_PILOT"
PAINT_PATH = "pilot/tank-body.png"
read_artifact, label, matrix = (
    transport.read_artifact,
    transport.label,
    transport.matrix,
)
command, build_command = transport.command, transport.build_command
capture, observe_window, publish_ack = (
    transport.capture,
    transport.observe_window,
    transport.publish_ack,
)


def prepare_paint(path):
    """Deterministic texture import, not an art edit: matte coverage then minify.

    Composite straight-alpha source over a wood matte before filtering so hidden
    black RGB cannot bleed into an opaque geometry edge. The runtime image is
    only 256px; the editable generated original remains unchanged in docs.
    """
    pipeline.require(
        path.is_absolute()
        and path.resolve() == path
        and path.is_file()
        and not path.is_symlink()
        and path.stat().st_size <= 32 * 1024 * 1024,
        "invalid pilot paint path",
    )
    raw = path.read_bytes()
    with Image.open(io.BytesIO(raw)) as image:
        pipeline.require(
            image.format == "PNG"
            and image.mode == "RGBA"
            and max(image.size) <= 2048
            and min(image.size) > 0,
            "pilot paint must be a bounded RGBA PNG",
        )
        background = Image.new("RGBA", image.size, (99, 86, 79, 255))
        prepared = Image.alpha_composite(background, image).resize(
            (256, 256), Image.Resampling.LANCZOS
        )
        output = io.BytesIO()
        prepared.save(output, format="PNG", compress_level=9)
        encoded = output.getvalue()
        spec = {
            "path": PAINT_PATH,
            "png_sha256": pipeline.digest(encoded),
            "rgba_sha256": pipeline.digest(prepared.tobytes()),
            "dimensions": [256, 256],
            "canvas_wu": [96.0, 96.0],
            "anchor_uv": [0.5, 0.625],
            "matte_linear_rgb": [0.125, 0.09375, 0.078125],
            "body_glb_sha256": "d14c32ac93d23db22d3158b0b02b202377d4e48c9326e26b18d8c6a001f2cf28",
        }
        return spec, encoded


def paint_input(path):
    return prepare_paint(path)[0]


def driver_inventory():
    paths = [
        Path(__file__),
        Path(oracle.__file__),
        Path(transport.__file__),
        Path(oracle.geometry.__file__),
        Path(oracle.common.__file__),
    ]
    return {
        **transport.driver_inventory(),
        **{p.name: pipeline.digest(p.read_bytes()) for p in paths},
    }


def check(value):
    pipeline.require(
        value["schema_version"] == 1
        and type(value["schema_version"]) is int
        and value["profile"] == PROFILE
        and value["status"] == "ready"
        and all(
            value[k] is False
            for k in ("accepted", "promotion_authority", "performance_evidence")
        ),
        "pilot scope differs",
    )
    pipeline.require(
        set(value)
        <= {
            "schema_version",
            "profile",
            "status",
            "repo",
            "job_root",
            "codec",
            "codec_sha256",
            "identity",
            "assets",
            "asset_files",
            "drivers",
            "adapter",
            "cases",
            "accepted",
            "promotion_authority",
            "performance_evidence",
            "paint",
            "paint_source",
            "paint_source_sha256",
            "resources",
            "launcher_command",
        },
        "foreign pilot plan fields",
    )
    repo = Path(value["repo"])
    static.require_identity(repo, value["identity"])
    static.check_root(repo, Path(value["job_root"]))
    codec = Path(value["codec"])
    pipeline.require(
        codec.is_absolute()
        and codec.resolve() == codec
        and pipeline.digest(codec.read_bytes()) == value["codec_sha256"],
        "pilot codec changed",
    )
    pipeline.require(
        pipeline.exact_json(value["assets"], m6.inventory(repo, codec))
        and pipeline.exact_json(
            value["asset_files"], m6.file_inventory(repo / "assets")
        )
        and value["drivers"] == driver_inventory(),
        "pilot source/normal assets/drivers changed",
    )
    pipeline.require(
        PAINT_PATH not in value["asset_files"], "pilot cannot overwrite a normal asset"
    )
    pipeline.require(
        value["paint"] == paint_input(Path(value["paint_source"]))
        and value["paint_source_sha256"]
        == pipeline.digest(Path(value["paint_source"]).read_bytes())
        and value["cases"] == [list(c) for c in matrix()],
        "pilot paint/view changed",
    )
    tanks = [b for b in value["assets"]["bindings"] if b["identity"]["kind"] == "Tank"]
    pipeline.require(
        len(tanks) == 1
        and tanks[0]["identity"]["generation"] == 3
        and tanks[0]["identity"]["manifest_sha256"]
        == "3eb6285f60f837280e3d97604ce055e0c651cfa80d2dd95c0937c86f27fd4ba2",
        "pilot requires the exact Tank g3",
    )


def plan(args):
    repo = native.validate_repo(str(args.repo))
    codec = args.codec.resolve()
    root = args.job_root.resolve()
    static.check_root(repo, root)
    pipeline.require(
        not root.exists() and not args.output.resolve().is_relative_to(root),
        "fresh pilot root/external plan required",
    )
    value = {
        "schema_version": 1,
        "profile": PROFILE,
        "status": "ready",
        "repo": str(repo),
        "job_root": str(root),
        "codec": str(codec),
        "codec_sha256": pipeline.digest(codec.read_bytes()),
        "identity": static.identity(repo),
        "assets": m6.inventory(repo, codec),
        "asset_files": m6.file_inventory(repo / "assets"),
        "drivers": driver_inventory(),
        "adapter": args.adapter,
        "cases": [list(c) for c in matrix()],
        "accepted": False,
        "promotion_authority": False,
        "performance_evidence": False,
        "paint_source": str(args.paint.resolve()),
        "paint_source_sha256": pipeline.digest(args.paint.resolve().read_bytes()),
        "paint": paint_input(args.paint.resolve()),
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


def session(value, case, root, nonce):
    base = transport.session(value, case, root, nonce)
    return {
        "schema_version": 1,
        "contract_id": PROFILE,
        "gallery": base,
        "paint": value["paint"],
    }


def environment(parent, value, declaration):
    env = {
        k: v
        for k, v in parent.items()
        if not k.startswith(
            (
                "HW_BUILDING_",
                "HW_M2_",
                "HW_BRIDGE_",
                "HW_WALL",
                "HW_DOOR",
                "HW_DISABLE_",
                "HW_PERF_",
                "HW_NATIVE_UI_",
            )
        )
    }
    env.update(
        {
            ENV: json.dumps(declaration),
            "BEVY_ASSET_ROOT": str(Path(value["job_root"]) / "asset-view"),
            "HW_WINDOW_BACKEND": "x11",
            "HW_PRESENT_MODE": "novsync",
            "WGPU_BACKEND": "vulkan",
            "WGPU_ADAPTER_NAME": value["adapter"],
            "RUST_LOG": "info,wgpu=error",
        }
    )
    return env


def check_view(value):
    # m6 file inventory includes sha/size metadata; preserve its exact schema.
    root = Path(value["job_root"]) / "asset-view/assets"
    observed = m6.file_inventory(root)
    paint = observed.pop(PAINT_PATH, None)
    pipeline.require(
        pipeline.exact_json(observed, value["asset_files"]) and paint is not None,
        "pilot view inventory changed",
    )
    pipeline.require(
        pipeline.digest((root / PAINT_PATH).read_bytes())
        == value["paint"]["png_sha256"],
        "pilot view paint changed",
    )


def prepare_view(value):
    root = Path(value["job_root"]) / "asset-view"
    shutil.copytree(Path(value["repo"]) / "assets", root / "assets")
    paint = root / "assets" / PAINT_PATH
    paint.parent.mkdir()
    paint.write_bytes(prepare_paint(Path(value["paint_source"]))[1])
    check_view(value)


def verify_case(value, case, binary_sha):
    root = Path(value["job_root"]) / "cases" / label(case)
    receipt = read_artifact(root / "case.json")
    probe, final, ack = (
        read_artifact(root / name)
        for name in ("probe.json", "final.json", "capture-ack.json")
    )
    pipeline.require(
        isinstance(receipt["nonce"], str)
        and re.fullmatch(r"[0-9a-f]{32}", receipt["nonce"]),
        "gallery nonce differs",
    )
    expected = session(value, case, root, receipt["nonce"])
    shot = receipt["screenshot"]
    pipeline.require(
        all(
            type(obj[key]) is int
            for obj, keys in (
                (receipt, ("exit_code", "pid")),
                (
                    ack,
                    ("schema_version", "pid", "width", "height", "captured_at_unix_ms"),
                ),
                (shot, ("window_pid", "captured_at_unix_ms")),
            )
            for key in keys
        ),
        "gallery receipt/ACK wire integer differs",
    )
    pipeline.require(
        receipt["status"] == "pass"
        and receipt["exit_code"] == 0
        and pipeline.exact_json(receipt["case"], list(case))
        and receipt["binary_sha256"] == binary_sha
        and receipt["pid"] == probe["pid"],
        "gallery process receipt differs",
    )
    oracle.validate(
        probe, final, expected, value, Path(value["job_root"]) / "asset-view"
    )
    shot = receipt["screenshot"]
    png_path = root / "actual-window.png"
    pipeline.require(
        not png_path.is_symlink()
        and png_path.is_file()
        and png_path.stat().st_size <= oracle.WIDTH * oracle.HEIGHT * 5 + 65536,
        "gallery PNG must be bounded regular file",
    )
    png = png_path.read_bytes()
    pipeline.require(
        set(ack)
        == {
            "schema_version",
            "nonce",
            "pid",
            "probe_sha256",
            "png_sha256",
            "window_id",
            "width",
            "height",
            "captured_at_unix_ms",
        }
        and ack["schema_version"] == 1
        and ack["nonce"] == receipt["nonce"]
        and ack["pid"] == receipt["pid"]
        and ack["probe_sha256"]
        == final["probe_sha256"]
        == pipeline.digest((root / "probe.json").read_bytes())
        and final["ack_sha256"]
        == pipeline.digest((root / "capture-ack.json").read_bytes())
        and pipeline.exact_json(final["ack"], ack)
        and ack["png_sha256"] == shot["sha256"] == pipeline.digest(png)
        and ack["window_id"] == shot["window_id"]
        and re.fullmatch(r"0x[0-9a-fA-F]{1,16}", ack["window_id"])
        and int(ack["window_id"], 16) > 0
        and shot["capture_scope"] == "owned-x11-client"
        and shot["path"] == "actual-window.png"
        and type(shot["window_pid"]) is int
        and shot["window_pid"] == receipt["pid"]
        and (ack["width"], ack["height"])
        == native.validate_png_structure(png)
        == (oracle.WIDTH, oracle.HEIGHT)
        and probe["ready_at_unix_ms"]
        <= ack["captured_at_unix_ms"]
        == shot["captured_at_unix_ms"]
        <= final["completed_at_unix_ms"]
        and final["completed_at_unix_ms"] - ack["captured_at_unix_ms"] <= 30_000,
        "gallery capture/ACK/final binding differs",
    )
    log = (root / "actual-window.log").read_text()
    adapters = re.findall(
        r'AdapterInfo \{ name: "([^"]+)".*?backend: ([A-Za-z0-9_]+)', log
    )
    pipeline.require(
        not re.search(r"\b(?:WARN|ERROR)\b|bevy_ecs::error::handler", log)
        and len(adapters) == 1
        and value["adapter"].casefold() in adapters[0][0].casefold()
        and adapters[0][1] == "Vulkan",
        "gallery native log/adapter invalid",
    )
    return {
        "case": list(case),
        "adapter": adapters[0][0],
        "screenshot_sha256": shot["sha256"],
    }


def verify(root):
    manifest = read_artifact(root / "manifest.json")
    value = manifest["plan"]
    pipeline.require(
        str(root) == value["job_root"]
        and manifest["profile"] == PROFILE
        and manifest["status"] == "pass"
        and manifest["accepted"] is False
        and manifest["performance_evidence"] is False
        and manifest["promotion_authority"] is False,
        "gallery manifest scope differs",
    )
    check(value)
    check_view(value)
    binary = Path(manifest["binary"]["path"])
    executable.validate(binary, root, manifest["binary"]["sha256"])
    rows = [
        verify_case(value, case, manifest["binary"]["sha256"])
        for case in value["cases"]
    ]
    pipeline.require(
        rows == manifest["observations"] and len({row["adapter"] for row in rows}) == 1,
        "gallery matrix actual environment differs",
    )
    check(value)
    check_view(value)
    return manifest


@native.activity_locked
def run(args):
    pipeline.require(
        os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1",
        "use registered no-prompt launcher",
    )
    value = pipeline.read(args.plan)
    pipeline.require(
        pipeline.exact_json(value, args.frozen_plan),
        "plan changed across activity admission",
    )
    check(value)
    repo, root = Path(value["repo"]), Path(value["job_root"])
    root.mkdir(parents=True, exist_ok=False)
    state = {
        "profile": PROFILE,
        "status": "running",
        "pid": os.getpid(),
        "started_at": native.utc_now(),
        "heartbeat_at": native.utc_now(),
    }
    job = root / "job.json"
    native.atomic_write_json(job, state)
    try:
        env = native.cargo_environment(repo)
        build = build_command(value)
        native.run_command(
            "build",
            build,
            repo=repo,
            env=env,
            log_path=root / "build.log",
            job_file=job,
            state=state,
            timeout_seconds=3600,
        )
        check(value)
        binary = executable.freeze(repo / "target/profiling/bevy_app", root)
        binary_sha = pipeline.digest(binary.read_bytes())
        prepare_view(value)
        rows = []
        for case in value["cases"]:
            pipeline.require(
                pipeline.exact_json(pipeline.read(args.plan), value),
                "gallery plan changed before child launch",
            )
            check(value)
            check_view(value)
            case_root = root / "cases" / label(case)
            case_root.mkdir(parents=True, exist_ok=False)
            nonce = secrets.token_hex(16)
            declaration = session(value, case, case_root, nonce)
            game_env = environment(env, value, declaration)
            native.admit_stage_start(label(case), state=state, job_file=job)
            screenshot = None
            deadline = time.monotonic() + 240
            with (case_root / "actual-window.log").open("x") as log:
                process = subprocess.Popen(
                    command(binary, case_root, case),
                    cwd=repo,
                    env=game_env,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    start_new_session=True,
                    pass_fds=native.activity_pass_fds(game_env),
                )
                native.update_state(
                    job, state, child_pid=process.pid, current_stage=label(case)
                )
                try:
                    while process.poll() is None:
                        if screenshot is None:
                            observe_window(case_root, process.pid, nonce)
                        if screenshot is None and (case_root / "probe.json").exists():
                            probe = read_artifact(case_root / "probe.json")
                            pipeline.require(
                                probe["pid"] == process.pid
                                and probe["session"] == declaration
                                and probe["status"] == "ready"
                                and probe["stable_frames"] >= 60,
                                "foreign/early gallery probe",
                            )
                            # Let the prepared frame reach the actual compositor before capture.
                            time.sleep(1)
                            screenshot = capture(case_root, process.pid)
                            publish_ack(
                                case_root,
                                {
                                    "schema_version": 1,
                                    "nonce": nonce,
                                    "pid": process.pid,
                                    "probe_sha256": pipeline.digest(
                                        (case_root / "probe.json").read_bytes()
                                    ),
                                    "png_sha256": screenshot["sha256"],
                                    "window_id": screenshot["window_id"],
                                    "width": oracle.WIDTH,
                                    "height": oracle.HEIGHT,
                                    "captured_at_unix_ms": screenshot[
                                        "captured_at_unix_ms"
                                    ],
                                },
                            )
                        pipeline.require(
                            time.monotonic() < deadline,
                            "group gallery deadline exceeded",
                        )
                        native.update_state(job, state, heartbeat_at=native.utc_now())
                        time.sleep(0.25)
                finally:
                    if process.poll() is None:
                        native.stop_command_process(process)
            pipeline.require(
                process.returncode == 0 and screenshot is not None,
                "group gallery run/capture incomplete",
            )
            receipt = {
                "status": "pass",
                "case": list(case),
                "exit_code": process.returncode,
                "pid": process.pid,
                "nonce": nonce,
                "binary_sha256": binary_sha,
                "screenshot": screenshot,
            }
            pipeline.put(case_root / "case.json", pipeline.canonical(receipt))
            rows.append(verify_case(value, case, binary_sha))
        check(value)
        check_view(value)
        manifest = {
            "profile": PROFILE,
            "status": "pass",
            "plan": value,
            "binary": {"path": str(binary), "sha256": binary_sha},
            "observations": rows,
            "accepted": False,
            "performance_evidence": False,
            "promotion_authority": False,
        }
        pipeline.put(root / "manifest.json", pipeline.canonical(manifest))
        pipeline.require(
            pipeline.exact_json(pipeline.read(args.plan), value),
            "gallery plan changed before verification",
        )
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    p = commands.add_parser("plan")
    p.add_argument("--repo", type=Path, required=True)
    p.add_argument("--codec", type=Path, required=True)
    p.add_argument("--adapter", default="Intel")
    p.add_argument("--paint", type=Path, required=True)
    p.add_argument("--job-root", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p = commands.add_parser("run")
    p.add_argument("--plan", type=Path, required=True)
    p = commands.add_parser("verify")
    p.add_argument("--root", type=Path, required=True)
    args = parser.parse_args()
    if args.action == "run":
        value = pipeline.read(args.plan)
        args.repo, args.job_root = Path(value["repo"]), Path(value["job_root"])
        args.frozen_plan = value
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
