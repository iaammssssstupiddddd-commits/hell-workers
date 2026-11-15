"""Two actual-window resource-only supplements on an instrumented old reference.

The immutable twelve-run timing/native/RSS reference is verified unchanged.
These two separately identified runs provide resource inventory, never substitute
new timing results or grant art, performance-budget, promotion or release approval.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys

if __package__:
    from . import building_art_static_acceptance as static
    from . import building_resource_inventory as oracle
    from . import building_native_executable as executable
else:
    import building_art_static_acceptance as static
    import building_resource_inventory as oracle
    import building_native_executable as executable

native = static.native
PROFILE = "building-foundation-resource-supplement-v1"
BASE = static.REFERENCE_COMMIT
MODULE = "crates/bevy_app/src/plugins/startup/perf_scenario/building_art_static/"
ALLOWED = {MODULE + "mod.rs", MODULE + "resource_inventory.rs",
           "scripts/perf_tool/arguments.py", "scripts/perf_tool/argument_validation/density.py",
           "scripts/tests/test_building_static_single_leg.py"}
CASES = (("small", 15), ("medium", 60))


def put(path, data):
    native.atomic_write_json(Path(path), data)


def drivers():
    result = {Path(module.__file__).name: static.sha256(Path(module.__file__))
              for module in (sys.modules[__name__], static, oracle, executable, native)}
    result["primary_verifier_harness"] = native.native_harness_fingerprint(Path(__file__).resolve().parents[1])
    return result


def origin(repo):
    result = subprocess.run(["git", "diff", "--name-only", BASE, "HEAD"], cwd=repo,
                            capture_output=True, text=True, check=True)
    native.require(set(result.stdout.splitlines()) == ALLOWED
        and subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"], cwd=repo).returncode == 0
        and native.git_subject(repo) != BASE, "resource supplement origin/scope differs")


def local_identity(repo):
    # The old checkout owns its own harness catalog. Import it in an isolated
    # read-only child; never copy today's harness into a historical subject.
    native.require(not native.git_dirty_paths(repo), "source-local reference is dirty")
    origin(repo)
    driver = repo / "scripts/building_art_static_acceptance.py"
    native.require(static.sha256(driver) == static.REFERENCE_DRIVER_SHA256,
                   "source-local reference helper changed")
    code = ("import sys,json; from pathlib import Path; "
            "sys.path.insert(0,str(Path(sys.argv[1])/'scripts')); "
            "import building_art_static_acceptance as local; "
            "repo=local.native.validate_repo(sys.argv[1]); "
            "local.native.require(not local.native.git_dirty_paths(repo),'reference source is dirty'); "
            "print(json.dumps(local.identity(repo),sort_keys=True))")
    completed = subprocess.run([sys.executable, "-I", "-B", "-c", code, str(repo)], cwd=repo,
                               capture_output=True, text=True, timeout=120, check=False)
    native.require(completed.returncode == 0, "source-local identity failed: " + completed.stderr[-2000:])
    result = json.loads(completed.stdout)
    native.require(set(result) == {"subject_commit", "source_fingerprint", "harness_fingerprint",
                                  "asset_view_fingerprint", "driver_sha256"}
        and result["driver_sha256"] == static.REFERENCE_DRIVER_SHA256,
        "source-local identity contract differs")
    return result


def parent(value):
    root, memory = Path(value["reference_root"]), Path(value["memory_root"])
    native.require(static.sha256(root / "manifest.json") == value["reference_manifest_sha256"],
                   "immutable reference manifest changed")
    manifest, verified = static.verify_reference_parent(root)
    extra = static.verify_memory_small(memory, root)
    native.require(extra["session"]["session_sha256"] == value["memory_session_sha256"],
                   "immutable small Memory session changed")
    native.require(verified["capture_runs"] == 6 and verified["memory_runs"] + extra["memory_runs"] == 6,
                   "twelve-run reference incomplete")
    native.require(native.validate_repo(manifest["repo"]) != native.validate_repo(value["repo"])
        and manifest["identity"]["asset_view_fingerprint"] == value["identity"]["asset_view_fingerprint"]
        and manifest["identity"]["harness_fingerprint"] == value["identity"]["harness_fingerprint"]
        and manifest["identity"]["source_fingerprint"] != value["identity"]["source_fingerprint"],
        "resource supplement must be a distinct source with exact baseline assets")
    return manifest, verified, extra


def check_plan(value):
    native.require(value.get("profile") == PROFILE and value.get("status") == "ready"
        and value.get("accepted") is False and value.get("promotion_authority") is False
        and value.get("performance_budget_evidence") is False and value.get("drivers") == drivers(),
        "resource supplement scope/dependencies differ")
    repo, root = native.validate_repo(value["repo"]), Path(value["job_root"])
    static.check_root(repo, root)
    native.require(local_identity(repo) == value["identity"], "source-local reference identity changed")
    return parent(value)


def plan(args):
    repo = native.validate_repo(args.repo)
    root = args.job_root.resolve()
    static.check_root(repo, root)
    native.require(not root.exists() and not args.output.resolve().is_relative_to(root), "fresh root/external plan required")
    reference = args.reference_root.resolve()
    extra = static.verify_memory_small(args.memory_root.resolve(), reference)
    value = {"profile": PROFILE, "status": "ready", "repo": str(repo), "job_root": str(root),
        "identity": local_identity(repo), "reference_root": str(reference), "memory_root": str(args.memory_root.resolve()),
        "reference_manifest_sha256": static.sha256(reference / "manifest.json"),
        "memory_session_sha256": extra["session"]["session_sha256"], "drivers": drivers(),
        "adapter": args.adapter, "accepted": False, "promotion_authority": False, "performance_budget_evidence": False}
    check_plan(value)
    resources = native.resource_snapshot(repo, require_launcher=True)
    native.require(not resources["failures"], str(resources["failures"]))
    value["resources"] = resources
    value["launcher_command"] = ["kitty", "--directory", str(repo), "--detach", "env",
        "HW_NATIVE_ACCEPTANCE_LAUNCHED=1", "PYTHONDONTWRITEBYTECODE=1", "python3", str(Path(__file__).resolve()),
        "run", "--plan", str(args.output.resolve())]
    put(args.output, value)
    return value


def observations(value):
    root = Path(value["job_root"])
    _manifest, old, _memory = check_plan(value)
    results = []
    for size, souls in CASES:
        result = static.verify_session(root, "capture", size, souls, value["identity"], value["adapter"], repeats=1)
        expected = next(row for row in old["sessions"] if row["instrumentation"] == "capture" and row["size"] == size)
        native.require(result["observations"][0]["layout_sha256"] == expected["observations"][0]["layout_sha256"]
            and result["observations"][0]["adapter"] == expected["observations"][0]["adapter"],
            "resource supplement baseline fixture/adapter differs")
        session = root / f"capture-{size}"
        case = static.Case("building-art-static", size, "gpu", static.SEED, souls, 0)
        raw = session / "cases" / case.identifier / "run-001/data/building_resource_inventory.json"
        payload = oracle.boundaries(native.read_json(raw))
        oracle.require_boundary_scale(payload, size)
        results.append({"size": size, "session_sha256": result["session_sha256"],
            "binary_sha256": result["binary_sha256"], "resource_sha256": static.sha256(raw), "inventory": payload})
    native.require(results[0]["binary_sha256"] == results[1]["binary_sha256"], "supplement binary drift")
    plateau = oracle.legacy_boundary_growth(results[0]["inventory"], results[1]["inventory"])
    check_plan(value)
    return results, plateau


def verify(root):
    result = native.read_json(root / "manifest.json")
    value = result["plan"]
    native.require(Path(value["job_root"]) == root and result["profile"] == PROFILE and result["status"] == "pass",
                   "resource supplement manifest differs")
    executable.validate(Path(result["binary"]["path"]), root, result["binary"]["sha256"])
    rows, plateau = observations(value)
    native.require(result["observations"] == rows and result["plateau"] == plateau
        and rows[0]["binary_sha256"] == result["binary"]["sha256"]
        and result["accepted"] is False and result["promotion_authority"] is False
        and result["performance_budget_evidence"] is False, "resource supplement result differs from raw")
    return {"status": "pass", "profile": PROFILE, "source": value["identity"], "capture_runs": 2,
        "parent_timing_runs": 12, "resource_only": True, "accepted": False, "promotion_authority": False,
        "performance_budget_evidence": False, "plateau": plateau}


@native.activity_locked
def run(args):
    native.require(os.environ.get("HW_NATIVE_ACCEPTANCE_LAUNCHED") == "1", "use registered no-prompt launcher")
    value = native.read_json(args.plan)
    native.require(value == args.frozen_plan, "resource plan changed across activity admission")
    check_plan(value)
    repo, root = Path(value["repo"]), Path(value["job_root"])
    resources = native.resource_snapshot(repo, require_launcher=False)
    native.require(not resources["failures"], str(resources["failures"]))
    root.mkdir(parents=True, exist_ok=False)
    job = root / "job.json"
    state = {"profile": PROFILE, "status": "running", "pid": os.getpid(),
             "started_at": native.utc_now(), "heartbeat_at": native.utc_now()}
    put(job, state)
    env = native.cargo_environment(repo)
    env["RUST_LOG"] = "info,wgpu=error"
    try:
        binary = None
        for size, souls in CASES:
            check_plan(value)
            native.require(native.read_json(args.plan) == value, "resource plan changed")
            argv = static.command(root, "capture", size, souls, value["adapter"])
            argv[argv.index("--repeat") + 1] = "1"
            argv.append("--building-static-single-leg")
            native.run_command(f"capture-{size}", argv, repo=repo, env=env,
                log_path=root / f"capture-{size}.log", job_file=job, state=state, timeout_seconds=3600)
            actual = repo / "target/profiling/bevy_app"
            if binary is None:
                frozen = executable.freeze(actual, root)
                binary = {"path": str(frozen), "sha256": executable.digest(frozen)}
            else:
                native.require(static.sha256(actual) == binary["sha256"], "resource binary changed between sizes")
        rows, plateau = observations(value)
        native.require(native.read_json(args.plan) == value, "resource plan changed before manifest")
        result = {"profile": PROFILE, "status": "pass", "plan": value, "binary": binary,
            "observations": rows, "plateau": plateau, "accepted": False, "promotion_authority": False,
            "performance_budget_evidence": False,
            "unverified": ["new fallback/candidate increment", "budget acceptance", "art", "release"]}
        put(root / "manifest.json", result)
        verify(root)
        native.update_state(job, state, status="valid", child_pid=None, completed_at=native.utc_now())
        return result
    except Exception as error:
        native.update_state(job, state, status="invalid", child_pid=None, failure=str(error))
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "run", "verify"))
    for name in ("repo", "job-root", "output", "reference-root", "memory-root", "plan"):
        parser.add_argument("--" + name, type=Path)
    parser.add_argument("--adapter")
    args = parser.parse_args()
    if args.command == "plan":
        native.require(all((args.repo, args.job_root, args.output, args.reference_root, args.memory_root,
                            args.adapter and args.adapter.strip())), "resource plan inputs missing")
        result = plan(args)
    elif args.command == "run":
        native.require(args.plan is not None, "resource plan required")
        args.frozen_plan = native.read_json(args.plan)
        args.repo, args.job_root = args.frozen_plan["repo"], args.frozen_plan["job_root"]
        result = run(args)
    else:
        native.require(args.job_root is not None, "resource job root required")
        result = verify(args.job_root.resolve())
    print(json.dumps(result, sort_keys=True, allow_nan=False))


if __name__ == "__main__":
    main()
